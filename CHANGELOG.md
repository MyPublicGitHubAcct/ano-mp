# Changelog

What changed in each release of ano-mp, newest first. The release
workflow cuts a version's section into its GitHub Release's notes
(`scripts/release.py`), so each `## [X.Y.Z] - YYYY-MM-DD` heading must
match the version `scripts/version.py` sets. Changes not yet released go
under `## [Unreleased]`; rename it when tagging (PLAN.md §8.7).

## [Unreleased]

### Added
- The first release: a music player for macOS that plays MP3, FLAC, AAC,
  ALAC, Opus, Vorbis, WAV, AIFF and WMA gaplessly, with a library of your
  folders, playlists and smart playlists, details and covers from
  MusicBrainz and the Cover Art Archive, visualizations, an equaliser,
  crossfade, ReplayGain and macOS media controls.
- ano-mp's own code is licensed under the MIT License (`LICENSE`); the
  components it is built on keep their own licences (`THIRD_PARTY_NOTICES`).
- Recording: with Recording on in Settings › Features, a Record button in
  the playing bar (and Controls › Record, ⌥⌘R) writes what you hear,
  effects, equaliser and crossfeed included but not the volume, to a file
  in a folder you choose, until you press it again: WAV (16-bit, 24-bit or
  32-bit float), AIFF, FLAC, Apple Lossless, AAC or MP3, set in Settings ›
  Recording. It carries on across track changes, gapless albums and
  crossfades, leaves out the time paused, and writes a cue sheet beside
  the file naming each track. MP3 is encoded by LAME 4.0, now built into
  the app's FFmpeg.
- Artists, in the sidebar under Favourites: every artist in the library,
  with a box to narrow the list by name. A click opens the artist's page;
  their menu plays, shuffles, queues or hearts them.
- Settings › About checks GitHub for a newer release and links to its
  download page; automatic checks are a switch there, off by default.
- Themes: Settings › Appearance picks a built-in theme (standard, light,
  dark, high contrast, paper, midnight, forest, ocean, rose, graphite,
  sunset, meadow) or edits one: colours,
  font, text size, density, corners and an accent from the playing
  album's cover, with every change shown as it's made and colours that
  fall short of WCAG AA flagged. Themes are saved by name and export to
  and import from JSON files.
- Effects on whatever is playing (Settings › Features › Effects, off by
  default): reverb, chorus, flanger, phaser, echo (with ping-pong),
  tremolo and auto-pan, lo-fi, and a spectral freeze that holds a moment
  of the music as a drone, held from Settings or the snowflake in the
  player bar. Each has a mix and its own controls, heard as they move,
  with presets; reverb and echo tails ring on into the next
  track, and the signal path lists the effects in use.
- Ten new visualizations: a Tonnetz that lights the chords as they're
  played, a recurrence plot that shows a song's repeats as it builds up,
  cymatics (sand on a plate ringing with the notes), a phase portrait of
  the sound's timbre, a pitch spiral, a harmonograph tuned to the
  interval sounding, rhythm rings that line up each bar's beats, a
  stereo stage showing where each sound sits between the speakers, and
  two combinations: Resonance (cymatics under the phase portrait) and
  Harmony (the pitch spiral beside the Tonnetz).
- More like this: tracks, albums and artists in your library like the
  one you're looking at (from a right-click, on album pages and on
  artist pages), and a "You might like" row on Home of albums you
  haven't played lately that resemble what you play. Each says why
  (a shared genre, era, label, artist, composer or band member, or being
  played together), and nothing leaves your Mac. Library radio picks
  its tracks the same way, so it now follows composers and what you
  play together too.
- Recommendations from outside the library (Settings › Features, off by
  default): artists you don't have, like the ones you play most, from
  ListenBrainz's similar artists and MusicBrainz's band members and
  side projects. They show on Home ("Beyond your library") and in More
  Like This for an artist, each saying why, with links to MusicBrainz,
  ListenBrainz, the artist's website and Bandcamp page. Only the
  MusicBrainz ids of your most-played artists are sent (Settings lists
  them); "Not Interested" stops suggesting an artist.
- The sidebar's Library and Playlists sections, Home's "You might like"
  and "Beyond your library", and an album's "More like this" fold away
  under their headings: closed at first, then as you left them.
- The sidebar's Now Playing item is hidden unless you turn it on in
  Settings › Display › Sidebar; the now-playing bar opens the same page.
