<script lang="ts">
  // The library's folders: where each is, how many tracks it has and when it
  // was last scanned, with rescan and remove, or "Locate…" for one that
  // can't be opened (PLAN.md F8); add a folder, rescan them all. Then
  // keeping the library in step with the disk (F9), and exporting and
  // importing what you've made of it (F20).
  import { ask } from "@tauri-apps/plugin-dialog";
  import type { Folder, FolderState } from "$lib/api";
  import { formatDay } from "$lib/format";
  import { count, t, type MessageKey } from "$lib/i18n";
  import { collection } from "$lib/state/collection.svelte";
  import { folderName, library } from "$lib/state/library.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import Icon from "../Icon.svelte";

  const settings = $derived(appSettings.current.library);

  /** Why a folder can't be read (PLAN.md H22). */
  const LONG: Record<FolderState, MessageKey> = {
    available: "folders.unavailableLong",
    missing: "folderState.missing",
    empty: "folderState.empty",
    mostlyGone: "folderState.mostlyGone",
    inTrash: "folderState.inTrash",
    noPermission: "folderState.noPermission",
  };

  async function remove(folder: Folder) {
    const confirmed = await ask(t("folders.removeConfirm", { path: folder.path }), {
      title: t("folders.removeTitle"),
      kind: "warning",
      okLabel: t("folders.removeOk"),
    });
    if (confirmed) library.removeFolder(folder);
  }

  async function importData() {
    const settings = await ask(t("menuAction.importSettings"), {
      title: t("menuAction.importTitle"),
      okLabel: t("menuAction.importWithSettings"),
      cancelLabel: t("menuAction.importWithout"),
    });
    await collection.importData(settings);
  }
</script>

{#if library.folders.length === 0}
  <p class="muted">{t("folders.none")}</p>
{:else}
  <ul class="folders card">
    {#each library.folders as folder (folder.id)}
      <li class:missing={folder.available === false}>
        <span class="glyph"><Icon name={folder.available === false ? "warning" : "folder"} /></span>
        <span class="text">
          <span class="name">{folderName(folder.path)}</span>
          <span class="muted small path" title={folder.path}>{folder.path}</span>
          <span class="muted small">
            {#if folder.available === false}
              {t(LONG[folder.status?.state ?? "available"])}
            {:else if folder.lastScanAt === null}
              {t("folders.notScannedYet")}
            {:else}
              {t("folders.scanned", {
                tracks: count("count.tracks", folder.trackCount),
                day: formatDay(folder.lastScanAt),
              })}{#if folder.datalessCount > 0}
                · {t("folders.dataless", { count: folder.datalessCount })}{/if}
            {/if}
          </span>
        </span>
        {#if folder.available === false}
          <button onclick={() => library.locateFolder(folder)}>{t("folders.locate")}</button>
        {:else}
          <button disabled={library.scanning} onclick={() => library.scan(folder.id)}>
            <Icon name="refresh" size="1rem" />
            {t("folders.rescan")}
          </button>
        {/if}
        <button
          class="icon"
          title={t("folders.removeTitle")}
          aria-label={t("folders.removePath", { path: folder.path })}
          disabled={library.scanning}
          onclick={() => remove(folder)}><Icon name="close" /></button
        >
      </li>
    {/each}
  </ul>
{/if}

{#if library.scanning || library.background}
  <p class="scan muted" role="status">
    {#if library.scanProgress && library.scanProgress.toRead > 0}
      {t("sidebar.scanProgress", { read: library.scanProgress.read, count: library.scanProgress.toRead })}
    {:else}
      {t("sidebar.scanning")}
    {/if}
  </p>
{/if}

<div class="actions">
  <button class="primary" onclick={() => library.addFolder()} disabled={library.scanning}>
    <Icon name="plus" />
    {t("folders.add")}
  </button>
  <button onclick={() => library.scan(null)} disabled={library.scanning || library.folders.length === 0}>
    <Icon name="refresh" />
    {t("folders.rescanAll")}
  </button>
</div>
<p class="hint">{t("folders.hint")}</p>

<h3>{t("folders.keepUp")}</h3>
<label class="switch">
  <input
    id="setting-rescanAtLaunch"
    type="checkbox"
    checked={settings.rescanAtLaunch}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      appSettings.save((next) => (next.library.rescanAtLaunch = on));
    }}
  />
  <span>
    <span class="title">{t("folders.rescanAtLaunch")}</span>
    <span class="hint">{t("folders.rescanAtLaunchHint")}</span>
  </span>
</label>
<label class="switch">
  <input
    id="setting-watchFolders"
    type="checkbox"
    checked={settings.watchFolders}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      appSettings.save((next) => (next.library.watchFolders = on));
    }}
  />
  <span>
    <span class="title">{t("folders.watch")}</span>
    <span class="hint">{t("folders.watchHint")}</span>
  </span>
</label>

<h3>{t("data.title")}</h3>
<p class="hint">{t("data.hint")}</p>
<div class="actions">
  <button onclick={collection.exportData}><Icon name="export" /> {t("data.export")}</button>
  <button onclick={importData}><Icon name="import" /> {t("data.import")}</button>
</div>

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

  .missing .glyph {
    color: var(--danger);
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
