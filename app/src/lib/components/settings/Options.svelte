<script lang="ts">
  // The styles settings sections share (`.field`, `.switch`, `.hint`…),
  // around a section wherever it shows: the settings page, and the effects
  // workbench (PLAN.md X8), which shows the effects' and recording's.
  import type { Snippet } from "svelte";

  let { children }: { children: Snippet } = $props();
</script>

<div class="options">
  {@render children()}
</div>

<style>
  .options :global(h3) {
    font-size: 1rem;
    margin: 1.5rem 0 0.5rem;
  }

  .options :global(h3:first-child) {
    margin-top: 0.5rem;
  }

  .options :global(p) {
    margin: 0;
  }

  .options :global(.hint) {
    color: var(--text-muted);
    font-size: 0.8rem;
  }

  .options :global(.field) {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35rem 0.75rem;
    padding: 0.4rem 0;
  }

  .options :global(.field > .label) {
    flex: 0 0 11rem;
    font-weight: 500;
  }

  .options :global(.field > .control) {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    flex: 1 1 16rem;
    min-width: 0;
  }

  .options :global(.field select) {
    max-width: 100%;
  }

  .options :global(.field .hint) {
    flex-basis: 100%;
  }

  .options :global(.stacked) {
    flex-direction: column;
    align-items: stretch;
  }

  .options :global(.stacked > .label) {
    flex-basis: auto;
  }

  .options :global(.switch) {
    display: flex;
    align-items: flex-start;
    gap: 0.6rem;
    padding: 0.4rem 0;
    cursor: pointer;
  }

  .options :global(.switch > span) {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  .options :global(.switch .title) {
    font-weight: 500;
  }

  .options :global(input[type="checkbox"]),
  .options :global(input[type="radio"]) {
    margin: 0.2rem 0 0;
    accent-color: var(--accent);
    width: 1rem;
    height: 1rem;
    flex: none;
  }

  .options :global(input[type="range"]) {
    flex: 1 1 10rem;
    max-width: 18rem;
  }

  .options :global(.value) {
    min-width: 4.5rem;
    font-variant-numeric: tabular-nums;
  }

  .options :global(.actions) {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-top: 1.5rem;
  }

  .options :global(.card) {
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
  }
</style>
