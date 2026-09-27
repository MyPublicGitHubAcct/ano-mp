<script lang="ts">
  // Search results for `library.query`, fetched 150 ms after typing stops,
  // grouped into artists, albums and tracks. An artist opens their page; an
  // album opens in the browser (under a rule that fits), or plays; a track
  // plays its album from that track. Words match anywhere in a field from
  // three letters (PLAN.md F12), and filters narrow it: `artist:`, `album:`,
  // `title:`, `genre:`, `year:1994` or `year:1990-1999`.
  //
  // ⌘-click (Ctrl elsewhere) and Shift-click select several tracks, which
  // the menu then acts on, and tracks drag onto playlists (F4).
  import { untrack } from "svelte";
  import {
    library as api,
    queue,
    type AlbumHit,
    type ArtistHit,
    type SearchKind,
    type SearchResults,
    type Track,
  } from "$lib/api";
  import { albumFeatureItems } from "$lib/featureMenu";
  import { fileName } from "$lib/format";
  import { count, t } from "$lib/i18n";
  import { click, emptySelection, rowsFor, type Selection } from "$lib/selection";
  import { collection } from "$lib/state/collection.svelte";
  import { drag } from "$lib/state/drag.svelte";
  import { adHocRule, library } from "$lib/state/library.svelte";
  import { player } from "$lib/state/player.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui, type MenuItem } from "$lib/state/ui.svelte";
  import { dragLabel, playlistItems, trackMenu } from "$lib/trackMenu";
  import Art from "./Art.svelte";
  import Heart from "./Heart.svelte";
  import TrackText from "./TrackText.svelte";

  const FIRST = { artists: 6, albums: 12, tracks: 50 };
  const MORE = 50;

  let results = $state.raw<SearchResults | null>(null);
  let selection = $state<Selection>(emptySelection);
  let request = 0;

  $effect(() => {
    const query = library.query.trim();
    void [library.version, collection.version];
    const current = ++request;
    const timer = setTimeout(
      () =>
        untrack(async () => {
          const found = await attempt(() =>
            Promise.all([
              api.search(query, 0, FIRST.artists, ["artists"]),
              api.search(query, 0, FIRST.albums, ["albums"]),
              api.search(query, 0, FIRST.tracks, ["tracks"]),
            ]),
          );
          if (current !== request || !found) return;
          const [artists, albums, tracks] = found;
          results = {
            ...tracks,
            artists: artists.artists,
            artistTotal: artists.artistTotal,
            albums: albums.albums,
            albumTotal: albums.albumTotal,
          };
          selection = emptySelection;
        }),
      150,
    );
    return () => clearTimeout(timer);
  });

  const more = (kind: SearchKind) =>
    attempt(async () => {
      if (!results) return;
      const current = request;
      const offset = { artists: results.artists, albums: results.albums, tracks: results.tracks }[kind].length;
      const page = await api.search(library.query.trim(), offset, MORE, [kind]);
      if (current !== request || !results) return;
      results = {
        ...results,
        artists: [...results.artists, ...page.artists],
        albums: [...results.albums, ...page.albums],
        tracks: [...results.tracks, ...page.tracks],
      };
    });

  const albumRule = adHocRule(["album"]);

  function showArtist(artist: { id: number; name: string }) {
    library.query = "";
    ui.showArtist(artist);
  }

  /** Their own albums oldest first, else the tracks they appear on. */
  const playArtist = (artist: ArtistHit) =>
    attempt(() =>
      queue.playNode(
        adHocRule([artist.albumArtistTrackCount > 0 ? "albumArtist" : "artist", "album"], "year"),
        [artist.id],
        true,
      ),
    );

  function openAlbum(album: AlbumHit) {
    if (!library.showAlbum(album)) playAlbum(album);
  }

  const playAlbum = (album: AlbumHit) => attempt(() => queue.playNode(albumRule, [album.id], true));

  const playTrack = (track: Track) =>
    attempt(() =>
      track.albumId !== null
        ? queue.playNode(albumRule, [track.albumId], true, track.id)
        : queue.play([track.id], 0),
    );

  const selectedTracks = (index: number) =>
    rowsFor(selection, index)
      .map((row) => results?.tracks[row])
      .filter((track) => track !== undefined);

  function openTrackMenu(event: MouseEvent, index: number) {
    if (!selection.rows.has(index)) selection = click(selection, index);
    const tracks = selectedTracks(index);
    const one = tracks.length === 1 ? tracks[0] : null;
    ui.openMenu(event, trackMenu(tracks, one ? () => playTrack(one) : undefined));
  }

  function trackClick(event: MouseEvent, index: number, track: Track) {
    if (event.metaKey || event.ctrlKey || event.shiftKey) {
      selection = click(selection, index, { shift: event.shiftKey, toggle: event.metaKey || event.ctrlKey });
    } else {
      selection = click(selection, index);
      playTrack(track);
    }
  }

  function albumMenu(event: MouseEvent, album: AlbumHit) {
    const ids = () => api.nodeTrackIds(albumRule, [album.id], true);
    const items: MenuItem[] = [
      { label: t("menu.play"), action: () => playAlbum(album) },
      { label: t("menu.playNext"), action: () => attempt(() => queue.addNode(albumRule, [album.id], true, true)) },
      { label: t("menu.addToQueue"), action: () => attempt(() => queue.addNode(albumRule, [album.id], true, false)) },
      { label: t("menu.addToPlaylist"), items: playlistItems(ids) },
      { label: t("heart.addShort"), action: () => collection.setFavourite("album", [album.id], true) },
    ];
    if (album.albumArtistId !== null && album.albumArtist !== null) {
      const artist = { id: album.albumArtistId, name: album.albumArtist };
      items.push({ label: t("menu.goToArtist"), action: () => showArtist(artist) });
    }
    const ref = { id: album.id, title: album.title };
    items.push(
      { separator: true },
      { label: t("menu.findDetails"), action: () => (ui.dialog = { kind: "findDetails", album: ref }) },
      { label: t("menu.chooseCover"), action: () => (ui.dialog = { kind: "chooseCover", album: ref }) },
      ...albumFeatureItems(ref),
    );
    ui.openMenu(event, items);
  }

  function artistMenu(event: MouseEvent, artist: ArtistHit) {
    ui.openMenu(event, [
      { label: t("menu.play"), action: () => playArtist(artist) },
      { label: t("menu.goToArtist"), action: () => showArtist(artist) },
      { label: t("heart.addShort"), action: () => collection.setFavourite("artist", [artist.id], true) },
    ]);
  }

  function press(event: PointerEvent, index: number) {
    drag.press(event, () => {
      const tracks = selectedTracks(index);
      if (tracks.length === 0) return null;
      const ids = tracks.map((track) => track.id);
      return {
        payload: { kind: "tracks", trackIds: async () => ids },
        label: tracks.length === 1 ? (tracks[0].title ?? fileName(tracks[0].path)) : dragLabel(tracks.length),
      };
    });
  }

  $effect(() => {
    ui.selectedTracks = () => selectedTracks(selection.focus);
    return () => (ui.selectedTracks = null);
  });
</script>

<section class="results" aria-live="polite">
  {#if results === null}
    <p class="muted status">{t("search.searching")}</p>
  {:else if results.artistTotal + results.albumTotal + results.trackTotal === 0}
    <p class="muted status">{t("search.none", { query: library.query.trim() })}</p>
    <p class="muted hint">{t("search.hint")}</p>
  {:else}
    {#if results.artistTotal > 0}
      <h3>{t("search.artists")} <span class="muted">{results.artistTotal}</span></h3>
      <ul class="chips">
        {#each results.artists as artist (artist.id)}
          <li>
            <button
              class="chip"
              onclick={() => showArtist(artist)}
              ondblclick={() => playArtist(artist)}
              oncontextmenu={(event) => artistMenu(event, artist)}
            >
              <span class="name">{artist.name}</span>
              <span class="muted small">{count("count.tracks", artist.trackCount)}</span>
            </button>
          </li>
        {/each}
      </ul>
      {#if results.artists.length < results.artistTotal}
        <button class="link" onclick={() => more("artists")}>{t("search.moreArtists")}</button>
      {/if}
    {/if}

    {#if results.albumTotal > 0}
      <h3>{t("search.albums")} <span class="muted">{results.albumTotal}</span></h3>
      <ul class="albums">
        {#each results.albums as album (album.id)}
          <li>
            <button
              class="album"
              onclick={() => openAlbum(album)}
              ondblclick={() => playAlbum(album)}
              oncontextmenu={(event) => albumMenu(event, album)}
            >
              <Art albumId={album.id} size="100%" />
              <span class="name">{album.title}</span>
              <span class="muted small">
                {[album.albumArtist, album.year].filter((v) => v !== null).join(" · ")}
              </span>
            </button>
          </li>
        {/each}
      </ul>
      {#if results.albums.length < results.albumTotal}
        <button class="link" onclick={() => more("albums")}>{t("search.moreAlbums")}</button>
      {/if}
    {/if}

    {#if results.trackTotal > 0}
      <h3>{t("search.tracks")} <span class="muted">{results.trackTotal}</span></h3>
      <ul class="tracks" aria-label={t("search.tracks")}>
        {#each results.tracks as track, index (track.id)}
          {@const name = track.title ?? fileName(track.path)}
          <li class="track-row" class:selected={selection.rows.has(index)}>
            <button
              class="track"
              aria-pressed={selection.rows.size > 1 ? selection.rows.has(index) : undefined}
              class:playing={player.currentItem?.trackId === track.id}
              onclick={(event) => trackClick(event, index, track)}
              oncontextmenu={(event) => openTrackMenu(event, index)}
              onpointerdown={(event) => press(event, index)}
            >
              <Art albumId={track.albumId} trackId={track.id} size="2.25rem" />
              <TrackText {track} />
            </button>
            <Heart on={track.favourite} label={name} onchange={(on) => collection.setFavourite("track", [track.id], on)} />
          </li>
        {/each}
      </ul>
      {#if results.tracks.length < results.trackTotal}
        <button class="link" onclick={() => more("tracks")}>{t("search.moreTracks")}</button>
      {/if}
    {/if}
  {/if}
</section>

<style>
  .results {
    height: 100%;
    overflow-y: auto;
    padding: 0.5rem 1rem 1.5rem;
  }

  .status {
    padding: 1rem 0;
  }

  h3 {
    font-size: 0.95rem;
    margin: 1.25rem 0 0.5rem;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .chip {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    border-radius: 8px;
    padding: 0.4rem 0.75rem;
    max-width: 16rem;
  }

  .albums {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(8.5rem, 1fr));
    gap: 1rem;
  }

  .album {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.25rem;
    width: 100%;
    padding: 0;
    border: none;
    background: none;
    text-align: left;
  }

  .album :global(.art) {
    aspect-ratio: 1;
    height: auto !important;
  }

  .tracks {
    container-type: inline-size;
  }

  .track {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    padding: 0.3rem 0.5rem;
    border: none;
    background: none;
    text-align: left;
    border-radius: 6px;
  }

  .track-row {
    display: flex;
    align-items: center;
    border-radius: 6px;
    padding-right: 0.25rem;
  }

  .track-row:hover {
    background: var(--hover);
  }

  .track-row.selected {
    background: var(--selected);
  }

  .hint {
    max-width: 40rem;
  }

  .name,
  .small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .small {
    font-size: 0.8rem;
  }

  .playing :global(.name) {
    color: var(--accent);
    font-weight: 600;
  }

  .link {
    margin-top: 0.5rem;
  }
</style>
