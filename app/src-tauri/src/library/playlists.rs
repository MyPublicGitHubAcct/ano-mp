//! Playlists (PLAN.md F1): lists of library tracks in the user's order, and
//! smart playlists (F2, `smart`), in the library DB (migration 007). A
//! track keeps its place in every playlist when its file moves, since moves
//! keep track ids (F10); a track that leaves the library leaves its
//! playlists too.
//!
//! M3U and M3U8 playlists are imported by resolving each entry against the
//! library folders, and exported with paths relative to the playlist file
//! where they can be. Export writes only the file the user names.

use std::path::{Component, Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::smart::{self, SmartRules};
use super::{
    cue, track_from_row, track_path, unix_now, Error, TrackSummary, TRACKS_FROM, TRACK_COLUMNS,
};
use crate::coded::{coded, gone, Gone};

/// Longest playlist name, in characters.
const MAX_NAME: usize = 200;

/// Largest playlist file read.
const MAX_M3U: u64 = 16 << 20;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    /// A smart playlist's rules; `None` for a list of tracks.
    pub rules: Option<SmartRules>,
    pub track_count: u32,
    /// Seconds.
    pub duration: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A track in a playlist, and which entry it is (a track can be listed
/// twice).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntry {
    /// `None` in a smart playlist.
    pub item_id: Option<i64>,
    #[serde(flatten)]
    pub track: TrackSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPage {
    pub playlist: Playlist,
    pub entries: Vec<PlaylistEntry>,
}

fn clean_name(name: &str) -> Result<String, Error> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(Error::Invalid(coded(
            "playlistName",
            &[("max", MAX_NAME.into())],
            format!("A playlist's name must be 1 to {MAX_NAME} characters"),
        )));
    }
    Ok(name.to_owned())
}

fn rules_json(rules: Option<&SmartRules>) -> Result<Option<String>, Error> {
    rules
        .map(|rules| {
            rules.validate()?;
            Ok(serde_json::to_string(rules).expect("rules serialize"))
        })
        .transpose()
}

/// Every playlist, by name.
pub fn playlists(conn: &Connection) -> Result<Vec<Playlist>, Error> {
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM playlists ORDER BY name COLLATE NOCASE, id")?
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    ids.into_iter()
        .map(|id| playlist(conn, id))
        .collect::<Result<Option<Vec<_>>, _>>()
        .map(Option::unwrap_or_default)
}

/// Playlist `id`, with its count and length; `None` if there is none.
pub fn playlist(conn: &Connection, id: i64) -> Result<Option<Playlist>, Error> {
    let row: Option<(String, Option<String>, i64, i64, u32, f64)> = conn
        .query_row(
            "SELECT p.name, p.rules, p.created_at, p.updated_at,
                    (SELECT count(*) FROM playlist_items WHERE playlist_id = p.id),
                    (SELECT IFNULL(sum(t.duration), 0) FROM playlist_items i
                     JOIN tracks t ON t.id = i.track_id WHERE i.playlist_id = p.id)
             FROM playlists p WHERE p.id = ?1",
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
        )
        .optional()?;
    let Some((name, rules, created_at, updated_at, count, duration)) = row else {
        return Ok(None);
    };
    // Rules this version can't read (a newer version's) match nothing.
    let rules = rules.map(|json| {
        serde_json::from_str::<SmartRules>(&json).unwrap_or(SmartRules {
            match_all: false,
            conditions: Vec::new(),
            order: smart::SmartOrder::Title,
            limit: Some(1),
            seed: 0,
        })
    });
    let (track_count, duration) = match &rules {
        Some(rules) => smart::summary(conn, rules)?,
        None => (count, duration),
    };
    Ok(Some(Playlist {
        id,
        name,
        rules,
        track_count,
        duration,
        created_at,
        updated_at,
    }))
}

fn require(conn: &Connection, id: i64) -> Result<Playlist, Error> {
    playlist(conn, id)?.ok_or_else(|| Error::Invalid(gone(Gone::Playlist)))
}

fn require_list(conn: &Connection, id: i64) -> Result<Playlist, Error> {
    let playlist = require(conn, id)?;
    if playlist.rules.is_some() {
        return Err(Error::Invalid(coded(
            "smartTracks",
            &[],
            "A smart playlist's tracks follow its rules",
        )));
    }
    Ok(playlist)
}

fn touch(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
        params![unix_now(), id],
    )?;
    Ok(())
}

/// Creates a playlist of `track_ids` (those in the library), or a smart
/// playlist of `rules`.
pub fn create(
    conn: &mut Connection,
    name: &str,
    rules: Option<&SmartRules>,
    track_ids: &[i64],
) -> Result<Playlist, Error> {
    let name = clean_name(name)?;
    let rules = rules_json(rules)?;
    let tx = conn.transaction()?;
    let now = unix_now();
    tx.execute(
        "INSERT INTO playlists (name, rules, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![name, rules, now],
    )?;
    let id = tx.last_insert_rowid();
    if rules.is_none() {
        insert_items(&tx, id, 0, track_ids)?;
    }
    tx.commit()?;
    require(conn, id)
}

pub fn rename(conn: &Connection, id: i64, name: &str) -> Result<Playlist, Error> {
    let name = clean_name(name)?;
    conn.execute(
        "UPDATE playlists SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name, unix_now(), id],
    )?;
    require(conn, id)
}

/// Replaces a smart playlist's rules.
pub fn set_rules(conn: &Connection, id: i64, rules: &SmartRules) -> Result<Playlist, Error> {
    if require(conn, id)?.rules.is_none() {
        return Err(Error::Invalid(coded(
            "notSmart",
            &[],
            "Not a smart playlist",
        )));
    }
    conn.execute(
        "UPDATE playlists SET rules = ?1, updated_at = ?2 WHERE id = ?3",
        params![rules_json(Some(rules))?, unix_now(), id],
    )?;
    require(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<(), Error> {
    conn.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
    Ok(())
}

/// Inserts `track_ids` (those in the library) at `position`, making room.
fn insert_items(
    conn: &Connection,
    playlist_id: i64,
    position: i64,
    track_ids: &[i64],
) -> Result<usize, Error> {
    let ids = serde_json::to_string(track_ids).expect("ids serialize");
    let valid: Vec<i64> = conn
        .prepare_cached(
            "SELECT j.value FROM json_each(?1) j JOIN tracks t ON t.id = j.value ORDER BY j.key",
        )?
        .query_map([ids], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    conn.execute(
        "UPDATE playlist_items SET position = position + ?1
         WHERE playlist_id = ?2 AND position >= ?3",
        params![valid.len() as i64, playlist_id, position],
    )?;
    for (offset, track_id) in valid.iter().enumerate() {
        conn.prepare_cached(
            "INSERT INTO playlist_items (playlist_id, position, track_id) VALUES (?1, ?2, ?3)",
        )?
        .execute(params![playlist_id, position + offset as i64, track_id])?;
    }
    Ok(valid.len())
}

/// The playlist's item ids in order.
fn item_ids(conn: &Connection, playlist_id: i64) -> rusqlite::Result<Vec<i64>> {
    conn.prepare_cached(
        "SELECT id FROM playlist_items WHERE playlist_id = ?1 ORDER BY position, id",
    )?
    .query_map([playlist_id], |row| row.get(0))?
    .collect()
}

/// Numbers the items 0, 1, 2… in `order`.
fn renumber(conn: &Connection, order: &[i64]) -> rusqlite::Result<()> {
    for (position, item) in order.iter().enumerate() {
        conn.prepare_cached("UPDATE playlist_items SET position = ?1 WHERE id = ?2")?
            .execute(params![position as i64, item])?;
    }
    Ok(())
}

/// Adds tracks before the entry at index `at`, or at the end. Returns how
/// many were added (tracks no longer in the library are left out).
pub fn add(
    conn: &mut Connection,
    id: i64,
    track_ids: &[i64],
    at: Option<usize>,
) -> Result<usize, Error> {
    require_list(conn, id)?;
    let tx = conn.transaction()?;
    let order = item_ids(&tx, id)?;
    renumber(&tx, &order)?;
    let at = at.map_or(order.len(), |at| at.min(order.len()));
    let added = insert_items(&tx, id, at as i64, track_ids)?;
    touch(&tx, id)?;
    tx.commit()?;
    Ok(added)
}

/// Removes entries.
pub fn remove(conn: &mut Connection, id: i64, item_ids: &[i64]) -> Result<(), Error> {
    require_list(conn, id)?;
    let tx = conn.transaction()?;
    let ids = serde_json::to_string(item_ids).expect("ids serialize");
    tx.execute(
        "DELETE FROM playlist_items
         WHERE playlist_id = ?1 AND id IN (SELECT value FROM json_each(?2))",
        params![id, ids],
    )?;
    renumber(&tx, &self::item_ids(&tx, id)?)?;
    touch(&tx, id)?;
    tx.commit()?;
    Ok(())
}

/// Moves entries (keeping their order) to index `to` of the list after the
/// move.
pub fn move_items(conn: &mut Connection, id: i64, moving: &[i64], to: usize) -> Result<(), Error> {
    require_list(conn, id)?;
    let tx = conn.transaction()?;
    let order = item_ids(&tx, id)?;
    let (moved, mut rest): (Vec<i64>, Vec<i64>) =
        order.into_iter().partition(|item| moving.contains(item));
    let to = to.min(rest.len());
    rest.splice(to..to, moved);
    renumber(&tx, &rest)?;
    touch(&tx, id)?;
    tx.commit()?;
    Ok(())
}

/// Up to `limit` entries from `offset`.
pub fn page(conn: &Connection, id: i64, offset: u32, limit: u32) -> Result<PlaylistPage, Error> {
    let playlist = require(conn, id)?;
    let entries = match &playlist.rules {
        Some(rules) => smart::tracks(conn, rules, offset, limit)?
            .into_iter()
            .map(|track| PlaylistEntry {
                item_id: None,
                track,
            })
            .collect(),
        None => conn
            .prepare_cached(&format!(
                "SELECT {TRACK_COLUMNS}, i.id {TRACKS_FROM}
                 JOIN playlist_items i ON i.track_id = t.id
                 WHERE i.playlist_id = ?1 ORDER BY i.position, i.id LIMIT ?2 OFFSET ?3"
            ))?
            .query_map(params![id, limit, offset], |row| {
                Ok(PlaylistEntry {
                    track: track_from_row(row)?,
                    item_id: Some(row.get(27)?),
                })
            })?
            .collect::<Result<_, _>>()?,
    };
    Ok(PlaylistPage { playlist, entries })
}

/// The playlist's tracks in order, to play.
pub fn track_ids(conn: &Connection, id: i64) -> Result<Vec<i64>, Error> {
    let playlist = require(conn, id)?;
    match &playlist.rules {
        Some(rules) => smart::track_ids(conn, rules),
        None => Ok(conn
            .prepare_cached(
                "SELECT track_id FROM playlist_items WHERE playlist_id = ?1 ORDER BY position, id",
            )?
            .query_map([id], |row| row.get(0))?
            .collect::<Result<_, _>>()?),
    }
}

// ---- M3U ------------------------------------------------------------------

/// What importing a playlist file did.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub playlist: Playlist,
    pub added: usize,
    /// Entries that aren't in the library, as the file names them.
    pub missing: Vec<String>,
}

/// Imports an M3U or M3U8 file as a new playlist named after it. Entries
/// are resolved against the library (`resolve`); those it hasn't got are
/// listed in the report.
pub fn import_m3u(conn: &mut Connection, file: &Path) -> Result<ImportReport, Error> {
    let size = std::fs::metadata(file)
        .map_err(|e| Error::Invalid(format!("Cannot open {}: {e}", file.display())))?
        .len();
    if size > MAX_M3U {
        let name = file.display().to_string();
        return Err(Error::Invalid(coded(
            "playlistTooLarge",
            &[("file", name.clone().into())],
            format!("{name} is too large for a playlist"),
        )));
    }
    let bytes = std::fs::read(file)
        .map_err(|e| Error::Invalid(format!("Cannot read {}: {e}", file.display())))?;
    let text = cue::decode(&bytes);
    let base = file.parent().unwrap_or(Path::new("/"));
    let mut track_ids = Vec::new();
    let mut missing = Vec::new();
    for entry in entries(&text) {
        match resolve(conn, base, &entry)? {
            Some(id) => track_ids.push(id),
            None => missing.push(entry),
        }
    }
    let name = file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("Imported playlist");
    let playlist = create(conn, name, None, &track_ids)?;
    Ok(ImportReport {
        added: playlist.track_count as usize,
        playlist,
        missing,
    })
}

/// The entries of an M3U playlist: its lines that aren't comments or
/// directives (#EXTINF and the like), with file: URLs decoded.
fn entries(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| match line.strip_prefix("file://") {
            Some(url) => percent_decode(url.strip_prefix("localhost").unwrap_or(url)),
            None => line.to_owned(),
        })
        .collect()
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(byte) = hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The library track an entry names: a path (absolute, or relative to the
/// playlist's folder, with '/' or '\' separators) inside a library folder;
/// else the one track whose path ends with the entry's last folders and
/// file name (a playlist made on another computer). The first part of a
/// file split into parts.
fn resolve(conn: &Connection, base: &Path, entry: &str) -> Result<Option<i64>, Error> {
    let normalised = entry.replace('\\', "/");
    let is_windows_absolute = normalised.len() > 2 && normalised.as_bytes()[1] == b':';
    if !is_windows_absolute {
        let path = if normalised.starts_with('/') {
            PathBuf::from(&normalised)
        } else {
            base.join(&normalised)
        };
        let path = lexical_clean(&path);
        let folders: Vec<(i64, String)> = conn
            .prepare_cached("SELECT id, path FROM folders")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (folder_id, folder) in folders {
            let Ok(rest) = path.strip_prefix(&folder) else {
                continue;
            };
            let relative: Option<Vec<&str>> = rest
                .components()
                .map(|component| component.as_os_str().to_str())
                .collect();
            let Some(relative) = relative else { continue };
            let found = conn
                .prepare_cached(
                    "SELECT id FROM tracks WHERE folder_id = ?1 AND relative_path = ?2
                     ORDER BY range_start LIMIT 1",
                )?
                .query_row(params![folder_id, relative.join("/")], |row| row.get(0))
                .optional()?;
            if found.is_some() {
                return Ok(found);
            }
        }
    }
    // By the end of its path: three names, then two, then the file alone,
    // as long as only one track fits.
    let names: Vec<&str> = normalised
        .split('/')
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .collect();
    for count in (1..=names.len().min(3)).rev() {
        let tail = names[names.len() - count..].join("/");
        let found: Vec<i64> = conn
            .prepare_cached(
                "SELECT id FROM tracks
                 WHERE range_start = 0
                   AND (relative_path = ?1
                        OR (length(relative_path) > length(?1)
                            AND substr(relative_path, -length(?1)) = ?1
                            AND substr(relative_path, -length(?1) - 1, 1) = '/'))
                 LIMIT 2",
            )?
            .query_map([&tail], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        if let [id] = found[..] {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

/// `path` with "." and ".." resolved without touching the disk.
fn lexical_clean(path: &Path) -> PathBuf {
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other.as_os_str()),
        }
    }
    clean
}

/// Writes playlist `id` as an M3U8 file (UTF-8) at `file`, which the user
/// chose: each track's path relative to the file's folder where they share
/// more than the root of the disk, else absolute. Returns the tracks
/// written. A part of a file (a cue sheet's track) is written as its file.
pub fn export_m3u(conn: &Connection, id: i64, file: &Path) -> Result<usize, Error> {
    let tracks = track_ids(conn, id)?;
    let base = file.parent().unwrap_or(Path::new("/"));
    let mut text = String::from("#EXTM3U\n");
    let mut written = 0;
    for track_id in tracks {
        let row: Option<(String, String, Option<String>, Option<String>, f64, String)> = conn
            .query_row(
                "SELECT f.path, t.relative_path, t.title, IFNULL(t.artist_credit, a.name),
                        t.duration, t.relative_path
                 FROM tracks t JOIN folders f ON f.id = t.folder_id
                 LEFT JOIN artists a ON a.id = t.artist_id WHERE t.id = ?1",
                [track_id],
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
            )
            .optional()?;
        let Some((folder, relative, title, artist, duration, _)) = row else {
            continue;
        };
        let path = track_path(Path::new(&folder), &relative);
        let title = title.unwrap_or_else(|| relative.rsplit('/').next().unwrap_or("").to_owned());
        let label = match artist {
            Some(artist) => format!("{artist} - {title}"),
            None => title,
        };
        text.push_str(&format!(
            "#EXTINF:{},{}\n{}\n",
            duration.round() as i64,
            label.replace(['\r', '\n'], " "),
            relative_to(base, &path)
        ));
        written += 1;
    }
    std::fs::write(file, text)
        .map_err(|e| Error::Invalid(format!("Cannot write {}: {e}", file.display())))?;
    Ok(written)
}

/// `target` relative to the folder `base` ('/'-separated), where the two
/// share more than the root (and, on macOS, the same volume), else
/// `target` as it is.
fn relative_to(base: &Path, target: &Path) -> String {
    let base: Vec<Component> = base.components().collect();
    let target_parts: Vec<Component> = target.components().collect();
    let common = base
        .iter()
        .zip(&target_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let volume = |parts: &[Component]| {
        parts
            .get(1)
            .is_some_and(|part| part.as_os_str() == "Volumes")
    };
    let shared_enough = common >= 2 && !(volume(&base) && common < 3);
    if !shared_enough {
        return target.to_string_lossy().into_owned();
    }
    let mut parts: Vec<String> =
        std::iter::repeat_n("..".to_owned(), base.len() - common).collect();
    parts.extend(
        target_parts[common..]
            .iter()
            .map(|part| part.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::smart::{Condition, SmartOrder};
    use crate::library::test_library::{titles, track, Library};

    fn library() -> Library {
        Library::new([
            track("Band/Record/01 One.flac").title("One").artist("Band"),
            track("Band/Record/02 Two.flac").title("Two").artist("Band"),
            track("Other/Three.mp3").title("Three").artist("Else"),
        ])
    }

    fn ids(library: &Library) -> Vec<i64> {
        crate::library::tracks(&library.conn)
            .unwrap()
            .iter()
            .map(|track| track.id)
            .collect()
    }

    fn listed(library: &Library, id: i64) -> Vec<String> {
        let page = page(&library.conn, id, 0, 100).unwrap();
        titles(
            &page
                .entries
                .into_iter()
                .map(|entry| entry.track)
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn edits_a_playlist() {
        let mut library = library();
        let t = ids(&library);
        let list = create(&mut library.conn, " Mix ", None, &[t[2], t[0], 999]).unwrap();
        assert_eq!((list.name.as_str(), list.track_count), ("Mix", 2));
        assert_eq!(list.duration, 120.0);
        assert_eq!(listed(&library, list.id), ["Three", "One"]);

        add(&mut library.conn, list.id, &[t[1], t[1]], Some(1)).unwrap();
        assert_eq!(listed(&library, list.id), ["Three", "Two", "Two", "One"]);
        let items: Vec<i64> = page(&library.conn, list.id, 0, 10)
            .unwrap()
            .entries
            .iter()
            .map(|entry| entry.item_id.unwrap())
            .collect();
        move_items(&mut library.conn, list.id, &[items[0]], 3).unwrap();
        assert_eq!(listed(&library, list.id), ["Two", "Two", "One", "Three"]);
        remove(&mut library.conn, list.id, &[items[1]]).unwrap();
        assert_eq!(listed(&library, list.id), ["Two", "One", "Three"]);
        assert_eq!(
            track_ids(&library.conn, list.id).unwrap(),
            [t[1], t[0], t[2]]
        );

        rename(&library.conn, list.id, "Road trip").unwrap();
        assert!(rename(&library.conn, list.id, "  ").is_err());
        assert_eq!(playlists(&library.conn).unwrap()[0].name, "Road trip");

        // A track that leaves the library leaves the playlist.
        library
            .conn
            .execute("DELETE FROM tracks WHERE id = ?1", [t[0]])
            .unwrap();
        assert_eq!(listed(&library, list.id), ["Two", "Three"]);
        delete(&library.conn, list.id).unwrap();
        assert!(playlists(&library.conn).unwrap().is_empty());
    }

    #[test]
    fn smart_playlists_follow_their_rules() {
        let mut library = library();
        let rules = SmartRules {
            match_all: true,
            conditions: vec![Condition::Artist {
                value: "Band".into(),
            }],
            order: SmartOrder::Title,
            limit: None,
            seed: 0,
        };
        let smart = create(&mut library.conn, "Band", Some(&rules), &[1]).unwrap();
        assert_eq!(smart.track_count, 2);
        assert_eq!(listed(&library, smart.id), ["One", "Two"]);
        assert!(page(&library.conn, smart.id, 0, 10).unwrap().entries[0]
            .item_id
            .is_none());
        assert!(add(&mut library.conn, smart.id, &[1], None).is_err());

        let mut other = rules.clone();
        other.conditions = vec![Condition::Artist {
            value: "Else".into(),
        }];
        set_rules(&library.conn, smart.id, &other).unwrap();
        assert_eq!(listed(&library, smart.id), ["Three"]);
    }

    #[test]
    fn imports_m3u_files_by_path_or_by_the_end_of_it() {
        let mut library = library();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Road trip.m3u8");
        std::fs::write(
            &file,
            "#EXTM3U\n#EXTINF:60,Band - One\n/Music/Band/Record/01 One.flac\n\
             \n# a comment\nfile:///Music/Other/Three.mp3\n\
             C:\\Users\\me\\Music\\Band\\Record\\02%20Two.flac\n\
             D:\\Elsewhere\\Record\\02 Two.flac\n/Music/Missing.flac\n",
        )
        .unwrap();
        let report = import_m3u(&mut library.conn, &file).unwrap();
        assert_eq!(report.playlist.name, "Road trip");
        assert_eq!(report.added, 3);
        assert_eq!(
            report.missing,
            [
                "C:\\Users\\me\\Music\\Band\\Record\\02%20Two.flac",
                "/Music/Missing.flac"
            ]
        );
        assert_eq!(
            listed(&library, report.playlist.id),
            ["One", "Three", "Two"]
        );

        // Relative to the playlist's folder, in the legacy code page.
        let relative = Path::new("/Music/Lists/old.m3u");
        let mut bytes = b"../Other/Three.mp3\r\n".to_vec();
        bytes.extend_from_slice(b"caf\xe9.mp3\r\n");
        assert_eq!(cue::decode(&bytes).lines().nth(1), Some("café.mp3"));
        let resolved = resolve(
            &library.conn,
            relative.parent().unwrap(),
            "../Other/Three.mp3",
        );
        assert_eq!(resolved.unwrap(), Some(ids(&library)[2]));
    }

    #[test]
    fn exports_m3u8_relative_where_it_can() {
        let mut library = library();
        let t = ids(&library);
        let list = create(&mut library.conn, "Mix", None, &[t[2], t[0]]).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("mix.m3u8");
        assert_eq!(export_m3u(&library.conn, list.id, &file).unwrap(), 2);
        let text = std::fs::read_to_string(&file).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "#EXTM3U");
        assert_eq!(lines[1], "#EXTINF:60,Else - Three");
        // The temporary folder shares only the root with /Music.
        assert_eq!(lines[2], "/Music/Other/Three.mp3");

        assert_eq!(
            relative_to(Path::new("/Music/Lists"), Path::new("/Music/Band/a.flac")),
            "../Band/a.flac"
        );
        assert_eq!(
            relative_to(
                Path::new("/Volumes/USB/Lists"),
                Path::new("/Volumes/Disk/a.flac")
            ),
            "/Volumes/Disk/a.flac"
        );
        assert_eq!(
            relative_to(
                Path::new("/Volumes/Disk/Lists"),
                Path::new("/Volumes/Disk/a.flac")
            ),
            "../a.flac"
        );
    }

    #[test]
    fn decodes_file_urls() {
        assert_eq!(
            entries("file:///Music/a%20b.flac\nfile://localhost/x.mp3\n#EXTINF:1,x\ny.ogg"),
            ["/Music/a b.flac", "/x.mp3", "y.ogg"]
        );
        assert_eq!(percent_decode("100%"), "100%");
    }
}
