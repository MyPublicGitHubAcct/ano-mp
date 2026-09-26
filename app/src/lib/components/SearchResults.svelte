<script lang="ts">
  // Search results for `library.query`, fetched 150 ms after typing stops,
  // grouped into artists, albums and tracks. An artist opens their page; an
  // album opens in the browser (under a rule that fits), or plays; a track
  // plays its album from that track.
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
  import { fileName, formatTime, plural } from "$lib/format";
  import { adHocRule, library } from "$lib/state/library.svelte";
  import { player } from "$lib/state/player.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui, type MenuItem } from "$lib/state/ui.svelte";
  import Art from "./Art.svelte";
  import Icon from "./Icon.svelte";

  const FIRST = { artists: 6, albums: 12, tracks: 50 };
  const MORE = 50;

  let results = $state.raw<SearchResults | null>(null);
  let request = 0;

  $effect(() => {
    const query = library.query.trim();
    void library.version;
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

  function trackMenu(event: MouseEvent, track: Track) {
    const items: MenuItem[] = [
      { label: "Play", action: () => playTrack(track) },
      { label: "Play next", action: () => attempt(() => queue.add([track.id], true)) },
      { label: "Add to queue", action: () => attempt(() => queue.add([track.id], false)) },
    ];
    if (track.artistId !== null && track.artist !== null) {
      const artist = { id: track.artistId, name: track.artist };
      items.push({ label: "Go to artist", action: () => showArtist(artist) });
    }
    ui.openMenu(event, items);
  }

  function albumMenu(event: MouseEvent, album: AlbumHit) {
    const items: MenuItem[] = [
      { label: "Play", action: () => playAlbum(album) },
      { label: "Play next", action: () => attempt(() => queue.addNode(albumRule, [album.id], true, true)) },
      { label: "Add to queue", action: () => attempt(() => queue.addNode(albumRule, [album.id], true, false)) },
    ];
    if (album.albumArtistId !== null && album.albumArtist !== null) {
      const artist = { id: album.albumArtistId, name: album.albumArtist };
      items.push({ label: "Go to artist", action: () => showArtist(artist) });
    }
    ui.openMenu(event, items);
  }
</script>

<section class="results" aria-live="polite">
  {#if results === null}
    <p class="muted status">Searching…</p>
  {:else if results.artistTotal + results.albumTotal + results.trackTotal === 0}
    <p class="muted status">No results for “{library.query.trim()}”.</p>
  {:else}
    {#if results.artistTotal > 0}
      <h3>Artists <span class="muted">{results.artistTotal}</span></h3>
      <ul class="chips">
        {#each results.artists as artist (artist.id)}
          <li>
            <button class="chip" onclick={() => showArtist(artist)} ondblclick={() => playArtist(artist)}>
              <span class="name">{artist.name}</span>
              <span class="muted small">{plural(artist.trackCount, "track")}</span>
            </button>
          </li>
        {/each}
      </ul>
      {#if results.artists.length < results.artistTotal}
        <button class="link" onclick={() => more("artists")}>More artists</button>
      {/if}
    {/if}

    {#if results.albumTotal > 0}
      <h3>Albums <span class="muted">{results.albumTotal}</span></h3>
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
        <button class="link" onclick={() => more("albums")}>More albums</button>
      {/if}
    {/if}

    {#if results.trackTotal > 0}
      <h3>Tracks <span class="muted">{results.trackTotal}</span></h3>
      <ul class="tracks">
        {#each results.tracks as track (track.id)}
          <li>
            <button
              class="track"
              class:playing={player.currentItem?.trackId === track.id}
              onclick={() => playTrack(track)}
              oncontextmenu={(event) => trackMenu(event, track)}
            >
              <Art albumId={track.albumId} trackId={track.id} size="2.25rem" />
              <span class="text">
                <span class="name">{track.title ?? fileName(track.path)}</span>
                <span class="muted small">{[track.artist, track.album].filter(Boolean).join(" · ")}</span>
              </span>
              <span class="muted time">{formatTime(track.duration)}</span>
            </button>
          </li>
        {/each}
      </ul>
      {#if results.tracks.length < results.trackTotal}
        <button class="link" onclick={() => more("tracks")}>More tracks</button>
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

  .track:hover {
    background: var(--hover);
  }

  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
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

  .time {
    font-variant-numeric: tabular-nums;
  }

  .playing .name {
    color: var(--accent);
    font-weight: 600;
  }

  .link {
    margin-top: 0.5rem;
  }
</style>
