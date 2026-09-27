<script lang="ts">
  // One node of the library under the current sort rule: its groups (album
  // artists, albums, subfolders…) and/or its tracks (with the fields the
  // display settings choose), fetched a page at a time as they scroll into
  // view. Click or Enter opens a group; double-click
  // or Enter plays the node's tracks from a track; right-click or the ⋯
  // button offers play next and add to queue, the artist's page, and an
  // album's "Find details" and "Choose cover". Inside an artist, the header
  // links to their page; inside an album, the album's details show above
  // its tracks.
  import { untrack } from "svelte";
  import {
    library as api,
    queue,
    type AlbumOrder,
    type BrowsePath,
    type Group,
    type Level,
    type Track,
  } from "$lib/api";
  import { fileName, plural } from "$lib/format";
  import { library } from "$lib/state/library.svelte";
  import { player } from "$lib/state/player.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui, type MenuItem } from "$lib/state/ui.svelte";
  import { albumFeatureItems, trackFeatureItems } from "$lib/featureMenu";
  import AlbumInfo from "./AlbumInfo.svelte";
  import Art from "./Art.svelte";
  import Icon from "./Icon.svelte";
  import TrackText from "./TrackText.svelte";
  import VirtualList from "./VirtualList.svelte";

  type Entry = { kind: "group"; group: Group } | { kind: "track"; track: Track };

  const PAGE = 200;

  let total = $state(0);
  let loaded = $state(false);
  /** Pages of entries by page number; replaced when one arrives. */
  let pages = $state.raw(new Map<number, Entry[]>());
  let selected = $state(-1);
  let requested = new Set<number>();
  let request = 0;

  const albums = $derived(library.level === "album");
  const showNumbers = $derived(appSettings.display.trackColumns.includes("trackNumber"));
  const rowHeight = $derived(albums ? 60 : 40);
  const title = $derived(library.crumbs.at(-1)?.name ?? library.rule?.name ?? "Library");

  const isArtistLevel = (level: Level | undefined | null) => level === "albumArtist" || level === "artist";

  /** The artist whose node this is, if it is one. */
  const nodeArtist = $derived.by(() => {
    const depth = library.crumbs.length - 1;
    const crumb = library.crumbs[depth];
    return crumb && typeof crumb.key === "number" && isArtistLevel(library.rule?.levels[depth])
      ? { id: crumb.key, name: crumb.name }
      : null;
  });

  /** The album whose node this is, if it is one. */
  const nodeAlbum = $derived.by(() => {
    const depth = library.crumbs.length - 1;
    const crumb = library.crumbs[depth];
    return crumb && typeof crumb.key === "number" && library.rule?.levels[depth] === "album"
      ? { id: crumb.key, title: crumb.name }
      : null;
  });

  function showArtist(artist: { id: number; name: string }) {
    ui.showArtist(artist);
  }

  // A new node, or a changed library (or the rules arriving): start over.
  $effect(() => {
    void [library.ruleId, library.crumbs, library.version, library.rules];
    untrack(() => {
      request++;
      requested = new Set();
      pages = new Map();
      total = 0;
      loaded = false;
      selected = -1;
      loadPage(0);
    });
  });

  async function loadPage(page: number) {
    if (requested.has(page) || !library.rule) return;
    requested.add(page);
    const current = request;
    const result = await attempt(() => api.browse(library.ruleId, library.path, page * PAGE, PAGE));
    if (current !== request) return; // A newer node replaced this one.
    if (!result) {
      requested.delete(page);
      return;
    }
    total = result.total;
    loaded = true;
    const entries: Entry[] = [
      ...result.groups.map((group) => ({ kind: "group" as const, group })),
      ...result.tracks.map((track) => ({ kind: "track" as const, track })),
    ];
    pages = new Map(pages).set(page, entries);
  }

  function need(start: number, end: number) {
    for (let page = Math.floor(start / PAGE); page <= Math.floor((end - 1) / PAGE); page++) loadPage(page);
  }

  const entryAt = (index: number) => pages.get(Math.floor(index / PAGE))?.[index % PAGE];

  const groupPath = (group: Group): BrowsePath => [...library.path, group.key];

  function openGroup(group: Group) {
    library.open({ key: group.key, name: group.name });
  }

  const playGroup = (group: Group) => attempt(() => queue.playNode(library.ruleId, groupPath(group), true));

  const playTrack = (track: Track) =>
    attempt(() => queue.playNode(library.ruleId, library.path, false, track.id));

  const playAll = () => attempt(() => queue.playNode(library.ruleId, library.path, true));

  const shuffleAll = () =>
    attempt(async () => {
      if (!player.shuffle) await queue.setShuffle(true);
      await queue.playNode(library.ruleId, library.path, true);
    });

  function activate(index: number) {
    const entry = entryAt(index);
    if (entry?.kind === "group") openGroup(entry.group);
    else if (entry) playTrack(entry.track);
  }

  function menuFor(entry: Entry): MenuItem[] {
    if (entry.kind === "group") {
      const group = entry.group;
      const path = groupPath(group);
      const items: MenuItem[] = [
        { label: "Play", action: () => playGroup(group) },
        { label: "Play next", action: () => attempt(() => queue.addNode(library.ruleId, path, true, true)) },
        { label: "Add to queue", action: () => attempt(() => queue.addNode(library.ruleId, path, true, false)) },
      ];
      if (isArtistLevel(library.level) && typeof group.key === "number") {
        const artist = { id: group.key, name: group.name };
        items.push({ label: "Go to artist", action: () => showArtist(artist) });
      }
      if (library.level === "album" && typeof group.key === "number") {
        const album = { id: group.key, title: group.name };
        items.push(
          { label: "Find details…", action: () => (ui.dialog = { kind: "findDetails", album }) },
          { label: "Choose cover…", action: () => (ui.dialog = { kind: "chooseCover", album }) },
          ...albumFeatureItems(album),
        );
      }
      return items;
    }
    const track = entry.track;
    const items: MenuItem[] = [
      { label: "Play from here", action: () => playTrack(track) },
      { label: "Play next", action: () => attempt(() => queue.add([track.id], true)) },
      { label: "Add to queue", action: () => attempt(() => queue.add([track.id], false)) },
    ];
    if (track.artistId !== null && track.artist !== null) {
      const artist = { id: track.artistId, name: track.artist };
      items.push({ label: "Go to artist", action: () => showArtist(artist) });
    }
    items.push(...trackFeatureItems({ id: track.id, title: track.title ?? fileName(track.path) }));
    return items;
  }

  function showMenu(index: number, event: MouseEvent) {
    const entry = entryAt(index);
    if (entry) ui.openMenu(event, menuFor(entry));
  }

  function describe(group: Group) {
    const details = [plural(group.trackCount, "track")];
    if (group.albumArtist !== null) details.unshift(group.albumArtist);
    if (group.year !== null) details.push(String(group.year));
    return details.join(" · ");
  }
</script>

<section class="pane">
  <header class="node">
    <div class="heading">
      <h2>{title}</h2>
      {#if loaded}<span class="muted">{plural(total, "item")}</span>{/if}
    </div>
    <div class="actions">
      {#if albums}
        <label class="sort">
          <span class="muted">Sort</span>
          <select
            value={library.rule?.albumOrder ?? "title"}
            onchange={(event) => library.setAlbumOrder(event.currentTarget.value as AlbumOrder)}
          >
            <option value="title">Title</option>
            <option value="year">Year</option>
            <option value="dateAdded">Date added</option>
          </select>
        </label>
      {/if}
      {#if nodeArtist}
        {@const artist = nodeArtist}
        <button title="About {artist.name}" onclick={() => showArtist(artist)}><Icon name="person" /> Artist</button>
      {/if}
      <button class="primary" onclick={playAll} disabled={total === 0}><Icon name="play" /> Play</button>
      <button onclick={shuffleAll} disabled={total === 0}><Icon name="shuffle" /> Shuffle</button>
    </div>
  </header>

  {#if nodeAlbum}
    {#key nodeAlbum.id}<AlbumInfo album={nodeAlbum} />{/key}
  {/if}

  {#if library.folders.length === 0}
    <div class="empty">
      <p>Add a folder of music to start.</p>
      <button class="primary" onclick={library.addFolder}><Icon name="plus" /> Add folder…</button>
    </div>
  {:else if loaded && total === 0}
    <div class="empty"><p>Nothing here.</p></div>
  {:else}
    <div class="list">
      {#key `${library.ruleId}/${JSON.stringify(library.path)}/${library.version}`}
        <VirtualList
          count={total}
          {rowHeight}
          item={entryAt}
          label={title}
          bind:selected
          onneed={need}
          onclick={(index) => {
            const entry = entryAt(index);
            if (entry?.kind === "group") openGroup(entry.group);
          }}
          onactivate={activate}
          oncontextmenu={showMenu}
        >
          {#snippet row(entry: Entry | undefined)}
            {#if entry === undefined}
              <div class="entry"><span class="placeholder"></span></div>
            {:else if entry.kind === "group"}
              {@const group = entry.group}
              <div class="entry group">
                {#if albums}
                  <Art albumId={typeof group.key === "number" ? group.key : null} size="2.75rem" />
                {:else if library.level === "folder" || library.rule?.levels[0] === "folder"}
                  <span class="glyph"><Icon name="folder" /></span>
                {/if}
                <div class="text">
                  <span class="name">{group.name}</span>
                  <span class="muted small">{describe(group)}</span>
                </div>
                <button
                  class="icon hover-only"
                  title="Play"
                  aria-label="Play {group.name}"
                  onclick={(event) => {
                    event.stopPropagation();
                    playGroup(group);
                  }}><Icon name="play" /></button
                >
                <button
                  class="icon"
                  title="More"
                  aria-label="More actions for {group.name}"
                  onclick={(event) => {
                    event.stopPropagation();
                    ui.openMenu(event, menuFor(entry));
                  }}><Icon name="more" /></button
                >
              </div>
            {:else}
              {@const track = entry.track}
              <div class="entry track" class:playing={player.currentItem?.trackId === track.id}>
                {#if showNumbers}<span class="number muted">{track.trackNumber ?? ""}</span>{/if}
                <TrackText {track} />
                {#if track.hasPrefs && appSettings.current.features.playbackPreferences}
                  <span class="badge" title="Has playback preferences">⚙</span>
                {/if}
                <button
                  class="icon"
                  title="More"
                  aria-label="More actions for {track.title ?? fileName(track.path)}"
                  onclick={(event) => {
                    event.stopPropagation();
                    ui.openMenu(event, menuFor(entry));
                  }}><Icon name="more" /></button
                >
              </div>
            {/if}
          {/snippet}
        </VirtualList>
      {/key}
    </div>
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
    align-items: baseline;
    gap: 0.75rem;
    min-width: 0;
  }

  h2 {
    margin: 0;
    font-size: 1.35rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .sort {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-right: 0.25rem;
  }

  .list {
    flex: 1;
    min-height: 0;
    container-type: inline-size;
  }

  .entry {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    height: 100%;
    padding: 0 0.5rem 0 1rem;
    min-width: 0;
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

  .badge {
    color: var(--text-faint);
    font-size: 0.8rem;
  }

  .number {
    width: 2ch;
    text-align: right;
    font-variant-numeric: tabular-nums;
    flex: none;
  }

  .playing :global(.name) {
    color: var(--accent);
    font-weight: 600;
  }

  .glyph {
    color: var(--text-faint);
    display: flex;
  }

  .placeholder {
    display: block;
    height: 0.8rem;
    width: 40%;
    border-radius: 4px;
    background: var(--surface-2);
  }

  .hover-only {
    visibility: hidden;
  }

  :global(.row:hover) .hover-only,
  :global(.row.selected) .hover-only {
    visibility: visible;
  }

  @container (max-width: 28rem) {
    .number {
      display: none;
    }
  }

  .empty {
    display: grid;
    place-items: center;
    align-content: center;
    gap: 1rem;
    flex: 1;
    color: var(--text-muted);
  }
</style>
