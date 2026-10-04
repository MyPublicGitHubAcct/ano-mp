# ano-mp Developer Guide

How ano-mp's code is organised and how it works, for anyone who wants to
change it, test it or find out why it behaves as it does. It assumes you
can program, not that you know JUCE, Rust, Tauri or Svelte: where one of
them matters, the guide says what it does here.

Read [Overview](01-overview.md) first. After that, go straight to the
part that interests you: the [repository map](03-repository-map.md) finds
the file behind a behaviour, [How things flow](08-flows.md) follows an
action across the layers, and [Troubleshooting](10-troubleshooting.md)
starts from a symptom.

This guide describes the code as it is. Three other documents sit beside
it, and it links to them rather than repeating them:

- [`CLAUDE.md`](../../CLAUDE.md): the rules each area keeps (threads,
  sandbox, migrations, logging, UI text…), stated once. This guide
  explains them; the rules stay there.
- [`PLAN.md`](../../PLAN.md): the roadmap, the decisions and why they were
  made, and what is still open.
- [`docs/design/`](../design/): how each phase was built and its known
  limits, in the order things happened.

The [user guide](../user-guide/README.md) explains the app to the people
who use it; its names for screens and controls are the ones the code's
messages (`en.json`) use.

## Chapters

1. [Overview](01-overview.md): the three layers, why the core is a
   static library, the one C API, who owns which thread, and a track's
   journey from disk to speaker.
2. [Getting set up](02-getting-set-up.md): tools, the first build,
   running the app and the tests, the presets, first-build failures.
3. [Repository map](03-repository-map.md): every folder and module in a
   line or two.
4. [The core](04-core.md): the C++ audio engine, decoding, gapless
   playback and crossfade, tags, analysis, recording, platform code and
   the C API's conventions.
5. [Effects](05-effects.md): the effect chain, how settings reach the
   audio thread, and adding an effect.
6. [The Rust backend](06-rust-backend.md): start-up, the engine and the
   queue on the main thread, the library, metadata, settings, the shell,
   the remote, logging and recovery.
7. [The frontend](07-frontend.md): windows and routes, state stores,
   commands and events, text, themes, lists and the visualizer.
8. [How things flow](08-flows.md): worked sequences across the layers:
   play, hand-off, scan, search, metadata, a setting, quitting.
9. [Data](09-data.md): the database table by table, the settings, the
   caches, and every file the app writes.
10. [Troubleshooting](10-troubleshooting.md): logs, diagnostics, the
    /dev page, one test, a fuzzer crash, the bundle, and a symptom table.
11. [Recipes](11-recipes.md): the full list of places to touch to add a
    C API function, a command, a setting, a migration, an effect and
    more.
12. [Testing](12-testing.md): the suites, fixtures, fakes, benchmarks and
    fuzzing.
13. [Building and releasing](13-building-and-releasing.md): the release
    scripts and what each checks.

## Conventions in this guide

- Paths are from the repository's root, or shortened to the part that
  identifies them (`library/scanner.rs` for
  `app/src-tauri/src/library/scanner.rs`); `check-docs.py` checks that
  every one exists.
- `Name::function` and `Name.method` refer to code; a function's file is
  named the first time it appears in a section.
- Plan items are cited by their numbers (O1–O19, F1–F21, H1–H22, X1–X7),
  which `PLAN.md` and the design notes explain.
- Diagrams are Mermaid, which GitHub renders, so they are reviewed and
  diffed like the code.

## Keeping it accurate

A change that moves a responsibility updates this guide in the same
commit, as it updates the code's comments. `scripts/check-docs.py` checks
every path and link here, and `scripts/check-developer-guide.py` checks
that the repository map names every module and nothing that has gone.
Both run in `scripts/check-all.py --quick` and the pre-commit hook.
