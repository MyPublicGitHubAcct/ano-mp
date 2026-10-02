<script lang="ts">
  // A track row's text, as the display settings ask: the title with the
  // chosen fields beside it in columns when the list is wide, or under it
  // when it's narrow (below 44rem; the list must be an inline-size
  // container), then the length if chosen. The track number, if chosen, is
  // the row's to show.
  import type { Track } from "$lib/api";
  import { columnText, isShortColumn, textColumns } from "$lib/columns";
  import { FOLDER_SHORT, unavailableState } from "$lib/folders";
  import { fileName } from "$lib/format";
  import { t } from "$lib/i18n";
  import { library } from "$lib/state/library.svelte";
  import { appSettings } from "$lib/state/settings.svelte";

  let { track }: { track: Track } = $props();

  /** Its folder can't be read now (H22b): dimmed, with the folder's reason. */
  const unavailable = $derived(unavailableState(library.unreadable, track.folderId));

  const columns = $derived(textColumns(appSettings.display.trackColumns));
  const showLength = $derived(appSettings.display.trackColumns.includes("duration"));
  const under = $derived(
    columns
      .map((column) => columnText(column, track))
      .filter(Boolean)
      .join(" · "),
  );
</script>

<span class="text" class:unavailable title={unavailable ? t(FOLDER_SHORT[unavailable]) : undefined}>
  <span class="name">{track.title ?? fileName(track.path)}</span>
  {#if under}<span class="muted small under">{under}</span>{/if}
</span>
{#each columns as column (column)}
  <span class="column muted" class:short={isShortColumn(column)} class:unavailable>{columnText(column, track)}</span>
{/each}
{#if showLength}<span class="muted time" class:unavailable>{columnText("duration", track)}</span>{/if}

<style>
  .unavailable {
    opacity: 0.45;
  }

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
