//! The user's own rules for playing a track or an album (PLAN.md O7), kept
//! in the library, never in the files. A track's rule takes over from its
//! album's; `playback` applies the gain offset and trims, the queue the
//! skips and "never shuffle" (`queue::track_infos`).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::Error;

/// The largest gain offset either way, in dB.
pub const MAX_OFFSET_DB: f64 = 15.0;

/// A track's rules; `None` leaves each to its album, or the default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
// Every field may be left out (`default`), so each is optional in TS.
#[cfg_attr(test, ts(optional_fields = nullable))]
#[serde(rename_all = "camelCase", default)]
pub struct TrackPrefs {
    /// Passed over in album and shuffle play; it still plays when chosen.
    pub skip: Option<bool>,
    /// dB added to its ReplayGain.
    pub gain_offset: Option<f64>,
    /// Seconds cut off the start and the end.
    pub trim_start: Option<f64>,
    pub trim_end: Option<f64>,
}

/// An album's rules, for all of its tracks.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
// Every field may be left out (`default`), so each is optional in TS.
#[cfg_attr(test, ts(optional_fields = nullable))]
#[serde(rename_all = "camelCase", default)]
pub struct AlbumPrefs {
    pub skip: Option<bool>,
    /// Shuffle plays it whole, in order.
    pub never_shuffle: Option<bool>,
    pub gain_offset: Option<f64>,
}

fn check_offset(offset: Option<f64>) -> Result<(), Error> {
    if offset.is_some_and(|db| !(-MAX_OFFSET_DB..=MAX_OFFSET_DB).contains(&db)) {
        return Err(Error::Invalid(format!(
            "The gain offset must be between -{MAX_OFFSET_DB} and {MAX_OFFSET_DB} dB"
        )));
    }
    Ok(())
}

pub fn track_prefs(conn: &Connection, track_id: i64) -> Result<TrackPrefs, Error> {
    Ok(conn
        .query_row(
            "SELECT skip, gain_offset, trim_start, trim_end FROM track_prefs WHERE track_id = ?1",
            [track_id],
            |row| {
                Ok(TrackPrefs {
                    skip: row.get::<_, Option<i64>>(0)?.map(|skip| skip != 0),
                    gain_offset: row.get(1)?,
                    trim_start: row.get(2)?,
                    trim_end: row.get(3)?,
                })
            },
        )
        .optional()?
        .unwrap_or_default())
}

/// Stores a track's rules (removing the row when none is set). Trims must
/// leave at least a second of the track.
pub fn set_track_prefs(conn: &Connection, track_id: i64, prefs: &TrackPrefs) -> Result<(), Error> {
    check_offset(prefs.gain_offset)?;
    let duration: Option<f64> = conn
        .query_row(
            "SELECT duration FROM tracks WHERE id = ?1",
            [track_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(duration) = duration else {
        return Err(Error::Invalid(crate::coded::gone(
            crate::coded::Gone::Track,
        )));
    };
    let trims = [prefs.trim_start, prefs.trim_end];
    if trims
        .iter()
        .flatten()
        .any(|&trim| !(trim.is_finite() && trim >= 0.0))
    {
        return Err(Error::Invalid("A trim must be 0 seconds or more".into()));
    }
    if trims.iter().flatten().sum::<f64>() > duration - 1.0 {
        return Err(Error::Invalid(
            "The trims must leave at least a second of the track".into(),
        ));
    }
    if *prefs == TrackPrefs::default() {
        conn.execute("DELETE FROM track_prefs WHERE track_id = ?1", [track_id])?;
    } else {
        conn.execute(
            "INSERT OR REPLACE INTO track_prefs (track_id, skip, gain_offset, trim_start, trim_end)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                track_id,
                prefs.skip.map(i64::from),
                prefs.gain_offset,
                prefs.trim_start,
                prefs.trim_end
            ],
        )?;
    }
    Ok(())
}

pub fn album_prefs(conn: &Connection, album_id: i64) -> Result<AlbumPrefs, Error> {
    Ok(conn
        .query_row(
            "SELECT skip, never_shuffle, gain_offset FROM album_prefs WHERE album_id = ?1",
            [album_id],
            |row| {
                Ok(AlbumPrefs {
                    skip: row.get::<_, Option<i64>>(0)?.map(|skip| skip != 0),
                    never_shuffle: row.get::<_, Option<i64>>(1)?.map(|on| on != 0),
                    gain_offset: row.get(2)?,
                })
            },
        )
        .optional()?
        .unwrap_or_default())
}

pub fn set_album_prefs(conn: &Connection, album_id: i64, prefs: &AlbumPrefs) -> Result<(), Error> {
    check_offset(prefs.gain_offset)?;
    let exists: bool = conn
        .query_row("SELECT 1 FROM albums WHERE id = ?1", [album_id], |_| Ok(()))
        .optional()?
        .is_some();
    if !exists {
        return Err(Error::Invalid(crate::coded::gone(
            crate::coded::Gone::Album,
        )));
    }
    if *prefs == AlbumPrefs::default() {
        conn.execute("DELETE FROM album_prefs WHERE album_id = ?1", [album_id])?;
    } else {
        conn.execute(
            "INSERT OR REPLACE INTO album_prefs (album_id, skip, never_shuffle, gain_offset)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                album_id,
                prefs.skip.map(i64::from),
                prefs.never_shuffle.map(i64::from),
                prefs.gain_offset
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};

    #[test]
    fn stores_and_clears_track_and_album_prefs() {
        let library = Library::new([track("a.flac").album("A")]);
        let conn = &library.conn;
        assert_eq!(track_prefs(conn, 1).unwrap(), TrackPrefs::default());

        let prefs = TrackPrefs {
            skip: Some(true),
            gain_offset: Some(-3.0),
            trim_start: Some(5.0),
            trim_end: None,
        };
        set_track_prefs(conn, 1, &prefs).unwrap();
        assert_eq!(track_prefs(conn, 1).unwrap(), prefs);
        set_track_prefs(conn, 1, &TrackPrefs::default()).unwrap();
        let rows: i64 = conn
            .query_row("SELECT count(*) FROM track_prefs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0);

        let album = AlbumPrefs {
            never_shuffle: Some(true),
            ..AlbumPrefs::default()
        };
        set_album_prefs(conn, 1, &album).unwrap();
        assert_eq!(album_prefs(conn, 1).unwrap(), album);
    }

    #[test]
    fn refuses_bad_prefs() {
        let library = Library::new([track("a.flac").album("A")]);
        let conn = &library.conn;
        let error = |prefs: TrackPrefs| set_track_prefs(conn, 1, &prefs).unwrap_err().to_string();
        assert!(error(TrackPrefs {
            gain_offset: Some(20.0),
            ..TrackPrefs::default()
        })
        .contains("gain offset"));
        assert!(error(TrackPrefs {
            trim_start: Some(-1.0),
            ..TrackPrefs::default()
        })
        .contains("0 seconds"));
        // The test tracks are 60 s long.
        assert!(error(TrackPrefs {
            trim_start: Some(30.0),
            trim_end: Some(29.5),
            ..TrackPrefs::default()
        })
        .contains("at least a second"));
        assert!(set_track_prefs(conn, 99, &TrackPrefs::default()).is_err());
        assert!(set_album_prefs(conn, 99, &AlbumPrefs::default()).is_err());
    }
}
