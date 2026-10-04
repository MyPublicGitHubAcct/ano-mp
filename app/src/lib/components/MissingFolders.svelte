<script lang="ts">
  // Library folders that can't be read now (PLAN.md H22): one message, not a
  // dialog, listing each folder with its reason and what can be done:
  // locate it, remove it (or, for a folder that reads as empty or lost most
  // of its tracks, remove the tracks that weren't found), or keep it, which
  // hides it until the next launch. Playback of the other folders carries on.
  // With `full`, it takes the main view's place: every folder is missing,
  // as with a library on a drive that isn't plugged in.
  import { ask } from "@tauri-apps/plugin-dialog";
  import type { Folder, FolderState } from "$lib/api";
  import { count, t, type MessageKey } from "$lib/i18n";
  import { folderName, library } from "$lib/state/library.svelte";
  import Icon from "./Icon.svelte";

  let { full = false }: { full?: boolean } = $props();

  const shown = $derived(full ? library.unavailable : library.unavailable.filter((f) => !library.kept.has(f.id)));

  const REASON: Record<Exclude<FolderState, "available">, MessageKey> = {
    missing: "folderState.missing",
    empty: "folderState.empty",
    mostlyGone: "folderState.mostlyGone",
    inTrash: "folderState.inTrash",
    noPermission: "folderState.noPermission",
  };

  const reason = (folder: Folder) => {
    const state = folder.status?.state ?? "missing";
    return t(REASON[state === "available" ? "missing" : state]);
  };

  /** A folder that's there but lost its tracks: they can go. Otherwise the folder can. */
  const tracksGone = (folder: Folder) => folder.status?.state === "empty" || folder.status?.state === "mostlyGone";

  async function remove(folder: Folder) {
    const confirmed = await ask(t("folders.removeConfirm", { path: folder.path }), {
      title: t("folders.removeTitle"),
      kind: "warning",
      okLabel: t("folders.removeOk"),
    });
    if (confirmed) await library.removeFolder(folder);
  }
</script>

{#if shown.length > 0}
  <section class="missing" class:full aria-label={t("missing.label")} role={full ? undefined : "status"}>
    <header>
      <Icon name="warning" />
      <div>
        <h2>{full ? t("missing.allTitle") : count("missing.title", shown.length)}</h2>
        <p class="muted">{full ? t("missing.allLead") : t("missing.lead")}</p>
      </div>
    </header>
    <ul>
      {#each shown as folder (folder.id)}
        <li>
          <div class="text">
            <span class="name" title={folder.path}>{folderName(folder.path)}</span>
            <span class="muted small path">{folder.path}</span>
            <span class="small">{reason(folder)}</span>
          </div>
          <div class="actions">
            <button onclick={() => library.locateFolder(folder)}>{t("folders.locate")}</button>
            {#if tracksGone(folder)}
              <button disabled={library.scanning} onclick={() => library.removeMissing(folder)}
                >{t("missing.removeTracks")}</button
              >
            {:else}
              <button disabled={library.scanning} onclick={() => remove(folder)}>{t("missing.remove")}</button>
            {/if}
            {#if !full}
              <button title={t("missing.keepHint")} onclick={() => library.kept.add(folder.id)}
                >{t("missing.keep")}</button
              >
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .missing {
    margin: 0.75rem 1rem 0;
    padding: 0.75rem 1rem;
    border: 1px solid var(--border);
    border-left: 4px solid var(--star);
    border-radius: var(--radius-lg);
    background: var(--surface);
  }

  .missing.full {
    max-width: 44rem;
    margin: 3rem auto;
    padding: 1.5rem;
  }

  header {
    display: flex;
    gap: 0.75rem;
    align-items: flex-start;
    color: var(--star);
  }

  header div {
    color: var(--text);
  }

  h2 {
    margin: 0;
    font-size: 1rem;
  }

  .full h2 {
    font-size: 1.3rem;
  }

  header p {
    margin: 0.25rem 0 0;
  }

  ul {
    list-style: none;
    margin: 0.5rem 0 0;
    padding: 0;
  }

  li {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem 1rem;
    align-items: center;
    justify-content: space-between;
    padding: 0.5rem 0;
    border-top: 1px solid var(--border);
  }

  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1 1 16rem;
  }

  .name {
    font-weight: 600;
  }

  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .small {
    font-size: 0.85rem;
  }
</style>
