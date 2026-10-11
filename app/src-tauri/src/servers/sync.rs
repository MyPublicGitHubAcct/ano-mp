//! Bringing a server source's tracks and playlists up to date: the
//! server's version of a scan (PLAN.md Phase 7e).
//!
//! A sync pages through the server's music and upserts each track through
//! the scanner's own write (`scanner::write_unfiled_track`) with the
//! server's metadata as its tags, so artists, albums, genres, credits,
//! search and MusicBrainz links work as for local files, and a track keeps
//! its id (and with it its plays, ratings, favourites and playlist entries)
//! from sync to sync. The first sync, and one the user asks for in full,
//! reads every track; later ones ask only for tracks changed since the last
//! (less `CURSOR_MARGIN`, for the two clocks), then list every track's id
//! to find those removed from the server.
//!
//! The scanner's rule holds (PLAN.md H22): a sync that would remove all of
//! a source's tracks, or most of a large one, keeps them and fails the
//! source as `FolderState::MostlyGone` (or `Empty`), since a server with a
//! library offline or a user whose access changed can answer with nothing.
//!
//! The server's audio playlists come in read-only (`server_playlists`),
//! refilled each time. Album pictures are listed for the caller to
//! download into the image cache (`missing_art`).

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use super::{ServerPlaylist, ServerTrack};
use crate::anomp::{ReplayGain, Tags};
use crate::library::access::FolderState;
use crate::library::scanner::{self, ScanProgress, ScanReport, VARIOUS_ARTISTS};
use crate::library::{remove_orphans, unix_now, Error};
use crate::metadata::images::ImageCache;

/// A sync of changes asks for tracks changed since the last sync started,
/// less this many seconds, in case the server's clock is behind.
pub const CURSOR_MARGIN: i64 = 6 * 3600;

/// A server's music, a page at a time: Jellyfin's (`jellyfin::Library`),
/// a Plex music library's (`plex::Library`), or a test's.
pub trait ServerLibrary {
    /// A page of tracks from `start`, changed since `changed_since` (Unix
    /// seconds) if given, and how many there are in all.
    fn tracks(
        &self,
        start: usize,
        changed_since: Option<i64>,
    ) -> Result<(Vec<ServerTrack>, usize), crate::metadata::Error>;
    /// A page of every track's id from `start`, and how many there are.
    fn track_ids(&self, start: usize) -> Result<(Vec<String>, usize), crate::metadata::Error>;
    fn playlists(&self) -> Result<Vec<ServerPlaylist>, crate::metadata::Error>;
}

/// How to sync.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SyncOptions {
    /// Read every track, not only those changed since the last sync.
    pub full: bool,
    /// Remove tracks gone from the server even when `scanner::holds` would
    /// keep them: the user said to.
    pub remove_missing: bool,
}

/// What a sync did: the scan's report, and the albums whose picture
/// changed, whose thumbnails the caller forgets.
#[derive(Debug, Default, PartialEq)]
pub struct Synced {
    pub report: ScanReport,
    pub art_changed: Vec<i64>,
}

/// A failure of the server, which leaves the library as it was, or of the
/// database.
#[derive(Debug)]
pub enum SyncError {
    Server(crate::metadata::Error),
    Library(Error),
}

impl From<Error> for SyncError {
    fn from(error: Error) -> Self {
        SyncError::Library(error)
    }
}

impl From<rusqlite::Error> for SyncError {
    fn from(error: rusqlite::Error) -> Self {
        SyncError::Library(Error::Db(error))
    }
}

impl From<crate::metadata::Error> for SyncError {
    fn from(error: crate::metadata::Error) -> Self {
        SyncError::Server(error)
    }
}

/// Syncs server source `folder_id` from `server`, reporting progress as a
/// scan does. A server that fails part way leaves what was written so far
/// (each page is a transaction) and removes nothing. Tracks gone from the
/// server that `scanner::holds` keeps make the report's `unavailable` say
/// so.
pub fn sync(
    conn: &mut Connection,
    server: &(impl ServerLibrary + ?Sized),
    folder_id: i64,
    options: SyncOptions,
    mut progress: impl FnMut(ScanProgress),
) -> Result<Synced, SyncError> {
    let started = unix_now();
    let cursor: Option<i64> = conn
        .query_row(
            "SELECT sync_cursor FROM server_sources WHERE folder_id = ?1",
            [folder_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let changed_since = cursor
        .filter(|_| !options.full)
        .map(|cursor| cursor - CURSOR_MARGIN);
    let known = known_tracks(conn, folder_id)?;
    let mut report = ScanReport {
        folder_id,
        ..ScanReport::default()
    };
    let mut art_changed = HashSet::new();

    // The tracks, new and changed.
    let mut seen: HashSet<String> = HashSet::new();
    let mut start = 0;
    loop {
        let (tracks, total) = server.tracks(start, changed_since)?;
        if start == 0 {
            progress(ScanProgress {
                folder_id,
                read: 0,
                to_read: total,
            });
        }
        if tracks.is_empty() {
            break;
        }
        start += tracks.len();
        let tx = conn.transaction()?;
        let now = unix_now();
        for track in &tracks {
            if let Some(album) = write(&tx, folder_id, track, now)? {
                art_changed.insert(album);
            }
            // A track a shifting page gives twice counts once.
            if !seen.insert(track.id.clone()) {
                continue;
            }
            if known.contains_key(&track.id) {
                report.updated += 1;
            } else {
                report.added += 1;
            }
        }
        tx.commit()?;
        progress(ScanProgress {
            folder_id,
            read: start.min(total),
            to_read: total,
        });
        if start >= total {
            break;
        }
    }

    // Every track's id, to find those removed: the full read has them.
    let present = if changed_since.is_none() {
        seen
    } else {
        let mut ids = seen;
        let mut start = 0;
        loop {
            let (page, total) = server.track_ids(start)?;
            if page.is_empty() {
                break;
            }
            start += page.len();
            ids.extend(page);
            if start >= total {
                break;
            }
        }
        ids
    };
    let gone: Vec<i64> = known
        .iter()
        .filter(|(item, _)| !present.contains(*item))
        .map(|(_, id)| *id)
        .collect();

    // Playlists: a server that can't list them leaves them as they were.
    let playlists = match server.playlists() {
        Ok(playlists) => Some(playlists),
        Err(error) => {
            log::warn!("cannot read server {folder_id}'s playlists: {error}");
            None
        }
    };

    let tx = conn.transaction()?;
    let held = !options.remove_missing && scanner::holds(known.len(), gone.len());
    if held {
        log::warn!(
            "kept the {} of {} tracks a sync of server {folder_id} didn't find",
            gone.len(),
            known.len()
        );
    } else {
        for id in &gone {
            tx.prepare_cached("DELETE FROM tracks WHERE id = ?1")?
                .execute([id])?;
        }
        report.removed = gone.len();
    }
    report.unchanged = known.len().saturating_sub(report.updated + report.removed);
    if let Some(playlists) = &playlists {
        write_playlists(&tx, folder_id, playlists)?;
    }
    remove_orphans(&tx)?;
    scanner::restore_kept(&tx)?;
    tx.execute(
        "UPDATE server_sources SET sync_cursor = ?1 WHERE folder_id = ?2",
        params![started, folder_id],
    )?;
    tx.execute(
        "UPDATE folders SET last_scan_at = ?1 WHERE id = ?2",
        params![unix_now(), folder_id],
    )?;
    tx.commit()?;
    if held {
        let state = if present.is_empty() {
            FolderState::Empty
        } else {
            FolderState::MostlyGone
        };
        report.unavailable = Some(crate::library::access::FolderStatus {
            state,
            detail: Some(format!("{} tracks not found", gone.len())),
        });
    }
    log::info!(
        "synced server {folder_id}: {} added, {} updated, {} removed",
        report.added,
        report.updated,
        report.removed
    );
    let mut art_changed: Vec<i64> = art_changed.into_iter().collect();
    art_changed.sort_unstable();
    Ok(Synced {
        report,
        art_changed,
    })
}

/// The source's tracks by item id.
fn known_tracks(conn: &Connection, folder_id: i64) -> Result<HashMap<String, i64>, Error> {
    let mut statement =
        conn.prepare("SELECT relative_path, id FROM tracks WHERE folder_id = ?1")?;
    let rows = statement.query_map([folder_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The server's track as the tags a file would have.
fn tags(track: &ServerTrack) -> Tags {
    let album_artist = track.album_artists.first().cloned();
    Tags {
        title: track.title.clone(),
        artist: (!track.artists.is_empty()).then(|| track.artists.join("; ")),
        artists: if track.artists.len() > 1 {
            track.artists.clone()
        } else {
            Vec::new()
        },
        album: track.album.clone(),
        compilation: album_artist
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(VARIOUS_ARTISTS)),
        album_artist,
        genre: (!track.genres.is_empty()).then(|| track.genres.join("; ")),
        track_number: track.track_number,
        disc_number: track.disc_number,
        year: track.year,
        date: track.date.clone(),
        duration: track.duration,
        sample_rate: track.sample_rate,
        channels: track.channels,
        bitrate_kbps: track.bitrate_kbps,
        replay_gain: ReplayGain {
            track_gain: track.track_gain,
            ..ReplayGain::default()
        },
        musicbrainz_recording_id: track.recording_mbid.clone(),
        musicbrainz_release_id: track.release_mbid.clone(),
        musicbrainz_release_group_id: track.release_group_mbid.clone(),
        musicbrainz_artist_id: track.artist_mbid.clone(),
        musicbrainz_album_artist_id: track.album_artist_mbid.clone(),
        ..Tags::default()
    }
}

/// Upserts one track and what the server says beyond the tags; returns its
/// album if the album's picture changed.
fn write(
    tx: &Transaction,
    folder_id: i64,
    track: &ServerTrack,
    now: i64,
) -> Result<Option<i64>, Error> {
    let id = scanner::write_unfiled_track(tx, folder_id, &track.id, track.size, &tags(track), now)?;
    let before: Option<Option<String>> = tx
        .prepare_cached("SELECT art_url FROM server_tracks WHERE track_id = ?1")?
        .query_row([id], |row| row.get(0))
        .optional()?;
    tx.prepare_cached(
        "INSERT INTO server_tracks (track_id, container, art_url, media_path)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (track_id) DO UPDATE SET
             container = excluded.container, art_url = excluded.art_url,
             media_path = excluded.media_path",
    )?
    .execute(params![
        id,
        track.container,
        track.art_url,
        track.media_path
    ])?;
    let changed = before.is_some_and(|before| before != track.art_url);
    if !changed {
        return Ok(None);
    }
    Ok(tx
        .prepare_cached("SELECT album_id FROM tracks WHERE id = ?1")?
        .query_row([id], |row| row.get(0))?)
}

/// Refills the source's read-only playlists from the server's, adding new
/// ones and removing those gone (which `server_playlists_gone` deletes).
fn write_playlists(
    tx: &Transaction,
    folder_id: i64,
    playlists: &[ServerPlaylist],
) -> Result<(), Error> {
    let tracks = known_tracks(tx, folder_id)?;
    let now = unix_now();
    let mut kept = Vec::new();
    for playlist in playlists {
        let existing: Option<i64> = tx
            .query_row(
                "SELECT playlist_id FROM server_playlists WHERE folder_id = ?1 AND item_id = ?2",
                params![folder_id, playlist.id],
                |row| row.get(0),
            )
            .optional()?;
        let name = if playlist.name.trim().is_empty() {
            "Playlist"
        } else {
            playlist.name.trim()
        };
        let playlist_id = match existing {
            Some(id) => {
                tx.execute(
                    "UPDATE playlists SET name = ?1, updated_at = ?2 WHERE id = ?3",
                    params![name, now, id],
                )?;
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
                    params![name, now],
                )?;
                let id = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO server_playlists (playlist_id, folder_id, item_id)
                     VALUES (?1, ?2, ?3)",
                    params![id, folder_id, playlist.id],
                )?;
                id
            }
        };
        tx.execute(
            "DELETE FROM playlist_items WHERE playlist_id = ?1",
            [playlist_id],
        )?;
        let mut position = 0i64;
        for item in &playlist.item_ids {
            if let Some(track_id) = tracks.get(item) {
                tx.prepare_cached(
                    "INSERT INTO playlist_items (playlist_id, position, track_id) VALUES (?1, ?2, ?3)",
                )?
                .execute(params![playlist_id, position, track_id])?;
                position += 1;
            }
        }
        kept.push(playlist.id.clone());
    }
    tx.execute(
        "DELETE FROM server_playlists WHERE folder_id = ?1
           AND item_id NOT IN (SELECT value FROM json_each(?2))",
        params![
            folder_id,
            serde_json::to_string(&kept).unwrap_or_else(|_| "[]".into())
        ],
    )?;
    Ok(())
}

/// The source's album pictures not in the image cache yet, each once.
pub fn missing_art(
    conn: &Connection,
    folder_id: i64,
    images: &ImageCache,
) -> Result<Vec<String>, Error> {
    let mut statement = conn.prepare(
        "SELECT DISTINCT st.art_url FROM server_tracks st JOIN tracks t ON t.id = st.track_id
         WHERE t.folder_id = ?1 AND st.art_url IS NOT NULL ORDER BY st.art_url",
    )?;
    let urls: Vec<String> = statement
        .query_map([folder_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(urls
        .into_iter()
        .filter(|url| !images.contains(url))
        .collect())
}

#[cfg(test)]
pub mod testing {
    use std::cell::RefCell;

    use super::*;

    /// A server whose library is whatever the test puts in it.
    #[derive(Default)]
    pub struct FakeLibrary {
        pub tracks: RefCell<Vec<ServerTrack>>,
        /// Tracks a sync of changes is given, if not all.
        pub changed: RefCell<Option<Vec<ServerTrack>>>,
        pub playlists: RefCell<Vec<ServerPlaylist>>,
        /// Page size, to test paging.
        pub page: usize,
        /// Fails every call, as an unreachable server does.
        pub offline: bool,
        pub asked_since: RefCell<Vec<Option<i64>>>,
    }

    impl FakeLibrary {
        fn check(&self) -> Result<(), crate::metadata::Error> {
            if self.offline {
                Err(crate::metadata::Error::Offline("down".into()))
            } else {
                Ok(())
            }
        }

        fn page_size(&self) -> usize {
            if self.page == 0 {
                100
            } else {
                self.page
            }
        }
    }

    impl ServerLibrary for FakeLibrary {
        fn tracks(
            &self,
            start: usize,
            changed_since: Option<i64>,
        ) -> Result<(Vec<ServerTrack>, usize), crate::metadata::Error> {
            self.check()?;
            if start == 0 {
                self.asked_since.borrow_mut().push(changed_since);
            }
            let all = match (changed_since, self.changed.borrow().as_ref()) {
                (Some(_), Some(changed)) => changed.clone(),
                _ => self.tracks.borrow().clone(),
            };
            let page = all
                .iter()
                .skip(start)
                .take(self.page_size())
                .cloned()
                .collect();
            Ok((page, all.len()))
        }

        fn track_ids(&self, start: usize) -> Result<(Vec<String>, usize), crate::metadata::Error> {
            self.check()?;
            let all = self.tracks.borrow();
            let page = all
                .iter()
                .skip(start)
                .take(self.page_size())
                .map(|track| track.id.clone())
                .collect();
            Ok((page, all.len()))
        }

        fn playlists(&self) -> Result<Vec<ServerPlaylist>, crate::metadata::Error> {
            self.check()?;
            Ok(self.playlists.borrow().clone())
        }
    }

    pub fn track(id: &str, title: &str, album: &str) -> ServerTrack {
        ServerTrack {
            id: id.into(),
            title: Some(title.into()),
            artists: vec!["Test Signal".into()],
            album_artists: vec!["Test Signal".into()],
            album: Some(album.into()),
            duration: 1.5,
            container: Some("flac".into()),
            size: 1000,
            sample_rate: 44100,
            channels: 2,
            art_url: Some(format!(
                "http://192.168.1.20:8096/Items/{album}/Images/Primary?tag=1"
            )),
            ..ServerTrack::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::library::{db, search};
    use crate::servers::testing::add_test_source;

    fn run(
        conn: &mut Connection,
        server: &FakeLibrary,
        folder_id: i64,
        options: SyncOptions,
    ) -> Result<Synced, SyncError> {
        sync(conn, server, folder_id, options, |_| {})
    }

    fn ids(conn: &Connection) -> HashMap<String, i64> {
        known_tracks(conn, 1).unwrap()
    }

    fn titles(conn: &Connection) -> Vec<String> {
        conn.prepare("SELECT title FROM tracks ORDER BY title")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn the_first_sync_reads_everything_a_page_at_a_time() {
        let mut conn = db::open_in_memory().unwrap();
        let folder = add_test_source(&mut conn);
        let server = FakeLibrary {
            page: 2,
            ..FakeLibrary::default()
        };
        *server.tracks.borrow_mut() = vec![
            track("a", "One", "Chirps"),
            track("b", "Two", "Chirps"),
            ServerTrack {
                artists: vec!["Test Signal".into(), "Guest".into()],
                genres: vec!["Electronic".into(), "Test".into()],
                recording_mbid: Some("7c1b4b62-8a3c-4b8e-9d5e-0c3a6f2b1e4d".into()),
                track_gain: Some(-6.5),
                ..track("c", "Three", "Other")
            },
        ];
        let mut steps = Vec::new();
        let synced = sync(&mut conn, &server, folder, SyncOptions::default(), |p| {
            steps.push((p.read, p.to_read))
        })
        .unwrap();
        assert_eq!((synced.report.added, synced.report.removed), (3, 0));
        assert_eq!(steps, [(0, 3), (2, 3), (3, 3)]);
        assert_eq!(*server.asked_since.borrow(), [None]);
        assert_eq!(titles(&conn), ["One", "Three", "Two"]);

        // As a scanned file's would be: artists, albums, genres, search.
        let (artist_credit, genre, recording, gain): (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<f64>,
        ) = conn
            .query_row(
                "SELECT artist_credit, genre, musicbrainz_recording_id, replaygain_track_gain
                 FROM tracks WHERE relative_path = 'c'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(artist_credit.as_deref(), Some("Test Signal; Guest"));
        assert_eq!(genre.as_deref(), Some("Electronic; Test"));
        assert!(recording.is_some());
        assert_eq!(gain, Some(-6.5));
        let albums: i64 = conn
            .query_row("SELECT count(*) FROM albums", [], |row| row.get(0))
            .unwrap();
        assert_eq!(albums, 2);
        let found = search::search(&conn, "three", &[search::SearchKind::Tracks], 10, 0).unwrap();
        assert_eq!(found.track_total, 1);
        let container: String = conn
            .query_row("SELECT container FROM server_tracks LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(container, "flac");
        let cursor: Option<i64> = conn
            .query_row("SELECT sync_cursor FROM server_sources", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(cursor.is_some());
    }

    #[test]
    fn later_syncs_ask_for_changes_and_keep_track_ids() {
        let mut conn = db::open_in_memory().unwrap();
        let folder = add_test_source(&mut conn);
        let server = FakeLibrary::default();
        *server.tracks.borrow_mut() = (0..30)
            .map(|i| track(&format!("t{i}"), &format!("Track {i}"), "Chirps"))
            .collect();
        run(&mut conn, &server, folder, SyncOptions::default()).unwrap();
        let before = ids(&conn);
        conn.execute(
            "INSERT INTO track_favourites (track_id, added_at) VALUES (?1, 0)",
            [before["t1"]],
        )
        .unwrap();

        // One retitled, one removed, one added.
        {
            let mut tracks = server.tracks.borrow_mut();
            tracks[1].title = Some("Renamed".into());
            tracks.remove(2);
            tracks.push(track("new", "New", "Chirps"));
        }
        *server.changed.borrow_mut() = Some(vec![
            server.tracks.borrow()[1].clone(),
            track("new", "New", "Chirps"),
        ]);
        let synced = run(&mut conn, &server, folder, SyncOptions::default()).unwrap();
        let cursor: i64 = conn
            .query_row("SELECT sync_cursor FROM server_sources", [], |row| {
                row.get(0)
            })
            .unwrap();
        let asked = server.asked_since.borrow()[1].unwrap();
        assert!(asked <= cursor - CURSOR_MARGIN + 5 && asked >= cursor - CURSOR_MARGIN - 5);
        assert_eq!(
            (
                synced.report.added,
                synced.report.updated,
                synced.report.removed
            ),
            (1, 1, 1)
        );
        assert_eq!(synced.report.unchanged, 28);
        let after = ids(&conn);
        assert_eq!(after["t1"], before["t1"], "the track kept its id");
        assert!(!after.contains_key("t2"));
        let favourite: i64 = conn
            .query_row("SELECT track_id FROM track_favourites", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(favourite, before["t1"]);
        assert!(titles(&conn).contains(&"Renamed".to_owned()));

        // In full when asked.
        run(
            &mut conn,
            &server,
            folder,
            SyncOptions {
                full: true,
                ..SyncOptions::default()
            },
        )
        .unwrap();
        assert_eq!(server.asked_since.borrow()[2], None);
    }

    #[test]
    fn a_server_that_answers_with_nothing_keeps_its_tracks() {
        let mut conn = db::open_in_memory().unwrap();
        let folder = add_test_source(&mut conn);
        let server = FakeLibrary::default();
        *server.tracks.borrow_mut() = (0..25)
            .map(|i| track(&format!("t{i}"), &format!("Track {i}"), "Chirps"))
            .collect();
        run(&mut conn, &server, folder, SyncOptions::default()).unwrap();

        server.tracks.borrow_mut().clear();
        let synced = run(&mut conn, &server, folder, SyncOptions::default()).unwrap();
        assert_eq!(
            synced.report.unavailable.map(|status| status.state),
            Some(FolderState::Empty)
        );
        assert_eq!(ids(&conn).len(), 25);

        // Most gone: kept too, until the user says to remove them.
        *server.tracks.borrow_mut() = vec![track("t0", "Track 0", "Chirps")];
        let synced = run(&mut conn, &server, folder, SyncOptions::default()).unwrap();
        assert_eq!(
            synced.report.unavailable.map(|status| status.state),
            Some(FolderState::MostlyGone)
        );
        let synced = run(
            &mut conn,
            &server,
            folder,
            SyncOptions {
                remove_missing: true,
                ..SyncOptions::default()
            },
        )
        .unwrap();
        assert_eq!(synced.report.removed, 24);
        assert_eq!(ids(&conn).len(), 1);
    }

    #[test]
    fn an_unreachable_server_changes_nothing() {
        let mut conn = db::open_in_memory().unwrap();
        let folder = add_test_source(&mut conn);
        let server = FakeLibrary::default();
        *server.tracks.borrow_mut() = vec![track("a", "One", "Chirps")];
        run(&mut conn, &server, folder, SyncOptions::default()).unwrap();
        let offline = FakeLibrary {
            offline: true,
            ..FakeLibrary::default()
        };
        let error = run(&mut conn, &offline, folder, SyncOptions::default()).unwrap_err();
        assert!(matches!(
            error,
            SyncError::Server(crate::metadata::Error::Offline(_))
        ));
        assert_eq!(ids(&conn).len(), 1);
    }

    #[test]
    fn playlists_come_in_read_only() {
        let mut conn = db::open_in_memory().unwrap();
        let folder = add_test_source(&mut conn);
        let server = FakeLibrary::default();
        *server.tracks.borrow_mut() =
            vec![track("a", "One", "Chirps"), track("b", "Two", "Chirps")];
        *server.playlists.borrow_mut() = vec![ServerPlaylist {
            id: "p".into(),
            name: "Road Trip".into(),
            item_ids: vec!["b".into(), "missing".into(), "a".into()],
        }];
        run(&mut conn, &server, folder, SyncOptions::default()).unwrap();
        let playlist: i64 = conn
            .query_row(
                "SELECT id FROM playlists WHERE name = 'Road Trip'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let listed = crate::library::playlists::playlist(&conn, playlist)
            .unwrap()
            .unwrap();
        assert_eq!(listed.server.as_deref(), Some("Living Room"));
        let error = crate::library::playlists::rename(&conn, playlist, "Mine").unwrap_err();
        assert!(crate::coded::is(&error, "playlistReadOnly"), "{error}");
        let order: Vec<String> = conn
            .prepare(
                "SELECT t.relative_path FROM playlist_items i JOIN tracks t ON t.id = i.track_id
                 WHERE i.playlist_id = ?1 ORDER BY i.position",
            )
            .unwrap()
            .query_map([playlist], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(order, ["b", "a"]);

        // Renamed and refilled, then gone.
        server.playlists.borrow_mut()[0].name = "Long Drive".into();
        server.playlists.borrow_mut()[0].item_ids = vec!["a".into()];
        run(&mut conn, &server, folder, SyncOptions::default()).unwrap();
        let (name, count): (String, i64) = conn
            .query_row(
                "SELECT name, (SELECT count(*) FROM playlist_items WHERE playlist_id = id)
                 FROM playlists WHERE id = ?1",
                [playlist],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((name.as_str(), count), ("Long Drive", 1));
        server.playlists.borrow_mut().clear();
        run(&mut conn, &server, folder, SyncOptions::default()).unwrap();
        let left: i64 = conn
            .query_row("SELECT count(*) FROM playlists", [], |row| row.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn a_plex_music_library_syncs_from_its_server() {
        use crate::metadata::http::testing::fake_client;
        use crate::servers::api::ServerApi;
        use crate::servers::plex::testing as plex;
        let mut conn = db::open_in_memory().unwrap();
        let folder = crate::servers::testing::add_plex_source(&mut conn, "1", "Music");
        let source = crate::servers::source(&conn, folder).unwrap().unwrap();
        let (http, transport, _) = fake_client();
        plex::serve_library(&transport);
        let device = crate::servers::jellyfin::testing::device();
        let api = ServerApi::new(&http, &device, &source, Some(plex::SERVER_TOKEN));
        let synced = sync(
            &mut conn,
            api.library(&source).as_ref(),
            folder,
            SyncOptions::default(),
            |_| {},
        )
        .unwrap();
        assert_eq!(synced.report.added, 3);
        assert_eq!(titles(&conn), ["Chirp One", "Chirp Two", "DSD Chirp"]);
        let (path, media, art): (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT t.relative_path, st.media_path, st.art_url
                 FROM tracks t JOIN server_tracks st ON st.track_id = t.id
                 WHERE t.title = 'Chirp One'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(path, "1001");
        assert_eq!(
            media.as_deref(),
            Some("/library/parts/827/1712000000/file.flac")
        );
        assert!(art.unwrap().starts_with("plex://"));
        // The playlist, in the server's order.
        let order: Vec<String> = conn
            .prepare(
                "SELECT t.relative_path FROM playlist_items i JOIN tracks t ON t.id = i.track_id
                 ORDER BY i.position",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(order, ["1003", "1001"]);
        // What plays from it: a file the server gives by its part.
        let track_id: i64 = conn
            .query_row(
                "SELECT id FROM tracks WHERE relative_path = '1001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let file = crate::servers::fetch::remote_file(&conn, track_id)
            .unwrap()
            .unwrap();
        assert_eq!(file.kind, crate::library::FolderKind::Plex);
        assert!(file.media_path.is_some());
    }

    #[test]
    fn pictures_to_download_and_those_that_changed() {
        let mut conn = db::open_in_memory().unwrap();
        let folder = add_test_source(&mut conn);
        let dir = tempfile::tempdir().unwrap();
        let images = ImageCache::new(dir.path().to_path_buf());
        let server = FakeLibrary::default();
        *server.tracks.borrow_mut() = vec![
            track("a", "One", "Chirps"),
            track("b", "Two", "Chirps"),
            track("c", "Three", "Other"),
        ];
        let synced = run(&mut conn, &server, folder, SyncOptions::default()).unwrap();
        assert!(synced.art_changed.is_empty());
        let missing = missing_art(&conn, folder, &images).unwrap();
        assert_eq!(missing.len(), 2);
        images
            .store(&missing[0], crate::metadata::images::testing::JPEG)
            .unwrap();
        assert_eq!(missing_art(&conn, folder, &images).unwrap().len(), 1);

        server.tracks.borrow_mut()[0].art_url = Some("http://192.168.1.20:8096/new".into());
        let synced = run(
            &mut conn,
            &server,
            folder,
            SyncOptions {
                full: true,
                ..SyncOptions::default()
            },
        )
        .unwrap();
        assert_eq!(synced.art_changed.len(), 1);
    }
}
