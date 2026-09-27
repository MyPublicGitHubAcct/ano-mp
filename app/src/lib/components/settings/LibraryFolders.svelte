<script lang="ts">
  // The library's folders: where each is, how many tracks it has and when it
  // was last scanned, with rescan and remove; add a folder, rescan them all.
  import { ask } from "@tauri-apps/plugin-dialog";
  import type { Folder } from "$lib/api";
  import { formatDay, plural } from "$lib/format";
  import { folderName, library } from "$lib/state/library.svelte";
  import Icon from "../Icon.svelte";

  async function remove(folder: Folder) {
    const confirmed = await ask(`Remove ${folder.path} from the library? The files stay where they are.`, {
      title: "Remove folder",
      kind: "warning",
      okLabel: "Remove",
    });
    if (confirmed) library.removeFolder(folder);
  }
</script>

{#if library.folders.length === 0}
  <p class="muted">No folders yet. Add the folders your music is in; each is scanned when added.</p>
{:else}
  <ul class="folders card">
    {#each library.folders as folder (folder.id)}
      <li>
        <span class="glyph"><Icon name="folder" /></span>
        <span class="text">
          <span class="name">{folderName(folder.path)}</span>
          <span class="muted small path" title={folder.path}>{folder.path}</span>
          <span class="muted small">
            {#if folder.lastScanAt === null}
              Not scanned yet
            {:else}
              {plural(folder.trackCount, "track")} · scanned {formatDay(folder.lastScanAt)}
            {/if}
          </span>
        </span>
        <button disabled={library.scanning} onclick={() => library.scan(folder.id)}>
          <Icon name="refresh" size="1rem" /> Rescan
        </button>
        <button
          class="icon"
          title="Remove from the library"
          aria-label="Remove {folder.path} from the library"
          disabled={library.scanning}
          onclick={() => remove(folder)}><Icon name="close" /></button
        >
      </li>
    {/each}
  </ul>
{/if}

{#if library.scanning}
  <p class="scan muted" role="status">
    {#if library.scanProgress}
      Scanning: read {library.scanProgress.read.toLocaleString()} of
      {library.scanProgress.toRead.toLocaleString()} new or changed files
    {:else}
      Scanning…
    {/if}
  </p>
{/if}

<div class="actions">
  <button class="primary" onclick={library.addFolder} disabled={library.scanning}>
    <Icon name="plus" /> Add folder…
  </button>
  <button onclick={() => library.scan(null)} disabled={library.scanning || library.folders.length === 0}>
    <Icon name="refresh" /> Rescan all
  </button>
</div>
<p class="hint">
  A rescan reads only files that are new or changed since the last one, and drops files that are gone. Removing
  a folder leaves its files where they are.
</p>

<style>
  .folders {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.6rem 0.5rem 0.6rem 0.8rem;
  }

  li + li {
    border-top: 1px solid var(--border);
  }

  .glyph {
    color: var(--text-faint);
    display: flex;
  }

  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .name {
    font-weight: 500;
  }

  .name,
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 0.8rem;
  }

  .scan {
    margin-top: 0.75rem;
  }

  .hint {
    margin-top: 0.75rem;
  }

  @media (max-width: 480px) {
    li {
      flex-wrap: wrap;
    }

    .text {
      flex-basis: calc(100% - 2.5rem);
    }
  }
</style>
