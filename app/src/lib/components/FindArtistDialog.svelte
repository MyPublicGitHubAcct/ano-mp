<script lang="ts">
  // "Find artist": the MusicBrainz artists that could be this one, to pick
  // the right one (names are shared: "Nirvana" is several bands), say it's
  // none of them, or go back to automatic matching. The search starts with
  // the artist's own name; a MusicBrainz artist link or id is looked up
  // directly.
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { metadata, type ArtistCandidate, type ArtistInfo } from "$lib/api";
  import { lifeSpan } from "$lib/format";
  import { errorText, t } from "$lib/i18n";
  import { attempt, toasts } from "$lib/state/toasts.svelte";
  import type { ArtistRef } from "$lib/state/ui.svelte";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  let { artist, info, onclose }: { artist: ArtistRef; info: ArtistInfo; onclose: () => void } = $props();

  let candidates = $state.raw<ArtistCandidate[] | null>(null);
  let name = $state("");
  let searching = $state(false);
  let busy = $state(false);
  let selected = $state<string | null>(null);
  let request = 0;

  const current = $derived(info.musicbrainz?.id ?? null);

  onMount(() => {
    name = artist.name;
    search(false);
  });

  async function search(typed: boolean) {
    const at = ++request;
    searching = true;
    try {
      const found = await metadata.artistCandidates(artist.id, typed ? name : null);
      if (at !== request) return;
      candidates = found;
      selected = found.some((c) => c.artist.id === current) ? current : null;
    } catch (error) {
      if (at === request) {
        candidates = [];
        toasts.show(errorText(error));
      }
    } finally {
      if (at === request) searching = false;
    }
  }

  function describe({ artist }: ArtistCandidate) {
    return [artist.type, artist.area, lifeSpan(artist) || null].filter(Boolean).join(" · ");
  }

  async function act(action: () => Promise<unknown>, done: string) {
    busy = true;
    try {
      await action();
      toasts.show(done, "info", 3000);
      onclose();
    } catch (error) {
      toasts.show(errorText(error));
    } finally {
      busy = false;
    }
  }

  const choose = () => {
    const mbid = selected;
    if (mbid) act(() => metadata.chooseArtist(artist.id, mbid), t("findArtist.updated", { name: artist.name }));
  };

  function openLink(event: MouseEvent) {
    event.preventDefault();
    const href = (event.currentTarget as HTMLAnchorElement).href;
    attempt(() => openUrl(href));
  }
</script>

<Dialog title={t("findArtist.title", { name: artist.name })} {onclose}>
  <form
    class="search"
    onsubmit={(event) => {
      event.preventDefault();
      search(true);
    }}
  >
    <label>
      <span class="muted small">{t("findArtist.query")}</span>
      <input type="text" bind:value={name} />
    </label>
    <button type="submit" disabled={searching || name.trim() === ""}><Icon name="search" /> {t("findArtist.search")}</button>
  </form>

  {#if candidates === null}
    <p class="muted" aria-live="polite">{t("findArtist.searching")}</p>
  {:else if candidates.length === 0}
    <p class="muted">{t(searching ? "findArtist.searching" : "findArtist.none")}</p>
  {:else}
    <ul class="candidates" aria-busy={searching}>
      {#each candidates as candidate (candidate.artist.id)}
        {@const found = candidate.artist}
        <li>
          <button
            class="candidate"
            class:selected={selected === found.id}
            aria-pressed={selected === found.id}
            onclick={() => (selected = found.id)}
          >
            <span class="line">
              <span class="name">
                {found.name}{#if found.disambiguation}{" "}<span class="muted">({found.disambiguation})</span>{/if}
              </span>
              <span class="score muted" title={t("findArtist.score")}>{candidate.score}</span>
            </span>
            {#if describe(candidate)}<span class="muted small">{describe(candidate)}</span>{/if}
            {#if found.id === current}
              <span class="badges"><span class="badge">{t(info.chosenByUser ? "cover.yourChoice" : "findArtist.current")}</span></span>
            {/if}
          </button>
          {#if selected === found.id}
            <a class="small more" href="https://musicbrainz.org/artist/{found.id}" onclick={openLink}>{t("findArtist.open")}</a>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#snippet actions()}
    {#if info.status !== null}
      <button
        onclick={() => act(() => metadata.useAutomaticArtist(artist.id), t("findArtist.again", { name: artist.name }))}
        disabled={busy}
        title={t("findArtist.automaticHint")}>{t("cover.useAutomatic")}</button
      >
    {/if}
    {#if !(info.status === "none" && info.chosenByUser)}
      <button
        onclick={() => act(() => metadata.rejectArtist(artist.id), t("findArtist.rejected", { name: artist.name }))}
        disabled={busy}>{t("findArtist.noneOfThese")}</button
      >
    {/if}
    <span class="spacer"></span>
    <button onclick={onclose} disabled={busy}>{t("dialog.cancel")}</button>
    <button
      class="primary"
      onclick={choose}
      disabled={busy || selected === null || (selected === current && info.chosenByUser)}>{t("findArtist.use")}</button
    >
  {/snippet}
</Dialog>

<style>
  p {
    margin: 0.25rem 0;
  }

  .small {
    font-size: 0.8rem;
  }

  .search {
    display: flex;
    align-items: flex-end;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
  }

  .search label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    flex: 1;
    min-width: 0;
  }

  .candidates {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .candidates[aria-busy="true"] {
    opacity: 0.6;
  }

  .candidate {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.1rem;
    width: 100%;
    padding: 0.5rem 0.75rem;
    text-align: left;
  }

  .candidate.selected {
    border-color: var(--accent);
    background: var(--selected);
  }

  .line {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
  }

  .name {
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .score {
    flex: none;
    font-variant-numeric: tabular-nums;
  }

  .badges {
    display: flex;
    margin-top: 0.2rem;
  }

  .badge {
    font-size: 0.72rem;
    padding: 0.05rem 0.45rem;
    border-radius: 999px;
    background: var(--accent);
    color: var(--accent-text);
  }

  .more {
    display: inline-block;
    margin: 0.3rem 0.75rem 0;
  }

  a {
    color: var(--accent);
    text-decoration: none;
  }

  a:hover {
    text-decoration: underline;
  }

  .spacer {
    flex: 1;
  }
</style>
