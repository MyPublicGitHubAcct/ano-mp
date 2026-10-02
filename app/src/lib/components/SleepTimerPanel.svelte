<script lang="ts">
  // The sleep timer (PLAN.md F13): stop in so many minutes (fading out over
  // the last ten seconds), at the end of this track or album, or after a
  // chosen queue item ("Stop after this track"). Counts down while it runs.
  import { onMount } from "svelte";
  import { formatTime } from "$lib/format";
  import { t } from "$lib/i18n";
  import { player } from "$lib/state/player.svelte";
  import Popover from "./Popover.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let now = $state(Date.now() / 1000);
  onMount(() => {
    const timer = setInterval(() => (now = Date.now() / 1000), 1000);
    return () => clearInterval(timer);
  });

  const MINUTES = [15, 30, 45, 60, 90];
  const sleep = $derived(player.sleep);
  const stopsAfterCurrent = $derived(player.stopAfter !== null && player.stopAfter === player.currentItem?.uid);
</script>

<Popover title={t("sleep.title")} {onclose}>
  <p class="status" role="status">
    {#if sleep?.kind === "at"}
      {t("sleep.stopsIn", { time: formatTime(Math.max(0, sleep.endsAt - now)) })}
    {:else if sleep?.kind === "endOfTrack"}
      {t("sleep.stopsEndOfTrack")}
    {:else if sleep?.kind === "endOfAlbum"}
      {t("sleep.stopsEndOfAlbum")}
    {:else if player.stopAfter !== null}
      {t("sleep.stopsAfterItem")}
    {:else}
      {t("sleep.off")}
    {/if}
  </p>
  <div class="choices" role="group" aria-label={t("sleep.title")}>
    {#each MINUTES as minutes (minutes)}
      <button
        class:on={sleep?.kind === "at" && sleep.minutes === minutes}
        aria-pressed={sleep?.kind === "at" && sleep.minutes === minutes}
        onclick={() => player.setSleep({ kind: "minutes", minutes })}>{t("sleep.minutes", { count: minutes })}</button
      >
    {/each}
    <button
      class:on={sleep?.kind === "endOfTrack"}
      aria-pressed={sleep?.kind === "endOfTrack"}
      onclick={() => player.setSleep({ kind: "endOfTrack" })}>{t("sleep.endOfTrack")}</button
    >
    <button
      class:on={sleep?.kind === "endOfAlbum"}
      aria-pressed={sleep?.kind === "endOfAlbum"}
      onclick={() => player.setSleep({ kind: "endOfAlbum" })}>{t("sleep.endOfAlbum")}</button
    >
  </div>
  <div class="footer">
    <label>
      <input
        type="checkbox"
        checked={stopsAfterCurrent}
        disabled={!player.currentItem}
        onchange={player.toggleStopAfter}
      />
      {t("queue.stopAfter")}
    </label>
    <button disabled={sleep === null} onclick={() => player.setSleep(null)}>{t("sleep.cancel")}</button>
  </div>
</Popover>

<style>
  .status {
    margin: 0 0 0.75rem;
    color: var(--text-muted);
  }

  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .choices button {
    font-size: 0.85rem;
  }

  .choices button.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
  }

  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    margin-top: 0.75rem;
  }
</style>
