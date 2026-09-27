<script lang="ts">
  // A small panel that opens above the playing bar from a button in it;
  // a click outside or Escape closes it.
  import type { Snippet } from "svelte";

  let { title, onclose, children }: { title: string; onclose: () => void; children: Snippet } = $props();
  let element = $state<HTMLDivElement>();
</script>

<svelte:window
  onpointerdown={(event) => {
    const target = event.target as HTMLElement;
    if (element && !element.contains(target) && !target.closest("[data-popover-toggle]")) onclose();
  }}
  onkeydown={(event) => {
    if (event.key === "Escape") onclose();
  }}
/>

<div class="popover" role="dialog" aria-label={title} bind:this={element}>
  <h2>{title}</h2>
  {@render children()}
</div>

<style>
  .popover {
    position: absolute;
    right: 0;
    bottom: calc(100% + 0.5rem);
    z-index: 40;
    width: min(22rem, calc(100vw - 2rem));
    padding: 0.75rem 1rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    box-shadow: var(--shadow);
    text-align: left;
  }

  h2 {
    margin: 0 0 0.5rem;
    font-size: 0.95rem;
  }
</style>
