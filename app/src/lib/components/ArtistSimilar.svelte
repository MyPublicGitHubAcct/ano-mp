<script lang="ts">
  // The artist page's "Similar artists" (PLAN.md X7): first the library's
  // artists most like this one (X4's scoring), each opening its own page;
  // then, while X5 is on, artists outside the library as links out. A row
  // of eight of each, "More" showing the rest. Hidden while there are none.
  // The library half reloads with the library; the outside half, which
  // may wait on ListenBrainz, only for another artist, and apart from
  // the library half, so it never holds that back.
  import { untrack } from "svelte";
  import { features as api, outside, type OutsideArtist, type SimilarArtist } from "$lib/api";
  import { t } from "$lib/i18n";
  import { reasonsText } from "$lib/similar";
  import { library } from "$lib/state/library.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import OutsideArtists from "./OutsideArtists.svelte";

  /** How many of each half show before "More". */
  const ROW = 8;

  let { artistId }: { artistId: number } = $props();

  let artists = $state.raw<SimilarArtist[]>([]);
  let outsideArtists = $state.raw<OutsideArtist[]>([]);
  let expanded = $state(false);
  let request = 0;
  let outsideRequest = 0;

  const f = $derived(appSettings.current.features);
  const withOutside = $derived(f.similarArtists && f.outsideRecommendations);

  const quiet = <T,>(promise: Promise<T>) => promise.catch(() => null);

  $effect(() => {
    const id = artistId;
    const on = f.similarArtists;
    void library.version;
    untrack(async () => {
      const current = ++request;
      const found = on ? ((await quiet(api.artistPageSimilar(id))) ?? []) : [];
      if (current === request) artists = found;
    });
  });

  $effect(() => {
    const id = artistId;
    const on = withOutside;
    untrack(async () => {
      const current = ++outsideRequest;
      expanded = false;
      outsideArtists = [];
      if (!on) return;
      const found = (await quiet(outside.likeArtist(id))) ?? [];
      if (current === outsideRequest) outsideArtists = found;
    });
  });

  const shown = $derived(expanded ? artists : artists.slice(0, ROW));
  const outsideShown = $derived(expanded ? outsideArtists : outsideArtists.slice(0, ROW));
  const more = $derived(artists.length > ROW || outsideArtists.length > ROW);
</script>

{#if artists.length > 0 || outsideArtists.length > 0}
  <section class="similar" aria-labelledby="similar-heading">
    <h2 id="similar-heading">{t("artist.similar")}</h2>
    {#if shown.length > 0}
      <ul class="artists" aria-label={t("artist.similar")}>
        {#each shown as artist (artist.artistId)}
          <li>
            <button class="link name" onclick={() => ui.showArtist({ id: artist.artistId, name: artist.name })}
              >{artist.name}</button
            >
            <span class="note small">{reasonsText(artist.reasons, t)}</span>
          </li>
        {/each}
      </ul>
    {/if}
    {#if outsideShown.length > 0}
      <h3>{t("outside.title")}</h3>
      <OutsideArtists
        artists={outsideShown}
        label={t("outside.title")}
        ondismissed={(mbid) => (outsideArtists = outsideArtists.filter((artist) => artist.mbid !== mbid))}
      />
    {/if}
    {#if more}
      <button class="link toggle" onclick={() => (expanded = !expanded)}
        >{t(expanded ? "artist.similarFewer" : "artist.similarMore")}</button
      >
    {/if}
  </section>
{/if}

<style>
  .similar {
    margin-bottom: 2rem;
  }

  h2 {
    font-size: 1rem;
    margin: 0 0 0.6rem;
  }

  h3 {
    font-size: 0.9rem;
    margin: 0.9rem 0 0.5rem;
    color: var(--text-muted);
  }

  .artists {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(14rem, 1fr));
    gap: 0.5rem;
  }

  li {
    display: flex;
    flex-direction: column;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    min-width: 0;
  }

  .name {
    text-align: left;
    font-weight: 600;
  }

  .name,
  .note {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .note {
    color: var(--text-muted);
  }

  .small {
    font-size: 0.8rem;
  }

  .toggle {
    margin-top: 0.6rem;
  }
</style>
