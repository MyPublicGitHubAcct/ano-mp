# Appendix B. Supported formats

## Formats ano-mp plays

| Format | File names end in | Kind |
|---|---|---|
| MP3 | .mp3 | lossy |
| AAC | .m4a, .m4b, .mp4, .aac | lossy |
| Apple Lossless (ALAC) | .m4a, .m4b, .mp4 | lossless |
| FLAC | .flac | lossless |
| Ogg Vorbis | .ogg, .oga | lossy |
| Opus | .opus, .ogg | lossy |
| WAV | .wav | uncompressed (PCM, 8- to 64-bit, A-law, µ-law) |
| AIFF | .aif, .aiff, .aifc | uncompressed |
| Windows Media Audio | .wma | lossy and lossless (WMA, WMA Pro, WMA Lossless) |

Any sample rate and bit depth these formats allow plays. ano-mp plays in
stereo: a mono file plays on both speakers, and a file with more channels
(surround) plays its first two, front left and right. Other files in your folders, such as pictures, cue
sheets and lyrics, are read for what they add, and anything else is passed
over.

ano-mp doesn't play DRM-protected files (for example, old iTunes Store
purchases in .m4p files), Monkey's Audio (.ape) or WavPack (.wv).

## Files ano-mp reads beside your music

| File | What for |
|---|---|
| .cue | a cue sheet that splits a one-file album into tracks ([chapter 5](05-track-and-album-details.md#albums-as-one-file-cue-sheets-and-chapters)) |
| .lrc | lyrics, usually synced ([chapter 5](05-track-and-album-details.md#lyrics)) |
| cover.jpg, folder.jpg, front.jpg, album.jpg, albumart.jpg (or .png) | the album's cover ([chapter 5](05-track-and-album-details.md#covers)) |
| .m3u, .m3u8 | playlists, when you import them ([chapter 4](04-playlists-and-favourites.md#import-and-export-playlists)) |

## Tags ano-mp reads

The usual tags of each format (ID3 for MP3, Vorbis comments for FLAC, Ogg
and Opus, MP4 tags for .m4a, and so on): title, artist, album, album
artist, track and disc numbers, year and release date, genre, composer,
work and movement, lyrics, embedded pictures, ReplayGain (and Opus's R128
gain), and MusicBrainz ids. **Get Info** shows every tag a file has.

## Formats ano-mp records

WAV, AIFF, FLAC, Apple Lossless (.m4a), AAC (.m4a) and MP3. See
[chapter 9](09-recording.md#formats).
