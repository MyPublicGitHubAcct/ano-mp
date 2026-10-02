//! Lyrics already on disk (PLAN.md O13), read when shown and never kept in
//! the library: a `.lrc` file next to the track (read through its folder's
//! bookmark, as folder images are), else synced lyrics in the tags (ID3v2
//! SYLT, or LRC text in a lyrics tag), else the tags' plain lyrics. Online
//! lyrics are out of scope.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use super::access::open_folder_of;
use super::{track_path, Error};
use crate::anomp::{self, TagParts};

/// `.lrc` files larger than this aren't lyrics.
const LRC_LIMIT: u64 = 1 << 20;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    /// Timed lines, in order; empty for unsynced lyrics.
    pub lines: Vec<LyricLine>,
    /// Unsynced lyrics, when there are no timed lines.
    pub text: Option<String>,
    /// Where they came from: "lrc" (a file next to the track) or "tags".
    pub source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    /// Seconds into the track.
    pub time: f64,
    pub text: String,
}

/// Timed lines from LRC text: `[mm:ss.xx]` stamps (several may share a
/// line), honouring an `[offset:±ms]` tag; other tags are skipped. Empty
/// if the text has no stamps.
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut offset = 0.0;
    let mut lines = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(inner) = rest.strip_prefix('[') {
            let Some(close) = inner.find(']') else { break };
            let tag = &inner[..close];
            rest = &inner[close + 1..];
            if let Some(ms) = tag.strip_prefix("offset:") {
                // A positive offset shows lyrics sooner.
                offset = ms.trim().parse::<f64>().unwrap_or(0.0) / 1000.0;
            } else if let Some(time) = parse_stamp(tag) {
                times.push(time);
            }
        }
        for time in times {
            lines.push(LyricLine {
                time,
                text: rest.trim().to_owned(),
            });
        }
    }
    for line in &mut lines {
        line.time = (line.time - offset).max(0.0);
    }
    lines.sort_by(|a, b| a.time.total_cmp(&b.time));
    lines
}

/// "mm:ss", "mm:ss.xx" or "mm:ss.xxx" as seconds.
fn parse_stamp(tag: &str) -> Option<f64> {
    let (minutes, seconds) = tag.split_once(':')?;
    let minutes: u32 = minutes.trim().parse().ok()?;
    let seconds: f64 = seconds.trim().parse().ok()?;
    (0.0..60.0)
        .contains(&seconds)
        .then(|| f64::from(minutes) * 60.0 + seconds)
}

/// The `.lrc` next to `audio`: "Song.lrc", else "Song.flac.lrc".
fn lrc_file(audio: &Path) -> Option<PathBuf> {
    let with_stem = audio.with_extension("lrc");
    if with_stem.is_file() {
        return Some(with_stem);
    }
    let mut name = audio.file_name()?.to_os_string();
    name.push(".lrc");
    let with_name = audio.with_file_name(name);
    with_name.is_file().then_some(with_name)
}

fn read_lrc(path: &Path) -> Option<String> {
    if std::fs::metadata(path).ok()?.len() > LRC_LIMIT {
        return None;
    }
    Some(super::cue::decode(&std::fs::read(path).ok()?))
}

/// The lyrics of track `track_id`, if it has any. A part of a file (a cue
/// sheet's track) gets the file's synced lines that fall within it.
pub fn lyrics(conn: &Connection, track_id: i64) -> Result<Option<Lyrics>, Error> {
    let row: Option<(String, String, f64, Option<f64>)> = conn
        .query_row(
            "SELECT f.path, t.relative_path, t.range_start, t.range_end
             FROM tracks t JOIN folders f ON f.id = t.folder_id WHERE t.id = ?1",
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((folder, relative, start, end)) = row else {
        return Ok(None);
    };
    let path = track_path(Path::new(&folder), &relative);
    let _folder = open_folder_of(conn, &path)?;
    let within = |lines: Vec<LyricLine>| -> Vec<LyricLine> {
        lines
            .into_iter()
            .filter(|line| line.time >= start && end.is_none_or(|end| line.time < end))
            .map(|line| LyricLine {
                time: line.time - start,
                ..line
            })
            .collect()
    };

    if let Some(text) = lrc_file(&path).and_then(|lrc| read_lrc(&lrc)) {
        let lines = within(parse_lrc(&text));
        if !lines.is_empty() {
            return Ok(Some(Lyrics {
                lines,
                text: None,
                source: "lrc",
            }));
        }
    }
    let tags = anomp::read_tags_with(
        &path,
        TagParts {
            lyrics: true,
            ..TagParts::default()
        },
    )
    .map_err(Error::Invalid)?;
    for synced in [tags.synced_lyrics.as_deref(), tags.lyrics.as_deref()]
        .into_iter()
        .flatten()
    {
        let lines = within(parse_lrc(synced));
        if !lines.is_empty() {
            return Ok(Some(Lyrics {
                lines,
                text: None,
                source: "tags",
            }));
        }
    }
    Ok(tags.lyrics.map(|text| Lyrics {
        lines: Vec::new(),
        text: Some(text),
        source: "tags",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db;

    #[test]
    fn parses_lrc() {
        let lines = parse_lrc(
            "[ar:Band]\n[offset:500]\n[00:12.50]First\n[01:02.00][00:30.25] Chorus \nno stamp\n[xx:yy]bad\n",
        );
        let summary: Vec<(f64, &str)> = lines.iter().map(|l| (l.time, l.text.as_str())).collect();
        assert_eq!(
            summary,
            [(12.0, "First"), (29.75, "Chorus"), (61.5, "Chorus")]
        );
        assert!(parse_lrc("Just words\nMore words").is_empty());
        assert_eq!(parse_lrc("[00:01.123]Milliseconds")[0].time, 1.123);
    }

    #[test]
    fn reads_an_lrc_file_next_to_the_track() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Music");
        std::fs::create_dir(&root).unwrap();
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../core/tests/fixtures/flac-44k.flac"),
            root.join("song.flac"),
        )
        .unwrap();
        let conn = db::open_in_memory().unwrap();
        let folder = crate::library::add_folder(&conn, &root).unwrap();
        conn.execute(
            "INSERT INTO tracks (folder_id, relative_path, file_size, file_mtime_ns, duration,
                                 sample_rate, channels, scanned_at)
             VALUES (?1, 'song.flac', 1, 1, 0.5, 44100, 2, 0)",
            [folder.id],
        )
        .unwrap();
        assert_eq!(lyrics(&conn, 1).unwrap(), None);

        std::fs::write(root.join("song.lrc"), "[00:00.10]Hello\n[00:00.40]World\n").unwrap();
        let found = lyrics(&conn, 1).unwrap().unwrap();
        assert_eq!(found.source, "lrc");
        assert_eq!(found.lines.len(), 2);

        // A part of the file gets its own lines, from its start.
        conn.execute("UPDATE tracks SET range_start = 0.3", [])
            .unwrap();
        let part = lyrics(&conn, 1).unwrap().unwrap();
        assert_eq!(part.lines.len(), 1);
        assert!((part.lines[0].time - 0.1).abs() < 1e-9);
        assert_eq!(lyrics(&conn, 99).unwrap(), None);
    }
}
