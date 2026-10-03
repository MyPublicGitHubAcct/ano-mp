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

## Proposals and decisions (§4.6)

Moved from `PLAN.md` on 2026-10-03, unchanged but for the headings.
Section numbers (§) refer to `PLAN.md`, whose §4.6 keeps the
table of items with their decisions or priorities.

- **O1 Loudness analysis for files without ReplayGain tags.** ReplayGain
  applies only to tagged files today. None of the dev library's files are
  tagged, and most players either ignore untagged files or rewrite their
  tags, which this app never does. A background pass decodes each track
  once and measures its integrated loudness and true peak (EBU R128 /
  ITU-R BS.1770), per track and per album. It stores them as computed
  gains. `PlaybackSettings::gain` uses the tag values first, then the
  computed ones, then the preamp for untagged files.
  - Core: a thread-safe `anomp_analyse_file` with its own
    `FFmpegAudioFormat` reader, separate from the engine, with progress
    and cancel callbacks. It returns everything O2–O4 need, so each file
    is decoded once.
  - Rust: an `analysis` worker thread shaped like the metadata worker,
    running at low priority a few files at a time. It can resume where it
    stopped and has a switch in Settings. It writes a `track_analysis`
    table (new migration) invalidated by the track's size and mtime.
  - Tests: EBU Tech 3341 test signals, or synthetic tones of known
    loudness, rendered like the chirp fixtures.
  - Cost: decoding a 50,000-track library takes hours of CPU time once.
  - **Decision:** accepted and built 2026-09-27, off by default for its
    CPU cost. `anomp_analyse_file` (`core/src/FileAnalyser.*`) measures
    a whole file or one part of it; the worker is `library/analysis.rs`
    (a few tracks at a time, the playing track first); results are in
    `track_analysis` and `album_analysis` (migration 006). An album is
    gated over 0.5 LU histograms of its tracks' 400 ms blocks, stored
    per track, so a new track doesn't mean decoding the album again.
    Computed gains are used only while the feature is on
    (`library::playback`).
- **O2 Waveform seek bar.** The analysis also keeps a coarse min/max
  envelope of about 1,000 points a track (about 2 KB as bytes).
  `SeekBar.svelte` draws it, with the played part in the accent colour.
  This shows quiet intros, drops and hidden tracks at a glance, and
  helps O12 set loop points. Until a track is analysed, the seek bar is
  the plain bar it is today. **Decision:** accepted and built
  2026-09-27, on by default. The envelope is 1,000 (min, max) byte
  pairs; the playing track is analysed ahead of the rest, even with O1
  off, so its waveform appears.
- **O3 Segue-aware shuffle and silence handling.** The analysis records
  each track's leading and trailing silence (below −60 dBFS). It marks a
  segue where one track's last 50 ms and the next track's first 50 ms on
  the same album both have sound. Shuffle keeps each run of segued tracks
  together, in order, as a single unit in `queue/model.rs`. Concept
  albums, live albums and DJ mixes then stop cutting mid-phrase, which
  plain shuffle does in almost every player. An optional "skip long
  silence" setting ends a track after N seconds of trailing silence (the
  gap before a hidden track). That needs the engine to accept an end
  position, which O5 adds too. **Decision:** accepted and built
  2026-09-27. Segue shuffle is on by default (`queue::album_units` gives
  each run a shuffle unit; `Queue::shuffle_from` moves runs whole).
  Skipping silence is off by default and differs from the proposal: a
  long silence *inside* a track (a hidden track's gap) is jumped over
  with a one-time skip region in the engine
  (`anomp_track_options.skip_from`/`skip_to`), so the hidden track still
  plays; silence that runs to the end ends the track (the range's end).
- **O4 Library health report.** A read-only view of problems the app can
  detect but otherwise hides. Each row can reveal the file in Finder.
  - Files that fail to decode, or whose decoded length falls short of
    the header's (truncated), found by the O1 pass.
  - Suspected lossy transcodes: "lossless" files (FLAC, ALAC, WAV) whose
    spectrum stops at 16–19 kHz for the whole track, as MP3 or AAC encoders
    leave it. This is a heuristic and is labelled "suspected".
  - Inconsistent albums: tracks of one album with different album
    artists, years or disc totals, and missing or repeated track numbers.
    These checks read only the DB and don't need O1.
  - Likely duplicates: the same recording MBID, or the same normalized
    title and artist with a length within 2 s. Audio fingerprinting stays
    deferred with AcoustID (Phase 4).
  - **Decision:** accepted and built 2026-09-27, on by default:
    `library/health.rs` and the Library health view, with "Reveal in
    Finder" (`opener:allow-reveal-item-in-dir`).
- **O5 Cue sheets and chapters as tracks.** Shows single-file albums
  (FLAC or APE with a `.cue` sheet, or a FLAC with an embedded cue sheet)
  and chaptered files (MP4 chapters, ID3 `CHAP`) as separate tracks.
  - Scanner: reads the `.cue` next to the file through the folder's
    bookmark (UTF-8, or a legacy code page detected) and makes a track
    for each `INDEX 01`, with its start and end in samples. The cue's
    `TITLE` and `PERFORMER` override the file's tags.
  - DB: a track becomes (folder, path, start), not (folder, path). That
    changes the unique key, so the migration rebuilds `tracks` and has to
    keep the FTS triggers (migration 002) in step.
  - Engine: `load` and `set_next` take an optional start and end. The
    engine treats the end as the track's end, so a hand-off to the next
    range of the same file is gapless and sample-exact (the reader's
    seeks are exact).
  - **Decision:** accepted and built 2026-09-27, on by default.
    Migration 005 rebuilds `tracks` keyed by (folder, path, range start)
    and keeps the FTS triggers; the scanner reads a `.cue` next to the
    file (UTF-8, else Windows-1252) or a CUESHEET tag
    (`library/cue.rs`), else chapters FFmpeg finds (MP4, ID3 CHAP, Ogg,
    FLAC cue sheet blocks; `FFmpegAudioFormat::readChapters`). Turning
    it on or off re-reads every file.
- **O6 Classical works and movements.** Most players show "Symphony No. 5
  in C minor, Op. 67: I. Allegro con brio" as one flat title. Tags
  already carry the structure: work, movement name and movement number
  (ID3 `TIT1`/`MVNM`/`MVIN`, MP4 `©wrk`/`©mvn`/`©mvi`, Vorbis
  `WORK`/`MOVEMENTNAME`/`MOVEMENT`), plus composer and conductor.
  - `TagReader` reads them from TagLib's property map. Check TagLib
    2.3's key names when building this.
  - A migration adds the columns, and triggers for any that search
    indexes.
  - Composer and work become browse grouping keys (`library/rules.rs`).
  - Album pages group movements under their work, with the composer and
    performers.
  - "Play work" queues a whole work, and shuffle treats a work as one
    unit, as O3 does segues.
  - Later, MusicBrainz work relationships can fill works for untagged
    files through the metadata worker.
  - **Decision:** accepted and built 2026-09-27 from tags, on by
    default: work, movement, composer (an artist) and conductor in
    `tracks`; Composer and Work browse levels and a default "Composer"
    rule; works grouped on album pages with "Play work"; a work is a
    shuffle unit. MusicBrainz works for untagged files are still later.
- **O7 Per-track and per-album playback preferences.** The user's own
  rules, set from the context menu, shown as a badge in lists, and stored
  in the DB, never in the files. They cascade from their track or album.
  - Skip a track in album and shuffle play (intros, skits, bonus
    tracks). It still plays when chosen directly.
  - Never shuffle an album: shuffle plays it whole, in order.
  - A gain offset in dB added to ReplayGain, passed with the track's
    gain as now.
  - A start or end trim, using O5's ranges.
  - **Decision:** accepted and built 2026-09-27, on by default:
    `track_prefs` and `album_prefs` (migration 006), `library/prefs.rs`,
    the preferences dialog from track and album menus, and a badge in
    lists. Trims use O5's ranges.
- **O8 Local listening history, with opt-in ListenBrainz.** The queue
  adds a row to a `plays` table (track, start time, seconds played) once
  a track passes half its length or 4 minutes, the usual scrobbling
  rule. By default this history stays on the machine.
  - Views few local players offer: albums once played often but not for a
    year, "a year ago today" and never played. O16 and O19 are views on
    this table too. Play count and last played become `TrackColumn`
    options.
  - Opt-in ListenBrainz submission. It is MetaBrainz's service, like
    MusicBrainz, and the MBIDs from tags make its matches exact. The
    user's token lives in the keychain through `metadata::keys`. Listens
    are queued while offline and sent through `http::Client`. Its terms
    are checked before release like every other source (§8.1).
  - **Decision:** accepted and built 2026-09-27. Local history is on by
    default (`history/`: a main-thread tracker, a history thread writing
    `plays`); ListenBrainz is off by default, with the token in the
    keychain (`keys::Account::ListenBrainz`) and listens queued in
    `listens_pending`. Its terms still need checking (§8.1). Plays and
    last played are track columns.
- **O9 Library radio.** When the queue runs out, or on "Start radio from
  this", the app keeps adding tracks from the local library that fit the
  seed track. It scores tracks by shared or related genres, nearby
  years, the same label, and artists linked by MusicBrainz relationships
  (members, collaborations). Tracks played recently in O8's history are
  weighted down. Each pick shows why it was chosen ("same label, 1994").
  Everything is local and no service is called. The logic lives in
  `queue/` as a source of next items, tested against the fake engine.
  **Decision:** accepted and built 2026-09-27, on by default:
  `queue/radio.rs` scores genre, era, label and linked artists;
  MusicBrainz artist lookups now include `artist-rels` (members,
  collaborations, subgroups). "Keep playing when the queue ends" is a
  separate switch, off by default.
- **O10 Signal path panel and sample-rate matching.** Clicking the format
  in the now-playing bar shows the signal path as it actually is:
  1. The file's codec, bit depth, rate and bitrate.
  2. The ReplayGain gain applied.
  3. Resampling from the file's rate to the device's, or none.
  4. The volume.
  5. The device's name, rate and buffer size.

  The engine knows every step, so the panel is small. An opt-in setting
  switches the device to the file's rate when the device supports it, to
  avoid resampling (the known limit in Phase 6). Constraints:
  - Changing the rate interrupts output. It happens only at a track
    start that isn't a gapless hand-off; when the next track's rate
    matches the current one, nothing changes.
  - On macOS the rate is the device's, so it also changes for other apps
    using that device.
  - `AudioEngine` needs to reopen the device with a given rate.

  **Decision:** accepted and built 2026-09-27: the panel is on by
  default (`anomp_engine_signal_path`, from the format badge in the
  playing bar); sample-rate matching is off by default
  (`anomp_engine_set_device_sample_rate`, only at a load, never at a
  gapless hand-off).
- **O11 Headphone crossfeed.** Hard-panned stereo (much of the 1960s)
  tires the ears on headphones. Crossfeed blends a delayed, low-passed
  part of each channel into the other (Bauer's stereo-to-binaural
  method, as in bs2b).
  - A few biquads and a short delay in `PlayerEngine`, after the tap so
    the visualizer still shows the mix. Written in-house, with no new
    dependency.
  - Off and three strengths in the playback settings.
  - Optionally on by itself when the output is headphones, as the OS
    reports: the data source of Core Audio's built-in output, or the
    `AVAudioSession` route on iOS. This goes behind a small platform
    interface.
  - Tested offline through `getNextAudioBlock`, as the other engine tests
    are.
  - **Decision:** accepted and built 2026-09-27, off by default:
    `core/src/Crossfeed.*` after the tap; "only with headphones" (on by
    default) uses `OutputRoute` (Core Audio's data source on the
    built-in output, the audio route on iOS); Bluetooth headphones can't
    be told from speakers.
- **O12 Practice mode.** For musicians learning a part or transcribing
  one, which few library players support.
  - An A–B loop set on the seek bar (easier with O2's waveform). The
    engine jumps back at B on the exact sample, as the gapless hand-off
    does, not on a UI timer.
  - Tempo from 50% to 150% without changing pitch, and optionally a pitch
    shift in semitones. This needs a time-stretcher before the
    resampler, and its licence decides which one:
    - Signalsmith Stretch (MIT, header-only C++) fits the closed-source
      and iOS static-link rules.
    - Rubber Band is GPL or paid.
    - SoundTouch is LGPL, so it would have to ship as a shared library
      like FFmpeg.
  - **Decision:** accepted and built 2026-09-27, off by default.
    Signalsmith Stretch 1.4.0 and its FFT library (MIT, header-only) are
    pinned in `cmake/Signalsmith.cmake`; the A–B loop keeps a second
    reader of the file waiting at A and swaps the two at B. Its quality
    at 50% is still to be checked by ear.
- **O13 Local synced lyrics.** Shows lyrics already on disk.
  - Unsynced lyrics: ID3 `USLT`, Vorbis `LYRICS`/`UNSYNCEDLYRICS`, MP4
    `©lyr`.
  - Synced lyrics: ID3 `SYLT`, LRC text in a lyrics tag, or a `.lrc` file
    next to the track, read through the folder's bookmark as folder
    images are.
  - Synced lines highlight with the position (already sent every 50 ms),
    and clicking a line seeks there.
  - Lyrics are read from the file when shown (a flag on
    `anomp_read_tags`) and never stored in the DB.
  - Online lyrics (LRCLIB and others) stay out of scope. Adding one would
    be a sources-table decision, as in Phase 4.
  - **Decision:** accepted and built 2026-09-27, local only, on by
    default: `library/lyrics.rs` and the Now Playing view.
- **O14 LAN remote control.** Controls the desktop app from a phone's
  browser on the same network, with nothing to install.
  - An opt-in HTTP and WebSocket server in Rust serves a compact remote
    page: now playing, cover, play/pause, next, seek, volume, queue and
    search. Its commands go through the same queue functions as the UI
    and the media keys.
  - Pairing uses a code or QR code shown in Settings and a random token
    for each pairing. The server listens on LAN addresses only, and is
    off by default.
  - Costs: the sandbox's `network.server` entitlement, macOS's
    local-network prompt, and an entry in the privacy policy.
  - A listening socket is attack surface, so it needs a security review:
    token checks, rate limits, and no file access beyond cover art and
    the page itself.
  - The Phase 8 iOS build could later act as a richer remote.
  - **Decision:** accepted and built 2026-09-27, off by default, and
    still needs its security review before a release (§8.1). It differs
    from the proposal: plain HTTP with the page polling once a second
    instead of a WebSocket (no new dependency), and a pairing code with
    the page's address instead of a QR code. The safeguards are listed
    in `remote/mod.rs`; the `network.server` entitlement and the
    local-network usage text are in place.
- **O15 Recently added.** A sidebar view of albums ordered by when their
  newest track arrived, grouped into this week, this month and earlier.
  - `scanned_at` can't serve: every rescan rewrites it. A migration adds
    `tracks.added_at`, which the scanner sets on insert only (the upsert
    leaves it alone). Existing rows take the file's mtime, the nearest
    thing to an arrival date, because their `scanned_at` values are all
    the same first scan.
  - "Date added" also becomes a sort key for the browse rules.
  - Limit: a track's identity is its path, so moving or renaming a file
    makes it look newly added.
  - **Decision:** accepted and built 2026-09-27, on by default:
    `tracks.added_at` (migration 005), a "Date added" sort key and album
    order, and the Home view.
- **O16 Recently played.** Needs O8's `plays` table. Lists what was
  played, newest first. Consecutive plays from one album collapse into a
  single album row ("11 tracks of …"), so an album doesn't fill the list,
  and one click plays it again. It differs from the queue, which holds
  only what is queued now. **Decision:** accepted and built 2026-09-27,
  on by default: `history::views::recently_played`, on the Home and
  History views.
- **O17 Albums released on this day.** Albums whose original release
  date falls on today's month and day, labelled with the anniversary
  ("30 years ago today"). It is shown as a card in a home or sidebar
  view, not as a notification.
  - The date comes first from the MusicBrainz release group's first
    release date, which the stored album details already hold, so a
    reissue counts on the original's date.
  - Otherwise it comes from a full `DATE` tag. `TagReader` keeps only the
    year today, so the full date needs a new column, re-read at the next
    scan as migration 004 did.
  - Albums dated only to a year or month never match. A 29 February
    release shows on 28 February in other years.
  - **Decision:** accepted and built 2026-09-27, on by default:
    `tracks.release_date` from ORIGINALDATE or DATE (migration 005
    re-reads every file), preferring the MusicBrainz release group's
    first date.
- **O18 Five more albums in this genre, at random.** A "More in <genre>"
  row on album pages: five random albums that share a genre with the
  current one, excluding the current album, with a button to draw again.
  - The genres come from the tags through `anomp_has_genre`. An album
    with several genres gets a chip for each, and the chips switch the
    row.
  - The five stay the same while the page is open. Drawing again picks a
    new five.
  - With O8, albums played recently are drawn less often.
  - A random order over the matching albums is cheap at 50,000 tracks
    (a few thousand albums). Check it with the ignored benchmarks.
  - **Decision:** accepted and built 2026-09-27, on by default:
    `library::discover::more_in_genre`, on album pages.
- **O19 Top 20 played by year or month.** Needs O8. The 20 most played
  tracks, albums and artists for a chosen year or month (a "year in
  review"), counted as plays that passed O8's rule. "Play these 20" puts
  them in the queue, and the period can step back and forward.
  **Decision:** accepted and built 2026-09-27, on by default:
  `history::views::top_played`, on the History view.
