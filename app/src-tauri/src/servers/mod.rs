//! Music servers as library sources (PLAN.md Phases 7e and 7f): the music
//! on the user's own Jellyfin or Plex server, next to the local library,
//! streamed on demand through the same browser, search, queue, gapless
//! hand-off and Now Playing as local files.
//!
//! A server, as one user sees it, is a library folder of its own kind
//! (`FolderKind::Jellyfin` or `Plex`, migrations 014 and 015): its tracks
//! are rows of `tracks` like any others, their `relative_path` the server's
//! item id, and `server_sources` says where the server is and who signed
//! in. A Plex server's music libraries are a source each (`section`). The
//! access tokens are in the keychain (`metadata::keys`, `Account::Server`).
//!
//! - `address`: which addresses the app may send a token to.
//! - `api`: one interface to every kind of server, for what doesn't care
//!   which it is.
//! - `jellyfin`: the Jellyfin server's API.
//! - `plex`, `plextv`: the Plex Media Server's API, and plex.tv's, which
//!   signs in and lists the servers and their addresses.
//! - `discovery`: finding servers on the local network.
//! - `sync`: bringing the source's tracks and playlists up to date, in
//!   place of a scan.
//! - `cache`: the audio cache. The core never goes online (FFmpeg is built
//!   without networking): a track is downloaded into the cache and the
//!   engine opens the file there (`fetch`).
//! - `playing`: telling the server what plays.
//! - `commands`, `plex_commands`: signing in and out, syncing, and keeping
//!   music offline.
//!
//! Each kind of server is behind a feature switch of its own (`jellyfin`,
//! `plex`; `kind_on`), off by default since it goes online. Kept apart from
//! each server's API where it can be, so a kind of server is a client
//! behind the same source, sync and cache.

pub mod address;
pub mod api;
pub mod cache;
pub mod commands;
pub mod discovery;
pub mod fetch;
pub mod jellyfin;
pub mod offline;
pub mod playing;
pub mod plex;
pub mod plex_commands;
pub mod plextv;
pub mod sync;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::library::{unix_now, Error, FolderKind};
use crate::metadata::keys::{self, Account};
use crate::settings::FeatureSettings;

/// Whether the feature switch of servers of `kind` is on.
pub fn kind_on(features: &FeatureSettings, kind: FolderKind) -> bool {
    match kind {
        FolderKind::Local => true,
        FolderKind::Jellyfin => features.jellyfin,
        FolderKind::Plex => features.plex,
    }
}

/// Whether any kind of server is turned on.
pub fn any_on(features: &FeatureSettings) -> bool {
    features.jellyfin || features.plex
}

/// The settings row holding this install's device id.
const DEVICE_ID_KEY: &str = "servers.deviceId";

/// This install as servers see it: a random id made once (so a server can
/// tell its sessions apart and the user can revoke this one), and a plain
/// name for the device.
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
}

impl Device {
    /// This install's device, making its id on first use.
    pub fn of(conn: &Connection) -> Result<Device, Error> {
        Ok(Device {
            id: device_id(conn)?,
            name: device_name().into(),
        })
    }
}

/// The device's kind, as a server lists it: no host or user names.
fn device_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "Mac"
    } else if cfg!(target_os = "ios") {
        "iPhone or iPad"
    } else if cfg!(target_os = "windows") {
        "Windows PC"
    } else {
        "Linux PC"
    }
}

/// This install's device id: 32 hex digits, made at random the first time
/// and kept in the `settings` table (not `AppSettings`: it isn't the user's
/// to change).
fn device_id(conn: &Connection) -> Result<String, Error> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [DEVICE_ID_KEY],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = stored.and_then(|json| serde_json::from_str::<String>(&json).ok()) {
        if id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Ok(id);
        }
    }
    let mut bytes = [0u8; 16];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes)
        .map_err(|_| Error::Invalid("Cannot make a device id".into()))?;
    let id: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![
            DEVICE_ID_KEY,
            serde_json::to_string(&id).expect("a string serializes")
        ],
    )?;
    Ok(id)
}

/// Who a server says it is.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
}

/// A user signed in to a server.
#[derive(Debug, Clone, PartialEq)]
pub struct SignedIn {
    pub token: String,
    pub user_id: String,
    pub user_name: String,
    /// The server's id, as signing in gives it.
    pub server_id: Option<String>,
}

/// A track as a server describes it, in the terms the library keeps.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ServerTrack {
    /// The server's id for it, which becomes its `relative_path`.
    pub id: String,
    pub title: Option<String>,
    pub artists: Vec<String>,
    pub album_artists: Vec<String>,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub year: Option<u32>,
    /// "2024-03-01", as precise as the server has it.
    pub date: Option<String>,
    /// Seconds.
    pub duration: f64,
    pub genres: Vec<String>,
    /// The file's container, lower case: "flac", "mp3", "m4a"…
    pub container: Option<String>,
    /// The file's size in bytes, 0 when not known.
    pub size: i64,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate: u32,
    pub channels: u32,
    pub recording_mbid: Option<String>,
    pub release_mbid: Option<String>,
    pub release_group_mbid: Option<String>,
    pub artist_mbid: Option<String>,
    pub album_artist_mbid: Option<String>,
    /// Its album's picture (else its own), to download into the image
    /// cache: a URL, or a key the server's client turns into one
    /// (`ServerApi::art_url`).
    pub art_url: Option<String>,
    /// The path on the server that serves the file, where the id doesn't
    /// give it (Plex's part key).
    pub media_path: Option<String>,
    /// dB to ReplayGain 2.0's reference loudness, as the server measured.
    pub track_gain: Option<f64>,
}

/// A playlist on a server: its tracks by the server's ids, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerPlaylist {
    pub id: String,
    pub name: String,
    pub item_ids: Vec<String>,
}

/// A server source: where the server is and who signed in.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub folder_id: i64,
    pub kind: FolderKind,
    pub address: String,
    pub server_id: String,
    pub server_name: String,
    pub version: Option<String>,
    pub user_id: String,
    pub user_name: String,
    /// The server's music library this source is, by its key on the
    /// server; empty for a source that is all the user's music (Jellyfin).
    pub section: String,
    /// The music library's name (Plex).
    pub section_name: Option<String>,
    /// When the last sync that finished started (Unix seconds).
    pub sync_cursor: Option<i64>,
    /// When the last sync finished (Unix seconds), as `folders.last_scan_at`.
    pub synced_at: Option<i64>,
    pub track_count: u32,
    /// Whether `address` is Plex's relay, which is slow and capped.
    pub via_relay: bool,
    /// Where else the server may answer, as plex.tv last listed it.
    #[serde(skip)]
    #[cfg_attr(test, ts(skip))]
    pub connections: Vec<plextv::Connection>,
}

impl Source {
    /// The source's key (`folder_key`).
    pub fn key(&self) -> String {
        folder_key(self.kind, &self.server_id, &self.user_id, &self.section)
    }

    /// What its access token is kept under: the server and user, shared by
    /// the sources of one server's music libraries.
    pub fn token_key(&self) -> String {
        folder_key(self.kind, &self.server_id, &self.user_id, "")
    }

    /// Its access token, from the keychain.
    pub fn token(&self) -> Result<Option<String>, Error> {
        Ok(keys::get(Account::Server(self.token_key()))?)
    }
}

/// Whether a folder's `path` is a server source's key, not a place on disk.
pub fn is_source_key(path: &str) -> bool {
    path.starts_with("jellyfin:") || path.starts_with("plex:")
}

/// What a server source's folder `path` holds: a key naming no file, one
/// per server, user and music library (`jellyfin:<server id>/<user id>`,
/// `plex:<server id>/<user id>/<section>`).
pub fn folder_key(kind: FolderKind, server_id: &str, user_id: &str, section: &str) -> String {
    if section.is_empty() {
        format!("{}:{server_id}/{user_id}", kind.as_str())
    } else {
        format!("{}:{server_id}/{user_id}/{section}", kind.as_str())
    }
}

/// What a Plex account's own token is kept under.
pub fn plex_account_key(account_id: &str) -> String {
    format!("plex-account:{account_id}")
}

/// A music library on a server: its key there and its name.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub key: String,
    pub name: String,
}

const SOURCE_COLUMNS: &str = "s.folder_id, f.kind, s.address, s.server_id, s.server_name,
     s.version, s.user_id, s.user_name, s.sync_cursor, f.last_scan_at,
     (SELECT count(*) FROM tracks t WHERE t.folder_id = f.id),
     s.section, s.section_name, s.connections";

fn source_from_row(row: &rusqlite::Row) -> rusqlite::Result<Option<Source>> {
    let Some(kind) = FolderKind::from_str(&row.get::<_, String>(1)?) else {
        return Ok(None);
    };
    let address: String = row.get(2)?;
    let connections: Vec<plextv::Connection> = row
        .get::<_, Option<String>>(13)?
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    let via_relay = connections
        .iter()
        .any(|connection| connection.relay && connection.uri == address);
    Ok(Some(Source {
        folder_id: row.get(0)?,
        kind,
        address,
        server_id: row.get(3)?,
        server_name: row.get(4)?,
        version: row.get(5)?,
        user_id: row.get(6)?,
        user_name: row.get(7)?,
        section: row.get(11)?,
        section_name: row.get(12)?,
        sync_cursor: row.get(8)?,
        synced_at: row.get(9)?,
        track_count: row.get(10)?,
        via_relay,
        connections,
    }))
}

/// Every server source, by name.
pub fn sources(conn: &Connection) -> Result<Vec<Source>, Error> {
    let mut statement = conn.prepare(&format!(
        "SELECT {SOURCE_COLUMNS} FROM server_sources s JOIN folders f ON f.id = s.folder_id
         ORDER BY s.server_name, s.user_name, s.folder_id"
    ))?;
    let rows = statement.query_map([], source_from_row)?;
    let sources: Vec<Option<Source>> = rows.collect::<Result<_, _>>()?;
    Ok(sources.into_iter().flatten().collect())
}

/// Server source `folder_id`, if there is one.
pub fn source(conn: &Connection, folder_id: i64) -> Result<Option<Source>, Error> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {SOURCE_COLUMNS} FROM server_sources s JOIN folders f ON f.id = s.folder_id
                 WHERE s.folder_id = ?1"
            ),
            [folder_id],
            source_from_row,
        )
        .optional()?
        .flatten())
}

/// Adds the library of `signed`'s user on the server `info` at `address`
/// as a source (the music library `section` of it, if given), or brings an
/// existing one's address, names and version up to date (signing in again
/// keeps its tracks and their ids). Returns its folder id. The token is
/// stored by the caller (`store_token`).
pub fn add_source(
    conn: &mut Connection,
    kind: FolderKind,
    address: &str,
    info: &ServerInfo,
    signed: &SignedIn,
    section: Option<&Section>,
) -> Result<i64, Error> {
    let server_id = signed.server_id.as_deref().unwrap_or(&info.id);
    let section_key = section.map_or("", |section| section.key.as_str());
    let key = folder_key(kind, server_id, &signed.user_id, section_key);
    let tx = conn.transaction()?;
    let existing: Option<i64> = tx
        .query_row(
            "SELECT id FROM folders WHERE path = ?1 AND kind = ?2",
            params![key, kind.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    let folder_id = match existing {
        Some(id) => id,
        None => {
            tx.execute(
                "INSERT INTO folders (path, added_at, kind) VALUES (?1, ?2, ?3)",
                params![key, unix_now(), kind.as_str()],
            )?;
            tx.last_insert_rowid()
        }
    };
    tx.execute(
        "INSERT INTO server_sources (folder_id, address, server_id, server_name, version,
                                     user_id, user_name, added_at, section, section_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT (folder_id) DO UPDATE SET
             address = excluded.address, server_name = excluded.server_name,
             version = excluded.version, user_name = excluded.user_name,
             section_name = excluded.section_name",
        params![
            folder_id,
            address,
            server_id,
            info.name,
            info.version,
            signed.user_id,
            signed.user_name,
            unix_now(),
            section_key,
            section.map(|section| section.name.as_str()),
        ],
    )?;
    tx.commit()?;
    Ok(folder_id)
}

/// Stores the access token kept under `key` (`Source::token_key`, or
/// `plex_account_key`) in the keychain, or deletes it.
pub fn store_token(key: &str, token: Option<&str>) -> Result<(), Error> {
    Ok(keys::set(Account::Server(key.to_owned()), token)?)
}

/// Records the address the server of source `folder_id` answered at last.
pub fn set_address(conn: &Connection, folder_id: i64, address: &str) -> Result<(), Error> {
    conn.execute(
        "UPDATE server_sources SET address = ?1 WHERE folder_id = ?2",
        params![address, folder_id],
    )?;
    Ok(())
}

/// Records where else the server of source `folder_id` may answer.
pub fn set_connections(
    conn: &Connection,
    folder_id: i64,
    connections: &[plextv::Connection],
) -> Result<(), Error> {
    conn.execute(
        "UPDATE server_sources SET connections = ?1 WHERE folder_id = ?2",
        params![
            serde_json::to_string(connections).expect("connections serialize"),
            folder_id
        ],
    )?;
    Ok(())
}

/// Records the server's name and version as it last gave them.
pub fn update_info(conn: &Connection, folder_id: i64, info: &ServerInfo) -> Result<(), Error> {
    conn.execute(
        "UPDATE server_sources SET server_name = ?1, version = ?2 WHERE folder_id = ?3",
        params![info.name, info.version, folder_id],
    )?;
    Ok(())
}

/// The kinds of server the sources are on and their versions, for "Copy
/// diagnostics": never addresses or names.
pub fn versions(conn: &Connection) -> Result<Vec<String>, Error> {
    Ok(sources(conn)?
        .into_iter()
        .map(|source| {
            format!(
                "{} {}",
                source.kind.as_str(),
                source.version.as_deref().unwrap_or("?")
            )
        })
        .collect())
}

#[cfg(test)]
pub mod testing {
    //! A library with a Jellyfin source, or a Plex one, for tests.

    use rusqlite::Connection;

    use super::jellyfin::testing::{ADDRESS, USER};
    use super::{add_source, ServerInfo, SignedIn};
    use crate::library::FolderKind;

    pub const SERVER_ID: &str = "4c5e9a1b2d3f4a5b6c7d8e9f0a1b2c3d";

    /// Adds the music library `section` of the test Plex server as a
    /// source, with its connections; returns its folder id.
    pub fn add_plex_source(conn: &mut Connection, section: &str, name: &str) -> i64 {
        use crate::servers::plex::testing as plex;
        let id = add_source(
            conn,
            FolderKind::Plex,
            plex::ADDRESS,
            &ServerInfo {
                id: plex::MACHINE.into(),
                name: "Den".into(),
                version: Some("1.41.9.9961".into()),
            },
            &SignedIn {
                token: plex::SERVER_TOKEN.into(),
                user_id: plex::ACCOUNT.into(),
                user_name: "mark".into(),
                server_id: Some(plex::MACHINE.into()),
            },
            Some(&super::Section {
                key: section.into(),
                name: name.into(),
            }),
        )
        .unwrap();
        super::set_connections(conn, id, &plex::connections()).unwrap();
        id
    }

    /// Adds the test server as a source; returns its folder id.
    pub fn add_test_source(conn: &mut Connection) -> i64 {
        add_source(
            conn,
            FolderKind::Jellyfin,
            ADDRESS,
            &ServerInfo {
                id: SERVER_ID.into(),
                name: "Living Room".into(),
                version: Some("10.10.7".into()),
            },
            &SignedIn {
                token: "6a1f0c9e8d7b4a3f2e1d0c9b8a7f6e5d".into(),
                user_id: USER.into(),
                user_name: "mark".into(),
                server_id: Some(SERVER_ID.into()),
            },
            None,
        )
        .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::library::{db, folders, local_folders};

    #[test]
    fn a_device_id_is_made_once() {
        let conn = db::open_in_memory().unwrap();
        let device = Device::of(&conn).unwrap();
        assert_eq!(device.id.len(), 32);
        assert_eq!(Device::of(&conn).unwrap(), device);
        assert_eq!(device.name, device_name());
        let other = db::open_in_memory().unwrap();
        assert_ne!(Device::of(&other).unwrap().id, device.id);
    }

    #[test]
    fn a_source_is_a_folder_of_its_own_kind() {
        let mut conn = db::open_in_memory().unwrap();
        let id = add_test_source(&mut conn);
        let listed = folders(&conn).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].kind, FolderKind::Jellyfin);
        assert_eq!(listed[0].name.as_deref(), Some("Living Room"));
        assert_eq!(
            listed[0].path,
            "jellyfin:4c5e9a1b2d3f4a5b6c7d8e9f0a1b2c3d/5f1b2c3d4e5f60718293a4b5c6d7e8f9"
        );
        assert!(local_folders(&conn).unwrap().is_empty());

        let source = source(&conn, id).unwrap().unwrap();
        assert_eq!(source.server_name, "Living Room");
        assert_eq!(source.user_name, "mark");
        assert_eq!(source.key(), listed[0].path);
        assert_eq!(sources(&conn).unwrap(), std::slice::from_ref(&source));
        assert_eq!(versions(&conn).unwrap(), ["jellyfin 10.10.7"]);

        // Signing in again keeps the folder.
        assert_eq!(add_test_source(&mut conn), id);
        assert_eq!(sources(&conn).unwrap().len(), 1);

        // Nothing that walks folders on disk touches it.
        assert!(crate::library::access::open_folder(&conn, id).is_err());
        assert!(crate::library::access::open_folder_of(
            &conn,
            std::path::Path::new(&source.key()).join("x").as_path()
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn a_plex_servers_music_libraries_are_sources_sharing_its_token() {
        let mut conn = db::open_in_memory().unwrap();
        let music = add_plex_source(&mut conn, "1", "Music");
        let live = add_plex_source(&mut conn, "4", "Live Recordings");
        assert_ne!(music, live);
        let music = source(&conn, music).unwrap().unwrap();
        let live = source(&conn, live).unwrap().unwrap();
        assert_eq!(music.kind, FolderKind::Plex);
        assert_eq!(
            music.key(),
            "plex:9d3f1c7a5b2e4f6081a3c5e7f9b1d3e5a7c9e1f3/31415926/1"
        );
        assert_eq!(music.token_key(), live.token_key());
        assert_eq!(
            music.token_key(),
            "plex:9d3f1c7a5b2e4f6081a3c5e7f9b1d3e5a7c9e1f3/31415926"
        );
        assert_eq!(music.section_name.as_deref(), Some("Music"));
        assert_eq!(music.connections.len(), 3);
        assert!(!music.via_relay);
        // The folder is named by server and library.
        let names: Vec<Option<String>> = folders(&conn)
            .unwrap()
            .into_iter()
            .map(|folder| folder.name)
            .collect();
        assert!(names.contains(&Some("Den · Music".into())));
        // At the relay, it says so.
        set_address(&conn, music.folder_id, crate::servers::plex::testing::RELAY).unwrap();
        assert!(source(&conn, music.folder_id).unwrap().unwrap().via_relay);
        // A Jellyfin source's token stays where 7e kept it.
        let jellyfin = add_test_source(&mut conn);
        let jellyfin = source(&conn, jellyfin).unwrap().unwrap();
        assert_eq!(jellyfin.token_key(), jellyfin.key());
        assert_eq!(versions(&conn).unwrap().len(), 3);
        assert_eq!(plex_account_key("31415926"), "plex-account:31415926");
    }

    #[test]
    fn each_kind_of_server_has_its_own_switch() {
        let mut features = FeatureSettings::default();
        assert!(!any_on(&features));
        features.plex = true;
        assert!(any_on(&features));
        assert!(kind_on(&features, FolderKind::Plex));
        assert!(!kind_on(&features, FolderKind::Jellyfin));
        assert!(kind_on(&features, FolderKind::Local));
    }

    #[test]
    fn tokens_go_in_the_keychain() {
        let key = "jellyfin:test-server/test-user";
        store_token(key, Some("abc123token")).unwrap();
        assert_eq!(
            keys::get(Account::Server(key.into())).unwrap().as_deref(),
            Some("abc123token")
        );
        store_token(key, None).unwrap();
        assert_eq!(keys::get(Account::Server(key.into())).unwrap(), None);
    }
}
