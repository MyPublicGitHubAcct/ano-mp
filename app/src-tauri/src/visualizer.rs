//! Streams the core's analysis of what the player plays to the visualizer
//! (PLAN.md Phase 5).
//!
//! The frontend subscribes with a Tauri `Channel` while a visualization is
//! showing. The core analyses only while there is a subscriber: the first
//! one starts it on the main thread (where the engine lives), the last one
//! to leave stops it. Frames arrive on the core's analysis thread, and are
//! sent from there in the compact binary form `encode` describes, about 60
//! times a second (2.2 KB each), rather than as JSON.

use std::sync::{Arc, Mutex, MutexGuard};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::anomp::{AnalysisConfig, AnalysisFrame};
use crate::audio;

/// What the frontend's visualizations are drawn from, at `frame_rate`
/// analyses a second (`settings::VisualizerSettings`).
pub fn config(frame_rate: u32) -> AnalysisConfig {
    AnalysisConfig {
        bands: 64,
        waveform_length: 512,
        frames_per_second: f64::from(frame_rate),
    }
}

/// The version byte `encode` starts with.
pub const FORMAT_VERSION: u8 = 1;

const FLAG_SILENT: u8 = 1;
const FLAG_BEAT: u8 = 2;

/// Encodes a frame for the frontend (`app/src/lib/visualizer/frame.ts`
/// decodes it). Little-endian:
///
/// | offset | size | field |
/// |---|---|---|
/// | 0 | u8 | version (`FORMAT_VERSION`) |
/// | 1 | u8 | flags: 1 silent, 2 beat |
/// | 2 | u16 | band count *n* |
/// | 4 | u16 | waveform length *m* |
/// | 6 | u16 | 0 |
/// | 8 | 7 × f32 | lowest Hz, highest Hz, peak L, peak R, RMS L, RMS R, onset |
/// | 36 | 12 × u8 | chroma, C first, 0..=255 for 0..=1 |
/// | 48 | *n* × u8 | bands, 0..=255 for 0..=1, then a 0 if *n* is odd |
/// | | *m* × i16 | left samples, ±32767 for ±1 |
/// | | *m* × i16 | right samples |
///
/// Bands and chroma are only drawn, so 8 bits are plenty; the waveform
/// keeps 16 so a large scope doesn't step.
pub fn encode(frame: &AnalysisFrame<'_>) -> Vec<u8> {
    let bands = frame.bands.len().min(u16::MAX as usize);
    let samples = frame
        .left
        .len()
        .min(frame.right.len())
        .min(u16::MAX as usize);
    let padded = bands + bands % 2;
    let mut bytes = Vec::with_capacity(48 + padded + 4 * samples);

    let flags = if frame.silent { FLAG_SILENT } else { 0 } | if frame.beat { FLAG_BEAT } else { 0 };
    bytes.extend_from_slice(&[FORMAT_VERSION, flags]);
    bytes.extend_from_slice(&(bands as u16).to_le_bytes());
    bytes.extend_from_slice(&(samples as u16).to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    for value in [
        frame.lowest_hz,
        frame.highest_hz,
        frame.peak[0],
        frame.peak[1],
        frame.rms[0],
        frame.rms[1],
        frame.onset,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let unit = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    bytes.extend(frame.chroma.iter().map(|&value| unit(value)));
    bytes.extend(frame.bands[..bands].iter().map(|&value| unit(value)));
    bytes.resize(48 + padded, 0);
    for channel in [frame.left, frame.right] {
        for &sample in &channel[..samples] {
            let sample = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
    }
    bytes
}

/// Where frames go; a `Channel` in the app, a fake in tests.
pub trait Sink: Send {
    /// Returns false if the frame can't be delivered any more.
    fn send(&self, bytes: Vec<u8>) -> bool;
}

impl Sink for Channel<InvokeResponseBody> {
    fn send(&self, bytes: Vec<u8>) -> bool {
        Channel::send(self, InvokeResponseBody::Raw(bytes)).is_ok()
    }
}

/// The subscribed sinks, by id.
pub struct Subscribers<S> {
    next_id: u32,
    sinks: Vec<(u32, S)>,
}

impl<S> Default for Subscribers<S> {
    fn default() -> Self {
        Subscribers {
            next_id: 1,
            sinks: Vec::new(),
        }
    }
}

impl<S: Sink> Subscribers<S> {
    pub fn add(&mut self, sink: S) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.sinks.push((id, sink));
        id
    }

    /// Returns whether `id` was subscribed.
    pub fn remove(&mut self, id: u32) -> bool {
        let before = self.sinks.len();
        self.sinks.retain(|(sink_id, _)| *sink_id != id);
        self.sinks.len() != before
    }

    pub fn clear(&mut self) {
        self.sinks.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.sinks.is_empty()
    }

    /// Sends `bytes` to every sink, dropping those that fail. Returns
    /// whether any were dropped.
    pub fn broadcast(&mut self, bytes: Vec<u8>) -> bool {
        let before = self.sinks.len();
        let last = before.saturating_sub(1);
        let mut bytes = Some(bytes);
        let mut index = 0;
        self.sinks.retain(|(_, sink)| {
            // The last sink gets the buffer itself; the rest get copies.
            let payload = if index == last {
                bytes.take().unwrap_or_default()
            } else {
                bytes.clone().unwrap_or_default()
            };
            index += 1;
            sink.send(payload)
        });
        self.sinks.len() != before
    }
}

type Shared = Arc<Mutex<Subscribers<Channel<InvokeResponseBody>>>>;

/// The subscribers, managed by Tauri.
#[derive(Default)]
pub struct VisualizerState(Shared);

impl VisualizerState {
    fn lock(&self) -> MutexGuard<'_, Subscribers<Channel<InvokeResponseBody>>> {
        lock(&self.0)
    }
}

fn lock<S>(shared: &Mutex<Subscribers<S>>) -> MutexGuard<'_, Subscribers<S>> {
    // Nothing is left half-done by a panic while it's held.
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Starts the analysis if anyone is subscribed and it isn't running, or
/// stops it if no one is. Runs on the main thread; the subscribers aren't
/// locked while the engine is called, since stopping waits for the
/// analysis thread, which locks them to send.
fn sync<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    sync_or_restart(app, false)
}

/// Like `sync`, but also restarts a running analysis, e.g. with a new
/// frame rate.
fn sync_or_restart<R: Runtime>(app: &AppHandle<R>, restart: bool) -> Result<(), String> {
    let Some(state) = app.try_state::<VisualizerState>() else {
        return Ok(());
    };
    let shared = state.0.clone();
    let app = app.clone();
    let config = config(crate::settings::current(&app).visualizer.frame_rate);
    audio::on_main(&app.clone(), move || {
        let wanted = !lock(&shared).is_empty();
        audio::engine_mut(|engine| {
            if wanted && (restart || !engine.is_analysing()) {
                // Replaces the running analysis, if any.
                let started = engine.start_analysis(config, forwarder(app, shared));
                if !started {
                    return Err("The audio engine refused the analysis settings".to_string());
                }
            } else if !wanted && engine.is_analysing() {
                engine.stop_analysis();
            }
            Ok(())
        })?
    })?
}

/// The analysis handler: sends each frame to the subscribers, and stops the
/// analysis (from the main thread) if the last of them has gone.
fn forwarder<R: Runtime>(
    app: AppHandle<R>,
    shared: Shared,
) -> impl FnMut(&AnalysisFrame<'_>) + Send + 'static {
    move |frame| {
        let bytes = encode(frame);
        let mut subscribers = lock(&shared);
        if subscribers.broadcast(bytes) && subscribers.is_empty() {
            drop(subscribers);
            let app = app.clone();
            let _ = app.clone().run_on_main_thread(move || {
                if let Err(error) = sync(&app) {
                    log::warn!("{error}");
                }
            });
        }
    }
}

/// Restarts the analysis, if it's running, with the settings' frame rate.
pub fn restart<R: Runtime>(app: &AppHandle<R>) {
    if let Err(error) = sync_or_restart(app, true) {
        log::warn!("{error}");
    }
}

/// Forgets every subscriber when the page (re)loads: a reloaded page's
/// channels still accept frames, but nothing receives them.
pub fn page_loading<R: Runtime>(app: &AppHandle<R>) {
    if let Some(state) = app.try_state::<VisualizerState>() {
        state.lock().clear();
    }
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        if let Err(error) = sync(&app) {
            log::warn!("{error}");
        }
    });
}

/// Sends the analysis to `channel` as `encode`d frames until
/// `visualizer_unsubscribe` is called with the id returned.
#[tauri::command]
pub fn visualizer_subscribe<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, VisualizerState>,
    channel: Channel<InvokeResponseBody>,
) -> Result<u32, String> {
    let id = state.lock().add(channel);
    if let Err(error) = sync(&app) {
        state.lock().remove(id);
        return Err(error);
    }
    Ok(id)
}

#[tauri::command]
pub fn visualizer_unsubscribe<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, VisualizerState>,
    id: u32,
) -> Result<(), String> {
    state.lock().remove(id);
    sync(&app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn frame<'a>(bands: &'a [f32], left: &'a [f32], right: &'a [f32]) -> AnalysisFrame<'a> {
        AnalysisFrame {
            silent: false,
            bands,
            lowest_hz: 30.0,
            highest_hz: 16000.0,
            chroma: std::array::from_fn(|i| if i == 9 { 1.0 } else { 0.1 }),
            peak: [0.9, 0.8],
            rms: [0.5, 0.4],
            left,
            right,
            onset: 0.25,
            beat: true,
        }
    }

    fn f32_at(bytes: &[u8], offset: usize) -> f32 {
        f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    fn i16_at(bytes: &[u8], offset: usize) -> i16 {
        i16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
    }

    #[test]
    fn encodes_the_documented_layout() {
        let bands = [0.0, 0.5, 1.0, 2.0, -1.0];
        let left = [0.0, 1.0, -1.0];
        let right = [0.5, -0.5, 3.0];
        let bytes = encode(&frame(&bands, &left, &right));

        assert_eq!(bytes[0], FORMAT_VERSION);
        assert_eq!(bytes[1], FLAG_BEAT);
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 5);
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 3);
        let floats: Vec<f32> = (0..7).map(|i| f32_at(&bytes, 8 + 4 * i)).collect();
        assert_eq!(floats, [30.0, 16000.0, 0.9, 0.8, 0.5, 0.4, 0.25]);
        assert_eq!(bytes[36 + 9], 255);
        assert_eq!(bytes[36], 26); // 0.1
                                   // Clamped to 0..=1, then padded to an even length.
        assert_eq!(&bytes[48..54], &[0, 128, 255, 255, 0, 0]);
        let samples: Vec<i16> = (0..6).map(|i| i16_at(&bytes, 54 + 2 * i)).collect();
        assert_eq!(samples, [0, 32767, -32767, 16384, -16384, 32767]);
        assert_eq!(bytes.len(), 54 + 12);

        // The whole frame, which app/tests/visualizer.test.mjs decodes too.
        let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(hex, ENCODED_EXAMPLE);
    }

    const ENCODED_EXAMPLE: &str =
        "01020500030000000000f04100007a466666663fcdcc4c3f0000003fcdcccc3e0000803e\
                                   1a1a1a1a1a1a1a1a1aff1a1a0080ffff00000000ff7f0180004000c0ff7f";

    #[test]
    fn encodes_a_silent_frame_and_the_real_size() {
        let bands = [0.0; 64];
        let samples = [0.0; 512];
        let silent = AnalysisFrame {
            silent: true,
            beat: false,
            ..frame(&bands, &samples, &samples)
        };
        let bytes = encode(&silent);
        assert_eq!(bytes[1], FLAG_SILENT);
        assert_eq!(bytes.len(), 48 + 64 + 4 * 512);
    }

    struct FakeSink {
        received: Arc<AtomicUsize>,
        open: Arc<AtomicBool>,
    }

    impl Sink for FakeSink {
        fn send(&self, bytes: Vec<u8>) -> bool {
            assert_eq!(bytes, [1, 2, 3]);
            self.received.fetch_add(1, Ordering::SeqCst);
            self.open.load(Ordering::SeqCst)
        }
    }

    #[test]
    fn subscribers_get_every_frame_until_they_fail_or_leave() {
        let mut subscribers = Subscribers::default();
        let sink = || {
            let received = Arc::new(AtomicUsize::new(0));
            let open = Arc::new(AtomicBool::new(true));
            let sink = FakeSink {
                received: received.clone(),
                open: open.clone(),
            };
            (sink, received, open)
        };
        let (a, a_received, _) = sink();
        let (b, b_received, b_open) = sink();
        assert!(subscribers.is_empty());
        let a_id = subscribers.add(a);
        let b_id = subscribers.add(b);
        assert_ne!(a_id, b_id);

        assert!(!subscribers.broadcast(vec![1, 2, 3]));
        assert_eq!(
            (
                a_received.load(Ordering::SeqCst),
                b_received.load(Ordering::SeqCst)
            ),
            (1, 1)
        );

        // A sink that fails is dropped.
        b_open.store(false, Ordering::SeqCst);
        assert!(subscribers.broadcast(vec![1, 2, 3]));
        assert!(!subscribers.broadcast(vec![1, 2, 3]));
        assert_eq!(
            (
                a_received.load(Ordering::SeqCst),
                b_received.load(Ordering::SeqCst)
            ),
            (3, 2)
        );
        assert!(!subscribers.remove(b_id));

        assert!(subscribers.remove(a_id));
        assert!(subscribers.is_empty());
        assert!(!subscribers.broadcast(vec![1, 2, 3]));
    }
}
