# Phase 0 — Toolchain and integration spike (highest risk first)

Moved from `PLAN.md` (H20) on 2026-10-03, unchanged but for this
heading. Section numbers (§) refer to `PLAN.md`, whose Phase 0
keeps the phase's status, decisions and open steps.

- [x] Install the prerequisites in §3.
- [x] Scaffold `app/` with `npm create tauri-app` (Tauri 2, Svelte + TypeScript
  template; see §4.2). The template is SvelteKit with `adapter-static` (SPA,
  no SSR); kept for its routing. Bundle identifier `dev.anomp.player` is a
  placeholder — choose the real one before §8 (it is hard to change after
  the first store release).
- [x] `build.rs`: build `anomp_core` via the `cmake` crate, link the static lib plus
  the Apple frameworks (CoreAudio, AudioToolbox, CoreMIDI, Accelerate,
  AVFoundation, Foundation, AppKit).
- [x] Call `anomp_version()` from a Tauri command and show it in the UI.
- [x] **Spike: JUCE inside a Tauri process.** Tauri (tao) owns the main thread and
  run loop. Verify that JUCE's `AudioDeviceManager` plays audio when
  initialised via `ScopedJuceInitialiser_GUI` without a `JUCEApplication`, and
  that device-change notifications still arrive.
  Findings (2026-09-25):
  - Works with no JUCE changes. Without a `JUCEApplication`, JUCE leaves
    `NSApp`'s delegate alone and posts its messages to a CFRunLoop source on
    the *main* run loop, which tao's `[NSApp run]` services. The one rule:
    create, use and destroy the engine on the main thread.
  - `AudioEngine` (core) owns the `ScopedJuceInitialiser_GUI`, the
    `AudioDeviceManager` and a test-tone source; the C API is
    `anomp_engine_*` plus one event callback (`ANOMP_EVENT_DEVICE_CHANGED` for
    now; Phase 1 adds playback events to the same callback).
  - Rust keeps the engine in a main-thread `thread_local`, reaches it from
    commands directly or through `run_on_main_thread`, and drops it on
    `RunEvent::Exit` so JUCE shuts down before the process exits.
  - Device changes checked by creating and removing a public CoreAudio
    aggregate device while the app runs: each change reached the Svelte UI
    as an `audio-device-changed` event.
