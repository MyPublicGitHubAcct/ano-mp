<script lang="ts">
  // A rating in whole stars (PLAN.md F3). With `onchange`, a slider the
  // keyboard moves (arrows, 0 to 5) and a click sets; clicking the rating
  // it has clears it. Without, a quiet display.
  import { t } from "$lib/i18n";
  import Icon from "./Icon.svelte";

  let {
    rating,
    onchange,
    size = "0.9rem",
    label = "",
  }: { rating: number | null; onchange?: (rating: number | null) => void; size?: string; label?: string } = $props();

  const value = $derived(rating ?? 0);

  function onkeydown(event: KeyboardEvent) {
    if (!onchange) return;
    const moves: Record<string, number> = {
      ArrowRight: value + 1,
      ArrowUp: value + 1,
      ArrowLeft: value - 1,
      ArrowDown: value - 1,
      Home: 0,
      End: 5,
    };
    if (event.key in moves) {
      event.preventDefault();
      event.stopPropagation();
      const next = Math.max(0, Math.min(5, moves[event.key]));
      onchange(next === 0 ? null : next);
    } else if (/^[0-5]$/.test(event.key)) {
      event.preventDefault();
      event.stopPropagation();
      const next = Number(event.key);
      onchange(next === 0 ? null : next);
    }
  }
</script>

{#if onchange}
  <span
    class="stars editable"
    role="slider"
    tabindex="0"
    aria-label={label ? t("rating.labelFor", { name: label }) : t("rating.label")}
    aria-valuemin={0}
    aria-valuemax={5}
    aria-valuenow={value}
    aria-valuetext={t("rating.value", { count: value })}
    {onkeydown}
  >
    {#each [1, 2, 3, 4, 5] as star (star)}
      <button
        class="icon star"
        class:on={star <= value}
        tabindex="-1"
        aria-hidden="true"
        onclick={(event) => {
          event.stopPropagation();
          onchange(star === value ? null : star);
        }}
      >
        <Icon name={star <= value ? "star" : "starOutline"} {size} />
      </button>
    {/each}
  </span>
{:else if value > 0}
  <span class="stars" role="img" aria-label={t("rating.value", { count: value })}>
    {#each [1, 2, 3, 4, 5] as star (star)}
      <span class="star" class:on={star <= value}><Icon name={star <= value ? "star" : "starOutline"} {size} /></span>
    {/each}
  </span>
{/if}

<style>
  .stars {
    display: inline-flex;
    align-items: center;
    flex: none;
  }

  .star {
    color: var(--text-faint);
    padding: 0.05rem;
  }

  .star.on {
    color: var(--star);
  }

  .editable {
    border-radius: 4px;
  }
</style>
