<script lang="ts">
  // The menu opened by right-click, a ⋯ button or the keyboard
  // (`ui.openMenu`), with submenus ("Add to Playlist ▸"), check marks and
  // separators.
  //
  // Keyboard (PLAN.md F18): the up and down arrows, Home and End move
  // between items; Right, Enter or Space opens a submenu; Left or Escape
  // closes one; a letter jumps to the next item starting with it; Escape at
  // the top closes the menu, and the focus goes back where it was.
  import { tick } from "svelte";
  import { ui, type MenuItem } from "$lib/state/ui.svelte";

  /** Indices of the open submenus, from the top menu down. */
  let open = $state<number[]>([]);
  let panels = $state<HTMLDivElement[]>([]);
  let positions = $state<{ left: number; top: number }[]>([]);

  const levels = $derived.by(() => {
    const menu = ui.menu;
    if (!menu) return [];
    const result: MenuItem[][] = [menu.items];
    for (const index of open) {
      const items = result.at(-1)?.[index]?.items;
      if (!items) break;
      result.push(items);
    }
    return result;
  });

  // A new menu: at the pointer (or the row), inside the window, with its
  // first item focused.
  $effect(() => {
    const menu = ui.menu;
    if (!menu) {
      open = [];
      return;
    }
    void tick().then(() => {
      place(0, menu.x, menu.y);
      focusFirst(0);
    });
  });

  function place(level: number, x: number, y: number, parentWidth = 0) {
    const panel = panels[level];
    if (!panel) return;
    const { width, height } = panel.getBoundingClientRect();
    // A submenu opens to the right of its item, or to the left if there's no room.
    const left = x + width > window.innerWidth - 4 && parentWidth > 0 ? x - width - parentWidth : x;
    positions[level] = {
      left: Math.max(4, Math.min(left, window.innerWidth - width - 4)),
      top: Math.max(4, Math.min(y, window.innerHeight - height - 4)),
    };
  }

  function buttons(level: number) {
    return [...(panels[level]?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
  }

  function focusFirst(level: number) {
    buttons(level)[0]?.focus();
  }

  function close() {
    ui.menu = null;
    open = [];
    const back = ui.menuReturn;
    ui.menuReturn = null;
    if (back?.isConnected) back.focus();
  }

  async function openSubmenu(level: number, index: number, button: HTMLButtonElement, focus: boolean) {
    open = [...open.slice(0, level), index];
    await tick();
    const box = button.getBoundingClientRect();
    place(level + 1, box.right - 2, box.top - 5, box.width);
    if (focus) focusFirst(level + 1);
  }

  function activate(level: number, index: number, item: MenuItem, button: HTMLButtonElement) {
    if ("separator" in item || item.disabled) return;
    if (item.items) {
      void openSubmenu(level, index, button, true);
    } else if (item.action) {
      const action = item.action;
      close();
      action();
    }
  }

  function onkeydown(event: KeyboardEvent, level: number) {
    const list = buttons(level);
    const at = list.indexOf(document.activeElement as HTMLButtonElement);
    const move = (to: number) => {
      event.preventDefault();
      list[(to + list.length) % list.length]?.focus();
    };
    switch (event.key) {
      case "ArrowDown":
        return move(at + 1);
      case "ArrowUp":
        return move(at < 0 ? -1 : at - 1);
      case "Home":
        return move(0);
      case "End":
        return move(-1);
      case "ArrowLeft":
      case "Escape":
        event.preventDefault();
        event.stopPropagation();
        if (level === 0) {
          if (event.key === "Escape") close();
          return;
        }
        {
          const parent = buttons(level - 1).find((button) => button.getAttribute("aria-expanded") === "true");
          open = open.slice(0, level - 1);
          parent?.focus();
        }
        return;
      case "ArrowRight": {
        const button = document.activeElement as HTMLButtonElement;
        if (button?.getAttribute("aria-haspopup") === "menu") {
          event.preventDefault();
          button.click();
        }
        return;
      }
      case "Tab":
        event.preventDefault();
        return move(event.shiftKey ? at - 1 : at + 1);
      default:
        if (event.key.length === 1 && /\S/.test(event.key)) {
          // Type-ahead: the next item starting with the letter.
          const letter = event.key.toLowerCase();
          const order = [...list.slice(at + 1), ...list.slice(0, at + 1)];
          order.find((button) => button.textContent?.trim().toLowerCase().startsWith(letter))?.focus();
        }
    }
  }
</script>

<svelte:window
  onpointerdown={(event) => {
    if (ui.menu && !panels.some((panel) => panel?.contains(event.target as Node))) close();
  }}
  onblur={() => ui.menu && close()}
  onresize={() => ui.menu && close()}
/>

{#each levels as items, level (level)}
  <div
    class="menu"
    role="menu"
    tabindex="-1"
    bind:this={panels[level]}
    style:left="{positions[level]?.left ?? -9999}px"
    style:top="{positions[level]?.top ?? -9999}px"
    onkeydown={(event) => onkeydown(event, level)}
  >
    {#each items as item, index (index)}
      {#if "separator" in item}
        <div class="separator" role="separator"></div>
      {:else if item.items}
        <button
          role="menuitem"
          aria-haspopup="menu"
          aria-expanded={open[level] === index}
          class:open={open[level] === index}
          disabled={item.disabled || item.items.length === 0}
          onclick={(event) => activate(level, index, item, event.currentTarget)}
          onpointerenter={(event) => openSubmenu(level, index, event.currentTarget, false)}
        >
          <span class="label">{item.label}</span><span class="chevron" aria-hidden="true">›</span>
        </button>
      {:else}
        <button
          role={item.checked === undefined ? "menuitem" : "menuitemcheckbox"}
          aria-checked={item.checked === undefined ? undefined : item.checked}
          disabled={item.disabled}
          onclick={(event) => activate(level, index, item, event.currentTarget)}
          onpointerenter={() => {
            if (open.length > level) open = open.slice(0, level);
          }}
        >
          <span class="check" aria-hidden="true">{item.checked ? "✓" : ""}</span>
          <span class="label">{item.label}</span>
        </button>
      {/if}
    {/each}
  </div>
{/each}

<style>
  .menu {
    position: fixed;
    z-index: 100;
    display: flex;
    flex-direction: column;
    min-width: 12rem;
    max-width: 22rem;
    max-height: calc(100vh - 8px);
    overflow-y: auto;
    padding: 0.25rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow);
    outline: none;
  }

  button {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    border: none;
    background: none;
    text-align: left;
    padding: 0.4rem 0.75rem 0.4rem 0.35rem;
    border-radius: var(--radius-sm);
  }

  .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .check {
    width: 1rem;
    flex: none;
    text-align: center;
  }

  .chevron {
    color: var(--text-muted);
    margin-left: 0.75rem;
  }

  button:hover:not(:disabled),
  button:focus-visible,
  button.open {
    background: var(--accent);
    color: var(--accent-text);
    outline: none;
  }

  button:hover:not(:disabled) .chevron,
  button:focus-visible .chevron,
  button.open .chevron {
    color: inherit;
  }

  .separator {
    height: 1px;
    margin: 0.25rem 0.5rem;
    background: var(--border);
  }
</style>
