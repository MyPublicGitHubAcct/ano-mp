<script lang="ts">
  // "Choose cover": the pictures each album-art source has for an album, in
  // the configured order, to pick one as its cover or go back to the first
  // found in that order. Local pictures show at once; the Cover Art
  // Archive's small previews are downloaded one at a time through the
  // metadata worker, so the window never contacts the service itself.
  import { onMount } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import { candidateArtUrl, metadata, type AlbumDetails, type CoverCandidate, type CoverChoices } from "$lib/api";
  import { errorText, t } from "$lib/i18n";
  import { attempt, toasts } from "$lib/state/toasts.svelte";
  import type { AlbumRef } from "$lib/state/ui.svelte";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  let { album, onclose }: { album: AlbumRef; onclose: () => void } = $props();

  type Preview = "loading" | "ready" | "missing" | "failed";

  let choices = $state.raw<CoverChoices | null>(null);
  let details = $state.raw<AlbumDetails | null>(null);
  /** Archive previews by URL. */
  const previews = new SvelteMap<string, Preview>();
  /** Pixel sizes of the pictures shown, by candidate. */
  const sizes = new SvelteMap<string, string>();
  let selected = $state<string | null>(null);
  let busy = $state(false);
  let open = true;

  const keyOf = (candidate: Pick<CoverCandidate, "source" | "reference">) =>
    `${candidate.source}\n${candidate.reference ?? ""}`;

  onMount(() => {
    attempt(async () => (details = await metadata.album(album.id)));
    attempt(async () => {
      const loaded = await metadata.coverCandidates(album.id);
      choices = loaded;
      if (loaded.chosen) selected = keyOf(loaded.chosen);
      await fetchPreviews(loaded);
    });
    return () => (open = false);
  });

  /** Downloads the archive's previews in order, until the dialog closes. */
  async function fetchPreviews(loaded: CoverChoices) {
    const urls = loaded.sources
      .filter((source) => source.source === "cover-art-archive")
      .flatMap((source) => source.candidates)
      .map((candidate) => candidate.preview)
      .filter((url) => url !== null);
    for (const url of urls) previews.set(url, "loading");
    for (const url of urls) {
      if (!open) return;
      try {
        previews.set(url, (await metadata.fetchImage(url)) ? "ready" : "missing");
      } catch {
        previews.set(url, "failed");
      }
    }
  }

  function previewState(candidate: CoverCandidate): Preview {
    if (candidate.source !== "cover-art-archive" || candidate.preview === null) return "ready";
    return previews.get(candidate.preview) ?? "loading";
  }

  const isChosen = (candidate: CoverCandidate) =>
    choices?.chosen?.source === candidate.source && choices.chosen.reference === candidate.reference;

  const selectedCandidate = $derived(
    choices?.sources.flatMap((source) => source.candidates).find((candidate) => keyOf(candidate) === selected) ?? null,
  );

  function onload(event: Event, candidate: CoverCandidate) {
    const img = event.currentTarget as HTMLImageElement;
    // The archive's preview is a thumbnail; its size says nothing.
    if (candidate.source !== "cover-art-archive")
      sizes.set(keyOf(candidate), t("cover.pixels", { width: img.naturalWidth, height: img.naturalHeight }));
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
    const candidate = selectedCandidate;
    if (candidate)
      act(
        () => metadata.chooseCover(album.id, candidate.source, candidate.reference),
        t("cover.changed", { title: album.title }),
      );
  };
  const automatic = () =>
    act(() => metadata.useAutomaticCover(album.id), t("cover.automaticDone", { title: album.title }));
</script>

<Dialog title={t("album.chooseCoverFor", { title: album.title })} {onclose}>
  {#if details}
    <p class="muted small">
      {#if details.cover}
        {t(details.cover.chosen ? "cover.showingChoice" : "cover.showing", { source: details.cover.sourceName })}
      {:else}
        {t("cover.noneYet")}
      {/if}
      {t("cover.hint")}
    </p>
  {/if}

  {#if choices === null}
    <p class="muted" aria-live="polite">{t("cover.looking")}</p>
  {:else if choices.sources.length === 0}
    <p class="note">{t("cover.allOff")}</p>
  {:else}
    {#each choices.sources as source (source.source)}
      <section aria-label={source.sourceName}>
        <h3>{source.sourceName}</h3>
        {#if source.note}
          <p class="note">{source.note}</p>
        {:else if source.candidates.length === 0}
          <p class="muted small">{t("cover.noPictures")}</p>
        {/if}
        <ul class="tiles">
          {#each source.candidates as candidate (keyOf(candidate))}
            {@const key = keyOf(candidate)}
            {@const state = previewState(candidate)}
            <li>
              <button
                class="tile"
                class:selected={selected === key}
                aria-pressed={selected === key}
                disabled={state === "missing" || state === "failed"}
                onclick={() => (selected = key)}
              >
                <span class="picture">
                  {#if state === "ready"}
                    <img
                      src={candidateArtUrl(album.id, candidate)}
                      alt=""
                      onload={(event) => onload(event, candidate)}
                    />
                  {:else}
                    <span class="muted small">
                      {t(
                        state === "loading" ? "common.loading" : state === "missing" ? "cover.missing" : "cover.failed",
                      )}
                    </span>
                  {/if}
                  {#if isChosen(candidate)}<span class="badge"
                      ><Icon name="check" size="0.9rem" /> {t("cover.yourChoice")}</span
                    >{/if}
                </span>
                <span class="label" title={candidate.label}>{candidate.label}</span>
                {#if candidate.detail || sizes.has(key)}
                  <span class="muted small detail" title={candidate.detail ?? undefined}>
                    {[sizes.get(key), candidate.detail].filter(Boolean).join(" · ")}
                  </span>
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  {/if}

  {#snippet actions()}
    {#if choices?.chosen}
      <button onclick={automatic} disabled={busy}>{t("cover.useAutomatic")}</button>
    {/if}
    <span class="spacer"></span>
    <button onclick={onclose} disabled={busy}>{t("dialog.cancel")}</button>
    <button
      class="primary"
      onclick={choose}
      disabled={busy || selectedCandidate === null || isChosen(selectedCandidate)}>{t("cover.use")}</button
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

  .note {
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    background: var(--surface-2);
  }

  h3 {
    margin: 1rem 0 0.4rem;
    font-size: 0.95rem;
  }

  .tiles {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(8.5rem, 1fr));
    gap: 0.75rem;
  }

  .tile {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.2rem;
    width: 100%;
    padding: 0.35rem;
    text-align: left;
    border-color: transparent;
    background: none;
  }

  .tile.selected {
    border-color: var(--accent);
    background: var(--selected);
  }

  .picture {
    position: relative;
    display: grid;
    place-items: center;
    aspect-ratio: 1;
    border-radius: 4px;
    overflow: hidden;
    background: var(--surface-2);
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .badge {
    position: absolute;
    left: 0.3rem;
    bottom: 0.3rem;
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
    font-size: 0.72rem;
    padding: 0.1rem 0.45rem;
    border-radius: 999px;
    background: var(--accent);
    color: var(--accent-text);
  }

  .label,
  .detail {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .spacer {
    flex: 1;
  }
</style>
