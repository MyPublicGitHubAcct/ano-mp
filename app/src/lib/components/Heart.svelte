<script lang="ts">
  // A heart that toggles (PLAN.md F3): filled when on. Announced as a
  // toggle button to assistive technology.
  import { t } from "$lib/i18n";
  import Icon from "./Icon.svelte";

  let {
    on,
    label,
    onchange,
    size = "1rem",
    quiet = false,
  }: {
    on: boolean;
    /** What it's a heart for, e.g. the track's title. */
    label: string;
    onchange: (on: boolean) => void;
    size?: string;
    /** Only shown while its row is hovered or selected, unless on. */
    quiet?: boolean;
  } = $props();
</script>

<button
  class="icon heart"
  class:on
  class:quiet={quiet && !on}
  aria-pressed={on}
  aria-label={on ? t("heart.remove", { name: label }) : t("heart.add", { name: label })}
  title={on ? t("heart.removeShort") : t("heart.addShort")}
  onclick={(event) => {
    event.stopPropagation();
    onchange(!on);
  }}
>
  <Icon name={on ? "heart" : "heartOutline"} {size} />
</button>

<style>
  .heart {
    color: var(--text-faint);
  }

  .heart.on {
    color: var(--heart);
  }

  .quiet {
    visibility: hidden;
  }

  :global(.row:hover) .quiet,
  :global(.row.selected) .quiet,
  .quiet:focus-visible {
    visibility: visible;
  }

  @media (hover: none) {
    .quiet {
      visibility: visible;
    }
  }
</style>
