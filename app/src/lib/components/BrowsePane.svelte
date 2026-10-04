<script lang="ts">
  // One node of the library under the current sort rule: its groups (album
  // artists, albums, subfolders…) and/or its tracks (with the fields the
  // display settings choose), fetched a page at a time as they scroll into
  // view. Click or Enter opens a group; double-click or Enter plays the
  // node's tracks from a track.
  //
  // Rows can be selected together (PLAN.md F4): the menu (right-click, the
  // ⋯ button, Shift-F10) then acts on all of them, and dragging them drops
  // them on a playlist or the queue. Tracks and albums and artists can be
  // hearted, and tracks rated (F3); "Favourites only" keeps what is hearted,
  // or on a hearted album or by a hearted artist. Inside an artist, the
  // header links to their page; inside an album, the album's details show
  // above its tracks, which always keep room for a few rows.
  import { untrack } from "svelte";
  import {
    library as api,
    queue,
    type AlbumOrder,
    type BrowseFilter,
    type BrowsePath,
    type Group,
    type Level,
    type Track,
  } from "$lib/api";
  import { count, t } from "$lib/i18n";
  import { fileName } from "$lib/format";
  import { emptySelection, rowsFor, type Selection } from "$lib/selection";
  import { appearance } from "$lib/state/appearance.svelte";
  import { collection } from "$lib/state/collection.svelte";
  import { groupName, library } from "$lib/state/library.svelte";
  import { player } from "$lib/state/player.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { loadPreference, savePreference, ui, type MenuItem } from "$lib/state/ui.svelte";
  import { albumFeatureItems, artistFeatureItems } from "$lib/featureMenu";
  import { dragLabel, playlistItems, trackMenu } from "$lib/trackMenu";
  import AlbumInfo from "./AlbumInfo.svelte";
  import Art from "./Art.svelte";
  import Heart from "./Heart.svelte";
  import Icon from "./Icon.svelte";
  import Stars from "./Stars.svelte";
  import TrackText from "./TrackText.svelte";
  import VirtualList from "./VirtualList.svelte";

  type Entry = { kind: "group"; group: Group } | { kind: "track"; track: Track };

  const PAGE = 200;
  /** Rows of an album's tracks kept in view however much its details need. */
  const ALBUM_ROWS_KEPT = 4;

  let total = $state(0);
  let loaded = $state(false);
  /** Pages of entries by page number; replaced when one arrives. */
  let pages = $state.raw(new Map<number, Entry[]>());
  let selection = $state<Selection>(emptySelection);
  let favouritesOnly = $state(loadPreference("favouritesOnly", false));
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- bookkeeping, not shown
  let requested = new Set<number>();
  let request = 0;

  $effect(() => savePreference("favouritesOnly", favouritesOnly));

  const filter = $derived<BrowseFilter | null>(favouritesOnly ? { favourites: true } : null);
  const albums = $derived(library.level === "album");
  const showNumbers = $derived(appSettings.display.trackColumns.includes("trackNumber"));
  const rowHeight = $derived(appearance.rowHeight(albums ? 60 : 40));
  const title = $derived(library.crumbs.at(-1)?.name ?? library.rule?.name ?? t("library.title"));

  const isArtistLevel = (level: Level | undefined | null) =>
    level === "albumArtist" || level === "artist" || level === "composer";

  /** The artist whose node this is, if it is one. */
  const nodeArtist = $derived.by(() => {
    const depth = library.crumbs.length - 1;
    const crumb = library.crumbs[depth];
    const level = library.rule?.levels[depth];
    return crumb && typeof crumb.key === "number" && (level === "albumArtist" || level === "artist")
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

  // A new node, a changed library, hearts or ratings (or the rules arriving,
  // or the filter): start over.
  $effect(() => {
    void [library.ruleId, library.crumbs, library.version, library.rules, collection.version, favouritesOnly];
    untrack(() => {
      request++;
      requested = new Set();
      pages = new Map();
      total = 0;
      loaded = false;
      selection = emptySelection;
      loadPage(0);
    });
  });

  async function loadPage(page: number) {
    if (requested.has(page) || !library.rule) return;
    requested.add(page);
    const current = request;
    const result = await attempt(() => api.browse(library.ruleId, library.path, page * PAGE, PAGE, filter));
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
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- a copy, into $state.raw
    pages = new Map(pages).set(page, entries);
  }

  function need(start: number, end: number) {
    for (let page = Math.floor(start / PAGE); page <= Math.floor((end - 1) / PAGE); page++) loadPage(page);
  }

  const entryAt = (index: number) => pages.get(Math.floor(index / PAGE))?.[index % PAGE];

  const groupPath = (group: Group): BrowsePath => [...library.path, group.key];

  function openGroup(group: Group) {
    library.open({ key: group.key, name: groupName(group, library.level) });
  }

  const playGroup = (group: Group) =>
    attempt(() => queue.playNode(library.ruleId, groupPath(group), true, null, filter));

  const playTrack = (track: Track) =>
    attempt(() => queue.playNode(library.ruleId, library.path, false, track.id, filter));

  const playAll = () => attempt(() => queue.playNode(library.ruleId, library.path, true, null, filter));

  const shuffleAll = () =>
    attempt(async () => {
      if (!player.shuffle) await queue.setShuffle(true);
      await queue.playNode(library.ruleId, library.path, true, null, filter);
    });

  function activate(index: number) {
    const entry = entryAt(index);
    if (entry?.kind === "group") openGroup(entry.group);
    else if (entry) playTrack(entry.track);
  }

  /** The loaded entries of `rows` (rows still loading are left out). */
  const entriesOf = (rows: number[]) => rows.map(entryAt).filter((entry) => entry !== undefined);

  /** The track ids of entries, groups' in their order. */
  async function trackIdsOf(entries: Entry[]) {
    const ids: number[] = [];
    for (const entry of entries) {
      if (entry.kind === "track") ids.push(entry.track.id);
      else ids.push(...(await api.nodeTrackIds(library.ruleId, groupPath(entry.group), true, filter)));
    }
    return ids;
  }

  function groupMenu(groups: Group[]): MenuItem[] {
    const entries = groups.map((group) => ({ kind: "group" as const, group }));
    const ids = () => trackIdsOf(entries);
    const one = groups.length === 1 ? groups[0] : null;
    const items: MenuItem[] = [
      {
        label: one ? t("menu.play") : t("menu.playGroups", { count: groups.length }),
        action: () => (one ? playGroup(one) : attempt(async () => queue.play(await ids(), 0))),
      },
      { label: t("menu.playNext"), action: () => attempt(async () => queue.add(await ids(), true)) },
      { label: t("menu.addToQueue"), action: () => attempt(async () => queue.add(await ids(), false)) },
      { label: t("menu.addToPlaylist"), items: playlistItems(ids) },
    ];
    const kind = library.level === "album" ? "album" : isArtistLevel(library.level) ? "artist" : null;
    const numbered = groups.filter((group) => typeof group.key === "number");
    if (kind && numbered.length > 0) {
      const allHearted = numbered.every((group) => group.favourite);
      items.push(
        { separator: true },
        {
          label: allHearted ? t("heart.removeShort") : t("heart.addShort"),
          action: () =>
            collection.setFavourite(
              kind,
              numbered.map((group) => group.key as number),
              !allHearted,
            ),
        },
      );
    }
    if (one && typeof one.key === "number") {
      if (isArtistLevel(library.level) && library.level !== "composer") {
        const artist = { id: one.key, name: one.name };
        items.push(
          { separator: true },
          { label: t("menu.goToArtist"), action: () => ui.showArtist(artist) },
          ...artistFeatureItems(artist),
        );
      }
      if (library.level === "album") {
        const album = { id: one.key, title: one.name };
        items.push(
          { separator: true },
          { label: t("menu.findDetails"), action: () => (ui.dialog = { kind: "findDetails", album }) },
          { label: t("menu.chooseCover"), action: () => (ui.dialog = { kind: "chooseCover", album }) },
          ...albumFeatureItems(album),
        );
      }
    }
    return items;
  }

  function menuFor(index: number): MenuItem[] {
    const entries = entriesOf(rowsFor(selection, index));
    const tracks = entries.filter((entry) => entry.kind === "track").map((entry) => entry.track);
    const groups = entries.filter((entry) => entry.kind === "group").map((entry) => entry.group);
    if (groups.length > 0 && tracks.length === 0) return groupMenu(groups);
    if (tracks.length > 0 && groups.length === 0) {
      const one = tracks.length === 1 ? tracks[0] : null;
      return trackMenu(tracks, one ? () => playTrack(one) : undefined);
    }
    // Folders and files together: play or queue them all.
    const ids = () => trackIdsOf(entries);
    return [
      {
        label: t("menu.playCount", { count: entries.length }),
        action: () => attempt(async () => queue.play(await ids(), 0)),
      },
      { label: t("menu.addToQueue"), action: () => attempt(async () => queue.add(await ids(), false)) },
    ];
  }

  function showMenu(index: number, event: MouseEvent) {
    if (entryAt(index)) ui.openMenu(event, menuFor(index));
  }

  function dragRows(rows: number[]) {
    const entries = entriesOf(rows);
    if (entries.length === 0) return null;
    const label =
      entries.length === 1
        ? entries[0].kind === "group"
          ? groupName(entries[0].group, library.level)
          : (entries[0].track.title ?? fileName(entries[0].track.path))
        : entries.every((entry) => entry.kind === "track")
          ? dragLabel(entries.length)
          : count("count.items", entries.length);
    return { payload: { kind: "tracks" as const, trackIds: () => trackIdsOf(entries) }, label };
  }

  // The selection's tracks for the menus' Get Info (⌘I).
  $effect(() => {
    ui.selectedTracks = () =>
      entriesOf(rowsFor(selection, selection.focus))
        .filter((entry) => entry.kind === "track")
        .map((entry) => entry.track);
    return () => (ui.selectedTracks = null);
  });

  function describe(group: Group) {
    const details = [count("count.tracks", group.trackCount)];
    if (group.albumArtist !== null) details.unshift(group.albumArtist);
    if (group.year !== null) details.push(String(group.year));
    return details.join(" · ");
  }

  const heartKind = $derived(library.level === "album" ? "album" : isArtistLevel(library.level) ? "artist" : null);
  const showRating = $derived(appSettings.display.trackColumns.includes("rating"));
</script>

<section class="pane">
  <header class="node">
    <div class="heading">
      <h2>{title}</h2>
      {#if loaded}<span class="muted">{count("count.items", total)}</span>{/if}
    </div>
    <div class="actions">
      {#if albums}
        <label class="sort">
          <span class="muted">{t("library.sort")}</span>
          <select
            value={library.rule?.albumOrder ?? "title"}
            onchange={(event) => library.setAlbumOrder(event.currentTarget.value as AlbumOrder)}
          >
            <option value="title">{t("library.sortTitle")}</option>
            <option value="year">{t("library.sortYear")}</option>
            <option value="dateAdded">{t("library.sortDateAdded")}</option>
          </select>
        </label>
      {/if}
      <button
        class="icon toggle"
        class:on={favouritesOnly}
        aria-pressed={favouritesOnly}
        title={t("library.favouritesOnly")}
        aria-label={t("library.favouritesOnly")}
        onclick={() => (favouritesOnly = !favouritesOnly)}
      >
        <Icon name={favouritesOnly ? "heart" : "heartOutline"} />
      </button>
      {#if nodeArtist}
        {@const artist = nodeArtist}
        <button title={t("library.aboutArtist", { name: artist.name })} onclick={() => ui.showArtist(artist)}>
          <Icon name="person" />
          {t("library.artist")}
        </button>
      {/if}
      <button class="primary" onclick={playAll} disabled={total === 0}><Icon name="play" /> {t("library.play")}</button>
      <button onclick={shuffleAll} disabled={total === 0}><Icon name="shuffle" /> {t("library.shuffle")}</button>
    </div>
  </header>

  {#if nodeAlbum}
    {#key nodeAlbum.id}<AlbumInfo album={nodeAlbum} />{/key}
  {/if}

  {#if loaded && total === 0}
    <div class="empty">
      {#if favouritesOnly}
        <p>{t("library.noFavouritesHere")}</p>
        <button onclick={() => (favouritesOnly = false)}>{t("library.showEverything")}</button>
      {:else if library.crumbs.length === 0}
        <p>{t("library.emptyRule")}</p>
        <button onclick={() => library.scan(null)} disabled={library.scanning}>{t("library.rescan")}</button>
      {:else}
        <p>{t("library.nothingHere")}</p>
        <button onclick={() => library.goUp(library.crumbs.length - 1)}>{t("library.goBack")}</button>
      {/if}
    </div>
  {:else}
    <div class="list" style:min-height={nodeAlbum ? `${Math.min(total, ALBUM_ROWS_KEPT) * rowHeight}px` : null}>
      {#key `${library.ruleId}/${JSON.stringify(library.path)}/${library.version}/${favouritesOnly}`}
        <VirtualList
          count={total}
          {rowHeight}
          item={entryAt}
          label={title}
          multiple
          bind:selection
          onneed={need}
          onclick={(index, event) => {
            const entry = entryAt(index);
            const modified = event.shiftKey || event.metaKey || event.ctrlKey;
            if (entry?.kind === "group" && !modified && (event.target as HTMLElement).closest("button") === null)
              openGroup(entry.group);
          }}
          onactivate={activate}
          oncontextmenu={showMenu}
          ondragrow={dragRows}
        >
          {#snippet row(entry: Entry | undefined, index: number)}
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
                  <span class="name">{groupName(group, library.level)}</span>
                  <span class="muted small">{describe(group)}</span>
                </div>
                {#if heartKind && typeof group.key === "number"}
                  {@const id = group.key}
                  <Heart
                    on={group.favourite}
                    label={groupName(group, library.level)}
                    quiet
                    onchange={(on) => collection.setFavourite(heartKind, [id], on)}
                  />
                {/if}
                <button
                  class="icon hover-only"
                  title={t("library.play")}
                  aria-label={t("library.playName", { name: groupName(group, library.level) })}
                  onclick={(event) => {
                    event.stopPropagation();
                    playGroup(group);
                  }}><Icon name="play" /></button
                >
                <button
                  class="icon"
                  title={t("library.more")}
                  aria-label={t("library.moreFor", { name: groupName(group, library.level) })}
                  onclick={(event) => {
                    event.stopPropagation();
                    ui.openMenu(event, menuFor(index));
                  }}><Icon name="more" /></button
                >
              </div>
            {:else}
              {@const track = entry.track}
              {@const name = track.title ?? fileName(track.path)}
              <div class="entry track" class:playing={player.currentItem?.trackId === track.id}>
                {#if showNumbers}<span class="number muted">{track.trackNumber ?? ""}</span>{/if}
                <TrackText {track} />
                {#if track.hasPrefs && appSettings.current.features.playbackPreferences}
                  <span class="badge" title={t("library.hasPrefs")}>⚙</span>
                {/if}
                {#if showRating}
                  <Stars
                    rating={track.rating}
                    label={name}
                    onchange={(rating) => collection.setRating([track.id], rating)}
                  />
                {/if}
                <Heart
                  on={track.favourite}
                  label={name}
                  quiet
                  onchange={(on) => collection.setFavourite("track", [track.id], on)}
                />
                <button
                  class="icon"
                  title={t("library.more")}
                  aria-label={t("library.moreFor", { name })}
                  onclick={(event) => {
                    event.stopPropagation();
                    ui.openMenu(event, menuFor(index));
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

  .toggle {
    color: var(--text-muted);
  }

  .toggle.on {
    color: var(--heart);
  }

  .sort {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-right: 0.25rem;
  }

  .list {
    flex: 1 1 0;
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
    border-radius: var(--radius-sm);
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
    padding: 1rem;
    text-align: center;
  }

  .empty p {
    margin: 0;
    max-width: 30rem;
  }
</style>
