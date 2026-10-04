<script lang="ts">
  // About (PLAN.md H9): the versions, and help for a bug report: the log
  // files in the Finder, and diagnostics to paste (versions, the OS, the
  // output device, counts, the folders' states, the switches that are on
  // and the log's last lines; no paths or titles). With the versions, the
  // third-party notices (PLAN.md §8.2) and Discogs' non-affiliation notice
  // (§8.1); then newer releases (§8.2: a check by hand, the automatic
  // checks' switch, and the release's page to download from).
  import { onMount } from "svelte";
  import { diagnostics } from "$lib/api";
  import { formatDay } from "$lib/format";
  import { t } from "$lib/i18n";
  import { openWebLink } from "$lib/openLink";
  import { appSettings } from "$lib/state/settings.svelte";
  import { attempt, toasts } from "$lib/state/toasts.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import { updates } from "$lib/state/updates.svelte";

  let text = $state<string | null>(null);
  let discogsNotice = $state<string | null>(null);

  onMount(() => {
    void attempt(async () => (text = await diagnostics.text()));
    void attempt(async () => (discogsNotice = await diagnostics.discogsNotice()));
  });

  /** The lines of the Versions section. */
  const versions = $derived(
    (text ?? "")
      .split("\n## ")
      .find((section) => section.startsWith("Versions"))
      ?.split("\n")
      .slice(1)
      .filter((line) => line.trim() !== "") ?? [],
  );

  async function copy() {
    const fresh = await attempt(() => diagnostics.text());
    if (fresh === undefined) return;
    text = fresh;
    try {
      await navigator.clipboard.writeText(fresh);
      toasts.show(t("about.copied"), "info");
    } catch {
      toasts.show(t("about.copyFailed"));
    }
  }
</script>

<h3>{t("about.versions")}</h3>
{#if text === null}
  <p class="muted">{t("common.loading")}</p>
{:else}
  <ul class="versions">
    {#each versions as line (line)}
      <li>{line}</li>
    {/each}
  </ul>
{/if}
<p class="muted">{t("about.noticesHint")}</p>
<div class="actions">
  <button onclick={() => (ui.dialog = { kind: "notices" })}>{t("about.notices")}</button>
</div>
{#if discogsNotice}
  <p class="muted small">{discogsNotice}</p>
{/if}

<h3>{t("updates.title")}</h3>
{#if updates.last}
  {@const last = updates.last}
  <p>
    {#if last.newer && last.latest}
      {t("updates.newer", { version: last.latest })}
    {:else if last.latest}
      {t("updates.upToDate")}
    {:else}
      {t("updates.noRelease")}
    {/if}
    <span class="muted">{t("updates.checked", { day: formatDay(last.checkedAt) })}</span>
  </p>
{/if}
<div class="actions">
  <button disabled={updates.checking} onclick={() => updates.check()}>
    {updates.checking ? t("updates.checking") : t("updates.check")}
  </button>
  {#if updates.last?.newer && updates.last.url}
    {@const url = updates.last.url}
    <button onclick={() => openWebLink(url)}>{t("updates.download")}</button>
  {/if}
</div>
<label class="switch">
  <input
    type="checkbox"
    checked={appSettings.current.features.updateCheck}
    disabled={appSettings.saving}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      void appSettings.save((next) => (next.features.updateCheck = on));
    }}
  />
  <span>
    <span class="title">{t("updates.automatic")}</span>
    <span class="hint">{t("updates.automaticHint")}</span>
  </span>
</label>

<h3>{t("about.help")}</h3>
<p class="muted">{t("about.helpHint")}</p>
<div class="actions">
  <button onclick={() => attempt(() => diagnostics.showLogs())}>{t("about.showLogs")}</button>
  <button onclick={copy}>{t("about.copyDiagnostics")}</button>
</div>

<style>
  .versions {
    margin: 0;
    padding-left: 1.25rem;
    font-variant-numeric: tabular-nums;
  }

  .small {
    font-size: 0.8rem;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
</style>
