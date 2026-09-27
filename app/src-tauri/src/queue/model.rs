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

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::anomp::PlayerState;

/// Identifies an item in the queue; unlike its index, it survives reorders,
/// and unlike the track id, it tells two copies of a track apart.
pub type Uid = u64;

/// Seconds into a track after which "previous" restarts it rather than
/// going back a track.
pub const RESTART_THRESHOLD: f64 = 3.0;

/// What the queue shows about a track.
#[derive(Debug, Clone, PartialEq, Serialize)]
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
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub uid: Uid,
    #[serde(flatten)]
    pub track: TrackInfo,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

/// A track that couldn't be opened and was passed over.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub uid: Uid,
    pub track_id: i64,
    pub title: String,
    pub error: String,
}

/// The engine, as the queue drives it.
pub trait Player {
    /// Opens a track as the current one, stopped at its start; this clears
    /// the next track. On failure nothing changes.
    fn load(&mut self, track_id: i64) -> Result<(), String>;
    /// Opens the track that follows the current one gaplessly, or clears it.
    fn set_next(&mut self, track_id: Option<i64>) -> Result<(), String>;
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
#[serde(rename_all = "camelCase")]
pub struct QueueState {
    /// Increases with every state sent.
    pub revision: u64,
    /// The whole list, only when it changed since the previous state (and in
    /// every full state), so moving to the next track doesn't resend it.
    pub items: Option<Vec<Item>>,
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
    /// relaunch), `resume_at` is where it will start, in seconds.
    pub loaded: bool,
    pub resume_at: f64,
}

/// The queue as saved across launches.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Saved {
    pub tracks: Vec<i64>,
    /// With shuffle on, the order before shuffling, as indices into `tracks`.
    pub original: Option<Vec<usize>>,
    pub current: Option<usize>,
    /// Seconds into the current track.
    pub position: f64,
    pub repeat: Repeat,
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
    /// The engine's advance count the queue has accounted for.
    seen_advances: i64,
    resume_at: f64,
    unavailable: HashSet<Uid>,
    revision: u64,
    changed: bool,
    list_changed: bool,
    skipped: Vec<Skipped>,
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
            seen_advances: 0,
            resume_at: 0.0,
            unavailable: HashSet::new(),
            revision: 0,
            changed: false,
            list_changed: false,
            skipped: Vec::new(),
        }
    }

    /// The saved queue, not loaded (nothing plays until asked). Tracks that
    /// `tracks` doesn't have (removed from the library) are dropped.
    pub fn restore(saved: Saved, tracks: &HashMap<i64, TrackInfo>, seed: u64) -> Queue {
        let mut queue = Queue::new(seed);
        let mut index_of_saved = Vec::with_capacity(saved.tracks.len());
        for id in &saved.tracks {
            index_of_saved.push(tracks.get(id).map(|track| {
                let item = queue.new_item(track.clone());
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
        queue.list_changed = true;
        queue
    }

    /// What to save; `position` is the seconds into the current track.
    pub fn saved(&self, position: f64) -> Saved {
        let index: HashMap<Uid, usize> = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.uid, index))
            .collect();
        Saved {
            tracks: self.items.iter().map(|item| item.track.track_id).collect(),
            original: self.original.as_ref().map(|uids| {
                uids.iter()
                    .filter_map(|uid| index.get(uid).copied())
                    .collect()
            }),
            current: self.current,
            position: if self.loaded {
                position
            } else {
                self.resume_at
            },
            repeat: self.repeat,
        }
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
        for item in &mut self.items {
            if let Some(track) = tracks.get(&item.track.track_id) {
                if *track != item.track {
                    item.track = track.clone();
                    self.list_changed = true;
                }
            }
        }
    }

    #[cfg(test)]
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub fn current_item(&self) -> Option<&Item> {
        self.current.map(|index| &self.items[index])
    }

    /// The whole state, list included.
    pub fn state(&mut self) -> QueueState {
        self.list_changed = true;
        self.take_state().expect("a changed queue has a state")
    }

    /// The state if anything changed since the last one, marking it sent.
    pub fn take_state(&mut self) -> Option<QueueState> {
        if !self.changed && !self.list_changed && self.skipped.is_empty() {
            return None;
        }
        self.revision += 1;
        let state = QueueState {
            revision: self.revision,
            items: self.list_changed.then(|| self.items.clone()),
            length: self.items.len(),
            current: self.current,
            current_item: self.current_item().cloned(),
            shuffle: self.original.is_some(),
            repeat: self.repeat,
            unavailable: self
                .items
                .iter()
                .map(|item| item.uid)
                .filter(|uid| self.unavailable.contains(uid))
                .collect(),
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
            resume_at: if self.loaded { 0.0 } else { self.resume_at },
        };
        self.changed = false;
        self.list_changed = false;
        Some(state)
    }

    // ---- Commands ---------------------------------------------------------

    /// Replaces the queue with `tracks` and starts `start` (with shuffle on,
    /// it plays first and the rest are shuffled). Playing unless `play` is
    /// false.
    pub fn replace(
        &mut self,
        p: &mut impl Player,
        tracks: Vec<TrackInfo>,
        start: usize,
        play: bool,
    ) {
        self.reconcile(p);
        if tracks.is_empty() {
            return self.clear(p);
        }
        self.items = tracks
            .into_iter()
            .map(|track| self.new_item(track))
            .collect();
        self.unavailable.clear();
        self.list_changed = true;
        let mut start = start.min(self.items.len() - 1);
        if self.original.is_some() {
            self.original = Some(self.items.iter().map(|item| item.uid).collect());
            let first = self.items.remove(start);
            self.items.insert(0, first);
            self.shuffle_from(1);
            start = 0;
        }
        self.current = Some(start);
        self.start(p, start, play, 0.0);
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
        self.items.splice(at..at, new);
        if let Some(original) = &mut self.original {
            let at = match current_uid.filter(|_| next) {
                Some(uid) => original.iter().position(|&o| o == uid).map_or(0, |i| i + 1),
                None => original.len(),
            };
            original.splice(at..at, uids);
        }
        if self.current.is_none() {
            self.current = Some(0);
        }
        self.list_changed = true;
        self.sync_next(p);
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
        let kept_before = self.items[..current]
            .iter()
            .filter(|item| !gone.contains(&item.uid))
            .count();
        self.items.retain(|item| !gone.contains(&item.uid));
        if let Some(original) = &mut self.original {
            original.retain(|uid| !gone.contains(uid));
        }
        self.unavailable.retain(|uid| !gone.contains(uid));
        self.list_changed = true;
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
        if self.loaded {
            let play = following && p.state() == PlayerState::Playing;
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
        self.list_changed = true;
        self.sync_next(p);
    }

    /// Empties the queue and stops playback.
    pub fn clear(&mut self, p: &mut impl Player) {
        self.reconcile(p);
        if self.loaded {
            p.stop();
            let _ = p.set_next(None);
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
        self.list_changed = true;
    }

    /// Plays an item, trying it again if it couldn't be opened before.
    pub fn jump(&mut self, p: &mut impl Player, uid: Uid) {
        self.reconcile(p);
        if let Some(index) = self.index_of(uid) {
            self.unavailable.remove(&uid);
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
        if self.loaded {
            p.play();
        } else if let Some(current) = self.current {
            let resume_at = self.resume_at;
            self.start(p, current, true, resume_at);
        }
    }

    pub fn pause(&mut self, p: &mut impl Player) {
        self.reconcile(p);
        if self.loaded {
            p.pause();
        }
    }

    pub fn toggle(&mut self, p: &mut impl Player) {
        if self.loaded && p.state() == PlayerState::Playing {
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
        self.list_changed = true;
        self.sync_next(p);
    }

    pub fn set_repeat(&mut self, p: &mut impl Player, repeat: Repeat) {
        self.reconcile(p);
        self.repeat = repeat;
        self.changed = true;
        self.sync_next(p);
    }

    // ---- Engine events ----------------------------------------------------

    /// The engine reported `TrackEnded`.
    pub fn on_track_ended(&mut self, p: &mut impl Player, advanced: bool) {
        self.reconcile(p);
        if !self.loaded {
            return;
        }
        if !advanced && p.state() == PlayerState::Stopped {
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
        if self.loaded {
            self.loaded = false;
            self.armed = None;
            self.resume_at = 0.0;
            self.changed = true;
        }
    }

    // ---- Internals --------------------------------------------------------

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
            .find(|&index| !self.unavailable.contains(&self.items[index].uid))
    }

    /// What plays after `from` when it ends.
    fn following(&self, from: usize) -> Option<usize> {
        match self.repeat {
            Repeat::One => (!self.unavailable.contains(&self.items[from].uid)).then_some(from),
            Repeat::All => self.find(from, true, true),
            Repeat::Off => self.find(from, true, false),
        }
    }

    /// Moves to `index`: loaded and keeping the play state if the queue is
    /// loaded, otherwise just selected.
    fn go_to(&mut self, p: &mut impl Player, index: usize) {
        if self.loaded {
            let play = p.state() == PlayerState::Playing;
            self.start(p, index, play, 0.0);
        } else {
            self.current = Some(index);
            self.resume_at = 0.0;
            self.changed = true;
        }
    }

    /// Loads `index` (or, if it can't be opened, the next item that can),
    /// seeks to `resume_at`, plays if `play`, and arms the following item.
    fn start(&mut self, p: &mut impl Player, index: usize, play: bool, resume_at: f64) {
        self.changed = true;
        let mut candidate = Some(index);
        while let Some(index) = candidate {
            let item = &self.items[index];
            let uid = item.uid;
            match p.load(item.track.track_id) {
                Ok(()) => {
                    self.current = Some(index);
                    self.loaded = true;
                    self.armed = None;
                    self.seen_advances = p.advance_count();
                    self.resume_at = 0.0;
                    self.unavailable.remove(&uid);
                    if resume_at > 0.0 {
                        p.seek(resume_at);
                    }
                    if play {
                        p.play();
                    }
                    self.sync_next(p);
                    return;
                }
                Err(error) => {
                    self.mark_unavailable(index, error);
                    candidate = self.find(index, true, self.repeat == Repeat::All);
                }
            }
        }
        // Nothing could be opened.
        if self.loaded {
            p.stop();
        }
        self.current = Some(index);
        self.loaded = false;
        self.armed = None;
        self.resume_at = 0.0;
    }

    /// Arms the engine with the item that should follow the current one,
    /// unless it already is; items that can't be opened are skipped.
    fn sync_next(&mut self, p: &mut impl Player) {
        self.changed = true;
        let Some(current) = self.current.filter(|_| self.loaded) else {
            return;
        };
        let current = self.next_pass(current);
        loop {
            let candidate = self.following(current);
            let uid = candidate.map(|index| self.items[index].uid);
            if uid == self.armed {
                return;
            }
            let track = candidate.map(|index| self.items[index].track.track_id);
            match (p.set_next(track), candidate) {
                (Ok(()), _) => {
                    self.armed = uid;
                    return;
                }
                (Err(error), Some(index)) => self.mark_unavailable(index, error),
                (Err(_), None) => {
                    // Clearing can't fail on a loaded engine; forget it.
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
        if n > 2 && Some(self.items[1].uid) == just_played {
            let other = 2 + (self.random() % (n as u64 - 2)) as usize;
            self.items.swap(1, other);
        }
        self.current = Some(0);
        self.list_changed = true;
        0
    }

    /// Shuffles the items from `from` on (Fisher–Yates).
    fn shuffle_from(&mut self, from: usize) {
        for i in (from + 1..self.items.len()).rev() {
            let j = from + (self.random() % (i - from + 1) as u64) as usize;
            self.items.swap(i, j);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// An engine that plays instantly: `finish` ends the current track.
    #[derive(Default)]
    struct Fake {
        current: Option<i64>,
        next: Option<i64>,
        state: Option<PlayerState>,
        position: f64,
        advances: i64,
        unreadable: HashSet<i64>,
        loads: Vec<i64>,
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

    impl Player for Fake {
        fn load(&mut self, track_id: i64) -> Result<(), String> {
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
        fn set_next(&mut self, track_id: Option<i64>) -> Result<(), String> {
            if let Some(id) = track_id.filter(|id| self.unreadable.contains(id)) {
                return Err(format!("cannot open {id}"));
            }
            self.next = track_id;
            Ok(())
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
            artist: None,
            artist_id: None,
            album: None,
            album_id: None,
            duration: 60.0,
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
    fn a_detached_queue_ignores_the_engine() {
        let mut p = Fake::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut p, infos([1, 2]), 0, true);
        queue.detach();
        p.load(99).unwrap();
        p.set_next(Some(98)).unwrap();
        end_track(&mut queue, &mut p);
        assert_eq!(current_id(&queue), Some(1));
        assert_eq!(p.next, None, "the queue didn't arm anything");
        queue.toggle(&mut p);
        check(&queue, &p);
        assert_eq!(p.current, Some(1));
    }
}
