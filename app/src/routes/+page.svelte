<script lang="ts">
  // The player: sidebar | browser (or search results) | queue, with the
  // now-playing bar along the bottom. Below 900 px the queue becomes an
  // overlay; below 640 px the sidebar becomes a drawer.
  import BrowsePane from "$lib/components/BrowsePane.svelte";
  import ContextMenu from "$lib/components/ContextMenu.svelte";
  import Header from "$lib/components/Header.svelte";
  import NowPlayingBar from "$lib/components/NowPlayingBar.svelte";
  import QueuePanel from "$lib/components/QueuePanel.svelte";
  import SearchResults from "$lib/components/SearchResults.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import { library } from "$lib/state/library.svelte";
  import { player } from "$lib/state/player.svelte";
  import { loadPreference, savePreference, ui } from "$lib/state/ui.svelte";

  const SEEK_STEP = 5;

  $effect(() => {
    const stopPlayer = player.connect();
    const stopLibrary = library.connect();
    return () => {
      stopPlayer();
      stopLibrary();
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
    if (typing(event.target) || event.altKey || ui.menu) return;
    if (event.key === " " && !command) {
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

<div class="app" class:queue-open={ui.queueOpen} class:sidebar-open={ui.sidebarOpen}>
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
    {:else}
      <BrowsePane />
    {/if}
  </main>
  {#if ui.queueOpen && !(ui.queueInMain && library.query.trim() === "")}
    <aside class="queue"><QueuePanel /></aside>
  {/if}
  <footer class="bar"><NowPlayingBar /></footer>
</div>

<ContextMenu />
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
