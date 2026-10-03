# Running a bundle check safely

Moved from `CLAUDE.md` on 2026-10-03, unchanged but for this heading.
`CLAUDE.md` keeps the rule: ask the owner before running one.

The bundle is sandboxed, so it uses the
real container (`~/Library/Containers/dev.anomp.player`), which holds the
owner's library, settings and bookmarks: ask before running one. macOS
won't let another process move the container or copy all of it, so with
the app quit, copy its `Data/Library` somewhere safe, then move the app's
own files out of it (`Application Support/dev.anomp.player`,
`Preferences/dev.anomp.player.plist`, `Caches/dev.anomp.player`,
`Caches/WebKit`, `WebKit`, `Logs/dev.anomp.player`) and `killall
cfprefsd`. Afterwards delete the scratch files, move the real ones back
and diff them against the copy. Never change the bundle identifier to
avoid this (an owner decision, PLAN.md §8.1). Run the executable directly
(`…/ano-mp.app/Contents/MacOS/ano-mp`): it is still sandboxed and gets
your environment, so a temporary probe started from `setup` can read its
stage from a variable. The sandbox can always read its own container, so
a scratch library written there (the fixtures, embedded with
`include_bytes!`) needs no open panel; folders elsewhere need the owner.
The log is `Data/Library/Logs/dev.anomp.player/ano-mp.log` in the
container; a crash leaves a report in `~/Library/Logs/DiagnosticReports`
(build with `CARGO_PROFILE_RELEASE_STRIP=none
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only` to get symbols in it).
Remove every probe afterwards.
