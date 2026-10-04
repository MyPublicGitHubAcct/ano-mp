<script lang="ts">
  // "More like this" on an album page (PLAN.md X4): the library's albums
  // most like this one, each saying why; hidden while there are none, and
  // folded away until opened.
  import { t } from "$lib/i18n";
  import { untrack } from "svelte";
  import { features as api, type SimilarAlbum } from "$lib/api";
  import { reasonsText } from "$lib/similar";
  import { library } from "$lib/state/library.svelte";
  import AlbumCards from "./AlbumCards.svelte";
  import Fold from "./Fold.svelte";

  let { albumId }: { albumId: number } = $props();

  let similar = $state.raw<SimilarAlbum[]>([]);

  $effect(() => {
    const id = albumId;
    // Folders coming and going change which albums can be suggested (H22b).
    void [library.version, library.unreadableKey];
    untrack(async () => {
      const found = await api.similarAlbums(id).catch(() => []);
      if (id === albumId) similar = found;
    });
  });

  const cards = $derived(similar.map(({ album, reasons }) => ({ ...album, note: reasonsText(reasons, t) })));
</script>

{#if cards.length > 0}
  <section>
    <Fold key="album.similar" heading={t("similar.title")} level={3}>
      <AlbumCards albums={cards} label={t("similar.title")} />
    </Fold>
  </section>
{/if}
