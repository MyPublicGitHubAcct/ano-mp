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
- Settings › About checks GitHub for a newer release and links to its
  download page; automatic checks are a switch there, off by default.
