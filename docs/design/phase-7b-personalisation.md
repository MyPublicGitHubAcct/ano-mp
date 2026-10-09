# Phase 7b — Personalisation, effects and discovery (macOS): the record

Moved from `PLAN.md` (D4) on 2026-10-09, unchanged but for the headings.
Corrected 2026-10-09: a garbled list of X6's formats and a line break in
X2, and X8's "Checked in the app" bullet moved after its record of the
build.
Section numbers (§) refer to `PLAN.md`, whose
[Phase 7b](../../PLAN.md#phase-7b--personalisation-effects-and-discovery-macos)
keeps the phase's priority and rules, the X1–X9 table with each item's
status, open work (X4's album menus) and the phase's exit.

## X1 Themes

- [x] **X1 Themes.** The UI's colours, fonts, density, corner radius and
  the cover-derived accent become design tokens (CSS custom properties on
  `:root`), which every component already reads or is moved to read. The
  user picks a built-in theme (light, dark, high contrast, and a few
  others) or edits one in Settings › Appearance, with a live preview. A
  theme is stored in `AppSettings` as token values, each validated
  (colours, a fixed list of fonts, numbers in range), never as raw CSS,
  so the CSP and `validate` stay meaningful. Themes export and import as
  JSON. High contrast and the system's light/dark and larger text sizes
  keep working (F18); a theme that fails WCAG AA contrast for text is
  flagged in the editor.
  - **Built** 2026-10-03, ahead of Phase 7's exit at the owner's request.
    `AppSettings.appearance` (`theme.rs`) holds the theme in use, the
    user's saved themes (by name, at most 50) and whether the system's
    "Increase contrast" swaps in the high-contrast colours (on by
    default). A theme is a light and a dark palette of twelve `#rrggbb`
    colours, a scheme (system, light, dark), a font from a fixed list
    (system, rounded, serif, mono, Avenir), a text size (12–22 px), a
    density (compact, regular, roomy), a corner radius (0–16 px) and
    "accent from the cover". The twelve built-ins (Standard, Light, Dark,
    High contrast, Paper, Midnight, Forest, Ocean, Rose, Graphite, Sunset,
    Meadow) are `app/src/lib/themes.json`,
    which Rust reads for the default and `contrast.test.mjs` checks for
    AA in both schemes. `lib/theme.ts` turns a theme into custom
    properties (hover, selection and shadow derived from the colours) and
    the cover's colour into a readable accent; `state/appearance.svelte.ts`
    sets them on `:root` in every window, first before the first paint.
    Radii and control padding read the tokens; list rows scale with text
    size and density (`appearance.rowHeight`). The editor previews colours
    and sliders as they move and saves when let go, flags each pair under
    AA, and shows the palette it edits in a sample. Theme files are
    `{"format": "ano-mp theme", "version": 1, "theme": …}`, imported as
    leniently as stored settings (`theme_import`). Switch: `themes`, on by
    default; off, the standard theme shows and Appearance is hidden.
    Left to check in the app: the exit's theme round trip between Macs,
    VoiceOver over the editor, and layouts at 22 px text.

## X2 Effects

- [x] **X2 Effects.** An effects chain in the core, after decoding and
  before volume, next to the equaliser (F15) and crossfeed (O11): reverb
  (JUCE's `dsp::Reverb`, then perhaps convolution with bundled impulse
  responses), chorus (`dsp::Chorus`) and spectral freeze (an STFT that
  holds the current magnitudes with randomised phases while held). Each
  effect has a bypass, a wet/dry mix and a few parameters, smoothed so
  changes never click; the chain's order is fixed at first. Parameters
  cross the C API as plain values and reach the audio thread lock-free.
  Settings are global with optional presets; per-track settings (O7) only
  if asked for. The signal path panel (O10) lists active effects. Off by
  default (it changes what is heard). Decide first: whether `juce_dsp`
  joins the core's modules (it has no GUI dependency), the CPU budget at
  the highest sample rate (a benchmark in `BenchTests.cpp`, H18), and how
  effects behave across a gapless hand-off and a crossfade (reverb tails
  carry over; freeze releases on a track change). Offline render tests in
  the style of `PlayerEngineTests.cpp`.
  - **Built** 2026-10-03, ahead of Phase 7's exit at the owner's request,
    as a library of its own: `effects/` (`anomp_effects`), plain C++20
    with no dependencies, which the core links privately. **Decided:**
    not `juce_dsp`'s effects: JUCE's CMake modules compile their sources
    into every target that links them, so a second JUCE library beside
    the core would carry JUCE twice, and a library without dependencies
    builds, tests and benchmarks alone and ports with nothing else. It
    takes the core's warning flags (JUCE's flag targets, which compile no
    JUCE code) and has its own Catch2 suite (`effects/tests`).
    `EffectChain.h` is its one public header: the catalogue (each
    effect's id, default mix, and parameters with unit, range, default
    and whether they move logarithmically) and the chain.
  - Eight effects, run in a fixed order: spectral freeze, lo-fi
    (bits, sample rate), tremolo and auto-pan (rate, depth, stereo),
    phaser (six allpasses; rate, depth, feedback), flanger (rate, depth,
    feedback), chorus (two voices a channel; rate, depth), echo (time,
    feedback, tone in the feedback, spread from straight to ping-pong; a
    new time crossfades over 50 ms rather than bending the pitch) and
    reverb (Freeverb's combs and allpasses, public domain, scaled to the
    rate; size, damping, width, pre-delay). Convolution with impulse
    responses wasn't built. The freeze keeps the magnitudes of the last
    0.15 s (an 8192-point Hann FFT at 44.1 and 48 kHz, longer above) and
    resynthesises them every quarter window with random phases, each
    bin keeping the phase difference between the channels so the image
    stays put, at the input's loudness; holding and letting go crossfade
    at equal power over its `fade`.
  - **The chain**: each effect's switch, mix and parameters are atomics,
    set from any thread and read by the audio thread every 64 samples;
    every change glides. An effect switched on fades in over 50 ms; one
    switched off stops taking input and lets its tail (reverb, echo)
    ring out before its wet signal fades. With every effect off the
    signal is untouched (the engine's exact-output tests still pass).
    The chain runs after the resampler and before the analysis tap, so
    the visualizer shows what the effects make; the equaliser,
    crossfeed and volume follow. Tails carry across a gapless hand-off
    and a crossfade; a held freeze lets go when a track takes over or
    another is loaded. While paused nothing is processed, so a tail
    resumes with the music.
  - **C API**: `anomp_effect_describe`, `anomp_engine_set_effect`
    (switch, mix, parameters; out of range refused, compared as the
    floats the core keeps), `anomp_engine_set_freeze`,
    `anomp_engine_freeze_held`; the signal path gains `effects` (a bit
    per effect on or ringing out) and `freeze_held`.
  - **Rust** (`effects.rs`): `AppSettings.effects`, each effect's switch,
    mix and parameters by id, validated against the core's catalogue;
    the `effects` feature switch, off by default (it changes what is
    heard), turns them all off. Commands: `effects_catalog`,
    `effects_preview` (plays settings without saving, while a slider
    moves), `effects_freeze`, `effects_status`.
  - **UI**: Settings › Effects, always listed (hidden at first while
    off, it couldn't be found), its first switch the feature's own (also
    in Features), the rest greyed out while off: a preset
    (eleven, `lib/effects.ts`, each replacing every effect's settings),
    then each effect in chain order with its switch, mix and parameters
    (logarithmic sliders for rates and times), heard as they move and
    saved when let go; the freeze's Hold button follows a new track
    letting go. While the freeze is on, a snowflake button in the
    now-playing bar holds and lets go too (`state/effects.svelte.ts`,
    shared with Settings, asking the engine again as the track changes
    and every second while held, since the core lets go by itself). The
    signal path panel lists the effects in use and a held freeze. Global
    settings only; per-track ones (O7) weren't asked for.
  - **CPU** (`core.effects.all_192k`, H18): all eight on, the freeze
    held, at 192 kHz in 512-sample blocks, 13.9 times real time
    on the baseline machine; its budget is at least 10.
  - Left to check in the app: each effect by ear, including during a
    gapless hand-off, a crossfade and a seek, with no clicks or dropouts
    at the smallest buffer size.

## X3 Ten visualizations

- [x] **X3 Ten visualizations** that few players have, beside Phase 5's.
  The five candidates, which the owner took, and on 2026-10-04 five more
  at the owner's request, two of them combinations of the others:
  - a **Tonnetz**: the harmonic lattice, lit by the chroma Phase 5
    already computes, so chords and modulations show as moving shapes;
  - a **recurrence plot** that builds up over the track, showing its
    repeats (verses, choruses) as a self-similarity matrix;
  - **cymatics**: Chladni plate figures driven by the dominant partials;
  - a **phase portrait**: a delay-embedded attractor of the waveform,
    whose shape follows timbre;
  - a **pitch spiral**: the spectrum wrapped one octave per turn, so
    notes line up along spokes and harmonics form patterns;
  - a **harmonograph**: damped-pendulum figures tuned, in just
    intonation, to the interval between the two loudest notes;
  - **rhythm rings**: one bar a turn, each bar a ring, the onsets marked
    where they fall, so a repeating rhythm lines up from ring to ring;
  - a **stereo stage**: each band placed between the speakers by its
    balance, bass at the bottom;
  - **Resonance** (a combination): cymatics with the phase portrait
    over it;
  - **Harmony** (a combination): the pitch spiral beside the Tonnetz.

  Each takes the cover's colours and obeys the flash guard and
  `prefers-reduced-motion` (F18). New analysis (the recurrence plot's
  features) goes in the core's analysis thread and the frame encoding,
  with tests on synthetic signals as in Phase 5. Check each one's CPU
  cost at Retina size.
  - **Built** 2026-10-04, ahead of Phase 7's exit at the owner's request.
    **New analysis** (`SpectrumAnalyser`, tests in `AnalysisTests.cpp`):
    `notes`, 84 semitones from C2 (MIDI 36) to B8, each the loudest
    component within half a semitone on the bands' scale, read from the
    chroma's 8192-point transform; and `balance`, per band, where its
    sound sits from left (-1) to right (+1), from 2048-point transforms
    of each channel, 0 for a band under the floor. Both cross the C API
    at the end of `anomp_analysis_frame` (`note_count`, `lowest_note`,
    `notes`, `balance`) and the frame format, now version 2 (`encode`:
    the notes as bytes, the balance as signed bytes; 2.3 KB a frame).
    **Decided:** the recurrence plot's features stay in the frontend, not
    the core: the chroma and bands each frame carries are its features,
    so the core has nothing new to compute for it.
  - **Pure logic** (`lib/visualizer/music.ts`, `recurrence.ts`; tests in
    `tests/visualizations.test.mjs`): the strongest triad (its weakest
    note above the mean of the other nine), the strongest interval named
    the consonant way round, the Tonnetz's lattice (fifths along a row,
    major thirds up), the Chladni mode each note rings (the square
    plate's modes in order of m² + n²), the embedding delay (the
    autocorrelation's first zero), and the tempo (`TempoTracker`): the
    autocorrelation of the last 8 s of onset strength, spread ±40 ms,
    over 50–200 bpm, leaning gently towards 120 bpm, with the beats where
    the onsets line up best on that period. A first version fitted the
    core's discrete beats and, on real music, never found a tempo:
    onsets are single-frame spikes and real beats are irregular. Compared
    on five synthetic grooves at 60–180 bpm, this finds 158 of 180, every
    miss an octave off at the extremes (60 read as 120, 180 as 90). Its
    threshold is low, so music without a pulse may show a tempo. The
    recurrence plot sums a feature (the chroma, and the bands pooled
    into eight about their mean) over half-second steps in a 256-step
    grid, and when it fills merges pairs and doubles the step, so the
    whole track always fits; it starts again with each track.
  - **Renderers** (`lib/visualizer/renderers/`): `tonnetz`,
    `recurrence`, `cymatics`, `portrait`, `spiral`, `harmonograph`,
    `rhythm`, `stage`, `resonance`, `harmony`, listed after Phase 5's.
    The combinations are `combine.ts`'s `overlay` (the top one drawn into
    a layer of its own, where `clearStage` fades to transparent; the
    scene's `layer`) and `sideBySide` (one above the other on a tall
    stage). Names, descriptions and their few
    words (chords, intervals, tempo) are in `en.json`.
  - **CPU** at Retina size (640 × 400 points at 2×), measured in an
    offscreen WKWebView on synthetic frames, mean per draw including the
    canvas's flush: under 2 ms for the Tonnetz, recurrence plot, phase
    portrait, harmonograph, rhythm rings and stereo stage; 7 to 10 ms
    for cymatics, the pitch spiral and both combinations, with 95% of
    draws within 5 ms. The stereo stage's 64 glows are sprites painted
    once per palette: drawn as gradients they cost 52 ms.
  - Left to check in the app: each one on real music (the recurrence
    plot on a song with a chorus, the rhythm rings' tempo against a
    known one), in calm mode, full screen on a Retina display, and in
    the auto-cycle.

## X4 Recommendations from the library

- [x] **X4 Recommendations from the library.** "More like this" for a
  track, album or artist, and a Home row of library items the user hasn't
  played lately that resemble what they have. Offline and local: scores
  from shared genres, artists and credits (MusicBrainz relations already
  fetched), era, the analysis (O1's loudness, Phase 5's key and tempo if
  stored), and co-listening in the history (O8). It extends what radio
  (O9) and O18 already pick, sharing their code, and leaves out
  unreadable folders (`availability::unreadable`). The scoring is a pure
  function, tested on a fixture library; run the benchmarks (H18) on a
  50k-track library.
  - **Built** 2026-10-04, ahead of Phase 7's exit at the owner's request,
    in `library/similar.rs`. An item (a track, an album, an artist, or
    the user's taste) is a **profile** of its tracks, each weighted: the
    share of them in each genre (folded, so "jazz" is "Jazz"), its
    artists (every credited one and the album artist, never Various
    Artists), composers and its albums' MusicBrainz labels (the heaviest
    few of each), its era (the weighted median year, only while the
    middle half spans at most ten years, so an artist across decades has
    none), and its mean loudness where O1 has analysed it. **Decided:**
    key and tempo aren't used: Phase 5 computes them live for the
    visualizer and stores neither, and storing them would mean a second
    analysis pass; loudness is the analysis that counts.
  - **The score** (`score`, pure, tested on hand-made profiles): a
    genre, 3 points times the share both have in common, up to one more
    for a second; the era, 2 within two years, 1 within five; a label, 3;
    an artist linked to the seed's (radio's: band members, subgroups and
    collaborations on MusicBrainz either way, or sharing an album in the
    library), 4; a shared artist, 2 between albums and 1 between tracks
    ("more by"); a shared composer who isn't a shared artist, 2;
    listening sessions together (plays no more than 30 minutes apart,
    O8), 2 for one, rising to 4 at three; and a point for loudness within
    1.5 LU, only beside something else. Recommendations need 3 (a genre
    alone is enough, an era alone isn't); the two strongest reasons worth
    a point each go to the UI as data (`Reason`) and are worded there
    (`lib/similar.ts`, `similar.reason.*`).
  - **What is recommended**: for a track, tracks on other albums (one an
    album, two an artist); for an album, albums (two an album artist);
    for an artist, artists, which X7's library half can show as they
    are. Candidates are made of their tracks outside unreadable folders,
    so an album only there is never one; a seed is all of its tracks
    wherever they are. **Home's "You might like"** takes the taste from
    the last 90 days' plays (a point a play) and the favourites (a track
    2, an album or an artist 3 spread over its tracks), or the last 200
    plays when nothing was played lately, and suggests albums none of
    whose tracks were in it, nor favourite albums, two an album artist,
    each raised by up to 15% by the day so the row changes daily.
    **Radio** (O9) now scores with the same function, keeping its
    randomness, its penalty for last week's plays and its limits, so it
    gains composers and sessions; its reasons stay its own few English
    words, as the queue shows them. O18 stays random by design.
  - Commands `library_similar_tracks`, `_albums`, `_artists` and
    `library_for_you`, behind the `recommendations` switch (on by
    default: local and cheap). UI: "More Like This" on tracks', albums'
    and artists' context menus and on the artist page opens a dialog
    (tracks with Play All and Add All to Queue; album cards; artists);
    album pages show a "More like this" row under "More in this genre";
    Home shows "You might like" first. Each hides while it has nothing.
  - **Benchmarks** (`bench_recommendations`, `similar.*`, budget 300 ms)
    on the 50k-track library with 20,000 plays: 58 to 82 ms for each
    kind, Home and a radio refill, every one reading the whole library
    (no cache to keep in step).
  - Left to check in the app: recommendations that make sense on the
    owner's library (the exit), and whether the weights want tuning.

## X5 Recommendations from outside the library

- [x] **X5 Recommendations from outside the library.** Artists and
  releases the user doesn't own, seeded from their library and history:
  candidates are ListenBrainz's similar-artist and recommendation data
  and MusicBrainz relations, checked first against each source's terms
  as in Phase 4's sources table (Last.fm stays excluded). Through
  `http::Client` and the metadata worker, off by default, named in the
  privacy policy (§8.1); what is sent (artist ids, not the library) is
  shown in Settings. Results are links out (MusicBrainz, the artist's
  site, Bandcamp's page; through `webLink`), never streams or downloads,
  and anything the user already owns is filtered out by MBID and folded
  names. Dismissed suggestions are remembered. Recorded fixtures in
  `metadata/fixtures/`, no network in tests.
  - **Built** 2026-10-04, ahead of Phase 7's exit at the owner's request,
    in `metadata/outside.rs` and `metadata/listenbrainz.rs`. **Terms**
    (read 2026-10-04): ListenBrainz's data is MetaBrainz's, "available for
    commercial use" with the same supporter tiers as MusicBrainz (§8.1's
    MetaBrainz decision covers both); its API asks each client for at most
    one call a second, which `http.rs` now applies to both ListenBrainz
    hosts. The sources table in `docs/design/phase-4-online-metadata.md`
    and the privacy table in `docs/release-decisions.md` have its row.
  - **Seeds**: the artists of the user's taste as X4 reads it (the last
    90 days' plays, else the last 200; a favourite track 2, a favourite
    album or artist 3), the heaviest five with a MusicBrainz id (the
    artist's match, else the tags'; never Various Artists). Only those
    ids go out, to the Labs API's `similar-artists` query (the algorithm
    ListenBrainz's own artist pages show), cached 30 days in the response
    cache like MusicBrainz's. Settings › Features lists the artists whose
    ids are sent. While online services are off, only cached lists are
    used.
  - **Candidates**: each seed's 30 best similar artists, scored by their
    place in its list (its score over the list's best) times the seed's
    share of the taste, summed across seeds; and MusicBrainz's relations
    stored with the seed's match (members, subgroups, collaborations; X4
    reads the same) at 0.8 of a list's best, with no request. Left out:
    anything the library has by MBID (tags or match) or by folded name,
    Various Artists, and dismissed ones (`outside_dismissed`, migration
    013, carried by F20's export). The two strongest reasons go to the UI
    ("like Radiohead on ListenBrainz", "Radiohead member"). A seed whose
    list fails still counts for its relations; the failure shows only
    when nothing was found.
  - **UI**: Home's "Beyond your library" after "You might like" (loaded
    apart from Home's other rows, as Labs can be slow); "More Like This"
    for an artist gains "Not in your library" (and shows while either
    switch is on). A suggestion's name opens a menu of links: MusicBrainz
    and ListenBrainz always, the homepage and Bandcamp page from a
    MusicBrainz lookup of that artist (`Artist::bandcamp`, new) while
    MusicBrainz may be contacted; each through `openWebLink`. "Not
    Interested" (the row's ×, or the menu) dismisses; Settings offers to
    suggest the dismissed again.
  - Commands `outside_for_you`, `outside_like_artist` (one seed: what X7's
    outside half will call), `outside_links`, `outside_dismiss`,
    `outside_status` and `outside_forget_dismissed`, behind the
    `outsideRecommendations` switch (off by default: it goes online).
    Fixtures: ListenBrainz's list for Radiohead, and Radiohead's
    MusicBrainz artist re-recorded with its Bandcamp link, a member and a
    subgroup (`record-fixtures.py` now selects from a response that is a
    list, path `""`); `live_similar_artists` checks the real service.
  - **Decided:** ListenBrainz's per-user recommendations
    (`/1/cf/recommendation/user/<name>/recording`) aren't used: they need
    the user's ListenBrainz name (known only with O8's token) and return
    recordings, each needing a MusicBrainz lookup at one a second to name
    its artist, while the seeds' similar artists already give artists
    the user doesn't own. Releases the user doesn't own are Phase 4's
    discography on the artist page (an owned artist's missing release
    groups), so X5 suggests artists only.
  - **Known limits:** Labs is ListenBrainz's experimental API: answers
    took 1 to 40 s on 2026-10-04, and one failed. A timeout marks the
    host unreachable (`http.rs`), so the other seeds fail at once and
    the cached lists and MusicBrainz's relations still show; the
    algorithm's name may change, which `live_similar_artists` would
    catch (§6).
  - Left to check in the app: suggestions that make sense on the owner's
    library, none of them owned (the exit), the links, and how long a
    first Home takes with ListenBrainz slow.

## X6 Record the output to a file

- [x] **X6 Record the output to a file.** Added 2026-10-04 at the
  owner's request. A Record button (in the now-playing bar, and in the
  Controls menu) writes what is playing, as heard, to a WAV file until
  it is pressed again: across track changes, gapless hand-offs and
  crossfades, with the effects (X2), equaliser (F15) and crossfeed
  (O11) applied.
  - **Core**: a tap after crossfeed and before volume, so the recording
    doesn't depend on the volume slider (decide: before or after
    volume). The audio thread only pushes samples into a lock-free FIFO;
    a writer thread drains it to the file (JUCE's
    `AudioFormatWriter::ThreadedWriter` over `WavAudioFormat`, which
    `juce_audio_formats` already has). 32-bit float at the engine's
    rate by default, 24-bit as an option; RF64 past 4 GB. While paused
    nothing is written, so a pause leaves no silence. A change of the
    engine's rate (O10's sample-rate matching) starts a new file with
    a numbered suffix rather than resampling. A full disk or a write
    error stops the recording, keeps what was written (the header
    finalised) and reports it through an event. A FIFO overrun (the
    writer falling behind) is counted and reported, never blocks the
    audio thread.
  - **C API**: `anomp_engine_record_start` (UTF-8 path, format),
    `anomp_engine_record_stop`, `anomp_engine_recording` (state, frames
    written, overruns), and a `RecordingStopped` event with its reason;
    main-thread only, like the rest of the engine. The signal path
    panel (O10) shows a recording in progress.
  - **Rust** (a new `recording` module): the recordings folder, picked once with
    the folder picker and kept as a read-write security-scoped bookmark
    (the sandbox already has `files.user-selected.read-write`); a file
    named by date and time (`ano-mp 2026-10-04 21.15.03.wav`), never by
    title. Optionally a cue sheet beside it listing the tracks and their
    offsets, so the recording reopens as tracks (Tracks are parts of
    files). Logs follow H9: ids and counts at info, the file name at
    debug.
  - **UI**: the Record button with an elapsed time and a red dot while
    recording; Settings › Recording for the folder, the sample format
    and the cue sheet. Strings in `en.json`, errors through `errorText`.
  - Switch: `recording`, off by default (it writes large files: about
    23 MB a minute at 48 kHz float). Streams (Phase 11) are never
    recorded: the button is disabled while one plays, under Bandcamp's
    terms.
  - Tests: offline renders in the style of `PlayerEngineTests.cpp` that
    record a gapless hand-off and a crossfade and compare the file with
    the rendered output, sample for sample; pause, a rate change, a
    write error and an overrun; and a benchmark (H18) that the tap costs
    the audio thread nothing measurable. A bundle self-test stage (H14)
    writes into a picked folder under the sandbox.
  - **Built** 2026-10-04, ahead of Phase 7's exit at the owner's request,
    who asked for a choice of output type ("wav, mp3, etc.") beyond the
    WAV above. **Decided:** six formats, all written by FFmpeg
    (`core/src/FFmpegEncoder.cpp`, the second file that includes its
    headers): WAV (16-bit, 24-bit or 32-bit float, the default; RF64 past
    4 GB), AIFF and FLAC (16 or 24), Apple Lossless and AAC in .m4a, and
    MP3; AAC and MP3 at 96–320 kbps. `build-ffmpeg.sh` now enables just
    those encoders and muxers (`FFmpegBuildTests` checks the exact list)
    and builds **LAME 4.0** (LGPL, July 2026; its decoder and programs
    left out) as a static library linked into libavcodec, so MP3 needs no
    new dylib; the pin is in `check-pins.py`/`bump-pin.py` (a
    `sourceforge` upstream) and in `THIRD_PARTY_NOTICES`. A lossy encoder
    that can't take the device's rate is resampled (MP3 to 48/44.1 kHz,
    AAC to 96/88.2 kHz); the rest record at the device's rate. The tap is
    **before the volume**, after crossfeed, so the slider doesn't change
    the recording; a pause fades it out and in with the same one-block
    ramp as what is heard, and while paused nothing is pushed.
  - **Core**: `Recorder` (`core/src/Recorder.*`) owns a 2^19-frame
    `AbstractFifo` (2.7 s at 192 kHz) the audio thread copies into, and a
    writer thread that wakes every 20 ms and drains it into the encoder;
    the audio thread never waits or allocates, and what doesn't fit is
    dropped and counted. A rate change (`prepareToPlay`) records a split
    at the frame it happened; the writer finishes the file there and opens
    "name 2.ext". A write error or ENOSPC stops the writer, which
    finalises the file; `PlayerEngine::dispatchEvents` reports it once
    (`onRecordingFailed`). Track marks are kept for the cue sheet: one at
    the start if a track is loaded, then at each `install`, gapless
    hand-off, and the *start* of a crossfade, accurate to the chunk the
    hand-off fell in. C API: `anomp_engine_record_start`/`_stop`,
    `anomp_engine_recording` (frames, seconds, overruns, files),
    `anomp_engine_recording_file`, `anomp_engine_recording_marks`,
    `anomp_record_format_available`/`_extension`,
    `ANOMP_EVENT_RECORDING_FAILED` (`result`: write failed or disk full),
    `anomp_signal_path.recording`, and `anomp_bookmark_create_writable`
    (a security-scoped bookmark without the read-only flag).
  - **Rust** (`recording.rs`): `AppSettings.recording` (format, bits,
    bitrate, cue sheet: on), the `recording` feature (off). The folder's
    bookmark is its own `settings` row (`recording.folder`), not part of
    `AppSettings`, so a data export never carries it; it is resolved (and
    refreshed when stale) for as long as a recording is written. Files are
    `ano-mp 2026-10-04 21.15.03.wav`, " (2)" if taken. The tracks that
    become current (`queue::publish`, once loaded) are matched to the
    core's marks, and on stop each file gets `<name>.cue` naming them
    (`library::cue` reads them back as tracks; a file after a rate change
    starts with the track playing). Commands `recording_status`,
    `_set_folder`, `_start`, `_stop`; events `recording` and
    `recording-failed`; coded errors `recordingNoFolder`,
    `recordingFolderUnavailable`, `recordingStartFailed`,
    `recordingFailed` (`.diskFull`), `recordingRunning`,
    `recordingNotRunning`. Turning the feature off, or quitting, stops and
    finishes the recording.
  - **UI**: Record in the now-playing bar (a red dot and the time recorded
    while it runs; the first press asks for a folder if there is none),
    Controls › Record (⌥⌘R, checked while recording), Settings ›
    Recording (folder, format, sample size or bitrate, cue sheet, the
    size a minute), and a Recording row in the signal path.
  - Tests: `RecorderTests.cpp` (a gapless hand-off and a crossfade
    recorded and compared with the rendered output sample for sample;
    pause, volume, a rate change and its numbered file and marks, write
    errors and a full disk, an overrun that never slows the audio thread,
    and each of the 14 format/size/rate cases decoded back), C API and
    writable-bookmark tests; Rust tests for names, settings, formats and
    cue sheets; a `recording` stage in the bundle self-test (a FLAC into a
    folder opened through a writable bookmark). Benchmarks:
    `core.record.tap_192k` (43,000× real time) and
    `core.play.flac_recording` (954× against 1,202× without) on
    2026-10-04.
  - **Known limits:** .m4a files (Apple Lossless, AAC) are readable only
    once finished: a crash or power loss while recording leaves them
    without their index, where WAV, AIFF, FLAC and MP3 keep what was
    written. AIFF has no 64-bit form, so it fails at 4 GB (about 4 hours
    at 48 kHz 24-bit). Cue marks are accurate to a chunk (about 10 ms), a
    crossfade cancelled by a seek keeps its mark, and a repeated track is
    named again by the track before it. Streams (Phase 11) don't exist
    yet; their rule above (Record disabled while one plays) lands with
    them. Left to check in the app: the exit's recording across a gapless
    album, a crossfade and a pause, played back elsewhere; the folder
    picked in a sandboxed bundle; a full disk; and Controls › Record.

## X7 Similar artists on artist pages

- [x] **X7 Similar artists on artist pages.** A "Similar artists" section
  on the artist page (`ArtistPage.svelte`), under the biography: first
  the library's artists most like this one, by X4's artist scoring
  (`similar::similar_artists`, already built and shown in X4's dialog:
  shared genres, credits and MusicBrainz relations, co-listening), each
  opening its own artist page; then, while X5 is on, artists the user
  doesn't own from X5's sources (ListenBrainz's similar artists for the
  artist's MBID), as links out through `webLink`. It needs no data of its
  own: the library half is a query on X4's scores, the outside half is
  X5's fetch for one artist, run on the metadata worker and cached as X5
  caches. Each row says why it is there ("shares 3 genres", "played
  together often", "similar on ListenBrainz"). Unreadable folders' artists
  are left out (`availability::unreadable`), as are artists with too
  little in common (a score threshold, tested on X4's fixture library).
  Its own switch (`similar_artists`, on by default, as it is local and
  cheap; its outside half follows X5's switch). Hidden while the artist
  has no matches, rather than showing an empty section. Before building,
  decide: how many to show (a row of about eight, with "More"), and
  whether band members and member-of relations show here or stay in the
  artist's details.
  - **Built** 2026-10-04, ahead of Phase 7's exit at the owner's request.
    **Decided:** a row of eight of each half, and "More" shows the rest
    in place (up to X4's and X5's twelve each) rather than opening X4's
    dialog, whose switches differ; "More Like This" stays in the page's
    header. Band members, subgroups and collaborations show here as
    reasons ("Radiohead member"), scored as X4 scores them (a link is
    worth 4, enough alone); the page lists no members of its own, as
    MusicBrainz's relations are read only for scoring. Reasons are X4's
    and X5's as worded (`similar.reason.*`, `outside.reason.*`) rather
    than new counts ("shares 3 genres"), so the dialog and the page
    agree.
  - **Library half**: `library_artist_page_similar`, X4's
    `similar::similar_artists` behind X7's switch, so the section works
    with X4's off; X4's threshold (`MIN_SCORE`, 3) and unreadable folders
    as X4 applies them (`artists_need_enough_in_common_and_a_readable_folder`
    on X4's fixture library: a genre alone is enough, a year alone or
    another genre isn't, and an artist only in an unreadable folder is
    left out). **Outside half**: X5's `outside_like_artist` (cached and
    on the metadata worker as X5's), called only while both switches
    are on and loaded apart from the library half, so a slow
    ListenBrainz never holds it back; its rows are X5's
    (`OutsideArtists`: links out through `openWebLink`, "Not
    Interested").
  - **UI**: `ArtistSimilar.svelte` under the About section of
    `ArtistPage.svelte`, hidden while both halves are empty; the library
    half reloads with the library, the outside half only for another
    artist or a switch. Switch: `similarArtists` (X7 in Settings ›
    Features), on by default.
  - Left to check in the app: similar artists that make sense on the
    owner's library, with and without X5 (the exit).

## X8 Effects workbench

- [x] **X8 Effects workbench.** Added 2026-10-04 at the owner's
  request: a page where the user picks one music file, plays it through
  the effects (X2), and records the output (X6), without touching the
  library or building a queue first. It reuses what
  X2, X6 and F5 built, so it adds no core or C API work unless offline
  rendering is chosen (below).
  - **The page**: a sidebar item (or a Sound menu item) opening a view
    with a file field ("Choose a file…", the open dialog filtered to
    the formats the registry plays, and drag and drop from the Finder),
    the file's title, length and format, a transport (play, pause, seek
    bar, A–B loop from practice mode), the effects' controls (the same
    component Settings › Effects uses, so presets and parameters stay
    one implementation), and a Record button with the format picker and
    the elapsed time. Strings in `en.json`, errors through `errorText`.
  - **Playing the file**: through the queue, never around it (the /dev
    page's raw-path commands stay debug-only, H2): the file plays as
    `queue_open_files` plays a file opened from the Finder (F5), so it
    opens off the main thread (H11) and the folder access rules hold
    (the open dialog grants the sandbox access to that one file).
    Decide: whether choosing a file replaces the queue (as F5 does) or
    the workbench keeps its own one-item queue and restores the
    listener's queue when it closes.
  - **Recording**: X6's recorder unchanged: the recordings folder, its
    formats and naming, the `recording` switch. The page shows the
    effects switch and the recording switch, and turns neither on by
    itself (Optional features: off by default, it changes what is
    heard and writes large files); a Record press with recording off
    explains where to turn it on. Decide: whether a workbench recording
    starts and stops with the file (record from the start, stop at its
    end or the loop's end, so the file is exactly one processed copy)
    or runs until Record is pressed again, as X6 does elsewhere.
  - **Decide first**: real-time only (what is heard is recorded, as
    X6) or also an offline render ("Save processed copy…": the engine
    renders the file through the chain faster than real time, with no
    device, as `PlayerEngineTests.cpp` does, a new C API call and a
    writer job on its own thread). Offline is exact and quick for long
    files but needs a new core path and a rule for the spectral
    freeze's Hold, which is a live gesture; real-time needs nothing new.
  - Tests: the frontend's state for the page (`node --test`), the
    command that opens the chosen file against the fake engine
    (`queue/model.rs`), and, if offline rendering is built, a render
    compared sample for sample with the same file played through the
    chain (`PlayerEngineTests.cpp`'s style) and a bundle self-test
    stage (H14) for a file granted by the open dialog.
  - Done when the user guide covers it (chapter 7, *Sound*, or 9,
    *Recording*), the developer guide's map and the class dictionary
    list its files and types, and `check-all.py` passes.
  - **Exit:** a user picks a FLAC outside the library, hears it with
    reverb and chorus, changes a parameter while it plays without a
    click, and records it to an MP3 that plays back as heard.
  - **Written** 2026-10-04, ahead of Phase 7's exit at the owner's request.
    **Decided** (owner): real-time only, no offline render, so no core or
    C API work; choosing a file plays it as F5 does (after the current
    item, the rest of the queue kept), not in a queue of its own; and a
    Record press records **one take**: the file from its start to its
    end, or once round the A–B loop, stopping by itself.
  - **Rust** (`workbench.rs`): `workbench_open` plays the file through
    `queue::open_file` (F5's resolution, split out of `open_files`; a
    library file plays as its library track) and returns a
    `WorkbenchFile` (its queue item, title, format, length, rate,
    channels, bitrate). `workbench_take` refuses while recording is off or
    running, has the model `ready_take` (playback stops after the item;
    paused and sought to the start or the loop's start; fails unless the
    item is current and open), starts X6's recording unchanged, and plays.
    A take ends on `TrackEnded` (stop-after stopped the engine, so the
    file holds exactly the file), near the loop's end (timed from the
    50 ms position reports by an `anomp-workbench-take` thread, allowing
    for the tempo; a jump back ends it at once), when another item becomes
    current, or when the recording stops any other way; then
    `Queue::end_take` puts back the item playback stopped after before,
    unless the user chose another. `Queue::play_now` now returns the
    first item's uid. Switch `effects_workbench`, on by default (local and
    cheap; the effects and recording keep their switches and stay off by
    default). Coded error `workbenchNotPlaying`.
  - **UI**: Effects workbench in the sidebar (`WorkbenchView.svelte`,
    state in `state/workbench.svelte.ts`, choices in `lib/workbench.ts`):
    Choose a File… (filtered to the extensions the app plays, now shared
    as `AUDIO_EXTENSIONS`), drops from the Finder while the page shows,
    the file's details, play/pause and the seek bar while it is current
    (else Play goes back to it or opens it again), practice mode's A–B
    loop (`LoopControls.svelte`, split out of the practice panel), Settings'
    Effects section with its switch, and Recording's switch, Record One
    Take with the time recorded, and Settings' Recording section. The
    settings sections' shared styles moved from `SettingsPage.svelte` to
    `settings/Options.svelte`, which wraps them in both places. With
    recording off, Record is disabled and says to turn it on.
  - Tests: the model's take against the fake engine (stop-after, pause
    and seek, the stop put back or left as the user chose), the take's
    timing (`workbench.rs`: the loop's end, the tempo, a jump back) and
    the file's description; `tests/workbench.test.mjs` for Play's and
    Record's choices, the take's span, and whose loop the page shows
    (`loopOwner`, `keepLoop`).
  - **Known limits:** a loop take stops within about a block of the
    loop's end (timed from position reports, not the audio thread), where
    a whole-file take is exact. A take may begin with what the reverb
    and echo still ring from before its seek (none heard in the app,
    2026-10-08). A seek during a take records
    what is heard, so the file is no longer one straight copy. No bundle
    self-test stage, as no offline render was built (the plan asked for
    one only with it).
  - **Checked in the app** 2026-10-08 (the owner, by ear), with
    `npm run tauri dev` and then a sandboxed, ad-hoc signed bundle
    (`docs/bundle-checks.md`, the container restored and compared after):
    the exit (a FLAC outside the library, chosen with the open dialog,
    heard with reverb and chorus, parameters and presets changed while it
    played without a click, and a take recorded to an MP3 that plays back
    in another app as heard, its length the file's, no gap or click at
    either end); a loop take (pauses near B, holds only A to B, ends
    cleanly, no reverb tail from before the seek heard); drops from the
    Finder (one file plays, several or a folder open the first and say
    so, a folder isn't added to the library); a take with other tracks
    queued and a stop-after set (the stop and the queue put back); and,
    sandboxed, a file granted by the open dialog and one dropped, each
    played and recorded. Nothing failed in Rust. **Fixed:** the page
    showed the previous file's A–B loop after another file was chosen or
    dropped, while the engine had dropped it with the track, so Record
    promised a loop take and recorded the whole file from its start.
    `LoopControls.svelte` read the loop only when mounted, and the page
    mounts it again while a dropped file is still opening (H11), when the
    engine still reports the last file's loop. It now reads the loop again
    for each item once it is open and drops an answer that arrives for an
    earlier one (`loopOwner`, `keepLoop` in `lib/workbench.ts`). Also: the
    workbench's log lines no longer repeat their module.
  - **Use the Playing Track** (added 2026-10-08 at the owner's request):
    a button beside Choose a File… takes the queue's current item, where
    it is, as the workbench's file, queueing nothing (`workbench_current`,
    reading the file through `queue::read_track_file` with its folder held
    open; disabled while nothing plays or the file is already the one
    playing, `canUseCurrent`). Play's "open it again" now goes by track
    id for every file (`workbench_reopen`, `queue::open_track`: a library
    track, a cue sheet's included, as itself; a file outside the library
    from where it is), so the page keeps no path. Coded error
    `workbenchNothingPlaying`.
    Checked in the app 2026-10-08 (`tauri dev`, the owner): a library
    track taken as it played (the queue unchanged), a loop take of it, and
    Play opening it again once it had left the queue.
    Also checked 2026-10-08, on a cue sheet's track: a scratch album
    outside the repo, one 30 s FLAC made from the chirp fixture with a
    `.cue` over it (three 10 s tracks at -12, -24 and -18 dBFS RMS, so a
    take shows by its level which track it holds), in `tauri dev` and
    then in a sandboxed, ad-hoc signed bundle (`docs/bundle-checks.md`,
    the container's `Data/Library` restored and identical to its copy
    after). With track 2 current, Use the Playing Track showed its title
    and its own 0:10, the queue unchanged; a whole-track take held only
    track 2 (10.005 s, -24.1 dBFS RMS throughout); and Play, once it had
    left the queue, opened track 2 again, not the file's first. In the
    bundle, a library track from the owner's music folder too: taken as it
    played, two loop takes, and opened again after leaving the queue.
    Nothing logged but MusicBrainz's 503s. Test:
    `a_cue_sheets_track_reads_and_plays_as_itself` (`queue/mod.rs`: a
    file's second part reads and plays by id as itself, its length and
    range its own).
  - **Seen once, not reproduced:** in `tauri dev`, the first time Play
    reopened the cue track (after a take, the track removed from the
    queue while current), it went in after track 3 and then left the queue
    within about 30 s with nothing done and nothing logged; the saved
    queue agreed. The same steps twice more kept it. No code path found
    that removes an item but `queue_remove`; if it comes back, note what
    the queue view showed and whether anything was selected in it.

## X9 Find a feature

- [x] **X9 Find a feature.** Added 2026-10-08 at the owner's request:
  the search box finds the app's features as well as its music, so a
  user who knows what they want ("crossfade", "equaliser", "record",
  "sleep timer") but not where it lives gets there in one step. Frontend
  only: no core, C API, Rust or database work.
  - **Decided** (owner, 2026-10-08): results show as a **Features group**
    in the search results, not a ⌘K palette (no new view, found by anyone
    who already searches; a palette can come later on the same index);
    **each setting** inside a section gets an entry, not only sections and
    switches, each checked by a test that its control's id is in its
    section's component; and **no switch**: it is local, cheap and changes
    nothing heard, and a `FeatureSettings` field is Rust, which would
    break "frontend only".
  - **Built** 2026-10-08 (frontend only, as planned). The index is
    `lib/find.ts`, pure so `npm test` runs it: 137 entries, one for each
    sidebar item, each Settings section and each setting in it (the eight
    effects included), each `PAGE_ITEMS` menu item, each `FeatureSettings`
    field, and the player bar's sleep timer (in the Controls menu, whose
    items Rust runs, so in none of `check-user-guide.py`'s lists; added
    since the entry names it). Each has its label's `en.json` key, the
    keys of where it lives, its `Place` and the features it `needs`;
    synonyms are `find.synonyms.<id>` (62 so far: "EQ", "dark mode",
    "scrobble", "tags"…). `find` folds case and accents (NFD, marks
    stripped), needs two letters, matches each query word to the start of
    a word, and ranks a label above a synonym above the place (where it
    lives with the label, so "playback crossfade" works), each from the
    text's first word before a later one, then in the index's order
    (sidebar, sections with their settings, menu, switches). `target`
    chooses where a result opens: its place, or, while every feature it
    needs is off, that feature's switch in Settings › Features; it never
    changes a setting (§4 #6). Results that would open in the same place
    show once (with Recording off, its section and its five settings are
    one "Off" result).
  - **Opening** (`lib/pageActions.ts`'s `openFound`): a view through
    `showView`, then the focus to the main area; a menu item through
    `runPageAction`, the menu's dispatch moved out of `+page.svelte` so
    both call it; a setting through `ui.showSettings(section, control)`,
    after which `SettingsPage.svelte` waits up to about a second for the
    section to draw the element (Sorting and Online sources load first;
    else it lands on the section's tab) and `lib/landOn.ts` scrolls to it,
    gives its row `.found` for two seconds (`--selected` and an `--accent`
    ring, fading out except under Reduce Motion, in `+layout.svelte`) and
    focuses its control: the checked choice of a group, or the row itself
    when the control is disabled. Playlists and the sleep timer go through
    counters in `ui` that `Sidebar.svelte` (opening the fold) and
    `NowPlayingBar.svelte` (opening the panel) watch. Controls got stable
    ids: `setting-<name>` (or the ids Appearance had), `feature-<field>`,
    `effect-<id>`, and the sections' `settings-tab-<id>`.
  - **UI**: a Features group above Artists in `SearchResults.svelte`,
    first five with More features, each row its label, where it lives
    ("Settings › Playback") and On or Off for a switch, or Off for
    something hidden (whose tooltip says it opens at the switch that turns
    it on). New `en.json` keys under `find.`: the group's text, where
    things live (Sidebar, File, Edit, View, Controls, Help, Player bar),
    and the menu items' labels that had no key (`find.menu.*`; Get Info,
    the playlist items, Mini player and Keyboard shortcuts reuse theirs).
    `search.hint` mentions the features.
  - **Tests** (`tests/find.test.mjs`, 16): folding and word starts, two
    letters, the three tiers and first-word ranking on a small index, the
    exit's queries ("cross" → Playback's Crossfade, "eq" → the Equaliser
    section, "record" and "theme" off → their switches, on → their
    sections), synonyms, on and off, each kind of place, hidden features
    opening at their switch with the settings object unchanged, no two
    results in one place, no /dev entry; and coverage read from the
    sources: every sidebar item (with the sidebar's own `on` as its
    `needs`), every `ALL_SECTIONS` section (the hidden ones as
    `SettingsPage.svelte` hides them), every `FeatureSettings` field with
    `check-user-guide.py`'s label (`SWITCH_LABELS`), every `PAGE_ITEMS` id
    with an entry and a case in `runPageAction`, every label, place and
    synonym key in `en.json`, and every control id in its section's
    component.
  - **Known limits:** nothing lists the settings inside a section, so a
    new setting without an entry fails no test (the recipe in
    `docs/developer-guide/11-recipes.md` says to add one). The library's
    own search isn't music-free at two letters, as the plan supposed: its
    word index matches word starts from one letter (only its
    match-anywhere trigrams wait for three, F12), so a two-letter query
    can show music under the features; unchanged. Not indexed: the
    Controls menu's other items (play, shuffle, repeat, Stop After, Record:
    their switches and the player bar cover them), the library views and
    playlists by name (the user's own), and the user guide's pages.
    Recording's Bitrate and Samples share one id, as only one shows for a
    format. A disabled control (the equaliser's, while it is off) leaves
    the focus on its row, so VoiceOver reads the row's text. Results are
    reached with Tab from the search box; Return doesn't open the first.
  - **Checked in the app** 2026-10-08 (`tauri dev`, the owner): "cross"
    first finds Playback's Crossfade and lands on it, highlighted and
    focused, with VoiceOver reading its label; "eq" first finds the
    Equaliser section, VoiceOver reading its tab; "record" with Recording
    off shows Off and opens at its switch without turning it on, and with
    it on opens Settings › Recording; "theme" opens Appearance's Themes
    with Themes on, and shows Off and opens at its switch with them off; a
    switch's On and Off ("lyrics"), a menu item ("get info"), "playlists"
    opening the sidebar's fold, "sleep" opening the sleep timer, and the
    highlight without its fade under Reduce Motion.
  - **Exit:** in the app, "cross", "eq", "record" and "theme" each put
    the right place first; opening one lands on its control with
    VoiceOver reading it; a feature that is off shows as off and opens
    at its switch.
