<script lang="ts">
  // About (PLAN.md H9): the versions, and help for a bug report: the log
  // files in the Finder, and diagnostics to paste (versions, the OS, the
  // output device, counts, the folders' states, the switches that are on
  // and the log's last lines; no paths or titles).
  import { onMount } from "svelte";
  import { diagnostics } from "$lib/api";
  import { t } from "$lib/i18n";
  import { attempt, toasts } from "$lib/state/toasts.svelte";

  let text = $state<string | null>(null);

  onMount(() => {
    void attempt(async () => (text = await diagnostics.text()));
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

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
</style>
