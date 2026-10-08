//! The play queue's logic, apart from the engine so it can be tested without
//! an audio device. The engine holds the current track and one pre-opened
//! next track (PLAN.md Phase 1); the queue decides what those are, and moves
//! on when the engine hands off to the next one.
//!
//! The list is kept in play order, which is also the order the UI shows.
//! Shuffle reorders the items after the current one and remembers the order
//! before, which turning shuffle off restores. After every change the queue
//! re-arms the engine's next track if it should now be a different one, so a
//! reorder, a removal or a new repeat mode still hands off gaplessly.
//!
//! Some tracks belong together (PLAN.md O3, O6, O7): tracks that segue into
//! each other, the movements of a work, an album the user never wants
//! shuffled. The host gives them a `unit`, and shuffle moves each run of
//! adjacent items with one unit as a block, in order. Tracks the user marked
//! to skip are passed over by next, previous and the automatic advance,
//! but play when chosen directly. In radio mode (O9) the host keeps adding
//! tracks as the queue nears its end (`radio_seed`).
//!
//! Playback can stop on its own (PLAN.md F13): after a chosen item ("stop
//! after this track"), at the end of the current track or album, or when a
//! sleep timer runs out, fading out over its last `SLEEP_FADE` seconds
//! through the volume (`tick`). The queue then moves to the next item,
//! paused, so play carries on from there.
//!
//! Items that can't be opened are passed over: those that failed to open,
//! and those whose folder can't be read now (PLAN.md H22,
//! `set_unavailable_tracks`), which aren't reported as skipped. When
//! nothing can be opened, nothing plays, the current item stays current and
//! keeps its position, and the queue doesn't spin.
//!
//! Opening a file can take seconds (a disk waking, a network share, a cloud
//! file downloading: PLAN.md H11), so the engine may open it on a thread of
//! its own: `Player::load` and `Player::set_next` then return
//! `Opening::Pending`, and the host reports the outcome with
//! `load_finished`. Meanwhile the item is current and shown as loading
//! (`QueueState::loading`), play, pause and seek apply to it when it's
//! ready, and the engine plays on what it had. One that takes longer than
//! `LOAD_TIMEOUT` (`check_loads`) fails with `open_timed_out` and is passed
//! over as a file that can't be opened is. A cloud placeholder (H12), which
//! the host says is `downloading`, gets `DOWNLOAD_TIMEOUT`.
//!
//! A long track (an audiobook, a DJ mix: over `LONG_TRACK` seconds) starts
//! where it was left (F17): the host gives each its saved position
//! (`TrackInfo::resume`), and the queue says where it left each one
//! (`take_positions`). A hand-off crossfades (F14) unless the two tracks
//! are on one album or in one unit, which stay gapless; the host decides how
//! long a crossfade is, if any.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::anomp::PlayerState;

/// Identifies an item in the queue; unlike its index, it survives reorders,
/// and unlike the track id, it tells two copies of a track apart.
pub type Uid = u64;

/// Seconds into a track after which "previous" restarts it rather than
/// going back a track.
pub const RESTART_THRESHOLD: f64 = 3.0;

/// Tracks longer than this, in seconds, start where they were left.
pub const LONG_TRACK: f64 = 20.0 * 60.0;

/// A long track's position is kept only this far (seconds) from either end.
const LONG_TRACK_MARGIN: f64 = 30.0;

/// Seconds a sleep timer fades out over before it pauses.
pub const SLEEP_FADE: f64 = 10.0;

/// Seconds a track may take to open before it's passed over (a sleeping
/// disk spins up within this).
pub const LOAD_TIMEOUT: f64 = 20.0;

/// Seconds a cloud placeholder may take to download and open (H12).
pub const DOWNLOAD_TIMEOUT: f64 = 300.0;

/// The error a track that took more than `seconds` to open fails with.
pub fn open_timed_out(seconds: f64) -> String {
    crate::coded::coded(
        "openTimedOut",
        &[("seconds", serde_json::json!(seconds))],
        format!("The file took more than {seconds} s to open"),
    )
}

/// What the queue shows about a track.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    pub track_id: i64,
    /// The title, or the file name if the track has none.
    pub title: String,
    pub artist: Option<String>,
    pub artist_id: Option<i64>,
    pub album: Option<String>,
    pub album_id: Option<i64>,
    pub duration: f64,
    /// Passed over in album and shuffle play; it still plays when chosen.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub skip: bool,
    /// Adjacent items with the same unit shuffle as one block, in order.
    #[serde(skip)]
    pub unit: Option<i64>,
    /// Why library radio picked it ("same label, 1994").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// A long track's saved position, where it starts (F17).
    #[serde(skip)]
    pub resume: Option<f64>,
    /// A file outside the library, opened from the Finder (F5): its track
    /// id is negative and only good for this session.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub external: bool,
}

impl TrackInfo {
    fn is_long(&self) -> bool {
        self.duration > LONG_TRACK
    }
}

/// When a sleep timer stops playback.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum SleepTimer {
    /// At this time (the host's clock, in seconds), fading out before it;
    /// set for `minutes`.
    At { ends_at: f64, minutes: u32 },
    /// When the current track ends.
    EndOfTrack,
    /// When the last track of the current item's album ends.
    EndOfAlbum,
}

/// A sleep timer running, and the volume a fade started from.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sleep {
    timer: SleepTimer,
    fading_from: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "QueueItem"))]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub uid: Uid,
    #[serde(flatten)]
    pub track: TrackInfo,
}

/// One change to the list (PLAN.md H16), sent to the frontend, which
/// applies it to its copy (`app/src/lib/queueEdits.ts`), and applied to the
/// saved rows (`store`). Indices are into the list as each edit finds it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "QueueEdit"))]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum Edit {
    /// `items` go in before index `at`.
    Insert {
        at: usize,
        items: Vec<Item>,
        /// With shuffle on, where they go in the order before shuffling;
        /// the store's business, not the frontend's.
        #[serde(skip)]
        #[cfg_attr(test, ts(skip))]
        original_at: Option<usize>,
    },
    /// `count` items from `at` leave.
    Remove { at: usize, count: usize },
    /// `count` items from `from` move to `to`, an index in the list
    /// without them.
    Move {
        from: usize,
        count: usize,
        to: usize,
    },
    /// The items from `at` show something new (their tags changed); same
    /// uids, same tracks.
    Update { at: usize, items: Vec<Item> },
}

/// The list's changes since one side last took them.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ListLog {
    #[default]
    Clean,
    Edits(Vec<Edit>),
    /// Replaced (or cleared, shuffled, unshuffled): send or write it whole.
    Reset,
}

impl ListLog {
    fn push(&mut self, edit: Edit) {
        match self {
            ListLog::Clean => *self = ListLog::Edits(vec![edit]),
            ListLog::Edits(edits) => edits.push(edit),
            ListLog::Reset => {}
        }
    }

    fn is_clean(&self) -> bool {
        *self == ListLog::Clean
    }
}

/// `indices`, sorted, as `Remove` edits from the last range back, so each
/// finds the indices before it unchanged.
fn removals(mut indices: Vec<usize>) -> Vec<Edit> {
    indices.sort_unstable();
    let mut edits: Vec<Edit> = Vec::new();
    for index in indices.into_iter().rev() {
        match edits.last_mut() {
            Some(Edit::Remove { at, count }) if *at == index + 1 => {
                *at = index;
                *count += 1;
            }
            _ => edits.push(Edit::Remove {
                at: index,
                count: 1,
            }),
        }
    }
    edits
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

/// A track that couldn't be opened and was passed over.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub uid: Uid,
    pub track_id: i64,
    pub title: String,
    pub error: String,
}

/// Identifies a request to open a track (`Opening::Pending`).
pub type Request = u64;

/// What `Player::load` and `Player::set_next` did.
#[derive(Debug, Clone, PartialEq)]
pub enum Opening {
    /// Done at once: the track is in place, or it couldn't be opened.
    Done(Result<(), String>),
    /// Opening; the host reports it with `Queue::load_finished`.
    Pending(Request),
}

/// The engine, as the queue drives it.
pub trait Player {
    /// Opens a track as the current one, stopped at its start; this clears
    /// the next track. On failure nothing changes. While it's pending, the
    /// engine keeps what it had.
    fn load(&mut self, track_id: i64) -> Opening;
    /// Opens the track that follows the current one gaplessly (crossfading
    /// into it if `crossfade`, as far as the settings say), or clears it
    /// (`None`, always done at once). A pending one clears the next track
    /// until it's ready.
    fn set_next(&mut self, track_id: Option<i64>, crossfade: bool) -> Opening;
    /// Gives up on a pending request; it isn't reported.
    fn cancel(&mut self, request: Request);
    /// The volume, 0 to 1, which a sleep timer fades.
    fn volume(&self) -> f64;
    fn set_volume(&mut self, volume: f64);
    fn play(&mut self) -> bool;
    fn pause(&mut self);
    fn stop(&mut self);
    fn seek(&mut self, seconds: f64) -> bool;
    fn state(&self) -> PlayerState;
    fn position(&self) -> f64;
    /// Hand-offs to the next track so far (`Engine::advance_count`).
    fn advance_count(&self) -> i64;
}

/// What the UI needs to show the queue and the current track.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct QueueState {
    /// Increases with every state sent.
    pub revision: u64,
    /// Increases with every state that changes the list (PLAN.md H16).
    pub list_version: u64,
    /// The whole list, when it was replaced (and in every full state).
    pub items: Option<Vec<Item>>,
    /// Otherwise, when it changed: the edits from `list_version - 1`. A
    /// frontend that has another version asks for the whole state.
    pub edits: Option<Vec<Edit>>,
    pub length: usize,
    /// Index of the current item; `None` only when the queue is empty.
    pub current: Option<usize>,
    pub current_item: Option<Item>,
    pub shuffle: bool,
    pub repeat: Repeat,
    /// Items that couldn't be opened the last time they were tried.
    pub unavailable: Vec<Uid>,
    /// Tracks passed over since the previous state.
    pub skipped: Vec<Skipped>,
    /// Whether `next` and `previous` have an item to go to.
    pub has_next: bool,
    pub has_previous: bool,
    /// Whether the engine has the current track open. If not (after a
    /// relaunch, or while it opens), `resume_at` is where it will start, in
    /// seconds.
    pub loaded: bool,
    /// The current item is being opened, to play if `playing` (H11).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub loading: bool,
    /// While loading: whether it will play once it's open.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub playing: bool,
    /// While loading: it's a cloud placeholder, downloading (H12).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub downloading: bool,
    pub resume_at: f64,
    /// Library radio keeps adding tracks as the queue runs out.
    pub radio: bool,
    /// Playback stops after this item (F13).
    pub stop_after: Option<Uid>,
    /// The sleep timer running, if any.
    pub sleep: Option<SleepTimer>,
}

/// The queue as saved across launches.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Saved {
    pub tracks: Vec<i64>,
    /// The items' uids, as the store saved them; empty for new ones.
    #[serde(skip)]
    pub uids: Vec<Uid>,
    /// With shuffle on, the order before shuffling, as indices into `tracks`.
    pub original: Option<Vec<usize>>,
    pub current: Option<usize>,
    /// Seconds into the current track.
    pub position: f64,
    pub repeat: Repeat,
}

/// The current item being opened (`Opening::Pending`).
#[derive(Debug, Clone, PartialEq)]
struct Loading {
    request: Request,
    uid: Uid,
    /// Plays once it's open.
    play: bool,
    /// When it was asked for, by the host's clock.
    since: f64,
    /// The item first asked for, and where: if nothing after it can be
    /// opened either, it stays current there.
    asked: Uid,
    asked_resume_at: f64,
    /// Whether the engine had the queue's track when this was asked for.
    was_loaded: bool,
    /// A cloud placeholder, downloading (`Queue::downloading`).
    downloading: bool,
}

/// The item being opened as the engine's next track.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Arming {
    request: Request,
    uid: Uid,
    since: f64,
    downloading: bool,
}

pub struct Queue {
    items: Vec<Item>,
    /// With shuffle on, every item's uid in the order before shuffling.
    original: Option<Vec<Uid>>,
    current: Option<usize>,
    repeat: Repeat,
    next_uid: Uid,
    rng: u64,
    /// Whether the engine's current track is the current item. False after
    /// a relaunch until playback starts, or after something else was loaded.
    loaded: bool,
    /// The item the engine will hand off to, if any.
    armed: Option<Uid>,
    /// The current item, while it's being opened.
    loading: Option<Loading>,
    /// The item being opened as the next track.
    arming: Option<Arming>,
    /// The engine's advance count the queue has accounted for.
    seen_advances: i64,
    resume_at: f64,
    unavailable: HashSet<Uid>,
    revision: u64,
    changed: bool,
    list_version: u64,
    /// The list's changes not yet sent, and not yet saved (`take_stored`).
    sent: ListLog,
    stored: ListLog,
    skipped: Vec<Skipped>,
    /// Radio mode (O9): the host adds tracks as the queue runs out.
    radio: bool,
    stop_after: Option<Uid>,
    sleep: Option<Sleep>,
    /// Long tracks' positions to save (`None` forgets one), by track id.
    positions: Vec<(i64, Option<f64>)>,
    /// The host's clock, in seconds, as of its last call (`set_clock`).
    clock: f64,
}

impl Queue {
    /// An empty queue; `seed` drives shuffling.
    pub fn new(seed: u64) -> Queue {
        Queue {
            items: Vec::new(),
            original: None,
            current: None,
            repeat: Repeat::Off,
            next_uid: 1,
            rng: seed | 1,
            loaded: false,
            armed: None,
            loading: None,
            arming: None,
            seen_advances: 0,
            resume_at: 0.0,
            unavailable: HashSet::new(),
            revision: 0,
            changed: false,
            list_version: 0,
            sent: ListLog::Clean,
            stored: ListLog::Clean,
            skipped: Vec::new(),
            radio: false,
            stop_after: None,
            sleep: None,
            positions: Vec::new(),
            clock: 0.0,
        }
    }

    /// The saved queue, not loaded (nothing plays until asked). Tracks that
    /// `tracks` doesn't have (removed from the library) are dropped.
    pub fn restore(saved: Saved, tracks: &HashMap<i64, TrackInfo>, seed: u64) -> Queue {
        let mut queue = Queue::new(seed);
        let mut index_of_saved = Vec::with_capacity(saved.tracks.len());
        queue.next_uid = saved.uids.iter().max().map_or(1, |max| max + 1);
        for (i, id) in saved.tracks.iter().enumerate() {
            index_of_saved.push(tracks.get(id).map(|track| {
                let item = match saved.uids.get(i) {
                    Some(&uid) => Item {
                        uid,
                        track: track.clone(),
                    },
                    None => queue.new_item(track.clone()),
                };
                queue.items.push(item);
                queue.items.len() - 1
            }));
        }
        let kept = |index: usize| index_of_saved.get(index).copied().flatten();
        if let Some(saved_current) = saved.current {
            queue.current = kept(saved_current);
            if queue.current.is_some() {
                queue.resume_at = saved.position.max(0.0);
            } else if !queue.items.is_empty() {
                // The current track is gone: the next one left takes its place.
                let before = index_of_saved[..saved_current.min(index_of_saved.len())]
                    .iter()
                    .flatten()
                    .count();
                queue.current = Some(before.min(queue.items.len() - 1));
            }
        } else if !queue.items.is_empty() {
            queue.current = Some(0);
        }
        queue.original = saved.original.map(|original| {
            let mut uids: Vec<Uid> = original
                .into_iter()
                .filter_map(kept)
                .map(|index| queue.items[index].uid)
                .collect();
            let listed: HashSet<Uid> = uids.iter().copied().collect();
            if listed.len() != uids.len() {
                // Not a permutation: fall back to the play order.
                return queue.items.iter().map(|item| item.uid).collect();
            }
            uids.extend(
                queue
                    .items
                    .iter()
                    .map(|item| item.uid)
                    .filter(|uid| !listed.contains(uid)),
            );
            uids
        });
        queue.repeat = saved.repeat;
        // The store's rows are what was restored (less any `retain` drops).
        queue.sent = ListLog::Reset;
        queue
    }

    /// What to save; `position` is the seconds into the current track.
    /// Files outside the library are left out: the OS lets the app open
    /// them only until it quits.
    pub fn saved(&self, position: f64) -> Saved {
        let kept: Vec<&Item> = self
            .items
            .iter()
            .filter(|item| !item.track.external)
            .collect();
        let index: HashMap<Uid, usize> = kept
            .iter()
            .enumerate()
            .map(|(index, item)| (item.uid, index))
            .collect();
        let current = self
            .current_item()
            .and_then(|item| index.get(&item.uid).copied());
        Saved {
            tracks: kept.iter().map(|item| item.track.track_id).collect(),
            uids: kept.iter().map(|item| item.uid).collect(),
            original: self.original.as_ref().map(|uids| {
                uids.iter()
                    .filter_map(|uid| index.get(uid).copied())
                    .collect()
            }),
            current,
            position: match (current, self.loaded) {
                (None, _) => 0.0,
                (Some(_), true) => position,
                (Some(_), false) => self.resume_at,
            },
            repeat: self.repeat,
        }
    }

    /// The list's changes since the store last took them (PLAN.md H16).
    pub fn take_stored(&mut self) -> ListLog {
        std::mem::take(&mut self.stored)
    }

    /// Every item, in play order.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// With shuffle on, the uids in their order before shuffling.
    pub fn original_order(&self) -> Option<&[Uid]> {
        self.original.as_deref()
    }

    /// Marks the state as changed, so it is sent and saved.
    pub fn touch(&mut self) {
        self.changed = true;
    }

    /// Every item's track id, in order.
    pub fn track_ids(&self) -> Vec<i64> {
        self.items.iter().map(|item| item.track.track_id).collect()
    }

    /// The tracks the engine has open for the queue: the current item's if
    /// it's loaded, then the armed next item's (the same track twice with
    /// repeat one).
    pub fn engine_track_ids(&self) -> Vec<i64> {
        if !self.loaded {
            return Vec::new();
        }
        let current = self.current_item().map(|item| item.track.track_id);
        let armed = self
            .armed
            .and_then(|uid| self.index_of(uid))
            .map(|index| self.items[index].track.track_id);
        current.into_iter().chain(armed).collect()
    }

    /// Replaces what the items show with `tracks` (e.g. after a rescan);
    /// items whose track isn't in it are left as they were.
    pub fn update_tracks(&mut self, tracks: &HashMap<i64, TrackInfo>) {
        let mut changed = Vec::new();
        for (index, item) in self.items.iter_mut().enumerate() {
            if let Some(track) = tracks.get(&item.track.track_id) {
                // Why radio picked it stays.
                let track = TrackInfo {
                    reason: item.track.reason.clone(),
                    ..track.clone()
                };
                let track = TrackInfo {
                    // Where the queue left it is newer than the library's.
                    resume: item.track.resume,
                    ..track
                };
                if track != item.track {
                    item.track = track;
                    changed.push(index);
                }
            }
        }
        // One edit per run of changed items.
        let mut start = 0;
        while start < changed.len() {
            let mut end = start + 1;
            while end < changed.len() && changed[end] == changed[end - 1] + 1 {
                end += 1;
            }
            let at = changed[start];
            let items = self.items[at..at + (end - start)].to_vec();
            self.edited(Edit::Update { at, items });
            start = end;
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub fn state_shuffle(&self) -> bool {
        self.original.is_some()
    }

    pub fn stop_after(&self) -> Option<Uid> {
        self.stop_after
    }

    pub fn current_item(&self) -> Option<&Item> {
        self.current.map(|index| &self.items[index])
    }

    /// The whole state, list included.
    pub fn state(&mut self) -> QueueState {
        self.sent = ListLog::Reset;
        self.take_state().expect("a changed queue has a state")
    }

    /// The state if anything changed since the last one, marking it sent.
    pub fn take_state(&mut self) -> Option<QueueState> {
        if !self.changed && self.sent.is_clean() && self.skipped.is_empty() {
            return None;
        }
        self.revision += 1;
        let (items, edits) = match std::mem::take(&mut self.sent) {
            ListLog::Clean => (None, None),
            ListLog::Reset => (Some(self.items.clone()), None),
            ListLog::Edits(edits) => (None, Some(edits)),
        };
        if items.is_some() || edits.is_some() {
            self.list_version += 1;
        }
        let state = QueueState {
            revision: self.revision,
            list_version: self.list_version,
            items,
            edits,
            length: self.items.len(),
            current: self.current,
            current_item: self.current_item().cloned(),
            shuffle: self.original.is_some(),
            repeat: self.repeat,
            // In list order; most states have none, so skip the walk then.
            unavailable: if self.unavailable.is_empty() {
                Vec::new()
            } else {
                self.items
                    .iter()
                    .map(|item| item.uid)
                    .filter(|uid| self.unavailable.contains(uid))
                    .collect()
            },
            skipped: std::mem::take(&mut self.skipped),
            has_next: self.current.is_some_and(|current| {
                self.find(current, true, self.repeat == Repeat::All)
                    .is_some()
            }),
            has_previous: self.current.is_some_and(|current| {
                self.find(current, false, self.repeat == Repeat::All)
                    .is_some()
            }),
            loaded: self.loaded,
            loading: self.loading.is_some(),
            playing: self.loading.as_ref().is_some_and(|loading| loading.play),
            downloading: self
                .loading
                .as_ref()
                .is_some_and(|loading| loading.downloading),
            resume_at: if self.loaded { 0.0 } else { self.resume_at },
            radio: self.radio,
            stop_after: self.stop_after,
            sleep: self.sleep.map(|sleep| sleep.timer),
        };
        self.changed = false;
        Some(state)
    }

    // ---- Commands ---------------------------------------------------------

    /// Replaces the queue with `tracks` and starts `start`, which the user
    /// chose (with shuffle on, it plays first and the rest are shuffled).
    /// Playing unless `play` is false. Ends radio mode.
    pub fn replace(
        &mut self,
        p: &mut impl Player,
        tracks: Vec<TrackInfo>,
        start: usize,
        play: bool,
    ) {
        self.replace_from(p, tracks, start, play, true);
    }

    /// `replace`, where `chosen` says whether the user picked the start
    /// track (a skipped track then still plays) or just played the lot (it
    /// starts at the first track not to skip).
    pub fn replace_from(
        &mut self,
        p: &mut impl Player,
        tracks: Vec<TrackInfo>,
        start: usize,
        play: bool,
        chosen: bool,
    ) {
        self.reconcile(p);
        self.radio = false;
        if tracks.is_empty() {
            return self.clear(p);
        }
        self.stop_after = None;
        self.leave_current(p);
        self.items = tracks
            .into_iter()
            .map(|track| self.new_item(track))
            .collect();
        self.unavailable.clear();
        self.reset_list();
        let mut start = start.min(self.items.len() - 1);
        if !chosen && self.items[start].track.skip {
            start = (start..self.items.len())
                .find(|&index| !self.items[index].track.skip)
                .unwrap_or(start);
        }
        if self.original.is_some() {
            self.original = Some(self.items.iter().map(|item| item.uid).collect());
            // The start's run (the rest of its work, say) comes first with it.
            let end = self.run_end(start);
            let first: Vec<Item> = self.items.drain(start..end).collect();
            self.items.splice(0..0, first);
            let after = end - start;
            self.shuffle_from(after);
            start = 0;
        }
        self.current = Some(start);
        self.start(p, start, play, 0.0);
    }

    /// Starts radio mode with `tracks` (the seed first), playing the seed.
    pub fn start_radio(&mut self, p: &mut impl Player, tracks: Vec<TrackInfo>) {
        let shuffle = self.original.take().is_some();
        self.replace(p, tracks, 0, true);
        if shuffle {
            self.original = Some(self.items.iter().map(|item| item.uid).collect());
        }
        self.radio = true;
        self.changed = true;
    }

    /// Radio mode: whether the queue ends within `lookahead` items of the
    /// current one, and if so the track to pick more like (the last one).
    pub fn radio_seed(&self, lookahead: usize) -> Option<i64> {
        let current = self.current?;
        let near_end = self.repeat == Repeat::Off && self.items.len() - current <= lookahead;
        near_end.then(|| self.items.last().map(|item| item.track.track_id))?
    }

    pub fn is_radio(&self) -> bool {
        self.radio
    }

    /// Turns radio mode off, e.g. when library radio is turned off.
    pub fn stop_radio(&mut self) {
        if self.radio {
            self.radio = false;
            self.changed = true;
        }
    }

    /// Adds tracks after the current item (`next`) or at the end, without
    /// starting them.
    pub fn add(&mut self, p: &mut impl Player, tracks: Vec<TrackInfo>, next: bool) {
        self.reconcile(p);
        if tracks.is_empty() {
            return;
        }
        let new: Vec<Item> = tracks
            .into_iter()
            .map(|track| self.new_item(track))
            .collect();
        let uids: Vec<Uid> = new.iter().map(|item| item.uid).collect();
        let at = match self.current {
            Some(current) if next => current + 1,
            _ => self.items.len(),
        };
        let current_uid = self.current_item().map(|item| item.uid);
        let mut original_at = None;
        if let Some(original) = &mut self.original {
            let at = match current_uid.filter(|_| next) {
                Some(uid) => original.iter().position(|&o| o == uid).map_or(0, |i| i + 1),
                None => original.len(),
            };
            original.splice(at..at, uids);
            original_at = Some(at);
        }
        self.edited(Edit::Insert {
            at,
            items: new.clone(),
            original_at,
        });
        self.items.splice(at..at, new);
        if self.current.is_none() {
            self.current = Some(0);
        }
        self.sync_next(p);
    }

    /// Adds tracks after the current item and plays the first of them, e.g.
    /// files opened from the Finder (F5).
    /// Plays `tracks` after the current item, starting now with the first,
    /// whose uid it returns.
    pub fn play_now(&mut self, p: &mut impl Player, tracks: Vec<TrackInfo>) -> Option<Uid> {
        if tracks.is_empty() {
            return None;
        }
        let first = self.next_uid;
        self.add(p, tracks, true);
        self.jump(p, first);
        Some(first)
    }

    /// Removes items. If the current one goes, the item after it takes its
    /// place (loaded, and playing if it was); if it was last, the one before
    /// it does, without playing.
    pub fn remove(&mut self, p: &mut impl Player, uids: &[Uid]) {
        self.reconcile(p);
        let gone: HashSet<Uid> = uids.iter().copied().collect();
        let Some(current) = self.current else {
            return;
        };
        if !self.items.iter().any(|item| gone.contains(&item.uid)) {
            return;
        }
        let removing_current = gone.contains(&self.items[current].uid);
        if removing_current {
            self.leave_current(p);
        }
        let kept_before = self.items[..current]
            .iter()
            .filter(|item| !gone.contains(&item.uid))
            .count();
        let indices = (0..self.items.len())
            .filter(|&index| gone.contains(&self.items[index].uid))
            .collect();
        for edit in removals(indices) {
            self.edited(edit);
        }
        self.items.retain(|item| !gone.contains(&item.uid));
        if let Some(original) = &mut self.original {
            original.retain(|uid| !gone.contains(uid));
        }
        if self.stop_after.is_some_and(|uid| gone.contains(&uid)) {
            self.stop_after = None;
        }
        self.unavailable.retain(|uid| !gone.contains(uid));
        if self.items.is_empty() {
            return self.clear(p);
        }
        if !removing_current {
            self.current = Some(kept_before);
            return self.sync_next(p);
        }
        let following = kept_before < self.items.len();
        let index = kept_before.min(self.items.len() - 1);
        self.current = Some(index);
        if self.loaded || self.loading.is_some() {
            let play = following && self.is_playing(p);
            self.start(p, index, play, 0.0);
        } else {
            self.resume_at = 0.0;
        }
    }

    /// Moves an item to `to` (an index in the list after the move).
    pub fn move_item(&mut self, p: &mut impl Player, uid: Uid, to: usize) {
        self.reconcile(p);
        let Some(from) = self.index_of(uid) else {
            return;
        };
        let to = to.min(self.items.len() - 1);
        if from == to {
            return;
        }
        let current_uid = self.current_item().map(|item| item.uid);
        let item = self.items.remove(from);
        self.items.insert(to, item);
        self.current = current_uid.and_then(|uid| self.index_of(uid));
        self.edited(Edit::Move { from, count: 1, to });
        self.sync_next(p);
    }

    /// Moves items (keeping their order) to `to`, an index in the list after
    /// the move: a block dragged together (F4).
    pub fn move_items(&mut self, p: &mut impl Player, uids: &[Uid], to: usize) {
        self.reconcile(p);
        let selected: HashSet<Uid> = uids.iter().copied().collect();
        let indices: Vec<usize> = (0..self.items.len())
            .filter(|&index| selected.contains(&self.items[index].uid))
            .collect();
        if indices.is_empty() {
            return;
        }
        let moving: Vec<Item> = indices
            .iter()
            .map(|&index| self.items[index].clone())
            .collect();
        let current_uid = self.current_item().map(|item| item.uid);
        self.items.retain(|item| !selected.contains(&item.uid));
        let to = to.min(self.items.len());
        // Sent as the block leaving and coming back; the store keeps the
        // moved items' places in the order before shuffling.
        for edit in removals(indices) {
            self.edited(edit);
        }
        self.edited(Edit::Insert {
            at: to,
            items: moving.clone(),
            original_at: None,
        });
        self.items.splice(to..to, moving);
        self.current = current_uid.and_then(|uid| self.index_of(uid));
        self.sync_next(p);
    }

    /// Empties the queue and stops playback.
    pub fn clear(&mut self, p: &mut impl Player) {
        self.reconcile(p);
        if let Some(index) = self.current.filter(|_| self.loaded) {
            self.record_position(index, p.position());
        }
        self.end_sleep(p);
        self.stop_after = None;
        let was_loaded = self.engine_has_queue();
        self.cancel_opening(p);
        if was_loaded {
            p.stop();
            let _ = p.set_next(None, false);
        }
        self.items.clear();
        if let Some(original) = &mut self.original {
            original.clear();
        }
        self.current = None;
        self.loaded = false;
        self.armed = None;
        self.resume_at = 0.0;
        self.unavailable.clear();
        self.radio = false;
        self.reset_list();
    }

    /// Marks the items of `track_ids` unavailable, their folders being out
    /// of reach (an unplugged drive, PLAN.md H22), and clears every other
    /// mark, so items that failed to open are tried again. The next track
    /// is armed again if it changed.
    pub fn set_unavailable_tracks(&mut self, p: &mut impl Player, track_ids: &HashSet<i64>) {
        self.reconcile(p);
        let unavailable: HashSet<Uid> = self
            .items
            .iter()
            .filter(|item| track_ids.contains(&item.track.track_id))
            .map(|item| item.uid)
            .collect();
        if unavailable != self.unavailable {
            self.unavailable = unavailable;
            self.changed = true;
        }
        self.sync_next(p);
    }

    /// Plays an item, trying it again if it couldn't be opened before.
    pub fn jump(&mut self, p: &mut impl Player, uid: Uid) {
        self.reconcile(p);
        if let Some(index) = self.index_of(uid) {
            self.unavailable.remove(&uid);
            self.leave_current(p);
            self.start(p, index, true, 0.0);
        }
    }

    /// Moves to the next item (wrapping with repeat all, and moving on even
    /// with repeat one), keeping playing or paused as it was.
    pub fn next(&mut self, p: &mut impl Player) {
        self.reconcile(p);
        let Some(current) = self.current else {
            return;
        };
        if let Some(index) = self.find(current, true, self.repeat == Repeat::All) {
            self.go_to(p, index);
        }
    }

    /// Restarts the track if it has played more than `RESTART_THRESHOLD`
    /// seconds, otherwise moves to the previous item (wrapping with repeat
    /// all; at the first item, restarts it).
    pub fn previous(&mut self, p: &mut impl Player) {
        self.reconcile(p);
        let Some(current) = self.current else {
            return;
        };
        let position = if self.loaded {
            p.position()
        } else {
            self.resume_at
        };
        let previous = self.find(current, false, self.repeat == Repeat::All);
        match previous {
            Some(index) if position <= RESTART_THRESHOLD => self.go_to(p, index),
            _ => self.seek(p, 0.0),
        }
    }

    pub fn play(&mut self, p: &mut impl Player) {
        self.reconcile(p);
        // Playing on through a sleep timer's fade, or after it ran out while
        // paused, ends it.
        if let Some(SleepTimer::At { ends_at, .. }) = self.sleep.map(|sleep| sleep.timer) {
            if self.clock >= ends_at - SLEEP_FADE {
                self.end_sleep(p);
            }
        }
        if self.loaded {
            p.play();
        } else if let Some(loading) = &mut self.loading {
            loading.play = true;
            self.changed = true;
        } else if let Some(current) = self.current {
            let resume_at = self.resume_at;
            self.start(p, current, true, resume_at);
        }
    }

    pub fn pause(&mut self, p: &mut impl Player) {
        self.reconcile(p);
        if let Some(loading) = &mut self.loading {
            loading.play = false;
            self.changed = true;
            // What played before it stops too.
            if loading.was_loaded {
                p.pause();
            }
        }
        if self.loaded {
            p.pause();
            if let Some(sleep) = &mut self.sleep {
                if let Some(volume) = sleep.fading_from.take() {
                    p.set_volume(volume);
                }
            }
            if let Some(index) = self.current {
                self.record_position(index, p.position());
            }
        }
    }

    pub fn toggle(&mut self, p: &mut impl Player) {
        if self.is_playing(p) {
            self.pause(p);
        } else {
            self.play(p);
        }
    }

    /// Seeks in the current track, or sets where it will start if it isn't
    /// loaded yet.
    pub fn seek(&mut self, p: &mut impl Player, seconds: f64) {
        self.reconcile(p);
        if self.loaded {
            p.seek(seconds);
        } else if self.current.is_some() && seconds.is_finite() {
            self.resume_at = seconds.max(0.0);
            self.changed = true;
        }
    }

    pub fn set_shuffle(&mut self, p: &mut impl Player, on: bool) {
        self.reconcile(p);
        if on == self.original.is_some() {
            return;
        }
        if on {
            self.original = Some(self.items.iter().map(|item| item.uid).collect());
            self.shuffle_from(self.current.map_or(0, |current| current + 1));
        } else if let Some(original) = self.original.take() {
            let current_uid = self.current_item().map(|item| item.uid);
            let mut by_uid: HashMap<Uid, Item> =
                self.items.drain(..).map(|item| (item.uid, item)).collect();
            self.items = original
                .iter()
                .filter_map(|uid| by_uid.remove(uid))
                .collect();
            self.items.extend(by_uid.into_values()); // None, if the invariant holds.
            self.current = current_uid.and_then(|uid| self.index_of(uid));
        }
        self.reset_list();
        self.sync_next(p);
    }

    pub fn set_repeat(&mut self, p: &mut impl Player, repeat: Repeat) {
        self.reconcile(p);
        self.repeat = repeat;
        self.changed = true;
        self.sync_next(p);
    }

    /// Stops after item `uid` (F13), or not with `None`.
    pub fn set_stop_after(&mut self, p: &mut impl Player, uid: Option<Uid>) {
        self.reconcile(p);
        self.stop_after = uid.filter(|&uid| self.index_of(uid).is_some());
        self.changed = true;
        self.sync_next(p);
    }

    /// Readies a take of item `uid` (the effects workbench, PLAN.md X8):
    /// playback will stop after it, and it waits paused at `start` for the
    /// recording to begin. Returns the item playback stopped after before,
    /// for `end_take`. Fails unless `uid` is the current item and open.
    pub fn ready_take(
        &mut self,
        p: &mut impl Player,
        uid: Uid,
        start: f64,
    ) -> Result<Option<Uid>, String> {
        self.reconcile(p);
        if self.current_item().map(|item| item.uid) != Some(uid) || !self.loaded {
            return Err("The file isn't playing".into());
        }
        let previous = self.stop_after;
        self.set_stop_after(p, Some(uid));
        self.pause(p);
        self.seek(p, start);
        Ok(previous)
    }

    /// A take of item `uid` ended: puts back `previous` as the item to stop
    /// after, unless the user chose another meanwhile.
    pub fn end_take(&mut self, p: &mut impl Player, uid: Uid, previous: Option<Uid>) {
        let previous = previous.filter(|&previous| previous != uid);
        let ours = self.stop_after.is_none_or(|stop| stop == uid);
        if ours && self.stop_after != previous {
            self.set_stop_after(p, previous);
        }
    }

    /// Starts a sleep timer, or ends it with `None` (restoring the volume
    /// if it was fading).
    pub fn set_sleep(&mut self, p: &mut impl Player, timer: Option<SleepTimer>) {
        self.reconcile(p);
        self.end_sleep(p);
        self.sleep = timer.map(|timer| Sleep {
            timer,
            fading_from: None,
        });
        self.changed = true;
        self.sync_next(p);
    }

    /// The host's clock, in seconds, which sleep timers are set by; call
    /// before each command.
    pub fn set_clock(&mut self, now: f64) {
        self.clock = now;
    }

    /// The user set the volume: a sleep timer's fade carries on from it.
    pub fn volume_changed(&mut self) {
        if let Some(sleep) = &mut self.sleep {
            sleep.fading_from = None;
        }
    }

    /// Runs a sleep timer while playing: fades the volume out over its last
    /// `SLEEP_FADE` seconds, then pauses, puts the volume back and ends it.
    /// Call often (on each position report); `now` as for `set_clock`.
    pub fn tick(&mut self, p: &mut impl Player, now: f64) {
        self.clock = now;
        self.reconcile(p);
        let Some(sleep) = self.sleep else {
            return;
        };
        let SleepTimer::At { ends_at, .. } = sleep.timer else {
            return;
        };
        if !self.loaded || p.state() != PlayerState::Playing {
            return;
        }
        let left = ends_at - now;
        if left > SLEEP_FADE {
            return;
        }
        let from = sleep.fading_from.unwrap_or_else(|| p.volume());
        if left <= 0.0 {
            p.pause();
            p.set_volume(from);
            self.sleep = None;
            self.changed = true;
            if let Some(index) = self.current {
                self.record_position(index, p.position());
            }
        } else {
            p.set_volume(from * left / SLEEP_FADE);
            self.sleep = Some(Sleep {
                fading_from: Some(from),
                ..sleep
            });
        }
    }

    /// Long tracks' positions to save since the last call: `None` forgets
    /// one (played to the end, or near either end).
    pub fn take_positions(&mut self) -> Vec<(i64, Option<f64>)> {
        std::mem::take(&mut self.positions)
    }

    /// Notes where the current track is, if it's long, e.g. at quit.
    pub fn remember_position(&mut self, p: &impl Player) {
        self.leave_current(p);
    }

    // ---- Engine events ----------------------------------------------------

    /// The engine reported `TrackEnded`.
    pub fn on_track_ended(&mut self, p: &mut impl Player, advanced: bool) {
        self.reconcile(p);
        if !self.loaded {
            return;
        }
        if !advanced && p.state() == PlayerState::Stopped {
            if let Some(current) = self.current {
                // Played to its end: a long track starts at the start again.
                let duration = self.items[current].track.duration;
                self.record_position(current, duration);
                if self.stops_after(current) {
                    // Stopped as asked: the next item waits, paused.
                    self.finish_stop(current);
                    if let Some(index) = self.following(current) {
                        self.start(p, index, false, 0.0);
                    }
                    return;
                }
            }
            // Nothing was armed when the track ended, but something may
            // follow it now (an edit too late for a gapless hand-off).
            if let Some(index) = self.current.and_then(|current| self.following(current)) {
                self.start(p, index, true, 0.0);
                return;
            }
        }
        self.sync_next(p);
    }

    /// Something other than the queue was loaded into the engine (the dev
    /// page's typed paths); the queue stops following it.
    pub fn detach(&mut self) {
        // The engine cancelled what was opening when the other track loaded.
        if self.loading.take().is_some() {
            self.changed = true;
        }
        self.arming = None;
        if self.loaded {
            self.loaded = false;
            self.armed = None;
            self.resume_at = 0.0;
            self.changed = true;
        }
    }

    /// The host reports a request `Player::load` or `Player::set_next`
    /// left pending: on success the track is in place; on failure it is
    /// passed over, as one that failed at once is. Requests the queue has
    /// moved on from are ignored.
    pub fn load_finished(
        &mut self,
        p: &mut impl Player,
        request: Request,
        result: Result<(), String>,
    ) {
        self.reconcile(p);
        if self
            .loading
            .as_ref()
            .is_some_and(|loading| loading.request == request)
        {
            let Some(loading) = self.loading.take() else {
                return;
            };
            self.changed = true;
            let Some(index) = self.index_of(loading.uid) else {
                // Removed while it opened (which cancels it): never reported.
                return;
            };
            match result {
                Ok(()) => {
                    let resume_at = self.resume_at;
                    self.started(p, index, loading.play, resume_at);
                }
                Err(error) => {
                    self.mark_unavailable(index, error);
                    let candidate = self.find(index, true, self.repeat == Repeat::All);
                    self.try_start(p, candidate, &loading);
                }
            }
        } else if self.arming.is_some_and(|arming| arming.request == request) {
            let Some(arming) = self.arming.take() else {
                return;
            };
            match result {
                Ok(()) => {
                    self.armed = Some(arming.uid);
                    self.changed = true;
                }
                Err(error) => {
                    if let Some(index) = self.index_of(arming.uid) {
                        self.mark_unavailable(index, error);
                    }
                    self.sync_next(p);
                }
            }
        }
    }

    /// The host found request `request` to be a cloud placeholder (H12),
    /// which opens only once downloaded: it is shown as downloading, and
    /// given `DOWNLOAD_TIMEOUT`.
    pub fn downloading(&mut self, request: Request) {
        if let Some(loading) = self.loading.as_mut().filter(|l| l.request == request) {
            loading.downloading = true;
            self.changed = true;
        }
        if let Some(arming) = self.arming.as_mut().filter(|a| a.request == request) {
            arming.downloading = true;
        }
    }

    /// When a request still opening should be given up on (`check_loads`),
    /// by the host's clock; `None` if nothing is opening.
    pub fn load_deadline(&self) -> Option<f64> {
        let loading = self
            .loading
            .as_ref()
            .map(|l| l.since + timeout(l.downloading));
        let arming = self.arming.map(|a| a.since + timeout(a.downloading));
        loading.into_iter().chain(arming).reduce(f64::min)
    }

    /// Fails requests that have been opening for longer than their timeout
    /// (`LOAD_TIMEOUT`, or `DOWNLOAD_TIMEOUT` for a placeholder), cancelling
    /// them; `now` as for `set_clock`.
    pub fn check_loads(&mut self, p: &mut impl Player, now: f64) {
        self.clock = now;
        let overdue =
            |(_, since, downloading): &(Request, f64, bool)| now - since >= timeout(*downloading);
        let loading = self
            .loading
            .as_ref()
            .map(|l| (l.request, l.since, l.downloading));
        if let Some((request, _, downloading)) = loading.filter(overdue) {
            p.cancel(request);
            self.load_finished(p, request, Err(open_timed_out(timeout(downloading))));
        }
        let arming = self.arming.map(|a| (a.request, a.since, a.downloading));
        if let Some((request, _, downloading)) = arming.filter(overdue) {
            p.cancel(request);
            self.load_finished(p, request, Err(open_timed_out(timeout(downloading))));
        }
    }

    // ---- Internals --------------------------------------------------------

    /// Whether playback stops when item `index` ends: it's the item to stop
    /// after, or a sleep timer ends with it (its track, or its album).
    fn stops_after(&self, index: usize) -> bool {
        let item = &self.items[index];
        if self.stop_after == Some(item.uid) {
            return true;
        }
        match self.sleep.map(|sleep| sleep.timer) {
            Some(SleepTimer::EndOfTrack) => true,
            Some(SleepTimer::EndOfAlbum) => self.following(index).is_none_or(|next| {
                item.track.album_id.is_none()
                    || self.items[next].track.album_id != item.track.album_id
            }),
            _ => false,
        }
    }

    /// Playback stopped after item `index`, as asked: that's done.
    fn finish_stop(&mut self, index: usize) {
        if self.stop_after == Some(self.items[index].uid) {
            self.stop_after = None;
        }
        if matches!(
            self.sleep.map(|sleep| sleep.timer),
            Some(SleepTimer::EndOfTrack | SleepTimer::EndOfAlbum)
        ) {
            self.sleep = None;
        }
        self.changed = true;
    }

    /// Ends the sleep timer, putting back the volume a fade started from.
    fn end_sleep(&mut self, p: &mut impl Player) {
        if let Some(sleep) = self.sleep.take() {
            if let Some(volume) = sleep.fading_from {
                p.set_volume(volume);
            }
            self.changed = true;
        }
    }

    /// Notes where the loaded current item is being left, if it's long.
    fn leave_current(&mut self, p: &impl Player) {
        if let Some(index) = self.current.filter(|_| self.loaded) {
            self.record_position(index, p.position());
        }
    }

    /// Notes that item `index` was left `position` seconds in, if it's a
    /// long library track: kept away from its ends, else forgotten.
    fn record_position(&mut self, index: usize, position: f64) {
        let track = &self.items[index].track;
        if !track.is_long() || track.external {
            return;
        }
        let track_id = track.track_id;
        let keep = position > LONG_TRACK_MARGIN && position < track.duration - LONG_TRACK_MARGIN;
        let value = keep.then_some(position);
        if track.resume == value {
            return;
        }
        for item in &mut self.items {
            if item.track.track_id == track_id {
                item.track.resume = value;
            }
        }
        self.positions.push((track_id, value));
    }

    /// Notes an edit for the frontend and the store.
    fn edited(&mut self, edit: Edit) {
        self.stored.push(edit.clone());
        self.sent.push(edit);
    }

    /// The list was replaced: both sides get it whole.
    fn reset_list(&mut self) {
        self.sent = ListLog::Reset;
        self.stored = ListLog::Reset;
    }

    fn new_item(&mut self, track: TrackInfo) -> Item {
        let uid = self.next_uid;
        self.next_uid += 1;
        Item { uid, track }
    }

    fn index_of(&self, uid: Uid) -> Option<usize> {
        self.items.iter().position(|item| item.uid == uid)
    }

    /// Applies hand-offs the engine made that the queue hasn't accounted for
    /// yet. `TrackEnded` reports one up to 50 ms late, so every command calls
    /// this first; otherwise a command in that window would work from the
    /// track that just finished.
    fn reconcile(&mut self, p: &mut impl Player) {
        if !self.loaded {
            return;
        }
        let count = p.advance_count();
        if self.seen_advances < count {
            match self.armed.take().and_then(|uid| self.index_of(uid)) {
                Some(index) => {
                    // The one before played to its end.
                    if let Some(previous) = self.current {
                        let duration = self.items[previous].track.duration;
                        self.record_position(previous, duration);
                    }
                    self.current = Some(index);
                    self.changed = true;
                }
                // The engine is playing something the queue no longer has.
                None => self.detach(),
            }
        }
        self.seen_advances = count;
    }

    /// The item after `from` that is not known to be unavailable, going
    /// forward or back, wrapping around if `wrap` (to `from` itself last).
    fn find(&self, from: usize, forward: bool, wrap: bool) -> Option<usize> {
        let n = self.items.len();
        (1..=n)
            .map_while(|step| {
                let index = if forward {
                    from + step
                } else {
                    from.wrapping_sub(step)
                };
                if index < n {
                    Some(index)
                } else if wrap {
                    Some(if forward {
                        index - n
                    } else {
                        index.wrapping_add(n)
                    })
                } else {
                    None
                }
            })
            .find(|&index| {
                let item = &self.items[index];
                !self.unavailable.contains(&item.uid) && !item.track.skip
            })
    }

    /// What plays after `from` when it ends.
    fn following(&self, from: usize) -> Option<usize> {
        match self.repeat {
            // A skipped track chosen directly repeats too.
            Repeat::One => (!self.unavailable.contains(&self.items[from].uid)).then_some(from),
            Repeat::All => self.find(from, true, true),
            Repeat::Off => self.find(from, true, false),
        }
    }

    /// Moves to `index`: loaded and keeping the play state if the queue is
    /// loaded, otherwise just selected.
    fn go_to(&mut self, p: &mut impl Player, index: usize) {
        self.leave_current(p);
        if self.loaded || self.loading.is_some() {
            let play = self.is_playing(p);
            self.start(p, index, play, 0.0);
        } else {
            self.current = Some(index);
            self.resume_at = 0.0;
            self.changed = true;
        }
    }

    /// Loads `index` (or, if it can't be opened, the next item that can),
    /// seeks to `resume_at`, plays if `play`, and arms the following item.
    /// With a load pending, all that happens when it's done.
    fn start(&mut self, p: &mut impl Player, index: usize, play: bool, resume_at: f64) {
        self.changed = true;
        let was_loaded = self.engine_has_queue();
        self.cancel_opening(p);
        let asked = Loading {
            request: 0,
            uid: self.items[index].uid,
            play,
            since: self.clock,
            asked: self.items[index].uid,
            asked_resume_at: resume_at,
            was_loaded,
            downloading: false,
        };
        self.try_start(p, Some(index), &asked);
    }

    /// `start` from `candidate` on, for what `asked` asked for.
    fn try_start(&mut self, p: &mut impl Player, mut candidate: Option<usize>, asked: &Loading) {
        let play = asked.play;
        while let Some(index) = candidate {
            let item = &self.items[index];
            let uid = item.uid;
            // A long track carries on where it was left.
            let resume_at = match item.track.resume {
                Some(saved) if asked.asked_resume_at <= 0.0 && item.track.is_long() => saved,
                _ => asked.asked_resume_at,
            };
            match p.load(item.track.track_id) {
                Opening::Done(Ok(())) => return self.started(p, index, play, resume_at),
                Opening::Done(Err(error)) => {
                    self.mark_unavailable(index, error);
                    candidate = self.find(index, true, self.repeat == Repeat::All);
                }
                Opening::Pending(request) => {
                    self.current = Some(index);
                    self.loaded = false;
                    self.armed = None;
                    self.resume_at = resume_at;
                    self.loading = Some(Loading {
                        request,
                        uid,
                        since: self.clock,
                        downloading: false,
                        ..asked.clone()
                    });
                    self.changed = true;
                    return;
                }
            }
        }
        // Nothing could be opened: it stays where it was asked to start.
        if asked.was_loaded {
            p.stop();
        }
        self.current = self.index_of(asked.asked).or(self.current);
        self.loaded = false;
        self.armed = None;
        self.resume_at = asked.asked_resume_at;
        self.changed = true;
    }

    /// Item `index` is open in the engine: seeks to `resume_at`, plays if
    /// `play`, and arms the following item.
    fn started(&mut self, p: &mut impl Player, index: usize, play: bool, resume_at: f64) {
        let uid = self.items[index].uid;
        self.current = Some(index);
        self.loaded = true;
        self.armed = None;
        self.seen_advances = p.advance_count();
        self.resume_at = 0.0;
        self.unavailable.remove(&uid);
        self.changed = true;
        if resume_at > 0.0 {
            p.seek(resume_at);
        }
        if play {
            p.play();
        }
        self.sync_next(p);
    }

    /// Whether the engine has a track of the queue's, or had one when the
    /// load pending was asked for (it plays on until that's ready).
    fn engine_has_queue(&self) -> bool {
        self.loaded
            || self
                .loading
                .as_ref()
                .is_some_and(|loading| loading.was_loaded)
    }

    /// Whether the queue is playing, or will once its pending load is ready.
    fn is_playing(&self, p: &impl Player) -> bool {
        match &self.loading {
            Some(loading) => loading.play,
            None => self.loaded && p.state() == PlayerState::Playing,
        }
    }

    /// Gives up on the requests still opening.
    fn cancel_opening(&mut self, p: &mut impl Player) {
        if let Some(loading) = self.loading.take() {
            p.cancel(loading.request);
        }
        if let Some(arming) = self.arming.take() {
            p.cancel(arming.request);
        }
    }

    /// Arms the engine with the item that should follow the current one,
    /// unless it already is; items that can't be opened are skipped.
    fn sync_next(&mut self, p: &mut impl Player) {
        self.changed = true;
        let Some(current) = self.current.filter(|_| self.loaded) else {
            return;
        };
        let current = self.next_pass(current);
        let stopping = self.stops_after(current);
        loop {
            let candidate = self.following(current).filter(|_| !stopping);
            let uid = candidate.map(|index| self.items[index].uid);
            match self.arming {
                // Already being opened.
                Some(arming) if Some(arming.uid) == uid => return,
                Some(arming) => {
                    self.arming = None;
                    p.cancel(arming.request);
                }
                None if uid == self.armed => return,
                None => {}
            }
            let track = candidate.map(|index| self.items[index].track.track_id);
            let crossfade = candidate.is_some_and(|index| {
                crossfades(&self.items[current].track, &self.items[index].track)
            });
            match (p.set_next(track, crossfade), candidate) {
                (Opening::Done(Ok(())), _) => {
                    self.armed = uid;
                    return;
                }
                (Opening::Done(Err(error)), Some(index)) => self.mark_unavailable(index, error),
                (Opening::Done(Err(_)), None) => {
                    // Clearing can't fail on a loaded engine; forget it.
                    self.armed = None;
                    return;
                }
                (Opening::Pending(request), Some(index)) => {
                    self.armed = None;
                    self.arming = Some(Arming {
                        request,
                        uid: self.items[index].uid,
                        since: self.clock,
                        downloading: false,
                    });
                    return;
                }
                (Opening::Pending(request), None) => {
                    // Clearing is never pending.
                    p.cancel(request);
                    self.armed = None;
                    return;
                }
            }
        }
    }

    fn mark_unavailable(&mut self, index: usize, error: String) {
        let item = &self.items[index];
        self.unavailable.insert(item.uid);
        self.skipped.push(Skipped {
            uid: item.uid,
            track_id: item.track.track_id,
            title: item.track.title.clone(),
            error,
        });
        self.changed = true;
    }

    /// With shuffle and repeat all, when the last item is current, draws the
    /// next pass: the item moves to the top and the others are shuffled
    /// after it, so every pass plays in a new order. The track that played
    /// just before doesn't come straight back. Returns the current index.
    fn next_pass(&mut self, current: usize) -> usize {
        let n = self.items.len();
        if self.original.is_none() || self.repeat != Repeat::All || n < 2 || current + 1 != n {
            return current;
        }
        let just_played = current.checked_sub(1).map(|index| self.items[index].uid);
        let item = self.items.remove(current);
        self.items.insert(0, item);
        self.shuffle_from(1);
        if n > 2 && Some(self.items[1].uid) == just_played && self.items[1].track.unit.is_none() {
            let other = 2 + (self.random() % (n as u64 - 2)) as usize;
            if self.items[other].track.unit.is_none() {
                self.items.swap(1, other);
            }
        }
        self.current = Some(0);
        self.reset_list();
        0
    }

    /// The end (exclusive) of the run of items with `index`'s unit that
    /// starts at `index`.
    fn run_end(&self, index: usize) -> usize {
        let unit = self.items[index].track.unit;
        let mut end = index + 1;
        while unit.is_some() && end < self.items.len() && self.items[end].track.unit == unit {
            end += 1;
        }
        end
    }

    /// Shuffles the items from `from` on (Fisher–Yates over runs, so
    /// adjacent items with one unit stay together, in order).
    fn shuffle_from(&mut self, from: usize) {
        let mut runs: Vec<Vec<Item>> = Vec::new();
        for item in self.items.drain(from.min(self.items.len())..) {
            match runs.last_mut() {
                Some(run) if item.track.unit.is_some() && run[0].track.unit == item.track.unit => {
                    run.push(item)
                }
                _ => runs.push(vec![item]),
            }
        }
        for i in (1..runs.len()).rev() {
            let j = (self.random() % (i as u64 + 1)) as usize;
            runs.swap(i, j);
        }
        self.items.extend(runs.into_iter().flatten());
    }

    /// xorshift64*: plenty for shuffling, and reproducible in tests.
    fn random(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}

/// Seconds a request may take to open: longer for a cloud placeholder.
fn timeout(downloading: bool) -> f64 {
    if downloading {
        DOWNLOAD_TIMEOUT
    } else {
        LOAD_TIMEOUT
    }
}

/// Whether a hand-off from `from` to `to` may crossfade: not between
/// tracks of one album, nor within a unit (a segue, a work), which stay
/// gapless, nor into the same track again.
fn crossfades(from: &TrackInfo, to: &TrackInfo) -> bool {
    let same_album = from.album_id.is_some() && from.album_id == to.album_id;
    let same_unit = from.unit.is_some() && from.unit == to.unit;
    !same_album && !same_unit && from.track_id != to.track_id
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An engine that plays instantly: `finish` ends the current track.
    #[derive(Default)]
    struct Fake {
        current: Option<i64>,
        next: Option<i64>,
        /// Whether the next track was set to crossfade.
        crossfade: bool,
        state: Option<PlayerState>,
        position: f64,
        advances: i64,
        unreadable: HashSet<i64>,
        loads: Vec<i64>,
        /// None until set: 1.
        volume: Option<f64>,
        /// Opens on a thread of its own: requests wait in `requests` until
        /// `complete` (H11).
        pending: bool,
        requests: Vec<(Request, i64, bool)>,
        cancelled: Vec<Request>,
        last_request: Request,
    }

    impl Fake {
        /// Plays the current track to its end, as the audio thread does;
        /// returns `advanced` for the `TrackEnded` the host sees later.
        fn finish(&mut self) -> bool {
            self.position = 0.0;
            match self.next.take() {
                Some(next) => {
                    self.current = Some(next);
                    self.advances += 1;
                    true
                }
                None => {
                    self.state = Some(PlayerState::Stopped);
                    false
                }
            }
        }
    }

    impl Fake {
        fn open_now(&mut self, track_id: i64) -> Result<(), String> {
            if self.unreadable.contains(&track_id) {
                return Err(format!("cannot open {track_id}"));
            }
            self.loads.push(track_id);
            self.current = Some(track_id);
            self.next = None;
            self.state = Some(PlayerState::Stopped);
            self.position = 0.0;
            Ok(())
        }

        fn arm_now(&mut self, track_id: Option<i64>, crossfade: bool) -> Result<(), String> {
            if let Some(id) = track_id.filter(|id| self.unreadable.contains(id)) {
                return Err(format!("cannot open {id}"));
            }
            self.next = track_id;
            self.crossfade = crossfade && track_id.is_some();
            Ok(())
        }

        fn ask(&mut self, track_id: i64, next: bool) -> Opening {
            self.last_request += 1;
            self.requests.push((self.last_request, track_id, next));
            Opening::Pending(self.last_request)
        }

        /// Finishes the oldest request still opening, as the engine would
        /// (the file opened, or not), and reports it; returns its track.
        fn complete(&mut self, queue: &mut Queue) -> i64 {
            let (request, track_id, next) = self.requests.remove(0);
            let result = if next {
                self.arm_now(Some(track_id), false)
            } else {
                self.open_now(track_id)
            };
            queue.load_finished(self, request, result);
            track_id
        }

        /// The tracks still opening.
        fn opening(&self) -> Vec<i64> {
            self.requests
                .iter()
                .map(|&(_, track_id, _)| track_id)
                .collect()
        }
    }

    impl Player for Fake {
        fn load(&mut self, track_id: i64) -> Opening {
            if self.pending {
                // A load supersedes every request before it, as the engine's does.
                self.requests.clear();
                return self.ask(track_id, false);
            }
            Opening::Done(self.open_now(track_id))
        }
        fn set_next(&mut self, track_id: Option<i64>, crossfade: bool) -> Opening {
            match track_id {
                Some(id) if self.pending => {
                    self.next = None;
                    self.requests.retain(|&(_, _, next)| !next);
                    self.ask(id, true)
                }
                _ => Opening::Done(self.arm_now(track_id, crossfade)),
            }
        }
        fn cancel(&mut self, request: Request) {
            self.cancelled.push(request);
            self.requests.retain(|&(r, _, _)| r != request);
        }
        fn volume(&self) -> f64 {
            self.volume.unwrap_or(1.0)
        }
        fn set_volume(&mut self, volume: f64) {
            self.volume = Some(volume);
        }
        fn play(&mut self) -> bool {
            self.state = Some(PlayerState::Playing);
            self.current.is_some()
        }
        fn pause(&mut self) {
            if self.state == Some(PlayerState::Playing) {
                self.state = Some(PlayerState::Paused);
            }
        }
        fn stop(&mut self) {
            self.state = Some(PlayerState::Stopped);
            self.position = 0.0;
        }
        fn seek(&mut self, seconds: f64) -> bool {
            self.position = seconds;
            true
        }
        fn state(&self) -> PlayerState {
            self.state.unwrap_or(PlayerState::Empty)
        }
        fn position(&self) -> f64 {
            self.position
        }
        fn advance_count(&self) -> i64 {
            self.advances
        }
    }

    fn info(id: i64) -> TrackInfo {
        TrackInfo {
            track_id: id,
            title: format!("Track {id}"),
            duration: 60.0,
            ..TrackInfo::default()
        }
    }

    fn infos(ids: impl IntoIterator<Item = i64>) -> Vec<TrackInfo> {
        ids.into_iter().map(info).collect()
    }

    fn ids(queue: &Queue) -> Vec<i64> {
        queue.items.iter().map(|item| item.track.track_id).collect()
    }

    fn current_id(queue: &Queue) -> Option<i64> {
        queue.current_item().map(|item| item.track.track_id)
    }

    fn uid_of(queue: &Queue, track_id: i64) -> Uid {
        queue
            .items
            .iter()
            .find(|item| item.track.track_id == track_id)
            .unwrap()
            .uid
    }

    /// Ends the engine's track and delivers the event.
    fn end_track(queue: &mut Queue, p: &mut Fake) {
        let advanced = p.finish();
        queue.on_track_ended(p, advanced);
    }

    /// Checks that the engine plays what the queue says, with the right next.
    fn check(queue: &Queue, p: &Fake) {
        assert!(queue.loaded);
        assert_eq!(p.current, current_id(queue), "engine's current track");
        let expected = queue
            .current
            .and_then(|current| queue.following(current))
            .map(|index| queue.items[index].track.track_id);
        assert_eq!(p.next, expected, "engine's next track");
        if let Some(original) = &queue.original {
            let mut a: Vec<Uid> = original.clone();
            let mut b: Vec<Uid> = queue.items.iter().map(|item| item.uid).collect();
            a.sort();
            b.sort();
            assert_eq!(a, b, "the original order has every item once");
        }
    }

    #[test]
    fn advances_through_the_queue_with_the_next_track_armed() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3]), 0, true);
        assert_eq!(p.state(), PlayerState::Playing);
        check(&queue, &p);
        assert_eq!(p.next, Some(2));

        end_track(&mut queue, &mut p);
        check(&queue, &p);
        assert_eq!((current_id(&queue), p.next), (Some(2), Some(3)));
        end_track(&mut queue, &mut p);
        assert_eq!((current_id(&queue), p.next), (Some(3), None));

        // The end of the queue: stopped on the last track.
        end_track(&mut queue, &mut p);
        assert_eq!(current_id(&queue), Some(3));
        assert_eq!(p.state(), PlayerState::Stopped);
        assert_eq!(p.loads, [1], "every change after the first was a hand-off");

        let state = queue.state();
        assert_eq!(state.current, Some(2));
        assert!(!state.has_next && state.has_previous);
    }

    #[test]
    fn names_the_tracks_the_engine_has_open() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        assert!(queue.engine_track_ids().is_empty());
        queue.replace(&mut p, infos([1, 2, 3]), 0, true);
        assert_eq!(queue.engine_track_ids(), [1, 2]);
        end_track(&mut queue, &mut p);
        end_track(&mut queue, &mut p);
        assert_eq!(queue.engine_track_ids(), [3]);
        queue.set_repeat(&mut p, Repeat::One);
        assert_eq!(queue.engine_track_ids(), [3, 3]);
        queue.detach();
        assert!(queue.engine_track_ids().is_empty());
    }

    #[test]
    fn starts_where_asked() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3]), 1, true);
        assert_eq!((p.current, p.next), (Some(2), Some(3)));
        queue.replace(&mut p, infos([7, 8]), 99, false);
        assert_eq!((p.current, p.next), (Some(8), None));
        assert_eq!(p.state(), PlayerState::Stopped);
    }

    #[test]
    fn repeat_modes() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3]), 0, true);

        queue.set_repeat(&mut p, Repeat::One);
        check(&queue, &p);
        assert_eq!(p.next, Some(1));
        end_track(&mut queue, &mut p);
        end_track(&mut queue, &mut p);
        assert_eq!(current_id(&queue), Some(1));
        assert_eq!(p.next, Some(1));

        // Next and previous move on even with repeat one.
        queue.next(&mut p);
        assert_eq!((p.current, p.next), (Some(2), Some(2)));

        queue.set_repeat(&mut p, Repeat::All);
        queue.next(&mut p);
        check(&queue, &p);
        assert_eq!(
            (p.current, p.next),
            (Some(3), Some(1)),
            "wraps to the start"
        );
        end_track(&mut queue, &mut p);
        assert_eq!((current_id(&queue), p.next), (Some(1), Some(2)));
        queue.previous(&mut p);
        assert_eq!(current_id(&queue), Some(3), "previous wraps too");

        queue.set_repeat(&mut p, Repeat::Off);
        check(&queue, &p);
        assert_eq!(p.next, None);
        queue.next(&mut p);
        assert_eq!(current_id(&queue), Some(3), "no next at the end");

        // A single track with repeat all plays again.
        queue.replace(&mut p, infos([9]), 0, true);
        queue.set_repeat(&mut p, Repeat::All);
        assert_eq!(p.next, Some(9));
    }

    #[test]
    fn shuffle_order_is_stable() {
        let mut p = Fake::default();
        let mut queue = Queue::new(42);
        queue.set_shuffle(&mut p, true);
        queue.replace(&mut p, infos(1..=20), 4, true);
        assert_eq!(p.current, Some(5), "the chosen track plays first");
        assert_eq!(queue.current, Some(0));
        let order = ids(&queue);
        assert_ne!(order, (1..=20).collect::<Vec<_>>());
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(sorted, (1..=20).collect::<Vec<_>>());
        check(&queue, &p);

        // Playing through keeps the order.
        let mut played = vec![p.current.unwrap()];
        for _ in 0..10 {
            end_track(&mut queue, &mut p);
            check(&queue, &p);
            played.push(p.current.unwrap());
        }
        assert_eq!(played[..], order[..11]);
        assert_eq!(ids(&queue), order);

        // Saved and restored, it's the same order.
        let saved = queue.saved(12.5);
        let json = serde_json::to_string(&saved).unwrap();
        let tracks: HashMap<i64, TrackInfo> = (1..=20).map(|id| (id, info(id))).collect();
        let mut restored = Queue::restore(serde_json::from_str(&json).unwrap(), &tracks, 7);
        assert_eq!(ids(&restored), order);

        // Off restores the original order, keeping the current track.
        let now = current_id(&queue);
        queue.set_shuffle(&mut p, false);
        assert_eq!(ids(&queue), (1..=20).collect::<Vec<_>>());
        assert_eq!(current_id(&queue), now);
        check(&queue, &p);

        // The restored queue unshuffles the same way.
        restored.set_shuffle(&mut Fake::default(), false);
        assert_eq!(ids(&restored), (1..=20).collect::<Vec<_>>());
        assert_eq!(current_id(&restored), now);

        // Turning it on again shuffles only what comes after.
        queue.jump(&mut p, uid_of(&queue, 10));
        queue.set_shuffle(&mut p, true);
        assert_eq!(ids(&queue)[..10], (1..=10).collect::<Vec<_>>()[..]);
        check(&queue, &p);
    }

    #[test]
    fn repeat_all_with_shuffle_reshuffles_each_pass() {
        let mut p = Fake::default();
        let mut queue = Queue::new(9);
        queue.set_shuffle(&mut p, true);
        queue.set_repeat(&mut p, Repeat::All);
        queue.replace(&mut p, infos(1..=20), 0, true);
        let first_pass = ids(&queue);

        let mut played = vec![p.current.unwrap()];
        for _ in 0..80 {
            end_track(&mut queue, &mut p);
            check(&queue, &p);
            played.push(p.current.unwrap());
        }
        assert_eq!(played[..20], first_pass[..]);
        // Each pass plays all 20 once, in a new order. A pass starts with the
        // track that ended the one before, which moved to the top as it
        // began, so passes overlap by that one play.
        let passes: Vec<Vec<i64>> = (0..4)
            .map(|k| played[19 * k..19 * k + 20].to_vec())
            .collect();
        for pass in &passes {
            let mut sorted = pass.clone();
            sorted.sort();
            assert_eq!(sorted, (1..=20).collect::<Vec<_>>());
        }
        assert_ne!(passes[1][1..], passes[0][1..]);
        assert_ne!(passes[2][1..], passes[1][1..]);
        for pair in played.windows(3) {
            assert_ne!(
                pair[0], pair[2],
                "the track before the last doesn't come straight back"
            );
        }

        // Jumping to the last item draws a new pass: it moves to the top.
        let last = queue.items[19].uid;
        queue.jump(&mut p, last);
        assert_eq!(queue.current, Some(0));
        assert_eq!(queue.items[0].uid, last);
        check(&queue, &p);

        // Edits don't draw it again.
        let order = ids(&queue);
        queue.set_repeat(&mut p, Repeat::All);
        queue.add(&mut p, infos([99]), false);
        assert_eq!(ids(&queue)[..20], order[..]);
        check(&queue, &p);

        // Two tracks alternate; with repeat off nothing is redrawn.
        queue.replace(&mut p, infos([1, 2]), 0, true);
        for expected in [2, 1, 2, 1] {
            end_track(&mut queue, &mut p);
            assert_eq!(p.current, Some(expected));
        }
        queue.set_repeat(&mut p, Repeat::Off);
        queue.replace(&mut p, infos(1..=5), 0, true);
        let order = ids(&queue);
        for _ in 0..4 {
            end_track(&mut queue, &mut p);
        }
        assert_eq!(ids(&queue), order);
        assert_eq!(p.next, None);
    }

    #[test]
    fn items_added_while_shuffled_keep_their_place_when_unshuffled() {
        let mut p = Fake::default();
        let mut queue = Queue::new(3);
        queue.replace(&mut p, infos([1, 2, 3, 4]), 0, true);
        queue.set_shuffle(&mut p, true);
        queue.add(&mut p, infos([50]), true);
        assert_eq!(ids(&queue)[1], 50);
        assert_eq!(p.next, Some(50));
        queue.add(&mut p, infos([60]), false);
        assert_eq!(*ids(&queue).last().unwrap(), 60);
        check(&queue, &p);
        queue.set_shuffle(&mut p, false);
        assert_eq!(ids(&queue), [1, 50, 2, 3, 4, 60]);
    }

    #[test]
    fn previous_restarts_a_track_that_has_played_a_while() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3]), 1, true);
        p.position = 3.5;
        queue.previous(&mut p);
        assert_eq!((current_id(&queue), p.position), (Some(2), 0.0));
        assert_eq!(p.loads, [2], "restarted by seeking, not reloading");

        p.position = 2.0;
        queue.previous(&mut p);
        assert_eq!(current_id(&queue), Some(1));
        check(&queue, &p);
        assert_eq!(p.state(), PlayerState::Playing);

        // At the first track, previous restarts it.
        p.position = 1.0;
        queue.previous(&mut p);
        assert_eq!((current_id(&queue), p.position), (Some(1), 0.0));
    }

    #[test]
    fn next_and_previous_keep_a_paused_player_paused() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3]), 0, true);
        queue.pause(&mut p);
        queue.next(&mut p);
        assert_eq!(p.current, Some(2));
        assert_ne!(p.state(), PlayerState::Playing);
        queue.toggle(&mut p);
        assert_eq!(p.state(), PlayerState::Playing);
        queue.toggle(&mut p);
        assert_eq!(p.state(), PlayerState::Paused);
    }

    #[test]
    fn edits_while_playing_rearm_the_next_track() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3, 4]), 0, true);
        assert_eq!(p.next, Some(2));

        queue.move_item(&mut p, uid_of(&queue, 4), 1);
        assert_eq!(ids(&queue), [1, 4, 2, 3]);
        check(&queue, &p);

        queue.remove(&mut p, &[uid_of(&queue, 4)]);
        check(&queue, &p);
        assert_eq!(p.next, Some(2));

        queue.add(&mut p, infos([9]), true);
        assert_eq!(ids(&queue), [1, 9, 2, 3]);
        check(&queue, &p);

        // Moving the current item keeps it current.
        queue.move_item(&mut p, uid_of(&queue, 1), 3);
        assert_eq!(ids(&queue), [9, 2, 3, 1]);
        assert_eq!(current_id(&queue), Some(1));
        check(&queue, &p);
        assert_eq!(p.next, None);

        queue.add(&mut p, infos([10]), false);
        check(&queue, &p);
        assert_eq!(p.next, Some(10));

        // Removing the playing item plays the one after it.
        queue.remove(&mut p, &[uid_of(&queue, 1)]);
        assert_eq!(current_id(&queue), Some(10));
        assert_eq!(p.state(), PlayerState::Playing);
        check(&queue, &p);

        // Removing the last one falls back without playing.
        queue.remove(&mut p, &[uid_of(&queue, 10)]);
        assert_eq!(current_id(&queue), Some(3));
        assert_ne!(p.state(), PlayerState::Playing);

        queue.clear(&mut p);
        assert_eq!(queue.current, None);
        assert_eq!(p.state(), PlayerState::Stopped);
        assert!(queue.state().items.unwrap().is_empty());
    }

    #[test]
    fn a_hand_off_not_yet_reported_is_accounted_for() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3, 4]), 0, true);

        // Track 2 took over, but the event hasn't arrived when the user
        // moves 4 up: it must follow 2, not 1.
        let advanced = p.finish();
        queue.move_item(&mut p, uid_of(&queue, 4), 2);
        assert_eq!(current_id(&queue), Some(2));
        assert_eq!(p.next, Some(4));
        // The late event doesn't advance again.
        queue.on_track_ended(&mut p, advanced);
        assert_eq!(current_id(&queue), Some(2));
        check(&queue, &p);

        // "Next" pressed in the window goes to the track after the new one.
        p.finish();
        queue.next(&mut p);
        assert_eq!(current_id(&queue), Some(3));
        check(&queue, &p);
        queue.on_track_ended(&mut p, true);
        assert_eq!(current_id(&queue), Some(3));
        check(&queue, &p);
    }

    #[test]
    fn a_late_edit_after_the_last_track_still_plays() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1]), 0, true);
        let advanced = p.finish(); // Stopped: nothing was armed.
        queue.add(&mut p, infos([2]), false);
        queue.on_track_ended(&mut p, advanced);
        assert_eq!(current_id(&queue), Some(2));
        assert_eq!(p.state(), PlayerState::Playing);
    }

    #[test]
    fn unreadable_tracks_are_skipped_and_reported() {
        let mut p = Fake {
            unreadable: [2, 4, 5].into(),
            ..Fake::default()
        };
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3, 4, 5]), 0, true);
        assert_eq!(p.next, Some(3), "2 is skipped when arming");
        let state = queue.take_state().unwrap();
        assert_eq!(state.skipped.len(), 1);
        assert_eq!(state.skipped[0].track_id, 2);
        assert_eq!(state.unavailable, [uid_of(&queue, 2)]);
        assert!(queue.take_state().is_none(), "reported once");

        end_track(&mut queue, &mut p);
        assert_eq!(current_id(&queue), Some(3));
        assert_eq!(p.next, None, "4 and 5 can't be opened");
        assert_eq!(queue.take_state().unwrap().skipped.len(), 2);

        // Starting at an unreadable track starts at the next one that opens.
        queue.replace(&mut p, infos([4, 1, 2, 3]), 0, true);
        assert_eq!(p.current, Some(1));
        assert_eq!(p.next, Some(3));

        // Jumping to one tries it again.
        p.unreadable.remove(&2);
        queue.jump(&mut p, uid_of(&queue, 2));
        assert_eq!(p.current, Some(2));
        assert!(!queue.state().unavailable.contains(&uid_of(&queue, 2)));

        // Nothing readable: nothing plays.
        let mut p = Fake {
            unreadable: [1, 2].into(),
            ..Fake::default()
        };
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2]), 0, true);
        assert!(!queue.is_loaded());
        assert_eq!(p.state(), PlayerState::Empty);
        assert_eq!(queue.state().skipped.len(), 2);
    }

    fn pending() -> Fake {
        Fake {
            pending: true,
            ..Fake::default()
        }
    }

    #[test]
    fn a_track_opening_shows_as_loading_and_plays_when_ready() {
        let mut p = pending();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3]), 0, true);
        let state = queue.take_state().unwrap();
        assert!(state.loading && state.playing && !state.loaded);
        assert_eq!(current_id(&queue), Some(1), "current while it opens");
        assert_eq!(p.opening(), [1]);
        assert_eq!(p.state(), PlayerState::Empty, "nothing in the engine yet");

        assert_eq!(p.complete(&mut queue), 1);
        let state = queue.take_state().unwrap();
        assert!(state.loaded && !state.loading);
        assert_eq!(p.state(), PlayerState::Playing);
        assert_eq!(
            p.opening(),
            [2],
            "the next track opens once the current one is in"
        );
        assert_eq!(p.next, None);

        p.complete(&mut queue);
        check(&queue, &p);
        end_track(&mut queue, &mut p);
        assert_eq!(current_id(&queue), Some(2));
        assert_eq!(p.opening(), [3]);
    }

    #[test]
    fn commands_while_a_track_opens_apply_when_it_is_ready() {
        let mut p = pending();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3]), 0, true);

        // Paused and moved while opening: it starts there, paused.
        queue.pause(&mut p);
        queue.seek(&mut p, 12.5);
        assert!(!queue.take_state().unwrap().playing);
        p.complete(&mut queue);
        assert_eq!(p.state(), PlayerState::Stopped);
        assert_eq!(p.position, 12.5);
        p.complete(&mut queue);

        // Next while the next one opens: the first request is given up on.
        queue.play(&mut p);
        queue.next(&mut p);
        let first = p.requests[0].0;
        queue.next(&mut p);
        assert!(p.cancelled.contains(&first));
        assert_eq!(p.opening(), [3]);
        assert_eq!(current_id(&queue), Some(3));
        assert_eq!(p.current, Some(1), "the engine plays on until it's ready");

        // A report for a request given up on changes nothing.
        queue.load_finished(&mut p, first, Ok(()));
        assert_eq!(current_id(&queue), Some(3));
        assert_eq!(p.complete(&mut queue), 3);
        assert_eq!(p.state(), PlayerState::Playing, "still playing, as it was");
        assert!(queue.is_loaded());
    }

    #[test]
    fn a_track_that_takes_too_long_to_open_is_skipped() {
        let mut p = pending();
        let mut queue = Queue::new(1);
        queue.set_clock(100.0);
        queue.replace(&mut p, infos([1, 2]), 0, true);
        assert_eq!(queue.load_deadline(), Some(100.0 + LOAD_TIMEOUT));
        let first = p.requests[0].0;

        queue.check_loads(&mut p, 100.0 + LOAD_TIMEOUT - 1.0);
        assert_eq!(p.opening(), [1], "not yet");

        queue.check_loads(&mut p, 100.0 + LOAD_TIMEOUT);
        assert!(p.cancelled.contains(&first));
        let state = queue.take_state().unwrap();
        assert_eq!(state.skipped.len(), 1);
        assert_eq!(state.skipped[0].track_id, 1);
        assert!(crate::coded::is(&state.skipped[0].error, "openTimedOut"));
        assert_eq!(state.unavailable, [uid_of(&queue, 1)]);
        assert_eq!(p.opening(), [2], "the next item is tried");
        assert_eq!(current_id(&queue), Some(2));
        assert_eq!(queue.load_deadline(), Some(100.0 + 2.0 * LOAD_TIMEOUT));

        // The next track timing out is passed over as well.
        p.complete(&mut queue);
        assert_eq!(queue.load_deadline(), None, "nothing follows 2");
        queue.add(&mut p, infos([3, 4]), false);
        queue.check_loads(&mut p, 200.0 + LOAD_TIMEOUT);
        assert_eq!(p.opening(), [4], "3 timed out as the next track");
        assert_eq!(queue.take_state().unwrap().skipped[0].track_id, 3);
    }

    #[test]
    fn a_cloud_placeholder_downloads_with_a_longer_timeout() {
        let mut p = pending();
        let mut queue = Queue::new(1);
        queue.set_clock(100.0);
        queue.replace(&mut p, infos([1, 2]), 0, true);
        let request = p.requests[0].0;
        queue.take_state();

        queue.downloading(request);
        let state = queue.take_state().unwrap();
        assert!(state.loading && state.downloading);
        assert_eq!(queue.load_deadline(), Some(100.0 + DOWNLOAD_TIMEOUT));
        queue.check_loads(&mut p, 100.0 + LOAD_TIMEOUT);
        assert_eq!(p.opening(), [1], "still downloading");

        queue.check_loads(&mut p, 100.0 + DOWNLOAD_TIMEOUT);
        let state = queue.take_state().unwrap();
        assert_eq!(state.skipped[0].track_id, 1);
        assert!(state.skipped[0]
            .error
            .contains(&DOWNLOAD_TIMEOUT.to_string()));
        assert_eq!(p.opening(), [2]);
        assert!(!state.downloading, "the next one isn't a placeholder");
    }

    #[test]
    fn tracks_that_fail_to_open_are_skipped_until_one_opens() {
        let mut p = Fake {
            unreadable: [1, 2].into(),
            ..pending()
        };
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3]), 0, true);
        p.complete(&mut queue);
        assert_eq!(p.opening(), [2]);
        p.complete(&mut queue);
        assert_eq!(p.opening(), [3]);
        assert_eq!(queue.take_state().unwrap().skipped.len(), 2);
        p.complete(&mut queue);
        assert_eq!(current_id(&queue), Some(3));
        assert_eq!(p.state(), PlayerState::Playing);

        // A next track that fails is passed over for the one after it.
        let mut p = Fake {
            unreadable: [2].into(),
            ..pending()
        };
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3]), 0, true);
        p.complete(&mut queue);
        p.complete(&mut queue);
        assert_eq!(p.opening(), [3]);
        p.complete(&mut queue);
        check(&queue, &p);

        // Nothing opens: it stays where it was asked to start, and doesn't spin.
        let mut p = Fake {
            unreadable: [1, 2].into(),
            ..pending()
        };
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2]), 0, true);
        p.complete(&mut queue);
        p.complete(&mut queue);
        assert!(p.opening().is_empty());
        let state = queue.state();
        assert!(!state.loaded && !state.loading);
        assert_eq!(current_id(&queue), Some(1));
    }

    #[test]
    fn an_unavailable_folder_means_not_now() {
        // A folder that can't be read (H22) fails the open; the item is
        // passed over, and tried again once the folder is back.
        let mut p = pending();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2]), 0, true);
        let request = p.requests.remove(0).0;
        let error = crate::coded::folder_unavailable("/Volumes/Music", "missing", None);
        queue.load_finished(&mut p, request, Err(error));
        assert_eq!(queue.take_state().unwrap().unavailable, [uid_of(&queue, 1)]);
        assert_eq!(p.opening(), [2]);
        p.complete(&mut queue);
        assert!(
            p.opening().is_empty(),
            "1 isn't tried again as the next track"
        );

        queue.set_unavailable_tracks(&mut p, &HashSet::new());
        assert!(queue.take_state().unwrap().unavailable.is_empty());
        queue.jump(&mut p, uid_of(&queue, 1));
        assert_eq!(p.complete(&mut queue), 1);
        assert_eq!(current_id(&queue), Some(1));
    }

    #[test]
    fn removing_or_clearing_a_track_that_opens_gives_it_up() {
        let mut p = pending();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3]), 0, true);
        let first = p.requests[0].0;
        queue.remove(&mut p, &[uid_of(&queue, 1)]);
        assert!(p.cancelled.contains(&first));
        assert_eq!(p.opening(), [2], "the item after it takes its place");
        assert!(queue.take_state().unwrap().playing);

        let second = p.requests[0].0;
        queue.clear(&mut p);
        assert!(p.cancelled.contains(&second));
        assert!(p.opening().is_empty());
        assert!(!queue.state().loading);
    }

    #[test]
    fn restores_paused_where_it_was() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3, 4]), 0, true);
        queue.set_repeat(&mut p, Repeat::All);
        end_track(&mut queue, &mut p);
        let saved = queue.saved(42.0);
        assert_eq!(saved.current, Some(1));
        assert_eq!(saved.position, 42.0);

        // Track 1 has left the library since.
        let tracks: HashMap<i64, TrackInfo> = [2, 3, 4].map(|id| (id, info(id))).into();
        let json = serde_json::to_value(&saved).unwrap();
        let mut restored = Queue::restore(serde_json::from_value(json).unwrap(), &tracks, 1);
        assert_eq!(ids(&restored), [2, 3, 4]);
        assert_eq!(current_id(&restored), Some(2));
        let state = restored.state();
        assert_eq!((state.repeat, state.shuffle), (Repeat::All, false));
        assert_eq!(state.resume_at, 42.0);
        assert!(!restored.is_loaded(), "nothing opens at launch");

        // Seeking before playing moves where it will start.
        let mut p = Fake::default();
        restored.seek(&mut p, 30.0);
        assert_eq!(p.state(), PlayerState::Empty);
        restored.toggle(&mut p);
        assert_eq!((p.current, p.next, p.position), (Some(2), Some(3), 30.0));
        assert_eq!(p.state(), PlayerState::Playing);
        check(&restored, &p);

        // A saved current track that is gone gives way to the one after it.
        let saved = Saved {
            tracks: vec![5, 1, 3],
            current: Some(1),
            ..Saved::default()
        };
        let restored = Queue::restore(saved, &tracks, 1);
        assert_eq!(current_id(&restored), Some(3));
        assert_eq!(Queue::restore(Saved::default(), &tracks, 1).current, None);
    }

    #[test]
    fn a_restored_queue_with_nothing_available_plays_nothing() {
        let tracks: HashMap<i64, TrackInfo> = [1, 2, 3].map(|id| (id, info(id))).into();
        let saved = Saved {
            tracks: vec![1, 2, 3],
            current: Some(1),
            position: 42.0,
            repeat: Repeat::All,
            ..Saved::default()
        };
        let mut queue = Queue::restore(saved, &tracks, 1);
        // Their drive isn't connected.
        let mut p = Fake {
            unreadable: [1, 2, 3].into(),
            ..Fake::default()
        };
        queue.set_unavailable_tracks(&mut p, &[1, 2, 3].into());
        let state = queue.take_state().unwrap();
        assert_eq!(state.unavailable.len(), 3);
        assert!(state.skipped.is_empty(), "not reported as skipped");
        assert_eq!((state.loaded, state.resume_at), (false, 42.0));

        // Play tries each once, with repeat all, and stops.
        queue.toggle(&mut p);
        assert_eq!(p.state(), PlayerState::Empty);
        assert!(!queue.is_loaded());
        assert_eq!(current_id(&queue), Some(2), "the current item stays");
        let state = queue.take_state().unwrap();
        assert_eq!(state.skipped.len(), 1, "only the current one is tried");
        assert_eq!(state.resume_at, 42.0, "with its position");
        // Next has nowhere to go.
        queue.next(&mut p);
        assert_eq!(p.state(), PlayerState::Empty);
        assert_eq!(current_id(&queue), Some(2));

        // The drive comes back: it plays from where it was.
        p.unreadable.clear();
        queue.set_unavailable_tracks(&mut p, &HashSet::new());
        assert!(queue.state().unavailable.is_empty());
        queue.toggle(&mut p);
        assert_eq!((p.current, p.next, p.position), (Some(2), Some(3), 42.0));
        assert_eq!(p.state(), PlayerState::Playing);
        check(&queue, &p);
    }

    #[test]
    fn a_folder_going_while_playing_is_passed_over() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3, 4]), 0, true);
        assert_eq!(p.next, Some(2));
        queue.set_unavailable_tracks(&mut p, &[2, 3].into());
        assert_eq!(p.next, Some(4), "re-armed past them");
        queue.set_unavailable_tracks(&mut p, &HashSet::new());
        assert_eq!(p.next, Some(2));
        check(&queue, &p);
    }

    #[test]
    fn a_detached_queue_ignores_the_engine() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2]), 0, true);
        queue.detach();
        assert_eq!(p.load(99), Opening::Done(Ok(())));
        assert_eq!(p.set_next(Some(98), false), Opening::Done(Ok(())));
        end_track(&mut queue, &mut p);
        assert_eq!(current_id(&queue), Some(1));
        assert_eq!(p.next, None, "the queue didn't arm anything");
        queue.toggle(&mut p);
        check(&queue, &p);
        assert_eq!(p.current, Some(1));
    }

    fn with_units(units: &[(i64, i64)], ids: impl IntoIterator<Item = i64>) -> Vec<TrackInfo> {
        ids.into_iter()
            .map(|id| TrackInfo {
                unit: units
                    .iter()
                    .find(|(track, _)| *track == id)
                    .map(|(_, unit)| *unit),
                ..info(id)
            })
            .collect()
    }

    #[test]
    fn shuffle_keeps_units_together_and_in_order() {
        for seed in 1..20 {
            let mut p = Fake::default();
            let mut queue = Queue::new(seed);
            let units = [(3, 100), (4, 100), (5, 100), (7, 200), (8, 200)];
            queue.replace(&mut p, with_units(&units, 1..=10), 0, true);
            queue.set_shuffle(&mut p, true);
            check(&queue, &p);
            let order = ids(&queue);
            let at = |id: i64| order.iter().position(|&x| x == id).unwrap();
            assert_eq!((at(4), at(5)), (at(3) + 1, at(3) + 2), "{order:?}");
            assert_eq!(at(8), at(7) + 1, "{order:?}");
            queue.set_shuffle(&mut p, false);
            assert_eq!(ids(&queue), (1..=10).collect::<Vec<_>>());
        }
    }

    #[test]
    fn starting_a_shuffled_work_keeps_its_movements_after_it() {
        let mut p = Fake::default();
        let mut queue = Queue::new(3);
        queue.set_shuffle(&mut p, true);
        let units = [(2, 7), (3, 7), (4, 7)];
        queue.replace(&mut p, with_units(&units, 1..=6), 1, true);
        assert_eq!(&ids(&queue)[..3], [2, 3, 4]);
        assert_eq!(current_id(&queue), Some(2));
    }

    #[test]
    fn skipped_tracks_play_only_when_chosen() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        let tracks: Vec<TrackInfo> = [1, 2, 3, 4]
            .into_iter()
            .map(|id| TrackInfo {
                skip: id == 2 || id == 4,
                ..info(id)
            })
            .collect();
        queue.replace(&mut p, tracks.clone(), 0, true);
        assert_eq!(p.next, Some(3), "2 is passed over");
        queue.next(&mut p);
        assert_eq!(current_id(&queue), Some(3));
        assert_eq!(p.next, None, "4 too");
        assert!(!queue.state().has_next);
        queue.previous(&mut p);
        assert_eq!(current_id(&queue), Some(1));

        // Chosen directly, it plays.
        let uid = uid_of(&queue, 2);
        queue.jump(&mut p, uid);
        assert_eq!(current_id(&queue), Some(2));
        queue.replace(&mut p, tracks.clone(), 1, true);
        assert_eq!(current_id(&queue), Some(2));
        // Playing the lot from it doesn't.
        queue.replace_from(&mut p, tracks, 1, true, false);
        assert_eq!(current_id(&queue), Some(3));
    }

    #[test]
    fn radio_asks_for_more_near_the_end() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3]), 0, true);
        assert!(!queue.is_radio());
        assert_eq!(queue.radio_seed(2), None);
        queue.next(&mut p);
        assert_eq!(queue.radio_seed(2), Some(3));
        queue.set_repeat(&mut p, Repeat::All);
        assert_eq!(queue.radio_seed(2), None, "repeat never runs out");

        queue.start_radio(&mut p, infos([9, 10]));
        assert!(queue.is_radio());
        assert!(queue.state().radio);
        assert_eq!(current_id(&queue), Some(9));
        queue.add(&mut p, infos([11]), false);
        assert!(queue.is_radio(), "adding keeps it on");
        queue.replace(&mut p, infos([1]), 0, true);
        assert!(!queue.is_radio(), "playing something else ends it");
    }

    fn album(ids: impl IntoIterator<Item = i64>, album_id: i64) -> Vec<TrackInfo> {
        ids.into_iter()
            .map(|id| TrackInfo {
                album_id: Some(album_id),
                ..info(id)
            })
            .collect()
    }

    #[test]
    fn stops_after_the_chosen_item_and_waits_on_the_next() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3]), 0, true);
        let two = uid_of(&queue, 2);
        queue.set_stop_after(&mut p, Some(two));
        assert_eq!(queue.state().stop_after, Some(two));
        assert_eq!(p.next, Some(2), "1 still hands off to 2");

        end_track(&mut queue, &mut p);
        assert_eq!(current_id(&queue), Some(2));
        assert_eq!(p.next, None, "nothing follows 2");
        end_track(&mut queue, &mut p);
        // Stopped, and 3 waits, loaded and paused.
        assert_eq!(current_id(&queue), Some(3));
        assert!(queue.is_loaded());
        assert_ne!(p.state(), PlayerState::Playing);
        assert_eq!(queue.state().stop_after, None);
        check(&queue, &p);

        // Removing the item forgets it; so does choosing none.
        queue.set_stop_after(&mut p, Some(uid_of(&queue, 3)));
        queue.remove(&mut p, &[uid_of(&queue, 3)]);
        assert_eq!(queue.state().stop_after, None);
        queue.set_stop_after(&mut p, Some(12345));
        assert_eq!(queue.state().stop_after, None, "not in the queue");
    }

    #[test]
    fn a_sleep_timer_fades_out_then_pauses() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2]), 0, true);
        p.set_volume(0.8);
        queue.set_clock(1000.0);
        queue.set_sleep(
            &mut p,
            Some(SleepTimer::At {
                ends_at: 1100.0,
                minutes: 1,
            }),
        );
        assert!(queue.state().sleep.is_some());
        assert_eq!(
            serde_json::to_value(queue.state().sleep).unwrap(),
            serde_json::json!({"kind": "at", "endsAt": 1100.0, "minutes": 1})
        );

        queue.tick(&mut p, 1050.0);
        assert_eq!(p.volume(), 0.8, "not yet");
        queue.tick(&mut p, 1095.0);
        assert!((p.volume() - 0.4).abs() < 1e-9, "halfway through the fade");
        // The user turns it up: the fade carries on from there.
        p.set_volume(1.0);
        queue.volume_changed();
        queue.tick(&mut p, 1097.5);
        assert!((p.volume() - 0.25).abs() < 1e-9);
        queue.tick(&mut p, 1100.5);
        assert_eq!(p.state(), PlayerState::Paused);
        assert_eq!(p.volume(), 1.0, "put back for next time");
        assert_eq!(queue.state().sleep, None);

        // Pausing during the fade puts the volume back; playing after the
        // end ends the timer.
        queue.play(&mut p);
        queue.set_sleep(
            &mut p,
            Some(SleepTimer::At {
                ends_at: 2000.0,
                minutes: 1,
            }),
        );
        queue.tick(&mut p, 1995.0);
        assert!((p.volume() - 0.5).abs() < 1e-9);
        queue.pause(&mut p);
        assert_eq!(p.volume(), 1.0);
        queue.set_clock(2500.0);
        queue.play(&mut p);
        assert_eq!(queue.state().sleep, None);
        assert_eq!(p.state(), PlayerState::Playing);
    }

    #[test]
    fn sleep_at_the_end_of_the_track_or_album() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        let mut tracks = album([1, 2], 7);
        tracks.extend(album([3], 8));
        queue.replace(&mut p, tracks, 0, true);
        queue.set_sleep(&mut p, Some(SleepTimer::EndOfAlbum));
        assert_eq!(p.next, Some(2), "the album goes on");
        end_track(&mut queue, &mut p);
        assert_eq!(p.next, None, "its last track");
        end_track(&mut queue, &mut p);
        assert_eq!(current_id(&queue), Some(3));
        assert_ne!(p.state(), PlayerState::Playing);
        assert_eq!(queue.state().sleep, None);

        queue.replace(&mut p, infos([4, 5]), 0, true);
        queue.set_sleep(&mut p, Some(SleepTimer::EndOfTrack));
        assert_eq!(p.next, None);
        queue.set_sleep(&mut p, None);
        assert_eq!(p.next, Some(5));
    }

    #[test]
    fn long_tracks_resume_where_they_were_left() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        let book = TrackInfo {
            duration: 3600.0,
            ..info(1)
        };
        queue.replace(&mut p, vec![book.clone(), info(2)], 0, true);
        p.position = 600.0;
        queue.next(&mut p);
        assert_eq!(queue.take_positions(), [(1, Some(600.0))]);

        // Back to it: it starts at 600 s.
        queue.jump(&mut p, uid_of(&queue, 1));
        assert_eq!(p.position, 600.0);
        assert!(
            queue.take_positions().is_empty(),
            "short tracks aren't kept"
        );

        // Near its end counts as finished.
        p.position = 3590.0;
        queue.pause(&mut p);
        assert_eq!(queue.take_positions(), [(1, None)]);

        // The host's saved position, from the library.
        let saved = TrackInfo {
            resume: Some(1234.0),
            ..book
        };
        queue.replace(&mut p, vec![saved], 0, true);
        assert_eq!(p.position, 1234.0);
        // Played to the end: forgotten.
        end_track(&mut queue, &mut p);
        assert_eq!(queue.take_positions(), [(1, None)]);
    }

    #[test]
    fn crossfades_between_albums_but_not_within_one() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        let mut tracks = album([1, 2], 7);
        tracks.extend(infos([3]));
        tracks.extend(with_units(&[(4, 9), (5, 9)], [4, 5]));
        queue.replace(&mut p, tracks, 0, true);
        assert!(!p.crossfade, "1 to 2: one album");
        end_track(&mut queue, &mut p);
        assert!(p.crossfade, "2 to 3: another album");
        end_track(&mut queue, &mut p);
        assert!(p.crossfade, "3 to 4");
        end_track(&mut queue, &mut p);
        assert!(!p.crossfade, "4 to 5: one unit");
        queue.set_repeat(&mut p, Repeat::One);
        assert!(!p.crossfade, "the same track again");
    }

    #[test]
    fn moves_a_block_of_items() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3, 4, 5]), 1, true);
        let (one, four) = (uid_of(&queue, 1), uid_of(&queue, 4));
        queue.move_items(&mut p, &[four, one], 1);
        assert_eq!(ids(&queue), [2, 1, 4, 3, 5]);
        assert_eq!(current_id(&queue), Some(2));
        check(&queue, &p);
        queue.move_items(&mut p, &[four], 99);
        assert_eq!(ids(&queue), [2, 1, 3, 5, 4]);
        queue.move_items(&mut p, &[12345], 0);
        assert_eq!(ids(&queue), [2, 1, 3, 5, 4]);
    }

    #[test]
    fn a_workbench_file_plays_now_and_a_take_stops_after_it() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2, 3]), 0, true);
        let three = uid_of(&queue, 3);
        queue.set_stop_after(&mut p, Some(three));
        let outside = TrackInfo {
            external: true,
            ..info(-1)
        };
        let uid = queue.play_now(&mut p, vec![outside]).expect("queued");
        assert_eq!(ids(&queue), [1, -1, 2, 3]);
        assert_eq!(queue.current_item().map(|item| item.uid), Some(uid));
        assert_eq!(p.state(), PlayerState::Playing);
        assert_eq!(queue.play_now(&mut p, Vec::new()), None);

        // Only the current, open item can be taken.
        assert!(queue.ready_take(&mut p, three, 0.0).is_err());
        p.position = 42.0;
        let previous = queue.ready_take(&mut p, uid, 1.5).unwrap();
        assert_eq!(previous, Some(three));
        assert_eq!(queue.state().stop_after, Some(uid));
        assert_eq!(p.state(), PlayerState::Paused, "waits for the recording");
        assert_eq!(p.position, 1.5);
        assert_eq!(p.next, None, "nothing follows the take");
        queue.play(&mut p);

        // The file ends: playback stops, 2 waits, and 3's stop comes back.
        end_track(&mut queue, &mut p);
        assert_eq!(current_id(&queue), Some(2));
        assert_ne!(p.state(), PlayerState::Playing);
        queue.end_take(&mut p, uid, previous);
        assert_eq!(queue.state().stop_after, Some(three));
        check(&queue, &p);
    }

    #[test]
    fn a_take_ended_early_puts_back_the_stop_unless_the_user_chose_another() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2]), 0, true);
        let one = uid_of(&queue, 1);
        let two = uid_of(&queue, 2);
        let previous = queue.ready_take(&mut p, one, 0.0).unwrap();
        assert_eq!(previous, None);
        queue.end_take(&mut p, one, previous);
        assert_eq!(queue.state().stop_after, None);
        assert_eq!(p.next, Some(2), "1 hands off to 2 again");

        let previous = queue.ready_take(&mut p, one, 0.0).unwrap();
        queue.set_stop_after(&mut p, Some(two));
        queue.end_take(&mut p, one, previous);
        assert_eq!(
            queue.state().stop_after,
            Some(two),
            "the user's choice stays"
        );
    }

    #[test]
    fn files_outside_the_library_are_played_but_not_saved() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2]), 0, true);
        let outside = TrackInfo {
            external: true,
            ..info(-1)
        };
        queue.play_now(&mut p, vec![outside]);
        assert_eq!(ids(&queue), [1, -1, 2]);
        assert_eq!(p.current, Some(-1));
        let saved = queue.saved(10.0);
        assert_eq!(saved.tracks, [1, 2]);
        assert_eq!(saved.current, None, "the current one isn't kept");
        assert_eq!(saved.position, 0.0);
        queue.next(&mut p);
        assert_eq!(queue.saved(5.0).current, Some(1));
    }

    // ---- Numbered edits (PLAN.md H16) ------------------------------------

    /// `before`'s uids after `edits`, applied as the frontend applies them.
    fn replay(mut list: Vec<Uid>, edits: &[Edit]) -> Vec<Uid> {
        for edit in edits {
            match edit {
                Edit::Insert { at, items, .. } => {
                    list.splice(*at..*at, items.iter().map(|item| item.uid));
                }
                Edit::Remove { at, count } => {
                    list.drain(*at..*at + *count);
                }
                Edit::Move { from, count, to } => {
                    let moving: Vec<Uid> = list.drain(*from..*from + *count).collect();
                    list.splice(*to..*to, moving);
                }
                Edit::Update { at, items } => {
                    for (offset, item) in items.iter().enumerate() {
                        assert_eq!(list[at + offset], item.uid, "an update keeps its uids");
                    }
                }
            }
        }
        list
    }

    fn uids(queue: &Queue) -> Vec<Uid> {
        queue.items.iter().map(|item| item.uid).collect()
    }

    /// Takes the state and the store's log after an edit, and checks both
    /// edit lists turn `before` into the list as it is now.
    fn edits_since(queue: &mut Queue, before: &[Uid]) -> Vec<Edit> {
        let state = queue.take_state().expect("a changed queue has a state");
        assert!(
            state.items.is_none(),
            "an edit isn't sent as the whole list"
        );
        let edits = state.edits.expect("edits");
        assert_eq!(replay(before.to_vec(), &edits), uids(queue));
        assert_eq!(queue.take_stored(), ListLog::Edits(edits.clone()));
        edits
    }

    #[test]
    fn list_changes_go_out_as_numbered_edits_on_50_000_items() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos(1..=50_000), 10, false);
        let first = queue.take_state().unwrap();
        assert_eq!(first.items.as_ref().map(Vec::len), Some(50_000));
        assert_eq!(queue.take_stored(), ListLog::Reset);
        let mut version = first.list_version;

        let mut step = |queue: &mut Queue, edit: &dyn Fn(&mut Queue, &mut Fake)| {
            let before = uids(queue);
            edit(queue, &mut p);
            let edits = edits_since(queue, &before);
            version += 1;
            assert_eq!(queue.list_version, version);
            edits
        };
        // Play next: one insert after the current item.
        let edits = step(&mut queue, &|q, p| q.add(p, infos([7, 8]), true));
        assert!(
            matches!(edits[..], [Edit::Insert { at: 11, ref items, original_at: None }] if items.len() == 2)
        );
        // Add to the end.
        step(&mut queue, &|q, p| q.add(p, infos(60_000..60_010), false));
        // Remove scattered items: one edit per run, the last run first.
        let edits = step(&mut queue, &|q, p| {
            let all = uids(q);
            let gone = [all[3], all[4], all[5], all[9000], all[49_999]];
            q.remove(p, &gone);
        });
        assert_eq!(
            edits,
            [
                Edit::Remove {
                    at: 49_999,
                    count: 1
                },
                Edit::Remove { at: 9000, count: 1 },
                Edit::Remove { at: 3, count: 3 },
            ]
        );
        // Move one item, then a scattered block.
        let edits = step(&mut queue, &|q, p| {
            let uid = uids(q)[20_000];
            q.move_item(p, uid, 2);
        });
        assert_eq!(
            edits,
            [Edit::Move {
                from: 20_000,
                count: 1,
                to: 2
            }]
        );
        step(&mut queue, &|q, p| {
            let all = uids(q);
            q.move_items(p, &[all[100], all[101], all[30_000]], 40_000);
        });
        // A state with nothing else changed sends no list.
        queue.set_repeat(&mut p, Repeat::All);
        let state = queue.take_state().unwrap();
        assert!(state.items.is_none() && state.edits.is_none());
        assert_eq!(state.list_version, version);
        assert_eq!(queue.take_stored(), ListLog::Clean);
    }

    #[test]
    fn retagged_tracks_go_out_as_updates_of_their_runs() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos(1..=10), 0, false);
        queue.take_state();
        queue.take_stored();
        let before = uids(&queue);
        let retagged: HashMap<i64, TrackInfo> = [3, 4, 8]
            .into_iter()
            .map(|id| {
                (
                    id,
                    TrackInfo {
                        title: format!("new {id}"),
                        ..info(id)
                    },
                )
            })
            .collect();
        queue.update_tracks(&retagged);
        let edits = edits_since(&mut queue, &before);
        assert!(
            matches!(edits[..], [Edit::Update { at: 2, ref items }, Edit::Update { at: 7, .. }] if items.len() == 2)
        );
        assert_eq!(queue.items[7].track.title, "new 8");
    }

    #[test]
    fn replacing_clearing_and_shuffling_send_the_whole_list() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos(1..=20), 0, false);
        let mut version = queue.take_state().unwrap().list_version;
        for change in [
            &(|q: &mut Queue, p: &mut Fake| q.set_shuffle(p, true))
                as &dyn Fn(&mut Queue, &mut Fake),
            &|q, p| q.set_shuffle(p, false),
            &|q, p| q.clear(p),
        ] {
            queue.take_stored();
            change(&mut queue, &mut p);
            let state = queue.take_state().unwrap();
            version += 1;
            assert_eq!(state.list_version, version);
            assert_eq!(state.items.as_ref().map(Vec::len), Some(queue.items.len()));
            assert!(state.edits.is_none());
            assert_eq!(queue.take_stored(), ListLog::Reset);
        }
        // The whole state asked for is a new version too, so an edit sent
        // before it isn't applied again.
        queue.add(&mut p, infos([1]), false);
        let full = queue.state();
        assert_eq!(full.list_version, version + 1);
        assert!(full.items.is_some() && full.edits.is_none());
        assert!(queue.take_state().is_none());
    }

    #[test]
    fn an_insert_while_shuffled_says_where_it_goes_in_the_original_order() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos(1..=5), 0, false);
        queue.set_shuffle(&mut p, true);
        queue.take_state();
        queue.take_stored();
        queue.add(&mut p, infos([9]), true);
        let ListLog::Edits(edits) = queue.take_stored() else {
            panic!("an edit");
        };
        let current = queue.current_item().unwrap().uid;
        let original = queue.original.as_ref().unwrap();
        let after = original.iter().position(|&uid| uid == current).unwrap() + 1;
        assert!(matches!(edits[..], [Edit::Insert { original_at: Some(at), .. }] if at == after));
        assert_eq!(original[after], queue.items[1].uid);
    }

    #[test]
    fn a_restored_queue_keeps_its_saved_uids() {
        let tracks: HashMap<i64, TrackInfo> = (1..=4).map(|id| (id, info(id))).collect();
        let saved = Saved {
            tracks: vec![1, 2, 99, 4],
            uids: vec![10, 20, 30, 40],
            current: Some(1),
            ..Saved::default()
        };
        let mut queue = Queue::restore(saved, &tracks, 1);
        assert_eq!(uids(&queue), [10, 20, 40]);
        let state = queue.take_state().unwrap();
        assert!(state.items.is_some());
        // The rows are what was saved: nothing to write but the drop.
        assert_eq!(queue.take_stored(), ListLog::Clean);
        let mut p = Fake::default();
        queue.add(&mut p, infos([3]), false);
        assert_eq!(queue.items.last().unwrap().uid, 41);
    }
}
