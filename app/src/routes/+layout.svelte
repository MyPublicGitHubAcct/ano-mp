<script lang="ts">
  // Global styles: the theme's tokens (PLAN.md X1) as custom properties on
  // :root, set by the appearance store in every window, and the base look
  // of controls, which read them.
  import { appearance } from "$lib/state/appearance.svelte";

  let { children } = $props();

  $effect(() => appearance.connect());
</script>

{@render children()}

<style>
  /* The tokens are `theme.ts`'s: colours (--bg, --surface, --surface-2, --text, --text-muted, --text-faint,
     --border, --accent, --accent-text, --danger, --heart, --star, --hover, --selected, --shadow), --font,
     --text-size, --density and --radius-sm, --radius, --radius-lg. */
  :global(:root) {
    font-family: var(--font);
    font-size: var(--text-size);
    line-height: 1.35;
    color: var(--text);
    background: var(--bg);
    -webkit-font-smoothing: antialiased;
  }

  :global(*, *::before, *::after) {
    box-sizing: border-box;
  }

  :global(body) {
    margin: 0;
    background: var(--bg);
    overflow: hidden;
  }

  :global(button) {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font: inherit;
    color: inherit;
    padding: calc(0.35rem * var(--density)) calc(0.8rem * var(--density));
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    cursor: pointer;
  }

  :global(button:hover:not(:disabled)) {
    background: var(--surface-2);
  }

  :global(button:disabled) {
    opacity: 0.4;
    cursor: default;
  }

  :global(button.primary) {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
  }

  :global(button.primary:hover:not(:disabled)) {
    background: var(--accent);
    filter: brightness(1.08);
  }

  :global(button.icon) {
    justify-content: center;
    padding: calc(0.35rem * var(--density));
    border: none;
    background: none;
    border-radius: var(--radius);
  }

  :global(button.icon:hover:not(:disabled)) {
    background: var(--hover);
  }

  :global(button.link) {
    border: none;
    background: none;
    padding: 0;
    color: var(--accent);
  }

  :global(input[type="search"], input[type="text"], input[type="number"], select) {
    font: inherit;
    color: inherit;
    padding: calc(0.35rem * var(--density)) calc(0.6rem * var(--density));
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
  }

  :global(input[type="range"]) {
    accent-color: var(--accent);
  }

  :global(:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  :global(.muted) {
    color: var(--text-muted);
  }

  /* Motion only for those who haven't asked for less (PLAN.md F18). */
  @media (prefers-reduced-motion: reduce) {
    :global(*, *::before, *::after) {
      animation-duration: 0.01ms !important;
      animation-iteration-count: 1 !important;
      transition-duration: 0.01ms !important;
      scroll-behavior: auto !important;
    }
  }
</style>
