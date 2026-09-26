<script lang="ts">
  // The current track alone: its title, artist and album on the left and its
  // cover, as large as the area allows, on the right. Narrow windows put the
  // cover on top.
  import { player } from "$lib/state/player.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import Art from "./Art.svelte";
  import Icon from "./Icon.svelte";

  const item = $derived(player.currentItem);
</script>

<section class="now-playing" aria-label="Now playing">
  <div class="details">
    {#if item}
      <p class="state muted small">{player.playing ? "Now playing" : "Paused"}</p>
      <h1 title={item.title}>{item.title}</h1>
      {#if item.artist}<p class="artist">{item.artist}</p>{/if}
      {#if item.album}<p class="album muted">{item.album}</p>{/if}
      {#if player.current !== null && player.items.length > 1}
        <p class="position muted small">
          {(player.current + 1).toLocaleString()} of {player.items.length.toLocaleString()} in the queue
        </p>
      {/if}
    {:else}
      <h1 class="muted">Not playing</h1>
      <p class="muted">Double-click a track in the library to play it.</p>
    {/if}
    <button class="back link" onclick={() => ui.leaveNowPlaying()}>
      <Icon name="close" size="1rem" /> Close
    </button>
  </div>
  <div class="cover">
    <div class="frame">
      <Art albumId={item?.albumId ?? null} trackId={item?.trackId ?? null} size="100%" />
    </div>
  </div>
</section>

<style>
  .now-playing {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(12rem, 1fr) minmax(0, 1.6fr);
    gap: 2rem;
    padding: 1.5rem 2rem 2rem;
    overflow: auto;
  }

  .details {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.35rem;
    min-width: 0;
  }

  p {
    margin: 0;
  }

  h1 {
    margin: 0 0 0.25rem;
    font-size: clamp(1.5rem, 3vw, 2.4rem);
    line-height: 1.15;
    overflow-wrap: anywhere;
  }

  .state {
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .artist {
    font-size: 1.2rem;
    font-weight: 600;
  }

  .album {
    font-size: 1.05rem;
  }

  .small {
    font-size: 0.8rem;
  }

  .position {
    margin-top: 0.75rem;
  }

  .back {
    align-self: flex-start;
    margin-top: 1.25rem;
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
  }

  /* The cover: a square as large as the pane's width and height allow. */
  .cover {
    min-width: 0;
    min-height: 0;
    container-type: size;
  }

  .frame {
    width: min(100cqw, 100cqh);
    aspect-ratio: 1;
    margin-left: auto;
    border-radius: 6px;
    overflow: hidden;
    box-shadow: var(--shadow);
  }

  .frame :global(.art) {
    border-radius: 0;
  }

  @media (max-width: 640px) {
    .now-playing {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(12rem, 1fr) auto;
      gap: 1.25rem;
      padding: 1rem;
    }

    .cover {
      order: -1;
    }

    .frame {
      margin: 0 auto;
    }

    .details {
      justify-content: flex-start;
      text-align: center;
      align-items: center;
    }

    .back {
      align-self: center;
    }
  }
</style>
