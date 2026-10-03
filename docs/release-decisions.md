# Release decisions: briefs for the owner

The §8.1 items in `PLAN.md` that only the owner can decide (Phase 7 Step 6),
ordered by how much they block. Each brief says what has to be decided, the
options, what it blocks in §8.2–8.3, and what could be checked from the
repo. Nothing here is decided (asked again 2026-10-02, Step 8: all eight
still open); only the drafts at the end are reviewed. Record each decision in `PLAN.md` §8.1 (and
§4 where it changes a decision there), with its date.

Written 2026-10-02. Checks made from here: the code, read-only DNS and
WHOIS lookups of the candidate domains, nothing else. No service was
contacted on the owner's behalf.

## 1. Name, then bundle identifier and publisher

**To decide:** the product name (candidate **AnoTracks**, runner-up
**Anotone**), then the bundle identifier (e.g. `com.<owner>.anotracks`,
the same on every platform) and the publisher name shown by Gatekeeper
and the stores.

**Blocks:**
- The first signed release. The identifier is in the signature, in the
  notarization ticket and in every user's container
  (`~/Library/Containers/<identifier>`). It can't change after the first
  release without moving users' libraries, settings, bookmarks and
  keychain items to a new container (§8.1).
- The Developer ID certificate's name (the publisher, from the Apple
  Developer account; see 3).
- The App Store record, if that channel is chosen (2).
- `release.yml`'s file names (`ano-mp_<version>_…`) and the DMG's name.

**Checked from here:**
- No DNS records for `anotracks.app`, `anotracks.com`, `anotone.app`,
  `anotone.com` or `anotraks.com`. WHOIS: `anotracks.com`, `anotone.com` and
  `anotraks.com` were unregistered on 2026-10-02 ("No match"). The `.app`
  registry's WHOIS server didn't answer from here, so the `.app` names are
  unknown; a registrar's search will say.
- Not checked (yours): the USPTO and EUIPO registers (class 9, "ANOTRACKS"
  and "ANOTRAKS"; §8.1 notes the Japanese label "Ano(t)raks" uses the handle
  `anotraks`), and the App Store name.
- Where the current name and identifier live, for the rename:
  - `dev.anomp.player`: only `tauri.conf.json` (`identifier`) among the
    code; the log directory, the container and the app's data folders
    follow from it. The docs (`CLAUDE.md`, `README.md`, `PLAN.md`,
    `docs/step4-checklist.md`) mention it.
  - "ano-mp": `productName` in `tauri.conf.json`, the keychain service
    (`metadata/keys.rs`, `"ano-mp metadata"`: renaming it loses the stored
    Discogs and ListenBrainz tokens, so do it before the first release),
    the `User-Agent` (`metadata/http.rs`), ListenBrainz's `media_player`
    and `submission_client` (`history/listenbrainz.rs`), the log file name
    (`ano-mp.log`), the crate and npm package names, the notices' title,
    the release file names, and the UI strings in `en.json`.
  - Your real library is in `~/Library/Containers/dev.anomp.player`; a new
    identifier starts a new, empty container.

## 2. Distribution channels

**To decide:** for macOS, a notarized direct download (DMG), the Mac App
Store, or both (§8.1 proposes direct download first). Linux and Windows
can wait for their phases.

**Blocks:**
- The updater (§8.2): the Tauri updater plugin is a new crate and npm
  package (licence to check against `deny.toml`) and only fits direct
  downloads; the Mac App Store forbids it. I'll ask before adding it, once
  this is decided. Its signing key, once made, must be backed up offline:
  losing it strands every install.
- Where releases are hosted: the repository was made public on
  2026-10-03 (owner), so its GitHub Releases can serve public downloads
  and an update manifest, and its Actions minutes are free. A separate
  public repository or a website remain options; whichever URL the
  updater uses is compiled into every build.
- The Mac App Store needs its own certificates and provisioning, App Store
  Connect's privacy answers (see 6), and review notes; the app is already
  sandboxed, which the store requires.

**Ready either way:** `release.yml` builds the universal app and a DMG,
with checksums, into a draft release.

## 3. Apple Developer Program and the Developer ID certificate

Not an open item in §8.1, but §8.3 needs it before any signed release.

**To decide:** enrol as an individual or an organization (an organization
shows its name as the publisher and needs a D-U-N-S number).

**Blocks:** signing, notarization and Gatekeeper accepting the app on
other Macs. Until then, `release.yml` makes ad-hoc signed builds that
other Macs refuse to open, and says so in a warning on each run.

**Then:** create a "Developer ID Application" certificate and an App
Store Connect API key for notarization, and add the six secrets that
`release.yml`'s header lists. `scripts/check-signing.py` lists the
certificate's expiry from then on (monthly, §9.1). Keep offline backups of
the `.p12` and the `.p8` key.

## 4. JUCE licence

**To decide:** confirm the commercial JUCE licence is in place (§4 #1
chose it: the Starter tier until revenue passes its cap). Without it, JUCE
is AGPLv3 and the app's source would have to be published.

**Blocks:** any public release. `THIRD_PARTY_NOTICES` reproduces JUCE's
licensing statement whichever licence applies; nothing in the build
changes.

## 5. AAC licensing opinion

**To decide:** get the opinion §4 #3 asks for on shipping FFmpeg's AAC
decoder, or choose the fallback.

**Options:**
- FFmpeg's AAC decoder everywhere (as now), if the opinion allows it.
- On Apple platforms, decode AAC (and ALAC) with CoreAudio instead: only
  `FFmpegAudioFormat`'s registration changes, plus a CoreAudio-backed
  `AudioFormat` (an engineering task of a few days, with tests over the
  existing AAC fixtures).
- Drop AAC from the build (`build-ffmpeg.sh`'s lists).

**Blocks:** the first public release with AAC playback.

## 6. Privacy policy and support page

**To decide:** where the policy and support page are hosted (§8.1;
needed by the App Store, and good practice for a direct download), and
their wording. Depends on 8 (crash reporting) and 7 (the contact address).

**From the code, what the app sends where** (2026-10-02):

| Service | Default | When | What it receives |
|---|---|---|---|
| MusicBrainz (`musicbrainz.org`) | On | Matching albums and artists in the background, and the "Find details" and "Find artist" dialogs | Album titles, artist names, track counts and lengths, MusicBrainz ids; the `User-Agent` (app name, version, contact); the user's IP address |
| Cover Art Archive (`coverartarchive.org`, redirecting to `archive.org`) | On | Fetching covers | MusicBrainz release ids |
| Wikipedia and Wikidata (`en.wikipedia.org`, `www.wikidata.org`) | On | Artist pages and album descriptions | Wikidata ids and article titles that MusicBrainz links to |
| Discogs (`api.discogs.com`) | Off; needs the user's own token | Album details, when the user turns it on | Album titles and artist names, with the user's token |
| ListenBrainz (`api.listenbrainz.org`) | Off; needs the user's own token | After each play, when turned on | Title, artist, album, duration, MusicBrainz ids, the time of the play, the user's token |
| LAN remote | Off | While on | Nothing leaves the local network: it answers only local addresses and paired phones, serving the now-playing state, the queue and album covers |

Every online request goes through one client (`metadata/http.rs`).
"Online sources" can be turned off as a whole in Settings. Nothing else
goes out: no analytics, no crash reports, no update checks (the updater
would add one: 2). Keys are in the macOS keychain. The log stays on the
Mac; "Copy diagnostics" copies to the clipboard only when clicked, and
holds no paths, titles or artists. The policy must also name each new
source or feature that goes online (§4 #6's rule).

**Blocks:** the App Store submission; the support URL for the About page
and the stores.

## 7. MetaBrainz supporter plan and the `User-Agent` contact

**To decide:**
- The contact in the `User-Agent` MusicBrainz receives: an email address
  or a public web page where MetaBrainz can reach you. Today it's
  `https://github.com/MyPublicGitHubAcct/ano-mp`, public since 2026-10-03
  (it reached no one while the repository was private), though still
  marked as a placeholder (`metadata/http.rs`, `CONTACT`, marked as a
  placeholder). MusicBrainz throttles clients it can't identify.
- Whether to become a MetaBrainz supporter: they ask commercial users of
  MusicBrainz's data to support them (Phase 4's sources table), so this
  depends on whether the app is sold.

**Blocks:** the first release (the contact); nothing in the build (the
version in the `User-Agent` now comes from CMake's `project(VERSION)`).
Changing the contact is one line plus its test.

## 8. Crash reporting

**To decide:** none (as now) or opt-in (e.g. Sentry).

**Now:** a panic writes its message and backtrace to the local log
(PLAN.md H9) and the app exits; the user can attach "Copy diagnostics" to a
report by hand. Nothing is sent.

**Opt-in would add:** a new crate (its licence checked against
`deny.toml`) and a service, a switch in Settings (off by default, as for
anything that goes online), an entry in the privacy policy and the App
Store's privacy answers, and a symbol upload in `release.yml`
(`CARGO_PROFILE_RELEASE_DEBUG`, as CLAUDE.md describes for crash reports).

**Blocks:** the privacy policy's wording (6); nothing else.

## 9. The remaining §8.1 gates

These need reading or a review rather than a choice, but they gate a
release too:
- **FFmpeg's LGPL:** satisfied for the direct download by the shared
  libraries in `Contents/Frameworks` (`check-bundle.py` checks them) and
  `THIRD_PARTY_NOTICES` (licence, version, configure flags, source
  tarball). The App Store opinion is still open (§4 #1).
- **Discogs:** written permission if the app is sold, or leave Discogs out
  of that build. Its non-affiliation notice is now in Settings › About.
- **ListenBrainz:** read its terms for a commercial client.
- **LAN remote:** the security review of `remote/` (§8.1) before a release
  ships it.
- **GitHub settings (from Step 2):** secret scanning with push protection
  and Dependabot security updates, turned on in the repository's settings.

## Drafts reviewed

Kept as drafted (owner, 2026-10-02):
- `CHANGELOG.md`'s first entry (the release notes `release.py` cuts).
- `error.noticesUnreadable` in `en.json`: "The third-party notices
  couldn't be read" (shown only if the bundled file is missing).
