//! The "folder" art source: an image file next to an album's tracks, such as
//! `cover.jpg` or `folder.png`, as ripping and tagging tools save them.
//! Reading it needs the library folder open (`library::access`).
//!
//! A picture is read only from one of its album's folders, and never
//! through a symbolic link (PLAN.md H4): the macOS sandbox would stop a
//! link leading out of the library, but Linux and Windows have none. The
//! folders are those `folders_for` gives for the album's tracks, reached by
//! the same path the scanner found the tracks by, so an album in a linked
//! folder (the scanner follows links) keeps its cover.

use std::path::{Path, PathBuf};

/// File names (without extension, ignoring case) taken as album art, best
/// first. Windows Media Player's `AlbumArt_{…}_Large.jpg` and
/// `AlbumArtSmall.jpg` come after these, large before small.
const NAMES: [&str; 5] = ["cover", "folder", "front", "album", "albumart"];

const EXTENSIONS: [(&str, &str); 4] = [
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("png", "image/png"),
    ("webp", "image/webp"),
];

/// Larger files aren't read.
const MAX_SIZE: u64 = 32 << 20;

/// The MIME type of an image file this source reads, from its extension.
pub fn mime_type(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    EXTENSIONS
        .iter()
        .find(|(known, _)| *known == extension)
        .map(|(_, mime)| *mime)
}

/// How good a name is as album art (lower is better), or `None` if it isn't.
fn rank(file_name: &str) -> Option<usize> {
    if file_name.starts_with('.') {
        return None;
    }
    mime_type(Path::new(file_name))?;
    let stem = Path::new(file_name).file_stem()?.to_str()?.to_lowercase();
    if let Some(index) = NAMES.iter().position(|name| *name == stem) {
        return Some(index);
    }
    if stem.starts_with("albumart") {
        return Some(NAMES.len() + usize::from(!stem.contains("large")));
    }
    None
}

/// The best album-art image directly in `dir`, if any. Ties (e.g.
/// `cover.jpg` and `cover.png`) go to the name that sorts first.
pub fn find(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            Some((rank(&name)?, name))
        })
        .min()
        .map(|(_, name)| dir.join(name))
}

/// The folders to look in for the art of a track stored as `relative` in the
/// library folder `root`: its own folder, and for a track in a disc folder
/// ("CD1", "Disc 2") the album folder above it, where the cover usually is.
/// Never above `root`.
pub fn folders_for(root: &Path, relative: &str) -> Vec<PathBuf> {
    relative_folders(relative)
        .into_iter()
        .map(|dir| {
            let mut path = root.to_path_buf();
            path.extend(dir.split('/').filter(|part| !part.is_empty()));
            path
        })
        .collect()
}

/// `folders_for` relative to the library folder, '/'-separated like track
/// paths; "" is the library folder itself.
pub fn relative_folders(relative: &str) -> Vec<String> {
    let mut parts: Vec<&str> = relative.split('/').collect();
    parts.pop(); // The file name.
    let mut folders = vec![parts.join("/")];
    if parts.len() > 1 && parts.last().is_some_and(|name| is_disc_folder(name)) {
        parts.pop();
        folders.push(parts.join("/"));
    }
    folders
}

/// The names of every image file directly in `dir` that this source can
/// read, the names it takes as album art first (best first), then the rest
/// by name. For the user to choose from.
pub fn images(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<(usize, String, String)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.') && mime_type(Path::new(name)).is_some())
        .map(|name| (rank(&name).unwrap_or(usize::MAX), name.to_lowercase(), name))
        .collect();
    names.sort();
    names.into_iter().map(|(_, _, name)| name).collect()
}

/// Whether `reference`, a picture's path relative to its library folder
/// (from the UI, or stored), stays inside that folder: '/'-separated parts,
/// none empty, "." or "..", and nothing a platform reads as a root or drive.
pub fn is_relative_path(reference: &str) -> bool {
    !reference.is_empty()
        && reference.split('/').all(|part| {
            !part.is_empty() && part != "." && part != ".." && !part.contains(['\\', ':'])
        })
}

/// The picture `reference` names, a path relative to the library folder
/// `root`, if it may be read as album art: a relative path
/// (`is_relative_path`) to a file directly in one of `folders`, the
/// album's folders as `relative_folders` gives them, compared as text.
/// `read` then refuses it if it is a symbolic link.
pub fn contained<'a>(
    root: &Path,
    reference: &str,
    folders: impl IntoIterator<Item = &'a str>,
) -> Option<PathBuf> {
    if !is_relative_path(reference) {
        return None;
    }
    let dir = reference.rsplit_once('/').map_or("", |(dir, _)| dir);
    if !folders.into_iter().any(|folder| folder == dir) {
        return None;
    }
    let mut path = root.to_path_buf();
    path.extend(reference.split('/'));
    Some(path)
}

/// Whether `path` is a regular file and not a symbolic link (`lstat`, which
/// never downloads a cloud placeholder).
pub fn is_plain_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
}

/// "CD1", "cd 2", "Disc 03", "Disk-1": a word for disc, then a number.
fn is_disc_folder(name: &str) -> bool {
    let lower = name.to_lowercase();
    let Some(rest) = ["cd", "disc", "disk"]
        .iter()
        .find_map(|word| lower.strip_prefix(word))
    else {
        return false;
    };
    let digits = rest.trim_start_matches([' ', '-', '_', '.']);
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

/// The image at `path` and its MIME type, if it's a readable image file of a
/// sensible size and not a symbolic link.
pub fn read(path: &Path) -> Option<(&'static str, Vec<u8>)> {
    let mime = mime_type(path)?;
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if metadata.file_type().is_symlink() {
        log::debug!(
            "not reading a picture that is a symbolic link: {}",
            path.display()
        );
        return None;
    }
    let size = metadata.len();
    if !metadata.file_type().is_file() || size == 0 || size > MAX_SIZE {
        return None;
    }
    Some((mime, std::fs::read(path).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"image").unwrap();
    }

    #[test]
    fn ranks_names() {
        assert_eq!(rank("cover.jpg"), Some(0));
        assert_eq!(rank("Folder.JPG"), Some(1));
        assert_eq!(rank("front.webp"), Some(2));
        assert_eq!(rank("AlbumArt_{A1B2}_Large.jpg"), Some(5));
        assert_eq!(rank("AlbumArtSmall.jpg"), Some(6));
        for name in ["cover.txt", "back.jpg", ".cover.jpg", "cover", "coverjpg"] {
            assert_eq!(rank(name), None, "{name}");
        }
    }

    #[test]
    fn finds_the_best_image() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(find(dir.path()), None);
        touch(dir.path(), "AlbumArtSmall.jpg");
        touch(dir.path(), "back.jpg");
        assert_eq!(find(dir.path()), Some(dir.path().join("AlbumArtSmall.jpg")));
        touch(dir.path(), "Folder.png");
        touch(dir.path(), "cover.png");
        touch(dir.path(), "cover.jpg");
        std::fs::create_dir(dir.path().join("cover.jpeg")).unwrap(); // Not a file.
        assert_eq!(find(dir.path()), Some(dir.path().join("cover.jpg")));
        assert_eq!(find(&dir.path().join("missing")), None);
    }

    #[test]
    fn looks_above_disc_folders_but_not_above_the_root() {
        let root = Path::new("/Music");
        assert_eq!(
            folders_for(root, "Artist/Album/CD 2/01.flac"),
            [root.join("Artist/Album/CD 2"), root.join("Artist/Album")]
        );
        assert_eq!(
            folders_for(root, "Artist/Album/01.flac"),
            [root.join("Artist/Album")]
        );
        assert_eq!(folders_for(root, "Disc1/01.flac"), [root.join("Disc1")]);
        assert_eq!(folders_for(root, "01.flac"), [root.to_path_buf()]);
        for name in ["CD1", "cd 2", "Disc 03", "disk-1", "DISC_4"] {
            assert!(is_disc_folder(name), "{name}");
        }
        for name in ["CD", "Discography", "cd one", "Disc 1 (Live)", "ACDC"] {
            assert!(!is_disc_folder(name), "{name}");
        }
    }

    #[test]
    fn lists_every_image_art_names_first() {
        let dir = tempfile::tempdir().unwrap();
        assert!(images(dir.path()).is_empty());
        for name in [
            "back.jpg",
            "Booklet 2.png",
            "cover.jpg",
            ".hidden.jpg",
            "notes.txt",
        ] {
            touch(dir.path(), name);
        }
        std::fs::create_dir(dir.path().join("scans.jpg")).unwrap(); // Not a file.
        assert_eq!(
            images(dir.path()),
            ["cover.jpg", "back.jpg", "Booklet 2.png"]
        );
        assert!(images(&dir.path().join("missing")).is_empty());
    }

    #[test]
    fn relative_folders_match_folders_for() {
        assert_eq!(
            relative_folders("Artist/Album/CD 2/01.flac"),
            ["Artist/Album/CD 2", "Artist/Album"]
        );
        assert_eq!(relative_folders("01.flac"), [""]);
    }

    #[test]
    fn relative_paths_stay_inside() {
        for good in ["cover.jpg", "Artist/Album/Scans/back.png", "a..b/c.jpg"] {
            assert!(is_relative_path(good), "{good}");
        }
        for bad in [
            "",
            "/etc/cover.jpg",
            "../cover.jpg",
            "Artist/../../x.jpg",
            "Artist//x.jpg",
            "./x.jpg",
            "C:/x.jpg",
            "Artist\\..\\x.jpg",
        ] {
            assert!(!is_relative_path(bad), "{bad}");
        }
    }

    #[test]
    fn reads_images_of_a_sensible_size() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "cover.png");
        assert_eq!(
            read(&dir.path().join("cover.png")),
            Some(("image/png", b"image".to_vec()))
        );
        std::fs::write(dir.path().join("empty.jpg"), b"").unwrap();
        assert_eq!(read(&dir.path().join("empty.jpg")), None);
        assert_eq!(read(&dir.path().join("missing.jpg")), None);
    }

    #[test]
    fn contains_pictures_to_the_albums_folders() {
        let root = Path::new("/Music");
        let folders = relative_folders("Artist/Album/CD 2/01.flac");
        let contained = |reference| contained(root, reference, folders.iter().map(String::as_str));
        assert_eq!(
            contained("Artist/Album/CD 2/back.png"),
            Some(root.join("Artist/Album/CD 2/back.png"))
        );
        assert_eq!(
            contained("Artist/Album/cover.jpg"),
            Some(root.join("Artist/Album/cover.jpg"))
        );
        assert_eq!(
            super::contained(root, "cover.jpg", [""]),
            Some(root.join("cover.jpg"))
        );
        for outside in [
            "Artist/Album/CD 1/back.png",
            "Artist/Album/Scans/back.png",
            "Artist/cover.jpg",
            "Other/Album/cover.jpg",
            "cover.jpg",
            "Artist/Album/CD 2/../cover.jpg",
            "/Artist/Album/cover.jpg",
        ] {
            assert_eq!(contained(outside), None, "{outside}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn never_reads_through_a_symbolic_link() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        touch(elsewhere.path(), "secret.png");
        std::os::unix::fs::symlink(
            elsewhere.path().join("secret.png"),
            dir.path().join("cover.png"),
        )
        .unwrap();
        assert!(!is_plain_file(&dir.path().join("cover.png")));
        assert_eq!(read(&dir.path().join("cover.png")), None);
        assert_eq!(find(dir.path()), None);
        assert!(images(dir.path()).is_empty());
        touch(dir.path(), "folder.png");
        assert!(is_plain_file(&dir.path().join("folder.png")));
        assert_eq!(find(dir.path()), Some(dir.path().join("folder.png")));
    }
}
