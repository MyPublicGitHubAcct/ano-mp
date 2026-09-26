<script lang="ts">
  // The queue in play order: the current item highlighted, click to play an
  // item, ✕ to remove it, drag the handle to move it. Drags use pointer
  // events rather than HTML drag and drop, so they also work on touch
  // screens (Phase 8) and aren't taken over by the window's file drop.
  import { untrack } from "svelte";
  import { queue, type QueueItem } from "$lib/api";
  import { formatTime, plural } from "$lib/format";
  import { library } from "$lib/state/library.svelte";
  import { player } from "$lib/state/player.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import Art from "./Art.svelte";
  import Icon from "./Icon.svelte";
  import VirtualList from "./VirtualList.svelte";

  /** In the main area rather than beside it: no hide button. */
  let { main = false }: { main?: boolean } = $props();

  const ROW = 52;

  let list = $state<VirtualList<QueueItem>>();
  let selected = $state(-1);
  /** The item being dragged, and the gap (0…length) it would drop into. */
  let drag = $state<{ uid: number; from: number; gap: number } | null>(null);
  let autoScroll = 0;

  const totalTime = $derived(player.items.reduce((sum, item) => sum + item.duration, 0));

  // Keep the current item in view as the queue moves on (only when it
  // does, so a drag elsewhere in the list doesn't scroll back to it).
  $effect(() => {
    const current = player.current;
    untrack(() => {
      if (current !== null && !drag) list?.reveal(current);
    });
  });

  function gapAt(clientY: number) {
    const viewport = list?.element();
    if (!viewport) return 0;
    const y = clientY - viewport.getBoundingClientRect().top + viewport.scrollTop;
    return Math.max(0, Math.min(player.items.length, Math.round(y / ROW)));
  }

  function startDrag(event: PointerEvent, item: QueueItem, index: number) {
    event.preventDefault();
    event.stopPropagation();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    drag = { uid: item.uid, from: index, gap: index };
  }

  function moveDrag(event: PointerEvent) {
    if (!drag) return;
    drag = { ...drag, gap: gapAt(event.clientY) };
    // Scroll when near the top or bottom edge.
    const viewport = list?.element();
    if (!viewport) return;
    const box = viewport.getBoundingClientRect();
    const edge = Math.min(ROW, box.height / 4);
    const speed =
      event.clientY < box.top + edge ? -8 : event.clientY > box.bottom - edge ? 8 : 0;
    cancelAnimationFrame(autoScroll);
    if (speed !== 0) {
      const step = () => {
        viewport.scrollTop += speed;
        autoScroll = requestAnimationFrame(step);
      };
      autoScroll = requestAnimationFrame(step);
    }
  }

  function endDrag() {
    cancelAnimationFrame(autoScroll);
    if (!drag) return;
    const { uid, from, gap } = drag;
    drag = null;
    // The gap counts the dragged item; its index after the move doesn't.
    const to = gap > from ? gap - 1 : gap;
    if (to !== from) attempt(() => queue.move(uid, to));
  }

  const remove = (item: QueueItem) => attempt(() => queue.remove([item.uid]));

  function itemMenu(index: number, event: MouseEvent) {
    const item = player.items[index];
    if (!item) return;
    ui.openMenu(event, [
      { label: "Play", action: () => attempt(() => queue.jump(item.uid)) },
      {
        label: "Play next",
        disabled: player.current === null || index === player.current,
        action: () =>
          attempt(() =>
            queue.move(item.uid, index > (player.current ?? 0) ? (player.current ?? 0) + 1 : (player.current ?? 0)),
          ),
      },
      { label: "Remove from queue", action: () => remove(item) },
      {
        label: "Go to artist",
        disabled: item.artistId === null,
        action: () => {
          if (item.artistId === null || item.artist === null) return;
          library.query = "";
          ui.showArtist({ id: item.artistId, name: item.artist });
        },
      },
    ]);
  }

  function onkeydown(event: KeyboardEvent) {
    const item = player.items[selected];
    if (item && (event.key === "Delete" || event.key === "Backspace")) {
      event.preventDefault();
      remove(item);
    }
  }
</script>

<section class="panel" class:main aria-label="Queue">
  <header>
    <div>
      <h2>Queue</h2>
      <span class="muted small">
        {plural(player.items.length, "track")}{player.items.length > 0 ? ` · ${formatTime(totalTime)}` : ""}
      </span>
    </div>
    <button onclick={() => attempt(queue.clear)} disabled={player.items.length === 0}>Clear</button>
    {#if !main}
      <button
        class="icon"
        title="Show in the main area"
        aria-label="Show the queue in the main area"
        onclick={() => {
          library.query = "";
          ui.showQueue();
        }}><Icon name="expand" /></button
      >
      <button class="icon close" title="Hide the queue" aria-label="Hide the queue" onclick={() => (ui.queueOpen = false)}>
        <Icon name="close" />
      </button>
    {/if}
  </header>

  {#if player.items.length === 0}
    <p class="empty muted">Nothing queued. Double-click a track, or use Play next or Add to queue.</p>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="list" class:dragging={drag !== null} {onkeydown} onpointermove={moveDrag} onpointerup={endDrag} onpointercancel={endDrag}>
      <VirtualList
        bind:this={list}
        count={player.items.length}
        rowHeight={ROW}
        item={(index) => player.items[index]}
        label="Queue"
        bind:selected
        onactivate={(index) => attempt(() => queue.jump(player.items[index].uid))}
        onclick={(index, event) => {
          if (event.detail === 1 && (event.target as HTMLElement).closest("button") === null)
            attempt(() => queue.jump(player.items[index].uid));
        }}
        oncontextmenu={itemMenu}
      >
        {#snippet row(item: QueueItem | undefined, index: number)}
          {#if item}
            <div
              class="item"
              class:current={index === player.current}
              class:unavailable={player.unavailable.has(item.uid)}
              class:lifted={drag?.uid === item.uid}
              class:gap-before={drag !== null && drag.gap === index && drag.gap !== drag.from && drag.gap !== drag.from + 1}
              class:gap-after={drag !== null && drag.gap === player.items.length && index === player.items.length - 1 && drag.from !== index}
            >
              <button
                class="handle icon"
                title="Drag to move"
                aria-label="Move {item.title}"
                onpointerdown={(event) => startDrag(event, item, index)}
              >
                <Icon name="drag" size="1rem" />
              </button>
              <Art albumId={item.albumId} trackId={item.trackId} size="2.25rem" />
              <div class="text">
                <span class="name">{item.title}</span>
                <span class="muted small">{item.artist ?? ""}</span>
              </div>
              <span class="muted time">{formatTime(item.duration)}</span>
              <button
                class="icon remove"
                title="Remove"
                aria-label="Remove {item.title}"
                onclick={(event) => {
                  event.stopPropagation();
                  remove(item);
                }}><Icon name="close" size="1rem" /></button
              >
            </div>
          {/if}
        {/snippet}
      </VirtualList>
    </div>
  {/if}
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .main header {
    padding: 0.75rem 1rem;
  }

  .main h2 {
    font-size: 1.35rem;
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 0.75rem 0.5rem 1rem;
  }

  header div {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  h2 {
    margin: 0;
    font-size: 1.1rem;
  }

  .empty {
    padding: 1rem;
  }

  .list {
    flex: 1;
    min-height: 0;
  }

  .dragging {
    cursor: grabbing;
    user-select: none;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    height: 100%;
    padding: 0 0.5rem 0 0.25rem;
    min-width: 0;
  }

  .current {
    background: var(--selected);
  }

  .current .name {
    color: var(--accent);
    font-weight: 600;
  }

  .unavailable {
    opacity: 0.45;
  }

  .lifted {
    opacity: 0.4;
  }

  .gap-before {
    box-shadow: inset 0 2px 0 var(--accent);
  }

  .gap-after {
    box-shadow: inset 0 -2px 0 var(--accent);
  }

  .handle {
    cursor: grab;
    touch-action: none;
    color: var(--text-faint);
  }

  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .name,
  .small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 0.8rem;
  }

  .time {
    font-variant-numeric: tabular-nums;
    font-size: 0.85rem;
  }

  .remove {
    visibility: hidden;
  }

  :global(.row:hover) .remove,
  :global(.row.selected) .remove {
    visibility: visible;
  }

  @media (hover: none) {
    .remove {
      visibility: visible;
    }
  }
</style>
