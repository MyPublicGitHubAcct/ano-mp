# Release smoke test on a clean Mac

`PLAN.md` §8.3's "verify on a clean Mac" and §8.7 step 6, for each
release's DMG before it is published. Yours to run: by hand, on a Mac (or
a macOS virtual machine) that has never had the app, signed in as a user
who has never run it. Results go in the release's entry in `PLAN.md`
(the Order of work until the first release, §8.7 after it).

The names below are today's (`ano-mp`, `dev.anomp.player`); they change
with the rename (`docs/release-decisions.md` § 1).

Before starting:
- The release's files from the draft GitHub Release (or the trial run's
  artifact): `ano-mp_<version>_macos-universal.dmg`, `.app.zip` and
  `SHA256SUMS`, downloaded with a browser, so they carry the quarantine
  flag that Gatekeeper checks.
- A folder of test music: an MP3 album, a FLAC album, an AAC (`.m4a`)
  album, and a gapless album (a live album or a continuous mix) in any of
  them. A copy, not your only one.
- Two artists in the test music whose albums share a genre in their
  tags and came out within two years of each other (on the same label
  is better still), so More Like This and Similar artists have something
  to find: a shared genre alone is only just enough.
- Media keys: a keyboard with them, or AirPods/headphones with a button.
- An empty folder for recordings (section 5), for example a new folder
  in Documents.
- For the update (section 6): the previous release installed and run
  once, with this test music in its library. The first release has none:
  skip section 6 for it.

## 1. Download and first launch

- [ ] **Checksums.** In the download folder, `shasum -a 256 -c
  SHA256SUMS`. Pass: both files `OK`.
- [ ] **Gatekeeper on the DMG.** `spctl --assess --type open --context
  context:primary-signature -v <file>.dmg`. Pass: `accepted`, `source=Notarized
  Developer ID`.
- [ ] **Install.** Open the DMG, drag the app to Applications, eject.
  Pass: the DMG window shows the app and the Applications link; no
  warning while copying.
- [ ] **First launch.** Double-click the app in Applications. Pass: one
  dialog, "… downloaded from the Internet. Are you sure you want to open
  it?", naming the publisher; no "can't be checked for malicious
  software" and no "damaged". `spctl --assess -v
  /Applications/ano-mp.app` says `accepted`, `source=Notarized Developer
  ID`.
- [ ] **No crash, a log.** The window opens to an empty library. Pass: no
  crash report in `~/Library/Logs/DiagnosticReports`; Settings › About
  shows the release's version and the third-party notices open.

## 2. Library

- [ ] **Import.** Add the test music folder (the open panel). Pass: it
  scans; every album appears with its tracks; embedded and folder covers
  show.
- [ ] **After a relaunch.** Quit and reopen. Pass: the folder is still
  readable (not "no access"); the library is unchanged; the queue is
  where it was.

## 3. Playback

- [ ] **MP3, FLAC and AAC.** Play a track of each. Pass: each plays, the
  now-playing bar shows its title, artist and cover.
- [ ] **Seeking.** In each format, seek near the end, back to the start
  and into the middle. Pass: playback resumes at once from the new
  place, with no noise.
- [ ] **A gapless album.** Play it from the start through two or three
  track changes. Pass: no gap or click at any change.
- [ ] **Media keys.** Play/pause, next and previous from the keyboard
  (and the headphones), with another app in front. Pass: each works;
  Control Center's Now Playing shows the track and cover.

## 4. Online

- [ ] **A MusicBrainz lookup.** With Online sources on, open an album's
  "Find details". Pass: releases are found and one can be chosen; the
  album's details and cover follow (a cover from the Cover Art Archive
  if it had none).
- [ ] **The log.** Settings › Show logs. Pass: the log opens in the
  Finder and has no file paths, titles or keys at info level.

## 5. Themes, effects, recording and recommendations

What the first release adds beyond playing (`PLAN.md` Phase 7b). Some of
it starts off: each check says when to turn a switch on in Settings ›
Features first.

- [ ] **Themes.** In Settings › Appearance, under **Themes**, pick
  **Midnight**, then **Paper**, then click **Use the standard theme**.
  Pass: each time the whole window follows at once (sidebar, lists,
  player bar and Settings), and no text is hard to read. The menu bar
  and macOS's own dialogs keep the system's look.
- [ ] **An effect.** Turn on **Effects** in Settings › Features (off at
  first). Play a track; in Settings › Effects turn on **Reverb** and
  move its **Mix**, then turn it off again. Pass: the reverb is clearly
  heard and glides in and out, with no click or jump in level; with it
  off, the track sounds as it did before, by ear (that it is
  bit-identical is the tests' job). Leave **Effects** on for the
  workbench.
- [ ] **Recording in each format.** Turn on **Recording** in Settings ›
  Features (off at first), and in Settings › Recording click **Choose
  Folder…** and choose the empty folder. For each **Format** in turn
  (**WAV**, **AIFF**, **FLAC**, **Apple Lossless (.m4a)**, **AAC
  (.m4a)**, **MP3**), play some music, click **Record** on the player
  bar, and click it again after about ten seconds. Pass: each time,
  "Recording saved" names the file; the folder holds the six, each named
  by date and time ("ano-mp 2026-10-04 21.15.03.wav", never a title)
  with its `.cue` beside it, and each plays in another app (QuickTime
  Player) as it sounded.
- [ ] **Recording after a relaunch.** Quit, reopen and record one more
  clip. Pass: it is saved in the same folder, with no request to choose
  the folder again.
- [ ] **The effects workbench.** Click **Effects workbench** in the
  sidebar, click **Choose a File…** and choose a short track from the
  test music. On the page, turn on **Echo** and click **Record One
  Take**. Pass: the track plays from its start with the echo, the
  recording stops by itself at its end, the page names the saved file,
  and that file plays back with the echo. Then turn **Effects** off in
  Settings › Features.
- [ ] **New visualizations.** Open the visualizer and choose
  **Tonnetz**, then **Cymatics**, in its picker. Pass: each draws and
  moves with the music.
- [ ] **More Like This.** Right-click one of the two artists' albums
  anywhere (on **Home**, in **Album artist** under **Library**, or on
  the artist's page) and choose **More Like This**. Pass: a list of
  albums from the library opens, each with its reason (the shared genre,
  an era, a label).
- [ ] **Recommendations from outside the library.** Turn it on in
  Settings › Features (off at first; Online sources stay on, as in
  section 4). Open the page of the artist whose album you matched in
  section 4 (its **About** says when and where they were born or formed,
  from MusicBrainz), click the heart beside their name, and play two or
  three of their tracks to past halfway (suggestions come from the
  artists you heart and play). Open **Home**. Pass: **Beyond your library**
  lists artists not in the library, each saying why; clicking one shows
  links (**MusicBrainz**, **ListenBrainz**) that open in the browser;
  **Not Interested** removes one.
- [ ] **Similar artists.** On that artist's page, and on the page of one
  of the two artists who share a genre. Pass: **Similar artists**, under
  the biography, lists the other library artist, which opens their page;
  with outside recommendations on, **Not in your library** lists
  artists you don't have.
- [ ] **Find a feature.** Type "cross" in the search box. Pass: under
  **Features**, above the music, **Crossfade** says "Settings ›
  Playback", and clicking it opens Settings at that setting,
  highlighted. A switch that is off (search "remote") says **Off** and
  opens at its switch, without turning it on.

## 6. Update from the previous version

After the release is published (the check ignores drafts). There is no
installer: the app links to the release, and the DMG replaces the app
(`PLAN.md` §8.2).

- [ ] **The update offer.** On the Mac with the previous release, click
  Settings › About › Check for updates. Pass: the new version is named,
  and "Download from GitHub" opens its release page in the browser.
- [ ] **Automatic checks.** Turn on "Check for updates automatically"
  and relaunch. Pass: within a minute a toast names the new version.
- [ ] **The update.** Download the DMG from that page, quit the app and
  drag the new one over the old in Applications. Pass: it launches as
  the new version (Settings › About), and Check for updates says it's
  the latest; the library, playlists, settings and online sources' keys
  are all still there; folders are still readable; Gatekeeper raises no
  dialog beyond the first launch of a download.
- [ ] **Updates off.** With automatic checks off, relaunch. Pass:
  nothing is checked (no "latest release" line in the log).

## 7. Performance (PLAN.md H18)

On the Mac you use day to day, with your own library (note its track
count from Settings › Library; the budgets are for 50,000 tracks), the
release build, and nothing else busy. Note each number in the results.

- [ ] **Launch to first paint.** Quit the app, wait a minute, open it.
  Then Settings › Show logs, the newest `first paint after N ms` line.
  Pass: at most 1,500 ms.
- [ ] **Memory at rest.** One minute after launch, nothing playing, the
  main window open on the library. In Activity Monitor › Memory, add up
  `ano-mp` and its `ano-mp Web Content` and `ano-mp Networking`
  processes (the "Memory" column). Pass: at most 400 MB in all.
- [ ] **CPU while playing.** Play an album with the visualizer closed,
  wait 30 s, and read `ano-mp` plus its Web Content process in Activity
  Monitor › CPU over another 30 s. Pass: at most 5% in all. Then open
  the visualizer (any style) and read them again: at most 25%.

## 8. Clean up

- [ ] Quit the app. Pass: no crash report on quitting.
- [ ] Note the macOS version and Mac model in the results, and anything
  that looked wrong even if it passed.
