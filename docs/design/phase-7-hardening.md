# Phase 7 — Hardening (macOS): the record

Moved from `PLAN.md` on 2026-10-03, unchanged but for the headings.
Section numbers (§) refer to `PLAN.md`, whose Phase 7 keeps the
order of work, the hardening table with each item's status,
the open items (H4, H13's P2 part, H19) and §9.2's open scripts.

## Order of work, step by step

**Order of work** (set 2026-10-02). Phases 0–6c are built, but the exit
checks of Phases 4, 5, 6, 6b and 6c are not done yet, and only part of
H3 and H7 among the P1 hardening items is done. Work through these steps
in order. Step 6 runs alongside all of them.

1. **A repeatable baseline (about 1 day). Done 2026-10-02.**
   - Every suite passed on a clean tree: ctest 99, `cargo test` 342 (9
     ignored), `npm test` 18, `svelte-check` 0 errors.
   - `check-all.py` runs them, after the formatters, the three M2 checks
     and the scripts' tests. It also builds the frontend before
     `cargo test`, since Tauri embeds `app/build` when the crate compiles
     and a fresh clone has none.
   - `check-c-api.py` found two C functions Rust doesn't bind
     (`anomp_track_options_default`, `anomp_media_controls_perform`);
     both are deliberate and listed in its `NOT_BOUND`.
   - The CI job (`macos-15`) hasn't run yet: it runs on the first push.
   - Run every suite (ctest, `cargo test`, `npm test`, `npm run check`)
     to confirm they pass.
   - Write `check-all.py` (M4), with the M1 pytest harness and the cheap
     M2 checks (`check-c-api.py`, `check-sources.py`,
     `check-migrations.py`).
   - Add a GitHub Actions macOS job that runs only `check-all.py`.
   - This comes first because everything after it is hardening, and
     hardening without CI tends to slip back.
2. **Quick P1 security and lint fixes (1–2 days). Done 2026-10-02.**
   - Every suite passed on a clean tree: ctest 100 (also under the `asan`
     and `tsan` presets), `cargo test` 342 (9 ignored), `npm test` 21,
     `svelte-check` 0 errors, script tests 30. `check-all.py` has 21
     steps; each item's entry below says what it added.
   - The CI job still hasn't run: Step 1's commit isn't pushed yet.
   - Left for the owner: turning on GitHub secret scanning with push
     protection (and Dependabot security updates) in the repository's
     settings, and installing the hook in each clone
     (`git config core.hooksPath scripts/hooks`).
   - H1 (CSP), H2 (developer surface out of release builds).
   - H3's remaining half: the opener still allows `http://*`.
   - H8 (clippy, `[lints]`, toolchain pins), H7's remainder (SHA pins for
     JUCE and Catch2, `cargo deny`, the update bot, secret scanning).
   - H13's P1 part (ESLint and Prettier), H6 (sanitizer presets).
3. **P1 items that protect user data (3–5 days). Done 2026-10-02
   (H22b with Step 4).**
   - Every suite passed on a clean tree: ctest 102 (also under the `asan`
     and `tsan` presets), `cargo test` 383 (9 ignored), `npm test` 21,
     `svelte-check` 0 errors, script tests 30. `check-all.py` (still 21
     steps) ran in full after each item.
   - The CI job still hasn't run: Steps 1–3 aren't pushed yet. Secret
     scanning and the hook are still the owner's to turn on.
   - H10 (DB backups and checks): the DB now holds playlists, ratings,
     history and picks that a rescan can't recreate.
   - H22 (missing folders at launch): a folder whose path exists but is
     empty was emptied by the launch rescan. Split in two: H22a (the
     data loss, the folder states, the launch message, the queue, the
     workers, drives coming back) is done; H22b (the lists) is left.
   - H9 (logs, panics, diagnostics): release builds aborted on a panic
     and left no trace.
   - Wording drafted for the owner to review: the missing-folders message
     (`missing.*`, `folderState.*`, `folderShort.*` and
     `error.folderUnavailable.*` in `en.json`), the database repair offer
     (`dbRepair.*`) and Settings › About (`about.*`).
   - What only a sandboxed bundle shows is in Step 4's list.
4. **Exit checks in a sandboxed bundle. Probe checks done 2026-10-02;
   the owner's checks wait.**
   - H22b came first (H22's entry). After it every suite passed:
     ctest 102 (also under `asan` and `tsan`), `cargo test` 387 (9
     ignored), `npm test` 25, `svelte-check` 0 errors, script tests 30,
     `check-all.py` all 21 steps.
   - The checklist is `docs/step4-checklist.md`, ordered by risk, each
     entry saying how to check it, what passing looks like, and who.
   - The real container was protected by moving the app's own files out
     of it and back (macOS won't let another process move or fully copy
     a container); see "Running a bundle check safely" in `CLAUDE.md`.
   - A temporary probe, compiled into the bundle and removed afterwards,
     ran stages against a scratch library of `core/tests/fixtures`
     written into the container:
     - scan, bookmark and covers in the sandbox, a gapless hand-off, the
       diagnostics without paths: passed;
     - folder states with real bookmarks (empty and kept through a
       scan, in a Trash and not followed, deleted, back), H22b, a queue of
       unreadable tracks not spinning: passed after a fix (H22);
     - the log in the container (format, no paths): passed;
     - a panic in a release build: passed;
     - the database repair, restore and rebuild: passed (the dialog is the
       owner's to see).
   - Found: every quit crashed (H9's entry), a deleted folder read as "no
     access" (H22's entry). Each fixed with a test; after the fixes every
     suite passed: ctest 104 (also under `asan` and `tsan`), `cargo test`
     387 (9 ignored), `npm test` 25, `check-all.py` all 21 steps.
   - Left for the owner (the checklist's unticked entries): Finder drops,
     Open With, USB, SMB, the Trash and deletion outside the container, a
     rebuilt bundle's folders, "Show logs", "Copy diagnostics", the repair
     dialog, the local-network prompt, the keychain, playback by ear, the
     rest of the app, and the visualizers.
   - Merge the exit lists of Phases 4, 5, 6, 6b and 6c into one
     checklist, ordered by risk:
     - sandbox-only behaviour first (folder drops, Open With, the LAN
       remote's local-network prompt, missing folders);
     - then gapless playback and cue sheets, crossfade, the equaliser and
       ReplayGain by ear;
     - the visualizers last.
   - This comes after H9 so that failures leave logs. Each failure
     becomes a fix with a test.
   - From Step 3, only a sandboxed bundle shows these:
     - H22's own list (a USB drive unplugged at launch, then plugged in;
       an SMB share not mounted; a folder moved to the Trash; a folder
       deleted), with what each shows: the folder's state and reason,
       the launch message, the queue skipping, the drive found again
       without a restart (the `VolumeWatcher`'s NSWorkspace notifications
       under the sandbox).
     - A rebuilt bundle's folders: their bookmarks don't resolve, so they
       should read as "no access" (`noPermission`), keep their tracks,
       and come back with "Locate…". `access::unresolved` tells that from
       "missing" by the bookmark error's text, or by the stored path still
       being a folder, which the sandbox may hide.
     - The log in the container (`~/Library/Containers/<id>/Data/Library/
       Logs/<id>/ano-mp.log`), "Show logs" revealing it in the Finder,
       and "Copy diagnostics" (`navigator.clipboard` in the webview).
     - A panic in a release build: the log ends with it (a temporary
       probe).
     - The database repair offer: restore and rebuild, each restarting
       the app (a damaged copy of a scratch library).
5. **The larger core P1 items (about 1 week). Done 2026-10-02; the
   owner's checks are § 5 of `docs/step4-checklist.md`.**
   - Every suite passed on a clean tree after each part: ctest 114 (also
     under `asan` and `tsan`), `cargo test` 398 (9 ignored), `npm test`
     25, `svelte-check` 0 errors, script tests 33, `check-all.py` all 22
     steps (the new one: "core fuzzing", each target for 60 s).
   - The owner's answers that shaped it: Steps 3 and 4 pushed (the Actions
     runs' results not yet seen here); none of Step 4's owner checks known
     to have failed; Homebrew LLVM for the fuzzers (`llvm@22`), with
     FFmpeg instrumented too; the CI cache and the weekly fuzzing job; the
     wording "Opening…", "The file took more than {seconds} s to open",
     "Downloading from iCloud…" and "{count} in iCloud, not downloaded".
   - The probe in the checklist's § 5 ran on 2026-10-02 with Step 7 and
     passed (Step 7's entry).
   - No bundle check was run in Step 5: H11 holding a folder open while
     another thread opens the file, and H12's placeholders, are in the
     checklist's § 5 (one probe, the rest the owner's).
   - H5 (fuzzing).
   - H11 (opening files off the main thread). This is the riskiest
     change to the engine, so fuzzing and the sanitizers come first.
   - H12 (cloud placeholders), which builds on H11.
6. **Owner decisions (§8.1), alongside steps 1–5.** These block the
   first release however far the engineering gets:
   - the name and trademark check, then the bundle identifier, which
     can't change after the first release and which signing needs;
   - the JUCE licence;
   - the AAC opinion;
   - the privacy policy and support URL;
   - the MetaBrainz plan and a real `User-Agent` contact;
   - whether to have crash reporting.

   **Briefs written 2026-10-02** (`docs/release-decisions.md`), one per
   item, in the order they block: the name, then the bundle identifier
   and publisher; the distribution channels; the Developer Program and
   Developer ID; the JUCE licence; the AAC opinion; the privacy policy
   (with a table, from the code, of what the app sends where); the
   MetaBrainz plan and the `User-Agent` contact; crash reporting. None is
   decided yet. Read-only lookups: no DNS records for the candidate
   domains, and WHOIS shows `anotracks.com`, `anotone.com` and
   `anotraks.com` unregistered; the `.app` registry's WHOIS didn't answer.
7. **Release setup without a certificate (§8.2, and §8.3's parts that
   need none; M6). Done 2026-10-02; the signed release waits on Step 6.**
   - Every suite passed on a clean tree after each part: ctest 115 (also
     under `asan` and `tsan`), `cargo test` 399 (9 ignored), `npm test`
     25, `svelte-check` 0 errors, script tests 72, `check-all.py` all 24
     steps (new: "version number", quick, and "third-party notices").
   - The owner's answers that shaped it: the Actions runs haven't run yet;
     which of the checklist's owner entries are done isn't known, and
     none is known to have failed; no §8.1 decision made yet; the crates'
     licences from `cargo metadata` rather than a new tool; the About
     wording ("Third-party notices" and its hint), with Discogs' notice
     there too; the release workflow as proposed.
   - Step 5's bundle probe (the checklist's § 5) passed in a sandboxed
     bundle, with the real container's files moved out and back (identical
     after): the 21 fixtures played through with 20 gapless hand-offs,
     every load handed to the engine with its folder held, none failed;
     then 9 skips back to back, 5 superseded while opening, no folder left
     held. The probe's first run stopped on its own mistake (it skipped
     while the short fixtures kept playing), not the app's.
   - One version number: `anomp_version()` is compiled from CMake's
     `project(VERSION)` (`ANOMP_VERSION`), and `scripts/version.py` sets
     or checks the copies the tools need (`Cargo.toml` and `Cargo.lock`,
     `tauri.conf.json`, `package.json` and both versions in
     `package-lock.json`). The `User-Agent` and ListenBrainz already took
     the version from the core; the contact is a marked placeholder
     until the owner gives one. Tests: 9 script tests, the C API test
     against `project(VERSION)`, a bump round trip through the core.
   - Third-party notices: `scripts/make-notices.py` writes
     `THIRD_PARTY_NOTICES` (374 KB): JUCE (its licensing statement, and
     zlib, the only code it vendors into the linked modules; FLAC, Ogg,
     Vorbis, Oboe and ASIO are listed as not in the app, and a new
     vendored library fails the script), FFmpeg (LGPL 2.1, version,
     configure flags, source tarball and SHA-256), TagLib under the MPL
     with utfcpp, Signalsmith Stretch and Linear, Catch2 as not shipped,
     264 crates and the 6 npm packages the frontend bundles (recorded by
     a Vite plugin at build time). Each licence expression must be
     satisfiable from `deny.toml`'s list; identical texts are printed
     once. 16 crates and the 4 Tauri npm packages ship no licence text,
     so standard texts are committed in `scripts/licenses/`. The bundle
     carries the file; Settings › About opens it in a dialog and shows
     Discogs' notice. Tests: 15 script tests, a Rust test that the bundle
     maps the file under the name the command reads.
   - The release workflow (`release.yml`): on a `v*` tag it calls
     `ci.yml` (now also `workflow_call`, and run on branch pushes only;
     since Step 8, by hand and through `release.yml` only),
     builds the universal app and DMG with `build-app.py`, notarizes when
     the secrets exist, and `release.py` checks the bundle, zips the app,
     writes `SHA256SUMS` and cuts the notes from the new `CHANGELOG.md`;
     the files go to a draft GitHub Release with `gh`. By hand, on a
     branch, it is a trial with the files as an artifact. It hasn't run
     yet.
   - macOS without a certificate: `build-app.py` (the hardened-runtime
     rule), `check-bundle.py`, `notarize.py` (dry run without
     credentials), `check-signing.py` (no identities yet), and the
     entitlements reviewed (§8.3). `rust-toolchain.toml` adds the x86_64
     target. A universal build here passed `check-bundle.py`: arm64 and
     x86_64 in the executable and the four FFmpeg dylibs, `@rpath` install
     names and the `@executable_path/../Frameworks` rpath, no library from
     outside the bundle and the OS, `codesign --verify --deep --strict`,
     the entitlements and notices as committed; the DMG (14.5 MB) verified,
     and `release.py`'s trial run wrote files whose `SHA256SUMS` check.
     Tauri's DMG step drives the Finder by AppleScript unless `CI=true`,
     and waits on the Automation permission locally, so local runs set it.
     The hardened runtime with a real identity, notarization and
     Gatekeeper wait for the certificate.
   - Found: fuzzing found undefined behaviour in TagLib's Shorten reader
     in the first full run; fixed with a test and a fixture (H5's entry).
   - Waiting on the owner, in the order they block: the decisions in
     `docs/release-decisions.md`; then the Developer ID certificate, the
     notarization key and the six secrets; the updater (after the
     distribution decision); the icon set (after the name); a first trial
     of `release.yml`; the wording of `CHANGELOG.md`'s first entry and of
     `error.noticesUnreadable`; the About page by eye.
8. **The signed release (§8.3 with a Developer ID, §8.7), as far as the
   owner's decisions allow. Begun 2026-10-02; its signing parts wait on
   Step 6.**
   - The owner's answers: Step 7 committed and pushed (`fd3aeae`); none
     of the eight decisions in `docs/release-decisions.md` made; which of
     the checklist's owner entries are done not known (none known to have
     failed); `CHANGELOG.md`'s first entry and `error.noticesUnreadable`
     kept as drafted; no identifier chosen, so no container move.
   - CI (from the owner's screenshot of the Actions page): runs #6
     ("completed step 5") and #9 ("completed phase 7 in six parts")
     failed on `main`, #9 after 7 min; Dependabot's runs based on them fail
     too. `check-all.py` passed here in full, all 24 steps, on the same
     tree, so the cause is in the runner; the failing step's log is
     awaited, and the fix comes first once it is in. Meanwhile the owner
     set CI to run by hand and through `release.yml` only (`ci.yml`,
     README's "When GitHub Actions run"), so a push no longer runs it.
   - Done: the `__pycache__` folders in `scripts/` and `scripts/tests/`
     (17 committed `.pyc` files) untracked and ignored.
   - Done: Part 4's checklist, `docs/release-smoke-test.md`: checksums,
     Gatekeeper on the DMG and at first launch, import and relaunch, MP3,
     FLAC and AAC, seeking, a gapless album, media keys, a MusicBrainz
     lookup, the log, and the update from the previous version (once the
     updater exists). The owner runs it on each release's DMG. The dry
     run of §8.7 waits on a signed trial of `release.yml`; `bench.py`
     (§8.7 step 4) doesn't exist yet (M4).
   - Done: H14, the sandboxed bundle's self-test (its entry), proposed as
     Part 5's first item and agreed. Every suite passed afterwards:
     ctest 115 (also under `asan` and `tsan`), `cargo test` 405 (9
     ignored), `npm test` 25, `svelte-check` 0 errors, script tests 79,
     `check-all.py` all 25 steps (new: "bundle self-test", which skips
     itself outside CI).
   - Waiting on the owner, in the order the owner takes them (set
     2026-10-03: the name moved last):
     1. ~~the CI log of run #9 (and #6)~~ fixed 2026-10-03 (below);
        CI on `main` itself waits for the owner to push the fix and run
        it;
     2. the distribution channel and where releases are hosted (Part 1,
        the updater, if a direct download). The owner made the
        repository public on 2026-10-03, so its GitHub Releases can host
        downloads and Actions minutes are free; the channel is still
        open, and iPhone and iPad wait for the last steps;
     3. the Developer Program and the Developer ID certificate (Part 2),
        then the App Store Connect API key and the six secrets (Part 3);
     4. the JUCE licence, the AAC opinion, the privacy policy and support
        URL, the `User-Agent` contact and MetaBrainz plan, crash
        reporting;
     5. the checklist's owner entries (`docs/step4-checklist.md`) and,
        per release, `docs/release-smoke-test.md`;
     6. the name, bundle identifier and publisher (Part 0's rename, then
        the icon set from the artwork). Still blocks the first signed
        release (the identifier is in the signature and the container)
        and an App Store record. Asked 2026-10-03: the trademark,
        domain and App Store checks not done, the account type
        undecided, no artwork. Checked then: no DNS records for
        `anotracks`, `anotone` or `anotraks` under .com, .app or .dev;
        WHOIS "No match" for the three .com names; .app and .dev have no
        WHOIS server any more (IANA lists none; Google's registry
        answers RDAP only), so a registrar's search settles those.
   - **Part 5 done 2026-10-03:** the CI fix first, then `bench.py` (M4)
     and H18's budgets, H20 (with `check-docs.py`, M2), H15, H16 and H17,
     each in its entry. After each part `check-all.py` ran in full; after
     the last, all 27 steps passed (new: "docs' paths and links", quick,
     and "docs' test counts"): ctest 115 (also under `asan` and `tsan`),
     `cargo test` 433 (12 ignored), `npm test` 30, `svelte-check` 0
     errors, script tests 132.
   - The owner's answers that shaped Part 5: Step 8 committed and pushed
     (`be6e6ad`); `gh` to read CI's logs; no §8.1 decision made; whether
     any checklist entry failed not known, so the container's log wasn't
     read; Homebrew `lld@22`, then (when it failed) Xcode 26.3 for the
     fuzz build only, and the downloads in CI's cache; a scratch branch
     pushed and CI run on it, then deleted; the budgets, the margin, the
     core benchmark, the owner checks' wording and `bench.py` as a release
     step; browse pages at 100 ms; playback at 20× real time; the split of
     this file, Phase 4's decisions kept here, `--counts`; the syn
     generator over tauri-specta; the test-only derive in `remote/`; the
     `image` crate over ImageIO, and the thumbnails' index.
   - CI (runs #6 and #9): two causes, both fixed.
     - The fuzz build failed on every run: the macos-15 runner's default
       Xcode 16.4 links with an ld that can't read clang 22's objects
       ("invalid r_symbolnum"); here the newer ld can. Homebrew's
       `lld@22` linked it, but broke C++ exceptions in it (the tags
       fuzzer died on an exception TagLib catches itself), so it was
       dropped and uninstalled. Now `run-fuzzers.py` passes
       `ANOMP_FUZZ_DEVELOPER_DIR` to its own builds as `DEVELOPER_DIR`,
       and `ci.yml` and `fuzz.yml` set it to the runner's Xcode 26.3;
       every other build keeps the default Xcode. 4 script tests.
     - Run #9 also lost the asan and tsan presets to 502s from GitHub's
       archive downloads, made likely by each preset downloading every
       tarball again. `cmake/Fetch.cmake` (`anomp_fetch_declare`) now
       downloads each pinned tarball once into `build/_downloads`, checks
       its SHA-256, retries three times with backoff, and hands FetchContent
       the file; `ci.yml` caches the folder. 6 script tests (`cmake -P`
       over `file://` URLs).
     - Verified on a scratch branch (ci/fuzz-linker, deleted after): CI
       passed all 25 steps it had then, the fuzzers built with Xcode 26.3,
       and H14's bundle self-test passed every stage on the runner for the
       first time, playback included ("handed off after 0.6 s on Apple
       Virtual Sound Device").
   - Allowed, not fixed (each in its entry): 44.1 kHz files resampled at
     30× real time (H18; the budget is 20×); browsing by year or genre at
     56–73 ms (H18; 100 ms budget); restore's 290 ms of track details
     (H16); `queue/model.rs` not split (H19), having changed little.
   - **Part 6 done 2026-10-03:** the engineering that needs no owner
     decision, each part followed by a full `check-all.py` run: H21
     (clang-tidy), `doctor.py` (M4, H8's remainder), M5's dependency
     tools and weekly audit, then M3's fixture tools, each in its entry.
     After the last, all 29 steps passed (new: "tools", quick, and "core
     lint"): ctest 115 (also under `asan` and `tsan`), `cargo test` 433
     (13 ignored), `npm test` 30, `svelte-check` 0 errors, script tests
     205.
   - The owner's answers that shaped Part 6: the docs restructure and
     Part 5 pushed, CI not yet run on `main`; no §8.1 decision or
     checklist entry changed; clang-tidy pinned from PyPI through uvx
     over Homebrew's `llvm@22`; `doctor.py` as check-all's first step in
     both modes.
   - H19: no large module changed substantially, so none was split.
   - Next: the owner's items above, in order; then Parts 0–3 of the signed
     release as decisions arrive. Engineering left that needs no
     decision: H19 as modules are touched, H21 (clang-tidy), M3 and M5's
     scripts, `doctor.py`.

Then the signed release (§8.3 with a Developer ID, §8.7), then Phase 8. H20 (design
records out of this file) is P2, but it can be pulled forward whenever
this file gets in the way.

## Hardening items (H1–H22) as proposed and built

- **H1 Content Security Policy.** `tauri.conf.json` has `"csp": null`.
  Set a strict policy:
  - `default-src 'self'`
  - `img-src 'self' anomp-art: data: blob:`
  - `connect-src ipc: http://ipc.localhost`
  - `style-src 'self' 'unsafe-inline'`
  - `object-src 'none'` and `frame-src 'none'`

  Add a `devCsp` for Vite's HMR. Check that covers still load, that the
  visualizer can still read a cover's pixels (CORS on `anomp-art`), and
  that the visualizer `Channel` still works. Nothing renders remote HTML
  today (no `{@html}`; biographies are text), so this is defence in
  depth. It is needed before any richer remote content, Bandcamp (Phase
  11) or O14.
  - **Done 2026-10-02.** `tauri.conf.json` has the policy above, plus
    `script-src 'self'`, `font-src 'self'`, `base-uri 'none'` and
    `form-action 'none'`. Tauri adds the hash of SvelteKit's inline boot
    script to `script-src`.
    - `dangerousDisableAssetCspModification: ["style-src"]`: otherwise
      Tauri puts a nonce in `style-src`, and a nonce makes browsers
      ignore `'unsafe-inline'`. That would block every `style` attribute
      (Svelte's, and `app.html`'s).
    - Checked in a built bundle with a temporary probe, run against a
      scratch library of `core/tests/fixtures`:
      - no violations;
      - a cover from `anomp-art` drawn into a canvas and its pixels read;
      - a `data:` image;
      - 149 visualizer `Channel` frames in 2.5 s of playback;
      - inline styles applied.
      A remote image was blocked and reported, so the policy is
      enforced.
    - `devCsp` (the same policy, plus `ws:` for Vite's HMR) does nothing
      on desktop: in `tauri dev`, Tauri loads Vite's URL directly and
      sets no CSP there. It applies only where Tauri proxies the dev
      server (iOS, Phase 8). The LAN remote's page (`remote/`) already
      sends its own CSP.
- **H2 Developer surface out of release builds.** Release builds
  register `player_load` and `player_set_next` (any typed path), the
  test-tone commands, and the `/dev` route.
  - Register those commands only under `cfg(debug_assertions)`, or a
    `dev-tools` Cargo feature. Leave `/dev` out of the release frontend.
  - On Linux and Windows (Phases 9–10) there is no sandbox. There,
    `player_load` would hand any readable file to FFmpeg.
  - **Done 2026-10-02.** The /dev page's ten commands (`core_version`,
    `audio_device_name`, the test tone, `player_load`/`set_next`/`play`/
    `pause`/`stop`/`seek`) are in `src/dev.rs`, compiled and registered
    under `cfg(debug_assertions)` only. `build.rs` still lists them in the
    main window's permission set, so that file is the same in every
    profile; in a release build there is nothing to call.
    - The page is `components/dev/DevPage.svelte`, imported by
      `routes/dev/+page.ts` only when `__DEV_TOOLS__` (a Vite `define`)
      is true: under `vite dev`, or in the Tauri CLI's debug builds
      (`TAURI_ENV_DEBUG`). Otherwise the import is dead code and its chunk
      isn't emitted; `/dev` redirects to `/`. A release frontend build
      holds none of those command names.
- **H3 URL opener and command permissions.**
  - `opener:allow-open-url` allows every `http://` and `https://` URL.
    The links come from MusicBrainz URL relations and Wikipedia, which
    anyone can edit. Allow `https` only, and upgrade or show as plain
    text any `http` link.
  - Use Tauri 2's app-command permissions
    (`tauri_build::Attributes::app_manifest`), so a window gets only the
    commands it needs. This is required before a second window (F7), and
    before any remote-facing surface (O14).
  - **Done 2026-09-27 (the command permissions, with F7):** `build.rs`
    declares every app command in Tauri's app manifest and writes the
    main window's permission set (`permissions/main-window.toml`, from
    `generate_handler!` in `lib.rs`); the mini player's
    (`permissions/mini-window.toml`) lists the commands it uses.
  - **Done 2026-10-02 (the URL opener):** `capabilities/default.json`
    allows `https://*` only.
    - Every link goes through `lib/openLink.ts` (`openLink` for an `<a>`,
      `openWebLink` for Get Info's MusicBrainz links). It opens only what
      `lib/links.ts`'s `webLink` allows: https, with `http` upgraded.
    - Every `href` built from service data (MusicBrainz homepages,
      Wikipedia articles and licences, release pages, credits) is
      `webLink(…)`. Any other scheme gives no `href`, so it shows as plain
      text.
    - Tested in `tests/links.test.mjs`. The six copies of `openLink` in
      the components are now one.
- **H5 Fuzzing.** Users' libraries hold arbitrary files. FFmpeg's
  demuxers and decoders and TagLib are the app's largest attack surface
  (§6).
  - libFuzzer targets in `core/fuzz/`:
    - the tag reader, through an internal overload that takes a
      `TagLib::IOStream` (a `ByteVectorStream`), not a path;
    - `FFmpegAudioFormat`: open, read, and seek at random positions,
      over a `juce::MemoryInputStream`.
  - Seed the corpus from `core/tests/fixtures/`, and build with ASan and
    UBSan (a `fuzz` preset).
  - CI runs each target for 60 s (on each version tag, through
    release.yml, or by hand; not on every push since 2026-10-02). A weekly job runs them
    for longer and keeps the corpus as an artifact.
  - Each crash found becomes a regression test with a committed fixture.
  - **Done 2026-10-02 (the P1 targets and the CI run; the weekly long run
    is set up, its results to come).**
    - Toolchain: Apple's clang has no libFuzzer runtime
      (`libclang_rt.fuzzer_osx.a`), so the `fuzz` preset compiles
      everything with Homebrew's `llvm@22` (`brew install llvm@22`, also
      in CI). LLVM 21's ASan hangs at start-up on macOS 26 (Darwin 25):
      `get_dyld_hdr` mallocs while ASan initialises. LLVM 22 and 23 work.
    - `core/fuzz/`: `DecoderFuzzer` opens the input from a
      `juce::MemoryInputStream`, reads its start and four positions the
      input picks (from its last bytes, so a seed loses little);
      `TagReaderFuzzer` runs `readTags` and `readFileInfo` over a
      `TagLib::ByteVectorStream`, through new overloads taking a
      `TagLib::IOStream` (`TagReader.h` forward-declares it, so no TagLib
      header leaks). The file overloads now go through them, and a test
      checks a stream gives the file's tags.
      `FFmpegAudioFormat::silenceLog` quiets FFmpeg's stderr for the
      decoder target, which includes no FFmpeg header.
    - FFmpeg is instrumented too: `build-ffmpeg.sh --fuzz` builds the same
      pin and formats as static arm64 libraries with libFuzzer coverage,
      ASan and UBSan into `third_party/ffmpeg/macos-arm64-fuzz/`, which
      `cmake/FFmpeg.cmake` imports when `ANOMP_BUILD_FUZZERS` is on.
      Without it libFuzzer would get no coverage from inside FFmpeg.
    - `scripts/run-fuzzers.py` builds both and runs each target (60 s by
      default) over `build/fuzz/corpus/<target>`, seeded from
      `core/tests/fixtures/`; failing inputs land in `build/fuzz/crashes/`.
      `check-all.py` runs it as "core fuzzing" (not `--quick`), so CI does.
      CI caches `build/fuzz`. `.github/workflows/fuzz.yml` runs each target
      for 30 min on Mondays (and by hand), keeping the corpus in the cache
      and uploading it with any failing inputs.
    - Found: no crashes, leaks, hangs or UBSan reports, in 10 min per
      target locally (32,681 decoder and 276,768 tag runs, 2,444 and 4,749
      new corpus units) and in every 60 s run since. One slow tag input
      (a passing stall; 0.12 s when run alone) was not a failure.
    - Found 2026-10-02 (Step 7's check-all run): UBSan in TagLib's
      Shorten reader (`shortenfile.cpp:135`, a signed left shift too far),
      reached through `FileRef`'s detection by content from a 40-byte
      input. Shorten is a format the app can't play. Fixed by building
      TagLib with only the formats FFmpeg plays (`cmake/TagLib.cmake`:
      Shorten, TrueAudio, DSF, tracker modules and Matroska off; APE kept
      for MP3's APE tags), which also takes their parsers out of the
      attack surface. The input is the first fuzz fixture
      (`core/tests/fixtures/fuzz/tags-shorten-shift.bin`), run by "Inputs
      the fuzzer found stay harmless", which failed under the asan preset
      before the fix and passes after. Not reported upstream (the owner's
      call).
- **H6 Sanitizer presets.** Add CMake presets `asan` (Address and
  Undefined) and `tsan` that build and run the Catch2 suite. `tsan`
  covers `SignalTap`, `AnalysisThread`, and the audio thread's hand-off
  and events. Both run in `check-all.py` (not `--quick`) and in CI.
  - **Done 2026-10-02.** `CMakePresets.json` has `asan` (`-fsanitize=
    address,undefined`, UBSan errors fatal) and `tsan`, each in its own
    build directory. Workflow presets (`cmake --workflow --preset asan`)
    configure, build and run ctest, with `ASAN_OPTIONS`/`UBSAN_OPTIONS`/
    `TSAN_OPTIONS` set to stop at the first report.
    - Both are steps in `check-all.py`. TSan takes about 4.5 min locally.
    - Both found nothing (100 of 100 each).
    - No test ran the engine on two threads: every `PlayerEngine` test
      renders on the thread that calls it. "PlayerEngine hands off while
      an audio thread renders" now renders on its own thread while the
      message thread queues next tracks, seeks, changes gains and
      dispatches events, for 20 hand-offs.
    - A racy program built with the same flags is caught, so the presets
      would report a real race.
- **H7 Supply chain.**
  - Pin JUCE and Catch2 by commit SHA, not tag, since a tag can move
    (TagLib and FFmpeg are already pinned by SHA-256).
  - CI installs with `npm ci`, and GitHub Actions are pinned by commit
    SHA.
  - `cargo deny` (`deny.toml`) checks advisories, licences, bans and
    allowed sources. M5's `audit-deps.py` runs it in place of
    `cargo audit`, and its licence list feeds `make-notices.py` (M6).
  - Dependabot or Renovate for Cargo, npm and Actions, grouped into the
    monthly update in §9.1.
  - GitHub secret scanning with push protection, and `gitleaks` in the
    pre-commit hook. The app handles keys now (the Discogs token), and
    signing secrets arrive with §8.2.
  - **Done 2026-09-27 (Tauri's pins):** `tauri`, `tauri-build` and the
    dialog and opener plugins are pinned exactly in `Cargo.toml`, and
    their npm packages (`@tauri-apps/api`, `cli` and the plugins)
    exactly in `package.json`. The loose `2` ranges had let `cargo`
    move `tauri` to 2.12 while npm kept `@tauri-apps/api` at 2.11,
    and `tauri build` refuses mismatched major.minor versions. Each
    crate moves together with its npm package.
  - **Done 2026-10-02 (the rest):**
    - JUCE and Catch2: `URL` GitHub's archive of the commit each release
      tag names (JUCE `7278278`, Catch2 `317ac1e`, from `git ls-remote`),
      plus its `URL_HASH`, as TagLib and Signalsmith are pinned. A
      shallow git clone can't fetch a commit by hash, and a full clone of
      JUCE is large. Each archive was hashed twice and matched the
      previous git checkout file for file.
    - Actions in `ci.yml` are pinned by commit with the release in a
      comment: checkout v5.1.0, setup-node v5.0.0, cache v4.3.0. Newer
      majors exist (v7, v7, v6); Dependabot's first run offers them.
    - `app/src-tauri/deny.toml`: `cargo deny check` (cargo-deny 0.20.2,
      installed `--locked` in CI) is a `check-all.py` step, not
      `--quick`, since it fetches the advisory database.
      - Licences: the permissive ones in the tree, plus MPL-2.0
        (cssparser and selectors, through Tauri).
      - Crates from crates.io only; wildcard versions denied; duplicate
        versions only warn.
      - It found RUSTSEC-2024-0370 (proc-macro-error unmaintained). That
        is ignored with its reason: Linux only, through Tauri's gtk-rs
        0.18. It also found a yanked `yoke-derive` 0.8.3, moved to 0.8.4.
    - `.github/dependabot.yml`: Cargo, npm and Actions, monthly, one
      grouped pull request each, with a 7-day cooldown. Tauri's crates
      and npm packages get patch updates only, since the two sides must
      move together by hand.
    - Secrets: gitleaks is a quick `check-all.py` step over the history
      (CI checks out every commit for it). `scripts/hooks/pre-commit`
      runs gitleaks over the staged changes, then `check-all.py --quick`.
      It is the pre-commit hook M4 describes, installed with
      `git config core.hooksPath scripts/hooks`. GitHub secret scanning
      with push protection is a repository setting the owner turns on.
- **H8 Lint gates and pinned toolchains.**
  - `cargo clippy --all-targets -- -D warnings` runs in `check-all.py`.
  - A `[lints]` table in `Cargo.toml`, including `unsafe_op_in_unsafe_fn`
    and `clippy::undocumented_unsafe_blocks`, so each `unsafe` block in
    `anomp.rs` says why it is sound.
  - `rust-toolchain.toml` pins §3's Rust version, and `.nvmrc` with
    `engines` pins Node, so local and CI builds match. `doctor.py` (M4)
    reads them.
  - Move to Rust edition 2024 in one separate commit.
  - **Done 2026-10-02 (all but the edition move and `doctor.py`):**
    - `[lints]` in `Cargo.toml` sets `unsafe_op_in_unsafe_fn` and
      `clippy::undocumented_unsafe_blocks` to warn, which `-D warnings`
      makes errors. `check-all.py` runs `cargo clippy --all-targets -- -D
      warnings`.
    - Fixed: 13 unsafe operations in `anomp.rs`'s unsafe fns, now in
      their own blocks with `SAFETY:` comments; a missing one on
      `FolderAccess`'s `Sync`; and 11 small lints (slices from
      references, `as_chunks`, `is_multiple_of`, `Range::contains`,
      `sort_by_key`, …).
    - Allowed:
      - `clippy::type_complexity`, crate-wide: its 15 cases are SQL rows
        read as tuples where they're queried.
      - `drop_non_drop` at one statement in `remote/mod.rs`
        (`drop(library)` on a `State`, which only marks the end of its
        use); the code itself is unchanged.
      - In release builds, `dead_code` on the four `Engine` wrappers
        only the debug-only /dev commands call (H2).
    - `rust-toolchain.toml` (repo root) pins Rust 1.98.1 with clippy and
      rustfmt; CI runs `rustup toolchain install`.
    - `.nvmrc` pins Node 26.10.0, which CI's setup-node reads.
      `package.json` `engines` (`>=26.10.0 <27`) with `engine-strict` in
      `app/.npmrc` makes npm refuse other versions.
    - C++ lint is H21. `doctor.py` (M4) doesn't exist yet; it should read
      both pins when written.
  - **`doctor.py` done 2026-10-03 (Step 8, Part 6)**; its record is under
    M4 below. It reads both pins. The edition move is still open.
- **H9 Logs, panics and diagnostics.** The Rust code reported problems
  with `eprintln!` (54 calls on 2026-10-02, none now), which a bundled app sends nowhere. With
  `panic = "abort"` in the release profile, a panic leaves no trace.
  - Use the `log` facade with `tauri-plugin-log`, writing a rotating file
    of a few MB in the app's log directory.
  - A panic hook writes the message and a backtrace there before the
    abort.
  - The core logs through a C callback (`anomp_set_log_callback`),
    including JUCE's `Logger` and failed assertions.
  - Redact keys, `Authorization` headers and `token=` query parameters.
    Library paths are logged at debug level only.
  - Settings → About gets "Show logs" and "Copy diagnostics": versions,
    OS, output device, and library counts, with no paths or titles.
  - This is needed whatever the crash-reporting decision in §8.1 is, and
    a crash reporter would build on it.
  - **Done 2026-10-02.**
    - `logging.rs`: tauri-plugin-log 2.10.0 (the crate and its npm
      package pinned exactly; MIT or Apache-2.0, `cargo deny` passes)
      writes `ano-mp.log` in the app's log directory
      (`~/Library/Logs/dev.anomp.player`, inside the container when
      sandboxed): 2 MB a file, the current one and two old ones.
      - Release builds write info and above; debug builds debug too, and
        copy it to stderr. Other crates log only warnings.
      - Lines are `<UTC time> <LEVEL> <target>: <message>`. Targets are
        module paths, "core" for the C++ core, "webview" for the page,
        "panic".
    - Redaction, at every level: keys the app holds (`keep_secret`,
      which `metadata::keys` calls for each key it reads or stores, the
      ListenBrainz token included), `Authorization` values, and the
      `token`, `access_token`, `api_key`, `apikey`, `key`, `secret` and
      `password` query parameters.
    - At info and above (the owner's decision): absolute paths become
      `<path>`, and URLs keep only their scheme and host. Messages give
      ids and counts; titles, artists and paths go at debug.
    - Panics: `logging::install_panic_hook`, the first thing `run` does,
      writes the message, where, the thread and a backtrace, and
      flushes before the default hook and the abort. In release builds
      (`strip = true`) the backtrace has addresses only.
    - The core: `anomp_set_log_callback` (`core/src/Log.*`). JUCE's
      `Logger` goes there, and `JUCE_LOG_ASSERTIONS=1` sends failed
      assertions there in every build.
    - The webview's uncaught errors and unhandled rejections
      (`lib/logErrors.ts`, both windows, `log:allow-log`).
    - Settings › About (`AboutOptions.svelte`): the versions, "Show logs"
      (reveals the file in the Finder) and "Copy diagnostics"
      (`diagnostics.rs`). The diagnostics are versions, OS, build,
      whether sandboxed, the output device, library counts, folder states
      (counts), the feature switches and online sources that are on, the
      schema version, the DB's size, its copies, the launch check, and
      the log's last 100 lines at info and above. No paths or titles;
      the owner approved the additions to the list above.
    - The `eprintln!` calls are log calls. 51 were swapped mechanically:
      the "[tag]" prefix dropped (the target names the module), setup
      failures as errors, other failures as warnings, the rest as
      information. Two are in `remote/mod.rs`, for the §8.1 review to see.
      The queue's skipped-track message logs the track's id, and the art
      handler's error its picture's path only at debug.
    - Tests: redaction, scrubbing, levels, timestamps, the log's last
      lines, the panic hook, the diagnostics' text, switches and counts,
      and the core's callback (Catch2 and Rust).
    - **Fixed 2026-10-02 (Step 4): every quit crashed.** In a bundle, each
      quit logged "JUCE Assertion failure in juce_Timer.cpp:99", then
      aborted about 16 s later with a crash report (a pure virtual call in
      `juce::Logger::~Logger` from `exit`).
      - The core's logger (`Log.cpp`'s forwarder, a static) was destroyed
        with the process's statics while still JUCE's current logger; the
        host never clears its callback. JUCE's destructor asserted, and
        logged that through the half-destroyed logger. The forwarder now
        stops being the current logger first.
      - The assertion itself: `AudioEngine`'s JUCE runtime was a member, so
        it shut JUCE down before the engine's `Timer` base let go of JUCE's
        timer thread. The runtime is now the engine's first base
        (`JuceRuntime`).
      - Tests: "C API log may stay set when the process exits" (aborted at
        exit before the fix) and "C API engine shuts JUCE down after its
        timer" (no assertion logged).
    - Left: readable release backtraces need symbols (`strip =
      "debuginfo"`, or a saved dSYM), a size question for §8.2. FFmpeg's
      own log (`av_log`) isn't routed; it would be noisy on damaged files.
- **H10 Library DB safety.** The DB now holds work users can't recreate
  by rescanning: their picks, and soon playlists and history (F1, O8).
  - Before applying migrations, write a copy with `VACUUM INTO`
    (`library.db.pre-<n>`), keeping the last two.
  - Run `PRAGMA quick_check` in the background at launch. On failure,
    offer to restore the last copy, or to rebuild by rescanning after
    exporting what F20 can save.
  - Run `PRAGMA optimize` at exit.
  - Prune `mb_cache` by age and size. 4.5 records that nothing prunes it.
  - **Done 2026-10-02.**
    - Copies: before applying migration n to an existing database,
      `db::open` writes `library.sqlite3.pre-<n>` with `VACUUM INTO` (to
      a `.partial` file, then renamed), and keeps the newest two. (The
      file is `library.sqlite3`, not `library.db`.) A copy that can't be
      written stops the migration: the database stays as it was, and the
      library doesn't open until there is room.
    - The check: `library/recovery.rs` runs `PRAGMA quick_check(20)` on
      a thread of its own at launch (`commands::upkeep`). A database that
      can't be opened at all fails the same way.
    - The offer: on failure the UI shows `DbRepairDialog`, to restore the
      newest copy or to rebuild, or to do neither until the next launch.
      - Either choice is written to `library.recovery.json`, and the app
        restarts; the next launch carries it out before anything opens the
        database.
      - The damaged database (with its WAL files) is moved aside as
        `library.sqlite3.damaged-<unix seconds>`, never deleted.
      - A rebuild first exports what F20 can save to
        `library-rescue-<unix seconds>.json` next to the database; if
        that fails, the dialog asks before going on without it. It starts
        a new database with the old one's folders (bookmarks included)
        and settings, scans every folder, then imports the export.
    - `PRAGMA optimize` at exit (`commands::shutdown`).
    - `mb_cache` is pruned after a passing launch check: copies older
      than 180 days (`cache::MAX_AGE`; the longest freshness is 30 days),
      then the oldest until the rest fit in 64 MB (`cache::MAX_BYTES`).
    - Tests: `db` (a copy before migrating, none for a new database, the
      newest two kept, a copy that can't be written stops the migration),
      `recovery` (a sound and a damaged file, restore, rebuild, a request
      carried out once, a restore names only a file next to the
      database), `cache` (age and size).
    - Left: no database has a copy until migration 010 ships; until then
      a damaged one can only be rebuilt. A copy at other times (say, once
      a week) would close that; not planned.
- **H11 Open files off the main thread.** Opens run synchronously on the
  main thread (Phase 1's known limit: 7 ms for a local MP3). A sleeping
  USB disk, a NAS, or a cloud placeholder (H12) can take seconds, which
  freezes the UI, the media keys and the queue's hand-off arming.
  - Open and prime the reader on a worker thread, holding the folder's
    access (`open_folder`) while it opens. Hand the ready reader to the
    engine on the main thread.
  - The C API gets an asynchronous load with a completion event. The UI
    shows the track as loading, and a timeout fails it with a reason
    that the queue skips as it skips a missing file.
  - **Done 2026-10-02.**
    - Core: `PlayerEngine::loadAsync`/`setNextAsync` open the reader and
      build the track, its read-ahead prefilled, on a thread of their own
      (one per request, so one stuck on a disk that went away doesn't
      hold up the next); `dispatchEvents` hands ready tracks over on the
      message thread, in request order, and reports each request once
      (`onLoadFinished`: loaded, failed or cancelled). A load supersedes
      earlier requests; a next waits for an earlier load; `setNextAsync`
      clears the next track at once, so the current one can't hand off to
      the track being replaced; the synchronous commands cancel requests
      too. `AudioEngine` dispatches as soon as a track is ready
      (`AsyncUpdater`), not at the next 50 ms tick. The engine stays
      main-thread only. As it goes, it waits 2 s for opening threads, then
      lets them finish alone; they own what they share and give up once
      cancelled.
    - C API: `anomp_engine_load_track_async`,
      `anomp_engine_set_next_track_async`, `anomp_engine_cancel_load`, and
      `ANOMP_EVENT_LOAD_FINISHED` (`request`, `result`, `error`, the event's
      new fields). Bound in `anomp.rs` (`Event::LoadFinished`).
    - Rust: `queue/opening.rs` reads the track's row and resolves its
      folder's bookmark on a blocking thread, asks the engine on the main
      thread, holds the folder (`open_folder`) until the engine reports,
      and passes the outcome to the queue. The queue (`model.rs`) gets
      `Opening::Pending` from `Player::load`/`set_next`: the item is
      current and shown as loading (`QueueState::loading`), play, pause and
      seek apply when it's ready, the engine plays on what it had, and
      `check_loads` fails one still opening after `LOAD_TIMEOUT` (20 s)
      with `openTimedOut`, skipped as a missing file is. A
      `folderUnavailable` failure is still "not now". The UI says
      "Opening…" once an open has taken 300 ms.
    - Fixed on the way: the skipped-track toast showed a coded error as
      JSON; it goes through `errorText` now.
    - Tests: Catch2 for the async load with and without read-ahead, a
      failure, cancellation, supersession, the gapless hand-off to a next
      track opened on another thread, callbacks requesting loads, and the
      engine going with files opening (all under ASan and TSan too); the C
      API's requests and events; queue tests for loading, commands while
      loading, the timeout, skips, a folder out of reach, and removing or
      clearing a track as it opens.
    - Open time, 5-minute VBR MP3, 20 opens: `load()` took 21.5–22.2 ms on
      the main thread (Debug and Release alike; most of it the read-ahead
      prefill, which polls in 5 ms sleeps, beyond the reader's 7 ms open).
      `loadAsync()` takes 0.06 ms there (Release; 0.12 ms Debug), and
      21.6 ms from asking to loaded: no slower end to end. The benchmark
      is the hidden test "PlayerEngine open time" (`ANOMP_BENCH_FILE`).
- **H12 Cloud, network and removable folders.**
  - With "Optimize Mac Storage", iCloud Drive keeps dataless placeholder
    files. Reading their tags downloads them, so one scan could download
    a whole library.
    - The scanner checks for dataless files (`SF_DATALESS` in
      `st_flags`), records them without reading them, and says so in
      the scan report.
    - Playing one downloads it first, through H11's asynchronous open.
    - The check lives behind a small core interface next to
      `FolderAccess`.
  - Test SMB folders, and a drive unmounted in the middle of a scan. A
    missing folder keeps its tracks, and so do an empty mount point and
    files that go during a scan (H22).
  - **Done 2026-10-02 (SMB and real placeholders are the owner's checks,
    `docs/step4-checklist.md` § 5).**
    - Core: `FileStatus::isDataless` (`stat()`'s `SF_DATALESS`, which
      doesn't download), next to `FolderAccess`, one file per platform;
      `anomp_file_is_dataless` in the C API.
    - Migration 010: `tracks.dataless`, a file that was a placeholder when
      last seen. The scanner checks new and changed files only: a
      placeholder is recorded without reading it (a track with no tags) or,
      if known, keeps its rows; either is marked and counted in the scan
      report (`dataless`). Every scan checks marked ones again and reads
      them once downloaded, which changes neither size nor time; reading a
      file clears the mark. `ScanOptions::placeholders` lets tests fake the
      check.
    - The analysis worker and embedded art check before reading: the
      worker marks a placeholder and leaves it until a scan finds it
      downloaded; art tries the album's other files and sources.
    - Playing one downloads it through H11's open: `opening` sees the file
      is a placeholder, and the queue shows "Downloading from iCloud…" and
      allows `DOWNLOAD_TIMEOUT` (5 min). Settings › Library shows "N in
      iCloud, not downloaded" per folder.
    - Tests: the scanner with a fake check (recorded unread, checked again,
      read once downloaded; a changed placeholder keeping its tags); a
      drive unmounted between the walk and the reads keeping every track
      (and adding nothing), then catching up; the analysis leaving a
      marked track; the queue's longer timeout; the C API's check.
- **H13 Frontend lint, format and tests.** The frontend has
  `svelte-check` and 5 tests of pure modules. There is no linter or
  formatter.
  - P1: ESLint (`eslint-plugin-svelte`, `typescript-eslint`) and
    Prettier (with its Svelte plugin), through a `format` script with a
    `--check` mode like the others.
    - **Done 2026-10-02.** ESLint 10 (`app/eslint.config.js`: the
      recommended sets of ESLint, typescript-eslint and
      eslint-plugin-svelte, with eslint-config-prettier) and Prettier 3
      (`app/.prettierrc.json`: width 120, the Svelte plugin), each pinned
      exactly in `package.json`.
    - `scripts/format-frontend.py [--check]` runs app/'s own Prettier
      and is a quick `check-all.py` step. `npm run lint` is a full one.
      The tree was reformatted once (77 files).
    - ESLint found 25 problems:
      - Fixed: a `$state` kept in step by an `$effect` became a writable
        `$derived` (`PlaylistView`); an unused `svelte-ignore`
        (`Dialog`).
      - Allowed at each line, with the reason:
        - `prefer-svelte-reactivity` (7): maps and sets that must not be
          reactive (scratch, bookkeeping, copies into `$state.raw`, the
          drop-target registry).
        - `no-useless-mustaches` (2): `{" "}`, a space Svelte would trim
          at the start of a block.
      - Configured, for 13 links: `no-navigation-without-resolve` skips
        links. They are web links that `openLink` opens, and the app has
        no base path.
    - Type-aware rules (`recommendedTypeChecked`) are not on; they would
      need `svelte-kit sync` first and would find more.
  - P2: Vitest for the rune modules (`state/*.svelte.ts`), which plain
    `node --test` can't compile. IPC is faked with
    `@tauri-apps/api/mocks` (`mockIPC`), using payloads recorded from
    the Rust tests.
  - P2: Playwright smoke tests against `vite dev` with the same mocks,
    run in WebKit: browse, search, queue edits, settings. This is §6's
    mitigation for WebKitGTK and WebView2 differences, and they run on
    each OS's CI job from Phases 9–10.
- **H14 Self-test of the sandboxed bundle.** Sandbox mistakes show only
  in a bundle (`CLAUDE.md`), and `tauri-driver` doesn't support macOS.
  - A `--self-test <folder>` flag, compiled into test builds only, runs a
    script inside the ad-hoc-signed sandboxed bundle and exits with a
    status: use a temporary data directory, add the fixtures folder,
    scan, play two tracks through a gapless hand-off, and read the
    covers.
  - CI runs it on the macOS runner after building the bundle.
  - **Done 2026-10-02 (Step 8).** `app/src-tauri/src/self_test.rs`,
    compiled in only with the `self-test` Cargo feature (no new crate):
    `ano-mp --self-test` runs before Tauri starts, with no window and
    none of the app's files. It writes the 21 core fixtures (embedded)
    into the temporary folder (the container's when sandboxed), then:
    the sandbox (`APP_SANDBOX_CONTAINER_ID`), a scratch library with the
    folder added (a security-scoped bookmark) and scanned (every fixture a
    track, none failed), the bookmark resolved, the two tagged fixtures'
    covers through `art::lookup`, every file decoded (`analyse_file`, its
    folder held), and the two shortest tracks through a gapless hand-off
    at volume 0 (`advance_count`). Without an output device playback is
    skipped (`--require-audio` fails instead); `--require-sandbox` fails
    an unsandboxed run. One line per stage; nothing with a path.
  - `scripts/self-test-bundle.py` builds the ad-hoc bundle with the
    feature into `target/self-test` (apart from `build-app.py`'s), runs
    the executable with `--require-sandbox` and checks the lines. It is
    `check-all.py`'s "bundle self-test" step, which runs only on CI (`CI`
    set) or with `--local`, since a sandboxed bundle runs in the app's
    real container. No workflow change: CI's `check-all.py` runs it.
  - Tests: 6 Rust tests (every stage outside the sandbox but playback, the
    sandbox required, a folder that can't be written, the command line,
    the lines, every fixture embedded) and 7 script tests. Run here:
    unsandboxed with `cargo run --features self-test -- --self-test`, and
    in the sandboxed bundle with the owner's go-ahead (the container's
    temporary folder only; left as it was): every stage passed, the
    hand-off after 0.5 s. Not yet seen on CI's runner, which may have no
    output device (playback would be skipped there).
- **H15 Typed IPC end to end.** Phase 6 notes that payloads other than
  the settings are still hand-written in `api.ts`. Derive their
  TypeScript types with ts-rs as the settings do. Then generate the
  command wrappers too (tauri-specta, or a small generator), so a
  renamed command or argument fails `npm run check` instead of failing
  at run time.
  - **Done 2026-10-03 (Step 8).** `app/src-tauri/src/bindings.rs`, a
    test like `settings::bindings`, writes two files and fails while
    either is stale (`ANOMP_WRITE_BINDINGS=1 cargo test bindings`):
    - `app/src/lib/generated/ipc.ts`: 97 types, every payload and event
      the commands send and every type inside them (ts-rs's dependency
      walk from the list in `declare_types`), derived with
      `#[cfg_attr(test, derive(ts_rs::TS))]`. Where the frontend already
      had another name, a test-only `ts(rename)` keeps it (`Item` is
      `QueueItem`, `TrackSummary` is `Track`, and eight more). The two
      prefs types take `ts(optional_fields = nullable)`, since serde
      fills in missing fields.
    - `app/src/lib/generated/commands.ts`: one wrapper per command in
      `generate_handler!` (134), read from its signature with syn (a
      new direct dev-dependency, `=2.0.119`, MIT OR Apache-2.0, already
      in `Cargo.lock` through the macros; the owner chose it over
      tauri-specta, whose 2.0 is still a release candidate). Arguments
      in camelCase, Tauri's own (`AppHandle`, `State`, …) left out,
      `Option<T>` arguments optional, `Result<T, _>` resolving to `T`.
      Each type is resolved through the command file's `use` items to its
      full path, then to its TypeScript name, so two Rust types of one
      name stay apart.
    - `app/src/lib/api.ts` lost its 75 hand-written payload types (801
      lines out, 163 in) and calls `commands.*` instead of `invoke`
      with a string. Two hand-written types had described another Rust
      type than their name said: the album header's `AlbumLink` was
      `SourcedLink`, the signal path panel's `SignalPath` was
      `SignalPathPayload`; the components now name them so.
    - With the owner's agreement, `remote/`'s `RemoteStatus` and
      `RemoteDevice` take the test-only derive; nothing the remote does
      changed.
  - Pass: renaming `library_remove_folder`'s `folder_id` made `npm run
    check` fail ("'folderId' does not exist… Did you mean 'folder'?"),
    and so did renaming the command; both put back.
  - Tests: 8 unit tests of the generator (handler list, camelCase, the
    type mapping, `use` resolution, ambiguous and unknown types), plus the
    up-to-date check; svelte-check 0 errors, ESLint clean.
  - Found in Part D's check-all run: ts-rs visits a type's dependencies in
    an order that changes between builds, so `ipc.ts` came out shuffled
    and the check failed with nothing changed. The declarations are now
    sorted by name.
- **H16 Queue storage and updates.** Phase 3's known limits: the saved
  queue is one JSON value rewritten on every change (350 KB for 50,000
  tracks), and every change sends the whole list (6.6 MB).
  - Store the items as rows (a migration).
  - Send changes as numbered edits (insert, remove, move ranges), which
    the UI applies to its copy. It asks for the whole list only when it
    misses a number.
  - **Done 2026-10-03 (Step 8).**
    - Storage: migration 011 adds `queue_items` (uid, track id, `ord`,
      `original`), with sparse REAL keys for the play order and, while
      shuffled, the order before shuffling, so an insert or a move writes
      only its rows; keys too close to split are renumbered. The old list
      moves out of the `player.queue` setting in the migration itself
      (SQLite's JSON functions; 71 ms for 50,000), which keeps the current
      index, position, repeat, the volume and a new `shuffled` flag (an
      empty queue can be shuffled). `queue/store.rs` mirrors the rows on
      the main thread; restoring keeps the rows' uids and deletes rows
      whose track left the library. Files opened from outside the library
      take a place in the mirror but no row, as before.
    - Updates: the model logs `Edit`s (insert, remove, move, update) for
      the frontend and the store separately; a replace, clear, shuffle or
      unshuffle (and each new pass of shuffle with repeat all) is a reset,
      sent and written whole. `QueueState` carries `listVersion` and
      either `items` or `edits`; `app/src/lib/queueEdits.ts` applies the
      edits to the UI's copy and asks for the whole state when a version
      is missing or an edit doesn't fit. A dragged block is sent as its
      ranges leaving and coming back, and the store keeps its places in
      the order before shuffling. The remote still gets the whole state.
    - `bench.py` (§9.2 M4), with 50,000 items in a file database:
      | | before | after |
      |---|---|---|
      | "Play next" (state and save) | 17.8 ms | 1.6 ms |
      | its state | 8.6 MB | 642 bytes |
      | next track (state and save) | 1.3 ms | 1.4 ms (one small write) |
      | restore: the saved list | a 350 KB JSON value | 5.3 ms of rows |
      Restore as a whole is the tracks' details (`track_infos`, 290 ms on
      the file database, 180 ms in memory), which H16 doesn't change.
      A state no longer walks every item for unavailable ones when there
      are none.
    - `queue/model.rs` changed in under a tenth of its lines, so H19's
      split waits; `move_items` now finds its items through a set.
    - Tests: 5 model tests (edits replayed on 50,000 items, runs of
      removals and updates, resets and the full state's version, the
      original order of an insert while shuffled, restored uids), 6 store
      tests (rows following 50,000 items edit by edit, only touched rows
      written, renumbering, outside files, a block moved while shuffled,
      dropped rows), the migration on a file database with its `pre-11`
      copy, a queue saved before the rows restored shuffled where it was,
      an empty shuffled queue, 5 frontend tests (`queueEdits.test.mjs`).
- **H17 Cover thumbnails.** Phase 3's known limit: art is served at full
  size from a cache in memory only, so it is re-read after each launch,
  and Now Playing artwork is sent at full size.
  - Make thumbnails at two sizes, for lists and for the album header, in
    the on-disk image cache (`metadata/images.rs`), keyed by the
    source's hash.
  - Serve full size only where it is shown full size, and cap Now
    Playing's artwork.
  - Pick a JPEG/PNG decoder and check what it adds to the binary.
  - **Done 2026-10-03 (Step 8).**
    - Decoder (the owner's choice over ImageIO in the core): the `image`
      crate, `=0.25.10`, default features off, JPEG, PNG and WebP. +650 KB
      per architecture in a stripped release binary (about 1.3 MB in the
      universal app); 8 new crates (image, image-webp, zune-jpeg,
      zune-core, moxcms, pxfm, byteorder-lite, quick-error), all under
      licences in `deny.toml`'s list; `THIRD_PARTY_NOTICES` regenerated
      (391 KB). GIF and anything it can't decode is served as it is.
    - `library/thumbs.rs`: 128 px for lists and 512 px for the album
      header, grids, the visualizer and the OS's Now Playing (which no
      longer gets the full picture), JPEG at quality 85 (PNG when the
      picture has transparency), never scaled up. Files in the image
      cache (`metadata/images.rs`, same budget and eviction), named by the
      SHA-256 of the picture they came from, so albums with one cover
      share them.
    - Migration 012 adds `art_thumbs`: which picture an album or track
      shows, where it came from (`art::Origin`: a file in a library folder,
      or a download) and a stamp of it (the file's size and time and its
      folder's time, or the URL). A launch checks the stamp with `stat`
      (which never downloads a cloud placeholder) and serves the
      thumbnail unread. Choosing or downloading a cover deletes the
      album's row; changing the sources deletes all; a scan deletes none,
      since a launch rescan would empty the index every time, and the
      stamps catch changed files and pictures added to a folder.
    - The UI asks `?size=list` or `?size=header` (`artUrl`, `Art.svelte`'s
      `quality`); full size only in the Now Playing view and the "Choose
      cover" dialog. The memory cache keeps each size apart.
    - Made only when the UI shows a cover, on the scheme's blocking
      threads, never in the background; `art::lookup` already passes over
      placeholders (H12).
    - `bench.py`: from a 1,500 px cover (204 KB), a list thumbnail is 11 KB
      made in 11 ms, a header one 76 KB in 21 ms, once per picture.
    - Tests: 5 (scaling and what is left alone, made once and indexed,
      the index served unread and a folder change caught, small pictures
      not indexed, forgetting), and size URLs parsed.
- **H18 Performance budgets.** Phase 7's performance bullet needs
  numbers. Commit budgets and check them with `bench.py` (M4):
  - launch to first paint;
  - memory at rest with 50,000 tracks;
  - scan time;
  - browse and search latency;
  - CPU while playing, with and without the visualizer.
  - **Done 2026-10-03 (Step 8).** The budgets are in
    `scripts/bench-baseline.json` with the baseline, measured on an Apple
    M1 Ultra (Virtual): 6 cores, 12 GB, macOS 26. `scripts/bench.py`
    checks them (M4); the owner agreed each budget, how it is measured,
    the margin and where it runs:

    | Budget | Ceiling | Measured by | Baseline |
    |---|---|---|---|
    | Search, any query, all kinds | 100 ms | Rust benchmark | 49 ms ("k") |
    | Browse, first page of a grouping or of its first group | 100 ms | Rust benchmark (new) | 73 ms (year) |
    | Browse, a whole-library node's tracks in order | 300 ms | Rust benchmark | 198 ms |
    | First scan of 50,000 files | 90 s | Rust benchmark (new: the fixtures as APFS clones) | 9.4 s |
    | Unchanged rescan | 10 s | the same | 0.27 s |
    | Playback through the engine, any file, effects included | ≥ 20× real time | C++ benchmark (new, `bench` preset) | 27× (MP3 with crossfeed and EQ) |
    | The visualizer's analysis | ≥ 50× real time | C++ benchmark (new) | 403× |
    | Launch to first paint | 1.5 s | owner check (`first paint after N ms` in the log) | not yet run |
    | Memory at rest, 50,000 tracks | 400 MB, app and WebKit processes | owner check | not yet run |
    | Whole-app CPU while playing | 5%, 25% with the visualizer | owner check | not yet run |

    - Past its budget, or more than 25% worse than the baseline (and over
      the unit's floor: 2 ms, 0.05 s, 0.1 MB), a result fails. On another
      machine only the budgets count. `--update` rewrites the results.
    - Not in `check-all.py`: it needs a release build of the crate and
      the `bench` preset's JUCE build, and shared runners are too noisy
      for the margin. It is §8.7 step 4, and §9.1's row after scanner,
      browse, search or DB changes.
    - The owner checks are § 6 of `docs/release-smoke-test.md`. The app
      logs `first paint after N ms` once (`diagnostics_first_paint`, two
      frames after the main page mounts; a count only).
    - Found and agreed, not fixed: the core resamples 44.1 kHz files to
      a 48 kHz device at about 30× real time (JUCE's windowed sinc, Phase
      1's design), against 700–1,200× for files at the device rate, so the
      playback budget is 20× for any file, not 100×. Browsing by year or
      genre and a folder's first page scan every track (each album's
      earliest year; `anomp_genres` per row): 56–73 ms, so browse pages
      share search's 100 ms. A stored album year and a genre table
      (migrations) would make them fast when browsing is next worked on.
    - Tests: 13 script tests (`test_bench.py`, over saved output), a
      Rust test of the first-paint timing; the new benchmarks are ignored
      (Rust) or hidden (`[.][bench]`, Catch2), so suite counts don't
      change.
- **H20 Design records out of `PLAN.md`.** The plan is over 3,000 lines
  and grows with every step. It mixes the roadmap with finished design
  notes. Move each finished phase's design and known limits to
  `docs/design/phase-<n>-<name>.md` (`docs/` is empty). Keep status,
  decisions, open steps, the backlogs (§4.6–§4.7, H1–H22) and links
  here. `check-docs.py` (M2) checks the links.
  - **Done 2026-10-03 (Step 8).** Phases 0–6c's design notes, steps as
    built, tests and known limits moved word for word (1,489 lines, checked
    line by line against the old file) to
    `docs/design/phase-{0-toolchain,1-playback-engine,2-metadata-library,3-player-ui,4-online-metadata,5-visualization,6-settings,6b-optional-features,6c-expected-features}.md`,
    each with a heading and a line saying where it came from. Each phase
    here keeps its heading, a link, and its status lines (**Exit**,
    **Checked**, **Not checked**, "Phase N complete"); Phase 4 also keeps
    its goal, the sources table and its design decisions (the owner's
    choice). Phase 7, Phases 8–11, §1–§4 and §6–§9 are unchanged. This
    file went from 4,411 lines to about 2,900.
  - `CLAUDE.md`'s two pointers to Phase 1's design now name its file, and
    it says where finished phases' design goes. The code's "PLAN.md Phase
    N" comments still lead here, then to the file.
  - `scripts/check-docs.py` (M2): every relative link and backquoted repo
    path in `PLAN.md`, `CLAUDE.md`, `README.md` and `docs/` exists (742
    paths and 7 links before the move, none broken). A shortened path
    (`library/access.rs`) matches the tail of a file git lists; built,
    fetched and run-time files and scripts §9.2 plans are skipped or
    listed in its `NOT_IN_REPO`. A quick check-all step. `--counts`
    compares §2's four test counts with the suites (`ctest -N`, `cargo
    test -- --list` less the ignored, `npm test`, pytest's collection), a
    full-run step. Tests: 30 in `test_check_docs.py`, one against the real
    tree.
- **H21 C++ static analysis.** `clang-tidy` with a small set of checks
  (`bugprone-*`, `performance-*`, `concurrency-*`) over `core/src`, in
  `check-all.py` but not `--quick`.
  - **Done 2026-10-03 (Step 8, Part 6).** `scripts/lint-cpp.py` runs
    clang-tidy 22.1.8 from PyPI through uvx, pinned like clang-format
    (the owner's choice over Homebrew's `llvm@22`, whose version drifts
    between machines; PyPI has no 23.x yet). It lints every `core/src`
    file in the debug preset's compile database (which now sets
    `CMAKE_EXPORT_COMPILE_COMMANDS`), in parallel, and prints each
    finding once however many files include its header; JUCE's, TagLib's
    and FFmpeg's headers are left out. On macOS it passes the SDK from
    `xcrun --show-sdk-path`, which Apple's driver otherwise supplies.
    The 20 files built on macOS take about 10 s; the six other platforms'
    files (`*_none.cpp`, `FolderAccess_unsandboxed.cpp`) wait for their
    platforms. "core lint" is a full-run `check-all.py` step after the
    core build. The checks and their exceptions are in `.clang-tidy`.
  - Found 39, none a bug:
    - Turned off in `.clang-tidy`, with their reasons:
      `performance-enum-size` (15; `anomp.h`'s enums are its C ABI),
      `bugprone-easily-swappable-parameters` (14; a naming heuristic)
      and `bugprone-multi-level-implicit-pointer-conversion` (4;
      `av_freep` and CoreAudio's property getters take `void*` by
      design).
    - Fixed: `TagReader`'s two copies of a file's path (now
      references); `SpectrumAnalyser`'s buffer size widened before the
      multiplication; `Log.cpp`'s static forwarder's constructor declared
      `noexcept`; and `PlayerEngine::takeFinishedLoads`'s two identical
      "failed" branches and assignment in a condition, rewritten with no
      change in behaviour (each outcome was already tested: a failed
      open, "No track is loaded", a current and a next track loaded).
    - `concurrency-*` found nothing.
  - Tests: 6 script tests (`test_lint_cpp.py`: the files picked from a
    compile database, the SDK argument per OS, splitting and
    deduplicating diagnostics, the real compile database and
    `.clang-tidy`). ctest stays 115.
- **H22 Missing folders at launch.**
  - **Today:**
    - F8 marks a folder whose bookmark doesn't resolve as unavailable in
      the sidebar and settings, with "Locate…".
    - The launch rescan (F9) fails that folder alone. The failure goes
      into the scan report and `eprintln!`, so nothing tells the user
      at launch.
  - **Gaps:**
    - **A folder that exists but is empty is emptied.**
      - The path of an unmounted network share, or a mount point left
        under `/Volumes`, can still be a directory.
      - `walk_folder` checks only `is_dir()`, so the rescan finds no
        files and removes every track in the folder.
      - That loses everything keyed by those tracks: plays, ratings,
        playlist entries, analysis. `kept_albums` keeps only album and
        artist picks.
    - **A folder moved to the Trash is followed there.** The bookmark
      resolves to its new place, and `open_folder` quietly records the
      Trash path.
    - **The restored queue and resume position** (F17, Phase 3) can
      point at tracks in a missing folder. It isn't defined what plays,
      or how the queue skips, when every item is unavailable.
    - **Nothing re-checks a folder when its drive returns.** The watcher
      leaves out folders that are unavailable at launch and doesn't pick
      them up until the settings change or the app restarts.
    - **The workers can treat missing files as failures.** The
      analysis worker, metadata worker and health view may record a
      missing file as unreadable or broken.
  - **Do:**
    - **Check every folder before the rescan, off the main thread.**
      Classify each as:
      - available;
      - missing (the bookmark doesn't resolve, or the path is gone);
      - empty where it had tracks;
      - in the Trash;
      - permission lost (including the ad-hoc-signing bookmark error in
        `CLAUDE.md`).

      Record the state and its reason with the folder, through
      `library_folders`.
    - **Never let a scan empty a folder.**
      - When a folder that had tracks reads as empty, or would lose most
        of its tracks in one scan, keep the tracks and mark the folder
        "empty, possibly not mounted".
      - Ask before removing them. Test it with a fake empty directory.
    - **Ask about a folder in the Trash.** Offer to locate it again or
      remove it, rather than following it there.
    - **Show it once at launch.** One message that isn't a dialog:
      - "N folders can't be found";
      - each folder's reason;
      - "Locate…", "Remove" and "Keep" buttons.

      It must not block playback of the other folders. When every folder
      is missing (a library on an external drive that isn't plugged in),
      show this in the main view in place of an empty library.
    - **Missing tracks in lists.**
      - Tracks of an unavailable folder stay in browse, search,
        playlists and history, shown as unavailable.
      - Radio, shuffle refills, smart playlists' "play" and Home's
        suggestions skip them.
    - **The queue at launch.**
      - A restored current track that can't be opened becomes "not
        available" with its position kept, and nothing plays by itself.
      - When playing, the queue skips unavailable items, as H11's
        timeout does, without spinning when all of them are unavailable.
      - Media keys and Now Playing show the stopped state.
    - **When drives come back.**
      - Watch for volumes mounting and unmounting (`NSWorkspace`
        notifications behind a core interface next to `FolderAccess`, or
        FSEvents on `/Volumes`).
      - A folder that becomes available is rescanned and watched, and
        the sidebar updates without a restart.
    - **Workers.** Analysis, metadata and health treat tracks of an
      unavailable folder as "not now", not "failed". They retry when the
      folder returns.
    - **Errors.** Use coded errors (`folder_unavailable` with a reason
      code) and `en.json` messages for each state. H9 logs each state
      change.
  - **Tests:**
    - unit tests over the folder states with a fake `FolderAccess`;
    - a scanner test where a folder that had tracks is now an empty
      directory, and keeps them;
    - queue tests for a restored queue with all items unavailable.
  - **In the step 4 checklist:**
    - launch with a USB drive unplugged, then plug it in;
    - an SMB share that isn't mounted;
    - a folder moved to the Trash;
    - a folder deleted.
  - **Done 2026-10-02 (H22a), all but the lists (H22b).**
    - States: `access::FolderState` (available, missing, empty,
      mostlyGone, inTrash, noPermission), with the system's words.
      - `access::check_folder` finds them: it resolves the bookmark
        through the `Bookmarks` trait (`System` over `FolderAccess`; a
        fake in tests), lists the folder, and calls it empty only if it
        had tracks.
      - `availability::FolderStates` keeps them in memory: they are found
        again at each launch, so no migration. `library_folders` returns
        them (`Folder.status`); each change is logged and announced
        (`library-folders`).
    - Never emptied: `scanner::holds` keeps a folder's tracks when a
      scan would remove all of them, or more than half of a folder of 20
      or more. Moves to other folders are matched first. The folder fails
      as `empty` (nothing found) or `mostlyGone`.
      - `library_remove_missing` removes them when the user says so
        (after a confirmation).
      - A file that fails to read because it went during the scan keeps
        its tracks too.
    - The Trash: `open_folder` refuses a bookmark that resolves into a
      Trash (`.Trash`, `.Trashes`, a freedesktop `Trash/files`) and leaves
      the stored path alone; the user locates or removes the folder.
    - At launch: every folder is checked off the main thread, on a
      connection of its own, before the launch rescan. Only folders that
      are there are rescanned and watched.
    - The message (`MissingFolders.svelte`) sits above the main view and
      isn't a dialog: "N folders can't be found", each folder's reason,
      and "Locate…", "Remove…" (or "Remove missing tracks…" for empty and
      mostlyGone) and "Keep" (hidden until the next launch).
      - When every folder is missing, it takes the place of the library
        and Home views.
      - The sidebar and Settings › Library show each folder's reason.
    - The queue: `Queue::set_unavailable_tracks` marks the items of
      unavailable folders (not reported as skipped) whenever folders
      change. When nothing opens, the current item stays with its
      position, and nothing spins. Now Playing shows a restored current
      item that can't be opened as stopped.
    - Drives coming back: the core's `VolumeWatcher`, next to
      `FolderAccess` (NSWorkspace's mount and unmount notifications;
      nothing on iOS or elsewhere yet), through `anomp_volume_watcher_*`,
      hosted on the main thread by `availability::watch_volumes`.
      - Each event checks the folders again.
      - A folder that came back is rescanned and watched again, its
        queue items are tried again, and its tracks analysed.
    - Workers:
      - The analysis worker leaves a folder it can't read alone until a
        scan or the folder's return, storing no failure. It retries rows
        earlier versions stored; the health view and the failure count
        leave those out.
      - The metadata worker doesn't open library files, so it needed no
        change.
    - Errors: `coded::folder_unavailable(path, reason, detail)`, and
      `errorText` shows `error.<code>.<reason>` when the catalogue has it.
    - Tests:
      - folder states with fake bookmarks (each state, the Trash not
        followed, states through errors);
      - `availability` (a folder going, then coming back through empty; a
        held folder staying held; tracks of folders);
      - the scanner (an empty directory keeps its tracks and playlist
        entries, then removes them when told; most of a folder held, half
        not; files moved to another folder not held);
      - the queue (a restored queue with nothing available plays nothing
        and keeps its position, then plays when it comes back; a folder
        going while playing is passed over);
      - Now Playing showing it stopped;
      - the volume watcher (Catch2 and Rust).
  - **Fixed 2026-10-02 (Step 4): a deleted folder read as "no access".**
    Under the sandbox, a deleted folder's security-scoped bookmark fails
    with "isn't in the correct format", the words `access::unresolved`
    took for a rebuilt bundle's bookmark. It now goes by the stored path
    first: nothing there is `missing`; a folder there, or a path the
    sandbox won't stat, is `noPermission`; only otherwise does the error's
    text decide. Test: the case in `a_folder_the_app_may_not_read_has_no_permission`.
    Checked in the bundle for a folder inside the container; one outside
    it is in the owner's list.
  - **Done 2026-10-02 (H22b):** tracks of unavailable folders shown as
    such in browse, search, playlists and history, and left out by radio
    (and its refills, which is what "shuffle refills" meant: shuffle only
    reorders the queue), smart playlists' "play" and Home's suggestions.
    - Which folders: `FolderStates::unreadable()` (every unavailable state
      but `mostlyGone`, whose remaining files still play), through
      `availability::unreadable(app)`. Their ids are bound as JSON and read
      with `json_each` (`availability::json_ids`), never formatted in.
    - Left out:
      - `radio::picks` (start radio and the refills);
      - `smart::track_ids`, which `playlists_track_ids` (play, add to queue)
        uses: a limited smart playlist plays its limit of tracks that open.
        Lists of tracks keep them; the queue passes over them. M3U export
        keeps them;
      - Home: recently added, released on this day, the highlights
        (forgotten, a year ago, never played) and "More in this genre"
        leave out albums none of whose tracks can be opened
        (`discover::playable_album`). Recently added dates an album by
        its readable tracks. Home and "More in this genre" reload when the
        set of unreadable folders changes (`library.unreadableKey`).
    - Shown: `TrackSummary` has `folder_id` (so browse, search, playlists
      and favourites have it), recently played's tracks `folder_id`, and a
      top entry its tracks' `folder_ids`. The UI works out the unreadable
      folders from `library.folders` (`lib/folders.ts`, the same rule as
      Rust) and dims a track (`TrackText`), a history entry or a Home
      card whose folders are all unreadable, with the folder's short reason
      (`folderShort.*`, the owner's choice: no new wording) as its tooltip.
      Hearts and stars stay usable. The sidebar's map of short reasons
      moved to `lib/folders.ts` (`FOLDER_SHORT`).
    - Tests: radio, a smart playlist's play (the limit filled from readable
      folders, and conditions joined by OR still needing the folder),
      recently added, on this day and more in genre, the highlights, the
      history's folder ids (`cargo test`, 4 new), and `tests/folders.test.mjs`
      (4).

## Scripts (§9.2 M1–M6) as specified

- [x] M1 Test harness (2026-10-02). `scripts/test-python.py` runs a pinned pytest
  through uvx (`uvx --from pytest==<version> pytest scripts/tests`), and
  `format-python.py` gains `ruff check` (lint) next to `ruff format`. Add
  tests for the existing scripts' pure logic (`format-cpp.py`'s file
  selection and batching; `make-test-fixtures.py`'s signal matching
  `TestSignal.h`'s constants).
- [ ] M2 Repo checks, each a read-only script that lists every problem it
  finds. `check-c-api.py`, `check-sources.py` and `check-migrations.py`
  are done (2026-10-02); the FTS warning covers migrations after 009,
  and a `-- fts:` comment acknowledges one that needs no trigger change.
  - `check-c-api.py`: parses the functions declared in `anomp.h` and the
    `extern "C"` block in `anomp.rs`; fails on any function missing from
    either side, and on declarations whose parameter counts differ.
  - `check-sources.py`: every `.cpp`/`.mm` under `core/src` and
    `core/tests` is listed in its `CMakeLists.txt`, and every listed file
    exists.
  - `check-migrations.py`: files in `library/migrations/` numbered from
    001 with no gaps, each listed in `MIGRATIONS` in order, and none that
    existed at the latest release tag changed since (`git diff` against
    the tag; before the first release there are no tags and this part is
    skipped). Warns when a migration alters `tracks`, `artists` or
    `albums` without touching the FTS triggers.
  - `sync-ffmpeg-frameworks.py`: rewrites `bundle.macOS.frameworks` from
    the dylibs in `third_party/ffmpeg/macos-universal/lib`; `--check`
    compares only.
  - `check-docs.py`: every backquoted repo path in `PLAN.md`, `CLAUDE.md`
    and `README.md` exists; with `--counts`, compares §2's test counts
    with `ctest --preset debug -N` and `cargo test -- --list` (needs
    builds, so not in the quick check). **Done 2026-10-03** (H20's entry),
    `docs/` and relative links included; `npm test`'s and the scripts'
    counts too.
- [x] M3 Fixture tools (with 4.8, which adds sources and their fixtures):
  - `record-fixtures.py`: a manifest next to the metadata fixtures lists
    each file's URL and trim rule (which JSON fields to keep, how many
    list items). It fetches at one request a second with the app's
    `User-Agent`, trims, writes, and with `--check` shows the diff
    instead. Re-record the existing fixtures with it once and confirm
    `cargo test` still passes. Tests: trimming over saved raw responses,
    the rate limiting with a fake clock, and the manifest covering every
    committed file.
  - `make-test-fixtures.py`: pass fixed stream serials to `oggenc`
    (`--serial`), so regenerating changes no file unless the signal or
    encoders changed (the Vorbis fixtures change once when this lands);
    add `--only NAME`; print each fixture's length for the tests' table.
  - **Done 2026-10-03 (Step 8, Part 6).**
    - `make-test-fixtures.py`: each Vorbis file gets a serial made from
      its name (`oggenc --serial`, CRC-32 of the name); ffmpeg's
      `-bitexact` already fixed the Opus files'. The two Vorbis fixtures
      changed once (only their serials; ctest passed as before), and a
      second run left every fixture byte for byte the same. `--only NAME`
      (repeatable) remakes those alone. Each line gives the file's bytes,
      its length as Homebrew's ffmpeg decodes it, and the signal's
      length. Those match the tests' table but for `vorbis-44k.ogg`:
      Homebrew's ffmpeg gives 22,400 samples where the pinned reader and
      the table have 22,371, so the docstring says to check a length
      against the pinned FFmpeg.
    - `record-fixtures.py` with `metadata/fixtures/manifest.json`: 19
      fetched files (URL and rules) and two made by hand (the Discogs
      search, which needs a token, and the 8×8 JPEG). A rule's `select`
      keeps the listed list items, in the listed order, by identifying
      fields (`id`; `id` and `role` for Discogs' credits, where one person
      has two; `type` and `url.id` for links; `type` and `value` for
      identifiers); `keep` is a nested object of the keys to keep; `set`
      replaces values (the release-group count to match its 14 groups,
      and Wikipedia's extracts, whose stand-in text the tests use since
      the articles are CC BY-SA). The rules were derived from each
      committed file against its live response, so they reproduce the
      hand trimming. It fetches at one request a second with the app's
      `User-Agent` (version from `CMakeLists.txt`, contact from
      `http.rs`), writes two-space-indented JSON, and with `--check`
      prints the diff and writes nothing. Each run checks the manifest
      covers every file.
    - Re-recorded once (2026-10-03): nothing selected had gone upstream.
      Besides the formatting, only timestamps (`created`, `touched`), the
      In Rainbows search's total (602,015 to 53: the earlier recording
      used a broader query) and Wikidata's unused `sitelinks.enwiki.url`
      changed. One test then failed: it edited a release's JSON as text
      (`"front":true`), which the new formatting broke; it now edits the
      parsed JSON (`coverartarchive.rs`). `cargo test` passed after.
    - Tests: 12 in `test_record_fixtures.py` (selection, keep, set, the
      output format, the throttle with a fake clock, recording with a
      fake fetch, coverage, the real manifest, and every recorded file
      unchanged by its own rule) and 5 more in
      `test_make_test_fixtures.py`.
- [x] M4 CI entry point (with Phase 7's CI). `check-all.py` is done
  (2026-10-02), with `.github/workflows/ci.yml` running it, and
  `scripts/hooks/pre-commit` runs its `--quick` mode (H7):
  - `check-all.py`: runs every formatter in `--check` mode, the M2
    checks and the Python tests, then (unless `--quick`) the C++, Rust
    and frontend builds and tests. CI calls this and nothing else, so a
    local run matches CI; `--quick` is the pre-commit hook.
  - `doctor.py`: checks the tools in §3 are installed at the minimum
    versions, that `third_party/ffmpeg/<platform>/BUILD_INFO` matches
    `build-ffmpeg.sh`, and on macOS that the Command Line Tools are
    selected. Prints what to install. Written for every OS from the
    start, since Phases 8–10 need it most.
  - `bench.py`: runs the ignored benchmarks in release mode, parses their
    timings and compares them with a committed baseline, failing past a
    set margin; `--update` rewrites the baseline. **Done 2026-10-03**
    (H18's entry): the Rust benchmarks and the core's (`bench` preset),
    H18's budgets, 13 tests.
  - **`doctor.py` done 2026-10-03 (Step 8, Part 6).**
    - Checks, each with an install hint for macOS, Linux and Windows:
      Python (3.11, what the scripts use: `datetime.UTC`, `tomllib`), git,
      uv, gitleaks, Node and npm; then CMake (at `CMakeLists.txt`'s
      `cmake_minimum_required`), Ninja, nasm, pkg-config, rustup with
      `rust-toolchain.toml`'s toolchain and this OS's targets, and
      cargo-deny at the version `ci.yml` installs. Node must be inside
      `package.json`'s `engines` and should be `.nvmrc`'s (another patch
      only warns). On macOS: the selected developer directory (the
      Command Line Tools or Xcode) with an SDK, and `llvm@22` for the
      fuzzers. Then FFmpeg's `BUILD_INFO` against what `build-ffmpeg.sh`
      would build now, from its new `--info` flag (the same text it
      compares before skipping a build), so no bash is parsed in Python.
      The edit changes CI's FFmpeg cache key (the script's hash), so
      CI's next run builds FFmpeg once.
      Checks that don't apply to an OS (llvm@22, the developer directory,
      an FFmpeg build Phases 9–10 add) are left out there.
    - A missing or too-old tool fails; a difference that still works
      (another Node patch, a Rust toolchain or target rustup fetches on
      first use, a newer cargo-deny) warns. It prints each tool's line,
      then each fix once.
    - `check-all.py` runs it first as "tools", in both modes (the owner's
      choice): `--quick` checks only what the quick steps need, a full
      run everything. A step can now have a separate command for the
      full run (`Step.full_command`).
    - `ruff.toml` now sets `target-version = "py311"`, which made ruff
      treat `tomllib` as the standard library, and moved `bench.py` to
      `datetime.UTC`.
    - Tests: 17 in `test_doctor.py` (versions, each check over fake
      command output for each OS, the report, the real pins) and one in
      `test_check_all.py`.
- [x] M5 Dependency tools (with Phase 7):
  - `check-pins.py`: reads every pin (JUCE, Catch2, TagLib, FFmpeg,
    clang-format, ruff) from its file and asks upstream for the latest
    release (GitHub releases, ffmpeg.org, PyPI); also summarizes
    `cargo update --dry-run` and `npm outdated`. Report only.
  - `bump-pin.py NAME VERSION`: downloads the release, computes its
    SHA-256 (and for FFmpeg verifies the GPG signature against the key
    recorded in `build-ffmpeg.sh`), rewrites the pin in place and prints
    the rebuild and test commands. Tests rewrite copies of the real files.
  - `audit-deps.py`: runs `cargo deny check` (H7) and `npm audit`, and
    checks the FFmpeg pin against ffmpeg.org's security page; a scheduled
    weekly CI job runs it with `check-pins.py`.
  - **Done 2026-10-03 (Step 8, Part 6).**
    - `check-pins.py` reads ten pins from their files: JUCE, Catch2,
      TagLib, Signalsmith Stretch and its FFT library (`linear`), FFmpeg,
      and clang-format, clang-tidy, ruff and pytest. GitHub projects'
      latest releases come from `git ls-remote --tags` (no API token or
      rate limit), FFmpeg's from ffmpeg.org's release listing, the tools'
      from PyPI; pre-releases are left out. Then `cargo update --dry-run`
      and `npm outdated`. Report only; `--summary FILE` appends it as
      Markdown (the workflow's run summary). A failed lookup is reported
      in its row.
    - Its first run: JUCE 9.0.3, clang-format 23.1.2 and ruff 0.16.10
      are out; 15 crates (Tauri's 2.7.x/2.12.x patches among them) and 9
      npm packages, including SvelteKit 3, adapter-static 4 and
      TypeScript 7 (majors). None was moved (the monthly update, §9.1).
    - `bump-pin.py NAME VERSION` uses the same table. JUCE and Catch2 go
      to GitHub's archive of the commit the tag names (the peeled commit
      for an annotated tag), TagLib and Signalsmith to their tarballs;
      each is downloaded and hashed, and the `anomp_fetch_declare` block
      (and JUCE's and Catch2's version comment) rewritten. FFmpeg's
      tarball is checked with gpg in a throwaway keyring holding only
      ffmpeg.org's published key, and must be signed by the fingerprint
      `build-ffmpeg.sh` records. PyPI pins only need the release to exist.
      It prints the rebuild and test commands and where the docs name
      the old version. `--check` downloads and verifies, prints the diff
      and writes nothing. Checked against the real upstreams: Catch2
      3.16.0 and FFmpeg 9.0.2 (signature included) reproduce the
      committed pins exactly, and `juce 9.0.3 --check` gives the expected
      diff.
    - `audit-deps.py`: `cargo deny check`, `npm audit`, and FFmpeg's pin
      against ffmpeg.org/security.html, which lists fixes under a heading
      per release: a newer release of the pinned branch listing fixes
      fails; a newer branch's, or git master's unreleased fixes, are
      notes. Each check runs even after one fails.
    - Found: `npm audit` reports GHSA-pxg6-pf52-xh8x (low): `cookie`
      before 0.7.0, through `@sveltejs/kit` 2 and `adapter-static` 3,
      both dev dependencies. Kit's cookie parsing runs only in a SvelteKit
      server; adapter-static prerenders the app, so the webview never
      runs it. The fix is SvelteKit 3, a major update, so it is allowed
      with that reason in `audit-deps.py`'s `NPM_IGNORED` (npm has no
      ignore list), which also notes an allowed advisory no longer
      reported. FFmpeg 9.0.2 is current; six fixes are in master only.
    - `.github/workflows/audit.yml`: Mondays 05:00 UTC (before the
      fuzzing run) and by hand, on `ubuntu-24.04` (nothing in it is
      macOS-specific). It installs Rust, Node (from `.nvmrc`), cargo-deny
      at `ci.yml`'s version (cached) and the npm packages, then runs
      `check-pins.py --summary "$GITHUB_STEP_SUMMARY"` and
      `audit-deps.py`. Actions pinned by the commits `git ls-remote`
      gives for checkout v5.1.0, setup-node v5.0.0 and cache v4.3.0 (the
      ones `ci.yml` uses; all lightweight tags). It hasn't run yet.
    - Tests (31, no network: fetches, `git ls-remote`, downloads and gpg
      are faked): `test_check_pins.py` (tags, listings, rows, the
      summaries and reports, every real pin read), `test_bump_pin.py`
      (each kind of pin rewritten in copies of the real files, a good,
      foreign or bad signature, the docs' mentions) and
      `test_audit_deps.py` (the security page, npm's advisories, every
      workflow action pinned by commit, the audit workflow's scripts and
      cargo-deny version).
- [ ] M6 Release tools (with §8.2). Done 2026-10-02 except `release.py`'s
  updater manifest (after the updater, §8.2): `version.py`,
  `make-notices.py`, `release.py` and `check-signing.py`, plus
  `build-app.py`, `check-bundle.py` and `notarize.py` (§8.3), with 39
  tests in `test_version.py`, `test_make_notices.py` and
  `test_release_tools.py`.
  - `version.py`: `--check` fails unless `CMakeLists.txt`, `Cargo.toml`,
    `tauri.conf.json` and `package.json` agree (`anomp_version()` is
    generated from CMake by then); `version.py 0.2.0` sets them all.
  - `make-notices.py`: builds `THIRD_PARTY_NOTICES` from JUCE, FFmpeg
    (licence, version and configure flags from `BUILD_INFO`, source
    link), TagLib, `cargo-about` output and the npm licences; fails on an
    unknown licence.
  - `release.py`: checksums for the built artifacts, the Tauri updater
    manifest, and release notes cut from `CHANGELOG.md`.
  - `check-signing.py` (with §8.3): lists the signing certificates'
    expiry dates from the keychain and warns within 60 days.
