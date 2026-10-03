# Phase 5 — Visualization

Moved from `PLAN.md` (H20) on 2026-10-03, unchanged but for this
heading. Section numbers (§) refer to `PLAN.md`, whose Phase 5
keeps the phase's status, decisions and open steps.

- [x] Core: lock-free FIFO tap on the output; FFT (`juce::dsp::FFT`) → log-spaced
  bins plus peak/RMS levels, published at ~30–60 Hz via callback.
- [x] Rust forwards the frames over a Tauri `Channel` (binary/compact payload,
  not per-frame JSON if it proves costly).
- [x] Frontend: canvas/WebGL renderers (spectrum bars, oscilloscope, VU) that
  users pick in preferences.
- [x] A cover wall (covers of albums from the current track's year or
  by its artist), and unusual visualizations: ridgelines, the circle of
  fifths with a key estimate, a vectorscope, a kaleidoscope of the cover.

  Built 2026-09-26. Design:
  - **The tap** (`SignalTap`) is a ring of relaxed atomic floats (32768
    samples a channel) that `PlayerEngine` writes each block while
    playing, before the volume (so turning it down doesn't shrink the
    visuals) and not while paused or stopped. Readers copy the latest
    window without taking it; a race costs at worst a glitched frame.
  - **Analysis** (`SpectrumAnalyser`) takes the latest 8192 samples: bands
    from a 2048-point FFT (about 43 ms, so the bars keep up), each band's
    loudest bin on a 70 dB scale tilted +3 dB an octave around 1 kHz;
    chroma (12 pitch classes, 80 Hz–5 kHz) from an 8192-point FFT for
    semitone resolution; peak and RMS over 2048 samples; a waveform
    starting at the latest rising zero crossing (a triggered scope); and
    beats, a bass (below 250 Hz) flux 1.5 deviations above its last
    1.5 s, at most 4 a second.
  - **`AnalysisThread`** runs only while a callback is set, at a steady
    rate (60 fps from Rust), and only when the tap has new samples. After
    150 ms without any it sends one silent frame and then nothing, so a
    paused player costs no IPC. The callback is on that thread, never the
    main one; the C API says it mustn't call the engine.
  - **Rust** (`visualizer.rs`) starts the analysis for the first
    subscriber and stops it after the last, on the main thread, never
    holding the subscriber lock while it waits for the analysis thread.
    Frames are 2.2 KB of binary (`encode` documents the layout: bands
    and chroma as bytes, the waveform as 16-bit), which go through
    Tauri's fetch path (over 1 KB). A page load drops all subscribers,
    since a reloaded page's channels accept frames that nobody reads.
  - **The cover wall** asks `library_cover_wall` for albums from the
    track's album year (only that year: widening to nearby years mixed
    them in, since tiles pick at random), or by the track's performer
    (their albums and those they appear on).
    The UI loads each cover as a 256 px thumbnail, leaves out albums
    without one, maps tiles to bands by distance from the centre (bass in
    the middle) and flips tiles on beats.
  - Every visualization takes its colours from the current cover
    (`palette.ts`); the art scheme now sends
    `Access-Control-Allow-Origin: *` so a canvas may read a cover's
    pixels. The VU meters put 0 VU at -12 dBFS RMS, not the studio's -18,
    which mastered music would pin.
  - The choice of visualization and the wall's year/artist switch are per
    viewer (`localStorage`) until Phase 6 moves them into the settings.
    Full screen (F, or double-click) is the window's, and shows the
    visualizer alone; V cycles the visualizations.
- **Known limits**: the bands' lowest octave has less than a bin each
  at 2048 points and is interpolated; the key estimate needs about ten
  seconds of music and trusts tonal music; the analysis sees what the
  player renders, a few blocks ahead of the speakers (output latency),
  and the bands lag about 20 ms behind the latest sample, which roughly
  cancel.
