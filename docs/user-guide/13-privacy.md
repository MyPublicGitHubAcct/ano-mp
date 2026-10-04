# 13. Privacy

ano-mp is built to keep your music and your listening on your Mac. This
chapter says what stays there, what goes online and only when you've
switched it on, and what the logs and diagnostics contain.

## What stays on your Mac

- **Your music files.** ano-mp reads them and never changes, moves,
  uploads or sends them, nor their names or locations.
- **Your library**: the tracks, albums and artists it found, your
  playlists, favourites, ratings, playback preferences, your choices of
  covers and matches, and your settings.
- **Your listening history**: what you played and when. It is used for
  History, Home's suggestions and recommendations, all worked out on your
  Mac. Turn it off, or clear it, in **Settings › Features**.
- **Recommendations** (**More Like This**, **You might like**): from your
  library and history alone. Nothing leaves your Mac for them.
- **Keys and tokens** (Discogs, ListenBrainz) are kept in the macOS
  keychain, not in ano-mp's settings, and never written to the logs.
- **Recordings** go only to the folder you chose.

## What goes online, and when

ano-mp contacts these services only while the feature that uses them is on.
None is contacted for anything else.

| Service | When | What is sent | On at first |
|---|---|---|---|
| MusicBrainz | looking albums and artists up | album titles, artist names, track lengths | yes |
| Cover Art Archive | fetching covers | the MusicBrainz release of an album | yes |
| Wikipedia and Wikidata | fetching biographies and descriptions | the article names MusicBrainz links to | yes |
| Discogs | looking albums up, showing their details | album titles and artist names, with your token | no |
| ListenBrainz | **Send listens to ListenBrainz** | each listen: title, artist, album and their MusicBrainz ids, with your token | no |
| ListenBrainz | **Recommendations from outside the library** | the MusicBrainz ids of your most-played artists | no |
| GitHub | **Check for updates** | nothing but the request (GitHub sees your IP address) | no (only when you click) |

Every service sees your Mac's IP address, as any website you visit does.

- **Use online services** in **Settings › Online sources** switches all
  the album and artist lookups off at once. The Welcome view, before your
  first scan, lists what is on and what each is sent.
- **Recommendations from outside the library** shows in Settings exactly
  which artists' ids it sends.
- The **remote control**, when on, listens on your local network only, and
  answers only phones you've paired with a code
  ([chapter 10](10-other-controls.md#what-it-exposes)).

## Logs and diagnostics

ano-mp keeps a log of what it did and what went wrong, to help fix
problems. **Show logs** in **Settings › About** opens its folder.

- The log records events and counts ("scanned 3 folders, 9,012 tracks"),
  never your keys or tokens.
- Titles, artists, file names and paths are left out of the log at its
  normal level, and paths are removed from any message that would contain
  one.

**Copy diagnostics** (in **Settings › About**) copies a short report for a
bug report: ano-mp's and macOS's versions, the output device, how many
tracks, albums and folders the library has (and how many folders can't be
read), which features and online sources are on, facts about the library's
database, and the log's last lines. It holds **no file paths, titles or
artists**. You can read it before you paste it anywhere: it is plain text.

Nothing is sent anywhere unless you send it: ano-mp has no analytics and
no crash reporting.
