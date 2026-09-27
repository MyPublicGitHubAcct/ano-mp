//! How the engine plays a library track: which file, which part of it, and
//! with what gain. Brings together the track's part of its file (a cue
//! sheet's track, O5), the user's trims and gain offset (O7), ReplayGain
//! from the tags or else from the loudness analysis (O1), and the long
//! silences to skip (O3), as far as the feature settings allow.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

use super::{track_path, Error};
use crate::anomp::{ReplayGain, TrackOptions, MAX_TRACK_GAIN};
use crate::settings::{FeatureSettings, PlaybackSettings};

/// ReplayGain's reference loudness (ReplayGain 2.0), in LUFS.
pub const REFERENCE_LUFS: f64 = -18.0;

/// A long silence is skipped to this many seconds before it ends, so the
/// hidden track after it doesn't start abruptly.
const SKIP_LEAD_OUT: f64 = 0.5;

/// What the engine needs to play a track.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackPlay {
    pub path: PathBuf,
    /// Seconds into the file, after trims.
    pub start: f64,
    /// `None`: the end of the file.
    pub end: Option<f64>,
    /// The tags' ReplayGain, filled in from the analysis where they have
    /// none (and the analysis is on).
    pub replay_gain: ReplayGain,
    /// dB the user added to the track (or its album).
    pub gain_offset_db: f64,
    /// Seconds within the track: a long silence to jump over.
    pub skip: Option<(f64, f64)>,
    /// Hz, from the tags.
    pub sample_rate: u32,
}

impl TrackPlay {
    /// The engine's options under `playback`.
    pub fn options(&self, playback: &PlaybackSettings) -> TrackOptions {
        let gain = playback.gain(&self.replay_gain) * 10f64.powf(self.gain_offset_db / 20.0);
        TrackOptions {
            gain: gain.clamp(0.0, MAX_TRACK_GAIN),
            start: self.start,
            end: self.end,
            skip: self.skip,
        }
    }
}

/// The track `track_id` as `features` would have it played; `None` if there
/// is no such track.
pub fn track_play(
    conn: &Connection,
    track_id: i64,
    features: &FeatureSettings,
) -> Result<Option<TrackPlay>, Error> {
    let row = conn
        .query_row(
            "SELECT f.path, t.relative_path, t.range_start, t.range_end, t.duration, t.sample_rate,
                    t.replaygain_track_gain, t.replaygain_track_peak, t.replaygain_album_gain,
                    t.replaygain_album_peak,
                    a.loudness, a.true_peak, a.duration, a.trailing_silence, a.gap_start,
                    a.gap_length, aa.loudness, aa.peak,
                    tp.skip, tp.gain_offset, tp.trim_start, tp.trim_end, ap.gain_offset
             FROM tracks t
             JOIN folders f ON f.id = t.folder_id
             LEFT JOIN track_analysis a ON a.track_id = t.id AND a.error IS NULL
                 AND a.file_size = t.file_size AND a.file_mtime_ns = t.file_mtime_ns
             LEFT JOIN album_analysis aa ON aa.album_id = t.album_id
             LEFT JOIN track_prefs tp ON tp.track_id = t.id
             LEFT JOIN album_prefs ap ON ap.album_id = t.album_id
             WHERE t.id = ?1",
            [track_id],
            |row| {
                Ok(Row {
                    folder: row.get(0)?,
                    relative: row.get(1)?,
                    start: row.get(2)?,
                    end: row.get(3)?,
                    duration: row.get(4)?,
                    sample_rate: row.get(5)?,
                    tags: ReplayGain {
                        track_gain: row.get(6)?,
                        track_peak: row.get(7)?,
                        album_gain: row.get(8)?,
                        album_peak: row.get(9)?,
                    },
                    loudness: row.get(10)?,
                    peak: row.get(11)?,
                    analysed_duration: row.get(12)?,
                    trailing_silence: row.get(13)?,
                    gap_start: row.get(14)?,
                    gap_length: row.get(15)?,
                    album_loudness: row.get(16)?,
                    album_peak: row.get(17)?,
                    trim_start: row.get(20)?,
                    trim_end: row.get(21)?,
                    track_offset: row.get(19)?,
                    album_offset: row.get(22)?,
                })
            },
        )
        .optional()?;
    Ok(row.map(|row| row.into_play(features)))
}

struct Row {
    folder: String,
    relative: String,
    start: f64,
    end: Option<f64>,
    duration: f64,
    sample_rate: u32,
    tags: ReplayGain,
    loudness: Option<f64>,
    peak: Option<f64>,
    analysed_duration: Option<f64>,
    trailing_silence: Option<f64>,
    gap_start: Option<f64>,
    gap_length: Option<f64>,
    album_loudness: Option<f64>,
    album_peak: Option<f64>,
    trim_start: Option<f64>,
    trim_end: Option<f64>,
    track_offset: Option<f64>,
    album_offset: Option<f64>,
}

impl Row {
    fn into_play(self, features: &FeatureSettings) -> TrackPlay {
        let mut replay_gain = self.tags;
        if features.loudness_analysis {
            // The tags first; the analysis only where they say nothing.
            if replay_gain.track_gain.is_none() {
                if let Some(loudness) = self.loudness {
                    replay_gain.track_gain = Some(REFERENCE_LUFS - loudness);
                    replay_gain.track_peak = self.peak;
                }
            }
            if replay_gain.album_gain.is_none() {
                if let Some(loudness) = self.album_loudness {
                    replay_gain.album_gain = Some(REFERENCE_LUFS - loudness);
                    replay_gain.album_peak = self.album_peak;
                }
            }
        }

        // The track's length as played; the analysis measured it exactly.
        let length = self.analysed_duration.unwrap_or(self.duration).max(0.0);
        let mut start = self.start;
        let mut length_left = length;
        let mut skip = None;
        let mut gain_offset_db = 0.0;

        if features.playback_preferences {
            gain_offset_db = self.track_offset.or(self.album_offset).unwrap_or(0.0);
            let trim_start = self.trim_start.unwrap_or(0.0).clamp(0.0, length);
            let trim_end = self.trim_end.unwrap_or(0.0).clamp(0.0, length - trim_start);
            start += trim_start;
            length_left = length - trim_start - trim_end;
        }
        let trimmed_start = start - self.start;

        if features.skip_silence {
            let after = f64::from(features.skip_silence_after);
            // Silence to the end: stop `after` seconds into it.
            if let Some(trailing) = self.trailing_silence.filter(|&s| s > after) {
                length_left = length_left.min(length - trailing + after - trimmed_start);
            }
            // A long gap inside (before a hidden track): jump most of it.
            if let (Some(gap_start), Some(gap_length)) = (self.gap_start, self.gap_length) {
                let from = gap_start + after - trimmed_start;
                let to = gap_start + gap_length - SKIP_LEAD_OUT - trimmed_start;
                if to - from > 1.0 && from > 0.0 {
                    skip = Some((from, to));
                }
            }
        }

        let trimmed = length_left < length - trimmed_start - 1e-6;
        let end = if trimmed {
            Some(start + length_left.max(0.0))
        } else {
            self.end
        };
        TrackPlay {
            path: track_path(Path::new(&self.folder), &self.relative),
            start,
            end,
            replay_gain,
            gain_offset_db,
            skip,
            sample_rate: self.sample_rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};

    fn library() -> (Library, i64) {
        let library = Library::new([track("a/1.flac").title("One").album("X")]);
        let id = library
            .conn
            .query_row("SELECT id FROM tracks", [], |row| row.get(0))
            .unwrap();
        library
            .conn
            .execute(
                "UPDATE tracks SET duration = 300.0, sample_rate = 96000",
                [],
            )
            .unwrap();
        (library, id)
    }

    fn analyse(library: &Library, id: i64, extra: &str) {
        library
            .conn
            .execute(
                "INSERT INTO track_analysis (track_id, file_size, file_mtime_ns, analysed_at,
                                             duration, loudness, true_peak, trailing_silence,
                                             gap_start, gap_length)
                 VALUES (?1, 0, 0, 0, 300.0, -10.0, 0.9, 0.0, NULL, NULL)",
                [id],
            )
            .unwrap();
        if !extra.is_empty() {
            library.conn.execute_batch(extra).unwrap();
        }
    }

    #[test]
    fn plays_the_whole_file_by_default() {
        let (library, id) = library();
        let play = track_play(&library.conn, id, &FeatureSettings::default())
            .unwrap()
            .unwrap();
        assert_eq!(play.path, Path::new("/Music/a/1.flac"));
        assert_eq!((play.start, play.end, play.skip), (0.0, None, None));
        assert_eq!(play.sample_rate, 96000);
        assert_eq!(play.replay_gain, ReplayGain::default());
        assert_eq!(
            track_play(&library.conn, 999, &FeatureSettings::default()).unwrap(),
            None
        );
    }

    #[test]
    fn fills_replay_gain_from_the_analysis_when_it_is_on() {
        let (library, id) = library();
        analyse(
            &library,
            id,
            "INSERT INTO album_analysis (album_id, loudness, peak, tracks) VALUES (1, -12.0, 0.95, 1)",
        );
        let mut features = FeatureSettings::default();
        let off = track_play(&library.conn, id, &features).unwrap().unwrap();
        assert_eq!(off.replay_gain, ReplayGain::default());

        features.loudness_analysis = true;
        let on = track_play(&library.conn, id, &features).unwrap().unwrap();
        assert_eq!(on.replay_gain.track_gain, Some(-8.0));
        assert_eq!(on.replay_gain.track_peak, Some(0.9));
        assert_eq!(on.replay_gain.album_gain, Some(-6.0));
        assert_eq!(on.replay_gain.album_peak, Some(0.95));

        // Tags win.
        library
            .conn
            .execute("UPDATE tracks SET replaygain_track_gain = -3.0", [])
            .unwrap();
        let tagged = track_play(&library.conn, id, &features).unwrap().unwrap();
        assert_eq!(tagged.replay_gain.track_gain, Some(-3.0));
        assert_eq!(tagged.replay_gain.track_peak, None);

        // An analysis of the file as it was before it changed doesn't count.
        library
            .conn
            .execute(
                "UPDATE tracks SET file_size = 5, replaygain_track_gain = NULL",
                [],
            )
            .unwrap();
        let stale = track_play(&library.conn, id, &features).unwrap().unwrap();
        assert_eq!(stale.replay_gain.track_gain, None);
    }

    #[test]
    fn applies_trims_and_gain_offsets() {
        let (library, id) = library();
        library
            .conn
            .execute_batch(&format!(
                "INSERT INTO album_prefs (album_id, gain_offset) VALUES (1, -2.0);
                 INSERT INTO track_prefs (track_id, trim_start, trim_end) VALUES ({id}, 10.0, 20.0);"
            ))
            .unwrap();
        let mut features = FeatureSettings::default();
        let play = track_play(&library.conn, id, &features).unwrap().unwrap();
        assert_eq!((play.start, play.end), (10.0, Some(280.0)));
        assert_eq!(play.gain_offset_db, -2.0);
        let options = play.options(&PlaybackSettings::default());
        assert!((options.gain - 10f64.powf(-0.1)).abs() < 1e-9);

        // The track's own offset wins over the album's.
        library
            .conn
            .execute("UPDATE track_prefs SET gain_offset = 3.0", [])
            .unwrap();
        assert_eq!(
            track_play(&library.conn, id, &features)
                .unwrap()
                .unwrap()
                .gain_offset_db,
            3.0
        );

        features.playback_preferences = false;
        let off = track_play(&library.conn, id, &features).unwrap().unwrap();
        assert_eq!((off.start, off.end, off.gain_offset_db), (0.0, None, 0.0));
    }

    #[test]
    fn skips_long_silences_when_asked() {
        let (library, id) = library();
        analyse(
            &library,
            id,
            "UPDATE track_analysis SET trailing_silence = 60.0, gap_start = 100.0, gap_length = 40.0",
        );
        let mut features = FeatureSettings {
            skip_silence: true,
            skip_silence_after: 5,
            ..FeatureSettings::default()
        };
        let play = track_play(&library.conn, id, &features).unwrap().unwrap();
        // 60 s of silence at the end: stop 5 s into it.
        assert_eq!(play.end, Some(245.0));
        // A 40 s gap from 100 s: jump from 105 s to half a second before it ends.
        assert_eq!(play.skip, Some((105.0, 139.5)));

        // Within a trimmed track, the jump is relative to the new start.
        library
            .conn
            .execute(
                &format!("INSERT INTO track_prefs (track_id, trim_start) VALUES ({id}, 50.0)"),
                [],
            )
            .unwrap();
        let trimmed = track_play(&library.conn, id, &features).unwrap().unwrap();
        assert_eq!(trimmed.start, 50.0);
        assert_eq!(trimmed.skip, Some((55.0, 89.5)));
        assert_eq!(trimmed.end, Some(245.0));

        features.skip_silence = false;
        let off = track_play(&library.conn, id, &features).unwrap().unwrap();
        assert_eq!(off.skip, None);
    }
}
