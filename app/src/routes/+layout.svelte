<script lang="ts">
  // Global styles: colours as custom properties, light and dark from
  // prefers-color-scheme, and the base look of controls.
  let { children } = $props();
</script>

{@render children()}

<style>
  :global(:root) {
    --bg: #f6f6f7;
    --surface: #ffffff;
    --surface-2: #e9e9ec;
    --text: #18181b;
    --text-muted: #686870;
    --text-faint: #85858d;
    --border: #dcdce1;
    --accent: #3056d3;
    --accent-text: #ffffff;
    --hover: rgb(0 0 0 / 0.05);
    --selected: rgb(48 86 211 / 0.12);
    --danger: #c62828;
    --shadow: 0 6px 24px rgb(0 0 0 / 0.14);
    /* Hearts and stars: at least 3:1 against the surfaces (WCAG 1.4.11). */
    --heart: #d0265f;
    --star: #a86800;

    color-scheme: light dark;
    font-family:
      system-ui,
      -apple-system,
      "Segoe UI",
      sans-serif;
    font-size: 15px;
    line-height: 1.35;
    color: var(--text);
    background: var(--bg);
    -webkit-font-smoothing: antialiased;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --bg: #151517;
      --surface: #1e1e21;
      --surface-2: #2a2a2e;
      --text: #ececef;
      --text-muted: #a0a0a8;
      --text-faint: #6c6c74;
      --border: #333338;
      --accent: #7f9cff;
      --accent-text: #0d0d10;
      --hover: rgb(255 255 255 / 0.06);
      --selected: rgb(127 156 255 / 0.16);
      --danger: #ef5350;
      --shadow: 0 6px 24px rgb(0 0 0 / 0.5);
      --heart: #ff6f9a;
      --star: #f5b83d;
    }
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
    padding: 0.35rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 6px;
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
    padding: 0.35rem;
    border: none;
    background: none;
    border-radius: 6px;
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
    padding: 0.35rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 6px;
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
