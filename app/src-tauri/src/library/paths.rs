//! Library folders' paths as the OS spells them, and whether a file is in
//! one. On macOS and Linux these are `std::fs::canonicalize` and
//! `Path::strip_prefix`; Windows differs in two ways (PLAN.md Phase 10):
//! `canonicalize` returns a verbatim path (`\\?\C:\Music`), which other
//! paths (a file opened from Explorer, an M3U8 entry) never match, and
//! its names are case-insensitive.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// `path` made absolute with every link resolved, as `std::fs::canonicalize`
/// does, but on Windows without the verbatim prefix it adds where the plain
/// form says the same (`\\?\C:\…` is `C:\…`, `\\?\UNC\server\…` is
/// `\\server\…`). Store folders' paths through this.
pub fn canonical(path: &Path) -> std::io::Result<PathBuf> {
    let path = std::fs::canonicalize(path)?;
    if cfg!(windows) {
        if let Some(plain) = path.to_str().and_then(without_verbatim) {
            return Ok(PathBuf::from(plain));
        }
    }
    Ok(path)
}

/// A Windows verbatim path in its plain form, or `None` if `text` isn't
/// one of a drive or a share.
fn without_verbatim(text: &str) -> Option<String> {
    let rest = text.strip_prefix(r"\\?\")?;
    if let Some(share) = rest.strip_prefix(r"UNC\") {
        return (!share.is_empty()).then(|| format!(r"\\{share}"));
    }
    let bytes = rest.as_bytes();
    let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    (drive && matches!(bytes.get(2), None | Some(b'\\'))).then(|| rest.to_owned())
}

/// `path` relative to `folder`, if it is in it (or is it): component by
/// component, ignoring case on Windows, where names are case-insensitive.
pub fn within(path: &Path, folder: &Path) -> Option<PathBuf> {
    within_folding(path, folder, cfg!(windows))
}

fn within_folding(path: &Path, folder: &Path, fold_case: bool) -> Option<PathBuf> {
    if !fold_case {
        return path.strip_prefix(folder).ok().map(Path::to_path_buf);
    }
    let mut rest = path.components();
    for part in folder.components() {
        let next = rest.next()?;
        if !same_name(part.as_os_str(), next.as_os_str()) {
            return None;
        }
    }
    Some(rest.as_path().to_path_buf())
}

fn same_name(a: &OsStr, b: &OsStr) -> bool {
    a == b || a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_paths_lose_their_prefix() {
        assert_eq!(
            without_verbatim(r"\\?\C:\Users\me\Music").as_deref(),
            Some(r"C:\Users\me\Music")
        );
        assert_eq!(without_verbatim(r"\\?\D:").as_deref(), Some("D:"));
        assert_eq!(
            without_verbatim(r"\\?\UNC\nas\Music").as_deref(),
            Some(r"\\nas\Music")
        );
        // Not verbatim, or not a drive's or a share's: kept as they are.
        for kept in [
            r"C:\Music",
            r"\\nas\Music",
            r"\\?\Volume{1234}\Music",
            r"\\?\UNC\",
            "/Users/me/Music",
        ] {
            assert_eq!(without_verbatim(kept), None, "{kept}");
        }
    }

    #[test]
    fn canonical_paths_are_absolute_folders() {
        let dir = tempfile::tempdir().unwrap();
        let path = canonical(dir.path()).unwrap();
        assert!(path.is_absolute() && path.is_dir());
        if !cfg!(windows) {
            assert_eq!(path, std::fs::canonicalize(dir.path()).unwrap());
        }
    }

    #[test]
    fn files_are_within_their_folder() {
        let folder = Path::new("/Music/Rock");
        assert_eq!(
            within(Path::new("/Music/Rock/A/b.flac"), folder),
            Some(PathBuf::from("A/b.flac"))
        );
        assert_eq!(within(folder, folder), Some(PathBuf::new()));
        assert_eq!(within(Path::new("/Music/Rockabilly/b.flac"), folder), None);
        assert_eq!(within(Path::new("/Music"), folder), None);
    }

    #[test]
    fn case_is_ignored_where_names_are_case_insensitive() {
        let folder = Path::new("/Music/Rock");
        let file = Path::new("/music/ROCK/A/b.flac");
        assert_eq!(
            within_folding(file, folder, true),
            Some(PathBuf::from("A/b.flac"))
        );
        assert_eq!(within_folding(file, folder, false), None);
        assert_eq!(
            within_folding(Path::new("/Music/Rockabilly"), folder, true),
            None
        );
    }
}
