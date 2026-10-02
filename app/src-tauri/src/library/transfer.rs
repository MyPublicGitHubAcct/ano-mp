//! Exporting and importing what the user made (PLAN.md F20): one JSON file
//! with the settings, sort rules and online source settings, the user's
//! picks (album and artist matches they chose, covers), playlists,
//! favourites, ratings, the listening history, playback preferences, where
//! long tracks were left, and the queue.
//!
//! It names tracks by their library folder and their path in it (and part
//! start), with the recording MBID as a fallback; albums by release MBID or
//! album artist and title; artists by name. So it imports into a fresh
//! library once its folders have been scanned, even at other paths: a
//! track is found by its path in any library folder when its folder's path
//! differs. It covers a lost or damaged database and moving to another
//! computer, and later carries data to iOS (Phase 8).
//!
//! Importing adds to what is there and never removes anything: a track
//! hearted stays hearted, a play already recorded isn't recorded twice, and
//! a playlist whose name and tracks are already there isn't made again.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{unix_now, Error};
use crate::coded::coded;

pub const FORMAT: &str = "ano-mp.user-data";
pub const VERSION: u32 = 1;

/// The settings rows the file carries, by key.
pub const SETTINGS_KEYS: [&str; 3] = ["app", "library.sort", "metadata.services"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserData {
    pub format: String,
    pub version: u32,
    pub exported_at: i64,
    pub app_version: String,
    /// Library folder paths, which `TrackRef::folder` indexes.
    pub folders: Vec<String>,
    /// Settings rows (`SETTINGS_KEYS`) as their JSON values.
    pub settings: HashMap<String, Value>,
    pub tracks: Vec<TrackRef>,
    pub albums: Vec<AlbumRef>,
    pub artists: Vec<ArtistRef>,
    pub favourites: Favourites,
    /// (track, stars): the user's own ratings, or cleared ones (`None`).
    pub ratings: Vec<(usize, Option<u8>)>,
    /// (track, when it started, seconds listened).
    pub plays: Vec<(usize, i64, f64)>,
    /// (track, seconds in, when saved).
    pub positions: Vec<(usize, f64, i64)>,
    pub track_prefs: Vec<(usize, Value)>,
    pub album_prefs: Vec<(usize, Value)>,
    /// (album or artist, a link row as JSON): only the ones the user chose.
    pub album_links: Vec<(usize, Value)>,
    pub album_art: Vec<(usize, Value)>,
    pub artist_links: Vec<(usize, Value)>,
    pub playlists: Vec<PlaylistData>,
    pub queue: Option<QueueData>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackRef {
    /// Index into `UserData::folders`.
    pub folder: usize,
    /// '/'-separated, relative to the folder.
    pub path: String,
    pub start: f64,
    pub recording: Option<String>,
    /// For the user reading the file, and for a report of what's missing.
    pub title: Option<String>,
    pub artist: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AlbumRef {
    pub title: String,
    pub artist: Option<String>,
    pub release: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArtistRef {
    pub name: String,
}

/// (item, when it was hearted), by kind.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Favourites {
    pub tracks: Vec<(usize, i64)>,
    pub albums: Vec<(usize, i64)>,
    pub artists: Vec<(usize, i64)>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaylistData {
    pub name: String,
    pub rules: Option<Value>,
    pub items: Vec<usize>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct QueueData {
    pub tracks: Vec<usize>,
    pub current: Option<usize>,
    pub position: f64,
    pub repeat: Value,
}

/// Interns tracks, albums and artists into the file's lists by id.
#[derive(Default)]
struct Refs {
    tracks: HashMap<i64, usize>,
    albums: HashMap<i64, usize>,
    artists: HashMap<i64, usize>,
}

/// Everything to export, with `queue` (track ids, the current index and the
/// position in it, and the repeat mode) if the queue has anything.
pub fn export(
    conn: &Connection,
    queue: Option<(Vec<i64>, Option<usize>, f64, Value)>,
) -> Result<UserData, Error> {
    let mut data = UserData {
        format: FORMAT.into(),
        version: VERSION,
        exported_at: unix_now(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        ..UserData::default()
    };
    let folder_ids: Vec<(i64, String)> = conn
        .prepare("SELECT id, path FROM folders ORDER BY id")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let folder_index: HashMap<i64, usize> = folder_ids
        .iter()
        .enumerate()
        .map(|(index, (id, _))| (*id, index))
        .collect();
    data.folders = folder_ids.into_iter().map(|(_, path)| path).collect();

    for key in SETTINGS_KEYS {
        let json: Option<String> = conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?;
        if let Some(value) = json.and_then(|json| serde_json::from_str(&json).ok()) {
            data.settings.insert(key.into(), value);
        }
    }

    let mut refs = Refs::default();
    let mut track = |data: &mut UserData, id: i64| -> Result<usize, Error> {
        if let Some(&index) = refs.tracks.get(&id) {
            return Ok(index);
        }
        let (folder_id, path, start, recording, title, artist): (
            i64,
            String,
            f64,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = conn.query_row(
            "SELECT t.folder_id, t.relative_path, t.range_start, t.musicbrainz_recording_id,
                    t.title, IFNULL(t.artist_credit, a.name)
             FROM tracks t LEFT JOIN artists a ON a.id = t.artist_id WHERE t.id = ?1",
            [id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )?;
        data.tracks.push(TrackRef {
            folder: folder_index.get(&folder_id).copied().unwrap_or(0),
            path,
            start,
            recording,
            title,
            artist,
        });
        refs.tracks.insert(id, data.tracks.len() - 1);
        Ok(data.tracks.len() - 1)
    };

    let pairs = |sql: &str| -> Result<Vec<(i64, i64)>, Error> {
        Ok(conn
            .prepare(sql)?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?)
    };
    for (id, added) in pairs("SELECT track_id, added_at FROM track_favourites ORDER BY added_at")? {
        let index = track(&mut data, id)?;
        data.favourites.tracks.push((index, added));
    }
    let ratings: Vec<(i64, Option<u8>)> = conn
        .prepare(
            "SELECT track_id, rating FROM track_ratings WHERE source = 'user' ORDER BY track_id",
        )?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (id, rating) in ratings {
        let index = track(&mut data, id)?;
        data.ratings.push((index, rating));
    }
    let plays: Vec<(i64, i64, f64)> = conn
        .prepare("SELECT track_id, played_at, seconds FROM plays ORDER BY played_at, id")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;
    for (id, at, seconds) in plays {
        let index = track(&mut data, id)?;
        data.plays.push((index, at, seconds));
    }
    let positions: Vec<(i64, f64, i64)> = conn
        .prepare("SELECT track_id, position, saved_at FROM track_positions ORDER BY track_id")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;
    for (id, position, saved) in positions {
        let index = track(&mut data, id)?;
        data.positions.push((index, position, saved));
    }
    let track_prefs: Vec<(i64, String)> = conn
        .prepare(
            "SELECT track_id, json_object('skip', skip, 'gainOffset', gain_offset,
                                          'trimStart', trim_start, 'trimEnd', trim_end)
             FROM track_prefs ORDER BY track_id",
        )?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (id, json) in track_prefs {
        let index = track(&mut data, id)?;
        data.track_prefs
            .push((index, serde_json::from_str(&json).unwrap_or_default()));
    }

    let playlists: Vec<(i64, String, Option<String>, i64, i64)> = conn
        .prepare("SELECT id, name, rules, created_at, updated_at FROM playlists ORDER BY id")?
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    for (id, name, rules, created_at, updated_at) in playlists {
        let item_ids: Vec<i64> = conn
            .prepare(
                "SELECT track_id FROM playlist_items WHERE playlist_id = ?1 ORDER BY position, id",
            )?
            .query_map([id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let mut items = Vec::with_capacity(item_ids.len());
        for item in item_ids {
            items.push(track(&mut data, item)?);
        }
        data.playlists.push(PlaylistData {
            name,
            rules: rules.and_then(|json| serde_json::from_str(&json).ok()),
            items,
            created_at,
            updated_at,
        });
    }

    if let Some((ids, current, position, repeat)) = queue.filter(|(ids, ..)| !ids.is_empty()) {
        let mut tracks = Vec::with_capacity(ids.len());
        for id in ids {
            // A track that left the library meanwhile is left out.
            if let Ok(index) = track(&mut data, id) {
                tracks.push(index);
            }
        }
        data.queue = Some(QueueData {
            tracks,
            current,
            position,
            repeat,
        });
    }

    // Albums and artists.
    let mut album = |data: &mut UserData, id: i64| -> Result<usize, Error> {
        if let Some(&index) = refs.albums.get(&id) {
            return Ok(index);
        }
        let (title, artist, release): (String, Option<String>, Option<String>) = conn.query_row(
            "SELECT al.title, ar.name, al.musicbrainz_release_id
             FROM albums al LEFT JOIN artists ar ON ar.id = al.artist_id WHERE al.id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        data.albums.push(AlbumRef {
            title,
            artist,
            release,
        });
        refs.albums.insert(id, data.albums.len() - 1);
        Ok(data.albums.len() - 1)
    };
    for (id, added) in pairs("SELECT album_id, added_at FROM album_favourites ORDER BY added_at")? {
        let index = album(&mut data, id)?;
        data.favourites.albums.push((index, added));
    }
    let album_rows = |sql: &str| -> Result<Vec<(i64, String)>, Error> {
        Ok(conn
            .prepare(sql)?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?)
    };
    for (id, json) in album_rows(
        "SELECT album_id, json_object('skip', skip, 'neverShuffle', never_shuffle,
                                      'gainOffset', gain_offset)
         FROM album_prefs ORDER BY album_id",
    )? {
        let index = album(&mut data, id)?;
        data.album_prefs
            .push((index, serde_json::from_str(&json).unwrap_or_default()));
    }
    for (id, json) in album_rows(
        "SELECT album_id, json_object('source', source, 'status', status,
                                      'externalId', external_id, 'score', score,
                                      'details', details, 'checkedAt', checked_at)
         FROM album_links WHERE chosen_by = 'user' ORDER BY album_id, source",
    )? {
        let index = album(&mut data, id)?;
        data.album_links
            .push((index, serde_json::from_str(&json).unwrap_or_default()));
    }
    for (id, json) in album_rows(
        "SELECT album_id, json_object('source', source, 'reference', reference)
         FROM album_art ORDER BY album_id",
    )? {
        let index = album(&mut data, id)?;
        data.album_art
            .push((index, serde_json::from_str(&json).unwrap_or_default()));
    }

    let mut artist = |data: &mut UserData, id: i64| -> Result<usize, Error> {
        if let Some(&index) = refs.artists.get(&id) {
            return Ok(index);
        }
        let name: String =
            conn.query_row("SELECT name FROM artists WHERE id = ?1", [id], |row| {
                row.get(0)
            })?;
        data.artists.push(ArtistRef { name });
        refs.artists.insert(id, data.artists.len() - 1);
        Ok(data.artists.len() - 1)
    };
    for (id, added) in pairs("SELECT artist_id, added_at FROM artist_favourites ORDER BY added_at")?
    {
        let index = artist(&mut data, id)?;
        data.favourites.artists.push((index, added));
    }
    for (id, json) in album_rows(
        "SELECT artist_id, json_object('source', source, 'status', status,
                                       'externalId', external_id, 'score', score,
                                       'details', details, 'checkedAt', checked_at)
         FROM artist_links WHERE chosen_by = 'user' ORDER BY artist_id, source",
    )? {
        let index = artist(&mut data, id)?;
        data.artist_links
            .push((index, serde_json::from_str(&json).unwrap_or_default()));
    }
    Ok(data)
}

/// What an import found and did.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    /// Tracks, albums and artists the file names that the library has, of
    /// how many.
    pub tracks_found: usize,
    pub tracks: usize,
    pub albums_found: usize,
    pub albums: usize,
    pub artists_found: usize,
    pub artists: usize,
    /// What was added.
    pub favourites: usize,
    pub ratings: usize,
    pub plays: usize,
    pub positions: usize,
    pub preferences: usize,
    pub picks: usize,
    pub playlists: usize,
    /// Some of the tracks the file names, that the library hasn't got.
    pub missing: Vec<String>,
}

/// Library ids for the file's tracks, albums and artists (`None` where the
/// library hasn't got one).
pub struct Resolved {
    pub tracks: Vec<Option<i64>>,
    pub albums: Vec<Option<i64>>,
    pub artists: Vec<Option<i64>>,
}

/// The error for a file that isn't one of these.
pub fn not_data_file() -> String {
    coded("notDataFile", &[], "That isn't an ano-mp data file")
}

/// Checks the file is one of these, of a version this app reads.
pub fn check(data: &UserData) -> Result<(), Error> {
    if data.format != FORMAT {
        return Err(Error::Invalid(not_data_file()));
    }
    if data.version > VERSION {
        return Err(Error::Invalid(coded(
            "newerDataFile",
            &[],
            "That data file is from a newer version of ano-mp",
        )));
    }
    Ok(())
}

/// Finds the file's tracks, albums and artists in the library.
pub fn resolve(conn: &Connection, data: &UserData) -> Result<Resolved, Error> {
    let folders: Vec<(i64, String)> = conn
        .prepare("SELECT id, path FROM folders")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let name_of = |path: &str| {
        path.rsplit(['/', '\\'])
            .find(|part| !part.is_empty())
            .map(str::to_owned)
    };
    // The library folder each exported folder is: the same path, else the
    // same name.
    let folder_map: Vec<Option<i64>> = data
        .folders
        .iter()
        .map(|exported| {
            folders
                .iter()
                .find(|(_, path)| path == exported)
                .or_else(|| {
                    folders.iter().find(|(_, path)| {
                        name_of(path).is_some() && name_of(path) == name_of(exported)
                    })
                })
                .map(|(id, _)| *id)
        })
        .collect();

    let mut tracks = Vec::with_capacity(data.tracks.len());
    for track in &data.tracks {
        let exact = match folder_map.get(track.folder).copied().flatten() {
            Some(folder_id) => conn
                .prepare_cached(
                    "SELECT id FROM tracks WHERE folder_id = ?1 AND relative_path = ?2
                     AND abs(range_start - ?3) < 0.001",
                )?
                .query_row(params![folder_id, track.path, track.start], |row| {
                    row.get(0)
                })
                .optional()?,
            None => None,
        };
        let found = match exact {
            Some(id) => Some(id),
            None => {
                let by_path: Vec<i64> = conn
                    .prepare_cached(
                        "SELECT id FROM tracks WHERE relative_path = ?1
                         AND abs(range_start - ?2) < 0.001 LIMIT 2",
                    )?
                    .query_map(params![track.path, track.start], |row| row.get(0))?
                    .collect::<Result<_, _>>()?;
                match (by_path.as_slice(), &track.recording) {
                    ([id], _) => Some(*id),
                    (_, Some(recording)) => {
                        let by_mbid: Vec<i64> = conn
                            .prepare_cached(
                                "SELECT id FROM tracks WHERE musicbrainz_recording_id = ?1 LIMIT 2",
                            )?
                            .query_map([recording], |row| row.get(0))?
                            .collect::<Result<_, _>>()?;
                        (by_mbid.len() == 1).then(|| by_mbid[0])
                    }
                    _ => None,
                }
            }
        };
        tracks.push(found);
    }

    let mut albums = Vec::with_capacity(data.albums.len());
    for album in &data.albums {
        let by_release = match &album.release {
            Some(release) => conn
                .prepare_cached("SELECT id FROM albums WHERE musicbrainz_release_id = ?1 LIMIT 1")?
                .query_row([release], |row| row.get(0))
                .optional()?,
            None => None,
        };
        let found = match by_release {
            Some(id) => Some(id),
            None => conn
                .prepare_cached(
                    "SELECT al.id FROM albums al LEFT JOIN artists ar ON ar.id = al.artist_id
                     WHERE al.title = ?1 AND ar.name IS ?2 LIMIT 1",
                )?
                .query_row(params![album.title, album.artist], |row| row.get(0))
                .optional()?,
        };
        albums.push(found);
    }

    let mut artists = Vec::with_capacity(data.artists.len());
    for artist in &data.artists {
        artists.push(
            conn.prepare_cached("SELECT id FROM artists WHERE name = ?1")?
                .query_row([&artist.name], |row| row.get(0))
                .optional()?,
        );
    }
    Ok(Resolved {
        tracks,
        albums,
        artists,
    })
}

/// Adds the file's data (all but the settings and the queue, which the
/// caller applies) to the library.
pub fn import(
    conn: &mut Connection,
    data: &UserData,
    found: &Resolved,
) -> Result<ImportReport, Error> {
    let mut report = ImportReport {
        tracks: data.tracks.len(),
        tracks_found: found.tracks.iter().flatten().count(),
        albums: data.albums.len(),
        albums_found: found.albums.iter().flatten().count(),
        artists: data.artists.len(),
        artists_found: found.artists.iter().flatten().count(),
        missing: data
            .tracks
            .iter()
            .zip(&found.tracks)
            .filter(|(_, id)| id.is_none())
            .take(20)
            .map(|(track, _)| track.path.clone())
            .collect(),
        ..ImportReport::default()
    };
    let track = |index: usize| found.tracks.get(index).copied().flatten();
    let album = |index: usize| found.albums.get(index).copied().flatten();
    let artist = |index: usize| found.artists.get(index).copied().flatten();

    let tx = conn.transaction()?;
    for &(index, added) in &data.favourites.tracks {
        if let Some(id) = track(index) {
            report.favourites += tx.execute(
                "INSERT OR IGNORE INTO track_favourites (track_id, added_at) VALUES (?1, ?2)",
                params![id, added],
            )?;
        }
    }
    for &(index, added) in &data.favourites.albums {
        if let Some(id) = album(index) {
            report.favourites += tx.execute(
                "INSERT OR IGNORE INTO album_favourites (album_id, added_at) VALUES (?1, ?2)",
                params![id, added],
            )?;
        }
    }
    for &(index, added) in &data.favourites.artists {
        if let Some(id) = artist(index) {
            report.favourites += tx.execute(
                "INSERT OR IGNORE INTO artist_favourites (artist_id, added_at) VALUES (?1, ?2)",
                params![id, added],
            )?;
        }
    }
    for &(index, rating) in &data.ratings {
        if let (Some(id), true) = (
            track(index),
            rating.is_none_or(|stars| (1..=5).contains(&stars)),
        ) {
            report.ratings += tx.execute(
                "INSERT INTO track_ratings (track_id, rating, source) VALUES (?1, ?2, 'user')
                 ON CONFLICT (track_id) DO UPDATE SET rating = excluded.rating, source = 'user'
                 WHERE track_ratings.source = 'tags'",
                params![id, rating],
            )?;
        }
    }
    for &(index, at, seconds) in &data.plays {
        if let Some(id) = track(index) {
            report.plays += tx.execute(
                "INSERT INTO plays (track_id, played_at, seconds)
                 SELECT ?1, ?2, ?3 WHERE NOT EXISTS
                     (SELECT 1 FROM plays WHERE track_id = ?1 AND played_at = ?2)",
                params![id, at, seconds],
            )?;
        }
    }
    for &(index, position, saved) in &data.positions {
        if let (Some(id), true) = (track(index), position.is_finite() && position >= 0.0) {
            report.positions += tx.execute(
                "INSERT INTO track_positions (track_id, position, saved_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (track_id) DO UPDATE SET position = excluded.position,
                     saved_at = excluded.saved_at WHERE excluded.saved_at > track_positions.saved_at",
                params![id, position, saved],
            )?;
        }
    }
    for (index, prefs) in &data.track_prefs {
        if let Some(id) = track(*index) {
            report.preferences += tx.execute(
                "INSERT OR IGNORE INTO track_prefs (track_id, skip, gain_offset, trim_start, trim_end)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id,
                    prefs["skip"].as_i64(),
                    finite(&prefs["gainOffset"]),
                    finite(&prefs["trimStart"]),
                    finite(&prefs["trimEnd"])
                ],
            )?;
        }
    }
    for (index, prefs) in &data.album_prefs {
        if let Some(id) = album(*index) {
            report.preferences += tx.execute(
                "INSERT OR IGNORE INTO album_prefs (album_id, skip, never_shuffle, gain_offset)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    id,
                    prefs["skip"].as_i64(),
                    prefs["neverShuffle"].as_i64(),
                    finite(&prefs["gainOffset"])
                ],
            )?;
        }
    }
    for (index, link) in &data.album_links {
        if let (Some(id), Some(source), Some(status)) = (
            album(*index),
            link["source"].as_str(),
            link["status"].as_str(),
        ) {
            report.picks += tx.execute(
                "INSERT INTO album_links (album_id, source, status, external_id, score, chosen_by,
                                          details, checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'user', ?6, ?7)
                 ON CONFLICT (album_id, source) DO UPDATE SET
                     status = excluded.status, external_id = excluded.external_id,
                     score = excluded.score, chosen_by = 'user', details = excluded.details,
                     checked_at = excluded.checked_at
                 WHERE album_links.chosen_by = 'auto'",
                params![
                    id,
                    source,
                    status,
                    link["externalId"].as_str(),
                    link["score"].as_f64().unwrap_or(1.0),
                    link["details"].as_str(),
                    link["checkedAt"].as_i64().unwrap_or(0)
                ],
            )?;
        }
    }
    for (index, art) in &data.album_art {
        if let (Some(id), Some(source)) = (album(*index), art["source"].as_str()) {
            report.picks += tx.execute(
                "INSERT OR IGNORE INTO album_art (album_id, source, reference) VALUES (?1, ?2, ?3)",
                params![id, source, art["reference"].as_str()],
            )?;
        }
    }
    for (index, link) in &data.artist_links {
        if let (Some(id), Some(source), Some(status)) = (
            artist(*index),
            link["source"].as_str(),
            link["status"].as_str(),
        ) {
            report.picks += tx.execute(
                "INSERT INTO artist_links (artist_id, source, status, external_id, score,
                                           chosen_by, details, checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'user', ?6, ?7)
                 ON CONFLICT (artist_id, source) DO UPDATE SET
                     status = excluded.status, external_id = excluded.external_id,
                     score = excluded.score, chosen_by = 'user', details = excluded.details,
                     checked_at = excluded.checked_at
                 WHERE artist_links.chosen_by = 'auto'",
                params![
                    id,
                    source,
                    status,
                    link["externalId"].as_str(),
                    link["score"].as_f64().unwrap_or(1.0),
                    link["details"].as_str(),
                    link["checkedAt"].as_i64().unwrap_or(0)
                ],
            )?;
        }
    }

    for playlist in &data.playlists {
        let name = playlist.name.trim();
        if name.is_empty() {
            continue;
        }
        let items: Vec<i64> = playlist
            .items
            .iter()
            .filter_map(|&index| track(index))
            .collect();
        let rules = playlist.rules.as_ref().map(Value::to_string);
        // The same name and the same tracks (or rules): already there.
        let same: Vec<i64> = tx
            .prepare_cached("SELECT id FROM playlists WHERE name = ?1 AND rules IS ?2")?
            .query_map(params![name, rules], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let mut duplicate = false;
        for id in same {
            let existing: Vec<i64> = tx
                .prepare_cached(
                    "SELECT track_id FROM playlist_items WHERE playlist_id = ?1 ORDER BY position, id",
                )?
                .query_map([id], |row| row.get(0))?
                .collect::<Result<_, _>>()?;
            duplicate |= existing == items;
        }
        if duplicate {
            continue;
        }
        tx.execute(
            "INSERT INTO playlists (name, rules, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![name, rules, playlist.created_at, playlist.updated_at],
        )?;
        let id = tx.last_insert_rowid();
        for (position, track_id) in items.iter().enumerate() {
            tx.execute(
                "INSERT INTO playlist_items (playlist_id, position, track_id) VALUES (?1, ?2, ?3)",
                params![id, position as i64, track_id],
            )?;
        }
        report.playlists += 1;
    }
    tx.commit()?;
    Ok(report)
}

fn finite(value: &Value) -> Option<f64> {
    value.as_f64().filter(|number| number.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::marks::{self, MarkKind};
    use crate::library::playlists;
    use crate::library::test_library::{track, Library};

    fn sample() -> Library {
        Library::new([
            track("A/X/1.flac").title("One").artist("A").album("X"),
            track("A/X/2.flac").title("Two").artist("A").album("X"),
            track("B/3.flac").title("Three").artist("B"),
        ])
    }

    fn ids(conn: &Connection, sql: &str) -> Vec<i64> {
        conn.prepare(sql)
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn round_trips_into_a_fresh_library_at_another_path() {
        let mut from = sample();
        let conn = &from.conn;
        let t = ids(conn, "SELECT id FROM tracks ORDER BY relative_path");
        let album = ids(conn, "SELECT id FROM albums")[0];
        let artist = ids(conn, "SELECT id FROM artists WHERE name = 'B'")[0];
        marks::set_favourite(conn, MarkKind::Track, &[t[2]], true).unwrap();
        marks::set_favourite(conn, MarkKind::Album, &[album], true).unwrap();
        marks::set_favourite(conn, MarkKind::Artist, &[artist], true).unwrap();
        marks::set_rating(conn, &[t[0]], Some(5)).unwrap();
        conn.execute_batch(&format!(
            "INSERT INTO plays (track_id, played_at, seconds) VALUES ({0}, 10, 60), ({0}, 20, 60);
             INSERT INTO track_positions (track_id, position, saved_at) VALUES ({1}, 1234.5, 3);
             INSERT INTO track_prefs (track_id, skip, gain_offset) VALUES ({1}, 1, -2.5);
             INSERT INTO album_prefs (album_id, never_shuffle) VALUES ({2}, 1);
             INSERT INTO album_art (album_id, source, reference) VALUES ({2}, 'folder', 'a.jpg');
             INSERT INTO album_links (album_id, source, status, external_id, score, chosen_by,
                                      checked_at)
             VALUES ({2}, 'musicbrainz', 'matched', 'rel', 1, 'user', 5),
                    ({2}, 'discogs', 'matched', 'auto-one', 0.9, 'auto', 5);
             INSERT INTO settings (key, value) VALUES ('library.sort', '{{\"ignoredArticles\":[]}}');",
            t[0], t[1], album
        ))
        .unwrap();
        playlists::create(&mut from.conn, "Mix", None, &[t[2], t[0]]).unwrap();
        let data = export(
            &from.conn,
            Some((vec![t[1], t[2]], Some(1), 42.0, "all".into())),
        )
        .unwrap();
        assert_eq!(data.album_links.len(), 1, "only the user's picks");
        let json = serde_json::to_string(&data).unwrap();

        // Another computer: the folder is elsewhere, with the same name.
        let conn = crate::library::db::open_in_memory().unwrap();
        let mut to = Library::from_conn(conn);
        to.conn
            .execute("UPDATE folders SET path = '/Volumes/Disk/Music'", [])
            .unwrap();
        for relative in ["B/3.flac", "A/X/2.flac", "A/X/1.flac"] {
            to.add(
                to.folder_id,
                match relative {
                    "B/3.flac" => track(relative).title("Three").artist("B"),
                    "A/X/2.flac" => track(relative).title("Two").artist("A").album("X"),
                    _ => track(relative).title("One").artist("A").album("X"),
                },
            );
        }
        let data: UserData = serde_json::from_str(&json).unwrap();
        check(&data).unwrap();
        let found = resolve(&to.conn, &data).unwrap();
        assert!(found.tracks.iter().all(Option::is_some));
        let report = import(&mut to.conn, &data, &found).unwrap();
        assert_eq!(
            (
                report.favourites,
                report.ratings,
                report.plays,
                report.positions
            ),
            (3, 1, 2, 1)
        );
        assert_eq!(
            (report.preferences, report.picks, report.playlists),
            (2, 2, 1)
        );
        assert!(report.missing.is_empty());
        let three = ids(&to.conn, "SELECT id FROM tracks WHERE title = 'Three'")[0];
        let listed =
            playlists::track_ids(&to.conn, playlists::playlists(&to.conn).unwrap()[0].id, &[])
                .unwrap();
        assert_eq!(listed[0], three);

        // Importing again adds nothing twice.
        let again = import(&mut to.conn, &data, &found).unwrap();
        assert_eq!(
            (
                again.favourites,
                again.plays,
                again.playlists,
                again.preferences
            ),
            (0, 0, 0, 0)
        );
        let plays: i64 = to
            .conn
            .query_row("SELECT count(*) FROM plays", [], |row| row.get(0))
            .unwrap();
        assert_eq!(plays, 2);
        assert_eq!(data.queue.as_ref().unwrap().tracks.len(), 2);
        assert!(data.settings.contains_key("library.sort"));
    }

    #[test]
    fn refuses_other_files_and_reports_whats_missing() {
        let mut data = UserData::default();
        assert!(check(&data).is_err());
        data.format = FORMAT.into();
        data.version = VERSION + 1;
        assert!(check(&data).unwrap_err().to_string().contains("newer"));

        let library = sample();
        data.version = VERSION;
        data.folders = vec!["/Elsewhere".into()];
        data.tracks = vec![TrackRef {
            path: "Gone/9.flac".into(),
            ..TrackRef::default()
        }];
        data.favourites.tracks = vec![(0, 1)];
        let found = resolve(&library.conn, &data).unwrap();
        let mut conn = library.conn;
        let report = import(&mut conn, &data, &found).unwrap();
        assert_eq!(
            (report.tracks, report.tracks_found, report.favourites),
            (1, 0, 0)
        );
        assert_eq!(report.missing, ["Gone/9.flac"]);
    }
}
