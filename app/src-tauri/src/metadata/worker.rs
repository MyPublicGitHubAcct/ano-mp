//! The metadata worker's thread in the app: it owns the HTTP client and a
//! database connection of its own, and runs `jobs::Worker` (PLAN.md Phase
//! 4.5). Other threads only queue requests (`Shared`), which never waits on
//! the database or the network, so it's safe from the main thread.
//!
//! Background enrichment is queued at launch (for a run cut short by
//! quitting, and albums due for another try), after each scan, and after
//! the settings are saved; the worker checks "match automatically" when it
//! gets to it. The album playing is queued ahead of that, and the artist
//! page being looked at ahead of both.
//!
//! On exit the worker is told to stop but not waited for: a request can
//! take up to its 30 s timeout, and nothing it writes is left half-done
//! (SQLite transactions, and the image cache's rename into place).

use std::sync::{mpsc, Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::http::{Client, SystemClock, UreqTransport};
use super::jobs::{
    Answer, CallContext, Host, Job, MetadataChanged, Priority, Progress, Shared, Worker,
};
use super::Error;
use crate::library::art::ArtKey;
use crate::library::commands::LibraryState;
use crate::library::db;

/// Frontend event with a `MetadataChanged` payload.
pub const METADATA_CHANGED_EVENT: &str = "metadata-changed";

/// Frontend event with a `Progress` payload.
pub const METADATA_PROGRESS_EVENT: &str = "metadata-progress";

pub struct MetadataWorker {
    shared: Arc<Shared>,
    /// The album last queued as playing, so a queue change that keeps it
    /// doesn't queue it again.
    playing: Mutex<Option<i64>>,
}

struct TauriHost<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> Host for TauriHost<R> {
    fn art_changed(&mut self, album_id: i64) {
        if let Some(library) = self.app.try_state::<LibraryState>() {
            library.art.remove(ArtKey::Album(album_id));
        }
    }

    fn metadata_changed(&mut self, changed: &MetadataChanged) {
        let _ = self.app.emit(METADATA_CHANGED_EVENT, changed);
        crate::media::art_changed(&self.app, &changed.albums);
    }

    fn progress(&mut self, progress: &Progress) {
        let _ = self.app.emit(METADATA_PROGRESS_EVENT, progress);
    }
}

/// Starts the worker and queues background enrichment. Call after the
/// library has started.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let library = app
        .try_state::<LibraryState>()
        .ok_or("The library is not available")?;
    let db_path = library.db_path().to_path_buf();
    let images = library.images.clone();
    let shared = Arc::new(Shared::default());
    let host = TauriHost { app: app.clone() };
    let worker_shared = shared.clone();
    std::thread::Builder::new()
        .name("metadata".into())
        .spawn(move || {
            let conn = match db::open(&db_path) {
                Ok(conn) => conn,
                Err(error) => {
                    eprintln!("[metadata] {error}");
                    worker_shared.stop();
                    return;
                }
            };
            let client = Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock));
            Worker::new(
                worker_shared,
                client,
                Box::new(SystemClock),
                conn,
                images,
                host,
            )
            .run();
        })
        .map_err(|e| format!("Cannot start the metadata worker: {e}"))?;
    shared.enrich_library();
    app.manage(MetadataWorker {
        shared,
        playing: Mutex::new(None),
    });
    Ok(())
}

/// Tells the worker to stop, without waiting for it.
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    if let Some(worker) = app.try_state::<MetadataWorker>() {
        worker.shared.stop();
    }
}

fn with_worker<R: Runtime, T>(
    app: &AppHandle<R>,
    f: impl FnOnce(&MetadataWorker) -> T,
) -> Option<T> {
    app.try_state::<MetadataWorker>().map(|worker| f(&worker))
}

/// Queues background enrichment of the library.
pub fn enrich_library<R: Runtime>(app: &AppHandle<R>) {
    with_worker(app, |worker| worker.shared.enrich_library());
}

/// The current item's album changed (or playback stopped, with `None`).
pub fn playing<R: Runtime>(app: &AppHandle<R>, album_id: Option<i64>) {
    with_worker(app, |worker| {
        let mut playing = worker.playing.lock().unwrap_or_else(|e| e.into_inner());
        if *playing != album_id {
            *playing = album_id;
            if let Some(album_id) = album_id {
                worker
                    .shared
                    .request(Job::Match(album_id), Priority::Playing, None);
            }
        }
    });
}

/// The artist page for `artist_id` is shown: look the artist up if it
/// needs it, ahead of automatic work.
pub fn viewing_artist<R: Runtime>(app: &AppHandle<R>, artist_id: i64) {
    with_worker(app, |worker| {
        worker
            .shared
            .request(Job::Artist(artist_id), Priority::Viewing, None)
    });
}

/// Matches album `album_id` and fetches its cover, as far as each is
/// needed, ahead of other work; waits for the answer. Blocks, so call it on
/// a blocking thread.
pub fn update_album<R: Runtime>(app: &AppHandle<R>, album_id: i64) -> Result<(), Error> {
    request_and_wait(app, Job::Match(album_id))
}

/// Matches artist `artist_id` and fetches its biography, as `update_album`
/// does for albums.
pub fn update_artist<R: Runtime>(app: &AppHandle<R>, artist_id: i64) -> Result<(), Error> {
    request_and_wait(app, Job::Artist(artist_id))
}

fn request_and_wait<R: Runtime>(app: &AppHandle<R>, job: Job) -> Result<(), Error> {
    let (reply, answer) = mpsc::channel();
    with_worker(app, |worker| {
        worker.shared.request(job, Priority::User, Some(reply))
    })
    .ok_or_else(|| Error::Invalid("The metadata worker is not running".into()))?;
    answer
        .recv()
        .map_err(|_| Error::Invalid("The metadata worker stopped".into()))?
}

/// Queues `job` as the user's request without waiting for it: what it
/// changes arrives in `metadata-changed`.
pub fn request<R: Runtime>(app: &AppHandle<R>, job: Job) {
    with_worker(app, |worker| {
        worker.shared.request(job, Priority::User, None)
    });
}

/// Runs `f` on the worker, with its HTTP client and connection, ahead of
/// every job, and waits for its result; what it reports as changed is sent
/// in `metadata-changed` first. Blocks, so call it on a blocking thread.
pub fn call<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    f: impl FnOnce(&mut CallContext) -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    let (reply, answer) = mpsc::channel();
    with_worker(app, |worker| {
        worker.shared.call(Box::new(move |context| {
            let result = f(context);
            Box::new(move || {
                let _ = reply.send(result);
            }) as Answer
        }))
    })
    .ok_or_else(|| Error::Invalid("The metadata worker is not running".into()))?;
    answer
        .recv()
        .map_err(|_| Error::Invalid("The metadata worker stopped".into()))?
}

/// Reports changes made outside the worker (the user's picks) as the
/// worker reports its own: the albums in `art` lose their cached art, and
/// `changed` goes out in `metadata-changed`.
pub fn report_changes<R: Runtime>(app: &AppHandle<R>, art: &[i64], changed: MetadataChanged) {
    let mut host = TauriHost { app: app.clone() };
    for &album_id in art {
        host.art_changed(album_id);
    }
    host.metadata_changed(&changed);
}

pub fn retry_now<R: Runtime>(app: &AppHandle<R>) {
    with_worker(app, |worker| worker.shared.retry_now());
}

pub fn progress<R: Runtime>(app: &AppHandle<R>) -> Progress {
    with_worker(app, |worker| worker.shared.progress()).unwrap_or_default()
}
