//! The effects workbench (PLAN.md X8): one file, chosen by the user, played
//! through the effects (X2) and recorded (X6) as one take.
//!
//! The file plays through the queue as a file opened from the Finder does
//! (F5, `queue::open_file`): after the current item, starting now, opened
//! off the main thread (H11), with the sandbox's access the open dialog or
//! the drop granted. Nothing here plays around the queue.
//!
//! A take records the file once with X6's recorder, unchanged: it waits
//! paused at the start (or the A–B loop's start), playback is set to stop
//! after the file, the recording starts, and the file plays. It ends by
//! itself when the file does (the engine stops, so the recording holds
//! exactly the file), or at the loop's end, timed from the position
//! reports to within about a block; or when another item becomes current,
//! or the recording stops any other way. Then the item playback stopped
//! after before is put back.
//!
//! Behind the `effects_workbench` switch; the effects and recording keep
//! their own switches, which the workbench never turns on.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Runtime};

use crate::anomp;
use crate::audio::{on_main, with_engine};
use crate::coded::coded;
use crate::queue::model::{QueueState, Uid};
use crate::recording::RecordingState;
use crate::settings;

/// How long before the loop's end a take's stop is timed: a little over two
/// of the engine's position reports (every 50 ms).
const LOOKAHEAD: f64 = 0.12;

/// A jump back larger than this between two position reports is the loop
/// going round (or a seek back), which ends a take at once.
const JUMP_BACK: f64 = 0.05;

/// The file the workbench plays: its queue item and what the page shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchFile {
    /// Its queue item.
    pub uid: Uid,
    /// A library track's id, or a negative id for a file outside it.
    pub track_id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub file_name: String,
    /// The file name's extension in capitals ("FLAC").
    pub format: String,
    pub duration: f64,
    pub sample_rate: u32,
    pub channels: u32,
    pub bitrate_kbps: Option<u32>,
}

/// A take being recorded. Main thread.
#[derive(Debug, Clone, PartialEq)]
struct Take {
    uid: Uid,
    /// The item playback stopped after before the take, put back after it.
    previous: Option<Uid>,
    /// The loop's end, if the take is of the loop.
    until: Option<f64>,
    /// The playing speed (practice mode's tempo).
    rate: f64,
    /// The last position reported.
    last: f64,
    /// Its end is on its way.
    ending: bool,
}

impl Take {
    fn new(uid: Uid, previous: Option<Uid>, start: f64, until: Option<f64>, rate: f64) -> Take {
        Take {
            uid,
            previous,
            until,
            rate: if rate > 0.0 { rate } else { 1.0 },
            last: start,
            ending: false,
        }
    }

    /// The engine reported `position`: for a take of the loop, the seconds
    /// to wait before stopping, once its end is near (or the playback
    /// jumped back past it); else `None`. Once only.
    fn due(&mut self, position: f64) -> Option<f64> {
        let until = self.until.filter(|_| !self.ending)?;
        let jumped_back = position + JUMP_BACK < self.last;
        self.last = position;
        let left = ((until - position) / self.rate).max(0.0);
        if jumped_back || left <= LOOKAHEAD {
            self.ending = true;
            return Some(if jumped_back { 0.0 } else { left });
        }
        None
    }

    /// Whether the take should end now: its item isn't current any more.
    fn left_behind(&self, current: Option<Uid>) -> bool {
        !self.ending && current != Some(self.uid)
    }
}

thread_local! {
    static TAKE: RefCell<Option<Take>> = const { RefCell::new(None) };
}

fn require<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if settings::current(app).features.effects_workbench {
        Ok(())
    } else {
        Err(crate::coded::feature_off(
            "effectsWorkbench",
            "Effects workbench",
        ))
    }
}

/// What the page shows of the file at `path`, queued as `item`.
fn describe(item: crate::queue::model::Item, path: &Path) -> Result<WorkbenchFile, String> {
    let tags = anomp::read_tags(path, false)?;
    Ok(WorkbenchFile {
        uid: item.uid,
        track_id: item.track.track_id,
        title: item.track.title,
        artist: item.track.artist,
        file_name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        format: path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_uppercase(),
        duration: item.track.duration,
        sample_rate: tags.sample_rate,
        channels: tags.channels,
        bitrate_kbps: tags.bitrate_kbps,
    })
}

/// Ends the take, if one runs: stops the recording (unless it stopped
/// already), pauses if asked, and puts back the item playback stopped
/// after. Any thread but the main one.
fn end<R: Runtime>(app: &AppHandle<R>, pause: bool) {
    let Ok(Some(take)) = on_main(app, || TAKE.with_borrow_mut(Option::take)) else {
        return;
    };
    // Already stopped if that's what ended the take.
    let _ = tauri::async_runtime::block_on(crate::recording::recording_stop(app.clone()));
    if let Err(error) = crate::queue::end_take(app, take.uid, take.previous, pause) {
        log::warn!("cannot end the take: {error}");
    }
    log::info!("take ended");
}

/// Ends the take after `wait` seconds, on a thread of its own.
fn end_after<R: Runtime>(app: &AppHandle<R>, wait: f64, pause: bool) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("anomp-workbench-take".into())
        .spawn(move || {
            if wait > 0.0 {
                std::thread::sleep(Duration::from_secs_f64(wait));
            }
            end(&app, pause);
        });
    if let Err(error) = spawned {
        log::warn!("cannot end the take: {error}");
    }
}

// ---- Hooks ------------------------------------------------------------------

/// The engine reported its position: a take of the loop stops at the
/// loop's end. Main thread.
pub fn position<R: Runtime>(app: &AppHandle<R>, position: f64) {
    if let Some(wait) = TAKE.with_borrow_mut(|take| take.as_mut()?.due(position)) {
        // At the loop's end the file would play on, so it pauses there.
        end_after(app, wait, true);
    }
}

/// The engine reported `TrackEnded`: a take of the whole file is over, as
/// playback stopped after it. Main thread.
pub fn track_ended<R: Runtime>(app: &AppHandle<R>) {
    let ending = TAKE.with_borrow_mut(|take| match take {
        Some(take) if !take.ending => {
            take.ending = true;
            true
        }
        _ => false,
    });
    if ending {
        end_after(app, 0.0, false);
    }
}

/// The queue changed: another item became current, which ends the take.
/// Main thread, inside the queue's turn, so the end waits for it.
pub fn queue_changed<R: Runtime>(app: &AppHandle<R>, state: &QueueState) {
    let current = state.current_item.as_ref().map(|item| item.uid);
    let ending = TAKE.with_borrow_mut(|take| match take {
        Some(take) if take.left_behind(current) => {
            take.ending = true;
            true
        }
        _ => false,
    });
    if ending {
        end_after(app, 0.0, false);
    }
}

/// A recording stopped (from the workbench, the now-playing bar, the menu,
/// an error, or the switch): a take it was ends. Main thread.
pub fn recording_finished<R: Runtime>(app: &AppHandle<R>) {
    if TAKE.with_borrow(Option::is_some) {
        end_after(app, 0.0, false);
    }
}

// ---- Commands ---------------------------------------------------------------

/// Plays the file at `path`, which the user just chose or dropped, as a
/// file opened from the Finder plays (F5), and describes it.
#[tauri::command]
pub async fn workbench_open<R: Runtime>(
    app: AppHandle<R>,
    path: PathBuf,
) -> Result<WorkbenchFile, String> {
    require(&app)?;
    let item = crate::queue::open_file(&app, path.clone()).await?;
    let file = tauri::async_runtime::spawn_blocking(move || describe(item, &path))
        .await
        .map_err(|e| e.to_string())??;
    log::info!("opened track {}", file.track_id);
    log::debug!("opened {}", file.file_name);
    Ok(file)
}

/// Describes queue item `item` from its file, found as playback finds it.
async fn described<R: Runtime>(
    app: &AppHandle<R>,
    item: crate::queue::model::Item,
) -> Result<WorkbenchFile, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let track_id = item.track.track_id;
        crate::queue::read_track_file(&app, track_id, |path| describe(item, path))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Takes the queue's current item, as it plays, as the workbench's file:
/// nothing is queued again.
#[tauri::command]
pub async fn workbench_current<R: Runtime>(app: AppHandle<R>) -> Result<WorkbenchFile, String> {
    require(&app)?;
    let item = crate::queue::queue_state(app.clone())?
        .current_item
        .ok_or_else(|| coded("workbenchNothingPlaying", &[], "Nothing is playing"))?;
    let file = described(&app, item).await?;
    log::info!("took track {} from the queue", file.track_id);
    Ok(file)
}

/// Plays track `track_id`, the workbench's file that has left the queue,
/// again after the current item, as `workbench_open` does, and describes it.
#[tauri::command]
pub async fn workbench_reopen<R: Runtime>(
    app: AppHandle<R>,
    track_id: i64,
) -> Result<WorkbenchFile, String> {
    require(&app)?;
    let item = crate::queue::open_track(&app, track_id).await?;
    let file = described(&app, item).await?;
    log::info!("opened track {} again", file.track_id);
    Ok(file)
}

/// Records one take of item `uid`, the workbench's file, which must be
/// current and open: from its start to its end, or once round the A–B
/// loop if one is set.
#[tauri::command]
pub async fn workbench_take<R: Runtime>(
    app: AppHandle<R>,
    uid: Uid,
) -> Result<RecordingState, String> {
    require(&app)?;
    crate::recording::require(&app)?;
    let (points, rate, running) = with_engine(&app, |engine| {
        (
            engine.loop_points(),
            engine.signal_path().tempo,
            crate::recording::running(),
        )
    })?;
    if running {
        return Err(coded("recordingRunning", &[], "Already recording"));
    }
    let (start, until) = match points {
        Some((start, end)) => (start, Some(end)),
        None => (0.0, None),
    };
    let previous = crate::queue::ready_take(&app, uid, start).map_err(|detail| {
        log::warn!("cannot ready a take: {detail}");
        coded(
            "workbenchNotPlaying",
            &[],
            "Play the file before recording it",
        )
    })?;
    let state = match crate::recording::recording_start(app.clone()).await {
        Ok(state) => state,
        Err(error) => {
            if let Err(detail) = crate::queue::end_take(&app, uid, previous, false) {
                log::warn!("cannot end the take: {detail}");
            }
            return Err(error);
        }
    };
    on_main(&app, move || {
        TAKE.with_borrow_mut(|slot| *slot = Some(Take::new(uid, previous, start, until, rate)));
    })?;
    crate::queue::play(&app)?;
    log::info!(
        "take started{}",
        if until.is_some() { " (loop)" } else { "" }
    );
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_whole_file_take_waits_for_the_file_to_end() {
        let mut take = Take::new(7, None, 0.0, None, 1.0);
        assert_eq!(take.due(10.0), None);
        assert_eq!(take.due(0.0), None, "a seek back doesn't end it");
        assert!(!take.left_behind(Some(7)));
        assert!(take.left_behind(Some(8)));
        assert!(take.left_behind(None));
    }

    #[test]
    fn a_loop_take_stops_at_the_loop_end_once() {
        let mut take = Take::new(7, Some(3), 10.0, Some(20.0), 1.0);
        assert_eq!(take.due(10.05), None);
        assert_eq!(take.due(19.5), None);
        let wait = take.due(19.95).expect("due at the end");
        assert!((wait - 0.05).abs() < 1e-9);
        assert_eq!(take.due(19.99), None, "once");
        assert!(!take.left_behind(Some(8)), "already ending");
    }

    #[test]
    fn a_loop_take_allows_for_the_tempo() {
        // At half speed 0.1 s of the track takes 0.2 s.
        let mut take = Take::new(7, None, 10.0, Some(20.0), 0.5);
        assert_eq!(take.due(19.9), None);
        let wait = take.due(19.95).expect("due");
        assert!((wait - 0.1).abs() < 1e-9);
        // A rate that isn't one plays at 1.
        assert_eq!(Take::new(7, None, 0.0, None, 0.0).rate, 1.0);
    }

    #[test]
    fn a_loop_take_ends_at_once_when_playback_jumps_back() {
        let mut take = Take::new(7, None, 10.0, Some(20.0), 1.0);
        assert_eq!(take.due(15.0), None);
        assert_eq!(take.due(10.02), Some(0.0), "round the loop, or a seek back");
    }

    #[test]
    fn describes_the_file() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../core/tests/fixtures/tagged-id3v23.mp3");
        let track = crate::library::external::register(&fixture).unwrap();
        let item = crate::queue::model::Item { uid: 4, track };
        let file = describe(item, &fixture).unwrap();
        assert_eq!(file.uid, 4);
        assert!(file.track_id < 0);
        assert_eq!(file.title, "Café Déjà Vu");
        assert_eq!(file.file_name, "tagged-id3v23.mp3");
        assert_eq!(file.format, "MP3");
        assert!(file.duration > 0.0);
        assert!(file.sample_rate > 0);
        assert!(file.channels > 0);
    }
}
