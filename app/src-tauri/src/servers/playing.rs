//! Telling a server what plays (PLAN.md Phases 7e and 7f), so its play counts,
//! "recently played" and other clients' "now playing" stay current: a
//! server track that starts is reported as playing, and one that ends or
//! is left as stopped, with how far it got (the server counts a play by its
//! own rule). A switch (`ServerSettings::report_plays`), on by default
//! since the server is the user's own. Local history, ratings and
//! favourites stay local (the owner's decision, 2026-10-11: one way).
//!
//! The follower runs on the main thread, where the queue and the engine's
//! position arrive, and hands reports to a thread of its own, which looks
//! the track up and calls the server, so the main thread never waits.

use std::cell::RefCell;
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;

use tauri::{AppHandle, Manager, Runtime};

use super::api::ServerApi;
use super::commands::ServersState;
use super::fetch::remote_file;
use super::Device;
use crate::library::commands::LibraryState;
use crate::library::db;
use crate::queue::model::QueueState;
use crate::settings;

/// What to tell a server about a library track.
#[derive(Debug, Clone, PartialEq)]
pub enum Report {
    Started { track_id: i64 },
    Stopped { track_id: i64, seconds: f64 },
}

/// Follows what plays and says what to report; separate from Tauri so
/// tests drive it directly.
#[derive(Debug, Default)]
pub struct Follower {
    /// The queue item playing, its track, and the engine's last position.
    playing: Option<(u64, i64, f64)>,
}

impl Follower {
    /// The queue's current item (uid, track) changed, or not.
    pub fn current(&mut self, item: Option<(u64, i64)>) -> Vec<Report> {
        let same = match (&self.playing, item) {
            (Some((uid, track_id, _)), Some((new_uid, new_track))) => {
                *uid == new_uid && *track_id == new_track
            }
            (None, None) => true,
            _ => false,
        };
        if same {
            return Vec::new();
        }
        let mut reports = self.finish();
        if let Some((uid, track_id)) = item {
            self.playing = Some((uid, track_id, 0.0));
            reports.push(Report::Started { track_id });
        }
        reports
    }

    pub fn position(&mut self, position: f64) {
        if let Some((_, _, last)) = &mut self.playing {
            *last = position;
        }
    }

    /// Stops following; reports where the track playing got to.
    pub fn finish(&mut self) -> Vec<Report> {
        self.playing
            .take()
            .map(|(_, track_id, seconds)| Report::Stopped { track_id, seconds })
            .into_iter()
            .collect()
    }
}

thread_local! {
    static FOLLOWER: RefCell<Follower> = RefCell::new(Follower::default());
}

/// The reporting thread's queue.
pub struct Reporter(Mutex<Sender<Report>>);

/// Starts the reporting thread. Call after the library and the settings.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let (sender, receiver) = mpsc::channel::<Report>();
    let thread_app = app.clone();
    std::thread::Builder::new()
        .name("server reports".into())
        .spawn(move || {
            let Some(db_path) = thread_app
                .try_state::<LibraryState>()
                .map(|library| library.db_path().to_path_buf())
            else {
                return;
            };
            let conn = match db::open(&db_path) {
                Ok(conn) => conn,
                Err(error) => {
                    log::warn!("{error}");
                    return;
                }
            };
            for report in receiver {
                send(&thread_app, &conn, &report);
            }
        })
        .map_err(|e| format!("Cannot start the server reports: {e}"))?;
    app.manage(Reporter(Mutex::new(sender)));
    Ok(())
}

/// The queue changed. Main thread.
pub fn queue_changed<R: Runtime>(app: &AppHandle<R>, state: &QueueState) {
    let item = state
        .current_item
        .as_ref()
        .filter(|item| !item.track.external && state.loaded)
        .map(|item| (item.uid, item.track.track_id));
    let reports = FOLLOWER.with_borrow_mut(|follower| follower.current(item));
    queue(app, reports);
}

/// The engine's position moved. Main thread.
pub fn position(position: f64) {
    FOLLOWER.with_borrow_mut(|follower| follower.position(position));
}

fn queue<R: Runtime>(app: &AppHandle<R>, reports: Vec<Report>) {
    if reports.is_empty() {
        return;
    }
    let current = settings::current(app);
    if !super::any_on(&current.features) || !current.servers.report_plays {
        return;
    }
    if let Some(reporter) = app.try_state::<Reporter>() {
        let sender = reporter.0.lock().unwrap_or_else(|e| e.into_inner());
        for report in reports {
            let _ = sender.send(report);
        }
    }
}

/// Sends a report, if its track is a server's. Failures are only logged:
/// a report is never worth bothering the user with.
fn send<R: Runtime>(app: &AppHandle<R>, conn: &rusqlite::Connection, report: &Report) {
    let track_id = match report {
        Report::Started { track_id } | Report::Stopped { track_id, .. } => *track_id,
    };
    let Ok(Some(file)) = remote_file(conn, track_id) else {
        return;
    };
    let (Ok(Some(source)), Some(servers)) = (
        super::source(conn, file.folder_id),
        app.try_state::<ServersState>(),
    ) else {
        return;
    };
    if !super::kind_on(&settings::current(app).features, source.kind) {
        return;
    }
    let (Ok(Some(token)), Ok(device)) = (source.token(), Device::of(conn)) else {
        return;
    };
    let server = ServerApi::new(&servers.http, &device, &source, Some(&token));
    let sent = match report {
        Report::Started { .. } => server.playing(&file),
        Report::Stopped { seconds, .. } => server.stopped(&file, *seconds),
    };
    if let Err(error) = sent {
        log::warn!(
            "cannot tell server {} what plays: {error}",
            source.folder_id
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_starts_and_stops_with_the_position() {
        let mut follower = Follower::default();
        assert!(follower.current(None).is_empty());
        assert_eq!(
            follower.current(Some((1, 10))),
            [Report::Started { track_id: 10 }]
        );
        follower.position(42.5);
        assert!(follower.current(Some((1, 10))).is_empty(), "no change");
        assert_eq!(
            follower.current(Some((2, 11))),
            [
                Report::Stopped {
                    track_id: 10,
                    seconds: 42.5
                },
                Report::Started { track_id: 11 }
            ]
        );
        // The same track again (repeat one, or queued twice) is a new play.
        assert_eq!(follower.current(Some((3, 11))).len(), 2);
        assert_eq!(
            follower.current(None),
            [Report::Stopped {
                track_id: 11,
                seconds: 0.0
            }]
        );
        assert!(follower.finish().is_empty());
    }
}
