<script lang="ts">
  // The queue in play order: the current item highlighted, click to play an
  // item, ✕ to remove it, drag the handle to move it. Items can be selected
  // together (PLAN.md F4): Delete removes them, and dragging a selected
  // item's handle moves them all. Tracks dragged from the library drop onto
  // the list. Drags use pointer events rather than HTML drag and drop, so
  // they also work on touch screens (Phase 8) and aren't taken over by the
  // window's file drop.
  //
  // The item playback stops after (F13) is marked, and the menu sets it;
  // the queue can be saved as a playlist (F1).
  import { untrack } from "svelte";
  import { queue, type QueueItem } from "$lib/api";
  import { count, t } from "$lib/i18n";
  import { formatTime } from "$lib/format";
  import { dropIndex, emptySelection, rowsFor, type Selection } from "$lib/selection";
  import { collection } from "$lib/state/collection.svelte";
  import { registerDropTarget } from "$lib/state/drag.svelte";
  import { library } from "$lib/state/library.svelte";
  import { player } from "$lib/state/player.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui, type MenuItem } from "$lib/state/ui.svelte";
  import { dragLabel, trackMenu } from "$lib/trackMenu";
  import Art from "./Art.svelte";
  import Icon from "./Icon.svelte";
  import VirtualList from "./VirtualList.svelte";

  /** In the main area rather than beside it: no hide button. */
  let { main = false }: { main?: boolean } = $props();

  const ROW = 52;

  let list = $state<VirtualList<QueueItem>>();
  let selection = $state<Selection>(emptySelection);
  /** The items being moved, and the gap (0…length) they would drop into. */
  let moving = $state<{ uids: number[]; rows: number[]; gap: number } | null>(null);
  let autoScroll = 0;

  const totalTime = $derived(player.items.reduce((sum, item) => sum + item.duration, 0));

  // Keep the current item in view as the queue moves on (only when it
  // does, so a drag elsewhere in the list doesn't scroll back to it).
  $effect(() => {
    const current = player.current;
    untrack(() => {
      if (current !== null && !moving) list?.reveal(current);
    });
  });

  // A new list: the selection may point at other items now.
  $effect(() => {
    void player.items;
    untrack(() => (selection = emptySelection));
  });

  // Tracks from the library drop onto the queue, at its end.
  const dropId = $derived(main ? "queue-main" : "queue-panel");
  $effect(() =>
    registerDropTarget(dropId, {
      accepts: (payload) => payload.kind === "tracks" || payload.kind === "playlist",
      drop: (payload) =>
        attempt(async () => {
          const ids =
            payload.kind === "tracks" ? await payload.trackIds() : payload.kind === "playlist" ? payload.trackIds : [];
          await queue.add(ids, false);
          ui.announce(t("queue.added", { count: ids.length }));
        }),
    }),
  );

  function gapAt(clientY: number) {
    const viewport = list?.element();
    if (!viewport) return 0;
    const y = clientY - viewport.getBoundingClientRect().top + viewport.scrollTop;
    return Math.max(0, Math.min(player.items.length, Math.round(y / ROW)));
  }

  function startMove(event: PointerEvent, index: number) {
    event.preventDefault();
    event.stopPropagation();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    const rows = rowsFor(selection, index);
    moving = { uids: rows.map((row) => player.items[row].uid), rows, gap: index };
  }

  function move(event: PointerEvent) {
    if (!moving) return;
    moving = { ...moving, gap: gapAt(event.clientY) };
    // Scroll when near the top or bottom edge.
    const viewport = list?.element();
    if (!viewport) return;
    const box = viewport.getBoundingClientRect();
    const edge = Math.min(ROW, box.height / 4);
    const speed = event.clientY < box.top + edge ? -8 : event.clientY > box.bottom - edge ? 8 : 0;
    cancelAnimationFrame(autoScroll);
    if (speed !== 0) {
      const step = () => {
        viewport.scrollTop += speed;
        autoScroll = requestAnimationFrame(step);
      };
      autoScroll = requestAnimationFrame(step);
    }
  }

  function endMove() {
    cancelAnimationFrame(autoScroll);
    if (!moving) return;
    const { uids, rows, gap } = moving;
    moving = null;
    const to = dropIndex(rows, gap);
    if (to !== rows[0] || rows.some((row, i) => row !== rows[0] + i)) {
      attempt(() => (uids.length === 1 ? queue.move(uids[0], to) : queue.moveItems(uids, to)));
    }
  }

  const remove = (uids: number[]) => attempt(() => queue.remove(uids));

  function itemMenu(index: number, event: MouseEvent) {
    const rows = rowsFor(selection, index);
    const items = rows.map((row) => player.items[row]).filter(Boolean);
    const item = items.length === 1 ? items[0] : null;
    const library_ = items.filter((entry) => !entry.external);
    const tracks = library_.map((entry) => ({
      id: entry.trackId,
      title: entry.title,
      artist: entry.artist,
      artistId: entry.artistId,
      album: entry.album,
      albumId: entry.albumId,
      albumArtist: null,
      favourite: false,
      rating: null,
    }));
    const current = player.current ?? 0;
    const menu: MenuItem[] = [];
    if (item) {
      menu.push(
        { label: t("menu.play"), action: () => attempt(() => queue.jump(item.uid)) },
        {
          label: t("menu.playNext"),
          disabled: player.current === null || index === player.current,
          action: () => attempt(() => queue.move(item.uid, index > current ? current + 1 : current)),
        },
        {
          label: t("queue.stopAfter"),
          checked: player.stopAfter === item.uid,
          action: () => attempt(() => queue.setStopAfter(player.stopAfter === item.uid ? null : item.uid)),
        },
      );
    } else {
      menu.push({
        label: t("menu.playNext"),
        disabled: player.current === null,
        action: () => {
          const uids = items.map((entry) => entry.uid).filter((uid) => uid !== player.currentItem?.uid);
          const before = rows.filter((row) => row <= current).length;
          return attempt(() => queue.moveItems(uids, current + 1 - (before > 0 ? before - 1 : 0)));
        },
      });
    }
    menu.push({
      label: t("queue.remove", { count: items.length }),
      action: () => remove(items.map((entry) => entry.uid)),
    });
    if (tracks.length > 0) {
      // The library's actions, less those that would re-queue them.
      const more = trackMenu(tracks).slice(3);
      menu.push({ separator: true }, ...more);
    }
    ui.openMenu(event, menu);
  }

  function dragRows(rows: number[]) {
    const items = rows.map((row) => player.items[row]).filter((item) => item && !item.external);
    if (items.length === 0) return null;
    const ids = items.map((item) => item.trackId);
    return {
      payload: { kind: "tracks" as const, trackIds: async () => ids },
      label: items.length === 1 ? items[0].title : dragLabel(items.length),
    };
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Delete" || event.key === "Backspace") {
      const rows = rowsFor(selection, selection.focus).filter((row) => row >= 0);
      const uids = rows.map((row) => player.items[row]?.uid).filter((uid) => uid !== undefined);
      if (uids.length > 0) {
        event.preventDefault();
        remove(uids);
      }
    }
  }
</script>

<section class="panel" class:main aria-label={t("queue.title")}>
  <header>
    <div>
      <h2>{t("queue.title")}</h2>
      <span class="muted small">
        {count("count.tracks", player.items.length)}{player.items.length > 0 ? ` · ${formatTime(totalTime)}` : ""}
      </span>
    </div>
    {#if player.radio}
      <button class="radio" title={t("queue.radioStop")} onclick={() => attempt(queue.stopRadio)}>
        <Icon name="radio" size="1rem" />
        {t("queue.radio")}
      </button>
    {/if}
    <button
      class="icon"
      title={t("queue.saveAsPlaylist")}
      aria-label={t("queue.saveAsPlaylist")}
      disabled={player.items.length === 0}
      onclick={collection.saveQueue}><Icon name="playlist" /></button
    >
    <button onclick={() => attempt(queue.clear)} disabled={player.items.length === 0}>{t("queue.clear")}</button>
    {#if !main}
      <button
        class="icon"
        title={t("queue.showInMain")}
        aria-label={t("queue.showInMain")}
        onclick={() => {
          library.query = "";
          ui.showQueue();
        }}><Icon name="expand" /></button
      >
      <button
        class="icon close"
        title={t("queue.hide")}
        aria-label={t("queue.hide")}
        onclick={() => (ui.queueOpen = false)}
      >
        <Icon name="close" />
      </button>
    {/if}
  </header>

  {#if player.items.length === 0}
    <div class="empty muted" data-drop={dropId}>
      <p>{t("queue.empty")}</p>
    </div>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="list"
      class:dragging={moving !== null}
      data-drop={dropId}
      {onkeydown}
      onpointermove={move}
      onpointerup={endMove}
      onpointercancel={endMove}
    >
      <VirtualList
        bind:this={list}
        count={player.items.length}
        rowHeight={ROW}
        item={(index) => player.items[index]}
        label={t("queue.title")}
        multiple
        bind:selection
        onactivate={(index) => attempt(() => queue.jump(player.items[index].uid))}
        onclick={(index, event) => {
          const modified = event.shiftKey || event.metaKey || event.ctrlKey;
          if (event.detail === 1 && !modified && (event.target as HTMLElement).closest("button") === null)
            attempt(() => queue.jump(player.items[index].uid));
        }}
        oncontextmenu={itemMenu}
        ondragrow={dragRows}
      >
        {#snippet row(item: QueueItem | undefined, index: number)}
          {#if item}
            <div
              class="item"
              class:current={index === player.current}
              class:unavailable={player.unavailable.has(item.uid)}
              class:lifted={moving?.uids.includes(item.uid)}
              class:gap-before={moving !== null && moving.gap === index}
              class:gap-after={moving !== null &&
                moving.gap === player.items.length &&
                index === player.items.length - 1}
            >
              <button
                class="handle icon"
                title={t("queue.dragToMove")}
                aria-label={t("queue.move", { name: item.title })}
                onpointerdown={(event) => startMove(event, index)}
              >
                <Icon name="drag" size="1rem" />
              </button>
              <Art albumId={item.albumId} trackId={item.trackId} size="2.25rem" />
              <div class="text">
                <span class="name">{item.title}</span>
                <span
                  class="muted small"
                  title={item.reason ? t("queue.radioReason", { reason: item.reason }) : undefined}
                >
                  {#if item.reason}<span class="reason">{item.reason}</span> ·
                  {/if}{item.artist ?? ""}{item.skip ? ` · ${t("queue.skipped")}` : ""}{item.external
                    ? ` · ${t("queue.external")}`
                    : ""}
                </span>
              </div>
              {#if player.stopAfter === item.uid}
                <span class="stop" title={t("queue.stopsAfter")} role="img" aria-label={t("queue.stopsAfter")}>
                  <Icon name="stopAfter" size="1rem" />
                </span>
              {/if}
              <span class="muted time">{formatTime(item.duration)}</span>
              <button
                class="icon remove"
                title={t("queue.removeOne")}
                aria-label={t("queue.removeName", { name: item.title })}
                onclick={(event) => {
                  event.stopPropagation();
                  remove([item.uid]);
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
  .reason,
  .radio,
  .stop {
    color: var(--accent);
  }

  .stop {
    display: flex;
  }

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
    flex: 1;
    padding: 1rem;
  }

  .empty p {
    margin: 0;
  }

  .list {
    flex: 1;
    min-height: 0;
  }

  .dragging {
    cursor: grabbing;
    user-select: none;
  }

  :global([data-drop-over]).list,
  :global([data-drop-over]).empty {
    box-shadow: inset 0 0 0 2px var(--accent);
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
  :global(.row.selected) .remove,
  .remove:focus-visible {
    visibility: visible;
  }

  @media (hover: none) {
    .remove {
      visibility: visible;
    }
  }
</style>
