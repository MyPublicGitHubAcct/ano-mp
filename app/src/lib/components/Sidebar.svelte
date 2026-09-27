<script lang="ts">
  // The sort rules as library views, the library folders with add, rescan
  // and remove, and scan progress; and the online sources with what the
  // metadata worker is doing.
  import { ask } from "@tauri-apps/plugin-dialog";
  import { plural } from "$lib/format";
  import { folderName, library } from "$lib/state/library.svelte";
  import { metadataStatus } from "$lib/state/metadata.svelte";
  import { player } from "$lib/state/player.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import Icon from "./Icon.svelte";

  function show(action: () => void) {
    action();
    library.query = "";
    ui.sidebarOpen = false;
  }

  const isOpen = (folderId: number) =>
    ui.mainView === "library" &&
    library.rule?.levels[0] === "folder" &&
    library.crumbs[0]?.key === folderId &&
    library.query === "";
</script>

<nav class="sidebar" aria-label="Library">
  <ul class="top">
    <li>
      <button
        class="item"
        class:active={ui.nowPlayingInMain && library.query === ""}
        onclick={() => {
          library.query = "";
          ui.showNowPlaying();
        }}
      >
        <span class="with-icon"><Icon name="note" size="1.1rem" /> Now Playing</span>
        <span class="muted small">{player.currentItem?.title ?? "nothing"}</span>
      </button>
    </li>
    <li>
      <button
        class="item"
        class:active={ui.visualizerInMain && library.query === ""}
        onclick={() => {
          library.query = "";
          ui.showVisualizer();
        }}
      >
        <span class="with-icon"><Icon name="wave" size="1.1rem" /> Visualizer</span>
      </button>
    </li>
    <li>
      <button
        class="item"
        class:active={ui.queueInMain && library.query === ""}
        onclick={() => {
          library.query = "";
          ui.showQueue();
        }}
      >
        <span class="with-icon"><Icon name="queue" size="1.1rem" /> Queue</span>
        <span class="muted small">
          {player.items.length === 0 ? "empty" : plural(player.items.length, "track")}
        </span>
      </button>
    </li>
  </ul>

  <h2>Library</h2>
  <ul>
    {#each library.rules as rule (rule.id)}
      <li>
        <button
          class="item"
          class:active={ui.mainView === "library" && rule.id === library.ruleId && library.crumbs.length === 0 && library.query === ""}
          onclick={() => show(() => library.navigate(rule.id, []))}
        >
          {rule.name}
        </button>
      </li>
    {/each}
  </ul>

  <div class="folders-heading">
    <h2>Folders</h2>
    <button class="icon" title="Add a folder" aria-label="Add a folder" onclick={library.addFolder} disabled={library.scanning}>
      <Icon name="plus" />
    </button>
    <button
      class="icon"
      title="Rescan all folders"
      aria-label="Rescan all folders"
      onclick={() => library.scan(null)}
      disabled={library.scanning || library.folders.length === 0}
    >
      <Icon name="refresh" />
    </button>
  </div>
  {#if library.scanning}
    <p class="scan muted small" role="status">
      {#if library.scanProgress}
        Scanning: read {library.scanProgress.read.toLocaleString()} of
        {library.scanProgress.toRead.toLocaleString()} new or changed files
      {:else}
        Scanning…
      {/if}
    </p>
  {/if}
  <ul>
    {#each library.folders as folder (folder.id)}
      <li class="folder">
        <button class="item" class:active={isOpen(folder.id)} title={folder.path} onclick={() => show(() => library.showFolder(folder))}>
          <span class="name">{folderName(folder.path)}</span>
          <span class="muted small">
            {folder.lastScanAt === null ? "not scanned" : plural(folder.trackCount, "track")}
          </span>
        </button>
        <button
          class="icon hover-only"
          title="Rescan"
          aria-label="Rescan {folder.path}"
          disabled={library.scanning}
          onclick={() => library.scan(folder.id)}><Icon name="refresh" size="1rem" /></button
        >
        <button
          class="icon hover-only"
          title="Remove from the library"
          aria-label="Remove {folder.path} from the library"
          disabled={library.scanning}
          onclick={async () => {
            const remove = await ask(`Remove ${folder.path} from the library? The files stay where they are.`, {
              title: "Remove folder",
              kind: "warning",
              okLabel: "Remove",
            });
            if (remove) library.removeFolder(folder);
          }}><Icon name="close" size="1rem" /></button
        >
      </li>
    {:else}
      <li><button class="item muted" onclick={library.addFolder}>Add a folder…</button></li>
    {/each}
  </ul>

  <div class="bottom">
    <button
      class="item"
      class:active={ui.servicesInMain && library.query === ""}
      onclick={() => {
        library.query = "";
        ui.showServices();
      }}
    >
      <span class="with-icon"><Icon name="cloud" size="1.1rem" /> Online sources</span>
      <span class="muted small" title={metadataStatus.summary}>{metadataStatus.summary}</span>
    </button>
    <a class="dev muted small" href="/dev">Developer tools</a>
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
  }

  .folders-heading {
    display: flex;
    align-items: center;
    margin-top: 0.75rem;
  }

  .folders-heading h2 {
    flex: 1;
    margin-top: 0;
    margin-bottom: 0;
  }

  .folder {
    display: flex;
    align-items: center;
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

  .scan {
    margin: 0.25rem 0.5rem;
    white-space: normal;
  }

  .hover-only {
    visibility: hidden;
  }

  .folder:hover .hover-only,
  .folder:focus-within .hover-only {
    visibility: visible;
  }

  @media (hover: none) {
    .hover-only {
      visibility: visible;
    }
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
