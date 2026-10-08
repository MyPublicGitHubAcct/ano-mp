<script lang="ts">
  // Practice mode's A–B loop (O12), set from the current position (the
  // engine jumps back on the exact sample): in the practice panel and the
  // effects workbench (X8), which records a take once round it. `loop` is
  // the loop set, read from the engine for each item once it is open (the
  // engine drops a loop when another track loads; `loopOwner`).
  import { t } from "$lib/i18n";
  import { player as api } from "$lib/api";
  import { formatTime } from "$lib/format";
  import { playback } from "$lib/state/position.svelte";
  import { player } from "$lib/state/player.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { keepLoop, loopOwner } from "$lib/workbench";

  let { loop = $bindable(null) }: { loop?: [number, number] | null } = $props();

  /** A set, waiting for B. */
  let pendingStart = $state<number | null>(null);

  const owner = $derived(loopOwner(player.currentItem?.uid ?? null, player.loaded));

  $effect(() => {
    const askedFor = owner;
    loop = null;
    pendingStart = null;
    if (askedFor === null) return;
    void api
      .practice()
      .catch(() => null)
      .then((practice) => {
        if (practice && keepLoop(askedFor, owner)) loop = practice.loop;
      });
  });

  function setA() {
    pendingStart = player.position;
    loop = null;
  }

  async function setB() {
    const start = pendingStart ?? loop?.[0] ?? 0;
    const end = player.position;
    if (end - start < 0.25) return;
    const set = await attempt(() => api.setLoop(start, end));
    if (set !== undefined) {
      loop = set;
      pendingStart = null;
    }
  }

  async function clearLoop() {
    pendingStart = null;
    if ((await attempt(() => api.setLoop(null, null))) !== undefined) loop = null;
  }
</script>

<div class="loop">
  <span class="label">{t("practice.loop")}</span>
  <button onclick={setA} disabled={!player.loaded}>{t("practice.setA")}</button>
  <button onclick={setB} disabled={!player.loaded || (pendingStart === null && loop === null)}
    >{t("practice.setB")}</button
  >
  <button onclick={clearLoop} disabled={loop === null && pendingStart === null}>{t("practice.clear")}</button>
</div>
<p class="muted small" role="status">
  {#if loop}
    {t("practice.looping", { from: formatTime(loop[0]), to: formatTime(loop[1]) })}
  {:else if pendingStart !== null}
    {t("practice.aAt", { time: formatTime(pendingStart) })}
  {:else}
    {t("practice.hint", { time: formatTime(playback.position) })}
  {/if}
</p>

<style>
  .loop {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0.35rem 0;
  }

  .label {
    width: 3.5rem;
    font-weight: 600;
    font-size: 0.88rem;
  }

  .small {
    font-size: 0.8rem;
    margin: 0 0 0.5rem;
  }
</style>
