<script lang="ts">
  // A playlist (PLAN.md F1): its name (click to rename), play and shuffle,
  // and its tracks, a page at a time. Entries can be selected together and
  // dragged by their handle to a new place, removed with Delete, and tracks
  // dragged from anywhere drop in where they're released. A smart playlist
  // (F2) lists whatever matches its rules now; its rules are edited in a
  // dialog, and its tracks can't be moved or removed by hand.
  import { untrack } from "svelte";
  import { PLAYLIST_NAME_MAX, playlists as api, queue, type PlaylistEntry } from "$lib/api";
  import { count, t } from "$lib/i18n";
  import { fileName, formatTime } from "$lib/format";
  import { dropIndex, emptySelection, rowsFor, type Selection } from "$lib/selection";
  import { collection } from "$lib/state/collection.svelte";
  import { registerDropTarget } from "$lib/state/drag.svelte";
  import { library } from "$lib/state/library.svelte";
  import { player } from "$lib/state/player.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui, type MenuItem } from "$lib/state/ui.svelte";
  import { dragLabel, trackMenu } from "$lib/trackMenu";
  import Heart from "./Heart.svelte";
  import Icon from "./Icon.svelte";
  import TrackText from "./TrackText.svelte";
  import VirtualList from "./VirtualList.svelte";

  const PAGE = 200;
  const ROW = 40;

  const playlist = $derived(collection.byId(ui.playlistId));
  const smart = $derived(playlist?.rules !== null && playlist?.rules !== undefined);

  let pages = $state.raw(new Map<number, PlaylistEntry[]>());
  let loaded = $state(false);
  let selection = $state<Selection>(emptySelection);
  let requested = new Set<number>();
  let request = 0;
  let list = $state<VirtualList<PlaylistEntry>>();
  let name = $state("");
  let nameInput = $state<HTMLInputElement>();
  let moving = $state<{ itemIds: number[]; rows: number[]; gap: number } | null>(null);

  const total = $derived(playlist?.trackCount ?? 0);

  // Another playlist, or it changed: start over.
  $effect(() => {
    void [ui.playlistId, collection.version, library.version];
    untrack(() => {
      request++;
      requested = new Set();
      pages = new Map();
      loaded = false;
      selection = emptySelection;
      loadPage(0);
    });
  });

  $effect(() => {
    name = playlist?.name ?? "";
  });

  // A playlist just made: its name is ready to type over.
  $effect(() => {
    if (playlist && ui.renaming === playlist.id && nameInput) {
      nameInput.focus();
      nameInput.select();
    }
  });

  async function loadPage(page: number) {
    const id = ui.playlistId;
    if (id === null || requested.has(page)) return;
    requested.add(page);
    const current = request;
    const result = await attempt(() => api.page(id, page * PAGE, PAGE));
    if (current !== request) return;
    if (!result) {
      requested.delete(page);
      return;
    }
    loaded = true;
    pages = new Map(pages).set(page, result.entries);
  }

  function need(start: number, end: number) {
    for (let page = Math.floor(start / PAGE); page <= Math.floor((end - 1) / PAGE); page++) loadPage(page);
  }

  const entryAt = (index: number) => pages.get(Math.floor(index / PAGE))?.[index % PAGE];
  const entriesOf = (rows: number[]) => rows.map(entryAt).filter((entry) => entry !== undefined);

  async function allIds() {
    return playlist ? api.trackIds(playlist.id) : [];
  }

  const playFrom = (index: number) =>
    attempt(async () => {
      const ids = await allIds();
      await queue.play(ids, index);
    });

  function commitName() {
    if (playlist) void collection.rename(playlist, name);
    if (ui.renaming === playlist?.id) ui.renaming = null;
  }

  function headerMenu(event: MouseEvent) {
    if (!playlist) return;
    const current = playlist;
    const items: MenuItem[] = [
      { label: t("menu.playNext"), action: () => collection.enqueue(current, true) },
      { label: t("menu.addToQueue"), action: () => collection.enqueue(current, false) },
      { separator: true },
      {
        label: t("playlist.rename"),
        action: () => {
          nameInput?.focus();
          nameInput?.select();
        },
      },
    ];
    if (smart) items.push({ label: t("playlist.editRules"), action: () => (ui.dialog = { kind: "smartPlaylist", playlist: current }) });
    items.push(
      { label: t("playlist.export"), action: () => collection.exportPlaylist(current) },
      { separator: true },
      { label: t("playlist.delete"), action: () => confirmDelete() },
    );
    ui.openMenu(event, items);
  }

  async function confirmDelete() {
    if (!playlist) return;
    const { ask } = await import("@tauri-apps/plugin-dialog");
    const yes = await ask(t("playlist.deleteConfirm", { name: playlist.name }), {
      title: t("playlist.delete"),
      kind: "warning",
      okLabel: t("playlist.deleteOk"),
    });
    if (yes) await collection.remove(playlist);
  }

  function entryMenu(index: number, event: MouseEvent) {
    const entries = entriesOf(rowsFor(selection, index));
    if (entries.length === 0 || !playlist) return;
    const current = playlist;
    const one = entries.length === 1 ? entries[0] : null;
    const items = trackMenu(entries, one ? () => playFrom(index) : undefined);
    if (!smart) {
      const itemIds = entries.map((entry) => entry.itemId).filter((id) => id !== null);
      items.push(
        { separator: true },
        {
          label: t("playlist.removeEntries", { count: itemIds.length }),
          action: () => attempt(() => api.removeItems(current.id, itemIds)),
        },
      );
    }
    ui.openMenu(event, items);
  }

  function dragRows(rows: number[]) {
    const entries = entriesOf(rows);
    if (entries.length === 0 || !playlist) return null;
    const trackIds = entries.map((entry) => entry.id);
    return {
      payload: {
        kind: "playlist" as const,
        playlistId: playlist.id,
        itemIds: entries.map((entry) => entry.itemId ?? -1),
        trackIds,
      },
      label: entries.length === 1 ? (entries[0].title ?? fileName(entries[0].path)) : dragLabel(entries.length),
    };
  }

  function gapAt(clientY: number) {
    const viewport = list?.element();
    if (!viewport) return total;
    const y = clientY - viewport.getBoundingClientRect().top + viewport.scrollTop;
    return Math.max(0, Math.min(total, Math.round(y / ROW)));
  }

  // Tracks from elsewhere drop in at the gap under the pointer; entries
  // of this playlist dragged by the row move there.
  $effect(() => {
    const id = playlist?.id;
    if (id === undefined || smart) return;
    return registerDropTarget(`playlist-view:${id}`, {
      accepts: (payload) => payload.kind === "tracks" || payload.kind === "playlist",
      drop: (payload, event) =>
        attempt(async () => {
          const gap = gapAt(event.clientY);
          if (payload.kind === "playlist" && payload.playlistId === id) {
            const rows = selection.rows.size > 0 ? [...selection.rows].sort((a, b) => a - b) : [];
            await api.move(id, payload.itemIds, dropIndex(rows, gap));
          } else if (payload.kind === "tracks") {
            await api.add(id, await payload.trackIds(), gap);
          } else if (payload.kind === "playlist") {
            await api.add(id, payload.trackIds, gap);
          }
        }),
    });
  });

  function startMove(event: PointerEvent, index: number) {
    event.preventDefault();
    event.stopPropagation();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    const rows = rowsFor(selection, index);
    const itemIds = entriesOf(rows)
      .map((entry) => entry.itemId)
      .filter((id) => id !== null);
    moving = { itemIds, rows, gap: index };
  }

  function endMove() {
    if (!moving || !playlist) return;
    const { itemIds, rows, gap } = moving;
    moving = null;
    const to = dropIndex(rows, gap);
    const id = playlist.id;
    attempt(() => api.move(id, itemIds, to));
  }

  function onkeydown(event: KeyboardEvent) {
    if (smart || !playlist || (event.key !== "Delete" && event.key !== "Backspace")) return;
    const itemIds = entriesOf(rowsFor(selection, selection.focus))
      .map((entry) => entry.itemId)
      .filter((id) => id !== null);
    if (itemIds.length === 0) return;
    event.preventDefault();
    const id = playlist.id;
    attempt(() => api.removeItems(id, itemIds));
  }

  $effect(() => {
    ui.selectedTracks = () => entriesOf(rowsFor(selection, selection.focus));
    return () => (ui.selectedTracks = null);
  });
</script>

<section class="pane">
  {#if playlist}
    <header class="node">
      <div class="heading">
        <span class="glyph"><Icon name={smart ? "smart" : "playlist"} size="1.6rem" /></span>
        <div class="title">
          <input
            class="name"
            bind:this={nameInput}
            bind:value={name}
            maxlength={PLAYLIST_NAME_MAX}
            aria-label={t("playlist.name")}
            onblur={commitName}
            onkeydown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
              if (event.key === "Escape") {
                name = playlist.name;
                event.currentTarget.blur();
              }
            }}
          />
          <span class="muted small">
            {smart ? t("playlist.smart") : t("playlist.list")} · {count("count.tracks", total)}{total > 0
              ? ` · ${formatTime(playlist.duration)}`
              : ""}
          </span>
        </div>
      </div>
      <div class="actions">
        {#if smart}
          <button onclick={() => (ui.dialog = { kind: "smartPlaylist", playlist })}>
            <Icon name="sliders" />
            {t("playlist.editRules")}
          </button>
        {/if}
        <button class="primary" disabled={total === 0} onclick={() => collection.play(playlist)}>
          <Icon name="play" />
          {t("library.play")}
        </button>
        <button disabled={total === 0} onclick={() => collection.play(playlist, true)}>
          <Icon name="shuffle" />
          {t("library.shuffle")}
        </button>
        <button class="icon" title={t("library.more")} aria-label={t("playlist.more")} onclick={headerMenu}>
          <Icon name="more" />
        </button>
      </div>
    </header>

    {#if loaded && total === 0}
      <div class="empty" data-drop={smart ? undefined : `playlist-view:${playlist.id}`}>
        {#if smart}
          <p>{t("playlist.smartEmpty")}</p>
          <button onclick={() => (ui.dialog = { kind: "smartPlaylist", playlist })}>{t("playlist.editRules")}</button>
        {:else}
          <p>{t("playlist.empty")}</p>
        {/if}
      </div>
    {:else}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="list"
        class:dragging={moving !== null}
        data-drop={smart ? undefined : `playlist-view:${playlist.id}`}
        {onkeydown}
        onpointermove={(event) => moving && (moving = { ...moving, gap: gapAt(event.clientY) })}
        onpointerup={endMove}
        onpointercancel={() => (moving = null)}
      >
        {#key `${playlist.id}/${collection.version}`}
          <VirtualList
            bind:this={list}
            count={total}
            rowHeight={ROW}
            item={entryAt}
            label={playlist.name}
            multiple
            bind:selection
            onneed={need}
            onactivate={playFrom}
            oncontextmenu={entryMenu}
            ondragrow={dragRows}
          >
            {#snippet row(entry: PlaylistEntry | undefined, index: number)}
              {#if entry === undefined}
                <div class="entry"><span class="placeholder"></span></div>
              {:else}
                {@const name = entry.title ?? fileName(entry.path)}
                <div
                  class="entry"
                  class:playing={player.currentItem?.trackId === entry.id}
                  class:lifted={moving?.rows.includes(index)}
                  class:gap-before={moving !== null && moving.gap === index}
                >
                  {#if !smart}
                    <button
                      class="handle icon"
                      title={t("queue.dragToMove")}
                      aria-label={t("queue.move", { name })}
                      onpointerdown={(event) => startMove(event, index)}
                    >
                      <Icon name="drag" size="1rem" />
                    </button>
                  {/if}
                  <span class="number muted">{index + 1}</span>
                  <TrackText track={entry} />
                  <Heart
                    on={entry.favourite}
                    label={name}
                    quiet
                    onchange={(on) => collection.setFavourite("track", [entry.id], on)}
                  />
                  <button
                    class="icon"
                    title={t("library.more")}
                    aria-label={t("library.moreFor", { name })}
                    onclick={(event) => {
                      event.stopPropagation();
                      entryMenu(index, event);
                    }}><Icon name="more" /></button
                  >
                </div>
              {/if}
            {/snippet}
          </VirtualList>
        {/key}
      </div>
    {/if}
  {:else}
    <div class="empty"><p>{t("playlist.gone")}</p></div>
  {/if}
</section>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .node {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem 1rem;
    padding: 0.75rem 1rem;
  }

  .heading {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    min-width: 0;
    flex: 1;
  }

  .glyph {
    color: var(--accent);
    display: flex;
  }

  .title {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .name {
    font: inherit;
    font-size: 1.35rem;
    font-weight: 700;
    color: inherit;
    border: 1px solid transparent;
    border-radius: 6px;
    background: none;
    padding: 0.05rem 0.3rem;
    margin-left: -0.3rem;
    min-width: 0;
    width: 100%;
  }

  .name:hover,
  .name:focus {
    border-color: var(--border);
    background: var(--surface);
  }

  .small {
    font-size: 0.8rem;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .list {
    flex: 1;
    min-height: 0;
    container-type: inline-size;
  }

  .dragging {
    cursor: grabbing;
    user-select: none;
  }

  :global([data-drop-over]).list,
  :global([data-drop-over]).empty {
    box-shadow: inset 0 0 0 2px var(--accent);
  }

  .entry {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    width: 100%;
    height: 100%;
    padding: 0 0.5rem 0 0.25rem;
    min-width: 0;
  }

  .number {
    width: 3ch;
    text-align: right;
    font-variant-numeric: tabular-nums;
    flex: none;
  }

  .handle {
    cursor: grab;
    touch-action: none;
    color: var(--text-faint);
  }

  .playing :global(.name) {
    color: var(--accent);
    font-weight: 600;
  }

  .lifted {
    opacity: 0.4;
  }

  .gap-before {
    box-shadow: inset 0 2px 0 var(--accent);
  }

  .placeholder {
    display: block;
    height: 0.8rem;
    width: 40%;
    border-radius: 4px;
    background: var(--surface-2);
    margin-left: 1rem;
  }

  .empty {
    display: grid;
    place-items: center;
    align-content: center;
    gap: 1rem;
    flex: 1;
    color: var(--text-muted);
    padding: 1rem;
    text-align: center;
  }

  .empty p {
    margin: 0;
    max-width: 30rem;
  }
</style>
