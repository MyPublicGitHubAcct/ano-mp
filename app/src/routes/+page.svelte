<script lang="ts">
  // The player: sidebar | browser (or search results, an artist's page, the
  // releases of theirs the library lacks, or the settings) | queue, with the now-playing bar along the bottom.
  // The now-playing view takes the queue's place with the cover. Below 900 px
  // the queue becomes an overlay; below 640 px the sidebar becomes a drawer.
  import ArtistPage from "$lib/components/ArtistPage.svelte";
  import BrowsePane from "$lib/components/BrowsePane.svelte";
  import ContextMenu from "$lib/components/ContextMenu.svelte";
  import Dialogs from "$lib/components/Dialogs.svelte";
  import DiscographyPage from "$lib/components/DiscographyPage.svelte";
  import Header from "$lib/components/Header.svelte";
  import NowPlaying from "$lib/components/NowPlaying.svelte";
  import NowPlayingBar from "$lib/components/NowPlayingBar.svelte";
  import QueuePanel from "$lib/components/QueuePanel.svelte";
  import SearchResults from "$lib/components/SearchResults.svelte";
  import SettingsPage from "$lib/components/SettingsPage.svelte";
  import VisualizerView from "$lib/components/VisualizerView.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import { library } from "$lib/state/library.svelte";
  import { metadataStatus } from "$lib/state/metadata.svelte";
  import { player } from "$lib/state/player.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { loadPreference, savePreference, ui } from "$lib/state/ui.svelte";
  import { visualizer } from "$lib/state/visualizer.svelte";
  import { features } from "$lib/state/features.svelte";
  import HomeView from "$lib/components/HomeView.svelte";
  import HistoryView from "$lib/components/HistoryView.svelte";
  import HealthView from "$lib/components/HealthView.svelte";

  const SEEK_STEP = 5;

  $effect(() => {
    const stopPlayer = player.connect();
    const stopLibrary = library.connect();
    const stopMetadata = metadataStatus.connect();
    const stopVisualizer = visualizer.connect();
    const stopSettings = appSettings.connect();
    const stopFeatures = features.connect();
    return () => {
      stopFeatures();
      stopPlayer();
      stopLibrary();
      stopMetadata();
      stopVisualizer();
      stopSettings();
    };
  });

  // Remember whether the queue was showing.
  ui.queueOpen = loadPreference("queueOpen", window.innerWidth > 900);
  $effect(() => savePreference("queueOpen", ui.queueOpen));

  const typing = (target: EventTarget | null) =>
    target instanceof HTMLElement &&
    (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName));

  function onkeydown(event: KeyboardEvent) {
    const command = event.metaKey || event.ctrlKey;
    if (command && event.key.toLowerCase() === "f") {
      event.preventDefault();
      ui.searchInput?.focus();
      ui.searchInput?.select();
      return;
    }
    if (command && event.key === ",") {
      event.preventDefault();
      library.query = "";
      ui.showSettings();
      return;
    }
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
    } else if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      const forward = event.key === "ArrowRight";
      if (command) {
        if (forward) player.next();
        else player.previous();
      } else {
        player.seek(player.position + (forward ? SEEK_STEP : -SEEK_STEP));
      }
    }
  }

</script>

<svelte:window {onkeydown} />

<div
  class="app"
  class:queue-open={ui.queueOpen}
  class:sidebar-open={ui.sidebarOpen}
  class:immersive={ui.visualizerInMain && visualizer.fullscreen && library.query.trim() === ""}
>
  <aside class="sidebar"><Sidebar /></aside>
  {#if ui.sidebarOpen}
    <button class="scrim" aria-label="Close the library" onclick={() => (ui.sidebarOpen = false)}></button>
  {/if}
  <div class="header"><Header /></div>
  <main class="main">
    {#if library.query.trim() !== ""}
      <SearchResults />
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
    {:else if ui.settingsInMain}
      <SettingsPage />
    {:else if ui.mainView === "home"}
      <HomeView />
    {:else if ui.mainView === "history"}
      <HistoryView />
    {:else if ui.mainView === "health"}
      <HealthView />
    {:else}
      <BrowsePane />
    {/if}
  </main>
  {#if ui.queueOpen && (["library", "home", "history", "health"].includes(ui.mainView) || ui.artistInMain || ui.discographyInMain || library.query.trim() !== "")}
    <aside class="queue"><QueuePanel /></aside>
  {/if}
  <footer class="bar"><NowPlayingBar /></footer>
</div>

<ContextMenu />
<Dialogs />
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
