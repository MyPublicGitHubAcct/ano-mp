# Phase 4 — Online metadata services

Moved from `PLAN.md` (H20) on 2026-10-03, unchanged but for this
heading. Section numbers (§) refer to `PLAN.md`, whose Phase 4
keeps the phase's status, decisions and open steps.

**MusicBrainz matching** (album → release):
1. A release MBID from the tags: look it up directly (score 1).
2. Otherwise search releases by album title and album artist (Lucene
   special characters escaped, "Various Artists" handled), then fetch the
   top few candidates with their tracklists.
3. Score each on title and artist similarity (folded like `sort_key`),
   track count, per-track durations (aligned by disc and track number;
   ±3 s counts as a match), and year. Accept automatically at a score of
   0.85 or more that also leads the runner-up by 0.05; otherwise keep the
   candidates for the "Find details" dialog and mark the album "needs
   review". The scorer is a pure function, tested on its own.
4. The release's release group gives the Cover Art Archive a fallback when
   the release has no front cover.
**Steps:**
- [x] 4.1 Foundation: migration 003, `metadata::settings`, the HTTP client
  (`Transport`, limiter, `User-Agent`, backoff), the response cache. Tests
  with the fake transport.

  Done 2026-09-26 (`app/src-tauri/src/metadata/`):
  - `settings.rs`: `SourceId` (embedded, folder, musicbrainz,
    cover-art-archive; the serialized id is what the `source` columns
    store) and `Kind` (release, albumArt). `ServiceSettings` holds the
    online switch, auto-match, per-source enabled flag and key, and an
    order per kind. Stored settings are completed on read: unknown
    sources dropped, missing ones appended in default order, each kind's
    order listing exactly its sources. `sources_for(kind)` gives the usable
    ones in order: enabled, online only with the switch on, keyed if they
    need a key, and with what they rely on usable (the Cover Art Archive
    needs MusicBrainz).
  - `http.rs`: `Client` over a `Transport` (ureq in the app, a scripted
    fake in tests) and a `Clock` (the fake one advances on sleep, so the
    tests run the limiter and backoff without waiting). Per-host minimum
    interval (MusicBrainz 1 s, others 250 ms); 503/429 retried up to 3
    tries, honouring `Retry-After` up to 30 s; an unreachable host is
    refused at once for 1 min, doubling to 30 min, until it answers or
    `retry_now`. `get_json` reads through the cache and falls back to a
    stale copy when offline. The `User-Agent` is `ano-mp/<core version> (
    <repo URL> )`, the version from `anomp_version()` so no new copy of it.
  - `cache.rs`: `mb_cache` as a URL → body cache for every service.
  - Commands: `metadata_settings` (with each source's name, kinds and
    dependencies for the UI), `metadata_save_settings`,
    `metadata_reset_settings`; typed wrappers in `api.ts`.
  - Provider traits are left until a second source of the same kind
    exists (Discogs, 4.7); MusicBrainz is called directly until then.
  - The online modules have no caller in the app until the worker (4.5),
    so they carry `#[allow(dead_code)]` until then.
- [x] 4.2 Local art: folder images as an `AlbumArt` source; `art::lookup`
  walks the user's choice, then local sources in the configured order.

  Done 2026-09-26 (`metadata/folder_art.rs`, `library/art.rs`):
  - An image directly in a track's folder named `cover`, `folder`,
    `front`, `album` or `albumart` (that order, any case; .jpg, .jpeg,
    .png, .webp; at most 32 MB), then Windows Media Player's
    `AlbumArt*Large`/`AlbumArtSmall`. For a track in a disc folder ("CD1",
    "Disc 2") the folder above is tried too, never above the library
    folder.
  - `art::lookup` tries the user's pick in `album_art` (falling back to the
    order if that picture has gone or its source is unknown), then
    `sources_for(AlbumArt)`. It opens each library folder by id through
    `access::open_folder`, and skips one that can't be opened rather than
    failing the request. Saving the settings clears the art cache.
- [x] 4.3 MusicBrainz provider and the matcher; album links and details
  stored.

  Done 2026-09-26 (`metadata/musicbrainz.rs`, `matcher.rs`, `albums.rs`):
  - Search (`release:(…) AND artist:(…) tracks:N`, Lucene-escaped terms
    rather than phrases, 10 hits, cached 7 days) and lookup (recordings,
    artist credits, labels, release group, genres; cached 30 days). Tag
    MBIDs are checked to be UUIDs before going into a URL. `Release`
    keeps title, credit, date, country, status, barcode, labels and
    catalogue numbers, release group and its types and first date, genres
    (the release's, else its group's, most voted first), track list with
    lengths, and whether the Cover Art Archive has a front cover.
  - Scoring as designed above, plus title and artist as **gates**: the
    weighted score is multiplied by a factor that is 1 at a similarity of
    0.7 and 0 at 0.3, so another album by the same artist, or the same
    title by another artist, can't get through on track count and year.
    Titles compare folded (`sort_key::fold`), punctuation-blind, "&" as
    "and", with a bracketed suffix ignored at a small cost.
  - `match_album`: a user's choice is left alone; a tagged release MBID
    is looked up (1 request) and trusted, falling back to a search if
    MusicBrainz no longer has it; otherwise one search plus lookups of
    the best 3 hits (about 4 s at the rate limit). The result is stored
    as matched, review (best candidate kept) or none. `choose_release`
    stores the user's pick; `clear_link` returns an album to automatic.
  - Tests: recorded responses for "In Rainbows" (a search and four
    releases, trimmed; CC0) in `metadata/fixtures/musicbrainz/`, served by
    the fake transport. `live_search_and_lookup` (ignored; `cargo test
    live_ -- --ignored`) checks the real service, TLS and `User-Agent`.
  - Artist links (`artist_links`) are filled with artist info in 4.7.
- [x] 4.4 Cover Art Archive provider and the image cache.

  Done 2026-09-26 (`metadata/coverartarchive.rs`, `images.rs`,
  `library/art.rs`):
  - `fetch_album_art(client, conn, cache, album_id)`, for the 4.5 worker:
    the archive picture the user chose (an `album_art` reference on
    `coverartarchive.org`), else for an album with a *matched*
    MusicBrainz link the release's `front-500` (skipped when MusicBrainz
    says the release has no front cover), else the release group's. It
    does nothing for an album that isn't linked or only awaits review, or
    whose cover is cached; a 404 moves on to the next URL. Offline it
    fails with `Error::Offline` and writes nothing. It returns what it did
    (`NotLinked`, `Cached`, `Downloaded`, `NotFound`); `fetch_image` does
    one URL, for the "Choose cover" dialog. Whether the source is enabled
    is the caller's check. Requests go through `http::Client` with
    `IMAGE_LIMIT`; ureq follows the 307 to archive.org.
  - 500 px, of the archive's 250/500/1200: sharp at the largest size the
    player shows art, at around 100 KB.
  - `release_images` lists a release's pictures (`/release/{mbid}/`,
    cached 7 days, a 404 meaning none) with types, the front flag,
    comment, and thumbnails by width. Older pictures only have `small`
    and `large`, read as 250 and 500; ids are sometimes numbers, sometimes
    strings; `http://` URLs are made `https://`. For the 4.6 dialog (now
    `cover_candidates`).
  - `images::ImageCache`: `<app cache dir>/images` (inside the container
    under the sandbox), one file per URL named by the **SHA-256** of the
    URL in hex, from `ring`, which rustls already links in, so no new
    code; SHA-256 is fixed by its standard, unlike `DefaultHasher`.
    Written to a `.part` file, synced, then renamed into place; `.part`
    files over an hour old are left by a crash and removed. Only data
    starting like a JPEG, PNG, GIF or WebP is stored or served, so an
    error page served with a 200 is refused. Budget 500 MB (about 5,000
    covers at 500 px): beyond it the least recently used files go, down
    to 90% so eviction doesn't run on every download. Recency is the
    file's modification time, moved on by a read at most once a day so
    browsing doesn't write; the total is counted by one directory scan at
    the first store and kept in memory after that.
  - `art::lookup` serves the `CoverArtArchive` source from the cache
    only: the user's picked URL, or the first cached of the album's
    cover URLs, which are derived from its matched link, so no schema
    change. It never goes online.
  - Tests: recorded listings for two "In Rainbows" releases and an 8×8
    JPEG in `metadata/fixtures/coverartarchive/`. `live_cover_and_listing`
    (ignored) fetches a real cover through the redirect and a listing.
  - Known limits, for 4.5: an album whose cover the archive doesn't have
    costs up to two requests every time it's fetched, since no "not
    found" is recorded (an `album_links` row for `cover-art-archive`
    could hold it); after a download the worker must drop that album from
    `ArtCache`, which may remember it as having no art or a local
    picture; the cache isn't cleared when the MusicBrainz match changes,
    so the old release's cover stays until evicted; with the online
    switch off, `sources_for` leaves the archive out, so downloaded covers
    aren't shown either.
- [x] 4.5 The metadata worker: job queue, priorities, background
  enrichment after a scan (if enabled), progress and `metadata-changed`
  events, offline backoff.

  Done 2026-09-26 (`metadata/jobs.rs`, `worker.rs`, `commands.rs`):
  - `jobs.rs` holds the logic, tested without Tauri or a thread: the
    queue, the skip rules, and `Worker::step`, which runs one job and
    reports through a `Host` trait (like `media.rs`'s `Publisher`).
    `worker.rs` runs it on a "metadata" thread that owns the
    `http::Client` (ureq, system clock) and a connection from `db::open`;
    other threads only queue requests, so nothing waits on the network
    while holding the shared connection, and nothing runs on the main
    thread. Started at setup; on `RunEvent::Exit` it's told to stop and
    not joined, since a request can take its 30 s timeout. Queued replies
    are dropped then, so a waiting caller hears at once.
  - Jobs: `Match(album)` matches the album if it needs it, then queues
    its cover at the front of its priority; `Cover(album)` fetches the
    cover. Priorities: user requests, then the album playing (queued from
    `queue::publish` when the current album changes), then background
    enrichment; first come first served within each. A job is queued once;
    a higher request moves it forward and adds its reply. Whether a job
    is needed and its source usable (`sources_for`) is decided when it
    runs, from the settings and the database then. Automatic jobs (the
    album playing, background) need "match automatically"; a user request
    doesn't, skips the 30-day waits, and gets an error if the source is
    off.
  - Background enrichment is queued after each scan, after the settings
    are saved, and at launch (for a run cut short by quitting, and albums
    due for a retry); the worker checks `auto_match` when it gets to it.
    Skip rules (`needs_match`, `needs_cover`): an album the user chose a
    match for is never re-matched, but its cover is still fetched if
    missing, since that follows their choice; an accepted match whose
    cover is downloaded is left alone; 'none' **and 'review'** are retried
    30 days after the check. 'review' is retried because MusicBrainz grows
    and a better release may appear. Meanwhile its candidate waits for the
    4.6 dialog and gets no cover. No cover is fetched when the user picked
    a picture from another source.
  - "No cover found" is recorded as the album's `album_links` row for
    `cover-art-archive` ('matched' or 'none', `external_id` = the release
    asked about), written by `fetch_album_art`; so no migration. A record
    for a release the album is no longer matched to doesn't count.
  - A changed match: the worker compares the album's cover URLs before
    and after matching. They're derived from the link, so the old
    release's cover is never picked again. When they differ, the album is
    dropped from `ArtCache` (new `ArtCache::remove`) and named in
    `metadata-changed`. The old file stays in the image cache until
    evicted; that's harmless.
  - Offline: while `http::Client` backs off from a host, automatic jobs
    for it stay queued and jobs for other hosts go on (covers keep coming
    while MusicBrainz is down). With only waiting jobs left, `step`
    returns how long until the first host is tried again (`Client::retry_at`)
    and the thread sleeps that long or until a request. A background job
    that fails offline goes back to the front of the queue, not counted
    as failed. A user request fails at once with `Error::Offline`. If it had
    also been queued automatically, it stays queued for that.
    `metadata_retry_now` clears the backoff.
  - Events: `metadata-changed` (`{albums, artists}`; no artist ids until
    4.7) is batched at most once a second, sent at once after a user
    request (before its answer) and when the worker goes idle or pauses.
    `metadata-progress` (`{done, total, current, paused, unreachable}`,
    counted in albums since the worker was last idle; zeros when idle) is
    sent at most four times a second, and at once on pausing and going
    idle. Changed art is dropped from `ArtCache` at once, and Now Playing
    looks up its artwork again if it shows that album
    (`media::art_changed`).
  - Reloading art in the webview: the UI keeps a count per album of the
    `metadata-changed` events that named it, and adds it to the art URL
    next to the scan count (`artUrl(key, generation, version)`), so
    `max-age=86400` stays.
  - Downloaded covers stay visible with the online switch off
    (`ServiceSettings::sources_shown`, used by `art::lookup`): the switch
    stops the app contacting services, and the Phase 4 exit asks for the
    app to behave the same with the network off. Turning the Cover Art
    Archive (or MusicBrainz, which it needs) off hides them.
  - Commands: `metadata_status`, `metadata_retry_now`, and
    `metadata_update_album` (a user request that waits for the match and
    cover); wrappers and payload types in `api.ts`. The dialogs and the
    Services panel are 4.6.
  - The online modules' `#[allow(dead_code)]` is gone. It's kept only on
    `choose_release`, `clear_link` and the image listing (for the 4.6
    dialogs), and on `cache::prune`.
  - Checked in the app (2026-09-26) on a 16-album library: with requests
    failing as unreachable (`ALL_PROXY` pointed at a closed port) the
    worker paused at once, nothing was written, and the app worked on
    local data. Online, 13 albums were matched and got covers in about 80
    s; one was 'none', and two searches failed with MusicBrainz 503s
    after three tries. The next launch matched those two and requested
    nothing else. Not checked by eye: covers appearing in an album list
    during a run (the window was on a view without art, and the check
    didn't drive the UI), and adding a folder through the picker and
    scanning it.
  - Known limits: a job that fails with an HTTP error (e.g. a 503 after
    three tries) is logged and dropped until the next enrichment; nothing
    prunes `mb_cache` yet (H10 does since 2026-10-02); a rescan that changes an album's tracks
    doesn't make an accepted match be looked at again; covers are
    downloaded even when a local picture comes first in the order (the
    DB doesn't record which albums have embedded art), so a library of
    more than about 5,000 albums can churn the 500 MB image cache; 4.6's
    "Use automatic" (`clear_link`) should drop the album from `ArtCache`
    itself, since the worker only does that once it has re-matched; on
    Windows (WebView2's HTTP cache persists across launches) the scan
    count and per-album count restart at 0 each launch, so an art URL may
    repeat one cached in an earlier run (Phase 10).
- [x] 4.6 Commands and UI: album details with source labels, "Find
  details" and "Choose cover" dialogs with candidates per source, and a
  Services panel (master switch, enable, order, keys, status) that Phase 6
  folds into the admin screen.

  Plan (2026-09-26):
  - **Worker calls.** The dialogs' searches, lookups and listings go
    online, so they run on the metadata worker (the only owner of
    `http::Client`): `Shared::call` queues a closure that the worker runs
    with its client, connection and image cache ahead of every job, and
    the command waits for its answer on a blocking thread. Offline, the
    client refuses at once, so a dialog shows "offline" rather than
    hanging. Changes a call makes are reported like a job's.
  - **Album details** (`metadata_album`): the tag values (title, album
    artist, year, tracks, length, genres, the tagged release MBID), the
    MusicBrainz link (status, chosen by the user or not, score, when
    checked) with the release's fields while MusicBrainz is shown, and
    where the cover shown comes from (`Art` records its source) and
    whether the user chose it. The UI labels each value with its source.
  - **"Find details"**: `metadata_release_candidates` lists, per
    album-details source in the configured order, the candidates: the
    search hits (the default query is the automatic one, so it's usually
    cached), the best three looked up and scored with track lengths, the
    rest scored on the search result alone and flagged so, plus the
    current match or review candidate. The title and artist searched for
    can be edited, and a MusicBrainz release URL or MBID is looked up
    directly. Actions: pick a release (`choose_release`, then its cover
    is fetched), "None of these" (a 'none' row chosen by the user, which
    automatic matching leaves alone), and "Use automatic" (clears the
    row, drops the album's art, and matches again at once).
  - **"Choose cover"**: `metadata_cover_candidates` lists the album-art
    sources in the configured order: the embedded picture, every image
    in the album's folders (`folder_art::images`), and the archive's
    listing for the matched release plus its release group's front. The
    archive's 250 px thumbnails are downloaded on demand through the
    worker (`metadata_fetch_image`, archive URLs only) into the image
    cache, so the webview never contacts a service. Previews come from
    the art handler: `anomp-art://localhost/album-<id>/<source>?ref=…`
    serves one candidate through the same `from_source`, never online; a
    folder reference must be a relative path without `..`. Picking one
    stores `album_art` (a folder picture only if it's one of the album's
    images; an archive picture by its 500 px URL, downloaded next);
    "Use automatic" deletes it. Both drop the album's art and send
    `metadata-changed`.
  - **Artists**: the same for the 'review' artists of 4.7: candidates from
    the artist search (name editable, MBID or URL looked up), pick, "None
    of these", "Use automatic".
  - **Services panel**: a main view opened from the sidebar, whose entry
    shows what the worker is doing. The online switch, "match
    automatically", each source (on/off, what it supplies, what it needs,
    a key field if it takes one, its status from `metadata-progress`: in
    use, off, can't be reached with "Try now"), each kind's order (move up
    and down), the worker's progress, and "Reset to defaults".
    `SourceInfo` gains the hosts each source contacts, so status maps to
    sources.
  - **UI**: a modal `Dialog` (native `<dialog>`) and the three dialogs,
    opened through `ui.dialog`. The browser shows an album header when the
    node is an album: cover (click to choose), the release facts with
    their source, the match status, "Find details…", "Choose cover…", and
    a details table (field, value, source). Album menus in the browser,
    search and artist page get "Find details…" and "Choose cover…"; the
    artist page gets "Choose…" for a 'review' match and "Wrong artist?"
    for a match.
  - **Tests**: the worker's calls; candidates, choosing, rejecting and
    clearing with the fake transport and recorded responses; folder image
    listing; cover choices' validation; the handler's candidate paths.
    Then `npm run check`, and the app checked by eye.

  Done 2026-09-26, as planned (`metadata/commands.rs`, `library/albums.rs`,
  `library/art.rs`, `AlbumInfo.svelte`, the `*Dialog.svelte` components,
  `ServicesPanel.svelte`). Beyond the plan:
  - `Release` gains each medium's format and the release's
    disambiguation (both defaulted, so details stored before still parse):
    without them the dialog's releases of one album look the same.
  - A call's answer is sent after the changes it made are reported
    (`Call` returns an `Answer`), as jobs do, so the UI reloads before the
    dialog closes. Calls run before any queued job, but wait for the job
    that's running.
  - The "Find details" dialog shows the selected release's tracks next to
    the album's with their lengths, marking those within 3 s, and scores
    from a search result alone as "~".
  - "Use automatic" for a cover queues the archive's cover, since a user's
    pick from another source had stopped it being fetched.
  - The art handler's candidate route answers with `no-store`; the
    webview's `convertFileSrc` encodes slashes, so the UI adds
    `/<source>` after it.
  - Checked in the app (2026-09-26) on the 16-album library, driven by a
    temporary dev-only script (scripted clicks need an accessibility
    permission the session didn't have): the album header and its details
    table; "Find details" listing the match with its tracks compared, and
    other releases told apart by country, format and label; choosing a
    CD release (its details and cover replaced the automatic ones);
    "Choose cover" with archive previews fetched through the worker, a
    pick marked "Your choice", then "Use automatic" for the cover and the
    release, which put back the original match and cover; the Online
    sources panel, where turning MusicBrainz off showed the Cover Art
    Archive and Wikipedia as needing it and struck them from the order;
    "Find artist" from an artist page. Two layout bugs found and fixed
    (the sidebar's section heading growing, the dialog body collapsing).
    Not checked by eye: the dialogs offline, dark mode, narrow windows,
    and a 'review' album or artist (none in that library).
  - Known limits: album details come from MusicBrainz only, but the
    commands and dialog are per source, ready for Discogs (4.7); "Choose
    cover" offers only the matched release's archive pictures and its
    release group's front, not other releases'; only the first embedded
    picture found in an album's first files is offered; archive previews
    go into the image cache and count toward its budget; keys, when a
    source takes one, are stored in the settings JSON (4.8 decides on the
    keychain); "Use automatic" while offline leaves the album unmatched
    until the worker can reach MusicBrainz again.
- [x] 4.7 More sources: Wikidata/Wikipedia descriptions, then Discogs
  (user token). fanart.tv, TheAudioDB, iTunes and Deezer after their terms
  are checked.

  Done 2026-09-26: artist biographies and album descriptions from
  Wikipedia. Discogs moved to 4.8 (below), with fanart.tv and the rest:
  its API terms, as far as they could be checked, rule out its covers in a
  commercial app and any offline copy of its data, so whether and how it
  ships is 4.8's decision, not an implementation step. The provider traits
  planned in 4.1 wait for it too, since no second source of the same kind
  exists yet.

  Artist pages with Wikipedia biographies done 2026-09-26
  (`metadata/artists.rs`, `wikipedia.rs`, `library/artists.rs`,
  `ArtistPage.svelte`), then album descriptions:
  - Settings: source `wikipedia` (requires MusicBrainz) and kind
    `artistInfo` (an artist's biography). Stored settings from before get
    its default order. Artists are matched on MusicBrainz whenever
    MusicBrainz is usable; the match gives the facts and the link to a
    biography.
  - Artist matching (`artists::match_artist`), first that works: the
    artist MBID in the tags; the artist credited alone, under the same
    name, on the releases the artist's albums are matched to (the most
    common, score 0.95); a search by name and alias. Names are shared
    ("Nirvana" is several bands), so a hit is accepted only at a
    MusicBrainz score of 90 or more with a lead of 15 over the next of
    the same name, else 'review'; MusicBrainz ranks the best-known well
    ahead (100 vs 75 for the two Nirvanas). "Various Artists" is never
    looked up. The lookup (`url-rels+genres`, cached 30 days) keeps type,
    area, begin/end area and dates, disambiguation, genres, and the
    Wikidata item, a direct English Wikipedia link (older entries) and the
    homepage. Stored in `artist_links` like albums; no migration.
  - Biography: Wikidata `wbgetentities` (sitelinks, `enwiki`; a merged
    item comes back under its new id) → the article's lead section as
    plain text (`prop=extracts&exintro&explaintext`, redirects followed),
    each cached 30 days. Stored as the artist's `wikipedia` row with the
    MusicBrainz artist it was fetched for as `external_id`, so a changed
    match never shows the old biography; "none" (no article, or no link)
    is retried after 30 days like the other sources. CC BY-SA 4.0: the
    page credits the article by name with a link, and the licence.
  - Worker: `Job::Artist` (match, then its biography) and
    `Job::Biography`; jobs are keyed by `Subject` (album or artist), a
    `Biography` job waits while either Wikimedia host is backing off, and
    progress counts albums and artists (`current` is tagged `album` or
    `artist`). New priority `Viewing` (the artist page shown) between the
    user's requests and the album playing: automatic, but its changes are
    sent at once. Background enrichment queues album artists after the
    albums (their matches help), only while a biography source is usable;
    the page looks an artist up when shown anyway. `metadata-changed` now
    names artists.
  - Commands: `library_artist` (the page: albums the artist is album
    artist of, oldest first, with the release type from their matched
    release; other artists' albums they appear on; the metadata; queues
    the artist at `Viewing`) and `metadata_update_artist` (a user request,
    skipping the retry waits). `TrackSummary` and queue items carry
    `artistId`.
  - UI: the artist view (`ui.showArtist`, with a back stack) shows type,
    area, disambiguation, formed/born and ended, genres, the biography
    (two paragraphs, then "Read more"), "Look up again" when there is
    none, links to MusicBrainz and the homepage, and albums grouped as
    Albums, EPs, Singles, Live albums, Compilations, Soundtracks and
    Other, then "Appears on". Opened from a search's artist, the "Artist"
    button inside an artist in the browser, "Go to artist" on tracks,
    queue items and artist rows, and the artist on the now-playing view.
    Links open in the browser through `tauri-plugin-opener`, allowed only
    `http(s)` URLs.
  - Tests: recorded MusicBrainz artist and search responses, a recorded
    Wikidata response, and an extract whose text is a stand-in (no CC
    BY-SA text committed). `live_biography` (ignored) checks the real
    chain. Checked in the app (2026-09-26): at launch the five album
    artists of a 16-album library were matched through their releases and
    got biographies; "Various Artists" made no request; the page for
    Swans showed its facts, biography, credit and five albums.
  - Album descriptions (done 2026-09-26; `wikipedia::fetch_description`,
    `musicbrainz::lookup_release_group`): kind `albumInfo`, supplied by
    Wikipedia (settings stored before get its default order). The album's
    matched release (not a 'review' candidate) gives its release group,
    looked up with `url-rels` (cached 30 days) for its Wikidata item or a
    direct English Wikipedia link, then the same sitelink and extract
    requests as biographies. A separate release group lookup rather than
    more `inc` on the release lookup, so albums matched before get
    descriptions without their releases being fetched again. Stored as
    the album's `wikipedia` row in `album_links` with the release group
    as `external_id`: another release of the same album keeps the
    description, and a match to another album hides it; 'none' is retried
    after 30 days. `AlbumLink` keeps the stored JSON (`details`, not sent
    to the UI) and parses `release` only for MusicBrainz;
    `albums::store_source_link` stores another source's row. `Biography`
    became `wikipedia::Article`, and `SourcedBiography` `SourcedArticle`
    (both in the UI too), used for both.
  - Worker: `Job::Description`, following an album's `Match` (after its
    `Cover`, if one is wanted) and `Cover`, so the user's "update" and a
    chosen release get one too; background enrichment queues it for
    albums needing nothing else. It pauses while MusicBrainz or either
    Wikimedia host is backing off. Skip rules in `needs_description`,
    like `needs_biography`'s.
  - UI: `AlbumDetails.description`; the album header shows its first
    paragraph with "Read more", credited like the biography ("From the
    Wikipedia article “…”, under CC BY-SA 4.0", both linked), and the
    details table names it. The Online sources panel lists "Album
    descriptions" with its order.
  - Tests: a recorded MusicBrainz release group (trimmed, CC0), a recorded
    Wikidata response, and a stand-in extract, as for artists; fetching,
    'none', the release group rule, the worker's chain and skip rules,
    and the album details. `live_description` (ignored) passed against the
    real services, as did `live_biography`. Checked in the app
    (2026-09-26): at launch the 15 matched albums of the 16-album library
    were looked up; 11 got the right article (disambiguated ones such as
    "Decay (Godflesh album)" and "Godflesh (EP)" included) and 4 have none.
    Not checked by eye: the album header showing the description.
  - Known limits: English Wikipedia only; a biography, a description and
    the artist's MusicBrainz details aren't refreshed once found (an
    accepted match is kept, like albums'); an album gets a description
    only once matched, not while it awaits review; a 'review' candidate is confirmed in the "Find
    artist" dialog (4.6); an artist found only as a track artist is looked up only
    when their page is opened; several artists in one tag ("A; B") are one
    name and rarely match.
  - Releases not in the library (done 2026-09-26;
    `metadata/discography.rs`, `DiscographyPage.svelte`): an artist page
    matched on MusicBrainz links to a page listing the artist's release
    groups that the library lacks, grouped like the artist page (the
    grouping moved to `releases.ts`), oldest first, each linked to its
    MusicBrainz page, with its credit when it isn't the artist alone.
    - MusicBrainz's browse (`release-group?artist=…`) with
      `release-group-status=website-default`, as its own artist page
      lists them: no groups with only bootleg or promotional releases (106
      of Radiohead's 585). 100 a page, at most 10 pages (1,000 groups,
      10 s), each page cached 7 days like searches; "Check again" skips
      the cache. Nothing else is stored: the list is worked out from the
      cached pages and the albums' matches each time, so no migration.
    - In the library: a release group any album is matched to (not a
      'review' candidate), whoever its album artist; otherwise, for the
      artist's albums without a match, a group whose title is alike
      (`title_similarity` ≥ 0.9, so "OK Computer (Collector's Edition)"
      counts), so an album isn't listed as missing only because matching
      hasn't reached it.
    - `metadata_artist_discography` runs on the worker while MusicBrainz
      is usable; while it is shown but online services are off, only the
      cached pages are read (else an error saying so); with MusicBrainz
      off it fails as turned off.
    - Tests: a recorded browse page (trimmed to 14 of Radiohead's groups,
      CC0), paging and its limit, pages that overlap or fall short, the
      cache-only path, an unmatched artist. `live_discography` (ignored)
      passed against the real service. `npm run check` passes; not checked
      by eye in the app.
    - Known limits: no covers (each would be a Cover Art Archive request);
      a title match can hide more than one group of that title (an EP and
      a single both called "Creep"); an unmatched album filed under
      another artist isn't counted; a match changing while the page is
      open shows when it's opened again.
- [x] 4.8 Select and configure the alternative sources. Go through the
  sources table above and decide which ones ship, recording each decision
  and its reason in the table. Start with Discogs (moved from 4.7): read
  its API terms first-hand, and decide whether a details-only source that
  stores only the match (the Discogs release id) and fetches details when
  shown, cached at most 6 hours, with no pictures and with its
  attribution, is worth shipping. The provider traits (4.1) come with the
  first second source of a kind. For each source that ships:
  - Settle its terms (commercial use, attribution, caching, image display)
    and record the outcome; §8.1 still re-checks them before release.
  - Add its `SourceId`, the kinds it supplies, what it relies on, and
    whether it needs a key.
  - Set its defaults: enabled or not, and its place in each kind's order.
  - Decide how it gets a key: supplied by the user, or a project key
    shipped with the app if its terms allow that. Decide where keys are
    stored (the settings JSON, or the OS keychain).
  - Set its request interval in `http::request_interval` from its
    published limits.
  - Show its attribution wherever its data appears, if its terms require
    one.
  - Add recorded-response fixtures (with `record-fixtures.py`, §9.2 M3)
    and an ignored live test.
  - Make sure the Services panel (4.6) lists it, with a key field if it
    needs one.

  Done 2026-09-26. Decisions (the sources table has each one's terms and
  reason): Discogs ships, off by default; fanart.tv waits for its written
  consent; TheAudioDB isn't shipped; the iTunes Search API and Deezer are
  excluded; AcoustID stays deferred. Discogs (`metadata/discogs.rs`):
  - **Terms, first-hand**: the API Terms of Use (updated 2025-05-27) were
    read in full through the Help Center's article API. Its release data is
    CC0, but the terms still forbid showing it more than 6 hours behind
    discogs.com or keeping it longer than needed; images are Restricted
    Data, not for commercial use. The fee clause (charging for the part of
    an app that uses the API needs their permission) became a release gate
    (§8.1), since the owner chose to ship it opt-in now.
  - **Settings**: `SourceId::Discogs` (`discogs`), supplying `release`,
    needing a key, off by default (`enabled_by_default`), second in the
    album-details order. Settings stored before get it off, at the end.
    `SourceInfo` gained `keyName`/`keyUrl`, `storesDetails`, `credit` and
    `notice` for the UI.
  - **Keys** (`metadata/keys.rs`): in the OS keychain, not the settings
    JSON, since a personal access token can act on the user's Discogs
    account and the database is a plain file. The settings keep only
    `hasKey`, which saving the settings can't change; `metadata_set_key`
    writes the keychain (and turns the source on), and a reset keeps keys.
    macOS uses `keyring-core` with `apple-native-keyring-store`'s
    `keychain` store (security-framework was already linked in);
    elsewhere saving a key fails until that platform's phase (iOS needs
    the `protected` store and a provisioning profile). Reads are cached
    for the process; tests use a map in memory.
  - **Only the match is stored.** `album_links` gets the Discogs release
    id, status, score and `chosen_by`, with `details` NULL. Responses go
    through `Client::get_json_fresh`: in memory only, at most 5 hours
    (`discogs::MAX_AGE`, under the 6 the terms allow), no stale copy
    offline, the token in an `Authorization` header so it's never in a
    URL, an error or a cache key. `metadata_release_details` fetches a
    linked release through the worker when the album is shown.
  - **Matching** (`albums::ReleaseSource`, now also MusicBrainz's): the
    matcher, "Find details" and the user's picks run the same for both
    sources. Discogs' known release is the one the album's accepted
    MusicBrainz match links to (`release?inc=url-rels`, cached 30 days,
    a separate lookup so matched albums aren't fetched again), else a
    search (`/database/search`) whose best three are looked up and scored
    with durations; a search hit has no track count, which the matcher now
    leaves out rather than scoring 0. Masters play the part of release
    groups in `decide`. A changed MusicBrainz match drops an automatic
    Discogs match (it may have come from it), never the user's.
  - **Worker**: `Job::Discogs`, after an album's match, cover and
    description (so the MusicBrainz match can name the release), pausing
    while MusicBrainz or Discogs is backing off; background enrichment
    queues it, and runs with MusicBrainz off if Discogs is on. Discogs is
    1 request/s (`request_interval`), its limit being 60 a minute.
  - **Attribution**: "Data provided by Discogs", linked to the release's
    page, next to its data in the album header's summary and every row of
    the details table, and in "Find details" (linked to the Discogs search
    for the section, to the release for the one selected). The
    non-affiliation notice is under Discogs in Online sources. No Discogs
    image is fetched or shown anywhere, not even thumbnails.
  - **UI**: the Online sources panel has a token field (a password field,
    "Get one" linking to discogs.com's developer settings, "Remove"), notes
    that Discogs data is "fetched when shown, never stored", and says
    "Needs a personal access token" until there is one. The album header
    shows Discogs' summary, genres and styles when it's the first matched
    source, and "Styles" and "Credits" rows; without a connection it says
    the details aren't available. "Find details" lists each source with
    its own "Use automatic" and "None of these", and takes MusicBrainz or
    Discogs release links.
  - **Tests** (12 new, 222 in all): the memory cache, key checks, Discogs
    parsing (release, search, dates, positions, durations, formats,
    names), ids in links, the token header and a refused token, matching
    through MusicBrainz's link and by search, the user's pick surviving a
    new MusicBrainz match, the worker's chain with and without MusicBrainz,
    the "needs a token" message, and album details' credit and page. The
    three Discogs releases are recorded (fetched without a token) and
    trimmed to CC0 fields; the search response couldn't be recorded
    without a token, so it was put together in the documented format from
    them. `live_discogs` (ignored) needs `DISCOGS_TOKEN` and hasn't been
    run. `npm run check` passes. The keychain calls were checked in a
    scratch program (write, replace, read, delete) outside the sandbox.
  - **Not done or not checked**: nothing checked by eye in the app, with or
    without a real token; the keychain inside the sandboxed bundle;
    `record-fixtures.py` (§9.2 M3), which waits for M1's test harness, so
    the new fixtures were trimmed by hand.
  - **Known limits**: Discogs details need a connection each time an album
    is shown after 5 hours; a Discogs release id in the tags (as Picard
    plugins and beets write) isn't read, since the core reads MusicBrainz
    ids only; Discogs artist profiles (CC0) could be a biography source
    later; matching a large library on Discogs takes 1 to 4 requests an
    album at 1 a second; `live_discogs` and Discogs' own paging aren't
    exercised (10 results are asked for).
