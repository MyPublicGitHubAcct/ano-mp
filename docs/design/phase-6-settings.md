# Phase 6 — Admin / settings screen

Moved from `PLAN.md` (H20) on 2026-10-03, unchanged but for this
heading. Section numbers (§) refer to `PLAN.md`, whose Phase 6
keeps the phase's status, decisions and open steps.

- [x] Settings are persisted in SQLite, with a typed schema in Rust and
  shared TS types (generated with ts-rs).
- [x] Sections: displayed fields and columns, enabled services, library
  folders and rescan, sort/grouping rules, visualization choice and
  parameters, audio output device and buffer size (desktop), and
  ReplayGain. The visualizer's choice and cover-wall basis moved here from
  `localStorage`.

  Built 2026-09-26. Design:
  - **Storage**: `settings.rs` keeps `AppSettings` (display, playback,
    output, visualizer) as one JSON value under `app` in `settings`; the
    sort rules (`library.sort`) and online sources (`metadata.services`)
    keep their keys and commands. Reading keeps each stored value that
    still parses and validates, field by field, and a list keeps the items
    it can, so a setting written by an older or newer version costs only
    that setting. `settings_save` validates the whole value, applies it,
    stores it and emits `settings-changed`.
  - **Types**: ts-rs (a dev-dependency, so not in the app) generates
    `app/src/lib/generated/settings.ts` for every settings type (these,
    the sort rules, the online sources, the output status); a test fails
    when the file is stale (`ANOMP_WRITE_BINDINGS=1 cargo test bindings`
    rewrites it). The other payloads are still hand-written in `api.ts`;
    moving them over is mechanical (derive `TS`, list the type in
    `bindings`).
  - **Output device**: `AudioEngine::openDevice` opens a device by name
    (of the platform's default type, Core Audio here) with a buffer size,
    at the device's own rate; an unknown name fails without touching the
    open device. The settings keep the name (null for the system default)
    and the buffer size (null for the device's). A device that won't open
    falls back to the default, and saving one reopens the previous and
    saves nothing. While the chosen device is unplugged the default plays
    (JUCE does that); when the device list shows it again, `audio.rs`
    reopens it, once per device list if that fails.
  - **ReplayGain**: `TagReader` reads the REPLAYGAIN_* tags in every
    format and Opus R128 gains (converted: Q7.8 dB + 5 to ReplayGain's
    level); migration 004 stores them and marks every file for re-reading
    at the next scan. Each `PlayerEngine` track has its own gain, applied
    as it's read (before the resampler, the tap and the volume), so a
    hand-off switches gain on the exact sample; the host passes it with
    `load`/`set_next`, and `anomp_engine_set_track_gain` changes it by
    file, which can't race a hand-off. `PlaybackSettings::gain` picks
    track or album gain (each falling back on the other), adds the preamp
    (or uses the untagged gain), limits it by the matching peak if asked,
    and clamps to +18 dB. A new setting reaches the current and armed
    tracks at once (`queue::refresh_gains`).
  - **Display**: track lists show the chosen fields as columns beside the
    title when the list is at least 44rem wide, under it when narrower
    (`TrackText.svelte`); the number leads and the length ends the row.
    Album pages show the chosen release facts in order, and descriptions
    and biographies can be turned off.
  - **Visualizer**: choice and cover wall basis, the analysis frame rate
    (a change restarts the running analysis), a sensitivity that scales
    the bands, colours from the cover or not, and changing visualization
    on a timer. The old `localStorage` choices are carried over once.
  - **UI**: one Settings view (⌘, or the sidebar) with sections down the
    side (along the top when narrow); the sidebar's Online sources item
    opens its section. Everything saves as it changes, except a sort rule's
    edits, which save together.
- **Known limits**: only the default device type is listed (on Windows,
  WASAPI but not ASIO; Phase 10); the sample rate follows the device's;
  "System default" opens the default at the time and doesn't follow later
  changes to it; the queue's items don't use the display columns.
