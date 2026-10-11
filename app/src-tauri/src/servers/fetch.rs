//! Playing a server's track (PLAN.md Phase 7e): the core never goes online,
//! so the track is downloaded into the audio cache first and the engine
//! opens the file there like any other, keeping gapless hand-off,
//! ReplayGain, effects and analysis as they are.
//!
//! What is fetched (`ServerAudio::choose`): a copy already in the cache, in
//! whatever form, plays as it is; otherwise the original file, unless the
//! engine can't decode its container, or the user set a bit rate for
//! servers away from home and this one is, when the server transcodes it
//! to MP3. The queue opens tracks through `queue::opening`, which calls
//! `download` on a blocking thread before handing the file to the engine;
//! the next track is armed ahead, so it downloads while one plays.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension};

use super::address::{self, System};
use super::api::{self, ServerApi};
use super::cache::{self, AudioCache, CachedName, Form};
use super::{plex, Source};
use crate::anomp;
use crate::library::FolderKind;

/// The bit rate of a transcode when the user set none: MP3's highest.
pub const TRANSCODE_KBPS: u32 = 320;

/// The largest file downloaded: a long hi-res recording fits.
const AUDIO_LIMIT: u64 = 4 << 30;

/// A server's track as the library has it: what fetching it needs.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteFile {
    pub folder_id: i64,
    /// The kind of server.
    pub kind: FolderKind,
    /// The source's key (its folder's `path`).
    pub source_key: String,
    pub item_id: String,
    /// Of the file on the server, in bytes (0 when not known).
    pub size: i64,
    pub container: Option<String>,
    /// The path on the server that serves the file, where the id doesn't
    /// give it (`server_tracks.media_path`).
    pub media_path: Option<String>,
    /// Seconds.
    pub duration: f64,
}

impl RemoteFile {
    /// Whether the server can give the original file: a Plex track needs
    /// its part's path.
    fn original_known(&self) -> bool {
        self.kind != FolderKind::Plex || self.media_path.is_some()
    }
}

/// How tracks are fetched: the bit rate away from home, and which sources
/// are known to be on the local network.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Policy {
    /// The most kbit/s to fetch from a server not on the local network;
    /// `None` fetches originals.
    pub away_kbps: Option<u32>,
    /// By folder id, as last found (`address::is_on_local_network`); a
    /// source not checked yet counts as local.
    pub local: HashMap<i64, bool>,
}

/// The audio cache, and how tracks get into it.
pub struct ServerAudio {
    pub cache: AudioCache,
    policy: Mutex<Policy>,
}

impl ServerAudio {
    pub fn new(cache: AudioCache) -> ServerAudio {
        ServerAudio {
            cache,
            policy: Mutex::new(Policy::default()),
        }
    }

    fn policy(&self) -> std::sync::MutexGuard<'_, Policy> {
        self.policy.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_away_kbps(&self, kbps: Option<u32>) {
        self.policy().away_kbps = kbps;
    }

    pub fn set_local(&self, folder_id: i64, local: bool) {
        self.policy().local.insert(folder_id, local);
    }

    /// The cached file to play `file` from, without going online: a copy
    /// already cached (the original first), else the form to fetch now.
    pub fn choose(&self, file: &RemoteFile) -> CachedName {
        let extension =
            playable_extension(file.container.as_deref()).filter(|_| file.original_known());
        let policy = self.policy().clone();
        let transcode_kbps = policy.away_kbps.unwrap_or(TRANSCODE_KBPS);
        let original = extension.as_ref().map(|extension| CachedName {
            form: Form::Original,
            name: cache::name(
                &file.source_key,
                &file.item_id,
                file.size,
                Form::Original,
                extension,
            ),
        });
        let transcode = CachedName {
            form: Form::Mp3(transcode_kbps),
            name: cache::name(
                &file.source_key,
                &file.item_id,
                file.size,
                Form::Mp3(transcode_kbps),
                "mp3",
            ),
        };
        if let Some(original) = original.as_ref().filter(|o| self.cache.contains(&o.name)) {
            return original.clone();
        }
        if self.cache.contains(&transcode.name) {
            return transcode;
        }
        let away = !policy.local.get(&file.folder_id).copied().unwrap_or(true);
        match original {
            Some(original) if !(away && policy.away_kbps.is_some()) => original,
            _ => transcode,
        }
    }

    /// Every name `file` could be cached under: what a source signing out,
    /// or keeping it offline, covers.
    pub fn names(&self, file: &RemoteFile) -> Vec<String> {
        let away_kbps = self.policy().away_kbps;
        let mut names = Vec::new();
        if let Some(extension) = playable_extension(file.container.as_deref()) {
            names.push(cache::name(
                &file.source_key,
                &file.item_id,
                file.size,
                Form::Original,
                &extension,
            ));
        }
        for kbps in [Some(TRANSCODE_KBPS), away_kbps].into_iter().flatten() {
            names.push(cache::name(
                &file.source_key,
                &file.item_id,
                file.size,
                Form::Mp3(kbps),
                "mp3",
            ));
        }
        names.dedup();
        names
    }
}

/// The extension the engine needs for a file in `container` (as a server
/// names it, perhaps a list: "mov,mp4,m4a"), if it decodes one; `None`
/// means transcode.
pub fn playable_extension(container: Option<&str>) -> Option<String> {
    container?
        .split(',')
        .map(|name| name.trim().to_ascii_lowercase())
        .map(|name| match name.as_str() {
            "mpeg" | "mpa" => "mp3".to_owned(),
            "asf" => "wma".to_owned(),
            "oga" => "ogg".to_owned(),
            "mp4" | "mov" => "m4a".to_owned(),
            "aif" => "aiff".to_owned(),
            _ => name,
        })
        .find(|extension| {
            !extension.is_empty()
                && extension.bytes().all(|b| b.is_ascii_alphanumeric())
                && anomp::can_decode_extension(extension)
        })
}

/// Track `track_id`'s file on a server, if it is a server's track.
pub fn remote_file(
    conn: &Connection,
    track_id: i64,
) -> Result<Option<RemoteFile>, crate::library::Error> {
    Ok(conn
        .query_row(
            "SELECT t.folder_id, f.kind, f.path, t.relative_path, t.file_size, st.container,
                    st.media_path, t.duration
             FROM tracks t JOIN folders f ON f.id = t.folder_id
             LEFT JOIN server_tracks st ON st.track_id = t.id
             WHERE t.id = ?1 AND f.kind != 'local'",
            [track_id],
            |row| {
                let Some(kind) = FolderKind::from_str(&row.get::<_, String>(1)?) else {
                    return Ok(None);
                };
                Ok(Some(RemoteFile {
                    folder_id: row.get(0)?,
                    kind,
                    source_key: row.get(2)?,
                    item_id: row.get(3)?,
                    size: row.get(4)?,
                    container: row.get(5)?,
                    media_path: row.get(6)?,
                    duration: row.get(7)?,
                }))
            },
        )
        .optional()?
        .flatten())
}

/// Downloads `file` from `source` (through `server`) as `choice` says into
/// the cache, unless it is there already, keeping `keep` (the files kept
/// offline) from eviction; returns its path. Fails with `CANCELLED` once
/// `cancelled` says so, and with the server's errors as `api::error_text`
/// words them (a Plex server away from home that refuses the track, as
/// `plexPassNeeded`). Blocking: never on the main thread.
pub fn download(
    server: &ServerApi,
    source: &Source,
    file: &RemoteFile,
    choice: &CachedName,
    cache: &AudioCache,
    keep: &HashSet<String>,
    cancelled: &dyn Fn() -> bool,
) -> Result<PathBuf, String> {
    if cache.contains(&choice.name) {
        return Ok(cache.path(&choice.name));
    }
    let request = server.fetch(file, choice.form).ok_or_else(|| {
        format!(
            "Server {} gave no way to fetch track {}",
            source.folder_id, file.item_id
        )
    })?;
    let headers = request.headers();
    log::info!("downloading a track of server {}", source.folder_id);
    cache.store(&choice.name, keep, |sink| {
        server
            .http()
            .download_with_headers(&request.url, &headers, sink, AUDIO_LIMIT, cancelled)
            .map_err(|error| match error {
                crate::metadata::Error::Status { status: 403, .. }
                    if source.kind == FolderKind::Plex
                        && !address::is_on_local_network(&source.address, &System) =>
                {
                    plex::pass_needed(&source.server_name)
                }
                error => api::error_text(&error, &source.server_name),
            })
    })
}

/// The error of a download given up on, as `download` gives it.
pub fn is_cancelled(error: &str) -> bool {
    error == crate::metadata::http::CANCELLED
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db;
    use crate::metadata::http::testing::fake_client;
    use crate::servers::jellyfin::testing::{self as server, FLAC};
    use crate::servers::testing::add_test_source;

    fn file(container: Option<&str>) -> RemoteFile {
        RemoteFile {
            folder_id: 1,
            kind: FolderKind::Jellyfin,
            source_key: "jellyfin:s/u".into(),
            item_id: "item".into(),
            size: 36202,
            container: container.map(str::to_owned),
            media_path: None,
            duration: 1.5,
        }
    }

    #[test]
    fn plays_what_the_engine_decodes_and_transcodes_the_rest() {
        assert_eq!(playable_extension(Some("flac")).as_deref(), Some("flac"));
        assert_eq!(playable_extension(Some("MP3")).as_deref(), Some("mp3"));
        assert_eq!(
            playable_extension(Some("mov,mp4,m4a")).as_deref(),
            Some("m4a")
        );
        assert_eq!(playable_extension(Some("ogg")).as_deref(), Some("ogg"));
        assert_eq!(playable_extension(Some("dsf")), None);
        assert_eq!(playable_extension(Some("../x")), None);
        assert_eq!(playable_extension(None), None);
    }

    #[test]
    fn chooses_a_cached_copy_then_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let audio = ServerAudio::new(AudioCache::new(dir.path().to_path_buf(), 1 << 30));
        let flac = file(Some("flac"));
        let chosen = audio.choose(&flac);
        assert_eq!(chosen.form, Form::Original);
        assert!(chosen.name.ends_with(".flac"));
        assert_eq!(
            audio.choose(&file(Some("dsf"))).form,
            Form::Mp3(TRANSCODE_KBPS)
        );

        // Away from home with a bit rate set: a transcode at it.
        audio.set_away_kbps(Some(192));
        assert_eq!(
            audio.choose(&flac).form,
            Form::Original,
            "not known to be away"
        );
        audio.set_local(1, false);
        assert_eq!(audio.choose(&flac).form, Form::Mp3(192));
        // But a copy already cached plays as it is.
        let none = HashSet::new();
        audio
            .cache
            .store::<String>(&chosen.name, &none, |_| Ok(()))
            .unwrap();
        assert_eq!(audio.choose(&flac), chosen);
        assert_eq!(audio.names(&flac).len(), 3);

        // A Plex track whose part isn't known can only be transcoded.
        let plex = RemoteFile {
            kind: FolderKind::Plex,
            folder_id: 2,
            ..file(Some("flac"))
        };
        assert_eq!(audio.choose(&plex).form, Form::Mp3(192));
        let plex = RemoteFile {
            media_path: Some("/library/parts/1/2/file.flac".into()),
            ..plex
        };
        assert_eq!(audio.choose(&plex).form, Form::Original);
    }

    #[test]
    fn downloads_into_the_cache_once() {
        let mut conn = db::open_in_memory().unwrap();
        let folder = add_test_source(&mut conn);
        let source = crate::servers::source(&conn, folder).unwrap().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let audio = ServerAudio::new(AudioCache::new(dir.path().to_path_buf(), 1 << 30));
        let (http, transport, _) = fake_client();
        let device = server::device();
        let api = ServerApi::new(&http, &device, &source, Some("token"));
        let track = RemoteFile {
            folder_id: folder,
            source_key: source.key(),
            ..file(Some("flac"))
        };
        let choice = audio.choose(&track);
        let url = server::url("/Audio/item/stream?static=true");
        transport.push(
            &url,
            Ok(crate::metadata::http::Response {
                status: 200,
                content_type: Some("audio/flac".into()),
                retry_after: None,
                body: FLAC.to_vec(),
            }),
        );
        let fetch = || {
            download(
                &api,
                &source,
                &track,
                &choice,
                &audio.cache,
                &HashSet::new(),
                &|| false,
            )
        };
        let path = fetch().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), FLAC);
        assert!(
            crate::anomp::read_tags(&path, false).is_ok(),
            "the engine can read it"
        );
        assert_eq!(fetch().unwrap(), path);
        assert_eq!(
            transport.urls(),
            std::slice::from_ref(&url),
            "the second came from the cache"
        );
        assert!(transport.auths.lock().unwrap()[0]
            .as_deref()
            .is_some_and(|auth| auth.contains("Token=\"token\"")));

        // A server that's gone: nothing cached, and an error to show.
        std::fs::remove_file(&path).unwrap();
        transport.push(
            &url,
            Err(crate::metadata::http::TransportError::Unreachable(
                "down".into(),
            )),
        );
        let error = fetch().unwrap_err();
        assert!(crate::coded::is(&error, "serverUnreachable"), "{error}");
        assert!(!audio.cache.contains(&choice.name));
    }

    #[test]
    fn a_plex_server_away_from_home_that_refuses_wants_a_plex_pass() {
        use crate::servers::plex::testing as plex_server;
        let mut conn = db::open_in_memory().unwrap();
        let folder = crate::servers::testing::add_plex_source(&mut conn, "1", "Music");
        crate::servers::set_address(&conn, folder, "https://203.0.113.9:32400").unwrap();
        let source = crate::servers::source(&conn, folder).unwrap().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let audio = ServerAudio::new(AudioCache::new(dir.path().to_path_buf(), 1 << 30));
        let (http, transport, _) = fake_client();
        let device = server::device();
        let api = ServerApi::new(&http, &device, &source, Some(plex_server::SERVER_TOKEN));
        let track = RemoteFile {
            folder_id: folder,
            kind: FolderKind::Plex,
            source_key: source.key(),
            media_path: Some("/library/parts/827/1712000000/file.flac".into()),
            ..file(Some("flac"))
        };
        let choice = audio.choose(&track);
        transport.push_status(
            "https://203.0.113.9:32400/library/parts/827/1712000000/file.flac",
            403,
            "",
        );
        let error = download(
            &api,
            &source,
            &track,
            &choice,
            &audio.cache,
            &HashSet::new(),
            &|| false,
        )
        .unwrap_err();
        assert!(crate::coded::is(&error, "plexPassNeeded"), "{error}");
    }

    #[test]
    fn finds_server_tracks() {
        let mut conn = db::open_in_memory().unwrap();
        let folder = add_test_source(&mut conn);
        conn.execute(
            "INSERT INTO tracks (folder_id, relative_path, file_size, file_mtime_ns, duration,
                                 sample_rate, channels, scanned_at)
             VALUES (?1, 'item', 99, 0, 1.0, 44100, 2, 0)",
            [folder],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO server_tracks (track_id, container) VALUES (?1, 'flac')",
            [id],
        )
        .unwrap();
        let found = remote_file(&conn, id).unwrap().unwrap();
        assert_eq!(found.item_id, "item");
        assert_eq!(found.size, 99);
        assert_eq!(found.container.as_deref(), Some("flac"));
        assert_eq!(found.kind, FolderKind::Jellyfin);
        assert!((found.duration - 1.0).abs() < 1e-9);
        assert!(found.source_key.starts_with("jellyfin:"));
        assert_eq!(remote_file(&conn, id + 1).unwrap(), None);
    }
}
