//! The library health report (PLAN.md O4): problems the app can detect
//! but otherwise hides. Read-only; each track row names its file, so the
//! UI can reveal it in Finder.
//!
//! - From the analysis (O1): files that fail to decode, files that decode
//!   shorter than their header says (truncated), and "lossless" files whose
//!   spectrum stops at 15–19.5 kHz as a lossy encoder leaves it (a
//!   heuristic, so only "suspected" transcodes).
//! - From the tags alone: albums whose tracks disagree (years, disc totals,
//!   track numbers missing or repeated), albums split by different album
//!   artists within one folder, and likely duplicates (the same recording
//!   id, or the same title and artist with lengths within 2 s).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use super::analysis::NOT_FOLDER_ERROR;
use super::{track_path, Error};

/// Rows listed per kind at most.
const LIMIT: usize = 500;

/// Lengths within this many seconds are the same, for duplicates.
const SAME_LENGTH: f64 = 2.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub undecodable: Vec<HealthTrack>,
    pub truncated: Vec<HealthTrack>,
    pub transcodes: Vec<HealthTrack>,
    pub albums: Vec<AlbumIssue>,
    pub duplicates: Vec<DuplicateGroup>,
    /// Tracks analysed, and in the library: the analysis checks cover only
    /// the first.
    pub analysed: u32,
    pub tracks: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct HealthTrack {
    pub track_id: i64,
    /// Absolute.
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// What's wrong, e.g. "Decodes 2:01 of 4:10".
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AlbumIssue {
    pub album_id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    /// Why they look alike.
    pub reason: String,
    pub tracks: Vec<HealthTrack>,
}

const TRACK: &str = "t.id, f.path, t.relative_path, IFNULL(t.title, t.relative_path),
     IFNULL(t.artist_credit, artist.name), album.title";
const FROM: &str = "FROM tracks t JOIN folders f ON f.id = t.folder_id
     LEFT JOIN artists artist ON artist.id = t.artist_id
     LEFT JOIN albums album ON album.id = t.album_id";
/// The track's current analysis, joined as `a`.
const ANALYSIS: &str = "JOIN track_analysis a ON a.track_id = t.id
     AND a.file_size = t.file_size AND a.file_mtime_ns = t.file_mtime_ns";

fn health_track(row: &rusqlite::Row, detail: String) -> rusqlite::Result<HealthTrack> {
    let folder: String = row.get(1)?;
    let relative: String = row.get(2)?;
    let title: String = row.get(3)?;
    Ok(HealthTrack {
        track_id: row.get(0)?,
        path: track_path(Path::new(&folder), &relative)
            .to_string_lossy()
            .into_owned(),
        title: title.rsplit('/').next().unwrap_or(&title).to_owned(),
        artist: row.get(4)?,
        album: row.get(5)?,
        detail,
    })
}

fn minutes(seconds: f64) -> String {
    let seconds = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

pub fn report(conn: &Connection) -> Result<HealthReport, Error> {
    let tracks = |sql: &str,
                  detail: &dyn Fn(&rusqlite::Row) -> rusqlite::Result<String>|
     -> Result<Vec<HealthTrack>, Error> {
        let mut statement = conn.prepare(sql)?;
        let rows = statement.query_map([LIMIT as i64], |row| {
            let text = detail(row)?;
            health_track(row, text)
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    };

    let undecodable = tracks(
        // A folder out of reach isn't a broken file (PLAN.md H22).
        &format!(
            "SELECT {TRACK}, a.error {FROM} {ANALYSIS}
             WHERE a.error IS NOT NULL AND a.{NOT_FOLDER_ERROR} LIMIT ?1"
        ),
        &|row| row.get(6),
    )?;
    let truncated = tracks(
        &format!(
            "SELECT {TRACK}, a.duration, t.duration {FROM} {ANALYSIS}
             WHERE a.error IS NULL AND t.range_end IS NULL AND t.range_start = 0
               AND a.duration < t.duration - max(1.0, 0.01 * t.duration)
             LIMIT ?1"
        ),
        &|row| {
            let (decoded, header): (f64, f64) = (row.get(6)?, row.get(7)?);
            Ok(format!(
                "Decodes {} of {}",
                minutes(decoded),
                minutes(header)
            ))
        },
    )?;
    // Lossless by extension (an .m4a may hold AAC or ALAC; ALAC's bit rate
    // is far above any AAC's).
    let transcodes = tracks(
        &format!(
            "SELECT {TRACK}, a.cutoff_hz {FROM} {ANALYSIS}
             WHERE a.cutoff_hz BETWEEN 15000 AND 19500 AND t.sample_rate >= 44100
               AND (lower(t.relative_path) GLOB '*.flac' OR lower(t.relative_path) GLOB '*.wav'
                    OR lower(t.relative_path) GLOB '*.aif' OR lower(t.relative_path) GLOB '*.aiff'
                    OR lower(t.relative_path) GLOB '*.aifc'
                    OR (lower(t.relative_path) GLOB '*.m4a' AND t.bitrate_kbps >= 400))
             ORDER BY a.cutoff_hz LIMIT ?1"
        ),
        &|row| {
            let cutoff: f64 = row.get(6)?;
            Ok(format!(
                "Suspected transcode: the spectrum stops at {:.1} kHz",
                cutoff / 1000.0
            ))
        },
    )?;

    let analysed = conn.query_row(
        &format!("SELECT count(*) FROM tracks t {ANALYSIS}"),
        [],
        |row| row.get(0),
    )?;
    let track_count = conn.query_row("SELECT count(*) FROM tracks", [], |row| row.get(0))?;

    Ok(HealthReport {
        undecodable,
        truncated,
        transcodes,
        albums: album_issues(conn)?,
        duplicates: duplicates(conn)?,
        analysed,
        tracks: track_count,
    })
}

fn album_issues(conn: &Connection) -> Result<Vec<AlbumIssue>, Error> {
    let mut issues: BTreeMap<i64, AlbumIssue> = BTreeMap::new();
    let mut add = |album_id: i64, title: String, artist: Option<String>, problem: String| {
        issues
            .entry(album_id)
            .or_insert_with(|| AlbumIssue {
                album_id,
                title,
                artist,
                problems: Vec::new(),
            })
            .problems
            .push(problem);
    };

    // Within an album. Parts of one file (a cue sheet's tracks) count as tracks.
    let mut statement = conn.prepare(
        "SELECT al.id, al.title, ar.name,
                count(DISTINCT t.year), count(DISTINCT t.disc_total),
                sum(t.track_number IS NULL), count(*),
                (SELECT count(*) FROM (SELECT 1 FROM tracks d WHERE d.album_id = al.id
                     AND d.track_number IS NOT NULL
                     GROUP BY IFNULL(d.disc_number, 1), d.track_number HAVING count(*) > 1))
         FROM albums al JOIN tracks t ON t.album_id = al.id
         LEFT JOIN artists ar ON ar.id = al.artist_id
         GROUP BY al.id ORDER BY al.title, al.id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, u32>(3)?,
            row.get::<_, u32>(4)?,
            row.get::<_, u32>(5)?,
            row.get::<_, u32>(6)?,
            row.get::<_, u32>(7)?,
        ))
    })?;
    for row in rows {
        let (id, title, artist, years, disc_totals, unnumbered, count, repeated) = row?;
        if years > 1 {
            add(
                id,
                title.clone(),
                artist.clone(),
                format!("Its tracks have {years} different years"),
            );
        }
        if disc_totals > 1 {
            add(
                id,
                title.clone(),
                artist.clone(),
                "Its tracks disagree on the number of discs".into(),
            );
        }
        if unnumbered > 0 && unnumbered < count {
            add(
                id,
                title.clone(),
                artist.clone(),
                format!("{unnumbered} of its {count} tracks have no track number"),
            );
        }
        if repeated > 0 {
            add(
                id,
                title,
                artist,
                "A track number is used twice on one disc".into(),
            );
        }
    }

    // One folder, one album title, several album artists: an album split in two.
    let mut statement = conn.prepare(
        "SELECT t.folder_id, rtrim(t.relative_path, replace(t.relative_path, '/', '')),
                lower(al.title), al.id, al.title, ar.name
         FROM tracks t JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.artist_id
         GROUP BY 1, 2, 3, 4",
    )?;
    let mut by_folder: HashMap<(i64, String, String), Vec<(i64, String, Option<String>)>> =
        HashMap::new();
    for row in statement.query_map([], |row| {
        Ok((
            (row.get(0)?, row.get(1)?, row.get(2)?),
            (row.get(3)?, row.get(4)?, row.get(5)?),
        ))
    })? {
        let (key, album) = row?;
        by_folder.entry(key).or_default().push(album);
    }
    for albums in by_folder.into_values().filter(|albums| albums.len() > 1) {
        let artists: Vec<String> = albums
            .iter()
            .map(|(_, _, artist)| artist.clone().unwrap_or_else(|| "no album artist".into()))
            .collect();
        for (id, title, artist) in &albums {
            add(
                *id,
                title.clone(),
                artist.clone(),
                format!(
                    "Split by album artist in one folder: {}",
                    artists.join(", ")
                ),
            );
        }
    }
    Ok(issues.into_values().take(LIMIT).collect())
}

fn duplicates(conn: &Connection) -> Result<Vec<DuplicateGroup>, Error> {
    let mut groups = Vec::new();

    let mut statement = conn.prepare(&format!(
        "SELECT {TRACK}, t.musicbrainz_recording_id {FROM}
         WHERE t.musicbrainz_recording_id IN (
             SELECT musicbrainz_recording_id FROM tracks WHERE musicbrainz_recording_id IS NOT NULL
             GROUP BY musicbrainz_recording_id HAVING count(*) > 1)
         ORDER BY t.musicbrainz_recording_id, t.id"
    ))?;
    let mut by_recording: BTreeMap<String, Vec<HealthTrack>> = BTreeMap::new();
    for row in statement.query_map([], |row| {
        Ok((row.get::<_, String>(6)?, health_track(row, String::new())?))
    })? {
        let (recording, track) = row?;
        by_recording.entry(recording).or_default().push(track);
    }
    for tracks in by_recording.into_values() {
        groups.push(DuplicateGroup {
            reason: "The same recording (MusicBrainz id)".into(),
            tracks,
        });
    }

    // The same title and artist, near enough the same length, not already
    // grouped by recording.
    let mut statement = conn.prepare(&format!(
        "SELECT {TRACK}, lower(trim(t.title)), t.artist_id, t.duration {FROM}
         WHERE t.title IS NOT NULL AND t.artist_id IS NOT NULL
           AND (lower(trim(t.title)), t.artist_id) IN (
               SELECT lower(trim(title)), artist_id FROM tracks
               WHERE title IS NOT NULL AND artist_id IS NOT NULL
               GROUP BY 1, 2 HAVING count(*) > 1)
         ORDER BY 7, 8, t.duration, t.id"
    ))?;
    let grouped: std::collections::HashSet<i64> = groups
        .iter()
        .flat_map(|group| group.tracks.iter().map(|track| track.track_id))
        .collect();
    let mut current: Vec<(HealthTrack, f64)> = Vec::new();
    let mut current_key: Option<(String, i64)> = None;
    let flush = |tracks: &mut Vec<(HealthTrack, f64)>, groups: &mut Vec<DuplicateGroup>| {
        // Runs of lengths each within 2 s of the previous.
        let mut run: Vec<HealthTrack> = Vec::new();
        let mut last = f64::NEG_INFINITY;
        for (track, duration) in tracks.drain(..) {
            if duration - last > SAME_LENGTH && run.len() > 1 {
                groups.push(DuplicateGroup {
                    reason: "The same title and artist, and about the same length".into(),
                    tracks: std::mem::take(&mut run),
                });
            } else if duration - last > SAME_LENGTH {
                run.clear();
            }
            last = duration;
            run.push(track);
        }
        if run.len() > 1 {
            groups.push(DuplicateGroup {
                reason: "The same title and artist, and about the same length".into(),
                tracks: run,
            });
        }
    };
    for row in statement.query_map([], |row| {
        Ok((
            health_track(row, String::new())?,
            row.get::<_, String>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, f64>(8)?,
        ))
    })? {
        let (track, title, artist, duration) = row?;
        if grouped.contains(&track.track_id) {
            continue;
        }
        let key = Some((title, artist));
        if key != current_key {
            flush(&mut current, &mut groups);
            current_key = key;
        }
        current.push((track, duration));
    }
    flush(&mut current, &mut groups);
    groups.truncate(LIMIT);
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};

    #[test]
    fn a_folder_out_of_reach_is_not_undecodable() {
        let library = Library::new([track("a.flac")]);
        let error = crate::coded::folder_unavailable("/Music", "missing", None);
        library
            .conn
            .execute(
                "INSERT INTO track_analysis (track_id, file_size, file_mtime_ns, analysed_at, error)
                 VALUES (1, 0, 0, 0, ?1)",
                [error],
            )
            .unwrap();
        assert!(report(&library.conn).unwrap().undecodable.is_empty());
    }

    #[test]
    fn reports_analysis_problems() {
        let library = Library::new([
            track("a.flac").title("Broken"),
            track("b.mp3").title("Short"),
            track("c.flac").title("Transcode"),
            track("d.mp3").title("Lossy"),
        ]);
        library
            .conn
            .execute_batch(
                "UPDATE tracks SET duration = 250.0;
                 INSERT INTO track_analysis (track_id, file_size, file_mtime_ns, analysed_at, error)
                 VALUES (1, 0, 0, 0, 'Cannot decode');
                 INSERT INTO track_analysis (track_id, file_size, file_mtime_ns, analysed_at, duration)
                 VALUES (2, 0, 0, 0, 121.0);
                 INSERT INTO track_analysis (track_id, file_size, file_mtime_ns, analysed_at, duration,
                                             cutoff_hz)
                 VALUES (3, 0, 0, 0, 250.0, 16000.0), (4, 0, 0, 0, 250.0, 16000.0);",
            )
            .unwrap();
        let report = report(&library.conn).unwrap();
        assert_eq!(report.undecodable.len(), 1);
        assert_eq!(report.undecodable[0].detail, "Cannot decode");
        assert_eq!(report.undecodable[0].path, "/Music/a.flac");
        assert_eq!(report.truncated.len(), 1);
        assert_eq!(report.truncated[0].detail, "Decodes 2:01 of 4:10");
        assert_eq!(report.transcodes.len(), 1, "an MP3 is lossy anyway");
        assert_eq!(report.transcodes[0].title, "Transcode");
        assert_eq!((report.analysed, report.tracks), (4, 4));
    }

    #[test]
    fn reports_inconsistent_albums_and_duplicates() {
        let library = Library::new([
            track("x/1.flac")
                .title("One")
                .artist("A")
                .album("Mixed")
                .year(1990)
                .number(1),
            track("x/2.flac")
                .title("Two")
                .artist("A")
                .album("Mixed")
                .year(1991)
                .number(1),
            track("x/3.flac")
                .title("Three")
                .artist("A")
                .album("Mixed")
                .year(1990),
            track("y/1.flac").title("Song").artist("B").album("Split"),
            track("y/2.flac").title("Other").artist("C").album("Split"),
            track("z/1.flac").title("Song").artist("B").album("Best Of"),
        ]);
        let report = report(&library.conn).unwrap();
        let mixed = report.albums.iter().find(|a| a.title == "Mixed").unwrap();
        assert_eq!(
            mixed.problems,
            [
                "Its tracks have 2 different years",
                "1 of its 3 tracks have no track number",
                "A track number is used twice on one disc"
            ]
        );
        let split: Vec<&AlbumIssue> = report
            .albums
            .iter()
            .filter(|a| a.title == "Split")
            .collect();
        assert_eq!(split.len(), 2);
        assert!(split[0].problems[0].starts_with("Split by album artist in one folder"));

        // "Song" by B twice, both 60 s long.
        assert_eq!(report.duplicates.len(), 1);
        let titles: Vec<&str> = report.duplicates[0]
            .tracks
            .iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(titles, ["Song", "Song"]);
    }
}
