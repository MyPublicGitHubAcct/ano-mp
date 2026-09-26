//! Pictures downloaded from online services, kept on disk: one file per
//! image URL in a folder of the app's cache dir (inside the container under
//! the macOS sandbox), not in SQLite. Every picture can be downloaded again,
//! so losing the folder is harmless; beyond a size budget the least recently
//! used files are removed.
//!
//! A file is named by the SHA-256 of its URL in hex. The name has to be the
//! same in every build, which `std`'s `DefaultHasher` doesn't promise, and
//! SHA-256 is fixed by its standard (FIPS 180-4). A file is written under a
//! temporary name and renamed into place, so a half-written picture is never
//! served. How recently a file was used is its modification time, moved on
//! when it's read.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use super::http::IMAGE_LIMIT;
use super::Error;

/// The cache's size budget: about 5,000 covers at the 500 px size (around
/// 100 KB each), more than most libraries have albums.
pub const BUDGET: u64 = 500 << 20;

/// Reading a file marks it used at most this often, so browsing doesn't
/// write to the disk for every picture. Eviction only needs rough order.
const TOUCH_AFTER: Duration = Duration::from_secs(86400);

/// Temporary files older than this were left by a crash and are removed.
const STALE_PART: Duration = Duration::from_secs(3600);

const PART_SUFFIX: &str = ".part";

/// Numbers temporary files, so two writes of one URL don't share a name.
static NEXT_PART: AtomicU64 = AtomicU64::new(0);

pub struct ImageCache {
    dir: PathBuf,
    budget: u64,
    /// The total size of the files, once counted by the first store.
    bytes: Mutex<Option<u64>>,
}

/// A cached picture's file: its size and when it was last used.
struct Entry {
    path: PathBuf,
    size: u64,
    used: SystemTime,
}

impl ImageCache {
    /// A cache in `dir`, which is created when the first picture is stored.
    pub fn new(dir: PathBuf) -> ImageCache {
        ImageCache::with_budget(dir, BUDGET)
    }

    pub fn with_budget(dir: PathBuf, budget: u64) -> ImageCache {
        ImageCache {
            dir,
            budget,
            bytes: Mutex::new(None),
        }
    }

    fn path(&self, url: &str) -> PathBuf {
        self.dir.join(file_name(url))
    }

    pub fn contains(&self, url: &str) -> bool {
        self.path(url).is_file()
    }

    /// The picture downloaded from `url` and its MIME type, if it's cached.
    pub fn get(&self, url: &str) -> Option<(&'static str, Vec<u8>)> {
        let path = self.path(url);
        let mut file = File::open(&path).ok()?;
        let metadata = file.metadata().ok()?;
        if metadata.len() > IMAGE_LIMIT {
            return None;
        }
        let mut data = Vec::with_capacity(metadata.len() as usize);
        file.read_to_end(&mut data).ok()?;
        let mime_type = image_type(&data)?;
        let age = metadata
            .modified()
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok());
        if age.is_some_and(|age| age > TOUCH_AFTER) {
            // Only recency is lost if this fails.
            let _ = set_used(&path, SystemTime::now());
        }
        Some((mime_type, data))
    }

    /// Stores `data`, downloaded from `url`, replacing any earlier copy, and
    /// removes the least recently used pictures if the cache is over budget.
    pub fn store(&self, url: &str, data: &[u8]) -> Result<(), Error> {
        if image_type(data).is_none() {
            return Err(Error::Invalid(format!(
                "{url}: not a JPEG, PNG, GIF or WebP image"
            )));
        }
        let failed = |error: std::io::Error| {
            Error::Invalid(format!(
                "Cannot write to the image cache {}: {error}",
                self.dir.display()
            ))
        };
        let path = self.path(url);
        let part = self.dir.join(format!(
            "{}.{}-{}{PART_SUFFIX}",
            file_name(url),
            std::process::id(),
            NEXT_PART.fetch_add(1, Ordering::Relaxed)
        ));
        let replaced = fs::metadata(&path).map_or(0, |metadata| metadata.len());
        let written = fs::create_dir_all(&self.dir)
            .and_then(|()| {
                let mut file = File::create(&part)?;
                file.write_all(data)?;
                // On disk before the rename, so a power cut can't leave a
                // short file under the final name.
                file.sync_data()
            })
            .and_then(|()| fs::rename(&part, &path));
        if let Err(error) = written {
            let _ = fs::remove_file(&part);
            return Err(failed(error));
        }

        let mut bytes = self.bytes.lock().unwrap_or_else(|e| e.into_inner());
        let total = match *bytes {
            Some(total) => (total + data.len() as u64).saturating_sub(replaced),
            None => self.entries().iter().map(|entry| entry.size).sum(),
        };
        // Down to 90% of the budget, so the next few stores don't evict
        // again.
        *bytes = Some(if total > self.budget {
            self.evict(self.budget / 10 * 9, &path)
        } else {
            total
        });
        Ok(())
    }

    /// The cached pictures. Removes temporary files left by a crash, and
    /// leaves alone anything that isn't the cache's.
    fn entries(&self) -> Vec<Entry> {
        let Ok(dir) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let now = SystemTime::now();
        let mut entries = Vec::new();
        for entry in dir.filter_map(Result::ok) {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let used = metadata.modified().unwrap_or(now);
            if !metadata.is_file() {
                continue;
            } else if is_cache_name(&name) {
                entries.push(Entry {
                    path: entry.path(),
                    size: metadata.len(),
                    used,
                });
            } else if name.ends_with(PART_SUFFIX)
                && now.duration_since(used).unwrap_or_default() > STALE_PART
            {
                let _ = fs::remove_file(entry.path());
            }
        }
        entries
    }

    /// Removes the least recently used pictures, except `keep`, until at
    /// most `target` bytes are left; returns the bytes left.
    fn evict(&self, target: u64, keep: &Path) -> u64 {
        let mut entries = self.entries();
        let mut total: u64 = entries.iter().map(|entry| entry.size).sum();
        entries.sort_by_key(|entry| entry.used);
        for entry in entries {
            if total <= target {
                break;
            }
            // One that can't be removed (e.g. open on Windows) still counts.
            if entry.path != keep && fs::remove_file(&entry.path).is_ok() {
                total -= entry.size;
            }
        }
        total
    }
}

/// The file name for the picture from `url`: the SHA-256 of the URL, in
/// lower-case hex (64 characters).
pub fn file_name(url: &str) -> String {
    ring::digest::digest(&ring::digest::SHA256, url.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_cache_name(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn set_used(path: &Path, time: SystemTime) -> std::io::Result<()> {
    // Windows needs write access to change the time; nothing is written.
    File::options().write(true).open(path)?.set_modified(time)
}

/// The MIME type of an image, from its first bytes; `None` if it isn't one
/// the webview shows (e.g. an HTML error page served with a 200).
pub fn image_type(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

#[cfg(test)]
pub mod testing {
    /// A tiny real JPEG (8×8 px), as the Cover Art Archive would serve.
    pub const JPEG: &[u8] = include_bytes!("fixtures/coverartarchive/front-500.jpg");

    /// A JPEG of `size` bytes: a JPEG header and padding.
    pub fn jpeg_of_size(size: usize) -> Vec<u8> {
        let mut data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        data.resize(size, 0);
        data
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    const URL: &str =
        "https://coverartarchive.org/release/1a33443c-3fff-450f-8298-efbc65659d32/front-500";

    fn days_ago(days: u64) -> SystemTime {
        SystemTime::now() - Duration::from_secs(days * 86400)
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn names_files_by_a_stable_hash() {
        // `printf '%s' URL | shasum -a 256`
        assert_eq!(
            file_name(URL),
            "f54617e73807b00425c3036c120203cf728f1e7e1c296f2d7a1d0e9117ec0cbd"
        );
        assert!(is_cache_name(&file_name("")));
        assert!(!is_cache_name(&format!("{}.1-0.part", file_name(URL))));
        assert!(!is_cache_name(".DS_Store"));
    }

    #[test]
    fn recognizes_images() {
        assert_eq!(image_type(JPEG), Some("image/jpeg"));
        assert_eq!(image_type(b"\x89PNG\r\n\x1a\n...."), Some("image/png"));
        assert_eq!(image_type(b"GIF89a...."), Some("image/gif"));
        assert_eq!(image_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        for bad in [&b""[..], b"<html>", b"RIFF\0\0\0\0WAVE", b"\xFF\xD8"] {
            assert_eq!(image_type(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn stores_and_serves_pictures() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(dir.path().join("images"));
        assert_eq!(cache.get(URL), None);
        assert!(!cache.contains(URL));

        cache.store(URL, JPEG).unwrap();
        assert!(cache.contains(URL));
        assert_eq!(cache.get(URL), Some(("image/jpeg", JPEG.to_vec())));
        // Only the picture is left: the temporary file was renamed.
        assert_eq!(names(&dir.path().join("images")), [file_name(URL)]);

        // Replaced by a new copy; something that isn't an image is refused.
        cache.store(URL, &jpeg_of_size(100)).unwrap();
        assert_eq!(cache.get(URL).unwrap().1.len(), 100);
        assert!(cache.store(URL, b"<html>Not found</html>").is_err());
        assert_eq!(cache.get(URL).unwrap().1.len(), 100);

        // A file that is no longer an image isn't served.
        fs::write(dir.path().join("images").join(file_name(URL)), b"").unwrap();
        assert_eq!(cache.get(URL), None);
    }

    #[test]
    fn evicts_the_least_recently_used_beyond_the_budget() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ImageCache::with_budget(dir.path().to_path_buf(), 3500);
        let url = |name: &str| format!("https://example.com/{name}.jpg");
        for (name, days) in [("a", 3), ("b", 2), ("c", 1)] {
            cache.store(&url(name), &jpeg_of_size(1000)).unwrap();
            set_used(&cache.path(&url(name)), days_ago(days)).unwrap();
        }
        // Reading "a" makes it the most recently used.
        assert!(cache.get(&url("a")).is_some());

        // 4000 bytes: down to 90% of the budget by removing "b", the least
        // recently used.
        cache.store(&url("d"), &jpeg_of_size(1000)).unwrap();
        let cached: Vec<bool> = ["a", "b", "c", "d"]
            .iter()
            .map(|name| cache.contains(&url(name)))
            .collect();
        assert_eq!(cached, [true, false, true, true]);
        assert_eq!(*cache.bytes.lock().unwrap(), Some(3000));

        // A picture larger than the budget is kept until the next one.
        cache.store(&url("e"), &jpeg_of_size(5000)).unwrap();
        assert!(cache.contains(&url("e")));
        assert!(!cache.contains(&url("a")));
    }

    #[test]
    fn counts_what_an_earlier_run_left_and_removes_stale_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let stale = dir
            .path()
            .join(format!("{}.1-0{PART_SUFFIX}", file_name("x")));
        let fresh = dir
            .path()
            .join(format!("{}.1-1{PART_SUFFIX}", file_name("y")));
        for path in [&stale, &fresh] {
            fs::write(path, jpeg_of_size(10)).unwrap();
        }
        set_used(&stale, days_ago(1)).unwrap();
        fs::write(dir.path().join("notes.txt"), b"not the cache's").unwrap();
        ImageCache::new(dir.path().to_path_buf())
            .store(URL, &jpeg_of_size(1000))
            .unwrap();
        // A second cache over the same folder counts what is there.
        let cache = ImageCache::new(dir.path().to_path_buf());
        cache
            .store(&format!("{URL}?2"), &jpeg_of_size(500))
            .unwrap();
        assert_eq!(*cache.bytes.lock().unwrap(), Some(1500));
        assert!(!stale.exists());
        assert!(fresh.exists(), "may still be being written");
        assert!(dir.path().join("notes.txt").exists());
    }
}
