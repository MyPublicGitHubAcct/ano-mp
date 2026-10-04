//! Waiting, with a bound, for background threads as the app quits.
//!
//! Tauri ends the process (`std::process::exit`) as soon as `lib.rs` has
//! handled `RunEvent::Exit`, whatever other threads are doing. A thread
//! with something to finish (a play to write, an analysis result to
//! store) holds a `Running` and drops it as it ends; `lib.rs` waits for each
//! one's `Finished` until a deadline shared by all, `WAIT` after quitting
//! began, so a slow network request never holds the quit up.
//!
//! Other threads may still be inside the core when the process ends: a
//! scan or a cover reading tags, Get Info, a file opening. `exit` would run
//! the C++ static destructors (the core's format registry, TagLib's and
//! JUCE's singletons) under them, and the quit could crash. So
//! `skip_static_destructors`, called last on `RunEvent::Exit`, registers an
//! `atexit` handler: registered after those statics were built, it runs
//! before their destructors, flushes the log and ends the process with
//! `_exit`. Tauri's own steps after the handler (its cleanup, a restart's
//! relaunch) still run, since they come before `exit`.

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, TryRecvError};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The longest quitting waits for the threads, all of them together.
pub const WAIT: Duration = Duration::from_secs(1);

/// Held by a thread until it ends.
pub struct Running {
    _sender: mpsc::Sender<()>,
}

/// Says when the thread holding the matching `Running` has ended.
pub struct Finished(Mutex<mpsc::Receiver<()>>);

/// A thread's `Running`, to move into it, and its `Finished`.
pub fn track() -> (Running, Finished) {
    let (sender, receiver) = mpsc::channel();
    (Running { _sender: sender }, Finished(Mutex::new(receiver)))
}

impl Finished {
    /// Waits until the thread has ended or `deadline` has passed; whether
    /// it ended. A thread still running is noted in the log as `name`.
    pub fn wait(&self, name: &str, deadline: Instant) -> bool {
        let receiver = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let ended = match receiver.try_recv() {
            Err(TryRecvError::Disconnected) => true,
            _ => matches!(
                receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())),
                Err(RecvTimeoutError::Disconnected)
            ),
        };
        if !ended {
            log::info!("{name} still running at quit");
        }
        ended
    }
}

/// The exit code the app was asked to end with (`RunEvent::ExitRequested`).
static EXIT_CODE: AtomicI32 = AtomicI32::new(0);

/// Notes the exit code asked for, if any, for `skip_static_destructors`.
pub fn exit_requested(code: Option<i32>) {
    if let Some(code) = code {
        EXIT_CODE.store(code, Ordering::Relaxed);
    }
}

/// Makes the process end, when `exit` comes, before the C++ static
/// destructors run. Call once, last on `RunEvent::Exit`, on the main thread.
pub fn skip_static_destructors() {
    // SAFETY: `end` is a plain `extern "C" fn()` that never returns or
    // unwinds, which is all `atexit` asks of it.
    if unsafe { libc::atexit(end) } != 0 {
        log::warn!("cannot register the end of the process");
    }
}

extern "C" fn end() {
    log::logger().flush();
    // SAFETY: `_exit` ends the process at once, running nothing else; the
    // engine and the queue were shut down on `RunEvent::Exit`.
    unsafe { libc::_exit(EXIT_CODE.load(Ordering::Relaxed)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_the_end_or_the_deadline() {
        let (running, finished) = track();
        let started = Instant::now();
        assert!(!finished.wait("test", started + Duration::from_millis(50)));
        assert!(started.elapsed() >= Duration::from_millis(50));

        let thread = std::thread::spawn(move || {
            let _running = running;
            std::thread::sleep(Duration::from_millis(20));
        });
        assert!(finished.wait("test", Instant::now() + Duration::from_secs(5)));
        thread.join().unwrap();
        // Ended already, even with the deadline gone.
        assert!(finished.wait("test", Instant::now()));
    }

    /// Set for `exit_helper` when `exit_skips_what_was_registered_before`
    /// runs it in a process of its own.
    const HELPER: &str = "ANOMP_EXIT_HELPER";

    #[test]
    fn exit_skips_what_was_registered_before() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("destructor-ran");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "quitting::tests::exit_helper", "--ignored"])
            .env(HELPER, &marker)
            .status()
            .unwrap();
        // The code asked for, and nothing registered earlier (as a static
        // destructor is) ran.
        assert_eq!(status.code(), Some(3));
        assert!(!marker.exists());
    }

    #[test]
    #[ignore = "run by exit_skips_what_was_registered_before"]
    fn exit_helper() {
        static MARKER: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
        extern "C" fn destructor() {
            if let Some(marker) = MARKER.get() {
                let _ = std::fs::write(marker, "ran");
            }
        }
        let Some(marker) = std::env::var_os(HELPER) else {
            return;
        };
        MARKER.set(marker.into()).unwrap();
        // SAFETY: `destructor` is a plain `extern "C" fn()`.
        unsafe { libc::atexit(destructor) };
        exit_requested(Some(3));
        skip_static_destructors();
        std::process::exit(0);
    }
}
