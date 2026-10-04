<script lang="ts">
  // Everything hearted (PLAN.md F3): artists, albums and tracks, newest
  // first. Tracks can be selected, played, dragged and unhearted like any
  // list's.
  import { untrack } from "svelte";
  import { marks, queue, type Favourites, type Track } from "$lib/api";
  import { count, t } from "$lib/i18n";
  import { fileName } from "$lib/format";
  import { emptySelection, rowsFor, type Selection } from "$lib/selection";
  import { appearance } from "$lib/state/appearance.svelte";
  import { collection } from "$lib/state/collection.svelte";
  import { library } from "$lib/state/library.svelte";
  import { player } from "$lib/state/player.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import { dragLabel, trackMenu } from "$lib/trackMenu";
  import AlbumCards from "./AlbumCards.svelte";
  import Heart from "./Heart.svelte";
  import Icon from "./Icon.svelte";
  import TrackText from "./TrackText.svelte";
  import VirtualList from "./VirtualList.svelte";

  let favourites = $state.raw<Favourites | null>(null);
  let selection = $state<Selection>(emptySelection);

  $effect(() => {
    void [collection.version, library.version];
    untrack(async () => {
      favourites = (await attempt(marks.favourites)) ?? favourites;
      selection = emptySelection;
    });
  });

  const tracks = $derived(favourites?.tracks ?? []);
  const empty = $derived(
    favourites !== null && tracks.length === 0 && favourites.albums.length === 0 && favourites.artists.length === 0,
  );

  const playFrom = (index: number) =>
    attempt(() =>
      queue.play(
        tracks.map((track) => track.id),
        index,
      ),
    );

  function menu(index: number, event: MouseEvent) {
    const chosen = rowsFor(selection, index)
      .map((row) => tracks[row])
      .filter(Boolean);
    ui.openMenu(event, trackMenu(chosen, chosen.length === 1 ? () => playFrom(index) : undefined));
  }

  function dragRows(rows: number[]) {
    const chosen: Track[] = rows.map((row) => tracks[row]).filter(Boolean);
    if (chosen.length === 0) return null;
    const ids = chosen.map((track) => track.id);
    return {
      payload: { kind: "tracks" as const, trackIds: async () => ids },
      label: chosen.length === 1 ? (chosen[0].title ?? fileName(chosen[0].path)) : dragLabel(chosen.length),
    };
  }

  $effect(() => {
    ui.selectedTracks = () =>
      rowsFor(selection, selection.focus)
        .map((row) => tracks[row])
        .filter(Boolean);
    return () => (ui.selectedTracks = null);
  });
</script>

<section class="view">
  <header>
    <h2>{t("favourites.title")}</h2>
    <button class="primary" disabled={tracks.length === 0} onclick={() => playFrom(0)}>
      <Icon name="play" />
      {t("favourites.playTracks")}
    </button>
  </header>

  {#if empty}
    <div class="empty">
      <Icon name="heartOutline" size="2.5rem" />
      <p>{t("favourites.empty")}</p>
    </div>
  {:else if favourites}
    <div class="scroll">
      {#if favourites.artists.length > 0}
        <h3>{count("count.artists", favourites.artists.length)}</h3>
        <ul class="artists">
          {#each favourites.artists as artist (artist.id)}
            <li>
              <button class="artist" onclick={() => ui.showArtist({ id: artist.id, name: artist.name })}>
                <Icon name="person" />
                <span class="name">{artist.name}</span>
                <span class="muted small">{count("count.tracks", artist.trackCount)}</span>
              </button>
              <Heart on label={artist.name} onchange={(on) => collection.setFavourite("artist", [artist.id], on)} />
            </li>
          {/each}
        </ul>
      {/if}
      {#if favourites.albums.length > 0}
        <h3>{count("count.albums", favourites.albums.length)}</h3>
        <AlbumCards albums={favourites.albums} label={t("favourites.albums")} wrap />
      {/if}
      {#if tracks.length > 0}
        <h3>{count("count.tracks", tracks.length)}</h3>
        <div class="tracks" style:height="{Math.min(tracks.length, 12) * 40 + 2}px">
          <VirtualList
            count={tracks.length}
            rowHeight={appearance.rowHeight(40)}
            item={(index) => tracks[index]}
            label={t("favourites.tracks")}
            multiple
            bind:selection
            onactivate={playFrom}
            oncontextmenu={menu}
            ondragrow={dragRows}
          >
            {#snippet row(track: Track | undefined, index: number)}
              {#if track}
                {@const name = track.title ?? fileName(track.path)}
                <div class="entry" class:playing={player.currentItem?.trackId === track.id}>
                  <TrackText {track} />
                  <Heart on label={name} onchange={(on) => collection.setFavourite("track", [track.id], on)} />
                  <button
                    class="icon"
                    title={t("library.more")}
                    aria-label={t("library.moreFor", { name })}
                    onclick={(event) => {
                      event.stopPropagation();
                      menu(index, event);
                    }}><Icon name="more" /></button
                  >
                </div>
              {/if}
            {/snippet}
          </VirtualList>
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .view {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.75rem 1rem;
  }

  h2 {
    margin: 0;
    font-size: 1.35rem;
  }

  h3 {
    font-size: 0.95rem;
    margin: 1rem 1rem 0.5rem;
    color: var(--text-muted);
  }

  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 1rem;
  }

  .artists {
    list-style: none;
    margin: 0;
    padding: 0 1rem;
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .artists li {
    display: flex;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
  }

  .artist {
    border: none;
    background: none;
  }

  .small {
    font-size: 0.8rem;
  }

  .tracks {
    container-type: inline-size;
    margin: 0 0.5rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    max-height: 60vh;
  }

  .entry {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    height: 100%;
    padding: 0 0.5rem 0 1rem;
    min-width: 0;
  }

  .playing :global(.name) {
    color: var(--accent);
    font-weight: 600;
  }

  .empty {
    display: grid;
    place-items: center;
    align-content: center;
    gap: 1rem;
    flex: 1;
    color: var(--text-muted);
    text-align: center;
    padding: 1rem;
  }

  .empty p {
    margin: 0;
    max-width: 28rem;
  }
</style>
