<script lang="ts" generics="T">
  // A list of `count` fixed-height rows that renders only those in view, so
  // a node of 50,000 tracks costs what a screenful does. Rows it needs are
  // reported through `onneed`, for fetching pages on demand. Arrow keys,
  // Home/End and Page Up/Down move the selection; Enter activates it.
  import type { Snippet } from "svelte";

  let {
    count,
    rowHeight,
    item,
    row,
    label,
    selected = $bindable(-1),
    onneed,
    onclick,
    onactivate,
    oncontextmenu,
    overscan = 6,
  }: {
    count: number;
    rowHeight: number;
    /** The item at an index, or undefined while it loads. */
    item: (index: number) => T | undefined;
    row: Snippet<[T | undefined, number, boolean]>;
    label: string;
    selected?: number;
    onneed?: (start: number, end: number) => void;
    onclick?: (index: number, event: MouseEvent) => void;
    onactivate?: (index: number) => void;
    oncontextmenu?: (index: number, event: MouseEvent) => void;
    overscan?: number;
  } = $props();

  let viewport = $state<HTMLDivElement>();
  let scrollTop = $state(0);
  let height = $state(0);

  const start = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - overscan));
  const end = $derived(Math.min(count, Math.ceil((scrollTop + height) / rowHeight) + overscan));
  const indices = $derived(Array.from({ length: Math.max(0, end - start) }, (_, i) => start + i));

  $effect(() => {
    if (end > start) onneed?.(start, end);
  });

  export function element() {
    return viewport;
  }

  /** Scrolls the least needed to show row `index`. */
  export function reveal(index: number) {
    if (!viewport) return;
    const top = index * rowHeight;
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + rowHeight > viewport.scrollTop + viewport.clientHeight)
      viewport.scrollTop = top + rowHeight - viewport.clientHeight;
  }

  function select(index: number) {
    selected = Math.max(0, Math.min(count - 1, index));
    reveal(selected);
  }

  function onkeydown(event: KeyboardEvent) {
    const page = Math.max(1, Math.floor(height / rowHeight) - 1);
    const moves: Record<string, number> = {
      ArrowDown: selected + 1,
      ArrowUp: selected - 1,
      PageDown: selected + page,
      PageUp: selected - page,
      Home: 0,
      End: count - 1,
    };
    if (event.key in moves && count > 0) {
      event.preventDefault();
      select(selected < 0 && event.key === "ArrowDown" ? 0 : moves[event.key]);
    } else if (event.key === "Enter" && selected >= 0) {
      event.preventDefault();
      onactivate?.(selected);
    }
  }
</script>

<div
  class="viewport"
  bind:this={viewport}
  bind:clientHeight={height}
  onscroll={() => (scrollTop = viewport?.scrollTop ?? 0)}
  role="listbox"
  aria-label={label}
  tabindex="0"
  {onkeydown}
>
  <div class="spacer" style:height="{count * rowHeight}px">
    {#each indices as index (index)}
      <div
        class="row"
        class:selected={index === selected}
        style:transform="translateY({index * rowHeight}px)"
        style:height="{rowHeight}px"
        role="option"
        aria-selected={index === selected}
        tabindex="-1"
        onclick={(event) => {
          selected = index;
          onclick?.(index, event);
        }}
        ondblclick={() => onactivate?.(index)}
        oncontextmenu={(event) => {
          selected = index;
          oncontextmenu?.(index, event);
        }}
        onkeydown={() => {}}
      >
        {@render row(item(index), index, index === selected)}
      </div>
    {/each}
  </div>
</div>

<style>
  .viewport {
    position: relative;
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    outline: none;
    contain: strict;
  }

  .spacer {
    position: relative;
  }

  .row {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    cursor: default;
    outline: none;
  }

  .row:hover {
    background: var(--hover);
  }

  .row.selected {
    background: var(--selected);
  }

  .viewport:focus-visible .row.selected {
    box-shadow: inset 2px 0 0 var(--accent);
  }
</style>
