<script lang="ts">
  // "More in <genre>" (O18): five random albums sharing a genre with this
  // one, the same five while the page is open; a chip per genre switches
  // the row, and "Draw again" picks a new five. Albums played lately are
  // drawn less often.
  import { untrack } from "svelte";
  import { features as api, type AlbumCard } from "$lib/api";
  import AlbumCards from "./AlbumCards.svelte";

  let { albumId, genres }: { albumId: number; genres: string[] } = $props();

  let genre = $state(untrack(() => genres[0] ?? ""));
  let seed = $state(Math.floor(Math.random() * 2 ** 31));
  let albums = $state.raw<AlbumCard[]>([]);

  $effect(() => {
    const [id, chosen, draw] = [albumId, genre, seed];
    untrack(async () => {
      if (!chosen) return;
      const drawn = await api.moreInGenre(id, chosen, draw).catch(() => []);
      if (id === albumId && chosen === genre && draw === seed) albums = drawn;
    });
  });
</script>

{#if genres.length > 0 && (albums.length > 0 || genres.length > 1)}
  <section class="more" aria-label="More in {genre}">
    <div class="head">
      <h3>More in {genre}</h3>
      {#if genres.length > 1}
        <div class="chips" role="radiogroup" aria-label="Genre">
          {#each genres as option (option)}
            <button role="radio" aria-checked={option === genre} class:on={option === genre} onclick={() => (genre = option)}>
              {option}
            </button>
          {/each}
        </div>
      {/if}
      <button class="link" onclick={() => (seed = Math.floor(Math.random() * 2 ** 31))}>Draw again</button>
    </div>
    {#if albums.length > 0}
      <AlbumCards {albums} label="More in {genre}" />
    {:else}
      <p class="muted">No other albums in {genre}.</p>
    {/if}
  </section>
{/if}

<style>
  .more {
    margin-top: 0.75rem;
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 1rem;
  }

  h3 {
    margin: 0;
    font-size: 0.95rem;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .chips button {
    font-size: 0.78rem;
    padding: 0.1rem 0.55rem;
    border-radius: 999px;
  }

  .chips button.on {
    background: var(--selected);
    color: var(--accent);
    border-color: var(--accent);
  }
</style>
