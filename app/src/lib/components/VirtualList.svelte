<script lang="ts" generics="T">
  // A list of `count` fixed-height rows that renders only those in view, so
  // a node of 50,000 tracks costs what a screenful does. Rows it needs are
  // reported through `onneed`, for fetching pages on demand.
  //
  // Selection (PLAN.md F4): a click selects a row; with `multiple`,
  // ⌘-click (Ctrl elsewhere) adds or removes one and Shift-click selects a
  // range, and the arrow keys, Home/End and Page Up/Down move the focus,
  // extending the selection with Shift; ⌘A selects all. Enter activates the
  // focused row; Shift-F10 or the context-menu key opens its menu. With
  // `ondragrow`, pressing on a row and moving drags the selection.
  //
  // For assistive technology (F18) it is a listbox whose rendered options
  // carry their position in the whole list, and whose focused row is its
  // active descendant.
  import type { Snippet } from "svelte";
  import { drag, type DragPayload } from "$lib/state/drag.svelte";
  import { click, emptySelection, moveTo, rowsFor, selectAll, type Selection } from "$lib/selection";

  let {
    count,
    rowHeight,
    item,
    row,
    label,
    selection = $bindable(emptySelection),
    multiple = false,
    onneed,
    onclick,
    onactivate,
    oncontextmenu,
    ondragrow,
    overscan = 6,
  }: {
    count: number;
    rowHeight: number;
    /** The item at an index, or undefined while it loads. */
    item: (index: number) => T | undefined;
    row: Snippet<[T | undefined, number, boolean]>;
    label: string;
    selection?: Selection;
    multiple?: boolean;
    onneed?: (start: number, end: number) => void;
    onclick?: (index: number, event: MouseEvent) => void;
    onactivate?: (index: number) => void;
    oncontextmenu?: (index: number, event: MouseEvent) => void;
    /** What dragging these rows carries, and what the drag says; null to not drag. */
    ondragrow?: (rows: number[]) => { payload: DragPayload; label: string } | null;
    overscan?: number;
  } = $props();

  let viewport = $state<HTMLDivElement>();
  let scrollTop = $state(0);
  let height = $state(0);
  const listId = `list-${Math.random().toString(36).slice(2)}`;

  const start = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - overscan));
  const end = $derived(Math.min(count, Math.ceil((scrollTop + height) / rowHeight) + overscan));
  const indices = $derived(Array.from({ length: Math.max(0, end - start) }, (_, i) => start + i));
  const focus = $derived(selection.focus);

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

  /** The selected rows in order (the focused one when nothing is). */
  export function selectedRows(): number[] {
    if (selection.rows.size === 0) return selection.focus >= 0 ? [selection.focus] : [];
    return [...selection.rows].sort((a, b) => a - b);
  }

  function onkeydown(event: KeyboardEvent) {
    const page = Math.max(1, Math.floor(height / rowHeight) - 1);
    const command = event.metaKey || event.ctrlKey;
    const moves: Record<string, number> = {
      ArrowDown: focus + 1,
      ArrowUp: focus - 1,
      PageDown: focus + page,
      PageUp: focus - page,
      Home: 0,
      End: count - 1,
    };
    if (event.key in moves && count > 0) {
      event.preventDefault();
      const target = focus < 0 && event.key === "ArrowDown" ? 0 : moves[event.key];
      selection = moveTo(selection, target, count, { shift: multiple && event.shiftKey });
      reveal(selection.focus);
    } else if (multiple && command && event.key.toLowerCase() === "a") {
      event.preventDefault();
      selection = selectAll(count);
    } else if (event.key === "Enter" && focus >= 0) {
      event.preventDefault();
      onactivate?.(focus);
    } else if ((event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) && focus >= 0) {
      event.preventDefault();
      reveal(focus);
      const box = viewport?.getBoundingClientRect();
      const x = (box?.left ?? 0) + 24;
      const y = (box?.top ?? 0) + focus * rowHeight - (viewport?.scrollTop ?? 0) + rowHeight;
      oncontextmenu?.(focus, new MouseEvent("contextmenu", { clientX: x, clientY: y }));
    } else if (event.key === "Escape" && selection.rows.size > 1) {
      selection = moveTo(selection, focus, count);
    }
  }

  function press(event: PointerEvent, index: number) {
    if (!ondragrow || (event.target as HTMLElement).closest("button, input, a")) return;
    drag.press(event, () => ondragrow(rowsFor(selection, index)));
  }
</script>

<div
  class="viewport"
  bind:this={viewport}
  bind:clientHeight={height}
  onscroll={() => (scrollTop = viewport?.scrollTop ?? 0)}
  role="listbox"
  aria-label={label}
  aria-multiselectable={multiple}
  aria-activedescendant={focus >= start && focus < end ? `${listId}-${focus}` : undefined}
  tabindex="0"
  {onkeydown}
>
  <div class="spacer" style:height="{count * rowHeight}px">
    {#each indices as index (index)}
      {@const selected = selection.rows.has(index)}
      <div
        class="row"
        class:selected
        class:focus={index === focus}
        id="{listId}-{index}"
        style:transform="translateY({index * rowHeight}px)"
        style:height="{rowHeight}px"
        role="option"
        aria-selected={selected}
        aria-setsize={count}
        aria-posinset={index + 1}
        tabindex="-1"
        onpointerdown={(event) => press(event, index)}
        onclick={(event) => {
          const modifiers = multiple ? { shift: event.shiftKey, toggle: event.metaKey || event.ctrlKey } : {};
          selection = click(selection, index, modifiers);
          onclick?.(index, event);
        }}
        ondblclick={() => onactivate?.(index)}
        oncontextmenu={(event) => {
          if (!selection.rows.has(index)) selection = click(selection, index);
          oncontextmenu?.(index, event);
        }}
        onkeydown={() => {}}
      >
        {@render row(item(index), index, selected)}
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

  /* The keyboard's row, while the list has the focus (F18). */
  .viewport:focus-visible .row.focus {
    box-shadow:
      inset 3px 0 0 var(--accent),
      inset 0 0 0 1px var(--accent);
  }
</style>
