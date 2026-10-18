//! Fonts installed on this machine, offered as document families for Add text (a local build's
//! addition; upstream draws page text only with standard and craft-fonts faces).
//!
//! The font folders are scanned once, on first use, reading only each face's table directory and
//! its `name`, `OS/2` and `cmap` headers. A face's file is read whole the first time text is
//! drawn with it, and stays loaded. Faces whose embedding permissions (`OS/2.fsType`) forbid
//! embedding, subsetting or outline embedding are never listed.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use harfrust::ShaperData;
use skrifa::FontRef;

use crate::{CRAFT_FONTS, CraftFont, ShapedScript};

/// Most font files looked at in one scan, and most faces read from one collection.
const MAX_FILES: usize = 6000;
const MAX_COLLECTION_FACES: usize = 64;
/// The largest font file loaded for drawing.
const MAX_FONT_BYTES: u64 = 96 * 1024 * 1024;
const STYLES: [&str; 4] = ["Regular", "Bold", "Italic", "Bold Italic"];

/// A family of installed faces: its regular, bold, italic and bold italic, as far as they exist.
pub struct InstalledFamily {
    pub name: &'static str,
    /// The font names recorded for its styles: regular, bold, italic, bold italic.
    pub base: [&'static str; 4],
    /// Whether its faces have Thai letters (their character maps cover ก, U+0E01).
    pub thai: bool,
    sources: [Option<(PathBuf, u32)>; 4],
    loaded: [OnceLock<Option<ShapedScript>>; 4],
}

/// A face read from disk for drawing.
pub(crate) struct LoadedFace {
    pub face: (FontRef<'static>, ShaperData),
    pub info: CraftFont,
    /// Its index in its file (a collection holds several).
    pub index: u32,
}

static LOADED: RwLock<Vec<&'static LoadedFace>> = RwLock::new(Vec::new());

/// The loaded face behind `ShapedScript::Craft(CRAFT_FONTS.len() + slot)`.
pub(crate) fn loaded(slot: usize) -> Option<&'static LoadedFace> {
    LOADED.read().ok()?.get(slot).copied()
}

fn disabled() -> bool {
    std::env::var_os("PDFCRAFT_SYSTEM_FONTS").is_some_and(|v| v == "0")
}

fn directories() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(windows) {
        let mut dirs: Vec<PathBuf> = std::env::var_os("WINDIR").map(|p| PathBuf::from(p).join("Fonts")).into_iter().collect();
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local).join("Microsoft").join("Windows").join("Fonts"));
        }
        dirs
    } else if cfg!(target_os = "macos") {
        let mut dirs = vec![PathBuf::from("/Library/Fonts"), PathBuf::from("/System/Library/Fonts")];
        if let Some(home) = home {
            dirs.push(home.join("Library/Fonts"));
        }
        dirs
    } else {
        let mut dirs = vec![PathBuf::from("/usr/share/fonts"), PathBuf::from("/usr/local/share/fonts")];
        if let Some(home) = home {
            dirs.push(home.join(".fonts"));
            dirs.push(home.join(".local/share/fonts"));
        }
        dirs
    }
}

/// The installed families, by name. Scans the font folders the first time (other callers wait
/// for that scan); empty with `PDFCRAFT_SYSTEM_FONTS=0`.
pub fn installed_families() -> &'static [InstalledFamily] {
    static FAMILIES: OnceLock<Vec<InstalledFamily>> = OnceLock::new();
    FAMILIES.get_or_init(|| if disabled() { Vec::new() } else { scan(&directories()) })
}

/// The index of the installed family called `name` (any case).
pub fn installed_family(name: &str) -> Option<u16> {
    let want = name.trim().to_lowercase();
    installed_families().iter().position(|f| f.name.to_lowercase() == want).and_then(|i| u16::try_from(i).ok())
}

/// A PDF font name reduced to its family, as lowercase letters and digits: no subset prefix
/// (`ABCDEF+`), and no style after a hyphen or comma (`AngsanaNew-Bold` → `angsananew`).
pub fn plain_family(base_font: &str) -> String {
    let name = base_font.rsplit('+').next().unwrap_or(base_font);
    let family = name.split(['-', ',']).next().unwrap_or(name);
    family.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// The installed family a PDF font name (`ABCDEF+AngsanaNew-Bold`) names, if this machine has it.
pub fn installed_family_like(base_font: &str) -> Option<u16> {
    let want = plain_family(base_font);
    if want.is_empty() {
        return None;
    }
    installed_families()
        .iter()
        .position(|f| f.base.first().is_some_and(|plain| plain.to_ascii_lowercase() == want))
        .and_then(|i| u16::try_from(i).ok())
}

/// The face of installed family `family` in a style, read from disk the first time. A missing
/// style falls back to the nearest one the family has.
pub fn installed_face(family: u16, bold: bool, italic: bool) -> Option<ShapedScript> {
    let f = installed_families().get(usize::from(family))?;
    let want = usize::from(bold) | usize::from(italic) << 1;
    // The asked style, then without italic, then without bold, then whatever exists.
    for slot in [want, want & 1, want & 2, 0, 1, 2, 3] {
        let (Some(source), Some(cell)) = (f.sources.get(slot).and_then(Option::as_ref), f.loaded.get(slot)) else { continue };
        let style = STYLES.get(slot).copied().unwrap_or("Regular");
        if let Some(face) = cell.get_or_init(|| load(&source.0, source.1, f.name, style)) {
            return Some(*face);
        }
    }
    None
}

/// The font program behind a shaped face: its bytes and its index in them.
pub fn face_data(script: ShapedScript) -> Option<(&'static [u8], u32)> {
    match script {
        ShapedScript::Craft(index) => match CRAFT_FONTS.get(usize::from(index)) {
            Some(font) => Some((font.bytes, 0)),
            None => loaded(usize::from(index).checked_sub(CRAFT_FONTS.len())?).map(|l| (l.info.bytes, l.index)),
        },
        other => other.font().map(|f| (f.bytes, 0)),
    }
}

fn load(path: &Path, index: u32, family: &'static str, style: &'static str) -> Option<ShapedScript> {
    if std::fs::metadata(path).ok()?.len() > MAX_FONT_BYTES {
        return None;
    }
    // Kept for the life of the process: glyph outlines and shaping borrow from it.
    let bytes: &'static [u8] = Box::leak(std::fs::read(path).ok()?.into_boxed_slice());
    let font = FontRef::from_index(bytes, index).ok()?;
    let data = ShaperData::new(&font);
    let face: &'static LoadedFace =
        Box::leak(Box::new(LoadedFace { face: (font, data), info: CraftFont { family, style, scripts: &[], bytes }, index }));
    let mut all = LOADED.write().ok()?;
    let id = u16::try_from(CRAFT_FONTS.len().checked_add(all.len())?).ok()?;
    all.push(face);
    Some(ShapedScript::Craft(id))
}

struct FaceInfo {
    index: u32,
    family: String,
    bold: bool,
    italic: bool,
    thai: bool,
}

fn font_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if out.len() >= MAX_FILES {
            return;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() {
            if depth > 0 {
                font_files(&path, depth - 1, out);
            }
        } else if meta.is_file()
            && path.extension().and_then(|e| e.to_str()).is_some_and(|e| ["ttf", "otf", "ttc", "otc"].iter().any(|x| e.eq_ignore_ascii_case(x)))
        {
            out.push(path);
        }
    }
}

/// A family found by [`scan`]: its name, the file and face index of each style, and whether it draws Thai.
type Found = (String, [Option<(PathBuf, u32)>; 4], bool);

fn scan(dirs: &[PathBuf]) -> Vec<InstalledFamily> {
    let mut files = Vec::new();
    for dir in dirs {
        font_files(dir, 3, &mut files);
    }
    // Lowercase name → (name, the file and face index of each style). The first face found wins.
    let mut found: BTreeMap<String, Found> = BTreeMap::new();
    for path in files {
        for face in faces_of(&path) {
            let slot = usize::from(face.bold) | usize::from(face.italic) << 1;
            let entry = found.entry(face.family.to_lowercase()).or_insert_with(|| (face.family.clone(), [None, None, None, None], false));
            // The regular face decides; a family without one goes by any face.
            if slot == 0 || entry.1.iter().all(Option::is_none) {
                entry.2 = face.thai;
            }
            if let Some(source) = entry.1.get_mut(slot)
                && source.is_none()
            {
                *source = Some((path.clone(), face.index));
            }
        }
    }
    found
        .into_values()
        .take(usize::from(u16::MAX))
        .enumerate()
        .map(|(i, (name, sources, thai))| {
            let plain: String = name.chars().filter(char::is_ascii_alphanumeric).collect();
            let plain = if plain.is_empty() { format!("Font{i}") } else { plain };
            let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
            InstalledFamily {
                base: [leak(plain.clone()), leak(format!("{plain}-Bold")), leak(format!("{plain}-Italic")), leak(format!("{plain}-BoldItalic"))],
                name: leak(name),
                thai,
                sources,
                loaded: [OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new()],
            }
        })
        .collect()
}

fn read_at(file: &mut File, offset: u64, len: usize) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; len];
    file.seek(SeekFrom::Start(offset)).ok()?;
    file.read_exact(&mut buf).ok()?;
    Some(buf)
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at.checked_add(2)?)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

/// The usable faces of a font file (one, or a collection's several).
fn faces_of(path: &Path) -> Vec<FaceInfo> {
    let Ok(mut file) = File::open(path) else { return Vec::new() };
    let Some(head) = read_at(&mut file, 0, 12) else { return Vec::new() };
    let offsets: Vec<u32> = if head.starts_with(b"ttcf") {
        let count = u32_at(&head, 8).map_or(0, |n| n as usize).min(MAX_COLLECTION_FACES);
        let Some(table) = read_at(&mut file, 12, count * 4) else { return Vec::new() };
        (0..count).filter_map(|i| u32_at(&table, i * 4)).collect()
    } else {
        vec![0]
    };
    offsets
        .into_iter()
        .enumerate()
        .filter_map(|(i, offset)| {
            let (family, bold, italic, thai) = face_at(&mut file, u64::from(offset))?;
            Some(FaceInfo { index: u32::try_from(i).ok()?, family, bold, italic, thai })
        })
        .collect()
}

/// The legacy family name and style of the face whose table directory is at `offset`; `None` for
/// a face that can't be a document font (no outlines, no Unicode map, or embedding not allowed).
fn face_at(file: &mut File, offset: u64) -> Option<(String, bool, bool, bool)> {
    let dir = read_at(file, offset, 12)?;
    if !matches!(dir.get(..4)?, [0, 1, 0, 0] | b"OTTO" | b"true") {
        return None;
    }
    let count = usize::from(u16_at(&dir, 4)?).min(128);
    let records = read_at(file, offset + 12, count * 16)?;
    let table = |tag: &[u8; 4]| -> Option<(u64, usize)> {
        records.as_chunks::<16>().0.iter().find(|r| r.starts_with(tag)).and_then(|r| Some((u64::from(u32_at(r, 8)?), u32_at(r, 12)? as usize)))
    };
    if table(b"glyf").is_none() && table(b"CFF ").is_none() && table(b"CFF2").is_none() {
        return None;
    }
    // Embedding permissions: the least restrictive of the basic levels applies.
    let (os2_at, os2_len) = table(b"OS/2")?;
    let os2 = read_at(file, os2_at, os2_len.min(64))?;
    let fs_type = u16_at(&os2, 8)?;
    let restricted = fs_type & 0x000F == 0x0002;
    let no_subsetting = fs_type & 0x0100 != 0;
    let bitmap_only = fs_type & 0x0200 != 0;
    if restricted || no_subsetting || bitmap_only {
        return None;
    }
    let selection = u16_at(&os2, 62).unwrap_or(0);
    // A Unicode character map (symbol fonts have none).
    let (cmap_at, cmap_len) = table(b"cmap")?;
    let cmap = read_at(file, cmap_at, cmap_len.min(8 * 1024 * 1024))?;
    let subtables = usize::from(u16_at(&cmap, 2)?).min(32);
    let unicode: Vec<usize> = (0..subtables)
        .filter(|i| {
            let (platform, encoding) = (u16_at(&cmap, 4 + i * 8), u16_at(&cmap, 6 + i * 8));
            matches!((platform, encoding), (Some(0), Some(_)) | (Some(3), Some(1 | 10)))
        })
        .filter_map(|i| u32_at(&cmap, 8 + i * 8).map(|at| at as usize))
        .collect();
    if unicode.is_empty() {
        return None;
    }
    // Thai: ko kai, the first letter of the alphabet.
    let thai = unicode.iter().any(|at| cmap_covers(&cmap, *at, 0x0E01));
    let (name_at, name_len) = table(b"name")?;
    let names = read_at(file, name_at, name_len.min(256 * 1024))?;
    let family = name_string(&names, 1)?;
    let family = family.trim();
    if family.is_empty() || family.starts_with(['.', '@']) || family.chars().count() > 80 || family.chars().any(char::is_control) {
        return None;
    }
    Some((family.to_string(), selection & 0x0020 != 0, selection & 0x0001 != 0, thai))
}

/// Whether the character-map subtable at `at` covers `code` (formats 4 and 12, the ones Unicode
/// fonts use). A covered code may still map to the missing glyph; this is a font's claim.
fn cmap_covers(cmap: &[u8], at: usize, code: u32) -> bool {
    let covers = || -> Option<bool> {
        match u16_at(cmap, at)? {
            4 => {
                let code = u16::try_from(code).ok()?;
                let segments = usize::from(u16_at(cmap, at.checked_add(6)?)?) / 2;
                let (ends, starts) = (at.checked_add(14)?, at.checked_add(16)?.checked_add(segments * 2)?);
                Some((0..segments).any(|i| {
                    matches!((u16_at(cmap, starts + i * 2), u16_at(cmap, ends + i * 2)), (Some(start), Some(end)) if start <= code && code <= end && end != 0xFFFF)
                }))
            }
            12 => {
                let groups = (u32_at(cmap, at.checked_add(12)?)? as usize).min(100_000);
                let first = at.checked_add(16)?;
                Some((0..groups).any(|i| {
                    matches!((u32_at(cmap, first + i * 12), u32_at(cmap, first + i * 12 + 4)), (Some(start), Some(end)) if start <= code && code <= end)
                }))
            }
            _ => Some(false),
        }
    };
    covers().unwrap_or(false)
}

/// Name `id` of a `name` table: the Windows English string when there is one, else another
/// Unicode string, else a Macintosh Roman one that is plain ASCII.
fn name_string(names: &[u8], id: u16) -> Option<String> {
    let count = usize::from(u16_at(names, 2)?).min(2048);
    let strings = usize::from(u16_at(names, 4)?);
    let mut best: Option<(u8, String)> = None;
    for i in 0..count {
        let at = 6 + i * 12;
        if u16_at(names, at + 6)? != id {
            continue;
        }
        let (platform, encoding, language) = (u16_at(names, at)?, u16_at(names, at + 2)?, u16_at(names, at + 4)?);
        let (len, offset) = (usize::from(u16_at(names, at + 8)?), usize::from(u16_at(names, at + 10)?));
        let Some(raw) = names.get(strings.checked_add(offset)?..strings.checked_add(offset)?.checked_add(len)?) else { continue };
        let (rank, text) = match (platform, encoding) {
            (3, 1 | 10) | (0, _) => {
                let units: Vec<u16> = raw.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect();
                let Ok(text) = String::from_utf16(&units) else { continue };
                (
                    if platform == 3 && language == 0x0409 {
                        3
                    } else if platform == 3 {
                        2
                    } else {
                        1
                    },
                    text,
                )
            }
            (1, 0) if raw.is_ascii() => (0, String::from_utf8_lossy(raw).into_owned()),
            _ => continue,
        };
        if best.as_ref().is_none_or(|(r, _)| rank > *r) {
            best = Some((rank, text));
        }
    }
    best.map(|(_, text)| text)
}
