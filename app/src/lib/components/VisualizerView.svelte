<script lang="ts">
  // The visualizer: a visualization filling the main area (or, full screen,
  // the window), with the picker, the cover wall's year/artist switch, full
  // screen and close along the top, and the track along the bottom. The
  // controls fade out while the pointer rests.
  import { player } from "$lib/state/player.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import { visualizer } from "$lib/state/visualizer.svelte";
  import { VISUALIZATIONS, visualization } from "$lib/visualizer";
  import Icon from "./Icon.svelte";
  import Visualizer from "./Visualizer.svelte";

  const IDLE_MS = 2500;

  const item = $derived(player.currentItem);
  const chosen = $derived(visualization(visualizer.choice));
  let idle = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function wake() {
    idle = false;
    clearTimeout(timer);
    timer = setTimeout(() => (idle = true), IDLE_MS);
  }

  $effect(() => {
    wake();
    return () => clearTimeout(timer);
  });

  // Leaving the visualizer leaves full screen too.
  $effect(() => () => {
    if (visualizer.fullscreen) void visualizer.setFullscreen(false);
  });
</script>

<section
  class="visualizer"
  class:idle
  aria-label="Visualizer"
  onpointermove={wake}
  onpointerdown={wake}
  onfocusin={wake}
  ondblclick={() => visualizer.setFullscreen(!visualizer.fullscreen)}
>
  <Visualizer id={chosen.id} />

  <div class="controls top">
    <select
      aria-label="Visualization"
      title={chosen.description}
      value={chosen.id}
      onchange={(event) => visualizer.choose(event.currentTarget.value)}
      onkeydown={(event) => event.stopPropagation()}
    >
      {#each VISUALIZATIONS as option (option.id)}
        <option value={option.id}>{option.name}</option>
      {/each}
    </select>
    {#if chosen.id === "covers"}
      <div class="segmented" role="group" aria-label="Covers from">
        <button aria-pressed={visualizer.coverBasis === "year"} onclick={() => visualizer.setCoverBasis("year")}>
          Same year
        </button>
        <button aria-pressed={visualizer.coverBasis === "artist"} onclick={() => visualizer.setCoverBasis("artist")}>
          Same artist
        </button>
      </div>
    {/if}
    <span class="spacer"></span>
    <button
      class="icon"
      title={visualizer.fullscreen ? "Leave full screen (F)" : "Full screen (F)"}
      aria-label={visualizer.fullscreen ? "Leave full screen" : "Full screen"}
      aria-pressed={visualizer.fullscreen}
      onclick={() => visualizer.setFullscreen(!visualizer.fullscreen)}
    >
      <Icon name="expand" />
    </button>
    {#if !visualizer.fullscreen}
      <button class="icon" title="Close" aria-label="Close the visualizer" onclick={() => ui.back()}>
        <Icon name="close" />
      </button>
    {/if}
  </div>

  <div class="controls bottom">
    {#if item}
      <p class="title">{item.title}</p>
      <p class="small">{[item.artist, item.album].filter(Boolean).join(" · ")}</p>
    {:else}
      <p class="title">Not playing</p>
    {/if}
    {#if visualizer.caption}<p class="small caption">{visualizer.caption}</p>{/if}
    {#if visualizer.fullscreen}
      <p class="small hint">Space plays or pauses · V changes the visualization · Esc leaves full screen</p>
    {/if}
  </div>
</section>

<style>
  .visualizer {
    position: relative;
    height: 100%;
    overflow: hidden;
    background: rgb(9 9 12);
    color: #f2f2f5;
  }

  .visualizer.idle {
    cursor: none;
  }

  .controls {
    position: absolute;
    left: 0;
    right: 0;
    padding: 0.75rem 1rem;
    transition: opacity 0.4s;
  }

  .idle .controls {
    opacity: 0;
  }

  .controls:focus-within {
    opacity: 1;
  }

  .top {
    top: 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: linear-gradient(rgb(0 0 0 / 0.55), transparent);
  }

  .spacer {
    flex: 1;
  }

  .bottom {
    bottom: 0;
    padding-top: 2.5rem;
    background: linear-gradient(transparent, rgb(0 0 0 / 0.6));
    pointer-events: none;
  }

  p {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    font-size: 1.15rem;
    font-weight: 600;
  }

  .small {
    font-size: 0.85rem;
    color: rgb(242 242 245 / 0.7);
  }

  .caption {
    margin-top: 0.3rem;
  }

  .hint {
    margin-top: 0.3rem;
    color: rgb(242 242 245 / 0.45);
  }

  /* Controls on the dark stage, whatever the app's theme. */
  select,
  .top button {
    color: #f2f2f5;
    background: rgb(255 255 255 / 0.08);
    border-color: rgb(255 255 255 / 0.16);
  }

  .top button:hover:not(:disabled) {
    background: rgb(255 255 255 / 0.16);
  }

  select option {
    color: initial;
    background: initial;
  }

  .top button.icon {
    background: none;
  }

  .segmented {
    display: inline-flex;
  }

  .segmented button {
    border-radius: 0;
    padding: 0.3rem 0.7rem;
  }

  .segmented button:first-child {
    border-radius: 6px 0 0 6px;
  }

  .segmented button:last-child {
    border-radius: 0 6px 6px 0;
    border-left: none;
  }

  .segmented button[aria-pressed="true"] {
    background: rgb(255 255 255 / 0.85);
    color: #111114;
  }

  @media (max-width: 640px) {
    .controls {
      padding: 0.5rem 0.75rem;
    }

    .segmented button {
      padding: 0.3rem 0.5rem;
      font-size: 0.85rem;
    }
  }
</style>
