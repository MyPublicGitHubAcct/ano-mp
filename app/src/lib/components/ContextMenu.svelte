<script lang="ts">
  // The menu opened by right-click or a ⋯ button (`ui.openMenu`).
  import { ui } from "$lib/state/ui.svelte";

  let element = $state<HTMLDivElement>();
  let position = $state({ left: 0, top: 0 });

  // Keep it inside the window.
  $effect(() => {
    const menu = ui.menu;
    if (!menu || !element) return;
    const { width, height } = element.getBoundingClientRect();
    position = {
      left: Math.max(4, Math.min(menu.x, window.innerWidth - width - 4)),
      top: Math.max(4, Math.min(menu.y, window.innerHeight - height - 4)),
    };
    element.querySelector("button")?.focus();
  });

  const close = () => (ui.menu = null);
</script>

<svelte:window
  onpointerdown={(event) => {
    if (ui.menu && !element?.contains(event.target as Node)) close();
  }}
  onkeydown={(event) => {
    if (ui.menu && event.key === "Escape") close();
  }}
  onblur={close}
  onresize={close}
/>

{#if ui.menu}
  <div class="menu" role="menu" bind:this={element} style:left="{position.left}px" style:top="{position.top}px">
    {#each ui.menu.items as item (item.label)}
      <button
        role="menuitem"
        disabled={item.disabled}
        onclick={() => {
          close();
          item.action();
        }}>{item.label}</button
      >
    {/each}
  </div>
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 100;
    display: flex;
    flex-direction: column;
    min-width: 11rem;
    padding: 0.25rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    box-shadow: var(--shadow);
  }

  button {
    border: none;
    background: none;
    text-align: left;
    padding: 0.4rem 0.75rem;
    border-radius: 5px;
  }

  button:hover:not(:disabled),
  button:focus-visible {
    background: var(--accent);
    color: var(--accent-text);
    outline: none;
  }
</style>
