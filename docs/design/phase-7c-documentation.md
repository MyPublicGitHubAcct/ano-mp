# Phase 7c — Documentation: the record

Moved from `PLAN.md` (D4) on 2026-10-09, unchanged but for the headings.
Section numbers (§) refer to `PLAN.md`, whose
[Phase 7c](../../PLAN.md#phase-7c--documentation) keeps the phase's
aims, the D1–D4 table, the unfinished D1, D2 and D4, and the order of
work. Each finished D item's record moves here.

## D3 Dictionary of classes

- [x] **D3 Dictionary of classes.** Written 2026-10-04:
  `docs/class-dictionary/`, one page per layer (the C API, the core, the
  effects, the Rust backend, the frontend; 698 entries), each
  alphabetical, and a README explaining the entries with a generated
  index by layer and folder and A to Z. `scripts/class-dictionary.py`
  lists the types from the sources (stdlib only, a brace-aware scan that
  skips comments and strings) and, with `--check`, in `check-all.py`'s
  quick checks, fails on a type without an entry, an entry for a type
  that's gone, a non-helper entry without a description, entries out of
  order, a link into the developer guide whose heading doesn't exist, or
  a stale index; without `--check` it rewrites the index. Decided while
  writing it: the unnamed enums of `anomp.h` are listed by their
  constants' prefix (`ANOMP_STATE_*`); a C API struct the core defines
  (`struct anomp_engine`) is one entry on the C API page with both files;
  nested C++ types are named with their owner (`PlayerEngine::Track`);
  test-only Rust code (`#[cfg(test)]` items, and the modules declared
  under it: `bindings.rs`, `guide.rs`, `library/bench.rs`,
  `library/test_library.rs`) is left out, as are TypeScript types that
  aren't exported; several modules' private helpers of one name (`Row`,
  `Entry`, the services' `Raw*` JSON) are told apart by file; the C
  layout mirrors in `anomp.rs` and the services' raw JSON types are
  helpers, described in a line. `CLAUDE.md` makes keeping it current part
  of each change.
  - Exit check done 2026-10-04: `--check` passes. Twenty entries (seed
    20261004, Helpers left out, at least two a page) were read against
    the code: three were wrong, all on the Rust page: `LoadResult` named
    `Queue::load_finished` as acting on it (it is `queue/opening.rs`'s
    `engine_finished`), `SleepTimer` wasn't marked as sent to the UI,
    and `Transport` was placed on the metadata worker (every `Client`
    uses it, ListenBrainz's and the update check's too) and left out its
    POST. Since that was more than two, twenty more (seed 20261005) were
    read, all correct, and every Rust entry's kind was compared with its
    type's derives, which found one more (`Edit`, sent to the UI but not
    marked, its `Update` variant missing). All four fixed.

  An alphabetical reference of every
  named type a developer meets: C++ classes and structs in `core/` and
  `effects/` (public and private), the C API's types and enums, Rust
  structs, enums and traits in `app/src-tauri`, and the frontend's
  TypeScript types, state modules and Svelte components. Each entry
  gives: the name, its layer and file (linked), one or two sentences on
  what it is responsible for, its main collaborators (what it owns, what
  calls it), the thread it lives on where that matters, and a link to
  the D2 section that explains it in context. Generated types
  (`generated/`) are listed once, by where they come from.
  - **Generated skeleton, written descriptions.** A stdlib-only script
    lists the types from the sources (C++ `class`/`struct` declarations,
    Rust `pub struct`/`enum`/`trait` items, TS exported types, Svelte
    files) and checks the dictionary against it: a type without an entry,
    or an entry for a type that's gone, fails `check-all.py`. The
    descriptions are written by hand, since a generated one would only
    repeat the name; where a type has a doc comment, the entry and the
    comment say the same thing (the comment wins and the entry is
    copied from it). Tiny private helpers may be listed by name only,
    marked as such, so the list stays complete without padding.
  - Indexes beside the alphabetical list: by layer and by folder, so a
    reader can scan "everything in the queue" as easily as look up one
    name.
  - **Exit:** every type in the tree has an entry and the check passes;
    a sample of twenty entries, picked at random, read correctly
    against the code.
