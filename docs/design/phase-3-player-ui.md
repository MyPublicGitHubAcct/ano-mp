# Phase 3 — Frontend: core player UI

Moved from `PLAN.md` (H20) on 2026-10-03, unchanged but for this
heading. Section numbers (§) refer to `PLAN.md`, whose Phase 3
keeps the phase's status, decisions and open steps.

- [x] Play queue in Rust (PLAN.md Phase 1: the host owns the queue).

  Done 2026-09-26 (`app/src-tauri/src/queue/`, `core/src/PlayerEngine.*`):
  - **Model and host are separate.** `queue/model.rs` is the logic, driving
    the engine through a small `Player` trait, so its tests run against a
    fake with no audio device. `queue/mod.rs` hosts it in a main-thread
    thread-local next to the engine: every operation runs on the main
    thread (`run`, via `audio::on_main`), so the queue never waits for the
    engine from another thread while holding a lock. Commands that need the
    database first (track ids → titles, a browse node → its tracks) do that
    on a blocking thread, then hop over. `TrackEnded` is handled inside
    the engine's event callback, which the C API allows: the engine has
    finished with what it reports before it calls back. (This note first
    said it was deferred to the next turn of the main loop; it never was,
    since `run_on_main_thread` runs at once on the main thread. Corrected
    2026-09-26, when `anomp.h` came to state the rule and a Catch2 test to
    cover it.)
  - **State:** the items (a uid per item, so a track can be queued twice and
    survives reorders) in play order, the current index, repeat
    (off/all/one), and with shuffle on, the order before shuffling.
    Shuffle reorders only the items after the current one (Fisher–Yates on
    a seeded xorshift); turning it off restores the order before, keeping
    the current item; tracks added meanwhile keep their place in both
    orders. The order stays put while playing and across launches. With
    repeat all, each pass gets a new order: when the last item starts, it
    moves to the top and the rest are shuffled after it (the track that
    played just before it doesn't come straight back), and the new first
    item is armed as its gapless next.
  - **Gapless across the queue:** the queue records which item it armed as
    the engine's next track. On `TrackEnded` with `advanced` it moves to
    that item and arms the one after. After *every* change (reorder,
    remove, add, shuffle, repeat, jump) it recomputes what should follow
    the current item and calls `set_next` only if that differs, so edits
    mid-track keep the hand-off gapless. Repeat one arms the current track
    again; repeat all wraps.
  - **Core change: `anomp_engine_advance_count`.** The engine hands off on
    the audio thread but reports it up to 50 ms later, so a queue edit in
    that window would re-arm against the track that just finished, and a
    `load` (Next pressed) left a stale `advanced` event behind that moved
    the queue one item too far. The count is the number of hand-offs so
    far, updated as they happen; every queue operation first applies any it
    hasn't seen (`reconcile`), which makes the event a harmless duplicate.
    `load` now also drops an end-of-track event not yet reported, which
    belonged to the track it replaced. Catch2: `PlayerEngineTests` (counted
    before reported, a load keeps a pending hand-off, a load drops a
    pending end) and `CApiTests`.
  - **Commands:** `queue_state`, `queue_play` (ids from an index),
    `queue_play_node` (a browse node's tracks from a track), `queue_add`
    and `queue_add_node` (after the current item or at the end),
    `queue_remove`, `queue_move`, `queue_clear`, `queue_jump`, `queue_next`,
    `queue_previous`, `queue_toggle`, `queue_seek`, `queue_set_shuffle`,
    `queue_set_repeat`. Next and previous keep the play state (paused stays
    paused). Previous restarts the track after 3 s, else goes back (at the
    first item, restarts it). Removing the current item starts the one
    after it (or, if it was last, selects the one before without playing).
    `queue::next`/`previous`/`toggle`/`seek` are plain main-thread
    functions, ready for the media controls item below.
  - **Node tracks:** `browse::node_track_ids` lists every track under a
    node in exactly the order browsing shows them (a test walks `browse`
    depth first and compares), so "play this artist" plays album by album.
    A folder node lists its own files, or with `recursive` its whole tree
    with subfolders first (a stable sort in Rust over SQL's track order).
    Under a genre level a track is listed once, ordered by its whole genre
    tag. A node may name a stored rule or carry a whole rule (`RuleSpec`),
    so search can play an album whatever rules exist.
  - **Unreadable tracks** (missing file, unavailable folder, a file the
    decoder refuses) are skipped when starting or arming, marked
    unavailable, and reported in the next `queue-changed`; they stay in the
    queue, and jumping to one tries it again. If nothing opens, nothing
    plays. Every open goes through `open_folder_of`, held until the file is
    open.
  - **Event:** `queue-changed` carries a `QueueState` (current index and
    item, shuffle, repeat, next/previous availability, unavailable items,
    skipped tracks). The whole list is included only when it changed, so
    moving to the next track doesn't resend it.
  - **Persistence:** `player.queue` in `settings` (no migration): track
    ids, the pre-shuffle order, the current index, the position in the
    track, repeat and the volume. Saved after every change, on pause and at
    exit. At launch the queue is restored *not loaded*: nothing opens and
    nothing plays until asked; the first play opens the current track at
    the saved position (seeking before that moves where it will start).
    Tracks no longer in the library are dropped.
  - The dev page's `player_load`/`player_set_next` detach the queue until
    it next starts a track itself.
  - Speed (release, 50,000 synthetic tracks): the ids of every track under
    the album-artist rule's top node, in order, 190 ms; a library folder's
    whole tree 113 ms; one artist's tracks 0.1 ms; their queue metadata
    60 ms; the list as JSON 6.6 MB, 8 ms to serialize.
  - Known limits: queued items keep the tags they had when queued until a
    scan refreshes them. The saved queue is one JSON value rewritten on
    every change (about 350 KB for 50,000 tracks). A 50,000-track queue
    sends its whole list (6.6 MB) whenever the list changes. With shuffle
    and repeat all, drawing a new pass drops the previous pass's order, so
    previous from the top of a pass goes to the end of the new one rather
    than to the track that played before. Next at the end of the queue with
    repeat off does nothing.
  - Tests (`cargo test`, `queue::model`): advancing with the next track
    armed, starting mid-list, repeat modes, shuffle order stability (and
    unshuffling, and after a save and restore), a new order each pass with
    repeat all, additions while shuffled,
    previous restarting, paused next, edits while playing re-arming, a
    hand-off not yet reported, a late edit after the last track,
    unreadable tracks, restoring paused at the saved position, detaching.
- [x] Search.

  Done 2026-09-26 (`library/search.rs`, `migrations/002_search.sql`):
  - **FTS5, not a LIKE scan.** rusqlite's bundled SQLite is built with
    `SQLITE_ENABLE_FTS5`. Three contentless tables (`contentless_delete=1`,
    rowid = the indexed row's id): `artists_search(name)`,
    `albums_search(title, artist)` and `tracks_search(title, artist, album,
    album_artist)`. The `unicode61 remove_diacritics 2` tokenizer folds
    case and accents itself, so no `anomp_*` function is involved (they
    must stay out of the schema). Triggers keep the indexes in step and
    fire only when an indexed value changes; migration 002 indexes existing
    rows. A track without a title is indexed by its file name.
  - **Query:** each whitespace-separated word becomes a quoted prefix
    phrase (`"ac/dc"*`), so FTS5 syntax typed in is plain text and every
    word must match the start of a word in some field. The expression is a
    bound parameter. Results are ranked by bm25, then by sort key.
    `library_search(query, kinds?, offset, limit)` returns artists (with
    their track counts), albums (with album artist and year) and tracks,
    each a page plus a total.
  - Speed (release, 50,000 synthetic tracks with an 8,000-word vocabulary,
    count + 50 of each kind): a word in 3% of tracks ("love") 4.2 ms, a rarer
    word 2 ms, two words 0.5 ms, no match 0.1 ms; the worst case, a one- or
    two-letter prefix matching 28% of tracks, 31 ms. For comparison a LIKE
    scan over pre-folded copies of the same fields takes 5–10 ms just to
    count, before sorting, and would need a column filled from Rust (a
    migration can't call the folding function). Cost to scanning: 50,000
    track inserts take 2.5 s with the triggers instead of 1.0 s; rescans
    touch only changed files.
  - Known limits: words match from their start ("tles" doesn't find
    "Beatles"). unicode61 folds without compatibility decomposition, so
    ligatures like "ﬁ" differ from browse's folding; like browse, "ø", "ł"
    and "ß" aren't folded. unicode61 doesn't segment CJK text, so a run of
    CJK characters is one word, matched only from its start.
- [x] Cover art.

  Done 2026-09-26 (`library/art.rs`):
  - Served to the webview on the `anomp-art` URI scheme
    (`anomp-art://localhost/album-<id>`, or `/track-<id>` for tracks
    without an album), so an `<img loading="lazy">` shows it with no IPC
    round trip. Requests are answered on a blocking thread.
  - `art::lookup` is the only way in; its one source is the embedded
    picture (the album's first three tracks, in disc/track order, are
    tried), read through `open_folder_of`. Phase 4 adds the Cover Art
    Archive as a second source there.
  - An in-memory cache (64 MB, least recently used out, "no art" remembered
    too) means scrolling an album list doesn't re-read files. It is cleared
    after each scan, and the UI adds a scan counter to the URL so the
    webview's cache (`max-age` one day) doesn't keep old art.
  - Known limits: images are served at full size (no thumbnails), and the
    cache is memory only, so art is re-read after a relaunch.
- [x] Library browser (artists / albums / tracks / folders), search, queue
  view, now-playing bar with transport and seek.
- [x] Desktop window layout. Build it responsive (no fixed widths) so the
  iPhone/iPad layouts in Phase 8 are adjustments, not rewrites.

  Done 2026-09-26 (`app/src/lib/`, `app/src/routes/`):
  - **Plain CSS**, no UI framework: colours are custom properties on
    `:root`, redefined under `prefers-color-scheme: dark`; icons are inline
    SVG paths.
  - **Layout:** a CSS grid of sidebar | main | queue, with the now-playing
    bar along the bottom. Below 900 px the queue becomes an overlay; below
    640 px the sidebar becomes a drawer (☰), the bar stacks into two rows
    and the volume slider hides. Widths are `minmax`/`clamp` ranges, and
    track rows use a container query. The window can shrink to 360 × 480.
  - **Components:** `Sidebar` (sort rules as views; folders with add,
    rescan, remove and scan progress), `Header` (breadcrumbs, search box),
    `BrowsePane`, `SearchResults`, `QueuePanel`, `NowPlayingBar` with
    `SeekBar`, `ContextMenu`, `Toasts`, `Art`, and a `VirtualList` that
    renders only the rows in view and asks for pages as they come into
    view. Payload types and command wrappers are in `lib/api.ts`.
  - **State:** Svelte 5 runes in `lib/state/`. The position (every 50 ms) is
    a store of its own that only `SeekBar` reads, so it never re-renders a
    list. The queue mirror applies `queue-changed` and keeps the list
    unless the event carries a new one.
  - **Browsing:** pages of 200 are fetched as they scroll into view (sparse,
    so jumping to the end of a 50,000-track node fetches only that page).
    Click or Enter opens a group; double-click or Enter on a track plays
    the node's tracks from it; ▶ plays a group; right-click or ⋯ offers
    play next and add to queue. Album rows show art, artist and year.
    `library_browse` now runs on a blocking thread, off the main thread.
  - **Album order:** a rule now has `albumOrder`, `title` (the default, and
    what rules saved before it get) or `year` (each album's earliest year,
    oldest first, undated last, ties by title). Wherever a node lists
    albums, a Sort menu in its header switches it and saves it with the
    rule; playing a node (`node_track_ids`) follows the same order.
  - **Queue view:** the sidebar's Queue item (or the panel's expand button)
    shows the queue in the main area, where the side panel then hides;
    choosing a library view goes back.
  - **Search** runs 150 ms after typing stops and shows artists, albums
    and tracks with "More" for each. An artist or album opens in the
    browser under the first rule that starts with album artist (→ album)
    or artist, else plays; a track plays its album from that track.
  - **Queue view:** the current item highlighted and kept in view; click
    to jump, ✕ or Delete to remove, drag the handle to move, Clear. Drags
    use pointer events rather than HTML drag and drop, so they work on
    touch screens and aren't intercepted by the window's file drop.
  - **Keyboard:** space plays/pauses, ←/→ seek 5 s, ⌘←/→ previous/next,
    ⌘F focuses search (Escape clears it); lists take ↑/↓, Page Up/Down,
    Home/End and Enter.
  - The Phase 0–2 dev panels (test tone, typed paths, event log) moved to
    `/dev`; folder management and scan progress moved into the sidebar.
    Removing a folder asks first (the dialog plugin's `ask`; WKWebView
    shows no `confirm()`).
  - Known limits: no multi-select; space on a focused button plays/pauses
    rather than pressing it; the context menu has no arrow-key navigation.
- [x] **Check by hand** in `npm run tauri dev` and in the sandboxed bundle
  (`npm run tauri build -- --bundles app`): browse each built-in rule,
  search, play an album and hear gapless transitions across the queue,
  shuffle/repeat, reorder the queue while playing, quit and relaunch with
  the queue restored, and resize the window down to phone width.
  Done 2026-09-26. The checks led to three changes, recorded above: a new
  shuffle order each pass with repeat all, the queue view in the main area,
  and the album order. One relaunch came back with an empty queue; the
  saved state was correct and restores in tests from a copy of that
  database, so the cause wasn't found, and it was set aside.
- [x] OS media integration: macOS Now Playing info and remote commands
  (`MPNowPlayingInfoCenter`, `MPRemoteCommandCenter`) implemented in Obj-C++
  in the core. This covers media keys and Control Center; the same APIs serve
  the iOS lock screen in Phase 8. The remote commands call
  `queue::next`/`previous`/`toggle`/`seek` on the main thread; the Now
  Playing info comes from the `QueueState`.

  Done 2026-09-26 (`core/src/MediaControls*`, `app/src-tauri/src/media.rs`):
  - **Core:** `MediaControls` is platform-neutral. It keeps what is
    published (title, artist, album, playing or paused, elapsed time and
    duration, artwork bytes, whether next and previous are enabled), checks
    the values (non-finite ones refused, the position clamped to the
    track), and gates the commands that come back: next and previous are
    dropped while disabled, and a seek must be finite and is clamped. The
    platform part is a `Backend` with one implementation per OS, like
    `FolderAccess`. `MediaControls_apple.mm` (ARC) is the only code that
    touches MediaPlayer, AppKit or UIKit. It rebuilds the `nowPlayingInfo`
    dictionary on each change, with rate 1 or 0 so the system moves the
    progress bar on by itself. It sets `playbackState`, which macOS uses to
    pick the app that gets the media keys; a cleared queue sets it to
    stopped. Artwork is decoded into an `NSImage` (a `UIImage` on iOS),
    and bytes that don't decode are refused. It registers play, pause,
    toggle, next, previous and change-playback-position. The handlers hold
    their target weakly, so a late one does nothing after destroy; one
    that arrives off the main thread is re-sent to the main queue. The base
    `Backend` does nothing and is the fallback (`MediaControls_none.cpp`
    until Phases 9–10). `build.rs` and the core's CMake link MediaPlayer.
  - **C API:** `anomp_media_controls_create(callback, user_data)`,
    `_destroy`, `_set_track`, `_set_playback` (an `ANOMP_STATE_*` value,
    elapsed, duration), `_set_artwork` (encoded bytes; empty clears),
    `_set_navigation`, `_clear`, `_supported`, and `_perform`, which
    delivers a command as if the OS had sent it (for tests). Main thread
    only, null handles safe. **Commands use a callback of their own, not
    new engine event types:** engine events are polled every 50 ms, but
    commands arrive the moment the OS sends them. Commands also carry
    their own payload (a seek position), and the controls don't depend on
    the audio engine: separate lifetime, and they are tested without it.
  - **Rust:** `anomp::MediaControls` wraps the C API. `media.rs` hosts it
    in a main-thread thread-local. `NowPlaying` decides what to publish,
    driven through a `Publisher` trait so `cargo test` runs it against a
    fake. It compares what the queue and player are doing with what it
    last published, and sends only the differences:
    - After every queue change (`queue::publish`) it gets the `QueueState`
      plus the engine's state, position and duration. Not loaded, or no
      current item: clear. A new current item (uid or tags differ): track
      and playback. Changed `has_next`/`has_previous`: navigation.
    - The engine's `StateChanged` republishes playback. `Position` (every
      50 ms while playing) is compared, not forwarded. Playback is
      republished when the duration changes (the exact one arrives on
      load), when paused and the position moved, or when playing and the
      position is more than 0.25 s from where the system has moved it on
      to. That is how seeks from anywhere (UI, keyboard, scrubbing in
      Control Center, the dev page) reach the progress bar without the
      position being pushed every 50 ms.
    - Until the engine has a duration, the tags' one is shown.
  - **Nothing is published until the queue has a track loaded,** so a
    launch (which restores the queue unloaded) doesn't take the media keys
    from another player. The dev page's direct loads detach the queue,
    which clears Now Playing.
  - **Commands** go straight to `queue::play`, `pause` (new, like `toggle`
    it saves the position), `toggle`, `next`, `previous` and `seek`,
    inside the callback. The design first deferred them with
    `run_on_main_thread`, but on the main thread Tauri runs that closure
    at once, so it would not defer anything. Calling directly is safe: the
    core calls the handler only from the OS's handler, never inside a call
    into the core, so no queue, engine or media borrow is held then. The
    Rust wrapper keeps the handler behind a raw pointer, so running it
    never overlaps a `&mut` borrow of the controls.
  - **Artwork:** the key is the current item's album (`ArtKey::Album`), or
    the track without one, the same as the UI uses. When it changes, the
    old art is cleared and `art::lookup` runs on a blocking thread (the
    library connection is used only there). The result comes back through
    `run_on_main_thread` and is shown only if that key is still current.
    Tracks from the same album keep the art, with no lookup and no flicker.
  - Tests: Catch2 `MediaControlsTests.cpp` (a recording backend: what is
    published and what changed, value checks, artwork refusal, command
    gating and clamping, the fallback; the C API: null handles, argument
    checks, commands reaching the callback through `_perform`, and PNG
    artwork decoding). No test publishes a track, which would show on the
    machine's real Now Playing. `cargo test`: the command and state
    mapping, and `NowPlaying` (nothing before a track is loaded, clearing,
    art kept per album, late art ignored, when positions are republished,
    the tags' duration, a rescan's new tags).
  - Known limits: a seek of less than 0.25 s while playing isn't
    republished. For up to 50 ms after a gapless hand-off, the progress bar
    can show the new track's position under the old title, until the queue
    moves on. Artwork is sent at full size. Nothing is shown after a
    relaunch until playback starts, so the media keys can't resume the
    restored queue until then. The OS's command status is not the
    queue's: a command the queue can't act on (play with an empty queue)
    still reports success.
  - Checked by hand 2026-09-26 in `tauri dev` (unbundled, which Now
    Playing accepts) and in the sandboxed bundle: the media keys play,
    pause, skip and go back; Control Center and the menu-bar widget show
    title, artist, album, artwork and a progress bar that follows playback
    and seeks; scrubbing there seeks the app; next and previous gray out
    at the ends of the queue with repeat off; clearing the queue clears
    the info.
