//! "Keep available offline" (PLAN.md Phase 7e, 7e.6): albums and playlists
//! whose server tracks stay in the audio cache, so they play without the
//! server. Their tracks are downloaded ahead (`missing`, fetched by the
//! commands' keeper) and never evicted (`keep`); turning it off lets them be
//! evicted as usual. Local tracks in a pinned album or playlist need
//! nothing.

use std::collections::HashSet;

use rusqlite::{params, Connection};
use serde::Serialize;

use super::cache::CachedName;
use super::fetch::{remote_file, RemoteFile, ServerAudio};
use crate::library::{playlists, unix_now, Error};

/// What is kept offline, and how much of it is downloaded.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct OfflineStatus {
    pub albums: Vec<i64>,
    pub playlists: Vec<i64>,
    /// Server tracks kept offline, and how many of them are downloaded.
    pub tracks: u32,
    pub downloaded: u32,
    /// The audio cache's files and bytes, and its budget in bytes.
    pub cache_files: u32,
    pub cache_bytes: u64,
    pub cache_budget: u64,
}

/// Keeps album `album_id` offline, or stops.
pub fn set_album(conn: &Connection, album_id: i64, on: bool) -> Result<(), Error> {
    if on {
        conn.execute(
            "INSERT INTO offline_albums (album_id, pinned_at) VALUES (?1, ?2)
             ON CONFLICT (album_id) DO NOTHING",
            params![album_id, unix_now()],
        )?;
    } else {
        conn.execute("DELETE FROM offline_albums WHERE album_id = ?1", [album_id])?;
    }
    Ok(())
}

/// Keeps playlist `playlist_id` offline, or stops.
pub fn set_playlist(conn: &Connection, playlist_id: i64, on: bool) -> Result<(), Error> {
    if on {
        conn.execute(
            "INSERT INTO offline_playlists (playlist_id, pinned_at) VALUES (?1, ?2)
             ON CONFLICT (playlist_id) DO NOTHING",
            params![playlist_id, unix_now()],
        )?;
    } else {
        conn.execute(
            "DELETE FROM offline_playlists WHERE playlist_id = ?1",
            [playlist_id],
        )?;
    }
    Ok(())
}

fn ids(conn: &Connection, sql: &str) -> Result<Vec<i64>, Error> {
    let mut statement = conn.prepare(sql)?;
    let ids = statement
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// The server tracks kept offline, each once, as fetching needs them.
pub fn pinned(conn: &Connection) -> Result<Vec<(i64, RemoteFile)>, Error> {
    let mut track_ids = ids(
        conn,
        "SELECT t.id FROM tracks t JOIN offline_albums o ON o.album_id = t.album_id
         ORDER BY t.album_id, t.disc_number, t.track_number, t.id",
    )?;
    for playlist in ids(
        conn,
        "SELECT playlist_id FROM offline_playlists ORDER BY pinned_at",
    )? {
        track_ids.extend(playlists::track_ids(conn, playlist, &[])?);
    }
    let mut seen = HashSet::new();
    let mut files = Vec::new();
    for id in track_ids {
        if seen.insert(id) {
            if let Some(file) = remote_file(conn, id)? {
                files.push((id, file));
            }
        }
    }
    Ok(files)
}

/// The names of every file kept offline, which eviction leaves.
pub fn keep(conn: &Connection, audio: &ServerAudio) -> Result<HashSet<String>, Error> {
    Ok(pinned(conn)?
        .iter()
        .flat_map(|(_, file)| audio.names(file))
        .collect())
}

/// The tracks kept offline that aren't downloaded, with what to fetch.
pub fn missing(
    conn: &Connection,
    audio: &ServerAudio,
) -> Result<Vec<(i64, RemoteFile, CachedName)>, Error> {
    Ok(pinned(conn)?
        .into_iter()
        .filter_map(|(id, file)| {
            let choice = audio.choose(&file);
            (!audio.cache.contains(&choice.name)).then_some((id, file, choice))
        })
        .collect())
}

pub fn status(conn: &Connection, audio: &ServerAudio) -> Result<OfflineStatus, Error> {
    let pinned = pinned(conn)?;
    let downloaded = pinned
        .iter()
        .filter(|(_, file)| audio.cache.contains(&audio.choose(file).name))
        .count();
    let (files, bytes) = audio.cache.usage();
    Ok(OfflineStatus {
        albums: ids(
            conn,
            "SELECT album_id FROM offline_albums ORDER BY album_id",
        )?,
        playlists: ids(
            conn,
            "SELECT playlist_id FROM offline_playlists ORDER BY playlist_id",
        )?,
        tracks: pinned.len() as u32,
        downloaded: downloaded as u32,
        cache_files: files as u32,
        cache_bytes: bytes,
        cache_budget: audio.cache.budget(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db;
    use crate::servers::cache::AudioCache;
    use crate::servers::sync::testing::{track, FakeLibrary};
    use crate::servers::sync::{sync, SyncOptions};
    use crate::servers::testing::add_test_source;

    #[test]
    fn pinned_albums_and_playlists_stay_downloaded() {
        let mut conn = db::open_in_memory().unwrap();
        let folder = add_test_source(&mut conn);
        let server = FakeLibrary::default();
        *server.tracks.borrow_mut() = vec![
            track("a", "One", "Chirps"),
            track("b", "Two", "Chirps"),
            track("c", "Three", "Other"),
        ];
        sync(&mut conn, &server, folder, SyncOptions::default(), |_| {}).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let audio = ServerAudio::new(AudioCache::new(dir.path().to_path_buf(), 1 << 30));
        assert!(missing(&conn, &audio).unwrap().is_empty());

        let album: i64 = conn
            .query_row("SELECT id FROM albums WHERE title = 'Chirps'", [], |row| {
                row.get(0)
            })
            .unwrap();
        set_album(&conn, album, true).unwrap();
        set_album(&conn, album, true).unwrap();
        let three: i64 = conn
            .query_row(
                "SELECT id FROM tracks WHERE relative_path = 'c'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let list = playlists::create(&mut conn, "Mix", None, &[three]).unwrap();
        set_playlist(&conn, list.id, true).unwrap();

        let wanted = missing(&conn, &audio).unwrap();
        assert_eq!(
            wanted
                .iter()
                .map(|(_, file, _)| file.item_id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        let keep = keep(&conn, &audio).unwrap();
        assert!(keep.contains(&wanted[0].2.name));
        audio
            .cache
            .store::<String>(&wanted[0].2.name, &keep, |_| Ok(()))
            .unwrap();
        let now = status(&conn, &audio).unwrap();
        assert_eq!((now.tracks, now.downloaded), (3, 1));
        assert_eq!((now.albums, now.playlists), (vec![album], vec![list.id]));

        set_album(&conn, album, false).unwrap();
        set_playlist(&conn, list.id, false).unwrap();
        assert!(missing(&conn, &audio).unwrap().is_empty());
        assert!(super::keep(&conn, &audio).unwrap().is_empty());
    }

    /// What `servers_list` answers for a synced Jellyfin source and a Plex
    /// one, the sync's report (as `servers_sync` answers),
    /// `servers_offline_status` before and after an album is kept offline,
    /// and what a command answers while Jellyfin is turned off, for the
    /// frontend's tests.
    #[test]
    fn ui_fixtures() {
        use crate::library::access::FolderStatus;
        use crate::library::FolderKind;
        use crate::servers::commands::{kind_off, ServerSummary};
        use crate::servers::testing::add_plex_source;
        use crate::ui_fixtures::{record, sample};
        let mut conn = db::open_in_memory().unwrap();
        let folder = add_test_source(&mut conn);
        add_plex_source(&mut conn, "1", "Music");
        let server = FakeLibrary::default();
        *server.tracks.borrow_mut() =
            vec![track("a", "One", "Chirps"), track("b", "Two", "Chirps")];
        let synced = sync(&mut conn, &server, folder, SyncOptions::default(), |_| {}).unwrap();
        let list: Vec<ServerSummary> = crate::servers::sources(&conn)
            .unwrap()
            .into_iter()
            .map(|mut source| {
                // The sync's times, fixed so that the file doesn't change.
                source.sync_cursor = source.sync_cursor.map(|_| 1_760_000_000);
                source.synced_at = source.synced_at.map(|_| 1_760_000_060);
                ServerSummary {
                    source,
                    status: FolderStatus::AVAILABLE,
                    syncing: false,
                }
            })
            .collect();
        assert_eq!(list.len(), 2);

        let dir = tempfile::tempdir().unwrap();
        let audio = ServerAudio::new(AudioCache::new(dir.path().to_path_buf(), 1 << 30));
        let none = status(&conn, &audio).unwrap();
        let album: i64 = conn
            .query_row("SELECT id FROM albums WHERE title = 'Chirps'", [], |row| {
                row.get(0)
            })
            .unwrap();
        set_album(&conn, album, true).unwrap();
        let kept = status(&conn, &audio).unwrap();
        record(
            "servers",
            &[
                ("list", sample(&list)),
                ("synced", sample(&synced.report)),
                ("offline", sample(&none)),
                ("albumKept", sample(&kept)),
                ("jellyfinOff", sample(kind_off(FolderKind::Jellyfin))),
            ],
        );
    }
}
