# 11. Recipes

The full list of places to touch for common changes, in the order that
keeps every check green. Each follows `CLAUDE.md`'s rules for its area;
where a step looks arbitrary, the rule there says why. After any of
them, run the formatters for the languages you touched and
`scripts/check-all.py` (at least `--quick`) before committing.

Two steps belong to almost every recipe, so they are not repeated each
time:

- **The user guide.** Anything the user can see (a view, a setting, a
  feature, a menu item, an error message) isn't done until
  `docs/user-guide/` covers it, in the same commit. `check-user-guide.py`
  catches a missing sidebar item, Settings section, feature switch, menu
  item or error; the settings reference is regenerated (below).
- **This guide.** A new source file gets its line in the
  [repository map](03-repository-map.md) (`check-developer-guide.py`
  fails without it), and a change of responsibility updates the chapter
  that explains it.

## A C API function

1. **Declare it** in `core/include/anomp/anomp.h`, in its section, in
   plain C, with a comment saying what it returns, who frees what, which
   thread may call it and how it fails ([chapter 4](04-core.md#the-c-apis-conventions)).
2. **Implement it** in `core/src/anomp_c_api.cpp`: check its arguments
   (a null handle does nothing), call the C++ class, catch every
   exception, copy strings out, translate `bool` to `int`.
3. **Test it** in `core/tests/CApiTests.cpp`, including a null handle and
   bad arguments.
4. **Bind it** in `app/src-tauri/src/anomp.rs`: the `extern "C"`
   declaration with the same parameters (`check-c-api.py` compares the
   counts), and a safe wrapper with a `// SAFETY:` comment on each
   `unsafe` block. A function Rust has no use for goes in
   `check-c-api.py`'s `NOT_BOUND`, with the reason.
5. **Test the wrapper** in `anomp.rs`'s tests; `cargo test` runs them
   against the real core.

A new C++ source file behind it goes in `core/CMakeLists.txt`'s
`add_library` list (platform files in its `if(APPLE)` branches), and its
tests in `core/tests/CMakeLists.txt`; there is no globbing, and
`check-sources.py` compares the lists with the disk.

## A command

1. **Write it** as a `#[tauri::command]` in the module that owns the
   work, returning `Result<T, String>`. Database work goes on a blocking
   thread (`on_library`), engine work through `audio::with_engine` or
   the queue's `run`; never hold the library's connection while waiting
   for the main thread.
2. **Register it** in `lib.rs`'s `generate_handler!`. The main window's
   permission is written from that list by `build.rs`; if the mini
   player or Help window calls it, add it to
   `app/src-tauri/permissions/mini-window.toml` or
   `help-window.toml`. A command that takes a raw path or bypasses the
   queue goes in `dev.rs` under `#[cfg(debug_assertions)]` instead.
3. **Types**: each new argument or result type derives `Serialize` (or
   `Deserialize`) with `#[serde(rename_all = "camelCase")]` and
   `#[cfg_attr(test, derive(ts_rs::TS))]`, and is added (or a type
   containing it is) to `bindings.rs`'s `declare_types`.
4. **Regenerate**: `ANOMP_WRITE_BINDINGS=1 cargo test bindings` writes
   its wrapper into `generated/commands.ts` and its types into
   `generated/ipc.ts`; commit them.
5. **Wrap it** in `app/src/lib/api.ts`, calling `commands.<name>` (never
   `invoke` with a string).
6. **Errors the user should read** are made with `crate::coded` and get
   an `error.<code>` message in `en.json` (`tests/i18n.test.mjs`
   checks), and an entry in the user guide's errors appendix
   (`appendix-d-errors.md`, placeholders written "…").
7. **Tests**: the logic it wraps is tested in its module; the command
   itself stays thin.

## An event

1. A `pub const NAME_EVENT: &str = "kebab-name";` next to the code that
   sends it, and `app.emit(NAME_EVENT, &payload)`.
2. The payload type derives `TS` and is in `declare_types`; regenerate.
3. Add it to `api.ts`'s `Events` map, then listen with `on(…)` in the
   store that follows it (inside its `connect`).

## A setting

1. **The field** goes in the right part of `AppSettings`
   (`settings.rs`, or the struct it holds: `PlaybackSettings`,
   `DisplaySettings`…), with its default in the `Default` impl and a
   rule in `validate` (a range, a length, a choice). Stored values are
   read leniently, field by field, so no migration is needed and an old
   file simply gets the default.
2. **Apply it**: if a change must reach something at once (the engine,
   a worker), add a case in `settings_save` after the value is stored,
   following the ones there.
3. **Regenerate** the bindings (`generated/settings.ts`) and the user
   guide's settings reference: add the setting's `row` to `guide.rs`'s
   rows (its section, its JSON path, its label's key, how it is shown,
   its range), or `cargo test` fails with "the setting … isn't in the
   user guide's settings reference".
4. **The control** in its Settings section
   (`lib/components/settings/*.svelte`), saved through
   `appSettings.save(next => …)`, labelled from `en.json`.
5. **The user guide**: its section in `11-settings.md`.

## A feature switch

An optional feature (O1–O19's kind) is a setting with rules of its own:

1. A field in `FeatureSettings` (`settings.rs`), **off by default** if
   it costs a lot, changes what is heard, goes online or listens on the
   network.
2. Check it **where the feature acts**: commands refuse with
   `coded::feature_off`, workers idle, the UI hides what it adds
   (`state/features.svelte.ts` knows which are on). React to it being
   switched in `settings_save`'s features block if a worker or the
   engine must follow at once.
3. Its label is `feature.<field>` in `en.json` (or list another key in
   `check-user-guide.py`'s `SWITCH_LABELS`), shown in
   `FeaturesOptions.svelte`.
4. Its `guide.rs` row, the bindings regenerated, and the user guide
   names it (`check-user-guide.py` checks every `FeatureSettings` field).
5. If it goes online, it goes through `http::Client`, and the privacy
   chapter of the user guide says what it sends. If it listens on the
   network, it needs the security review in `PLAN.md` §8.1.

## A migration

1. **A new file**, `app/src-tauri/src/library/migrations/<NNN>_<name>.sql`,
   numbered one past the last (`check-migrations.py` checks there are no
   gaps), with comments explaining each table and column. Never edit a
   migration that has shipped.
2. **List it** at the end of `MIGRATIONS` in `library/db.rs`.
3. **Keep to plain SQLite**: no `anomp_sort_key`, `anomp_genres` or
   `anomp_has_genre` in a table, index or trigger (other clients don't
   have them). A change to a column of `tracks`, `albums` or `artists`
   that the search indexes cover updates **both** sets of triggers (word
   and trigram).
4. **Test it** on an in-memory database and on a file one (`db` tests'
   `file_at`), so the copy `db::open` writes first is exercised; test
   data written by the old schema reads correctly after.
5. If the new data is the user's, add it to the export and import
   (`library/transfer.rs`).
6. Describe the table in [chapter 9](09-data.md). Run `scripts/bench.py
   --only rust` if it touches what browsing or search reads.

## A metadata source

A new online service is also an owner decision (its terms, `PLAN.md`
§8.1) and a privacy question; settle those first.

1. **Its id**: a `SourceId` in `metadata/settings.rs`, with its
   `SourceInfo` (name, kinds of data, `online`, `needs_key`, notices),
   and its place in each kind's default order.
2. **Its client**: a module in `metadata/` that fetches only through
   `http::Client` (`get_json`, cached; or `get_json_fresh` if its terms
   forbid keeping data), with its rate in `http.rs`'s
   `request_interval`, parsing into the shared types (`Release`, …). An
   album-details source implements `ReleaseSource` (`albums.rs`).
3. **Its jobs**: what the worker does with it, in `metadata/jobs.rs`,
   with a `needs_…` rule saying when to try again.
4. **A key**, if it needs one, through `metadata::keys` (the keychain;
   registered with the log's redaction), never in the settings.
5. **Tests** with recorded responses in `metadata/fixtures/<source>/`,
   each with its entry in `manifest.json` saying how it was made
   (`scripts/record-fixtures.py` re-records them), and the fake
   `Transport` and `Clock`. A live check is `#[ignore]`d and named
   `live_…`.
6. **The UI**: its name and notices in `en.json`, shown by
   `ServicesPanel.svelte`; what it is sent in `WelcomeView.svelte`.
7. **Docs**: the sources table in
   `docs/design/phase-4-online-metadata.md`, and the user guide's online
   and privacy chapters.

## An effect

See [chapter 5](05-effects.md#adding-an-effect): the effect class, its
place at the end of `EffectType` and in `chainOrder`, `create`, the
catalogue, `ANOMP_EFFECT_*`, `EffectsSettings` and its `get`, the
`en.json` messages, its tests, and the 192 kHz budget.

## A visualization

1. **The renderer**: a file in `app/src/lib/visualizer/renderers/`
   exporting a `Visualization` (`types.ts`): an `id`, a `name` and
   `description` read from `en.json` (`viz.<id>.name`,
   `viz.<id>.description`), and `create()` returning a `Renderer` whose
   `draw(scene)` paints one frame. Clear the stage with `util.ts`'s
   helpers, take colours from `scene.palette`, and honour `scene.calm`
   (move slowly, never jump); beats already respect the flash limit.
2. **Register it** in `lib/visualizer/index.ts`'s `VISUALIZATIONS`, in
   picker order, and add its id to `guide.rs`'s `VISUALIZATIONS` (same
   order), which checks its name exists.
3. **Pure logic** it needs (a musical analysis, a transform) goes in a
   module without imports, tested in `tests/visualizations.test.mjs`.
4. The user guide's visualizations chapter describes it; the map here
   lists its file.

## A theme colour

1. A field in `ThemePalette` (`theme.rs`), and its entry in
   `ThemePalette::colors` (the array's length with it), so `validate`
   checks it is a `#rrggbb` colour.
2. A value in **every** theme in `app/src/lib/themes.json`, light and
   dark.
3. `theme.ts`: its custom property in `COLOR_TOKENS`, and in `PAIRS`
   for each colour it is drawn on, so `tests/contrast.test.mjs` checks
   WCAG AA for it and the theme editor flags a pair below.
4. Its label, `appearance.color.<token>`, in `en.json` for the theme
   editor (`AppearanceOptions.svelte`); regenerate the bindings.
5. Use it in components as `var(--its-name)`; never a literal colour.

## A UI string

1. A key in `app/src/lib/i18n/en.json`, named by area
   (`library.…`, `settings.…`). A count is a plural: an object of
   `Intl.PluralRules` forms (`"one"`, `"other"`) using `{count}`.
2. Show it with `t("key", params)`; `npm run check` fails on a key that
   doesn't exist. Never a literal in a component.
3. An error from Rust is a coded error ([A command](#a-command), step 6),
   shown with `errorText`.
4. If the user guide names the control, it names it exactly as this
   string does; change both together.

## A decoded format

1. Its demuxer, decoder and parser in `scripts/build-ffmpeg.sh`'s lists
   (the configure flags stay minimal on purpose), then rebuild FFmpeg.
2. `core/tests/FFmpegBuildTests.cpp`'s exact list of what the build
   contains.
3. A fixture of the test chirp in `core/tests/fixtures/`
   (`scripts/make-test-fixtures.py`, `--only NAME`), its length and lag in
   the decoder tests' table, and in `self_test.rs`'s embedded list.
4. If TagLib should read its tags, its `WITH_*` in `cmake/TagLib.cmake`
   (only with its FFmpeg demuxer).
5. Its extension in `tauri.conf.json`'s `fileAssociations` if the Finder
   should offer ano-mp for it, and the user guide's formats appendix.

## A recording format

A muxer and encoder in `build-ffmpeg.sh`'s lists, `FFmpegBuildTests.cpp`'s
exact list, a `RecordingFormat::Kind` (`core/src/Recorder.h`) and its
case in `FFmpegEncoder.cpp`, an `ANOMP_RECORD_*` value, Rust's
`RecordingKind` (`anomp.rs`) and its use in `recording.rs`, its label in `en.json`, and the user
guide's recording chapter.
