//! The OS's media controls: Now Playing (Control Center and the menu-bar
//! widget on macOS, the lock screen on iOS) and the commands those and the
//! media keys send back.
//!
//! The controls live in a main-thread thread-local like the engine and the
//! queue. `NowPlaying` decides what to publish: it compares what the queue
//! and the player are doing with what it published last, and sends only the
//! differences. Its inputs are every queue change (`queue_changed`, from
//! `queue::publish`) and the engine's state and position events
//! (`player_event`). Positions arrive every 50 ms while playing but are not
//! forwarded: the system moves the progress bar on by itself, so the
//! position is published again only when the state changes or the position
//! departs from where the system has it (a seek, from anywhere).
//!
//! A queue restored at launch is published paused where it will resume,
//! before anything is loaded (PLAN.md F17), so the media keys and Now
//! Playing can start it; until it loads, the engine's events (it has
//! nothing open) are ignored. An empty queue publishes nothing. A current
//! item that can't be opened (its folder out of reach, PLAN.md H22) is
//! published stopped.
//!
//! Commands go straight to the queue (`queue::play`, `next`, ...). The core
//! calls the handler only from the OS's own handler, never inside a call
//! into the core, so no queue, engine or media borrow is held then.
//! (`run_on_main_thread` would not defer them: on the main thread Tauri
//! runs the closure at once.)
//!
//! Artwork comes from `library::art::lookup` for the current item's album,
//! or the track if it has none. It is looked up on a blocking thread and
//! published when it arrives, unless the current item's art has changed
//! meanwhile.

use std::cell::RefCell;
use std::time::Instant;

use tauri::{AppHandle, Manager, Runtime};

use crate::anomp::{Event, MediaCommand, MediaControls, PlayerState};
use crate::audio;
use crate::library::art::{self, Art, ArtKey};
use crate::library::commands::LibraryState;
use crate::queue;
use crate::queue::model::{QueueState, TrackInfo, Uid};

thread_local! {
    static MEDIA: RefCell<Option<Media>> = const { RefCell::new(None) };
}

/// While playing, the position is published again when it is further than
/// this many seconds from where the system has moved it on to.
const DRIFT_TOLERANCE: f64 = 0.25;

/// Smaller differences (seconds) count as the same position or duration.
const EPSILON: f64 = 0.001;

struct Media {
    controls: MediaControls,
    now_playing: NowPlaying,
    /// Origin of the clock `NowPlaying` measures extrapolation with.
    epoch: Instant,
}

/// Starts the media controls, where the platform has them. Call on the main
/// thread after the queue.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if !MediaControls::supported() {
        return Ok(());
    }
    let handler_app = app.clone();
    let controls = MediaControls::new(move |command| on_command(&handler_app, command))
        .ok_or("Cannot start the media controls")?;
    MEDIA.with_borrow_mut(|slot| {
        *slot = Some(Media {
            controls,
            now_playing: NowPlaying::default(),
            epoch: Instant::now(),
        })
    });
    Ok(())
}

/// Clears what is shown and stops the media controls. Main thread.
pub fn shutdown() {
    let media = MEDIA.with_borrow_mut(Option::take);
    drop(media);
}

/// Runs `f` with the controls, if they are running and not busy.
fn with_media<T>(f: impl FnOnce(&mut NowPlaying, &mut MediaControls, f64) -> T) -> Option<T> {
    MEDIA.with(|slot| {
        let mut slot = slot.try_borrow_mut().ok()?;
        let media = slot.as_mut()?;
        let now = media.epoch.elapsed().as_secs_f64();
        Some(f(&mut media.now_playing, &mut media.controls, now))
    })
}

/// The queue changed (and has emitted `queue-changed`). Main thread.
pub fn queue_changed<R: Runtime>(app: &AppHandle<R>, state: &QueueState) {
    let player = audio::engine_mut(|engine| Player {
        state: engine.state(),
        position: engine.position(),
        duration: engine.duration(),
    })
    .unwrap_or_default();
    let lookup = with_media(|now_playing, controls, now| {
        now_playing.queue_changed(controls, state, player, now)
    })
    .flatten();
    if let Some(key) = lookup {
        fetch_artwork(app, key);
    }
}

/// An engine event. Main thread.
pub fn player_event(event: Event) {
    with_media(|now_playing, controls, now| match event {
        Event::StateChanged(state) => now_playing.player_state(controls, state, now),
        Event::Position { position, duration } => {
            now_playing.position(controls, position, duration, now)
        }
        Event::DeviceChanged | Event::TrackEnded { .. } => {}
    });
}

/// The art of albums `album_ids` may have changed (e.g. a cover was
/// downloaded): it's looked up again if it's the current item's. Any thread.
pub fn art_changed<R: Runtime>(app: &AppHandle<R>, album_ids: &[i64]) {
    if album_ids.is_empty() {
        return;
    }
    let album_ids = album_ids.to_vec();
    let main_app = app.clone();
    let _ = app.run_on_main_thread(move || {
        let key = with_media(|now_playing, _, _| now_playing.art_changed(&album_ids)).flatten();
        if let Some(key) = key {
            fetch_artwork(&main_app, key);
        }
    });
}

/// Looks up the art for `key` off the main thread, then publishes it there
/// if it is still the current item's.
fn fetch_artwork<R: Runtime>(app: &AppHandle<R>, key: ArtKey) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let art = app.try_state::<LibraryState>().and_then(|library| {
            art::lookup(&library, key).unwrap_or_else(|error| {
                log::warn!("artwork: {error}");
                None
            })
        });
        // Not on the main thread, so this queues the closure rather than waiting.
        let _ = app.run_on_main_thread(move || {
            with_media(|now_playing, controls, _| {
                now_playing.artwork(controls, key, art.as_deref())
            });
        });
    });
}

fn on_command<R: Runtime>(app: &AppHandle<R>, command: MediaCommand) {
    let result = match command {
        MediaCommand::Play => queue::play(app),
        MediaCommand::Pause => queue::pause(app),
        MediaCommand::Toggle => queue::toggle(app),
        MediaCommand::Next => queue::next(app),
        MediaCommand::Previous => queue::previous(app),
        MediaCommand::Seek(seconds) => queue::seek(app, seconds),
    };
    if let Err(error) = result {
        log::warn!("{command:?}: {error}");
    }
}

// ---- What to publish ----------------------------------------------------------

/// Where `NowPlaying` publishes: the OS's media controls, or a fake in tests.
pub trait Publisher {
    fn set_track(&mut self, track: &TrackInfo);
    fn set_playback(&mut self, state: PlayerState, elapsed: f64, duration: f64);
    fn set_artwork(&mut self, art: Option<&Art>);
    fn set_navigation(&mut self, has_next: bool, has_previous: bool);
    fn clear(&mut self);
}

impl Publisher for MediaControls {
    fn set_track(&mut self, track: &TrackInfo) {
        MediaControls::set_track(
            self,
            &track.title,
            track.artist.as_deref(),
            track.album.as_deref(),
        );
    }
    fn set_playback(&mut self, state: PlayerState, elapsed: f64, duration: f64) {
        if !MediaControls::set_playback(self, state, elapsed, duration) {
            log::warn!("bad playback position {elapsed} of {duration}");
        }
    }
    fn set_artwork(&mut self, art: Option<&Art>) {
        if !MediaControls::set_artwork(self, art.map(|art| art.data.as_slice())) {
            let mime_type = art.map_or("", |art| art.mime_type.as_str());
            log::warn!("cannot show the artwork ({mime_type})");
        }
    }
    fn set_navigation(&mut self, has_next: bool, has_previous: bool) {
        MediaControls::set_navigation(self, has_next, has_previous);
    }
    fn clear(&mut self) {
        MediaControls::clear(self);
    }
}

/// The engine as `NowPlaying` last heard of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Player {
    pub state: PlayerState,
    /// Seconds into the current track, and its length.
    pub position: f64,
    pub duration: f64,
}

impl Default for Player {
    fn default() -> Player {
        Player {
            state: PlayerState::Empty,
            position: 0.0,
            duration: 0.0,
        }
    }
}

/// Playback as last published; `at` is when, on the `now` clock.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Published {
    state: PlayerState,
    elapsed: f64,
    duration: f64,
    at: f64,
}

/// Keeps the OS's Now Playing in step with the queue and the player. `now`
/// arguments are seconds on any monotonic clock.
#[derive(Default)]
pub struct NowPlaying {
    player: Player,
    /// The published item, if anything is published.
    track: Option<(Uid, TrackInfo)>,
    navigation: Option<(bool, bool)>,
    art: Option<ArtKey>,
    playback: Option<Published>,
    /// The queue's current track isn't open yet (after a relaunch).
    restored: bool,
}

impl NowPlaying {
    /// The queue changed. Returns the art to look up if the current item's
    /// art is different now; hand the result to `artwork`.
    pub fn queue_changed(
        &mut self,
        p: &mut impl Publisher,
        state: &QueueState,
        player: Player,
        now: f64,
    ) -> Option<ArtKey> {
        self.player = player;
        let Some(item) = state.current_item.as_ref() else {
            self.clear(p);
            return None;
        };
        // Not loaded: restored after a relaunch (or the dev page loaded
        // something else), shown paused where it will start.
        self.restored = !state.loaded;
        if self.restored {
            let unavailable = state.unavailable.contains(&item.uid);
            self.player = Player {
                state: if unavailable {
                    PlayerState::Stopped
                } else {
                    PlayerState::Paused
                },
                position: state.resume_at,
                duration: 0.0,
            };
        }

        let key = art_key(&item.track);
        let new_art = self.art != Some(key);
        if new_art && self.art.is_some() {
            p.set_artwork(None);
        }
        self.art = Some(key);

        let new_track = self
            .track
            .as_ref()
            .is_none_or(|(uid, track)| *uid != item.uid || *track != item.track);
        if new_track {
            p.set_track(&item.track);
            self.track = Some((item.uid, item.track.clone()));
        }

        let navigation = (state.has_next, state.has_previous);
        if self.navigation != Some(navigation) {
            p.set_navigation(navigation.0, navigation.1);
            self.navigation = Some(navigation);
        }

        self.sync_playback(p, now, new_track);
        new_art.then_some(key)
    }

    /// The engine's state changed.
    pub fn player_state(&mut self, p: &mut impl Publisher, state: PlayerState, now: f64) {
        if self.restored {
            return;
        }
        self.player.state = state;
        self.sync_playback(p, now, false);
    }

    /// The engine reported its position (every 50 ms while playing, and
    /// when it changes otherwise).
    pub fn position(&mut self, p: &mut impl Publisher, position: f64, duration: f64, now: f64) {
        if self.restored {
            return;
        }
        self.player.position = position;
        self.player.duration = duration;
        self.sync_playback(p, now, false);
    }

    /// The art of albums `album_ids` may have changed. Returns the current
    /// item's art to look up again if it's among them; hand the result to
    /// `artwork`.
    pub fn art_changed(&self, album_ids: &[i64]) -> Option<ArtKey> {
        self.art.filter(|key| {
            self.track.is_some() && matches!(key, ArtKey::Album(id) if album_ids.contains(id))
        })
    }

    /// The art for `key` arrived; it is shown if it is still the current
    /// item's.
    pub fn artwork(&mut self, p: &mut impl Publisher, key: ArtKey, art: Option<&Art>) {
        if self.track.is_some() && self.art == Some(key) {
            if let Some(art) = art {
                p.set_artwork(Some(art));
            }
        }
    }

    fn clear(&mut self, p: &mut impl Publisher) {
        if self.track.is_some() {
            p.clear();
        }
        self.track = None;
        self.navigation = None;
        self.art = None;
        self.playback = None;
    }

    /// Publishes the playback state if it differs from what the system
    /// shows (or always, with `force`).
    fn sync_playback(&mut self, p: &mut impl Publisher, now: f64, force: bool) {
        let Some((_, track)) = &self.track else {
            return;
        };
        let wanted = Published {
            state: self.player.state,
            elapsed: self.player.position,
            // The engine's is exact; the tags' stands in until it has one.
            duration: if self.player.duration > 0.0 {
                self.player.duration
            } else {
                track.duration
            },
            at: now,
        };
        let stale = match self.playback {
            None => true,
            Some(published) => {
                let shown = if published.state == PlayerState::Playing {
                    published.elapsed + (now - published.at)
                } else {
                    published.elapsed
                };
                let tolerance = if wanted.state == PlayerState::Playing {
                    DRIFT_TOLERANCE
                } else {
                    EPSILON
                };
                force
                    || published.state != wanted.state
                    || (published.duration - wanted.duration).abs() > EPSILON
                    || (shown - wanted.elapsed).abs() > tolerance
            }
        };
        if stale {
            p.set_playback(wanted.state, wanted.elapsed, wanted.duration);
            self.playback = Some(wanted);
        }
    }
}

/// The art the UI shows for a track: its album's, or its own without one.
fn art_key(track: &TrackInfo) -> ArtKey {
    track
        .album_id
        .map_or(ArtKey::Track(track.track_id), ArtKey::Album)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::model::{Item, Repeat};

    #[derive(Debug, Clone, PartialEq)]
    enum Call {
        Track(String),
        Playback(PlayerState, f64, f64),
        Artwork(Option<Vec<u8>>),
        Navigation(bool, bool),
        Clear,
    }

    #[derive(Default)]
    struct Fake(Vec<Call>);

    impl Fake {
        fn take(&mut self) -> Vec<Call> {
            std::mem::take(&mut self.0)
        }
    }

    impl Publisher for Fake {
        fn set_track(&mut self, track: &TrackInfo) {
            self.0.push(Call::Track(track.title.clone()));
        }
        fn set_playback(&mut self, state: PlayerState, elapsed: f64, duration: f64) {
            self.0.push(Call::Playback(state, elapsed, duration));
        }
        fn set_artwork(&mut self, art: Option<&Art>) {
            self.0.push(Call::Artwork(art.map(|art| art.data.clone())));
        }
        fn set_navigation(&mut self, has_next: bool, has_previous: bool) {
            self.0.push(Call::Navigation(has_next, has_previous));
        }
        fn clear(&mut self) {
            self.0.push(Call::Clear);
        }
    }

    fn item(uid: Uid, title: &str, album_id: Option<i64>) -> Item {
        Item {
            uid,
            track: TrackInfo {
                track_id: uid as i64 + 100,
                title: title.into(),
                artist: Some("Artist".into()),
                artist_id: Some(1),
                album: album_id.map(|id| format!("Album {id}")),
                album_id,
                duration: 180.0,
                ..TrackInfo::default()
            },
        }
    }

    fn queue(
        current: Option<Item>,
        loaded: bool,
        has_next: bool,
        has_previous: bool,
    ) -> QueueState {
        QueueState {
            revision: 1,
            items: None,
            length: usize::from(current.is_some()),
            current: current.as_ref().map(|_| 0),
            current_item: current,
            shuffle: false,
            repeat: Repeat::Off,
            unavailable: Vec::new(),
            skipped: Vec::new(),
            has_next,
            has_previous,
            loaded,
            resume_at: 0.0,
            radio: false,
            stop_after: None,
            sleep: None,
        }
    }

    fn playing(position: f64) -> Player {
        Player {
            state: PlayerState::Playing,
            position,
            duration: 181.5,
        }
    }

    fn art(bytes: &[u8]) -> Art {
        Art {
            mime_type: "image/png".into(),
            data: bytes.to_vec(),
            source: crate::metadata::settings::SourceId::Embedded,
            chosen: false,
        }
    }

    #[test]
    fn an_unavailable_current_item_is_published_stopped() {
        let (mut p, mut now_playing) = (Fake::default(), NowPlaying::default());
        let current = item(1, "One", Some(7));
        let mut restored = queue(Some(current.clone()), false, true, false);
        restored.resume_at = 42.0;
        restored.unavailable = vec![current.uid];
        now_playing.queue_changed(&mut p, &restored, Player::default(), 0.0);
        assert_eq!(
            p.take().last(),
            Some(&Call::Playback(PlayerState::Stopped, 42.0, 180.0))
        );
        // Its drive comes back: paused, ready to play.
        restored.unavailable.clear();
        now_playing.queue_changed(&mut p, &restored, Player::default(), 1.0);
        assert_eq!(p.take(), [Call::Playback(PlayerState::Paused, 42.0, 180.0)]);
    }

    #[test]
    fn publishes_the_restored_queue_paused_where_it_will_resume() {
        let (mut p, mut now_playing) = (Fake::default(), NowPlaying::default());
        // After a relaunch the queue is restored but not loaded.
        let mut restored = queue(Some(item(1, "One", Some(7))), false, true, false);
        restored.resume_at = 42.0;
        assert_eq!(
            now_playing.queue_changed(&mut p, &restored, Player::default(), 0.0),
            Some(ArtKey::Album(7))
        );
        assert_eq!(
            p.take(),
            [
                Call::Track("One".into()),
                Call::Navigation(true, false),
                Call::Playback(PlayerState::Paused, 42.0, 180.0),
            ]
        );
        // The engine has nothing open meanwhile: its events don't count.
        now_playing.player_state(&mut p, PlayerState::Empty, 0.1);
        now_playing.position(&mut p, 0.0, 0.0, 0.2);
        assert_eq!(p.take(), []);

        let started = queue(Some(item(1, "One", Some(7))), true, true, false);
        assert_eq!(
            now_playing.queue_changed(&mut p, &started, playing(42.0), 1.0),
            None
        );
        assert_eq!(
            p.take(),
            [Call::Playback(PlayerState::Playing, 42.0, 181.5)]
        );

        // The same state again changes nothing.
        assert_eq!(
            now_playing.queue_changed(&mut p, &started, playing(42.5), 1.5),
            None
        );
        assert_eq!(p.take(), []);
    }

    #[test]
    fn clears_once_when_the_queue_empties_or_detaches() {
        let (mut p, mut now_playing) = (Fake::default(), NowPlaying::default());
        let state = queue(Some(item(1, "One", None)), true, false, false);
        now_playing.queue_changed(&mut p, &state, playing(0.0), 0.0);
        p.take();

        now_playing.queue_changed(
            &mut p,
            &queue(None, false, false, false),
            Player::default(),
            1.0,
        );
        assert_eq!(p.take(), [Call::Clear]);
        now_playing.queue_changed(
            &mut p,
            &queue(None, false, false, false),
            Player::default(),
            2.0,
        );
        now_playing.position(&mut p, 1.0, 2.0, 2.1);
        assert_eq!(p.take(), []);

        // Everything is sent again for the next track, even if unchanged.
        assert_eq!(
            now_playing.queue_changed(&mut p, &state, playing(0.0), 3.0),
            Some(ArtKey::Track(101))
        );
        assert_eq!(p.take().len(), 3);

        // The dev page loaded something else: the queue detached, and shows
        // its track paused, as after a relaunch.
        let detached = queue(Some(item(1, "One", None)), false, false, false);
        now_playing.queue_changed(&mut p, &detached, playing(0.0), 4.0);
        assert_eq!(p.take(), [Call::Playback(PlayerState::Paused, 0.0, 180.0)]);
    }

    #[test]
    fn a_new_track_keeps_its_albums_art() {
        let (mut p, mut now_playing) = (Fake::default(), NowPlaying::default());
        let first = queue(Some(item(1, "One", Some(7))), true, true, false);
        now_playing.queue_changed(&mut p, &first, playing(0.0), 0.0);
        now_playing.artwork(&mut p, ArtKey::Album(7), Some(&art(b"seven")));
        p.take();

        // The same album: no lookup and no change to the art.
        let second = queue(Some(item(2, "Two", Some(7))), true, true, true);
        assert_eq!(
            now_playing.queue_changed(&mut p, &second, playing(0.0), 10.0),
            None
        );
        assert_eq!(
            p.take(),
            [
                Call::Track("Two".into()),
                Call::Navigation(true, true),
                Call::Playback(PlayerState::Playing, 0.0, 181.5),
            ]
        );

        // Another album: the old art goes at once, and the new one is looked up.
        let third = queue(Some(item(3, "Three", Some(8))), true, false, true);
        assert_eq!(
            now_playing.queue_changed(&mut p, &third, playing(0.0), 20.0),
            Some(ArtKey::Album(8))
        );
        assert_eq!(
            p.take(),
            [
                Call::Artwork(None),
                Call::Track("Three".into()),
                Call::Navigation(false, true),
                Call::Playback(PlayerState::Playing, 0.0, 181.5),
            ]
        );
    }

    #[test]
    fn art_arriving_late_is_ignored() {
        let (mut p, mut now_playing) = (Fake::default(), NowPlaying::default());
        let first = queue(Some(item(1, "One", Some(7))), true, true, false);
        now_playing.queue_changed(&mut p, &first, playing(0.0), 0.0);
        let second = queue(Some(item(2, "Two", Some(8))), true, false, true);
        now_playing.queue_changed(&mut p, &second, playing(0.0), 0.1);
        p.take();

        now_playing.artwork(&mut p, ArtKey::Album(7), Some(&art(b"seven")));
        assert_eq!(p.take(), [], "album 7 is no longer current");
        now_playing.artwork(&mut p, ArtKey::Album(8), None);
        assert_eq!(p.take(), [], "no art: already cleared");
        now_playing.artwork(&mut p, ArtKey::Album(8), Some(&art(b"eight")));
        assert_eq!(p.take(), [Call::Artwork(Some(b"eight".to_vec()))]);

        now_playing.queue_changed(
            &mut p,
            &queue(None, false, false, false),
            Player::default(),
            1.0,
        );
        p.take();
        now_playing.artwork(&mut p, ArtKey::Album(8), Some(&art(b"eight")));
        assert_eq!(p.take(), [], "cleared meanwhile");
    }

    #[test]
    fn positions_are_published_only_when_the_system_would_be_wrong() {
        let (mut p, mut now_playing) = (Fake::default(), NowPlaying::default());
        let state = queue(Some(item(1, "One", None)), true, false, false);
        now_playing.queue_changed(&mut p, &state, playing(10.0), 100.0);
        p.take();

        // Playing on as the system expects, give or take the tolerance.
        now_playing.position(&mut p, 10.05, 181.5, 100.05);
        now_playing.position(&mut p, 15.2, 181.5, 105.0);
        now_playing.position(&mut p, 19.8, 181.5, 110.0);
        assert_eq!(p.take(), []);

        // A seek.
        now_playing.position(&mut p, 60.0, 181.5, 110.05);
        assert_eq!(
            p.take(),
            [Call::Playback(PlayerState::Playing, 60.0, 181.5)]
        );

        // Pausing, then a seek while paused, however small.
        now_playing.player_state(&mut p, PlayerState::Paused, 111.0);
        assert_eq!(p.take(), [Call::Playback(PlayerState::Paused, 60.0, 181.5)]);
        now_playing.position(&mut p, 60.9, 181.5, 111.05);
        now_playing.position(&mut p, 60.9, 181.5, 120.0);
        assert_eq!(p.take(), [Call::Playback(PlayerState::Paused, 60.9, 181.5)]);
        now_playing.position(&mut p, 61.0, 181.5, 121.0);
        assert_eq!(p.take(), [Call::Playback(PlayerState::Paused, 61.0, 181.5)]);

        // Resuming, then a change of duration.
        now_playing.player_state(&mut p, PlayerState::Playing, 130.0);
        now_playing.position(&mut p, 61.05, 181.5, 130.05);
        assert_eq!(
            p.take(),
            [Call::Playback(PlayerState::Playing, 61.0, 181.5)]
        );
        now_playing.position(&mut p, 61.1, 182.0, 130.1);
        assert_eq!(
            p.take(),
            [Call::Playback(PlayerState::Playing, 61.1, 182.0)]
        );

        // Stopped at the end of the queue.
        now_playing.player_state(&mut p, PlayerState::Stopped, 140.0);
        now_playing.position(&mut p, 0.0, 182.0, 140.05);
        assert_eq!(
            p.take(),
            [
                Call::Playback(PlayerState::Stopped, 61.1, 182.0),
                Call::Playback(PlayerState::Stopped, 0.0, 182.0),
            ]
        );
    }

    #[test]
    fn the_tags_duration_stands_in_until_the_engine_has_one() {
        let (mut p, mut now_playing) = (Fake::default(), NowPlaying::default());
        let state = queue(Some(item(1, "One", None)), true, false, false);
        let unknown = Player {
            state: PlayerState::Stopped,
            position: 0.0,
            duration: 0.0,
        };
        now_playing.queue_changed(&mut p, &state, unknown, 0.0);
        assert_eq!(
            p.take().last(),
            Some(&Call::Playback(PlayerState::Stopped, 0.0, 180.0))
        );
    }

    #[test]
    fn a_downloaded_cover_is_shown_if_it_is_the_current_albums() {
        let (mut p, mut now_playing) = (Fake::default(), NowPlaying::default());
        assert_eq!(now_playing.art_changed(&[7]), None, "nothing published");
        let state = queue(Some(item(1, "One", Some(7))), true, false, false);
        now_playing.queue_changed(&mut p, &state, playing(0.0), 0.0);
        now_playing.artwork(&mut p, ArtKey::Album(7), None);
        p.take();
        assert_eq!(now_playing.art_changed(&[3, 8]), None);
        let key = now_playing.art_changed(&[3, 7]).unwrap();
        assert_eq!(key, ArtKey::Album(7));
        now_playing.artwork(&mut p, key, Some(&art(b"cover")));
        assert_eq!(p.take(), [Call::Artwork(Some(b"cover".to_vec()))]);
    }

    #[test]
    fn a_rescan_that_changes_the_tags_republishes_the_track() {
        let (mut p, mut now_playing) = (Fake::default(), NowPlaying::default());
        now_playing.queue_changed(
            &mut p,
            &queue(Some(item(1, "One", None)), true, false, false),
            playing(5.0),
            0.0,
        );
        p.take();
        let mut renamed = item(1, "One (Remastered)", None);
        renamed.track.duration = 181.0;
        now_playing.queue_changed(
            &mut p,
            &queue(Some(renamed), true, false, false),
            playing(5.0),
            0.0,
        );
        assert_eq!(
            p.take(),
            [
                Call::Track("One (Remastered)".into()),
                Call::Playback(PlayerState::Playing, 5.0, 181.5),
            ]
        );
    }
}
