<script lang="ts">
  // The third-party notices (PLAN.md §8.2): THIRD_PARTY_NOTICES, which
  // scripts/make-notices.py writes and the app bundles, shown as it is.
  import { onMount } from "svelte";
  import { diagnostics } from "$lib/api";
  import { errorText, t } from "$lib/i18n";
  import Dialog from "./Dialog.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let text = $state<string | null>(null);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      text = await diagnostics.notices();
    } catch (e) {
      error = errorText(e);
    }
  });
</script>

<Dialog title={t("about.notices")} {onclose}>
  {#if error !== null}
    <p class="muted">{error}</p>
  {:else if text === null}
    <p class="muted">{t("common.loading")}</p>
  {:else}
    <pre>{text}</pre>
  {/if}
</Dialog>

<style>
  pre {
    margin: 0;
    font-size: 0.75rem;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }
</style>
