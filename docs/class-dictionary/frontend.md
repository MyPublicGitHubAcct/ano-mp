# The frontend's types and components

Everything named in `app/src/`, in alphabetical order: the types,
interfaces and classes each `.ts` file exports, each state module (named
by its file, with the store it exports), each Svelte component (by name;
a route by its path), and each generated file once. The generated files'
types are the Rust backend's, of the same names: see [the Rust
page](rust.md). The developer guide's [chapter
7](../developer-guide/07-frontend.md) explains how the frontend fits
together.

Back to the [index](README.md).

### `AboutOptions`

Svelte component · [`app/src/lib/components/settings/AboutOptions.svelte`](../../app/src/lib/components/settings/AboutOptions.svelte) · [D2: "Copy diagnostics"](../developer-guide/10-troubleshooting.md#copy-diagnostics)

Settings › About: the versions, the logs in the Finder, "Copy diagnostics"
(no paths or titles), the "Detailed logging" switch, the third-party
notices, Discogs' notice, and checking for newer releases.

### `AlbumCards`

Svelte component · [`app/src/lib/components/AlbumCards.svelte`](../../app/src/lib/components/AlbumCards.svelte) · [D2: Lists](../developer-guide/07-frontend.md#lists)

A row or grid of albums with their covers, for Home, History, "More in
this genre" and "More like this": click opens, the play button plays,
right-click for more. Takes `AlbumCard`s.

### `AlbumInfo`

Svelte component · [`app/src/lib/components/AlbumInfo.svelte`](../../app/src/lib/components/AlbumInfo.svelte) · [D2: Calling the backend](../developer-guide/07-frontend.md#calling-the-backend)

The header of the album being browsed: its cover (click to choose
another), the facts the details sources give with each source named, its
match status, "Find details…" and "Choose cover…", and its description,
credited. Reloads after a scan and on `metadata-changed`.

### `AlbumRef`

TS type · [`app/src/lib/state/ui.svelte.ts`](../../app/src/lib/state/ui.svelte.ts)

An album by id and title, as dialogs take it.

### `AlbumWorks`

Svelte component · [`app/src/lib/components/AlbumWorks.svelte`](../../app/src/lib/components/AlbumWorks.svelte)

An album's classical works (O6): each work's movements, composer and
conductor, and "Play work". Shown on albums whose tracks carry work tags,
folded until opened.

### `AppearanceOptions`

Svelte component · [`app/src/lib/components/settings/AppearanceOptions.svelte`](../../app/src/lib/components/settings/AppearanceOptions.svelte) · [D2: Themes and styling](../developer-guide/07-frontend.md#themes-and-styling)

Settings › Appearance (X1): pick, edit, save, export and import themes
with the whole app as the live preview; flags colour pairs under WCAG AA
(`contrastIssues`).

### `Art`

Svelte component · [`app/src/lib/components/Art.svelte`](../../app/src/lib/components/Art.svelte) · [D2: Lists](../developer-guide/07-frontend.md#lists)

Cover art for an album or an album-less track, loaded lazily from
`anomp-art` at the size asked for (`artUrl`), with a placeholder when
there is none.

### `ArtistPage`

Svelte component · [`app/src/lib/components/ArtistPage.svelte`](../../app/src/lib/components/ArtistPage.svelte) · [D2: Windows and routes](../developer-guide/07-frontend.md#windows-and-routes)

An artist's page: MusicBrainz facts, the Wikipedia biography (credited),
their albums by release type, albums they appear on, a link to their
releases the library lacks, and similar artists. Reloads on
`metadata-changed`.

### `ArtistRef`

TS type · [`app/src/lib/state/ui.svelte.ts`](../../app/src/lib/state/ui.svelte.ts)

An artist by id and name, as dialogs and pages take it.

### `ArtistSimilar`

Svelte component · [`app/src/lib/components/ArtistSimilar.svelte`](../../app/src/lib/components/ArtistSimilar.svelte)

The artist page's "Similar artists" (X7): the library's artists most like
this one (X4), then, while X5 is on, artists outside the library as links
out. The outside half loads apart so it never holds back the rest.

### `ArtistsView`

Svelte component · [`app/src/lib/components/ArtistsView.svelte`](../../app/src/lib/components/ArtistsView.svelte) · [D2: Lists](../developer-guide/07-frontend.md#lists)

Every artist in the library in its sort order, narrowed by a filter box;
opens an artist's page, and its menu plays, queues or hearts them.

### `ArtSize`

TS type · [`app/src/lib/api.ts`](../../app/src/lib/api.ts) · [D2: Calling the backend](../developer-guide/07-frontend.md#calling-the-backend)

The sizes `artUrl` asks for: a list thumbnail, a header thumbnail, or full
size (only where a picture is shown full size, H17).

### `Block`

TS type · [`app/src/lib/guide.ts`](../../app/src/lib/guide.ts) · [D2: Windows and routes](../developer-guide/07-frontend.md#windows-and-routes)

A block of a user guide page as `guide.ts` parses it: a heading,
paragraph, list or table, drawn by the Help window with ordinary elements,
never as HTML.

### `BrowsePane`

Svelte component · [`app/src/lib/components/BrowsePane.svelte`](../../app/src/lib/components/BrowsePane.svelte) · [D2: Lists](../developer-guide/07-frontend.md#lists)

One node of the library under the current sort rule: its groups and
tracks, fetched a page at a time into a `VirtualList` as they scroll into
view. Opens groups, plays tracks, and lets rows be selected together,
given a menu and dragged (F4).

### `BrowsePath`

TS type · [`app/src/lib/api.ts`](../../app/src/lib/api.ts)

Where the browser is: one `GroupKey` per level browsed, null for "Unknown
…" groups.

### `ChooseCoverDialog`

Svelte component · [`app/src/lib/components/ChooseCoverDialog.svelte`](../../app/src/lib/components/ChooseCoverDialog.svelte) · [D2: Calling the backend](../developer-guide/07-frontend.md#calling-the-backend)

"Choose cover": each album-art source's pictures for an album, to pick one
or go back to the first found. The archive's previews come through the
metadata worker, so the window never contacts the service.

### `Conditions`

TS type · [`app/src/lib/theme.ts`](../../app/src/lib/theme.ts) · [D2: Themes and styling](../developer-guide/07-frontend.md#themes-and-styling)

What a theme's palette depends on: the system's dark mode, more contrast,
and the current cover's colour for a theme that takes its accent from it.

### `ContextMenu`

Svelte component · [`app/src/lib/components/ContextMenu.svelte`](../../app/src/lib/components/ContextMenu.svelte)

The menu `ui.openMenu` opens (right-click, a ⋯ button or the keyboard),
with submenus, checks and separators, and full keyboard navigation (F18).

### `ContrastIssue`

TS type · [`app/src/lib/theme.ts`](../../app/src/lib/theme.ts) · [D2: Themes and styling](../developer-guide/07-frontend.md#themes-and-styling)

A colour pair of a palette under WCAG AA: the two tokens, their ratio and
the minimum.

### `Crumb`

TS type · [`app/src/lib/state/library.svelte.ts`](../../app/src/lib/state/library.svelte.ts)

A breadcrumb of the browser's path: its key and name.

### `DbRepairDialog`

Svelte component · [`app/src/lib/components/DbRepairDialog.svelte`](../../app/src/lib/components/DbRepairDialog.svelte) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Offered when the launch check finds the database damaged (H10): restore
the copy made before the last upgrade, or rebuild after saving what the
user made. Either restarts the app.

### `DevPage`

Svelte component (debug builds) · [`app/src/lib/components/dev/DevPage.svelte`](../../app/src/lib/components/dev/DevPage.svelte) · [D2: The developer page](../developer-guide/10-troubleshooting.md#the-developer-page)

The /dev page (H2): the core version and device, a test tone, and loading
files by path straight into the engine, with an event log. Only in debug
builds.

### `Dialog`

Svelte component · [`app/src/lib/components/Dialog.svelte`](../../app/src/lib/components/Dialog.svelte)

A modal dialog over the window (the native `<dialog>`, so focus stays
inside and Escape closes it): title bar, scrolling body, optional actions.

### `Dialog`

TS type · [`app/src/lib/state/ui.svelte.ts`](../../app/src/lib/state/ui.svelte.ts) · [D2: State stores](../developer-guide/07-frontend.md#state-stores)

The modal dialog showing, if any, and what it is about: find details,
choose cover, find artist, preferences, smart playlist, Get Info,
shortcuts, notices, "More like this"… `Dialogs` draws it.

### `Dialogs`

Svelte component · [`app/src/lib/components/Dialogs.svelte`](../../app/src/lib/components/Dialogs.svelte)

Draws the dialog `ui.dialog` asks for, if any.

### `DiscographyPage`

Svelte component · [`app/src/lib/components/DiscographyPage.svelte`](../../app/src/lib/components/DiscographyPage.svelte)

The releases MusicBrainz lists for an artist that the library lacks, by
release type, each linked to MusicBrainz. Fetched through the metadata
worker, cached for a week.

### `DisplayOptions`

Svelte component · [`app/src/lib/components/settings/DisplayOptions.svelte`](../../app/src/lib/components/settings/DisplayOptions.svelte)

Settings › Display: track list columns, the album summary's facts, and
whether descriptions and biographies show.

### `DragGhost`

Svelte component · [`app/src/lib/components/DragGhost.svelte`](../../app/src/lib/components/DragGhost.svelte)

What a drag of tracks carries, beside the pointer (F4).

### `DragPayload`

TS type · [`app/src/lib/state/drag.svelte.ts`](../../app/src/lib/state/drag.svelte.ts)

What a drag carries: tracks (found when dropped), queue items, or playlist
entries.

### `DropTarget`

TS type · [`app/src/lib/state/drag.svelte.ts`](../../app/src/lib/state/drag.svelte.ts)

A drop target registered under a `data-drop` id: what it accepts and what
a drop does.

### `EffectId`

TS type · [`app/src/lib/effects.ts`](../../app/src/lib/effects.ts) · [D2: On the Rust and UI side](../developer-guide/05-effects.md#on-the-rust-and-ui-side)

An effect's id: a key of the generated `EffectsSettings`.

### `EffectsOptions`

Svelte component · [`app/src/lib/components/settings/EffectsOptions.svelte`](../../app/src/lib/components/settings/EffectsOptions.svelte) · [D2: On the Rust and UI side](../developer-guide/05-effects.md#on-the-rust-and-ui-side)

Settings › Effects (X2): a preset, then each effect in chain order with
its switch, mix and parameters (ranges from the core); sliders preview as
they move and save when let go; the freeze's Hold.

### `EqualiserOptions`

Svelte component · [`app/src/lib/components/settings/EqualiserOptions.svelte`](../../app/src/lib/components/settings/EqualiserOptions.svelte)

Settings › Equaliser (F15): on or off, ten bands and a preamp from a
preset or by hand, and a headphones profile when it follows the output.

### `FavouritesView`

Svelte component · [`app/src/lib/components/FavouritesView.svelte`](../../app/src/lib/components/FavouritesView.svelte)

Everything hearted (F3), newest first: artists, albums and tracks,
playable and draggable like any list.

### `FeaturesOptions`

Svelte component · [`app/src/lib/components/settings/FeaturesOptions.svelte`](../../app/src/lib/components/settings/FeaturesOptions.svelte) · [D2: Settings and features](../developer-guide/06-rust-backend.md#settings-and-features)

Settings › Features: every optional feature's switch and its options (the
analysis's progress, crossfeed, the ListenBrainz token, what outside
suggestions send, the remote's pairing and phones).

### `FindArtistDialog`

Svelte component · [`app/src/lib/components/FindArtistDialog.svelte`](../../app/src/lib/components/FindArtistDialog.svelte) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

"Find artist": the MusicBrainz artists this could be, to pick one, say
none, or go back to automatic matching; a link or id is looked up
directly.

### `FindDetailsDialog`

Svelte component · [`app/src/lib/components/FindDetailsDialog.svelte`](../../app/src/lib/components/FindDetailsDialog.svelte) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

"Find details": each album-details source's releases for an album, best
first, to pick, reject all, or go back to automatic, per source; shows
Discogs' credit with its results.

### `FlashGuard`

TS class · [`app/src/lib/visualizer/safety.ts`](../../app/src/lib/visualizer/safety.ts) · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

Keeps the visualizer safe to look at (F18): passes at most three beats a
second (none in calm mode) and dims the picture while its brightness
flashes more than three times a second. Used by `Visualizer`.

### `Fold`

Svelte component · [`app/src/lib/components/Fold.svelte`](../../app/src/lib/components/Fold.svelte)

A section that folds away under its heading, closed until opened;
remembers the viewer's choice under a key.

### `Frame`

TS type · [`app/src/lib/visualizer/frame.ts`](../../app/src/lib/visualizer/frame.ts) · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

One analysis decoded from the binary frames `visualizer.rs` streams:
bands, chroma, levels, waveform, onset, beat, notes and balance.

### `GeneralOptions`

Svelte component · [`app/src/lib/components/settings/GeneralOptions.svelte`](../../app/src/lib/components/settings/GeneralOptions.svelte)

Settings › General: menu-bar controls, the mini player, notifications and
the keyboard shortcuts.

### `generated/commands.ts`

Generated module · [`app/src/lib/generated/commands.ts`](../../app/src/lib/generated/commands.ts) · [D2: Generated TypeScript](../developer-guide/06-rust-backend.md#generated-typescript)

A typed wrapper for every command `lib.rs` registers, generated by
`bindings.rs` from each command's signature. `api.ts` calls these, never
`invoke` with a string. Regenerate with `ANOMP_WRITE_BINDINGS=1 cargo test
bindings`; never edit.

### `generated/ipc.ts`

Generated module · [`app/src/lib/generated/ipc.ts`](../../app/src/lib/generated/ipc.ts) · [D2: Generated TypeScript](../developer-guide/06-rust-backend.md#generated-typescript)

Every payload and event type the backend sends or takes, generated by
ts-rs from the Rust types of the same names (see [the Rust page](rust.md):
those marked "sent to the UI"). Re-exported by `api.ts`.

### `generated/settings.ts`

Generated module · [`app/src/lib/generated/settings.ts`](../../app/src/lib/generated/settings.ts) · [D2: Generated TypeScript](../developer-guide/06-rust-backend.md#generated-typescript)

The settings' types (`AppSettings` and everything in it), generated by
ts-rs from `settings.rs` and the modules it includes. Re-exported by
`api.ts`.

### `Header`

Svelte component · [`app/src/lib/components/Header.svelte`](../../app/src/lib/components/Header.svelte)

The browser's breadcrumbs, the search box, and on narrow windows the
sidebar button.

### `HealthView`

Svelte component · [`app/src/lib/components/HealthView.svelte`](../../app/src/lib/components/HealthView.svelte)

The library health report (O4): undecodable, truncated and suspected
transcoded files, albums whose tags disagree, likely duplicates; each file
can be shown in Finder.

### `Heart`

Svelte component · [`app/src/lib/components/Heart.svelte`](../../app/src/lib/components/Heart.svelte)

A heart that toggles (F3), announced as a toggle button.

### `HistoryView`

Svelte component · [`app/src/lib/components/HistoryView.svelte`](../../app/src/lib/components/HistoryView.svelte)

The listening history (O8): the top tracks, albums or artists of a year or
month with "Play these", and everything played recently.

### `HomeView`

Svelte component · [`app/src/lib/components/HomeView.svelte`](../../app/src/lib/components/HomeView.svelte)

Home: ways into the library besides browsing, each while its feature is
on: outside artists, on this day, recently played, highlights, recently
added, never played, and albums like what the user plays.

### `Icon`

Svelte component · [`app/src/lib/components/Icon.svelte`](../../app/src/lib/components/Icon.svelte)

A named 24×24 icon, filled with the current colour.

### `Inline`

TS type · [`app/src/lib/guide.ts`](../../app/src/lib/guide.ts)

Inline user guide text as `guide.ts` parses it: text, bold, emphasis, code
or a link.

### `Key`

TS type · [`app/src/lib/visualizer/key.ts`](../../app/src/lib/visualizer/key.ts) · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

A musical key estimated from chroma (Krumhansl–Schmuckler): tonic, major
or minor, and how strongly it fits. The fifths, spiral and Tonnetz
visualizations use it.

### `LibraryFolders`

Svelte component · [`app/src/lib/components/settings/LibraryFolders.svelte`](../../app/src/lib/components/settings/LibraryFolders.svelte) · [D2: Adding a folder and its scan](../developer-guide/08-flows.md#adding-a-folder-and-its-scan)

Settings › Library folders: each folder with rescan, remove or "Locate…",
adding folders, keeping the library in step with the disk (F9), and
exporting and importing the user's data (F20).

### `LinkTarget`

TS type · [`app/src/lib/guide.ts`](../../app/src/lib/guide.ts)

Where a guide link goes: another page (and heading), the web, or nowhere
the Help window can follow.

### `ListEdit`

TS type · [`app/src/lib/queueEdits.ts`](../../app/src/lib/queueEdits.ts) · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

One change to the queue's list, as Rust's `Edit` sends it; `queueEdits.ts`
applies them by `listVersion` and asks for the whole state when it misses
one.

### `LyricsPanel`

Svelte component · [`app/src/lib/components/LyricsPanel.svelte`](../../app/src/lib/components/LyricsPanel.svelte)

The current track's lyrics (O13): synced lines light up and seek when
clicked; plain ones show as text.

### `MainView`

TS type · [`app/src/lib/state/ui.svelte.ts`](../../app/src/lib/state/ui.svelte.ts) · [D2: State stores](../developer-guide/07-frontend.md#state-stores)

Which view fills the main area: library, now playing, visualizer, artist,
settings, home, history and the rest.

### `MenuItem`

TS type · [`app/src/lib/state/ui.svelte.ts`](../../app/src/lib/state/ui.svelte.ts)

An item of a context menu: an action, a submenu or a separator.

### `MenuTrack`

TS type · [`app/src/lib/trackMenu.ts`](../../app/src/lib/trackMenu.ts)

The fields of a track the track menu needs (and its path, for "Show in
Finder").

### `MessageKey`

TS type · [`app/src/lib/i18n/index.ts`](../../app/src/lib/i18n/index.ts) · [D2: Text](../developer-guide/07-frontend.md#text)

A key of the message catalogue, typed from `en.json`, so a missing key
fails `npm run check`. `t` takes one.

### `MissingFolders`

Svelte component · [`app/src/lib/components/MissingFolders.svelte`](../../app/src/lib/components/MissingFolders.svelte) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Library folders that can't be read now (H22), each with its reason and
what can be done: locate, remove, remove the tracks not found, or keep.
With every folder missing, it takes the main view's place.

### `Modifiers`

TS type · [`app/src/lib/selection.ts`](../../app/src/lib/selection.ts)

The keys held with a click or arrow in a list: Shift, and ⌘ (Ctrl
elsewhere).

### `MoreInGenre`

Svelte component · [`app/src/lib/components/MoreInGenre.svelte`](../../app/src/lib/components/MoreInGenre.svelte)

"More in <genre>" (O18): five random albums sharing a genre with this one,
with a chip per genre and "Draw again".

### `NoticesDialog`

Svelte component · [`app/src/lib/components/NoticesDialog.svelte`](../../app/src/lib/components/NoticesDialog.svelte)

The third-party notices as the app bundles them.

### `NowPlaying`

Svelte component · [`app/src/lib/components/NowPlaying.svelte`](../../app/src/lib/components/NowPlaying.svelte)

The now-playing view: the current track's title, artist and album beside
its cover, as large as fits.

### `NowPlayingBar`

Svelte component · [`app/src/lib/components/NowPlayingBar.svelte`](../../app/src/lib/components/NowPlayingBar.svelte) · [D2: Windows and routes](../developer-guide/07-frontend.md#windows-and-routes)

The bar along the bottom: the current track and its heart, transport, seek
bar, volume, shuffle, repeat, sleep timer, Hold, Record and the queue
toggle. The mini player shows it alone.

### `OrderedChoices`

Svelte component · [`app/src/lib/components/settings/OrderedChoices.svelte`](../../app/src/lib/components/settings/OrderedChoices.svelte)

Some of a list of options, in an order, with buttons to move and remove
each and a menu to add the rest.

### `OutsideArtists`

Svelte component · [`app/src/lib/components/OutsideArtists.svelte`](../../app/src/lib/components/OutsideArtists.svelte)

Artists outside the library (X5), each saying why, with a menu of links
out (passed through `webLink`) and "Not Interested"; never anything that
plays.

### `Page`

TS type · [`app/src/lib/guide.ts`](../../app/src/lib/guide.ts) · [D2: Windows and routes](../developer-guide/07-frontend.md#windows-and-routes)

A parsed user guide page: its file name, title and blocks.

### `Palette`

TS type · [`app/src/lib/visualizer/types.ts`](../../app/src/lib/visualizer/types.ts) · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

The colours a visualization draws with (from the cover or the defaults)
and a dark shade for backgrounds.

### `Params`

TS type · [`app/src/lib/i18n/index.ts`](../../app/src/lib/i18n/index.ts)

A message's placeholder values, by name.

### `PlaybackOptions`

Svelte component · [`app/src/lib/components/settings/PlaybackOptions.svelte`](../../app/src/lib/components/settings/PlaybackOptions.svelte)

Settings › Playback: the output device and buffer size, what the device
does now, and ReplayGain. A new device opens before it is saved.

### `PlaylistView`

Svelte component · [`app/src/lib/components/PlaylistView.svelte`](../../app/src/lib/components/PlaylistView.svelte)

A playlist (F1): rename, play, shuffle, and its tracks a page at a time,
reordered by dragging; a smart playlist (F2) lists what matches its rules
now.

### `Popover`

Svelte component · [`app/src/lib/components/Popover.svelte`](../../app/src/lib/components/Popover.svelte)

A small panel above the playing bar, opened from a button there; a click
outside or Escape closes it.

### `PracticePanel`

Svelte component · [`app/src/lib/components/PracticePanel.svelte`](../../app/src/lib/components/PracticePanel.svelte)

Practice mode (O12): an A–B loop from the current position, the tempo
(50–150%) and the pitch (±12 semitones).

### `PrefsDialog`

Svelte component · [`app/src/lib/components/PrefsDialog.svelte`](../../app/src/lib/components/PrefsDialog.svelte)

Playback preferences (O7) for a track and its album: skip, never shuffle,
gain offset and trims, kept in the library.

### `PresetEffect`

TS type · [`app/src/lib/effects.ts`](../../app/src/lib/effects.ts)

What an effects preset sets for one effect: its mix and parameters,
switched on.

### `QueuePanel`

Svelte component · [`app/src/lib/components/QueuePanel.svelte`](../../app/src/lib/components/QueuePanel.svelte) · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

The queue in play order: play, remove, select and drag items (pointer
events, not HTML drag and drop), and tracks dropped from the library.
Marks stop-after and the current item.

### `RecordingOptions`

Svelte component · [`app/src/lib/components/settings/RecordingOptions.svelte`](../../app/src/lib/components/settings/RecordingOptions.svelte)

Settings › Recording (X6): the folder, the format with its bits or
bitrate, and cue sheets; a format this build can't write isn't offered.

### `Recurrence`

TS class · [`app/src/lib/visualizer/recurrence.ts`](../../app/src/lib/visualizer/recurrence.ts) · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

The recurrence plot's self-similarity matrix: the music summarised every
few seconds as a feature, each compared with every other, merging steps as
it fills. Used by the recurrence visualization.

### `Renderer`

TS type · [`app/src/lib/visualizer/types.ts`](../../app/src/lib/visualizer/types.ts) · [D2: A visualization](../developer-guide/11-recipes.md#a-visualization)

A visualization's drawing object: `draw` once per frame with a `Scene`,
and `dispose`.

### `Rgb`

TS type · [`app/src/lib/theme.ts`](../../app/src/lib/theme.ts), [`app/src/lib/visualizer/types.ts`](../../app/src/lib/visualizer/types.ts)

A colour as red, green and blue; one in `theme.ts` and one in the
visualizer's types.

### `routes/+layout.svelte`

Route · [`app/src/routes/+layout.svelte`](../../app/src/routes/+layout.svelte) · [D2: Windows and routes](../developer-guide/07-frontend.md#windows-and-routes)

Every window's root: global styles and the theme's tokens as custom
properties on `:root`, kept in step by the appearance store. `+layout.ts`
loads the settings before any page renders.

### `routes/+page.svelte`

Route · [`app/src/routes/+page.svelte`](../../app/src/routes/+page.svelte) · [D2: Windows and routes](../developer-guide/07-frontend.md#windows-and-routes)

The main window: sidebar, the main view, the queue and the now-playing
bar, adapting to narrow windows; the welcome view for a library without
folders. Handles the menu bar's `menu` events and files dropped on the
window.

### `routes/dev/+page.svelte`

Route (debug builds) · [`app/src/routes/dev/+page.svelte`](../../app/src/routes/dev/+page.svelte) · [D2: The developer page](../developer-guide/10-troubleshooting.md#the-developer-page)

The /dev page, which loads `DevPage` only when `__DEV_TOOLS__` is true.

### `routes/help/+page.svelte`

Route · [`app/src/routes/help/+page.svelte`](../../app/src/routes/help/+page.svelte) · [D2: Windows and routes](../developer-guide/07-frontend.md#windows-and-routes)

The Help window: the bundled user guide, parsed by `guide.ts` and drawn
with ordinary elements, with its contents down the side and Back. Its
capability allows reading the settings only.

### `routes/mini/+page.svelte`

Route · [`app/src/routes/mini/+page.svelte`](../../app/src/routes/mini/+page.svelte) · [D2: Windows and routes](../developer-guide/07-frontend.md#windows-and-routes)

The mini player window (F7): the playing bar alone, following the queue
and player through their events; its capability allows only the commands
the bar uses.

### `Scene`

TS type · [`app/src/lib/visualizer/types.ts`](../../app/src/lib/visualizer/types.ts) · [D2: A visualization](../developer-guide/11-recipes.md#a-visualization)

What a visualization draws from each frame: the canvas, its size, time,
the latest `Frame`, whether a beat came, calm mode, the track and cover,
the palette and settings.

### `SearchResults`

Svelte component · [`app/src/lib/components/SearchResults.svelte`](../../app/src/lib/components/SearchResults.svelte) · [D2: A search keystroke](../developer-guide/08-flows.md#a-search-keystroke)

Search results, fetched 150 ms after typing stops, in artists, albums and
tracks, with field filters (`artist:`, `year:`…) and multi-select.

### `SeekBar`

Svelte component · [`app/src/lib/components/SeekBar.svelte`](../../app/src/lib/components/SeekBar.svelte) · [D2: State stores](../developer-guide/07-frontend.md#state-stores)

The position slider and times: the only component reading the position
store, so its 50 ms updates re-render only this. Draws the waveform under
it when that feature is on (O2).

### `Selection`

TS type · [`app/src/lib/selection.ts`](../../app/src/lib/selection.ts) · [D2: Lists](../developer-guide/07-frontend.md#lists)

A list's multi-selection (F4): the rows selected, the anchor of a range,
and the focus. `selection.ts`'s pure functions update it as Finder does.

### `ServicesPanel`

Svelte component · [`app/src/lib/components/ServicesPanel.svelte`](../../app/src/lib/components/ServicesPanel.svelte) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

Settings › Services: the online switch, automatic lookups, each source
(with its key, reachability and notices) and the order sources are tried
per kind. Keys go to the keychain.

### `SettingsPage`

Svelte component · [`app/src/lib/components/SettingsPage.svelte`](../../app/src/lib/components/SettingsPage.svelte) · [D2: A setting changed in the UI](../developer-guide/08-flows.md#a-setting-changed-in-the-ui)

The settings view (⌘,): its sections down the side, each a component;
every change saves as it is made. Holds the shared styles the sections
use.

### `SettingsSection`

TS type · [`app/src/lib/state/ui.svelte.ts`](../../app/src/lib/state/ui.svelte.ts)

The settings view's sections: library, sorting, display, appearance,
playback, equaliser, effects, recording, visualizer, sources, features,
general and about.

### `ShortcutsDialog`

Svelte component · [`app/src/lib/components/ShortcutsDialog.svelte`](../../app/src/lib/components/ShortcutsDialog.svelte)

The keyboard shortcuts (F6), the menus' and the page's own.

### `Sidebar`

Svelte component · [`app/src/lib/components/Sidebar.svelte`](../../app/src/lib/components/Sidebar.svelte) · [D2: Windows and routes](../developer-guide/07-frontend.md#windows-and-routes)

The views, the sort rules as library views, the playlists, the library
folders with scan progress, the online sources' activity, and the
settings; tracks drop onto a playlist or the queue.

### `SignalPathPanel`

Svelte component · [`app/src/lib/components/SignalPathPanel.svelte`](../../app/src/lib/components/SignalPathPanel.svelte) · [D2: The signal path](../developer-guide/04-core.md#the-signal-path)

The signal path (O10): every step from the file to the speakers as the
engine has it now, from `SignalPathPayload`.

### `SimilarAlbums`

Svelte component · [`app/src/lib/components/SimilarAlbums.svelte`](../../app/src/lib/components/SimilarAlbums.svelte)

"More like this" on an album page (X4): the library's most similar albums,
each saying why.

### `SimilarDialog`

Svelte component · [`app/src/lib/components/SimilarDialog.svelte`](../../app/src/lib/components/SimilarDialog.svelte)

"More like this" (X4) for a track, album or artist, from the library
alone, each saying why; X5 adds outside artists for an artist.

### `SimilarSeed`

TS type · [`app/src/lib/state/ui.svelte.ts`](../../app/src/lib/state/ui.svelte.ts)

What "More like this" is about: a track, album or artist.

### `SleepTimerPanel`

Svelte component · [`app/src/lib/components/SleepTimerPanel.svelte`](../../app/src/lib/components/SleepTimerPanel.svelte)

The sleep timer (F13): stop in some minutes (fading out), at the end of
the track or album, or after a chosen queue item.

### `SmartPlaylistDialog`

Svelte component · [`app/src/lib/components/SmartPlaylistDialog.svelte`](../../app/src/lib/components/SmartPlaylistDialog.svelte)

A smart playlist's rules (F2): name, match all or any, conditions, order
and limit.

### `SortRules`

Svelte component · [`app/src/lib/components/settings/SortRules.svelte`](../../app/src/lib/components/settings/SortRules.svelte)

Settings › Sort rules: edit each rule's name, levels and orders, add and
remove rules, and the leading words sorting skips.

### `Stars`

Svelte component · [`app/src/lib/components/Stars.svelte`](../../app/src/lib/components/Stars.svelte)

A rating in whole stars (F3): a keyboard slider that a click sets, or a
quiet display.

### `state/appearance.svelte.ts`

State module (`appearance`) · [`app/src/lib/state/appearance.svelte.ts`](../../app/src/lib/state/appearance.svelte.ts) · [D2: Themes and styling](../developer-guide/07-frontend.md#themes-and-styling)

The theme on screen (X1): the settings' theme under the system's light,
dark and contrast, with the accent from the cover if asked; previews a
theme before it is saved. The root layout keeps `:root` in step.

### `state/collection.svelte.ts`

State module (`collection`) · [`app/src/lib/state/collection.svelte.ts`](../../app/src/lib/state/collection.svelte.ts) · [D2: State stores](../developer-guide/07-frontend.md#state-stores)

Playlists, and the actions on playlists, hearts and ratings every view
offers, with their errors as toasts; follows `collection-changed`.

### `state/drag.svelte.ts`

State module (`drag`) · [`app/src/lib/state/drag.svelte.ts`](../../app/src/lib/state/drag.svelte.ts) · [D2: State stores](../developer-guide/07-frontend.md#state-stores)

Dragging tracks within the window (F4) with pointer events: the drag in
progress and the registered drop targets.

### `state/effects.svelte.ts`

State module (`effects`) · [`app/src/lib/state/effects.svelte.ts`](../../app/src/lib/state/effects.svelte.ts) · [D2: On the Rust and UI side](../developer-guide/05-effects.md#on-the-rust-and-ui-side)

The spectral freeze's Hold, shared by the playing bar and Settings ›
Effects, asking the engine again when the track changes.

### `state/features.svelte.ts`

State module (`features`) · [`app/src/lib/state/features.svelte.ts`](../../app/src/lib/state/features.svelte.ts) · [D2: State stores](../developer-guide/07-frontend.md#state-stores)

Which optional features are on, the analysis's progress, and waveforms
cached per track, with counters views reload on.

### `state/library.svelte.ts`

State module (`library`) · [`app/src/lib/state/library.svelte.ts`](../../app/src/lib/state/library.svelte.ts) · [D2: State stores](../developer-guide/07-frontend.md#state-stores)

The library's folders and sort rules, where the browser is (rule and
path), the search query, and scanning.

### `state/metadata.svelte.ts`

State module (`metadataStatus`) · [`app/src/lib/state/metadata.svelte.ts`](../../app/src/lib/state/metadata.svelte.ts) · [D2: State stores](../developer-guide/07-frontend.md#state-stores)

What the metadata worker is doing, from `metadata-progress`, for the
sidebar and Services.

### `state/player.svelte.ts`

State module (`player`) · [`app/src/lib/state/player.svelte.ts`](../../app/src/lib/state/player.svelte.ts) · [D2: State stores](../developer-guide/07-frontend.md#state-stores)

The queue and transport as the backend last reported them
(`queue-changed`, `player-state`), applying list edits by `listVersion`.

### `state/position.svelte.ts`

State module (`playback`) · [`app/src/lib/state/position.svelte.ts`](../../app/src/lib/state/position.svelte.ts) · [D2: State stores](../developer-guide/07-frontend.md#state-stores)

The position and duration of the current track, the only state that
changes every 50 ms, kept apart so only the seek bar re-renders.

### `state/recording.svelte.ts`

State module (`recording`) · [`app/src/lib/state/recording.svelte.ts`](../../app/src/lib/state/recording.svelte.ts) · [D2: State stores](../developer-guide/07-frontend.md#state-stores)

Recording (X6): whether it runs and for how long, the folder, and starting
and stopping.

### `state/settings.svelte.ts`

State module (`appSettings`) · [`app/src/lib/state/settings.svelte.ts`](../../app/src/lib/state/settings.svelte.ts) · [D2: A setting changed in the UI](../developer-guide/08-flows.md#a-setting-changed-in-the-ui)

The app's settings: read before the first page renders
(`routes/+layout.ts`), kept in step with `settings-changed`, saved whole
after each change, with the defaults to reset to.

### `state/toasts.svelte.ts`

State module (`toasts`) · [`app/src/lib/state/toasts.svelte.ts`](../../app/src/lib/state/toasts.svelte.ts) · [D2: Text](../developer-guide/07-frontend.md#text)

Short messages at the bottom of the window: command errors (through
`errorText`) and skipped tracks.

### `state/ui.svelte.ts`

State module (`ui`) · [`app/src/lib/state/ui.svelte.ts`](../../app/src/lib/state/ui.svelte.ts) · [D2: State stores](../developer-guide/07-frontend.md#state-stores)

Layout and transient UI state: the main view, the settings section, the
dialog, the context menu, panels, and preferences remembered per viewer.

### `state/updates.svelte.ts`

State module (`updates`) · [`app/src/lib/state/updates.svelte.ts`](../../app/src/lib/state/updates.svelte.ts)

Newer releases: the last check, and a toast when an automatic check finds
one.

### `state/visualizer.svelte.ts`

State module (`visualizer`) · [`app/src/lib/state/visualizer.svelte.ts`](../../app/src/lib/state/visualizer.svelte.ts) · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

The visualizer: which visualization and cover wall, what shows under the
track's name, and full screen.

### `TempoTracker`

TS class · [`app/src/lib/visualizer/music.ts`](../../app/src/lib/visualizer/music.ts) · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

Follows the tempo from each frame's onset strength (an autocorrelation
between 50 and 200 bpm, leaning towards 120) and places the beats. Used by
the rhythm visualization.

### `Toast`

TS type · [`app/src/lib/state/toasts.svelte.ts`](../../app/src/lib/state/toasts.svelte.ts)

A toast: its id, text and kind (error or info).

### `Toasts`

Svelte component · [`app/src/lib/components/Toasts.svelte`](../../app/src/lib/components/Toasts.svelte)

The toasts in `toasts`, each with a dismiss button, announced politely.

### `TrackInfoDialog`

Svelte component · [`app/src/lib/components/TrackInfoDialog.svelte`](../../app/src/lib/components/TrackInfoDialog.svelte)

Get Info (F16), read only: title and rating, the format, where the file
is, MusicBrainz links, pictures and every tag field.

### `TrackText`

Svelte component · [`app/src/lib/components/TrackText.svelte`](../../app/src/lib/components/TrackText.svelte) · [D2: Lists](../developer-guide/07-frontend.md#lists)

A track row's text as the display settings ask: the chosen fields in
columns when wide, under the title when narrow.

### `Triad`

TS type · [`app/src/lib/visualizer/music.ts`](../../app/src/lib/visualizer/music.ts)

A major or minor triad found in the chroma: root, minor and strength. The
Tonnetz visualization uses it.

### `Typed`

TS type · [`app/src/lib/releases.ts`](../../app/src/lib/releases.ts)

A release group's primary and secondary types, by which the artist and
discography pages group releases.

### `VirtualList`

Svelte component · [`app/src/lib/components/VirtualList.svelte`](../../app/src/lib/components/VirtualList.svelte) · [D2: Lists](../developer-guide/07-frontend.md#lists)

A list of fixed-height rows that renders only those in view and reports
the rows it needs, with multi-selection and keyboard navigation (F4).
Every long list uses it.

### `Visualization`

TS type · [`app/src/lib/visualizer/types.ts`](../../app/src/lib/visualizer/types.ts) · [D2: A visualization](../developer-guide/11-recipes.md#a-visualization)

A visualization in the picker: its id, name, description, and how to
create its `Renderer`.

### `Visualizer`

Svelte component · [`app/src/lib/components/Visualizer.svelte`](../../app/src/lib/components/Visualizer.svelte) · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

The canvas: subscribes to the analysis while mounted, and draws the chosen
visualization once per animation frame from the latest `Frame`, with the
cover's colours, through a `FlashGuard`.

### `VisualizerOptions`

Svelte component · [`app/src/lib/components/settings/VisualizerOptions.svelte`](../../app/src/lib/components/settings/VisualizerOptions.svelte)

Settings › Visualizer: which visualization, the cover wall, the frame
rate, sensitivity, colours from the cover and cycling.

### `VisualizerView`

Svelte component · [`app/src/lib/components/VisualizerView.svelte`](../../app/src/lib/components/VisualizerView.svelte) · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

The visualizer view: a visualization filling the main area or the screen,
with the picker and controls that fade, the track along the bottom, and
the flashing-light note first (F18).

### `WelcomeView`

Svelte component · [`app/src/lib/components/WelcomeView.svelte`](../../app/src/lib/components/WelcomeView.svelte)

What a new library shows (F8): how to add music, what the online sources
that are on send, and the first scan's progress.
