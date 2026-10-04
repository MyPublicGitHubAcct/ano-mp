<script lang="ts">
  // "More like this" (PLAN.md X4): tracks like a track on other albums,
  // albums like an album, or artists like an artist, from the library
  // alone, each saying why. Opening an album or artist closes the dialog.
  // For an artist, X5 adds artists outside the library while it is on.
  import { t } from "$lib/i18n";
  import { formatTime } from "$lib/format";
  import {
    features as api,
    outside,
    queue,
    type OutsideArtist,
    type SimilarAlbum,
    type SimilarArtist,
    type SimilarTrack,
  } from "$lib/api";
  import { reasonsText } from "$lib/similar";
  import { appSettings } from "$lib/state/settings.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui, type SimilarSeed } from "$lib/state/ui.svelte";
  import AlbumCards from "./AlbumCards.svelte";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";
  import OutsideArtists from "./OutsideArtists.svelte";

  let { seed, onclose }: { seed: SimilarSeed; onclose: () => void } = $props();

  let tracks = $state.raw<SimilarTrack[] | null>(null);
  let albums = $state.raw<SimilarAlbum[] | null>(null);
  let artists = $state.raw<SimilarArtist[] | null>(null);
  let outsideArtists = $state.raw<OutsideArtist[]>([]);
  let outsideLoading = $state(false);

  const on = $derived(appSettings.current.features);

  $effect(() => {
    const { kind, id } = seed;
    if (kind === "track") attempt(async () => (tracks = await api.similarTracks(id)));
    else if (kind === "album") attempt(async () => (albums = await api.similarAlbums(id)));
    else {
      if (on.recommendations) attempt(async () => (artists = await api.similarArtists(id)));
      else artists = [];
      if (on.outsideRecommendations) {
        outsideLoading = true;
        attempt(async () => (outsideArtists = await outside.likeArtist(id))).finally(() => (outsideLoading = false));
      }
    }
  });

  const found = $derived(tracks ?? albums ?? artists);
  const ids = $derived((tracks ?? []).map((track) => track.trackId));
  const cards = $derived((albums ?? []).map(({ album, reasons }) => ({ ...album, note: reasonsText(reasons, t) })));

  function showArtist(artist: SimilarArtist) {
    ui.showArtist({ id: artist.artistId, name: artist.name });
    onclose();
  }
</script>

<Dialog title={t("similar.titleFor", { name: seed.name })} {onclose}>
  {#if found === null}
    <p class="muted">{t("common.loading")}</p>
  {:else if found.length === 0 && outsideArtists.length === 0 && !outsideLoading}
    <p class="muted">{t("similar.none")}</p>
  {:else if tracks}
    <ul class="rows" aria-label={t("similar.title")}>
      {#each tracks as track, index (track.trackId)}
        <li>
          <button
            class="icon"
            title={t("library.playName", { name: track.title })}
            aria-label={t("library.playName", { name: track.title })}
            onclick={() => attempt(() => queue.play(ids, index))}><Icon name="play" size="0.9rem" /></button
          >
          <span class="text">
            <span class="title">{track.title}</span>
            <span class="muted small">{[track.artist, track.album].filter((part) => part !== null).join(" · ")}</span>
          </span>
          <span class="note small">{reasonsText(track.reasons, t)}</span>
          <span class="muted small time">{formatTime(track.duration)}</span>
        </li>
      {/each}
    </ul>
  {:else if albums}
    <AlbumCards albums={cards} wrap label={t("similar.title")} onopen={onclose} />
  {:else if artists}
    <ul class="rows" aria-label={t("similar.title")}>
      {#each artists as artist (artist.artistId)}
        <li>
          <span class="text">
            <button class="link title" onclick={() => showArtist(artist)}>{artist.name}</button>
          </span>
          <span class="note small">{reasonsText(artist.reasons, t)}</span>
        </li>
      {/each}
    </ul>
  {/if}
  {#if seed.kind === "artist" && (outsideLoading || outsideArtists.length > 0)}
    <h3 class="outside">{t("outside.title")}</h3>
    {#if outsideArtists.length > 0}
      <OutsideArtists
        artists={outsideArtists}
        label={t("outside.title")}
        ondismissed={(mbid) => (outsideArtists = outsideArtists.filter((artist) => artist.mbid !== mbid))}
      />
    {:else}
      <p class="muted">{t("common.loading")}</p>
    {/if}
  {/if}
  {#snippet actions()}
    {#if tracks && tracks.length > 0}
      <button onclick={() => attempt(() => queue.add(ids, false))}>{t("similar.addAll")}</button>
      <span class="spacer"></span>
      <button class="primary" onclick={() => attempt(() => queue.play(ids, 0))}>{t("similar.playAll")}</button>
    {:else}
      <span class="spacer"></span>
      <button onclick={onclose}>{t("dialog.close")}</button>
    {/if}
  {/snippet}
</Dialog>

<style>
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .rows li {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.3rem 0;
    border-bottom: 1px solid var(--border);
  }

  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .title,
  .text .muted {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .link.title {
    text-align: left;
  }

  .note {
    color: var(--text-muted);
    text-align: right;
  }

  .time {
    font-variant-numeric: tabular-nums;
  }

  .spacer {
    flex: 1;
  }

  h3.outside {
    margin: 0.9rem 0 0.4rem;
    font-size: 0.95rem;
  }
</style>
