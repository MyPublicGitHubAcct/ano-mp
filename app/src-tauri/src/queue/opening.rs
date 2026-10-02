//! Opening the queue's tracks off the main thread (PLAN.md H11).
//!
//! Opening a file can take seconds: a disk waking, a network share, a
//! cloud file downloading. So `EnginePlayer::load` and `set_next` only ask
//! here, and the queue waits for `Queue::load_finished`:
//!
//! 1. On a blocking thread, the track's row is read and its folder's
//!    bookmark resolved (`access::open_folder_of`); either can wait on a
//!    disk.
//! 2. Back on the main thread, the engine opens the file on a thread of
//!    its own (`Engine::load_track_async`), holding the folder open, as the
//!    sandbox needs, until the engine reports the request.
//! 3. The engine's `LoadFinished` (main thread) goes to the queue, with the
//!    request the queue was given.
//!
//! A request the queue gives up on (`cancel`: it moved on, or timed out)
//! is cancelled wherever it is, and never reported to it.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use tauri::{AppHandle, Manager, Runtime};

use super::model::Request;
use crate::anomp::{Engine, LoadRequest, LoadResult, TrackOptions};
use crate::audio;
use crate::library::access::OpenFolder;
use crate::library::commands::LibraryState;
use crate::library::external;
use crate::library::playback::TrackPlay;
use crate::settings::{self, FeatureSettings, PlaybackSettings};

thread_local! {
    static REQUESTS: RefCell<Requests> = RefCell::new(Requests::default());
}

/// The queue's requests not yet reported to it. Main thread.
#[derive(Default)]
struct Requests {
    last: Request,
    /// Being resolved, not yet given to the engine.
    resolving: HashSet<Request>,
    /// Given to the engine, by the engine's id.
    in_engine: HashMap<LoadRequest, InEngine>,
}

struct InEngine {
    request: Request,
    /// A current track (not a next one), and the rate to set the device to
    /// as it takes over (O10), if any.
    current: bool,
    sample_rate: Option<u32>,
    /// The file's folder, held open while the engine opens it.
    _folder: Option<OpenFolder>,
}

/// What to open.
pub struct Ask {
    pub track_id: i64,
    /// As the next track, crossfading into it over `crossfade` seconds.
    pub next: bool,
    pub crossfade: f64,
    pub playback: PlaybackSettings,
    pub features: FeatureSettings,
}

/// How to play track `track_id`: a library track as `library::playback`
/// says, or a file opened from outside the library, whole.
pub fn track_play(
    library: &LibraryState,
    track_id: i64,
    features: &FeatureSettings,
) -> Result<TrackPlay, String> {
    if track_id < 0 {
        let file = external::file(track_id).ok_or("The file is no longer open to the app")?;
        return Ok(TrackPlay {
            path: file.path,
            start: 0.0,
            end: None,
            replay_gain: file.replay_gain,
            gain_offset_db: 0.0,
            skip: None,
            sample_rate: file.sample_rate,
        });
    }
    library.track_play(track_id, features)
}

/// Starts opening a track for the queue and returns its request, which
/// `queue::load_finished` reports. Main thread.
pub fn ask<R: Runtime>(app: &AppHandle<R>, ask: Ask) -> Request {
    let request = REQUESTS.with_borrow_mut(|requests| {
        requests.last += 1;
        requests.resolving.insert(requests.last);
        requests.last
    });
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let resolved = resolve(&app, ask.track_id, &ask.features);
        let main = app.clone();
        let posted = app.run_on_main_thread(move || give_to_engine(&main, request, &ask, resolved));
        if let Err(error) = posted {
            log::warn!("cannot open a track: {error}");
        }
    });
    request
}

/// A track resolved: how to play it, its folder (open), and whether its
/// file is a cloud placeholder, which opening downloads (H12).
struct Resolved {
    play: TrackPlay,
    folder: Option<OpenFolder>,
    dataless: bool,
}

/// The track's row and its folder, opened. Blocking.
fn resolve<R: Runtime>(
    app: &AppHandle<R>,
    track_id: i64,
    features: &FeatureSettings,
) -> Result<Resolved, String> {
    let library = app
        .try_state::<LibraryState>()
        .ok_or("The library is not available")?;
    let play = track_play(&library, track_id, features)?;
    let folder = library.open_folder_of(&play.path)?;
    let dataless = crate::anomp::file_is_dataless(&play.path) == Some(true);
    Ok(Resolved {
        play,
        folder,
        dataless,
    })
}

/// Hands a resolved request to the engine, unless the queue gave up on it
/// meanwhile. Main thread.
fn give_to_engine<R: Runtime>(
    app: &AppHandle<R>,
    request: Request,
    ask: &Ask,
    resolved: Result<Resolved, String>,
) {
    if !REQUESTS.with_borrow_mut(|requests| requests.resolving.remove(&request)) {
        return; // Dropping `resolved` lets go of the folder.
    }
    let dataless = resolved.as_ref().is_ok_and(|resolved| resolved.dataless);
    let result = resolved.and_then(|Resolved { play, folder, .. }| {
        let options = TrackOptions {
            crossfade: ask.crossfade,
            ..play.options(&ask.playback)
        };
        let id = audio::engine_mut(|engine| {
            if ask.next {
                engine.set_next_track_async(&play.path, &options)
            } else {
                engine.load_track_async(&play.path, &options)
            }
        })??;
        let sample_rate = (!ask.next && ask.features.match_sample_rate && play.sample_rate > 0)
            .then_some(play.sample_rate);
        let entry = InEngine {
            request,
            current: !ask.next,
            sample_rate,
            _folder: folder,
        };
        REQUESTS.with_borrow_mut(|requests| requests.in_engine.insert(id, entry));
        Ok(())
    });
    match result {
        Ok(()) if dataless => super::downloading(app, request),
        Ok(()) => {}
        Err(error) => super::load_finished(app, request, Err(error)),
    }
}

/// Gives up on a request: it's cancelled wherever it is and never reported
/// to the queue. Main thread.
pub fn cancel(engine: &mut Engine, request: Request) {
    let in_engine = REQUESTS.with_borrow_mut(|requests| {
        if requests.resolving.remove(&request) {
            return None;
        }
        requests
            .in_engine
            .iter()
            .find(|(_, entry)| entry.request == request)
            .map(|(&id, _)| id)
    });
    // Its folder is let go of when the engine reports it cancelled.
    if let Some(id) = in_engine {
        engine.cancel_load(id);
    }
}

/// The engine reported a request (`Event::LoadFinished`). Main thread.
pub fn engine_finished<R: Runtime>(app: &AppHandle<R>, id: LoadRequest, result: LoadResult) {
    let Some(entry) = REQUESTS.with_borrow_mut(|requests| requests.in_engine.remove(&id)) else {
        return;
    };
    let result = match result {
        LoadResult::Loaded => Ok(()),
        LoadResult::Failed(error) => Err(error),
        LoadResult::Cancelled => return,
    };
    if result.is_ok() && entry.current {
        // As it takes over, not before: switching the rate interrupts output.
        let features = settings::current(app).features;
        let _ = audio::engine_mut(|engine| {
            if let Some(rate) = entry.sample_rate {
                engine.set_device_sample_rate(f64::from(rate));
            }
            // Headphones may have been plugged in since the last track.
            audio::apply_crossfeed(engine, &features);
        });
    }
    let request = entry.request;
    drop(entry);
    super::load_finished(app, request, result);
}
