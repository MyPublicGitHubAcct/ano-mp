<script lang="ts">
  // A track row's text, as the display settings ask: the title with the
  // chosen fields beside it in columns when the list is wide, or under it
  // when it's narrow (below 44rem; the list must be an inline-size
  // container), then the length if chosen. The track number, if chosen, is
  // the row's to show.
  import type { Track } from "$lib/api";
  import { columnText, isShortColumn, textColumns } from "$lib/columns";
  import { fileName } from "$lib/format";
  import { appSettings } from "$lib/state/settings.svelte";

  let { track }: { track: Track } = $props();

  const columns = $derived(textColumns(appSettings.display.trackColumns));
  const showLength = $derived(appSettings.display.trackColumns.includes("duration"));
  const under = $derived(
    columns
      .map((column) => columnText(column, track))
      .filter(Boolean)
      .join(" · "),
  );
</script>

<span class="text">
  <span class="name">{track.title ?? fileName(track.path)}</span>
  {#if under}<span class="muted small under">{under}</span>{/if}
</span>
{#each columns as column (column)}
  <span class="column muted" class:short={isShortColumn(column)}>{columnText(column, track)}</span>
{/each}
{#if showLength}<span class="muted time">{columnText("duration", track)}</span>{/if}

<style>
  .text {
    display: flex;
    flex-direction: column;
    flex: 2 1 0;
    min-width: 0;
  }

  .name,
  .small,
  .column {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 0.8rem;
  }

  .column {
    display: none;
    flex: 1 1 0;
    min-width: 0;
    font-size: 0.9rem;
  }

  .column.short {
    flex: 0 0 5.5rem;
    font-variant-numeric: tabular-nums;
  }

  .time {
    flex: none;
    font-variant-numeric: tabular-nums;
  }

  @container (min-width: 44rem) {
    .under {
      display: none;
    }

    .column {
      display: block;
    }
  }
</style>
