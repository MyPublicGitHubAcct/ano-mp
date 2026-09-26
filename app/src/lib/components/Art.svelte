<script lang="ts">
  // Cover art for an album (or an album-less track), with a placeholder when
  // there is none. Loaded lazily from the `anomp-art` scheme.
  import { artUrl } from "$lib/api";
  import { library } from "$lib/state/library.svelte";
  import Icon from "./Icon.svelte";

  let {
    albumId = null,
    trackId = null,
    size = "3rem",
  }: { albumId?: number | null; trackId?: number | null; size?: string } = $props();

  const src = $derived(
    albumId !== null
      ? artUrl({ albumId }, library.version, library.artVersions.get(albumId))
      : trackId !== null
        ? artUrl({ trackId }, library.version)
        : null,
  );
  let failed = $state<string | null>(null);
</script>

<div class="art" style:width={size} style:height={size}>
  {#if src !== null && failed !== src}
    <img {src} alt="" loading="lazy" decoding="async" onerror={() => (failed = src)} />
  {:else}
    <Icon name="note" size="45%" />
  {/if}
</div>

<style>
  .art {
    flex: none;
    display: grid;
    place-items: center;
    overflow: hidden;
    border-radius: 4px;
    background: var(--surface-2);
    color: var(--text-faint);
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
