//! Listening history (PLAN.md O8): what was played, kept on this computer
//! in `plays`, and, if the user opts in, sent to ListenBrainz.
//!
//! A play counts once a track has been listened to for half its length or
//! four minutes, whichever comes first (the usual scrobbling rule),
//! counting only time actually played, not seeks. The tracker follows the
//! queue and the engine's position on the main thread, where both arrive;
//! it hands plays to the history thread, which owns a database connection
//! and the ListenBrainz client, so the main thread never waits on either.
//! The views built on the history (recently played, O16; top played, O19;
//! and the history page's highlights) are in `views`.

pub mod listenbrainz;
pub mod views;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{params, Connection};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::library::commands::LibraryState;
use crate::library::{db, unix_now};
use crate::queue::model::QueueState;
use crate::settings;

/// Frontend event, with no payload, after a play is recorded.
pub const HISTORY_CHANGED_EVENT: &str = "history-changed";

/// A play counts after this much of the track, or `MAX_SECONDS`.
const FRACTION: f64 = 0.5;
const MAX_SECONDS: f64 = 240.0;

/// Position steps longer than this are seeks, not listening.
const MAX_STEP: f64 = 1.0;

/// The track playing, as far as the history is concerned.
#[derive(Debug, Clone, PartialEq)]
struct Listening {
    /// Tells this play from others, for the history thread.
    token: u64,
    uid: u64,
    track_id: i64,
    started_at: i64,
    duration: f64,
    played: f64,
    last_position: Option<f64>,
    recorded: bool,
}

/// Follows what plays; the host feeds it the queue's current item and the
/// engine's position. Separate from Tauri so tests drive it directly.
#[derive(Debug, Default)]
pub struct Tracker {
    listening: Option<Listening>,
    next_token: u64,
}

/// What the tracker tells the history thread.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// A play that has just counted.
    Played {
        token: u64,
        track_id: i64,
        started_at: i64,
        seconds: f64,
    },
    /// How long a counted play was listened to in the end.
    Listened { token: u64, seconds: f64 },
    /// Settings changed (ListenBrainz on or off, a new token).
    Wake,
}

impl Tracker {
    /// The queue's current item changed (or not). `loaded` says whether
    /// the engine has it open.
    pub fn current(
        &mut self,
        item: Option<(u64, i64, f64)>,
        loaded: bool,
        now: i64,
    ) -> Vec<Message> {
        let item = item.filter(|_| loaded);
        let same = match (&self.listening, item) {
            (Some(listening), Some((uid, track_id, _))) => {
                listening.uid == uid && listening.track_id == track_id
            }
            (None, None) => true,
            _ => false,
        };
        if same {
            return Vec::new();
        }
        let mut messages = self.finish();
        if let Some((uid, track_id, duration)) = item {
            messages.extend(self.start(uid, track_id, duration, now));
        }
        messages
    }

    /// The engine's position moved; only steps of normal playback count.
    pub fn position(&mut self, position: f64, duration: f64, now: i64) -> Vec<Message> {
        let mut messages = Vec::new();
        let Some(listening) = &mut self.listening else {
            return messages;
        };
        if duration > 0.0 {
            listening.duration = duration;
        }
        // Back to the start after the end: repeat one played it again.
        if let Some(last) = listening.last_position {
            if position + MAX_STEP < last && last >= listening.duration - 2.0 && position < 2.0 {
                let (uid, track_id, duration) =
                    (listening.uid, listening.track_id, listening.duration);
                messages.extend(self.finish());
                messages.extend(self.start(uid, track_id, duration, now));
                let listening = self.listening.as_mut().expect("just started");
                listening.last_position = Some(position);
                return messages;
            }
        }
        if let Some(last) = listening.last_position {
            let step = position - last;
            if step > 0.0 && step <= MAX_STEP {
                listening.played += step;
            }
        }
        listening.last_position = Some(position);
        let needed = (listening.duration * FRACTION).min(MAX_SECONDS);
        if !listening.recorded && listening.duration > 0.0 && listening.played >= needed {
            listening.recorded = true;
            messages.push(Message::Played {
                token: listening.token,
                track_id: listening.track_id,
                started_at: listening.started_at,
                seconds: listening.played,
            });
        }
        messages
    }

    fn start(&mut self, uid: u64, track_id: i64, duration: f64, now: i64) -> Vec<Message> {
        self.next_token += 1;
        self.listening = Some(Listening {
            token: self.next_token,
            uid,
            track_id,
            started_at: now,
            duration,
            played: 0.0,
            last_position: None,
            recorded: false,
        });
        Vec::new()
    }

    fn finish(&mut self) -> Vec<Message> {
        match self.listening.take() {
            Some(listening) if listening.recorded => vec![Message::Listened {
                token: listening.token,
                seconds: listening.played,
            }],
            _ => Vec::new(),
        }
    }
}

// ---- The history thread -------------------------------------------------------

/// Writes plays, and sends listens, off the main thread.
pub struct History {
    sender: Mutex<Sender<Message>>,
}

thread_local! {
    static TRACKER: RefCell<Tracker> = RefCell::new(Tracker::default());
}

/// Starts the history thread. Call after the library and the settings.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let library = app
        .try_state::<LibraryState>()
        .ok_or("The library is not available")?;
    let db_path = library.db_path().to_path_buf();
    let (sender, receiver) = mpsc::channel();
    let thread_app = app.clone();
    std::thread::Builder::new()
        .name("history".into())
        .spawn(move || match db::open(&db_path) {
            Ok(conn) => run(&thread_app, &conn, &receiver),
            Err(error) => eprintln!("[history] {error}"),
        })
        .map_err(|e| format!("Cannot start the history: {e}"))?;
    app.manage(History {
        sender: Mutex::new(sender),
    });
    Ok(())
}

fn send<R: Runtime>(app: &AppHandle<R>, messages: Vec<Message>) {
    if messages.is_empty() {
        return;
    }
    if let Some(history) = app.try_state::<History>() {
        let sender = history.sender.lock().unwrap_or_else(|e| e.into_inner());
        for message in messages {
            let _ = sender.send(message);
        }
    }
}

/// The queue changed. Main thread.
pub fn queue_changed<R: Runtime>(app: &AppHandle<R>, state: &QueueState) {
    if !settings::current(app).features.listening_history {
        TRACKER.with_borrow_mut(|tracker| *tracker = Tracker::default());
        return;
    }
    let item = state
        .current_item
        .as_ref()
        .map(|item| (item.uid, item.track.track_id, item.track.duration));
    let messages =
        TRACKER.with_borrow_mut(|tracker| tracker.current(item, state.loaded, unix_now()));
    send(app, messages);
}

/// The engine's position moved. Main thread.
pub fn position<R: Runtime>(app: &AppHandle<R>, position: f64, duration: f64) {
    let messages =
        TRACKER.with_borrow_mut(|tracker| tracker.position(position, duration, unix_now()));
    send(app, messages);
}

/// The settings changed: ListenBrainz may be on or off now.
pub fn wake<R: Runtime>(app: &AppHandle<R>) {
    send(app, vec![Message::Wake]);
}

fn run<R: Runtime>(app: &AppHandle<R>, conn: &Connection, receiver: &Receiver<Message>) {
    let mut rows: HashMap<u64, i64> = HashMap::new();
    let mut sender = listenbrainz::Sender::new();
    loop {
        let message = receiver.recv_timeout(sender.wait());
        match message {
            Ok(Message::Played {
                token,
                track_id,
                started_at,
                seconds,
            }) => match record(conn, track_id, started_at, seconds) {
                Ok(row) => {
                    rows.insert(token, row);
                    let _ = app.emit(HISTORY_CHANGED_EVENT, ());
                    let features = settings::current(app).features;
                    if features.listenbrainz {
                        if let Err(error) = listenbrainz::queue(conn, track_id, started_at) {
                            eprintln!("[history] {error}");
                        }
                    }
                }
                Err(error) => eprintln!("[history] {error}"),
            },
            Ok(Message::Listened { token, seconds }) => {
                if let Some(row) = rows.remove(&token) {
                    if let Err(error) = conn.execute(
                        "UPDATE plays SET seconds = ?1 WHERE id = ?2",
                        params![seconds, row],
                    ) {
                        eprintln!("[history] {error}");
                    }
                }
            }
            Ok(Message::Wake) => sender.retry_now(),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        if settings::current(app).features.listenbrainz {
            sender.send_pending(conn);
        }
    }
}

/// Adds a play to the history; returns its row.
pub fn record(
    conn: &Connection,
    track_id: i64,
    started_at: i64,
    seconds: f64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO plays (track_id, played_at, seconds) VALUES (?1, ?2, ?3)",
        params![track_id, started_at, seconds],
    )?;
    Ok(conn.last_insert_rowid())
}

/// The longest the history thread sleeps between checks of what's pending.
pub const IDLE_WAIT: Duration = Duration::from_secs(300);

#[cfg(test)]
mod tests {
    use super::*;

    fn play(tracker: &mut Tracker, from: f64, to: f64, duration: f64) -> Vec<Message> {
        let mut messages = Vec::new();
        let mut position = from;
        while position <= to + 1e-9 {
            messages.extend(tracker.position(position, duration, 1000));
            position += 0.05;
        }
        messages
    }

    #[test]
    fn a_play_counts_at_half_the_track_or_four_minutes() {
        let mut tracker = Tracker::default();
        assert!(tracker.current(Some((1, 10, 60.0)), true, 500).is_empty());
        assert!(play(&mut tracker, 0.0, 29.0, 60.0).is_empty());
        let messages = play(&mut tracker, 29.05, 31.0, 60.0);
        assert_eq!(messages.len(), 1);
        let Message::Played {
            track_id,
            started_at,
            seconds,
            ..
        } = messages[0]
        else {
            panic!("{messages:?}")
        };
        assert_eq!((track_id, started_at), (10, 500));
        assert!((seconds - 30.0).abs() < 0.1, "{seconds}");
        // Once only.
        assert!(play(&mut tracker, 31.05, 59.0, 60.0).is_empty());

        // Moving on sends how long it was listened to.
        let messages = tracker.current(Some((2, 11, 600.0)), true, 600);
        assert!(matches!(messages[..], [Message::Listened { seconds, .. }] if seconds > 58.0));

        // A long track counts after four minutes.
        assert!(play(&mut tracker, 0.0, 239.0, 600.0).is_empty());
        assert_eq!(play(&mut tracker, 239.05, 241.0, 600.0).len(), 1);
    }

    #[test]
    fn seeks_and_skips_dont_count() {
        let mut tracker = Tracker::default();
        tracker.current(Some((1, 10, 60.0)), true, 0);
        play(&mut tracker, 0.0, 5.0, 60.0);
        // Seeking to near the end doesn't count as listening to the middle.
        assert!(play(&mut tracker, 55.0, 59.9, 60.0).is_empty());
        // Skipped before it counted: nothing to send.
        assert!(tracker.current(Some((2, 11, 60.0)), true, 0).is_empty());
        // Not loaded (restored after a relaunch) isn't listening.
        assert!(tracker.current(Some((2, 11, 60.0)), false, 0).is_empty());
        assert!(tracker.listening.is_none());
    }

    #[test]
    fn repeating_a_track_counts_again() {
        let mut tracker = Tracker::default();
        tracker.current(Some((1, 10, 20.0)), true, 0);
        assert_eq!(play(&mut tracker, 0.0, 19.9, 20.0).len(), 1);
        let messages = play(&mut tracker, 0.0, 10.5, 20.0);
        assert!(
            matches!(messages[0], Message::Listened { .. }),
            "{messages:?}"
        );
        assert!(
            matches!(messages[1], Message::Played { .. }),
            "{messages:?}"
        );
    }

    #[test]
    fn records_plays() {
        let conn = db::open_in_memory().unwrap();
        let library = crate::library::test_library::Library::from_conn(conn);
        library.add(
            library.folder_id,
            crate::library::test_library::track("a.flac").title("A"),
        );
        let row = record(&library.conn, 1, 1234, 31.5).unwrap();
        let (track, at, seconds): (i64, i64, f64) = library
            .conn
            .query_row(
                "SELECT track_id, played_at, seconds FROM plays WHERE id = ?1",
                [row],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((track, at, seconds), (1, 1234, 31.5));
    }
}
