<script lang="ts">
  // The views (Home, Favourites, History…), the sort rules as library
  // views, the playlists (PLAN.md F1, F2), the library folders with add,
  // rescan and remove, and scan progress; the online sources with what the
  // metadata worker is doing, and the settings.
  //
  // Tracks dragged from a list drop onto a playlist or the queue (F4). A
  // folder that can't be opened (an unplugged drive, a folder moved out of
  // reach) says so, and "Locate…" points it at where it is now (F8).
  import { ask } from "@tauri-apps/plugin-dialog";
  import { queue, type Folder, type Playlist } from "$lib/api";
  import { count, t } from "$lib/i18n";
  import { collection } from "$lib/state/collection.svelte";
  import { registerDropTarget } from "$lib/state/drag.svelte";
  import { folderName, library } from "$lib/state/library.svelte";
  import { metadataStatus } from "$lib/state/metadata.svelte";
  import { features } from "$lib/state/features.svelte";
  import { player } from "$lib/state/player.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import Icon from "./Icon.svelte";

  function show(action: () => void) {
    action();
    library.query = "";
    ui.sidebarOpen = false;
  }

  const f = $derived(features.on);
  const views = $derived(
    [
      {
        id: "home" as const,
        name: t("sidebar.home"),
        icon: "home" as const,
        on: f.recentlyAdded || f.onThisDay || f.listeningHistory,
      },
      { id: "favourites" as const, name: t("sidebar.favourites"), icon: "heart" as const, on: true },
      { id: "history" as const, name: t("sidebar.history"), icon: "history" as const, on: f.listeningHistory },
      { id: "health" as const, name: t("sidebar.health"), icon: "health" as const, on: f.healthReport },
    ].filter((view) => view.on),
  );

  const isOpen = (folderId: number) =>
    ui.mainView === "library" &&
    library.rule?.levels[0] === "folder" &&
    library.crumbs[0]?.key === folderId &&
    library.query === "";

  // Tracks dropped on the queue item, or on a playlist.
  $effect(() =>
    registerDropTarget("sidebar-queue", {
      accepts: (payload) => payload.kind !== "queue",
      drop: (payload) =>
        attempt(async () => {
          const ids =
            payload.kind === "tracks" ? await payload.trackIds() : payload.kind === "playlist" ? payload.trackIds : [];
          await queue.add(ids, false);
          ui.announce(t("queue.added", { count: ids.length }));
        }),
    }),
  );

  $effect(() => {
    const stops = collection.playlists
      .filter((playlist) => playlist.rules === null)
      .map((playlist) =>
        registerDropTarget(`sidebar-playlist:${playlist.id}`, {
          accepts: (payload) =>
            payload.kind === "tracks" ||
            payload.kind === "queue" ||
            (payload.kind === "playlist" && payload.playlistId !== playlist.id),
          drop: async (payload) => {
            const ids =
              payload.kind === "tracks"
                ? await payload.trackIds()
                : payload.kind === "playlist"
                  ? payload.trackIds
                  : player.items
                      .filter((item) => payload.uids.includes(item.uid) && !item.external)
                      .map((item) => item.trackId);
            await collection.addTo(playlist, ids);
          },
        }),
      );
    return () => stops.forEach((stop) => stop());
  });

  function playlistMenu(event: MouseEvent, playlist: Playlist) {
    ui.openMenu(event, [
      { label: t("menu.play"), action: () => collection.play(playlist) },
      { label: t("library.shuffle"), action: () => collection.play(playlist, true) },
      { label: t("menu.playNext"), action: () => collection.enqueue(playlist, true) },
      { label: t("menu.addToQueue"), action: () => collection.enqueue(playlist, false) },
      { separator: true },
      {
        label: t("playlist.rename"),
        action: () => {
          ui.showPlaylist(playlist.id);
          ui.renaming = playlist.id;
        },
      },
      ...(playlist.rules
        ? [{ label: t("playlist.editRules"), action: () => (ui.dialog = { kind: "smartPlaylist", playlist }) }]
        : []),
      { label: t("playlist.export"), action: () => collection.exportPlaylist(playlist) },
      { separator: true },
      {
        label: t("playlist.delete"),
        action: async () => {
          const yes = await ask(t("playlist.deleteConfirm", { name: playlist.name }), {
            title: t("playlist.delete"),
            kind: "warning",
            okLabel: t("playlist.deleteOk"),
          });
          if (yes) await collection.remove(playlist);
        },
      },
    ]);
  }

  function newMenu(event: MouseEvent) {
    ui.openMenu(event, [
      { label: t("sidebar.newPlaylist"), action: () => collection.newPlaylist() },
      { label: t("sidebar.newSmartPlaylist"), action: () => (ui.dialog = { kind: "smartPlaylist", playlist: null }) },
      { label: t("sidebar.importPlaylist"), action: collection.importPlaylist },
    ]);
  }

  async function removeFolder(folder: Folder) {
    const remove = await ask(t("folders.removeConfirm", { path: folder.path }), {
      title: t("folders.removeTitle"),
      kind: "warning",
      okLabel: t("folders.removeOk"),
    });
    if (remove) library.removeFolder(folder);
  }

  function folderStatus(folder: Folder) {
    if (folder.available === false) return t("folders.unavailable");
    return folder.lastScanAt === null ? t("folders.notScanned") : count("count.tracks", folder.trackCount);
  }
</script>

<nav class="sidebar" aria-label={t("sidebar.label")}>
  <ul class="top">
    {#each views as view (view.id)}
      <li>
        <button
          class="item"
          class:active={ui.mainView === view.id && library.query === ""}
          aria-current={ui.mainView === view.id && library.query === "" ? "page" : undefined}
          onclick={() => show(() => ui.showView(view.id))}
        >
          <span class="with-icon"><Icon name={view.icon} size="1.1rem" /> {view.name}</span>
        </button>
      </li>
    {/each}
    <li>
      <button
        class="item"
        class:active={ui.nowPlayingInMain && library.query === ""}
        onclick={() => show(() => ui.showNowPlaying())}
      >
        <span class="with-icon"><Icon name="note" size="1.1rem" /> {t("sidebar.nowPlaying")}</span>
        <span class="muted small">{player.currentItem?.title ?? t("sidebar.nothingPlaying")}</span>
      </button>
    </li>
    <li>
      <button
        class="item"
        class:active={ui.visualizerInMain && library.query === ""}
        onclick={() => show(() => ui.showVisualizer())}
      >
        <span class="with-icon"><Icon name="wave" size="1.1rem" /> {t("sidebar.visualizer")}</span>
      </button>
    </li>
    <li data-drop="sidebar-queue">
      <button
        class="item"
        class:active={ui.queueInMain && library.query === ""}
        onclick={() => show(() => ui.showQueue())}
      >
        <span class="with-icon"><Icon name="queue" size="1.1rem" /> {t("queue.title")}</span>
        <span class="muted small">
          {player.items.length === 0 ? t("sidebar.queueEmpty") : count("count.tracks", player.items.length)}
        </span>
      </button>
    </li>
  </ul>

  <h2>{t("library.title")}</h2>
  <ul>
    {#each library.rules.filter((rule) => f.classical || !rule.levels.some((level) => level === "composer" || level === "work")) as rule (rule.id)}
      <li>
        <button
          class="item"
          class:active={ui.mainView === "library" &&
            rule.id === library.ruleId &&
            library.crumbs.length === 0 &&
            library.query === ""}
          onclick={() => show(() => library.navigate(rule.id, []))}
        >
          {rule.name}
        </button>
      </li>
    {/each}
  </ul>

  <div class="section-heading">
    <h2>{t("sidebar.playlists")}</h2>
    <button
      class="icon"
      title={t("sidebar.newPlaylistMenu")}
      aria-label={t("sidebar.newPlaylistMenu")}
      onclick={newMenu}
    >
      <Icon name="plus" />
    </button>
  </div>
  <ul>
    {#each collection.playlists as playlist (playlist.id)}
      <li class="playlist" data-drop={playlist.rules === null ? `sidebar-playlist:${playlist.id}` : undefined}>
        <button
          class="item"
          class:active={ui.playlistInMain && ui.playlistId === playlist.id && library.query === ""}
          onclick={() => show(() => ui.showPlaylist(playlist.id))}
          oncontextmenu={(event) => playlistMenu(event, playlist)}
        >
          <span class="with-icon name">
            <Icon name={playlist.rules ? "smart" : "playlist"} size="1.05rem" />
            <span class="name">{playlist.name}</span>
          </span>
        </button>
        <button
          class="icon hover-only"
          title={t("library.more")}
          aria-label={t("library.moreFor", { name: playlist.name })}
          onclick={(event) => playlistMenu(event, playlist)}><Icon name="more" size="1rem" /></button
        >
      </li>
    {:else}
      <li><p class="hint muted small">{t("sidebar.noPlaylists")}</p></li>
    {/each}
  </ul>

  <div class="section-heading">
    <h2>{t("sidebar.folders")}</h2>
    <button
      class="icon"
      title={t("sidebar.addFolder")}
      aria-label={t("sidebar.addFolder")}
      onclick={() => library.addFolder()}
      disabled={library.scanning}
    >
      <Icon name="plus" />
    </button>
    <button
      class="icon"
      title={t("sidebar.rescanAll")}
      aria-label={t("sidebar.rescanAll")}
      onclick={() => library.scan(null)}
      disabled={library.scanning || library.background || library.folders.length === 0}
    >
      <Icon name="refresh" />
    </button>
  </div>
  {#if library.scanning || library.background}
    <p class="scan muted small" role="status">
      {#if library.scanProgress && library.scanProgress.toRead > 0}
        {t("sidebar.scanProgress", { read: library.scanProgress.read, count: library.scanProgress.toRead })}
      {:else if library.background}
        {t("sidebar.checkingFolders")}
      {:else}
        {t("sidebar.scanning")}
      {/if}
    </p>
  {/if}
  <ul>
    {#each library.folders as folder (folder.id)}
      <li class="folder" class:missing={folder.available === false}>
        <button
          class="item"
          class:active={isOpen(folder.id)}
          title={folder.path}
          onclick={() => show(() => library.showFolder(folder))}
        >
          <span class="name with-icon">
            {#if folder.available === false}<Icon name="warning" size="0.95rem" />{/if}
            <span class="name">{folderName(folder.path)}</span>
          </span>
          <span class="muted small">{folderStatus(folder)}</span>
        </button>
        {#if folder.available === false}
          <button class="locate" onclick={() => library.locateFolder(folder)}>{t("folders.locate")}</button>
        {:else}
          <button
            class="icon hover-only"
            title={t("folders.rescan")}
            aria-label={t("folders.rescanPath", { path: folder.path })}
            disabled={library.scanning}
            onclick={() => library.scan(folder.id)}><Icon name="refresh" size="1rem" /></button
          >
        {/if}
        <button
          class="icon hover-only"
          title={t("folders.removeTitle")}
          aria-label={t("folders.removePath", { path: folder.path })}
          disabled={library.scanning}
          onclick={() => removeFolder(folder)}><Icon name="close" size="1rem" /></button
        >
      </li>
    {:else}
      <li><button class="item muted" onclick={() => library.addFolder()}>{t("sidebar.addFolderLong")}</button></li>
    {/each}
  </ul>

  <div class="bottom">
    <button
      class="item"
      class:active={ui.settingsInMain && ui.settingsSection === "sources" && library.query === ""}
      onclick={() => show(() => ui.showSettings("sources"))}
    >
      <span class="with-icon"><Icon name="cloud" size="1.1rem" /> {t("sidebar.sources")}</span>
      <span class="muted small" title={metadataStatus.summary}>{metadataStatus.summary}</span>
    </button>
    <button
      class="item"
      class:active={ui.settingsInMain && ui.settingsSection !== "sources" && library.query === ""}
      title={t("sidebar.settingsShortcut")}
      onclick={() => show(() => ui.showSettings(ui.settingsSection === "sources" ? "library" : undefined))}
    >
      <span class="with-icon"><Icon name="gear" size="1.1rem" /> {t("sidebar.settings")}</span>
    </button>
    {#if __DEV_TOOLS__}
      <a class="dev muted small" href="/dev">{t("sidebar.developerTools")}</a>
    {/if}
  </div>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow-y: auto;
    padding: 0.75rem 0.5rem;
  }

  h2 {
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
    margin: 0.75rem 0.5rem 0.25rem;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .item {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
    min-width: 0;
    padding: 0.35rem 0.5rem;
    border: none;
    border-radius: 6px;
    background: none;
    text-align: left;
  }

  .item:hover {
    background: var(--hover);
  }

  .item.active {
    background: var(--selected);
    color: var(--accent);
    font-weight: 600;
  }

  .top {
    margin-bottom: 0.25rem;
  }

  .with-icon {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    min-width: 0;
    max-width: 100%;
  }

  .section-heading {
    display: flex;
    align-items: center;
    margin-top: 0.75rem;
  }

  .section-heading h2 {
    flex: 1;
    margin-top: 0;
    margin-bottom: 0;
  }

  .folder,
  .playlist {
    display: flex;
    align-items: center;
    border-radius: 6px;
  }

  .missing .name {
    color: var(--text-muted);
  }

  .locate {
    font-size: 0.8rem;
    padding: 0.15rem 0.5rem;
    flex: none;
  }

  .name,
  .small {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 0.78rem;
    font-weight: normal;
  }

  .hint {
    margin: 0.2rem 0.5rem;
    white-space: normal;
  }

  .scan {
    margin: 0.25rem 0.5rem;
    white-space: normal;
  }

  .hover-only {
    visibility: hidden;
  }

  .folder:hover .hover-only,
  .folder:focus-within .hover-only,
  .playlist:hover .hover-only,
  .playlist:focus-within .hover-only {
    visibility: visible;
  }

  @media (hover: none) {
    .hover-only {
      visibility: visible;
    }
  }

  /* A drop target under a dragged track (F4). */
  :global(li[data-drop-over]) {
    box-shadow: inset 0 0 0 2px var(--accent);
    background: var(--selected);
  }

  .bottom {
    margin-top: auto;
    padding-top: 1rem;
  }

  .dev {
    display: block;
    padding: 0.5rem 0.5rem 0;
  }
</style>
