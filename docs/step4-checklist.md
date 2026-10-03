# Phase 7 Step 4: exit checks in a sandboxed bundle

The exit lists of Phases 4, 5, 6, 6b and 6c, and what Step 3 left for a
sandboxed bundle (`PLAN.md` Phase 7, "Order of work"), in one list ordered
by risk: sandbox-only behaviour first, then playback by ear, then the rest
of the app, then the visualizers. Results go in `PLAN.md` Step 4 and in
each item's entry there.

Who:
- **probe**: Claude, with a temporary probe compiled into the bundle and
  removed afterwards (results in `ano-mp.log`);
- **you**: by ear, by eye, or with hardware.

Before any bundle run, protect the real container and use a scratch
library ("Running a bundle check safely" in `CLAUDE.md`).

## 1. Sandbox-only behaviour

- [x] **Quitting** (probe, found while probing). *Failed, then passed
  2026-10-02: every quit aborted (a crash report each time) since H9's
  core logger, which JUCE destroyed while still its current logger; and
  JUCE's timer thread outlived its message manager. Both fixed in the
  core with tests (PLAN.md H9). Check that quitting from the menu leaves
  no crash report in `~/Library/Logs/DiagnosticReports`.*

### Folders and bookmarks

- [x] **Scan in the sandbox** (probe). *Passed 2026-10-02: 21 tracks, none failed, both covers (463-byte PNGs) served; then a gapless hand-off between the two shortest tracks (advance count +1, the queue on item 2).* A folder of `core/tests/fixtures`
  inside the container is added (a security-scoped bookmark made under
  the sandbox), scanned and its covers read. Pass: every fixture is a
  track, nothing fails, the tagged fixtures' covers are served.
- [x] **Folder states with real bookmarks** (probe), each followed by
  putting the folder back. *Passed 2026-10-02 after a fix: a deleted
  folder read as `noPermission` (its bookmark fails with "isn't in the
  correct format" under the sandbox, as a rebuilt bundle's does); it now
  goes by the stored path first (PLAN.md H22).*
  - its files removed, the folder left: `empty`, tracks kept;
  - the folder moved into a `.Trash`: `inTrash`, the stored path not
    followed there, tracks kept;
  - the folder deleted: `missing`, tracks kept;
  - back again: `available`, rescanned.
- [x] **H22b in the bundle** (probe). *Passed 2026-10-02; a queue of
  the unreadable folder's tracks played nothing and didn't spin.* With the folder unreadable: radio and
  a smart playlist's "play" leave its tracks out; browse still lists
  them, each with its folder id.
- [ ] **A folder dropped from the Finder** (you). Drag a folder of music
  onto the window. Pass: it becomes a library folder and scans; after a
  relaunch it is still readable (its bookmark resolves).
- [ ] **Open With and double-click** (you). Open an audio file outside the
  library with "Open With › ano-mp", and double-click an `.m3u8` whose
  tracks are in the library. Pass: the file plays without being added; the
  playlist is imported as a new playlist.
- [ ] **A USB drive unplugged at launch, then plugged in** (you). Add a
  folder on a USB stick, quit, unplug, launch. Pass: the launch message
  says the folder can't be found, with its reason; the sidebar shows
  "not found"; its tracks stay, dimmed in lists, with "not found" as their
  tooltip; queue items of it are skipped. Plug it in: without a restart
  the message goes, the folder rescans, the tracks are no longer dimmed.
  The log has "a volume was mounted".
- [ ] **An SMB share not mounted** (you). Add a folder on a share,
  quit, eject the share, launch. Pass: as the USB drive (`missing`, or
  `empty` if the mount point stays), tracks kept.
- [ ] **A folder moved to the Trash** (you). Add a folder, quit, move it to
  the Trash, launch. Pass: "in the Trash", with "Locate…" and "Remove…";
  the folder is not followed into the Trash.
- [ ] **A folder deleted** (you). As above, deleted. Pass: "not found",
  tracks kept until "Remove…".
- [ ] **A rebuilt bundle's folders** (you, after a rebuild). Folders added
  in the previous build, outside the container. Pass: each reads "no
  access" (`noPermission`), keeps its tracks, and "Locate…" brings it
  back.

### Step 3's items

- [x] **The log in the container** (probe). *Passed 2026-10-02: 158
  lines, every one in the format, no paths, file names or debug lines.* Pass:
  `~/Library/Containers/dev.anomp.player/Data/Library/Logs/dev.anomp.player/ano-mp.log`
  exists, lines are `<UTC time> <LEVEL> <target>: <message>`, nothing at
  info or above has a path, title or URL path.
- [ ] **"Show logs"** (you). Settings › About › Show logs. Pass: the Finder
  opens with `ano-mp.log` selected.
- [ ] **"Copy diagnostics"** (you; `navigator.clipboard` needs a click).
  Pass: the clipboard holds the diagnostics, with "sandboxed: yes", the
  folder states as counts, and no paths or titles.
- [x] **A panic in a release build** (probe). *Passed 2026-10-02 (the
  backtrace has addresses only, as H9 says).* Pass: the log ends with the
  panic's message, where, the thread and a backtrace, and the app exits.
- [ ] **The database repair offer** (probe for the mechanics, you for the
  dialog). *Mechanics passed 2026-10-02: restore and rebuild each
  restarted the app, kept the damaged files, and came back with 21
  tracks; the rebuild kept the folders and settings. The dialog itself is
  yours.* A damaged copy of the scratch library, with a copy to restore.
  Pass: the launch check fails and the repair dialog shows; "Restore"
  restarts the app with the copy in place and the damaged file kept as
  `library.sqlite3.damaged-<secs>`; damaged again, "Rebuild" restarts,
  keeps the folders and settings, and rescans.

### Network

- [ ] **The LAN remote's local-network prompt** (you). Turn on the remote
  (Settings › Features), pair a phone. Pass: macOS asks for local-network
  access once, with the `NSLocalNetworkUsageDescription` text; after
  allowing, the phone pairs and controls playback. Note what happens
  after denying (System Settings › Privacy › Local Network): the app must
  keep working.
- [ ] **The keychain in the sandbox** (you). Enter a Discogs token and a
  ListenBrainz token. Pass: each is accepted, still there after a
  relaunch, and never in the log.

## 2. Playback by ear (you)

- [ ] **Gapless**: a gapless album (a live album or a continuous mix) plays
  through with no gap or click at each track change.
- [ ] **Cue sheets**: a single-file album with a cue sheet lists its
  tracks and plays them gaplessly; a hidden track's gap is skipped.
- [ ] **Chapters**: an audiobook's chapters are listed; resuming it after a
  relaunch starts where it stopped; the media keys work after a relaunch.
- [ ] **Crossfade**: on, tracks overlap by the set time; skipping during a
  fade moves cleanly to the next track.
- [ ] **Equaliser**: a preset is audible and matches its curve; switching
  profiles with headphones plugged and unplugged follows the output.
- [ ] **ReplayGain**: tagged music plays at even loudness in track and
  album mode; a mode change mid-track applies without a jump or click;
  after the loudness analysis, untagged tracks are levelled too.
- [ ] **Output devices**: switching device and buffer size in Settings
  plays on the new one; unplugging the chosen device falls back without
  hanging, and replugging it works.
- [ ] **Sample-rate matching**: on a device with several rates, the signal
  path shows the file's rate and the device follows it.
- [ ] **Crossfeed**: with headphones plugged and unplugged, as set.
- [ ] **Practice mode**: 50% and 150% sound right (Signalsmith's quality);
  a tight A–B loop is seamless.
- [ ] **Segues in shuffle**: a live album's segues stay together.

## 3. The rest of the app (you)

- [ ] **Online metadata (Phase 4)**: tagged and untagged albums get details
  and covers from MusicBrainz and the Cover Art Archive; reordering and
  turning off sources takes effect; "Choose cover" and "Find details"
  pick another; with the network off the app shows what it cached; a
  Discogs match shows its credit linked to the release.
- [ ] **Artist pages**: facts, a Wikipedia biography with its credit and
  licence, and the missing releases.
- [ ] **Library**: the welcome page on a fresh library; a compilation and a
  duet in the browser; watching while files are copied in, renamed and
  moved; dragging within and between lists.
- [ ] **Playlists**: M3U8 round trips with Music or VLC, including a
  legacy code page.
- [ ] **Shell**: every menu item and shortcut, the Dock menu, the menu-bar
  item, the mini player over a full-screen app, notifications with covers.
- [ ] **Features (6b)**: waveforms; classical works on album pages; each
  playback preference; plays counting and ListenBrainz receiving them;
  radio's picks; lyrics from an `.lrc` and from SYLT.
- [ ] **Accessibility**: VoiceOver over the main views; the layouts at
  larger text sizes.
- [ ] **Export and import**: export on one Mac, import on another after a
  scan.

## 4. Visualizers (you)

- [ ] Frames arrive smoothly with music playing (no stutter), in each
  visualization.
- [ ] The beat detector follows real music; the cover wall flips on beats.
- [ ] Full screen (F, or double-click) and V to cycle.
- [ ] CPU: Activity Monitor while each runs at Retina size, the wall with
  a large library.
- [ ] The flash guard on a strobing track.

## 5. Step 5: opening files off the main thread, cloud and network folders

What Step 5 (`PLAN.md` H11, H12) changed that only real disks, shares and
iCloud show. The probe checks wait for your go-ahead (a sandboxed bundle
uses the real container).

- [x] **A folder held open while a file opens off the main thread**
  (probe). *Passed 2026-10-02: the 21 fixtures played through with 20
  gapless hand-offs, every load handed to the engine with its folder
  held, none failed; then 9 skips back to back (paused), 5 superseded
  while opening and cancelled, no folder left held; no warning or error
  in the log, no crash report on quitting.* In the sandboxed bundle, play a scratch library's tracks: the
  bookmark is resolved on a blocking thread and held until the engine's
  opening thread reports the file. Pass: every track plays, gapless
  hand-offs included; nothing in the log says a file couldn't be opened.
- [ ] **A sleeping disk** (you). A library folder on a USB hard disk (one
  that spins down). Let it sleep, then play a track from it. Pass: the UI
  and the media keys keep responding while it wakes; the now-playing bar
  says "Opening…" after a moment, then plays. A disk that takes more than
  20 s skips the track with "The file took more than 20 s to open".
- [ ] **An SMB folder** (you). Add a folder on a network share and scan
  it. Pass: it scans; tracks play (and hand off gaplessly) from it. With
  the share's server switched off while it plays: the UI doesn't freeze,
  the next track is skipped after 20 s, and the folder's tracks stay.
  Ejecting the share during a scan keeps its tracks (H22).
- [ ] **iCloud placeholders** (you). With iCloud Drive's "Optimize Mac
  Storage" on, a library folder in iCloud Drive with some files evicted
  (Finder › Remove Download). Scan it. Pass: the scan doesn't download
  them (no download progress in the Finder); Settings › Library shows
  "N in iCloud, not downloaded" for new ones; the log says "cloud
  placeholders left unread". Play one: the bar says "Downloading from
  iCloud…", then it plays. Rescan after it played: its tags show, and
  the count goes down. Turn on file analysis: placeholders aren't
  downloaded by it.
- [ ] **Folder watching after a download** (you). After playing a
  placeholder, note whether its tags appear without a manual rescan (the
  watcher may or may not see a download as a change).
