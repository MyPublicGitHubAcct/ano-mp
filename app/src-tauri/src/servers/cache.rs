//! The audio cache (PLAN.md Phase 7e): music servers' tracks, downloaded
//! for the engine to open like any file. In the app's cache folder (inside
//! the container under the macOS sandbox), apart from the image cache but
//! built like it: a file is named by a SHA-256, written under a temporary
//! name (`.part`) and renamed into place, and beyond a budget (1 GB by
//! default, `ServerSettings::cache_mb`) the least recently used files go
//! first. How recently a file was used is its modification time, moved on
//! when it is played.
//!
//! A cached track's name is the SHA-256 of its source's key, its item id,
//! its file's size (so a file replaced on the server is fetched again) and
//! the form fetched (the original, or an MP3 transcode at a bit rate), with
//! the extension the engine needs to pick its decoder. Files kept offline
//! (`offline`) and the last few opened for playback (`RECENT`: what plays,
//! what is armed next, what was just downloaded) are never evicted.

use std::collections::{HashSet, VecDeque};
use std::fs::{self, File};
use std::io::Write;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// The default budget: about 30 albums of FLAC, or 150 of MP3.
pub const DEFAULT_BUDGET_MB: u32 = 1024;

/// Temporary files older than this were left by a crash and are removed.
const STALE_PART: Duration = Duration::from_secs(3600);

const PART_SUFFIX: &str = ".part";

/// Numbers temporary files, so two downloads of one track don't share one.
static NEXT_PART: AtomicU64 = AtomicU64::new(0);

/// How many of the files opened most recently are kept from eviction: the
/// engine may open the current or the next track's file again (a seek back
/// reopens it), so neither may go while it plays.
const RECENT: usize = 4;

/// What is fetched of a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Form {
    /// The file as it is on the server.
    Original,
    /// An MP3 the server transcodes, at this many kbit/s.
    Mp3(u32),
}

impl Form {
    fn tag(self) -> String {
        match self {
            Form::Original => "original".into(),
            Form::Mp3(kbps) => format!("mp3-{kbps}"),
        }
    }
}

/// A track's cached file: what to fetch, and the name it is kept under.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedName {
    pub form: Form,
    pub name: String,
}

/// The name of a cached track: the SHA-256 (hex) of the source's key, the
/// item id, the file's size and the form, then `extension`.
pub fn name(source_key: &str, item_id: &str, size: i64, form: Form, extension: &str) -> String {
    let identity = format!("{source_key}\n{item_id}\n{size}\n{}", form.tag());
    let digest = ring::digest::digest(&ring::digest::SHA256, identity.as_bytes());
    let hex: String = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{hex}.{extension}")
}

fn is_cache_name(name: &str) -> bool {
    let Some((hex, extension)) = name.split_once('.') else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && !extension.is_empty()
        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

pub struct AudioCache {
    dir: PathBuf,
    budget: AtomicU64,
    /// The files opened most recently, newest last: never evicted.
    recent: Mutex<VecDeque<String>>,
}

/// A cached file: its name, size and when it was last used.
struct Entry {
    name: String,
    path: PathBuf,
    size: u64,
    used: SystemTime,
}

impl AudioCache {
    /// A cache in `dir` (created when the first track is stored) of at most
    /// `budget` bytes.
    pub fn new(dir: PathBuf, budget: u64) -> AudioCache {
        AudioCache {
            dir,
            budget: AtomicU64::new(budget),
            recent: Mutex::new(VecDeque::new()),
        }
    }

    #[cfg(test)]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn set_budget(&self, budget: u64) {
        self.budget.store(budget, Ordering::Relaxed);
    }

    pub fn budget(&self) -> u64 {
        self.budget.load(Ordering::Relaxed)
    }

    fn recent(&self) -> std::sync::MutexGuard<'_, VecDeque<String>> {
        self.recent.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.path(name).is_file()
    }

    /// `name` is being opened for playback: marks it used now and keeps it
    /// from eviction until `RECENT` others have been.
    pub fn opened(&self, name: &str) {
        self.touch(name);
        let mut recent = self.recent();
        recent.retain(|other| other != name);
        recent.push_back(name.to_owned());
        while recent.len() > RECENT {
            recent.pop_front();
        }
    }

    /// Marks `name` used now, so it is evicted last.
    pub fn touch(&self, name: &str) {
        // Windows needs write access to change the time; nothing is written.
        let _ = File::options()
            .write(true)
            .open(self.path(name))
            .and_then(|file| file.set_modified(SystemTime::now()));
    }

    /// Writes `name` with `fill`, into a temporary file renamed into place
    /// once `fill` succeeds, so a half-written track is never played; then
    /// evicts down to the budget, keeping `keep` (the files kept offline).
    /// Returns the file's path.
    pub fn store<E: From<String>>(
        &self,
        name: &str,
        keep: &HashSet<String>,
        fill: impl FnOnce(&mut File) -> Result<(), E>,
    ) -> Result<PathBuf, E> {
        let failed = |error: std::io::Error| {
            E::from(format!(
                "Cannot write to the audio cache {}: {error}",
                self.dir.display()
            ))
        };
        let path = self.path(name);
        let part = self.dir.join(format!(
            "{name}.{}-{}{PART_SUFFIX}",
            std::process::id(),
            NEXT_PART.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&self.dir).map_err(failed)?;
        let mut file = File::create(&part).map_err(failed)?;
        let filled = fill(&mut file);
        let written = filled.and_then(|()| {
            file.flush()
                .and_then(|()| file.sync_data())
                .map_err(failed)?;
            drop(file);
            fs::rename(&part, &path).map_err(failed)
        });
        if let Err(error) = written {
            let _ = fs::remove_file(&part);
            return Err(error);
        }
        self.opened(name);
        self.evict(keep);
        Ok(path)
    }

    /// The cached files. Removes temporary files left by a crash, and
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
            let name = entry.file_name().to_string_lossy().into_owned();
            let used = metadata.modified().unwrap_or(now);
            if !metadata.is_file() {
                continue;
            } else if is_cache_name(&name) {
                entries.push(Entry {
                    name,
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

    /// How many files the cache holds and their total size in bytes.
    pub fn usage(&self) -> (usize, u64) {
        let entries = self.entries();
        (entries.len(), entries.iter().map(|entry| entry.size).sum())
    }

    /// Removes the least recently used files, except `keep` and those in
    /// use, until the cache is within its budget; returns the bytes left.
    pub fn evict(&self, keep: &HashSet<String>) -> u64 {
        let budget = self.budget();
        let mut entries = self.entries();
        let mut total: u64 = entries.iter().map(|entry| entry.size).sum();
        if total <= budget {
            return total;
        }
        entries.sort_by_key(|entry| entry.used);
        let recent = self.recent();
        for entry in entries {
            if total <= budget {
                break;
            }
            if keep.contains(&entry.name) || recent.contains(&entry.name) {
                continue;
            }
            // One that can't be removed (e.g. open on Windows) still counts.
            if fs::remove_file(&entry.path).is_ok() {
                total -= entry.size;
            }
        }
        total
    }

    /// Removes the files `names` (a source signed out of). The engine
    /// keeps one it has open readable until it lets go.
    pub fn remove(&self, names: &HashSet<String>) {
        self.recent().retain(|name| !names.contains(name));
        for entry in self.entries() {
            if names.contains(&entry.name) {
                let _ = fs::remove_file(&entry.path);
            }
        }
    }

    /// Removes every file except `keep` (the files kept offline) and the
    /// last few opened (`RECENT`).
    pub fn clear(&self, keep: &HashSet<String>) {
        let recent = self.recent();
        for entry in self.entries() {
            if !recent.contains(&entry.name) && !keep.contains(&entry.name) {
                let _ = fs::remove_file(&entry.path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(cache: &AudioCache, name: &str, bytes: usize, keep: &HashSet<String>) -> PathBuf {
        cache
            .store::<String>(name, keep, |file| {
                file.write_all(&vec![7; bytes]).map_err(|e| e.to_string())
            })
            .unwrap()
    }

    fn named(n: u8, extension: &str) -> String {
        name(
            "jellyfin:s/u",
            &format!("item{n}"),
            100,
            Form::Original,
            extension,
        )
    }

    fn age(cache: &AudioCache, name: &str, seconds: u64) {
        File::options()
            .write(true)
            .open(cache.path(name))
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(seconds))
            .unwrap();
    }

    #[test]
    fn names_say_what_was_fetched() {
        let original = name("jellyfin:s/u", "abc", 100, Form::Original, "flac");
        assert!(is_cache_name(&original), "{original}");
        assert!(original.ends_with(".flac"));
        assert_eq!(
            original,
            name("jellyfin:s/u", "abc", 100, Form::Original, "flac")
        );
        for other in [
            name("jellyfin:s/u", "abc", 101, Form::Original, "flac"),
            name("jellyfin:s/u", "abd", 100, Form::Original, "flac"),
            name("jellyfin:t/u", "abc", 100, Form::Original, "flac"),
            name("jellyfin:s/u", "abc", 100, Form::Mp3(192), "flac"),
        ] {
            assert_ne!(other[..64], original[..64]);
        }
        assert!(!is_cache_name("notes.txt"));
        assert!(!is_cache_name(&format!("{original}.1-2.part")));
    }

    #[test]
    fn stores_whole_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let cache = AudioCache::new(dir.path().join("audio"), 1 << 20);
        let name = named(1, "flac");
        let path = store(&cache, &name, 10, &HashSet::new());
        assert_eq!(fs::read(&path).unwrap(), vec![7; 10]);
        assert!(cache.contains(&name));
        assert_eq!(cache.usage(), (1, 10));

        // A download that fails leaves nothing behind.
        let broken = named(2, "mp3");
        let error = cache
            .store::<String>(&broken, &HashSet::new(), |file| {
                file.write_all(b"half").unwrap();
                Err("connection lost".into())
            })
            .unwrap_err();
        assert_eq!(error, "connection lost");
        assert!(!cache.contains(&broken));
        assert_eq!(fs::read_dir(cache.dir()).unwrap().count(), 1);
    }

    #[test]
    fn evicts_the_least_recently_used_except_kept_and_held_files() {
        let dir = tempfile::tempdir().unwrap();
        let cache = AudioCache::new(dir.path().to_path_buf(), 1000);
        let (a, b, c, d) = (
            named(1, "flac"),
            named(2, "flac"),
            named(3, "flac"),
            named(4, "flac"),
        );
        let none = HashSet::new();
        for (name, seconds) in [(&a, 400), (&b, 300), (&c, 200)] {
            store(&cache, name, 300, &none);
            age(&cache, name, seconds);
        }
        // `a` is kept offline and `b` is playing: `c` goes, though newer.
        let keep: HashSet<String> = [a.clone()].into();
        for other in ["x", "y", "z"] {
            cache.opened(other);
        }
        cache.opened(&b);
        age(&cache, &b, 300);
        store(&cache, &d, 300, &keep);
        assert!(cache.contains(&a) && cache.contains(&b) && cache.contains(&d));
        assert!(!cache.contains(&c));

        // Others opened since: `b` can go, before `a`, which was just played.
        for other in ["v", "w", "x", "y"] {
            cache.opened(other);
        }
        cache.touch(&a);
        cache.set_budget(600);
        cache.evict(&none);
        assert!(cache.contains(&a));
        assert!(!cache.contains(&b));
    }

    #[test]
    fn removes_a_sources_files_and_crash_leftovers() {
        let dir = tempfile::tempdir().unwrap();
        let cache = AudioCache::new(dir.path().to_path_buf(), 1 << 20);
        let (a, b) = (named(1, "flac"), named(2, "mp3"));
        let none = HashSet::new();
        store(&cache, &a, 10, &none);
        store(&cache, &b, 10, &none);
        let stale = dir.path().join(format!("{a}.1-1.part"));
        fs::write(&stale, "x").unwrap();
        File::options()
            .write(true)
            .open(&stale)
            .unwrap()
            .set_modified(SystemTime::now() - STALE_PART * 2)
            .unwrap();
        fs::write(dir.path().join("readme.txt"), "not ours").unwrap();

        cache.remove(&[a.clone()].into());
        assert!(!cache.contains(&a) && cache.contains(&b));
        assert!(!stale.exists());
        for other in ["1", "2", "3", "4"] {
            cache.opened(other);
        }
        cache.clear(&HashSet::new());
        assert!(!cache.contains(&b));
        assert!(dir.path().join("readme.txt").exists());
    }
}
