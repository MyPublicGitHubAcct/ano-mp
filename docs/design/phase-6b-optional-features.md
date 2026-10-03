# Phase 6b — Optional features (O1–O19)

Moved from `PLAN.md` (H20) on 2026-10-03, unchanged but for this
heading. Section numbers (§) refer to `PLAN.md`, whose Phase 6b
keeps the phase's status, decisions and open steps.


Built 2026-09-27: all nineteen features of §4.6, each with a switch (and
its options) in Settings › Features. `FeatureSettings` in `settings.rs`
holds them, read leniently like the rest of the settings; `settings_save`
applies a change at once (the analysis starts or stops, the remote
listens or stops, crossfeed and practice reset, the queue's skips and
shuffle units follow, and turning cue sheets on or off re-reads every
file).
- **Core**: `FileAnalyser` (loudness, peaks, silences, spectrum cutoff,
  waveform) behind `anomp_analyse_file`, over a whole file or a part;
  more tags from `anomp_read_tags` (work, movement, composer, conductor,
  full dates, and with flags lyrics, SYLT as LRC, chapters and CUESHEET);
  the engine plays parts of files (`anomp_track_options`: start, end,
  one skip region), loops A–B on the exact sample, stretches tempo and
  pitch (Signalsmith Stretch), crossfeeds after the tap, reports the
  signal path and switches the device's sample rate; `OutputRoute` says
  whether the output is headphones.
- **Library**: migration 005 rebuilds `tracks` for parts of files, the
  arrival date, release dates and classical tags (keeping ids and the
  search index, which gains work and composer); migration 006 adds the
  analysis, plays, pending listens, preferences and paired phones. The
  scanner splits files by cue sheet or chapters; `library::playback`
  decides how each track is played; `analysis`, `discover`, `health`,
  `lyrics` and `prefs` are the features' library code.
- **Queue and history**: shuffle units, skips, radio mode and its
  refills in `queue/`; the play tracker, `plays`, ListenBrainz and the
  history's views in `history/`; the LAN remote in `remote/`.
- **UI**: the Features settings section; Home (released on this day,
  recently played, recently added, the history's highlights), History
  (top 20 of a year or month, recently played) and Library health
  views; the waveform seek bar; the signal path and practice panels in
  the playing bar; lyrics in Now Playing; works and "More in this genre"
  on album pages; "Start radio" and "Playback preferences…" in menus;
  new track columns, sort keys and browse levels.
- **Tests**: 89 Catch2 tests (EBU Tech 3341-style loudness cases, true
  peak, silences, cutoff, parts of files, gapless hand-off between parts,
  skip regions, loops with and without read-ahead, tempo and pitch,
  crossfeed, the new tags, SYLT and chapters) and 300 `cargo test` tests
  (migrations, cue sheets, the scanner's parts, playback options,
  analysis storage and album gating, shuffle units, skips, radio,
  history and its views, discovery, health, lyrics, preferences,
  ListenBrainz with the fake transport, the remote's HTTP parsing,
  pairing and tokens). A scan and analysis of the dev library ran clean.
- **Known limits**: analysing a large library takes hours (debug builds
  about 7 s for a five-minute track); crossfeed's "only with headphones"
  can't tell Bluetooth headphones from speakers; the remote polls rather
  than pushing and pairs by code rather than QR code; MusicBrainz works
  (O6) and online lyrics stay out; user data keyed by track (plays,
  preferences, analysis) is lost when a file moves (F10).
