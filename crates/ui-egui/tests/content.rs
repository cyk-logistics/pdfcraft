//! Edit a PDF ▸ Add content in the real shell (egui_kittest): type text onto a page, move it,
//! restyle it from the panel, add an image, delete with the keyboard.

use egui::{Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use pdfcraft_ui_egui::{PdfCraftApp, QuickTool};

fn harness() -> Harness<'static, PdfCraftApp> {
    harness_with_step_dt(0.25)
}

fn harness_with_step_dt(step_dt: f32) -> Harness<'static, PdfCraftApp> {
    harness_bytes_with_step_dt(include_bytes!("data/form.pdf").to_vec(), step_dt)
}

fn harness_bytes(bytes: Vec<u8>) -> Harness<'static, PdfCraftApp> {
    harness_bytes_with_step_dt(bytes, 0.25)
}

fn harness_bytes_with_step_dt(bytes: Vec<u8>, step_dt: f32) -> Harness<'static, PdfCraftApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).with_step_dt(step_dt).build_eframe(move |_cc| {
        let mut app = PdfCraftApp::new();
        app.set_option("language", "en").unwrap();
        app.open_bytes("form.pdf", None, bytes.clone()).unwrap();
        app.set_option("zoom", "150").unwrap();
        app
    });
    for _ in 0..60 {
        h.run_steps(2);
        if !h.state().render_pending() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    h
}

fn at(h: &Harness<'static, PdfCraftApp>, x: f32, y: f32) -> Pos2 {
    let r = h.state().views[0].page_screen_rect(0).expect("on screen");
    let k = r.width() / 300.0;
    r.min + egui::vec2(x * k, y * k)
}

fn click(h: &mut Harness<'static, PdfCraftApp>, p: Pos2) {
    h.hover_at(p);
    h.run_steps(1);
    h.drag_at(p);
    h.run_steps(1);
    h.drop_at(p);
    h.run_steps(3);
}

fn added(h: &Harness<'static, PdfCraftApp>) -> Vec<pdfcraft_engine::Added> {
    let s = h.state();
    s.session.get(s.views[0].id).unwrap().added().to_vec()
}

/// A click whose press and release arrive in the same frame, as a quick real mouse click does.
fn quick_click(h: &mut Harness<'static, PdfCraftApp>, p: Pos2) {
    h.hover_at(p);
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run_steps(3);
}

fn texts(h: &Harness<'static, PdfCraftApp>) -> Vec<String> {
    added(h)
        .into_iter()
        .filter_map(|a| match a.content {
            pdfcraft_engine::AddedContent::Text(t) => Some(t.text),
            _ => None,
        })
        .collect()
}

/// The screen centre of added item `i` on page 1.
fn item_centre(h: &Harness<'static, PdfCraftApp>, i: usize) -> Pos2 {
    let s = h.state();
    let doc = s.session.get(s.views[0].id).unwrap();
    let (w, ht) = (doc.info.pages[0].width, doc.info.pages[0].height);
    let r = s.views[0].page_screen_rect(0).expect("on screen");
    let b = doc.added()[i].content.rect();
    let (cx, cy) = (((b[0] + b[2]) / 2.0) as f32, ht - ((b[1] + b[3]) / 2.0) as f32);
    r.min + egui::vec2(cx / w * r.width(), cy / ht * r.height())
}

/// Thai edition: Arrange from the right-click menu and from Ctrl+] / Ctrl+[ reorders overlapping
/// text boxes, and the selection stays on the box that moved.
#[test]
fn overlapping_text_boxes_are_arranged_from_the_menu_and_the_keys() {
    let mut h = harness();
    assert!(h.state_mut().execute("edit.text"));
    h.run_steps(2);
    for text in ["Under", "Over"] {
        let added = pdfcraft_engine::AddedText { rect: [40.0, 200.0, 200.0, 230.0], text: text.into(), size: 14.0, ..Default::default() };
        h.state_mut().apply_edit(pdfcraft_engine::Edit::AddText { page: 0, text: added });
        h.run_steps(3);
    }
    assert_eq!(texts(&h), ["Under", "Over"]);
    // Right-click the boxes (the top one is hit) and send it to the back.
    let p = item_centre(&h, 1);
    h.hover_at(p);
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run_steps(3);
    assert_eq!(h.state().views[0].content.selected, Some((0, 1)), "the right-click selected the box on top");
    assert_eq!(h.state().views[0].content.menu, Some((0, 1)), "the menu is for that box");
    // The submenu button's label carries its arrow.
    h.get_by_label_contains("Arrange").click();
    h.run_steps(2);
    // Its items show their shortcuts too.
    h.get_by_label_contains("Send to back").click();
    h.run_steps(6);
    assert_eq!(texts(&h), ["Over", "Under"]);
    assert_eq!(h.state().views[0].content.selected, Some((0, 0)), "the selection follows the box");
    let s = h.state();
    assert_eq!(s.session.get(s.views[0].id).unwrap().can_undo(), Some("Send to back"));
    // Ctrl+] brings it forward again; Ctrl+[ sends it back. The key's place counts, not its letter.
    let key = |h: &mut Harness<'static, PdfCraftApp>, key: egui::Key| {
        let modifiers = egui::Modifiers::COMMAND;
        h.event(egui::Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers });
        h.event(egui::Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers });
        h.run_steps(6);
    };
    key(&mut h, egui::Key::CloseBracket);
    assert_eq!(texts(&h), ["Under", "Over"]);
    assert_eq!(h.state().views[0].content.selected, Some((0, 1)));
    key(&mut h, egui::Key::OpenBracket);
    assert_eq!(texts(&h), ["Over", "Under"]);
}

/// Thai edition: a comment drawn over text goes behind it with Send to back from its
/// right-click menu (placed on the page first).
#[test]
fn a_comment_placed_on_the_page_goes_behind_text() {
    let mut h = harness();
    let rect = [40.0, 180.0, 200.0, 250.0];
    let comment = pdfcraft_engine::NewAnnotation {
        page: 0,
        shape: pdfcraft_engine::Shape::Rectangle { rect },
        style: pdfcraft_engine::Style { fill: Some([1.0, 1.0, 1.0]), ..Default::default() },
        contents: String::new(),
        author: "Now".into(),
    };
    h.state_mut().apply_edit(pdfcraft_engine::Edit::AddAnnotation(comment));
    h.run_steps(3);
    let text = pdfcraft_engine::AddedText { rect: [50.0, 200.0, 190.0, 230.0], text: "Over".into(), size: 14.0, ..Default::default() };
    h.state_mut().apply_edit(pdfcraft_engine::Edit::AddText { page: 0, text });
    h.run_steps(3);
    let comments = |h: &Harness<'static, PdfCraftApp>| {
        let s = h.state();
        s.session.get(s.views[0].id).unwrap().info.annotations.len()
    };
    assert_eq!(comments(&h), 1);
    // Right-click the comment's corner (outside the text box), with the Select tool.
    h.state_mut().quick_tool = QuickTool::Select;
    h.run_steps(2);
    let p = {
        let s = h.state();
        let doc = s.session.get(s.views[0].id).unwrap();
        let (w, ht) = (doc.info.pages[0].width, doc.info.pages[0].height);
        let r = s.views[0].page_screen_rect(0).unwrap();
        r.min + egui::vec2(45.0 / w * r.width(), (ht - 245.0) / ht * r.height())
    };
    h.hover_at(p);
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run_steps(3);
    // The page's only comment: Send to back places it on the page, behind the text.
    h.get_by_label_contains("Arrange").click();
    h.run_steps(2);
    h.get_by_label_contains("Send to back").click();
    h.run_steps(6);
    assert_eq!(comments(&h), 0, "no longer a comment");
    assert_eq!(texts(&h), ["Over"]);
    assert_eq!(added(&h).len(), 2);
    assert!(matches!(added(&h)[0].content, pdfcraft_engine::AddedContent::Image(_)), "the comment is now the picture behind the text");
    let undo = {
        let s = h.state();
        s.session.get(s.views[0].id).unwrap().can_undo().map(str::to_owned)
    };
    assert_eq!(undo.as_deref(), Some("Send to back"));
}

/// Thai edition: a selected comment is copied with ⌘C and pasted with ⌘V, a step down and right,
/// and the copy is selected; a text highlight can't be copied.
#[test]
fn comments_are_copied_and_pasted_with_the_keys() {
    let mut h = harness();
    let comment = pdfcraft_engine::NewAnnotation {
        page: 0,
        shape: pdfcraft_engine::Shape::Rectangle { rect: [40.0, 180.0, 200.0, 250.0] },
        style: Default::default(),
        contents: "Check\r\nthis".into(),
        author: "Now".into(),
    };
    h.state_mut().apply_edit(pdfcraft_engine::Edit::AddAnnotation(comment));
    h.run_steps(3);
    let rects = |h: &Harness<'static, PdfCraftApp>| {
        let s = h.state();
        s.session.get(s.views[0].id).unwrap().info.annotations.iter().filter(|a| a.subtype == "Square").map(|a| (a.index, a.rect)).collect::<Vec<_>>()
    };
    let (index, before) = rects(&h)[0];
    h.state_mut().views[0].comments.selected = Some((0, index));
    h.event(egui::Event::Copy);
    h.run_steps(2);
    let marker = h.state().comment_clip.as_ref().map(|c| c.marker.clone());
    assert_eq!(marker.as_deref(), Some("Check\nthis"), "the comment's text goes to the clipboard with it");
    h.event(egui::Event::Paste("Check\nthis".into()));
    h.run_steps(4);
    let after = rects(&h);
    assert_eq!(after.len(), 2, "a copy was pasted");
    let (copy_index, copy) = after[1];
    assert!((copy[0] - before[0] - 12.0).abs() < 0.01 && (copy[1] - before[1] + 12.0).abs() < 0.01, "{before:?} -> {copy:?}");
    assert_eq!(h.state().views[0].comments.selected, Some((0, copy_index)), "the copy is selected");
    // Something else on the clipboard is not pasted as the comment.
    h.event(egui::Event::Paste("other text".into()));
    h.run_steps(3);
    assert_eq!(rects(&h).len(), 2);
}

/// Two pages, 300 × 400 points, for moving things between them.
const TWO_PAGES: &[u8] = b"%PDF-1.7
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 300 400] >> endobj
3 0 obj << /Type /Page /Parent 2 0 R >> endobj
4 0 obj << /Type /Page /Parent 2 0 R >> endobj
trailer << /Root 1 0 R >>
%%EOF";

/// Thai edition: a text box dragged onto the next page goes there; with Ctrl held, a copy goes
/// and the original stays. Comments and pictures go across too.
#[test]
fn items_are_dragged_and_copied_onto_another_page() {
    let mut h = harness_bytes(TWO_PAGES.to_vec());
    h.state_mut().set_option("zoom", "60").unwrap();
    h.run_steps(4);
    assert!(h.state_mut().execute("edit.text"));
    h.run_steps(2);
    let text = pdfcraft_engine::AddedText { rect: [40.0, 300.0, 200.0, 330.0], text: "Travels".into(), size: 14.0, ..Default::default() };
    h.state_mut().apply_edit(pdfcraft_engine::Edit::AddText { page: 0, text });
    h.run_steps(4);
    let on = |h: &Harness<'static, PdfCraftApp>, page: usize| added(h).iter().filter(|a| a.page == page).count();
    // Drag it from page 1 onto page 2 (both on screen at this zoom).
    // Press on a box, move in steps, release over `to`, with `modifiers` held throughout.
    let drag = |h: &mut Harness<'static, PdfCraftApp>, from: Pos2, to: Pos2, modifiers: egui::Modifiers| {
        h.event(egui::Event::ModifiersChanged(modifiers));
        h.hover_at(from);
        h.run_steps(2);
        h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers });
        h.run_steps(2);
        for k in 1..=10 {
            h.hover_at(from + (to - from) * (k as f32 / 10.0));
            h.run_steps(2);
        }
        h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers });
        h.run_steps(6);
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        h.run_steps(2);
    };
    // Where item `i` is drawn on screen.
    let centre_of = |h: &Harness<'static, PdfCraftApp>, i: usize| {
        let s = h.state();
        let doc = s.session.get(s.views[0].id).unwrap();
        let a = &doc.added()[i];
        let (w, ht) = (doc.info.pages[a.page].width, doc.info.pages[a.page].height);
        let r = s.views[0].page_screen_rect(a.page).unwrap();
        let b = a.content.rect();
        r.min + egui::vec2(((b[0] + b[2]) / 2.0) as f32 / w * r.width(), (ht - ((b[1] + b[3]) / 2.0) as f32) / ht * r.height())
    };
    for _ in 0..100 {
        h.run_steps(2);
        if !h.state().render_pending() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let page2 = h.state().views[0].page_screen_rect(1).expect("page 2 on screen");
    let from = centre_of(&h, 0);
    drag(&mut h, from, page2.center(), egui::Modifiers::NONE);
    assert_eq!((on(&h, 0), on(&h, 1)), (0, 1), "moved to page 2");
    let undo = {
        let s = h.state();
        s.session.get(s.views[0].id).unwrap().can_undo().map(str::to_owned)
    };
    assert_eq!(undo.as_deref(), Some("Move to another page"));
    // Back to page 1, with Ctrl: a copy goes and the original stays.
    let page1 = h.state().views[0].page_screen_rect(0).expect("page 1 on screen");
    let from = centre_of(&h, 0);
    drag(&mut h, from, page1.center(), egui::Modifiers::COMMAND);
    assert_eq!((on(&h, 0), on(&h, 1)), (1, 1), "a copy on page 1, the original still on page 2");
    // A comment goes across, and a copy of it too.
    let comment = pdfcraft_engine::NewAnnotation {
        page: 0,
        shape: pdfcraft_engine::Shape::Rectangle { rect: [40.0, 40.0, 120.0, 90.0] },
        style: Default::default(),
        contents: String::new(),
        author: "Now".into(),
    };
    h.state_mut().apply_edit(pdfcraft_engine::Edit::AddAnnotation(comment));
    h.run_steps(3);
    let comments = |h: &Harness<'static, PdfCraftApp>, page: usize| {
        let s = h.state();
        s.session.get(s.views[0].id).unwrap().info.annotations.iter().filter(|a| a.page == page).map(|a| a.index).collect::<Vec<_>>()
    };
    let index = comments(&h, 0)[0];
    h.state_mut().apply_edit(pdfcraft_engine::Edit::MoveAnnotationToPage { page: 0, index, to_page: 1, dx: 0.0, dy: 100.0, copy: false });
    h.run_steps(3);
    assert_eq!((comments(&h, 0).len(), comments(&h, 1).len()), (0, 1));
    let index = comments(&h, 1)[0];
    h.state_mut().apply_edit(pdfcraft_engine::Edit::MoveAnnotationToPage { page: 1, index, to_page: 0, dx: 0.0, dy: 0.0, copy: true });
    h.run_steps(3);
    assert_eq!((comments(&h, 0).len(), comments(&h, 1).len()), (1, 1));
}

/// Thai edition: an added picture is copied with ⌘C and pasted with ⌘V, on its page or another.
#[test]
fn added_pictures_are_copied_and_pasted() {
    let mut h = harness_bytes(TWO_PAGES.to_vec());
    assert!(h.state_mut().execute("edit.text"));
    h.run_steps(2);
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png, 2, 2);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&[255u8; 16]).unwrap();
    }
    h.state_mut().apply_edit(pdfcraft_engine::Edit::AddImage {
        page: 0,
        rect: Some([40.0, 200.0, 140.0, 300.0]),
        name: "dot.png".into(),
        bytes: std::sync::Arc::new(png),
    });
    h.run_steps(4);
    h.state_mut().views[0].content.selected = Some((0, 0));
    h.event(egui::Event::Copy);
    h.run_steps(2);
    let marker = h.state().image_clip.as_ref().map(|c| c.marker.clone()).expect("copied");
    h.event(egui::Event::Paste(marker.clone()));
    h.run_steps(4);
    let images = |h: &Harness<'static, PdfCraftApp>, page: usize| {
        added(h)
            .into_iter()
            .filter(|a| a.page == page && matches!(a.content, pdfcraft_engine::AddedContent::Image(_)))
            .map(|a| a.content.rect())
            .collect::<Vec<_>>()
    };
    let on_one = images(&h, 0);
    assert_eq!(on_one.len(), 2, "pasted beside it");
    assert!((on_one[1][0] - on_one[0][0] - 12.0).abs() < 0.01);
    h.state_mut().views[0].current = 1;
    h.event(egui::Event::Paste(marker));
    h.run_steps(4);
    assert_eq!(images(&h, 1).len(), 1, "pasted on page 2");
}

/// Thai edition: in Edit text & images, ⌘C copies what that tool selected even while a comment
/// selected earlier with the Select tool is still selected; a click on empty page space goes
/// back to the Select tool.
#[test]
fn edit_text_copies_its_own_selection_and_a_click_on_empty_space_selects() {
    let mut h = harness_bytes(TWO_PAGES.to_vec());
    let comment = pdfcraft_engine::NewAnnotation {
        page: 0,
        shape: pdfcraft_engine::Shape::Rectangle { rect: [40.0, 40.0, 120.0, 90.0] },
        style: Default::default(),
        contents: "old comment".into(),
        author: "Now".into(),
    };
    h.state_mut().apply_edit(pdfcraft_engine::Edit::AddAnnotation(comment));
    let text = pdfcraft_engine::AddedText { rect: [40.0, 300.0, 200.0, 330.0], text: "New box".into(), size: 14.0, ..Default::default() };
    h.state_mut().apply_edit(pdfcraft_engine::Edit::AddText { page: 0, text });
    h.run_steps(4);
    let index = {
        let s = h.state();
        s.session.get(s.views[0].id).unwrap().info.annotations[0].index
    };
    // The comment, selected and copied with the Select tool.
    h.state_mut().views[0].comments.selected = Some((0, index));
    h.event(egui::Event::Copy);
    h.run_steps(2);
    assert!(h.state().comment_clip.is_some());
    // Edit text & images: the text box selected there is what ⌘C copies now.
    assert!(h.state_mut().execute("edit.edit_text"));
    h.run_steps(2);
    assert_eq!(h.state().quick_tool, QuickTool::EditText);
    h.state_mut().views[0].content.selected = Some((0, 0));
    h.event(egui::Event::Copy);
    h.run_steps(2);
    assert_eq!(h.state().text_clipboard.as_ref().map(|c| c.text.text.clone()).as_deref(), Some("New box"));
    // A click on empty page space: back to Select.
    for _ in 0..60 {
        h.run_steps(2);
        if !h.state().render_pending() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    h.state_mut().views[0].content.selected = None;
    let empty = h.state().views[0].page_screen_rect(0).unwrap().center();
    click(&mut h, empty);
    assert_eq!(h.state().quick_tool, QuickTool::Select);
}

#[test]
fn text_tool_drafts_grow_for_latin_and_arabic() {
    for text in ["Wide words ".repeat(8), "سلام عليكم ".repeat(8)] {
        let mut h = harness();
        assert!(h.state_mut().execute("edit.text"));
        h.run_steps(2);
        let point = at(&h, 40.0, 300.0);
        click(&mut h, point);
        let before = h.state().views[0].content.draft.as_ref().unwrap().rect;
        h.event(egui::Event::Text(text.clone()));
        h.run_steps(4);
        let draft = h.state().views[0].content.draft.as_ref().unwrap();
        assert!(draft.rect[2] - draft.rect[0] > before[2] - before[0] + 10.0, "the editor grows before wrapping: {}", text);
        assert!(draft.rect[2] <= 300.0 && draft.rect[1] < before[1], "the frame stays on the page and grows vertically");
        if let Ok(dir) = std::env::var("PDFCRAFT_SHOTS") {
            let language = if text.starts_with("Wide") { "latin" } else { "arabic" };
            h.render().unwrap().save(format!("{dir}/growing-{language}-text.png")).unwrap();
        }
        h.get_by_label("Done adding text").click();
        h.run_steps(3);
        if text.starts_with("سلام") && pdfcraft_fonts::document_arabic_font().is_none() {
            assert!(h.state().views[0].content.draft.is_some(), "without a document font the draft remains available");
            assert!(added(&h).is_empty());
            continue;
        }
        let pdfcraft_engine::AddedContent::Text(saved) = &added(&h)[0].content else { panic!() };
        assert_eq!(saved.text, text.trim_end());
        assert!(saved.rect[2] - saved.rect[0] > before[2] - before[0] + 10.0, "Save uses the grown width");
    }
}

#[test]
fn text_tool_single_click_retypes_added_text_without_another_item() {
    for command in ["edit.text", "edit.edit_text"] {
        for (original, replacement) in [("Original", "Replacement"), ("سلام", "سلام عليكم ورحمة الله")] {
            if original == "سلام" && pdfcraft_fonts::document_arabic_font().is_none() {
                continue;
            }
            let mut h = harness();
            assert!(h.state_mut().apply_edit(pdfcraft_engine::Edit::AddText {
                page: 0,
                text: pdfcraft_engine::AddedText { rect: [40.0, 195.0, 140.0, 210.0], text: original.into(), ..Default::default() },
            }));
            assert!(h.state_mut().execute(command));
            h.run_steps(3);
            let point = at(&h, 45.0, 195.0);
            click(&mut h, point);
            let draft = h.state().views[0].content.draft.as_ref().expect("one click edits the existing item");
            assert_eq!(draft.index, Some(0));
            assert_eq!(draft.text, original);
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
            h.event(egui::Event::Text(replacement.into()));
            h.run_steps(3);
            h.get_by_label("Done adding text").click();
            h.run_steps(3);
            assert_eq!(texts(&h), [replacement]);
            let state = h.state();
            let bytes = state.session.save_bytes(state.views[0].id).unwrap();
            let mut reopened = pdfcraft_engine::Session::new();
            let id = reopened.open("again.pdf", None, bytes, None).unwrap();
            assert_eq!(reopened.get(id).unwrap().added().len(), 1);
            let pdfcraft_engine::AddedContent::Text(text) = &reopened.get(id).unwrap().added()[0].content else { panic!() };
            assert_eq!(text.text, replacement);
        }
    }
}

#[test]
fn text_tool_deleting_a_draft_hides_the_old_added_glyphs_and_cancel_restores_them() {
    let mut h = harness();
    assert!(h.state_mut().apply_edit(pdfcraft_engine::Edit::AddText {
        page: 0,
        text: pdfcraft_engine::AddedText { rect: [40.0, 195.0, 240.0, 220.0], text: "Original glyphs".into(), size: 18.0, ..Default::default() },
    }));
    let bytes = h.state().session.get(h.state().views[0].id).unwrap().bytes.clone();
    assert!(h.state_mut().execute("edit.edit_text"));
    h.run_steps(3);
    let point = at(&h, 45.0, 185.0);
    click(&mut h, point);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.key_press(egui::Key::Backspace);
    for _ in 0..100 {
        h.run_steps(2);
        if !h.state().render_pending() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(h.state().views[0].content.draft.as_ref().unwrap().text.is_empty());
    let shot = h.render().unwrap();
    for x in 105..140 {
        for y in 185..197 {
            let pixel = at(&h, x as f32, y as f32);
            let [r, g, b, a] = shot.get_pixel(pixel.x as u32, pixel.y as u32).0;
            assert!(r > 200 && g > 200 && b > 200 && a == 255, "old glyph ink remains at {x},{y}: {r},{g},{b},{a}");
        }
    }
    if let Ok(dir) = std::env::var("PDFCRAFT_SHOTS") {
        shot.save(format!("{dir}/cleared-added-text.png")).unwrap();
    }
    assert_eq!(h.state().session.get(h.state().views[0].id).unwrap().bytes, bytes, "previewing does not edit the document");
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert_eq!(texts(&h), ["Original glyphs"]);
    assert!(h.state().views[0].content.draft.is_none());
}

#[test]
fn clicking_elsewhere_keeps_the_text_and_starts_a_new_box() {
    // #74: a click elsewhere on the page threw away the text being typed and opened a new box.
    let mut h = harness();
    assert!(h.state_mut().execute("edit.text"));
    h.run_steps(2);
    let p = at(&h, 40.0, 100.0);
    click(&mut h, p);
    h.event(egui::Event::Text("First".into()));
    h.run_steps(2);
    // A slow click (press and release in different frames)…
    let p = at(&h, 40.0, 200.0);
    click(&mut h, p);
    assert_eq!(texts(&h), ["First"]);
    assert!(h.state().views[0].content.draft.is_some(), "a new box opened");
    assert_eq!(h.state().quick_tool, QuickTool::AddText, "still adding text");
    h.event(egui::Event::Text("Second".into()));
    h.run_steps(2);
    // …and a quick one (both in one frame, as a real mouse click usually arrives).
    let p = at(&h, 40.0, 300.0);
    quick_click(&mut h, p);
    assert_eq!(texts(&h), ["First", "Second"]);
    // Discard throws away only the box being typed.
    h.event(egui::Event::Text("Third".into()));
    h.run_steps(2);
    h.get_by_label("Discard this text (Esc)").click();
    h.run_steps(3);
    assert_eq!(texts(&h), ["First", "Second"]);
    assert!(h.state().views[0].content.draft.is_none());
}

#[test]
fn typing_moving_styling_and_deleting_added_content() {
    let mut h = harness();
    assert!(h.state_mut().execute("edit.text"));
    h.run_steps(2);
    h.get_by_label("Edit a PDF");
    let p = at(&h, 40.0, 300.0);
    click(&mut h, p);
    assert!(h.state().views[0].content.draft.is_some(), "the editor opened");
    h.event(egui::Event::Text("Reviewed".into()));
    h.run_steps(2);
    // Done keeps the text and goes back to Select, with the new text selected.
    h.get_by_label("Done adding text").click();
    h.run_steps(3);
    let a = added(&h);
    assert_eq!(a.len(), 1);
    let pdfcraft_engine::AddedContent::Text(t) = &a[0].content else { panic!() };
    assert_eq!(t.text, "Reviewed");
    assert_eq!(h.state().quick_tool, QuickTool::Select);
    h.run_steps(2);
    assert_eq!(h.state().views[0].content.selected, Some((0, 0)), "the new text is selected");
    // Bold from the Format panel: one undoable change.
    h.get_by_label("B").click();
    h.run_steps(3);

    let pdfcraft_engine::AddedContent::Text(t) = &added(&h)[0].content else { panic!() };
    assert!(t.bold);
    {
        let s = h.state();
        assert_eq!(s.session.get(s.views[0].id).unwrap().can_undo(), Some("Edit content"));
    }
    // Delete.
    h.key_press(egui::Key::Delete);
    h.run_steps(3);
    assert!(added(&h).is_empty());
    // An image lands in the middle of the page, selected.
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png, 30, 20);
        enc.set_color(png::ColorType::Rgb);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&[90u8; 1800]).unwrap();
    }
    h.state_mut().add_image("logo.png".into(), png);
    h.run_steps(3);
    let a = added(&h);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].content.rect(), [135.0, 190.0, 165.0, 210.0]);
    assert_eq!(h.state().views[0].content.selected, Some((0, 0)));
    // Edit image: rotate clockwise turns the box around its centre.
    h.get_by_label("Rotate clockwise").click();
    h.run_steps(3);
    let a = added(&h);
    let pdfcraft_engine::AddedContent::Image(img) = &a[0].content else { panic!() };
    assert_eq!((img.rotation, img.rect), (3, [140.0, 185.0, 160.0, 215.0]));
    h.get_by_label("Flip horizontal").click();
    h.run_steps(3);
    let pdfcraft_engine::AddedContent::Image(img) = &added(&h)[0].content else { panic!() };
    assert!(img.flip_h);
}

/// #125: text the standard fonts can't draw stays in the editor with a warning instead of being
/// written to the page as `?` (or lost).
#[test]
fn text_the_standard_fonts_cant_draw_stays_in_the_editor() {
    let mut h = harness();
    assert!(h.state_mut().execute("edit.text"));
    h.run_steps(2);
    let p = at(&h, 40.0, 300.0);
    click(&mut h, p);
    h.event(egui::Event::Text("Hello 世界".into()));
    h.run_steps(2);
    h.get_by_label_contains("can't draw “世” (U+4E16)");
    if let Ok(dir) = std::env::var("PDFCRAFT_SHOTS") {
        h.render().unwrap().save(format!("{dir}/undrawable-text.png")).unwrap();
    }
    h.get_by_label("Done adding text").click();
    h.run_steps(3);
    assert!(texts(&h).is_empty(), "nothing was written to the page");
    let draft = h.state().views[0].content.draft.as_ref().map(|d| d.text.clone());
    assert_eq!(draft.as_deref(), Some("Hello 世界"), "the typed text is kept");
    // A click elsewhere on the page doesn't finish it, or replace it with a new box.
    let elsewhere = at(&h, 40.0, 150.0);
    click(&mut h, elsewhere);
    let draft = h.state().views[0].content.draft.as_ref().map(|d| d.text.clone());
    assert_eq!(draft.as_deref(), Some("Hello 世界"), "clicking elsewhere keeps the typed text");
    assert!(texts(&h).is_empty());
    // Once the text can be drawn, Done adds it (Done took the focus, so edit the draft directly).
    if let Some(d) = h.state_mut().views[0].content.draft.as_mut() {
        d.text = "Hello world".into();
    }
    h.run_steps(2);
    h.get_by_label("Done adding text").click();
    h.run_steps(3);
    assert_eq!(texts(&h), ["Hello world"]);
}

/// Text kept because it can't be drawn goes with its page when the page is deleted, so nothing
/// is left blocking Save out of sight.
#[test]
fn a_kept_draft_goes_with_its_deleted_page() {
    let mut h = harness();
    assert!(h.state_mut().apply_edit(pdfcraft_engine::Edit::InsertBlankPage { at: 1, width: 612.0, height: 792.0 }));
    h.run_steps(2);
    assert!(h.state_mut().execute("edit.text"));
    h.state_mut().views[0].go_to_page(1);
    h.run_steps(4);
    let p = h.state().views[0].page_screen_rect(1).expect("page 2 on screen").min + egui::vec2(40.0, 40.0);
    click(&mut h, p);
    h.event(egui::Event::Text("日本語".into()));
    h.run_steps(2);
    assert_eq!(h.state().views[0].content.draft.as_ref().map(|d| (d.page, d.text.as_str())), Some((1, "日本語")));
    assert!(h.state_mut().apply_edit(pdfcraft_engine::Edit::DeletePages { pages: vec![1] }));
    h.run_steps(2);
    assert!(h.state().views[0].content.draft.is_none());
}

#[test]
fn typing_thai_text_keeps_marks_and_commits_to_page() {
    if pdfcraft_fonts::document_thai_font().is_none() {
        return;
    }
    let fonts = pdfcraft_ui_egui::theme::font_definitions();
    let face = pdfcraft_fonts::document_thai_font().unwrap();
    assert!(fonts.families[&egui::FontFamily::Proportional].contains(&face.name()));
    let mut h = harness();
    assert!(h.state_mut().execute("edit.text"));
    h.run_steps(2);
    let p = at(&h, 40.0, 100.0);
    click(&mut h, p);
    h.event(egui::Event::Text("น้ำ กิ่ ผู้ใช้ Hello".into()));
    h.run_steps(3);
    let p = at(&h, 40.0, 200.0);
    click(&mut h, p);
    assert_eq!(texts(&h), ["น้ำ กิ่ ผู้ใช้ Hello"]);
}

#[test]
fn finished_text_over_form_field_keeps_selection_and_drag_ownership() {
    let mut h = harness();
    assert!(h.state_mut().execute("edit.text"));
    h.run_steps(2);
    let p = at(&h, 60.0, 202.0);
    click(&mut h, p);
    h.event(egui::Event::Text("Reviewed".into()));
    h.run_steps(2);
    h.get_by_label("Done adding text").click();
    h.run_steps(3);
    assert_eq!(texts(&h), ["Reviewed"]);
    let rect = added(&h)[0].content.rect();
    let mid_x = ((rect[0] + rect[2]) / 2.0) as f32;
    let mid_y = ((rect[1] + rect[3]) / 2.0) as f32;
    assert!((20.0..280.0).contains(&mid_x) && (180.0..200.0).contains(&mid_y), "fixture text must overlap city: {rect:?}");
    let center = at(&h, mid_x, 400.0 - mid_y);
    click(&mut h, center);
    assert!(h.state().views[0].forms.focus.is_none(), "selecting finished text must not focus the field underneath");
    assert_eq!(h.state().views[0].content.selected, Some((0, 0)));
    h.run_steps(60);
    let start = at(&h, mid_x, 400.0 - mid_y);
    let end = at(&h, mid_x, 400.0 - mid_y - 80.0);
    h.drag_at(start);
    h.run_steps(1);
    for step in 1..=40 {
        h.hover_at(start + (end - start) * (step as f32 / 40.0));
        h.run_steps(1);
    }
    h.drop_at(end);
    h.run_steps(4);
    let moved = added(&h)[0].content.rect();
    for (actual, expected) in moved.into_iter().zip([rect[0], rect[1] + 80.0, rect[2], rect[3] + 80.0]) {
        assert!((actual - expected).abs() < 1.0, "text should move upward: {rect:?} -> {moved:?}");
    }
    assert!(h.state().views[0].forms.focus.is_none());
    let bare = at(&h, 270.0, 210.0);
    click(&mut h, bare);
    assert_eq!(h.state().views[0].forms.focus.as_ref().map(|f| f.name.as_str()), Some("city"));
}

fn text_over_city() -> Harness<'static, PdfCraftApp> {
    let mut h = harness_with_step_dt(1.0 / 60.0);
    assert!(h.state_mut().execute("edit.text"));
    h.run_steps(2);
    let p = at(&h, 60.0, 202.0);
    click(&mut h, p);
    h.event(egui::Event::Text("Reviewed".into()));
    h.run_steps(2);
    h.get_by_label("Done adding text").click();
    h.run_steps(3);
    h
}

#[test]
fn finished_text_over_field_keeps_move_and_resize_cursors() {
    let mut h = text_over_city();
    let rect = added(&h)[0].content.rect();
    let center = at(&h, ((rect[0] + rect[2]) / 2.0) as f32, (400.0 - (rect[1] + rect[3]) / 2.0) as f32);
    h.hover_at(center);
    h.run_steps(2);
    assert_eq!(h.output().platform_output.cursor_icon, egui::CursorIcon::Move);
    // The upper-right handle overlaps the city field; it retains its resize gesture.
    let corner = at(&h, rect[2] as f32, (400.0 - rect[3]) as f32);
    h.hover_at(corner);
    h.run_steps(2);
    assert_eq!(h.output().platform_output.cursor_icon, egui::CursorIcon::ResizeNeSw);
    let end = corner + egui::vec2(30.0, -20.0);
    h.drag_at(corner);
    h.run_steps(1);
    for step in 1..=20 {
        h.hover_at(corner + (end - corner) * (step as f32 / 20.0));
        h.run_steps(1);
    }
    h.drop_at(end);
    h.run_steps(4);
    let resized = added(&h)[0].content.rect();
    assert!(resized[2] > rect[2] + 1.0 && resized[3] > rect[3] + 1.0, "resize must retain ownership: {rect:?} -> {resized:?}");
    assert!(h.state().views[0].forms.focus.is_none());
}

#[test]
fn finished_text_over_field_still_opens_its_editor() {
    let mut h = text_over_city();
    let rect = added(&h)[0].content.rect();
    let center = at(&h, ((rect[0] + rect[2]) / 2.0) as f32, (400.0 - (rect[1] + rect[3]) / 2.0) as f32);
    h.run_steps(60);
    h.hover_at(center);
    h.run_steps(1);
    for _ in 0..2 {
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: center, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        }
        h.run_steps(1);
    }
    h.run_steps(2);
    assert!(h.state().views[0].content.draft.as_ref().is_some_and(|d| d.index == Some(0)), "double click edits the added text");
    assert!(h.state().views[0].forms.focus.is_none());
}

#[test]
fn closing_edit_panel_leaves_form_filling_available() {
    let mut h = text_over_city();
    h.state_mut().set_option("left", "closed").unwrap();
    h.run_steps(3);
    let p = at(&h, 100.0, 210.0);
    click(&mut h, p);
    assert_eq!(h.state().views[0].forms.focus.as_ref().map(|f| f.name.as_str()), Some("city"));
}

/// The screen rectangle of an added item's display-space box (y up, from the page's bottom).
fn item_on_screen(h: &Harness<'static, PdfCraftApp>, r: [f64; 4]) -> egui::Rect {
    let s = h.state();
    let ph = s.session.get(s.views[0].id).unwrap().info.pages[0].height;
    let page = s.views[0].page_screen_rect(0).expect("on screen");
    let k = page.width() / 300.0;
    let p = |x: f64, y: f64| page.min + egui::vec2(x as f32 * k, (ph - y as f32) * k);
    egui::Rect::from_two_pos(p(r[0], r[1]), p(r[2], r[3]))
}

/// Dark pixels in `r`: glyph ink (the selection outline is blue, the page white).
fn ink(pixels: &image::RgbaImage, r: egui::Rect) -> usize {
    let (x0, y0, x1, y1) = (r.left().max(0.0) as u32, r.top().max(0.0) as u32, r.right() as u32, r.bottom() as u32);
    (y0..y1.min(pixels.height()))
        .flat_map(|y| (x0..x1.min(pixels.width())).map(move |x| (x, y)))
        .filter(|(x, y)| pixels.get_pixel(*x, *y).0[..3].iter().all(|v| *v < 90))
        .count()
}

/// Only the left part of the glyphs: kittest paints its cursor down and right of the pointer.
fn probe(r: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_size(r.min, egui::vec2(r.width() * 0.6, r.height()))
}

/// Ink in the probes of the text's first and dragged places.
fn ink_at(h: &mut Harness<'static, PdfCraftApp>, from: egui::Rect, to: egui::Rect) -> (usize, usize) {
    let pixels = h.render().unwrap();
    (ink(&pixels, probe(from)), ink(&pixels, probe(to)))
}

/// Types text, finishes it, and drags it up without releasing, until the drag preview shows.
/// Returns the text's box, its screen rectangles before and after, and the pointer.
fn drag_added_text(h: &mut Harness<'static, PdfCraftApp>) -> ([f64; 4], egui::Rect, egui::Rect, Pos2) {
    assert!(h.state_mut().execute("edit.text"));
    h.run_steps(2);
    let p = at(h, 40.0, 300.0);
    click(h, p);
    h.event(egui::Event::Text("MMMMMMMM".into()));
    h.run_steps(2);
    h.get_by_label("Done adding text").click();
    h.run_steps(3);
    let rect = added(h)[0].content.rect();
    let from = item_on_screen(h, rect);
    let mut shown = 0;
    for _ in 0..100 {
        h.run_steps(2);
        shown = ink(&h.render().unwrap(), probe(from));
        if shown > 20 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(shown > 20, "the added text is on the page ({shown} dark pixels)");
    let grab = pos2(from.right() - 12.0, from.center().y);
    let shift = egui::vec2(0.0, -150.0);
    h.hover_at(grab);
    h.run_steps(1);
    h.drag_at(grab);
    h.run_steps(1);
    h.hover_at(grab + shift);
    let to = from.translate(shift);
    for _ in 0..100 {
        h.run_steps(2);
        let (old, new) = ink_at(h, from, to);
        if old == 0 && new > 20 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    (rect, from, to, grab + shift)
}

/// #1488: while a finished text box is dragged, its text follows the box instead of staying put
/// until the button is released.
#[test]
fn dragged_text_follows_the_pointer_before_release() {
    let mut h = harness();
    let (rect, from, to, end) = drag_added_text(&mut h);
    if let Ok(dir) = std::env::var("PDFCRAFT_SHOTS") {
        h.render().unwrap().save(format!("{dir}/added-text-drag.png")).unwrap();
    }
    let (old, new) = ink_at(&mut h, from, to);
    assert!(new > 20, "the text is drawn where the box was dragged ({new} dark pixels)");
    assert_eq!(old, 0, "and no longer where it was");
    assert_eq!(added(&h)[0].content.rect(), rect, "nothing is applied before release");
    h.drop_at(end);
    h.run_steps(3);
    assert_ne!(added(&h)[0].content.rect(), rect, "releasing moves it");
}

/// After release the text stays in its new place while the page is re-rendered with the move,
/// instead of jumping back to where it was for those frames.
#[test]
fn released_text_does_not_jump_back_before_the_page_is_redrawn() {
    // A page slow enough to render that the frames before its new raster can be looked at: many
    // pale squares, too light to count as ink.
    let mut content = String::from("0.9 g\n");
    for i in 0..60_000 {
        content.push_str(&format!("{} {} 1 1 re f\n", i % 290, (i / 290) % 390));
    }
    let pdf = format!(
        "%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 300 400] >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R /Contents 4 0 R >> endobj\n4 0 obj << /Length {} >> stream\n{content}\nendstream endobj\ntrailer << /Root 1 0 R >>\n%%EOF",
        content.len()
    );
    let mut h = harness_bytes(pdf.into_bytes());
    let (rect, from, to, end) = drag_added_text(&mut h);
    h.drop_at(end);
    // Frame by frame until the page has been re-rendered. The one frame that swaps in the new
    // raster reads back blank here (kittest renders it after its old texture is freed).
    let mut shown = Vec::new();
    while h.state().render_pending() || shown.is_empty() {
        h.run_steps(1);
        assert!(shown.len() < 500, "the page is re-rendered");
        let (old, new) = ink_at(&mut h, from, to);
        assert_eq!(old, 0, "frame {} after release: the text is back where it was", shown.len() + 1);
        shown.push(new);
    }
    assert!(shown.len() > 2, "frames before the new raster: {shown:?}");
    assert!(shown.iter().filter(|n| **n <= 20).count() <= 1, "the text stays where it was released: {shown:?}");
    assert_ne!(added(&h)[0].content.rect(), rect, "the move is applied");
    h.run_steps(2);
    let (old, new) = ink_at(&mut h, from, to);
    assert!(new > 20 && old == 0, "the redrawn page has the text in its new place ({old} old, {new} new)");
}

/// A press at `from`, a move to `to` and the release there, with `mods` held throughout.
fn drag_with(h: &mut Harness<'static, PdfCraftApp>, from: Pos2, to: Pos2, mods: egui::Modifiers) {
    let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: mods };
    h.hover_at(from);
    h.run_steps(1);
    h.event_modifiers(button(from, true), mods);
    h.run_steps(1);
    for k in 1..=4 {
        let p = from + (to - from) * (k as f32 / 4.0);
        h.event_modifiers(egui::Event::PointerMoved(p), mods);
        h.run_steps(1);
    }
    h.event_modifiers(button(to, false), mods);
    h.run_steps(4);
}

fn text_of(a: &pdfcraft_engine::Added) -> (String, [f64; 4]) {
    match &a.content {
        pdfcraft_engine::AddedContent::Text(t) => (t.text.clone(), t.rect),
        pdfcraft_engine::AddedContent::Image(_) => panic!("an image"),
    }
}

/// Thai edition: ⌘C and ⌘V copy a text box as a new one a step down and right (each paste a
/// step further), and Ctrl held as a box is dropped leaves it where it was and drops a copy.
#[test]
fn text_boxes_copy_paste_and_ctrl_drag() {
    let mut h = harness();
    assert!(h.state_mut().execute("edit.text"));
    h.run_steps(2);
    let p = at(&h, 40.0, 300.0);
    click(&mut h, p);
    h.event(egui::Event::Text("Copy me".into()));
    h.run_steps(2);
    h.get_by_label("Done adding text").click();
    h.run_steps(3);
    assert_eq!(h.state().views[0].content.selected, Some((0, 0)), "the new text is selected");
    h.event(egui::Event::Copy);
    h.run_steps(2);
    for _ in 0..2 {
        h.event(egui::Event::Paste("Copy me".into()));
        h.run_steps(3);
    }
    let items = added(&h);
    assert_eq!(items.len(), 3, "two pastes add two boxes");
    let (first, a) = text_of(&items[0]);
    for (n, item) in items.iter().enumerate().skip(1) {
        let (text, r) = text_of(item);
        assert_eq!(text, first);
        let step = 12.0 * n as f64;
        assert!((r[0] - a[0] - step).abs() < 0.01 && (a[3] - r[3] - step).abs() < 0.01, "paste {n}: {a:?} -> {r:?}");
    }
    assert_eq!(h.state().views[0].content.selected, Some((0, 2)), "the last paste is selected");
    // Something else copied since is not pasted as a box.
    h.event(egui::Event::Paste("Copied elsewhere".into()));
    h.run_steps(3);
    assert_eq!(added(&h).len(), 3);

    // Ctrl+drag the first box: it stays, a copy lands where it was dropped.
    let from = item_on_screen(&h, a);
    let grab = pos2(from.left() + 6.0, from.center().y);
    drag_with(&mut h, grab, grab + egui::vec2(0.0, 120.0), egui::Modifiers::COMMAND);
    let items = added(&h);
    assert_eq!(items.len(), 4, "Ctrl+drag adds a copy");
    assert_eq!(text_of(&items[0]).1, a, "the original stays put");
    let (text, r) = text_of(&items[3]);
    assert_eq!(text, first);
    assert!(r[3] < a[3] - 20.0, "the copy is lower on the page: {a:?} -> {r:?}");
    // Without Ctrl a drag still just moves it.
    let from = item_on_screen(&h, r);
    let grab = pos2(from.left() + 6.0, from.center().y);
    drag_with(&mut h, grab, grab + egui::vec2(60.0, 0.0), egui::Modifiers::NONE);
    assert_eq!(added(&h).len(), 4, "a plain drag moves, it doesn't copy");
}

/// Thai edition: ⌘Enter finishes the text being added, as Done does (and as it applies an edit
/// in Edit text), without a new line in it.
#[test]
fn command_enter_finishes_added_text_like_done() {
    let mut h = harness();
    assert!(h.state_mut().execute("edit.text"));
    h.run_steps(2);
    let p = at(&h, 40.0, 300.0);
    click(&mut h, p);
    h.event(egui::Event::Text("Hello".into()));
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
    h.run_steps(3);
    let items = added(&h);
    assert_eq!(items.len(), 1, "the text is added");
    assert_eq!(text_of(&items[0]).0, "Hello", "without a new line");
    assert!(h.state().views[0].content.draft.is_none(), "the editor closed");
    assert_eq!(h.state().quick_tool, QuickTool::Select, "back to Select, as after Done");
}
