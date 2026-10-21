//! What this build calls itself. A local or patched build sets `PDFCRAFT_BUILD_LABEL` at compile
//! time (`PDFCRAFT_BUILD_LABEL=titlebar cargo build`, or through `packaging/linux/package.sh`), so
//! it can be told from the release of the same version. The version itself stays the release's:
//! the update check compares it with published releases.

/// The release version from the workspace `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The compile-time build label, if any.
pub const LABEL: Option<&str> = option_env!("PDFCRAFT_BUILD_LABEL");

/// The Thai edition's own version, such as `0.10.1-th.1`: the upstream release it is built on
/// and the edition's release number on it (Thai edition). `thai/VERSION` is the one place it is
/// written; `thai/package-windows.ps1` names the packages after it too.
const EDITION_FILE: &str = include_str!("../../../thai/VERSION");

/// The Thai edition's version, such as `0.10.1-th.1` (Thai edition).
pub fn edition_version() -> &'static str {
    EDITION_FILE.trim()
}

/// The version as shown to people: `0.10.1-th.1`, or `0.10.1-th.1 (titlebar)` for a labelled
/// build (Thai edition: the edition's version, not the bare upstream one).
pub fn version() -> String {
    describe(edition_version(), LABEL)
}

fn describe(version: &str, label: Option<&str>) -> String {
    match label.map(str::trim).filter(|l| !l.is_empty()) {
        Some(label) => format!("{version} ({label})"),
        None => version.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{VERSION, describe, edition_version};

    #[test]
    fn a_label_follows_the_version_and_a_blank_one_is_ignored() {
        assert_eq!(describe("0.7.1", None), "0.7.1");
        assert_eq!(describe("0.7.1", Some("")), "0.7.1");
        assert_eq!(describe("0.7.1", Some("  ")), "0.7.1");
        assert_eq!(describe("0.7.1", Some("titlebar")), "0.7.1 (titlebar)");
    }

    /// After merging an upstream release, `thai/VERSION` must move to it (Thai edition):
    /// `<upstream version>-th.<n>` with n ≥ 1.
    #[test]
    fn the_edition_version_is_built_on_this_upstream_release() {
        let v = edition_version();
        let n = v
            .strip_prefix(VERSION)
            .and_then(|rest| rest.strip_prefix("-th."))
            .unwrap_or_else(|| panic!("thai/VERSION is {v:?}, not {VERSION}-th.<n>"));
        assert!(n.parse::<u64>().is_ok_and(|n| n >= 1), "thai/VERSION is {v:?}: the edition number must be a whole number from 1");
    }
}
