<script lang="ts">
  // The offer when the launch check finds the library database damaged
  // (PLAN.md H10): restore the copy made before the last upgrade, or
  // rebuild by rescanning after saving what the user made. Either restarts
  // the app; the damaged database is kept.
  import { library as api, type DbCheck } from "$lib/api";
  import { formatDay } from "$lib/format";
  import { errorCode, errorText, t } from "$lib/i18n";
  import Dialog from "./Dialog.svelte";

  let { check, onclose }: { check: Extract<DbCheck, { state: "failed" }>; onclose: () => void } = $props();

  let busy = $state(false);
  let error = $state<string | null>(null);
  /** The export failed: a second press of Rebuild goes ahead without it. */
  let force = $state(false);

  async function restore() {
    busy = true;
    error = null;
    try {
      await api.dbRestore();
    } catch (failure) {
      error = errorText(failure);
    } finally {
      busy = false;
    }
  }

  async function rebuild() {
    busy = true;
    error = null;
    try {
      await api.dbRebuild(force);
    } catch (failure) {
      if (!force && errorCode(failure) === "dbExportFailed") {
        force = true;
        error = t("dbRepair.exportFailed");
      } else {
        error = errorText(failure);
      }
    } finally {
      busy = false;
    }
  }
</script>

<Dialog title={t("dbRepair.title")} {onclose}>
  <p>{t("dbRepair.lead")}</p>
  {#if check.copy}
    <p>
      {check.copy.writtenAt === null
        ? t("dbRepair.restoreHintUndated")
        : t("dbRepair.restoreHint", { date: formatDay(check.copy.writtenAt) })}
    </p>
  {:else}
    <p>{t("dbRepair.noCopy")}</p>
  {/if}
  <p>{t("dbRepair.rebuildHint")}</p>
  <p>{t("dbRepair.kept")}</p>
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
  <details>
    <summary>{t("dbRepair.details")}</summary>
    <ul>
      {#each check.problems as problem, index (index)}
        <li>{problem}</li>
      {/each}
    </ul>
  </details>
  {#snippet actions()}
    <button onclick={onclose} disabled={busy}>{t("dbRepair.later")}</button>
    <span class="spacer"></span>
    <button onclick={rebuild} disabled={busy}>{force ? t("dbRepair.rebuildAnyway") : t("dbRepair.rebuild")}</button>
    {#if check.copy}
      <button class="primary" onclick={restore} disabled={busy}>{t("dbRepair.restore")}</button>
    {/if}
  {/snippet}
</Dialog>

<style>
  p {
    margin: 0 0 0.75rem;
  }

  .error {
    color: var(--danger, var(--text));
    font-weight: 600;
  }

  details {
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  ul {
    margin: 0.5rem 0 0;
    padding-left: 1.25rem;
    font-family: ui-monospace, monospace;
    overflow-wrap: anywhere;
  }

  .spacer {
    flex: 1;
  }
</style>
