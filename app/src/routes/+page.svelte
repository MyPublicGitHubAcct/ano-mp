<script lang="ts">
  // The player: sidebar | browser (or search results, an artist's page, the
  // releases of theirs the library lacks, a playlist, or the settings) |
  // queue, with the now-playing bar along the bottom. The now-playing view
  // takes the queue's place with the cover. Below 900 px the queue becomes
  // an overlay; below 640 px the sidebar becomes a drawer. A library without
  // folders shows the welcome view (PLAN.md F8).
  //
  // The menu bar's items that are the page's to do arrive as `menu` events
  // (F6); files and folders dropped from the Finder play or join the library
  // (F4, F5); track changes are announced to screen readers (F18).
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { diagnostics, on, queue, shell } from "$lib/api";
  import { t } from "$lib/i18n";
  import ArtistPage from "$lib/components/ArtistPage.svelte";
  import BrowsePane from "$lib/components/BrowsePane.svelte";
  import ContextMenu from "$lib/components/ContextMenu.svelte";
  import DbRepairDialog from "$lib/components/DbRepairDialog.svelte";
  import Dialogs from "$lib/components/Dialogs.svelte";
  import DiscographyPage from "$lib/components/DiscographyPage.svelte";
  import DragGhost from "$lib/components/DragGhost.svelte";
  import FavouritesView from "$lib/components/FavouritesView.svelte";
  import Header from "$lib/components/Header.svelte";
  import NowPlaying from "$lib/components/NowPlaying.svelte";
  import NowPlayingBar from "$lib/components/NowPlayingBar.svelte";
  import PlaylistView from "$lib/components/PlaylistView.svelte";
  import QueuePanel from "$lib/components/QueuePanel.svelte";
  import SearchResults from "$lib/components/SearchResults.svelte";
  import SettingsPage from "$lib/components/SettingsPage.svelte";
  import VisualizerView from "$lib/components/VisualizerView.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import WelcomeView from "$lib/components/WelcomeView.svelte";
  import { collection } from "$lib/state/collection.svelte";
  import { folderName, library } from "$lib/state/library.svelte";
  import { metadataStatus } from "$lib/state/metadata.svelte";
  import { player } from "$lib/state/player.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { attempt, toasts } from "$lib/state/toasts.svelte";
  import { loadPreference, savePreference, ui } from "$lib/state/ui.svelte";
  import { visualizer } from "$lib/state/visualizer.svelte";
  import { features } from "$lib/state/features.svelte";
  import HomeView from "$lib/components/HomeView.svelte";
  import HistoryView from "$lib/components/HistoryView.svelte";
  import HealthView from "$lib/components/HealthView.svelte";
  import MissingFolders from "$lib/components/MissingFolders.svelte";

  const SEEK_STEP = 5;

  /** Something is being dragged in from the Finder. */
  let dropping = $state(false);
  /** The user put off repairing a damaged database until the next launch. */
  let repairLater = $state(false);

  // Two frames after the page mounted, it has painted: Rust logs the time
  // since launch, once (PLAN.md H18's launch budget).
  $effect(() => {
    requestAnimationFrame(() => requestAnimationFrame(() => void diagnostics.firstPaint().catch(() => {})));
  });

  $effect(() => {
    const stopPlayer = player.connect();
    const stopLibrary = library.connect();
    const stopMetadata = metadataStatus.connect();
    const stopVisualizer = visualizer.connect();
    const stopSettings = appSettings.connect();
    const stopFeatures = features.connect();
    const stopCollection = collection.connect();
    const menu = on("menu", (id) => onMenu(id));
    const drops = getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "enter" || payload.type === "over") dropping = true;
      else if (payload.type === "leave") dropping = false;
      else if (payload.type === "drop") {
        dropping = false;
        void dropped(payload.paths);
      }
    });
    return () => {
      stopFeatures();
      stopPlayer();
      stopLibrary();
      stopMetadata();
      stopVisualizer();
      stopSettings();
      stopCollection();
      menu.then((stop) => stop());
      drops.then((stop) => stop());
    };
  });

  // Remember whether the queue was showing.
  ui.queueOpen = loadPreference("queueOpen", window.innerWidth > 900);
  $effect(() => savePreference("queueOpen", ui.queueOpen));

  // A new track, for screen readers (F18).
  let announced: number | null = null;
  $effect(() => {
    const item = player.currentItem;
    if (!item || !player.loaded || item.uid === announced) return;
    announced = item.uid;
    ui.announce(
      item.artist
        ? t("a11y.nowPlayingBy", { title: item.title, artist: item.artist })
        : t("a11y.nowPlaying", { title: item.title }),
    );
  });

  /** Paths dropped from the Finder: files play, folders may join the library. */
  async function dropped(paths: string[]) {
    const sorted = await attempt(() => shell.sortDropped(paths));
    if (!sorted) return;
    if (sorted.files.length > 0) await attempt(() => queue.openFiles(sorted.files));
    for (const folder of sorted.folders) {
      const known = library.folders.some((existing) => existing.path === folder);
      if (known) continue;
      const add = await ask(t("drop.addFolder", { name: folderName(folder) }), {
        title: t("drop.addFolderTitle"),
        okLabel: t("drop.addFolderOk"),
      });
      if (add) await library.addFolderAt(folder);
    }
    if (sorted.files.length === 0 && sorted.folders.length === 0) toasts.show(t("drop.nothing"));
  }

  const AUDIO = [
    "mp3",
    "flac",
    "m4a",
    "m4b",
    "aac",
    "ogg",
    "oga",
    "opus",
    "wav",
    "aif",
    "aiff",
    "aifc",
    "wma",
    "wv",
    "ape",
  ];

  /** Plays the current track's album, or the search box, and so on: the menu bar's page items. */
  async function onMenu(id: string) {
    ui.menu = null;
    switch (id) {
      case "settings":
        library.query = "";
        return ui.showSettings();
      case "add-folder":
        return library.addFolder();
      case "open-files": {
        const paths = await open({ multiple: true, filters: [{ name: t("drop.audioFiles"), extensions: AUDIO }] });
        if (paths && paths.length > 0) await attempt(() => queue.openFiles(paths));
        return;
      }
      case "new-playlist":
        return collection.newPlaylist();
      case "new-smart-playlist":
        ui.dialog = { kind: "smartPlaylist", playlist: null };
        return;
      case "import-playlist":
        return collection.importPlaylist();
      case "export-playlist": {
        const playlist = ui.playlistInMain ? collection.byId(ui.playlistId) : null;
        if (playlist) return collection.exportPlaylist(playlist);
        return toasts.show(t("menuAction.openPlaylistFirst"), "info");
      }
      case "export-data":
        return collection.exportData();
      case "import-data": {
        const settings = await ask(t("menuAction.importSettings"), {
          title: t("menuAction.importTitle"),
          okLabel: t("menuAction.importWithSettings"),
          cancelLabel: t("menuAction.importWithout"),
        });
        return collection.importData(settings);
      }
      case "find":
        ui.searchInput?.focus();
        ui.searchInput?.select();
        return;
      case "get-info": {
        const track = ui.selectedTracks?.()[0];
        const trackId = track?.id ?? (player.currentItem?.external ? undefined : player.currentItem?.trackId);
        if (trackId !== undefined) ui.dialog = { kind: "trackInfo", trackId };
        return;
      }
      case "go-to-current":
        return goToCurrent();
      case "show-home":
        library.query = "";
        return ui.showView("home");
      case "show-library":
        library.query = "";
        return ui.showLibrary();
      case "show-favourites":
        library.query = "";
        return ui.showView("favourites");
      case "show-now-playing":
        library.query = "";
        return ui.showNowPlaying();
      case "show-queue":
        library.query = "";
        return ui.showQueue();
      case "show-visualizer":
        library.query = "";
        return ui.showVisualizer();
      case "toggle-queue":
        ui.queueOpen = !ui.queueOpen;
        return;
      case "shortcuts":
        ui.dialog = { kind: "shortcuts" };
        return;
    }
  }

  /** Shows the current track: its album in the browser, and the track in it. */
  function goToCurrent() {
    const item = player.currentItem;
    if (!item) return;
    library.query = "";
    if (item.albumId !== null && item.album !== null) {
      const shown = library.showAlbum({ id: item.albumId, title: item.album, albumArtist: null, albumArtistId: null });
      if (shown) return;
    }
    ui.showNowPlaying();
  }

  const typing = (target: EventTarget | null) =>
    target instanceof HTMLElement &&
    (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName));

  // The menu bar's shortcuts (⌘F, ⌘→…) work wherever the focus is; these
  // are the page's own.
  function onkeydown(event: KeyboardEvent) {
    const command = event.metaKey || event.ctrlKey;
    // A dialog handles its own keys (Escape closes it).
    if (typing(event.target) || event.altKey || ui.menu || ui.dialog) return;
    const visualizing = ui.visualizerInMain && library.query.trim() === "";
    if (event.key === "Escape" && visualizing && visualizer.fullscreen) {
      visualizer.setFullscreen(false);
    } else if (event.key === "Escape" && ui.canGoBack && library.query.trim() === "") {
      ui.back();
    } else if (visualizing && !command && event.key.toLowerCase() === "v") {
      visualizer.cycle(event.shiftKey ? -1 : 1);
    } else if (visualizing && !command && event.key.toLowerCase() === "f") {
      visualizer.setFullscreen(!visualizer.fullscreen);
    } else if (event.key === " " && !command) {
      event.preventDefault();
      player.toggle();
    } else if (!command && (event.key === "ArrowLeft" || event.key === "ArrowRight")) {
      // In a list the arrows move the selection; elsewhere they seek.
      if ((event.target as HTMLElement).closest?.("[role=listbox], [role=slider]")) return;
      event.preventDefault();
      const forward = event.key === "ArrowRight";
      player.seek(player.position + (forward ? SEEK_STEP : -SEEK_STEP));
    }
  }

  const searching = $derived(library.query.trim() !== "");
  const welcome = $derived(
    library.loaded && library.folders.length === 0 && (ui.mainView === "library" || ui.mainView === "home"),
  );
  /** Every folder is out of reach (a library on a drive that isn't plugged in, H22). */
  const allMissing = $derived(
    library.folders.length > 0 &&
      library.unavailable.length === library.folders.length &&
      (ui.mainView === "library" || ui.mainView === "home"),
  );
</script>

<svelte:window {onkeydown} />

<a class="skip" href="#main">{t("a11y.skipToMain")}</a>
<div
  class="app"
  class:queue-open={ui.queueOpen}
  class:sidebar-open={ui.sidebarOpen}
  class:immersive={ui.visualizerInMain && visualizer.fullscreen && !searching}
>
  <aside class="sidebar"><Sidebar /></aside>
  {#if ui.sidebarOpen}
    <button class="scrim" aria-label={t("a11y.closeLibrary")} onclick={() => (ui.sidebarOpen = false)}></button>
  {/if}
  <div class="header"><Header /></div>
  <main class="main" id="main" tabindex="-1">
    {#if !searching && !allMissing && !ui.settingsInMain}
      <MissingFolders />
    {/if}
    {#if searching}
      <SearchResults />
    {:else if welcome}
      <WelcomeView />
    {:else if allMissing && !ui.queueInMain && !ui.nowPlayingInMain && !ui.visualizerInMain && !ui.artistInMain && !ui.discographyInMain && !ui.playlistInMain && !ui.settingsInMain}
      <MissingFolders full />
    {:else if ui.queueInMain}
      <QueuePanel main />
    {:else if ui.nowPlayingInMain}
      <NowPlaying />
    {:else if ui.visualizerInMain}
      <VisualizerView />
    {:else if ui.artistInMain}
      <ArtistPage />
    {:else if ui.discographyInMain}
      <DiscographyPage />
    {:else if ui.playlistInMain}
      <PlaylistView />
    {:else if ui.settingsInMain}
      <SettingsPage />
    {:else if ui.mainView === "home"}
      <HomeView />
    {:else if ui.mainView === "favourites"}
      <FavouritesView />
    {:else if ui.mainView === "history"}
      <HistoryView />
    {:else if ui.mainView === "health"}
      <HealthView />
    {:else}
      <BrowsePane />
    {/if}
  </main>
  {#if ui.queueOpen && (["library", "home", "history", "health", "favourites", "playlist"].includes(ui.mainView) || ui.artistInMain || ui.discographyInMain || searching)}
    <aside class="queue"><QueuePanel /></aside>
  {/if}
  <footer class="bar"><NowPlayingBar /></footer>
</div>

{#if dropping}
  <div class="drop-overlay" aria-hidden="true"><p>{t("drop.hint")}</p></div>
{/if}

<div class="visually-hidden" role="status" aria-live="polite">{ui.announcement}</div>

<ContextMenu />
<Dialogs />
{#if library.dbCheck.state === "failed" && !repairLater}
  <DbRepairDialog check={library.dbCheck} onclose={() => (repairLater = true)} />
{/if}
<DragGhost />
<Toasts />

<style>
  .app {
    height: 100vh;
    height: 100dvh;
    display: grid;
    grid-template-columns: minmax(10rem, 15rem) minmax(0, 1fr) auto;
    grid-template-rows: auto minmax(0, 1fr) auto;
    grid-template-areas:
      "sidebar header queue"
      "sidebar main queue"
      "bar bar bar";
  }

  .sidebar {
    grid-area: sidebar;
    min-height: 0;
    background: var(--surface);
    border-right: 1px solid var(--border);
  }

  .header {
    grid-area: header;
    min-width: 0;
  }

  .main {
    grid-area: main;
    min-width: 0;
    min-height: 0;
  }

  .queue {
    grid-area: queue;
    width: clamp(16rem, 28vw, 24rem);
    min-height: 0;
    background: var(--surface);
    border-left: 1px solid var(--border);
  }

  .bar {
    grid-area: bar;
    min-width: 0;
    background: var(--surface);
    border-top: 1px solid var(--border);
  }

  .scrim {
    display: none;
  }

  .main:focus {
    outline: none;
  }

  /* Keyboard users can skip the sidebar (F18). */
  .skip {
    position: fixed;
    top: -3rem;
    left: 1rem;
    z-index: 300;
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    background: var(--accent);
    color: var(--accent-text);
  }

  .skip:focus {
    top: 1rem;
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }

  .drop-overlay {
    position: fixed;
    inset: 0;
    z-index: 150;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.35);
    pointer-events: none;
  }

  .drop-overlay p {
    padding: 1rem 1.5rem;
    border-radius: 10px;
    background: var(--surface);
    border: 2px dashed var(--accent);
    font-weight: 600;
  }

  /* Full screen: the visualizer alone. */
  .app.immersive {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    grid-template-areas: "main";
  }

  .app.immersive > :not(.main) {
    display: none;
  }

  /* The queue over the browser rather than beside it. */
  @media (max-width: 900px) {
    .app {
      grid-template-columns: minmax(10rem, 13rem) minmax(0, 1fr);
      grid-template-areas:
        "sidebar header"
        "sidebar main"
        "bar bar";
    }

    .queue {
      grid-area: 1 / 1 / 3 / -1;
      justify-self: end;
      width: min(24rem, 100%);
      z-index: 10;
      box-shadow: var(--shadow);
    }
  }

  /* The sidebar as a drawer. */
  @media (max-width: 640px) {
    .app {
      grid-template-columns: minmax(0, 1fr);
      grid-template-areas:
        "header"
        "main"
        "bar";
    }

    .sidebar {
      grid-area: 1 / 1 / 3 / -1;
      justify-self: start;
      width: min(18rem, 85%);
      z-index: 30;
      box-shadow: var(--shadow);
      display: none;
    }

    .sidebar-open .sidebar {
      display: block;
    }

    .sidebar-open .scrim {
      display: block;
      grid-area: 1 / 1 / 3 / -1;
      z-index: 25;
      border: none;
      border-radius: 0;
      background: rgb(0 0 0 / 0.35);
    }
  }
</style>
