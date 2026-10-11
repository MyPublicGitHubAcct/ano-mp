<script lang="ts">
  // The keyboard shortcuts (PLAN.md F6): the menus' (which work wherever
  // the focus is), and the page's own, with ⌘ written as the platform has it (`keys.ts`).
  import { onMount } from "svelte";
  import { shell } from "$lib/api";
  import { t, type MessageKey } from "$lib/i18n";
  import { keys } from "$lib/keys";
  import Dialog from "./Dialog.svelte";

  let { onclose }: { onclose: () => void } = $props();

  /** A row of the page's own shortcuts: what it does, its keys (any one
      of them works), and a note on where. The sections and their rows
      follow the user guide's Appendix A. */
  type Row = [MessageKey, string[], MessageKey?];
  const PAGE: [MessageKey, Row[]][] = [
    [
      "shortcuts.lists",
      [
        ["shortcuts.seekBack", ["Left"]],
        ["shortcuts.seekForward", ["Right"]],
        ["shortcuts.move", ["Up", "Down"]],
        ["shortcuts.select", ["Shift+Up", "Shift+Down"]],
        ["shortcuts.selectAll", ["CmdOrCtrl+A"]],
        ["shortcuts.addToSelection", ["CmdOrCtrl+Click"]],
        ["shortcuts.selectRange", ["Shift+Click"]],
        ["shortcuts.keepFocused", ["Escape"]],
        ["shortcuts.menu", ["Shift+F10"], "shortcuts.orRightClick"],
        ["shortcuts.open", ["Return"]],
        ["shortcuts.remove", ["Delete"]],
        ["shortcuts.rate", ["0–5"], "shortcuts.onStars"],
        ["shortcuts.cancelDrag", ["Escape"]],
        ["shortcuts.back", ["Escape"]],
        ["shortcuts.clearSearch", ["Escape"], "shortcuts.inSearchBox"],
      ],
    ],
    [
      "shortcuts.visualizer",
      [
        ["shortcuts.nextVisualization", ["V"]],
        ["shortcuts.previousVisualization", ["Shift+V"]],
        ["shortcuts.fullScreenVisualizer", ["F"]],
        ["shortcuts.leaveVisualizer", ["Escape"]],
      ],
    ],
    [
      "shortcuts.settings",
      [
        ["shortcuts.nextSection", ["Down", "Right"], "shortcuts.inSections"],
        ["shortcuts.previousSection", ["Up", "Left"], "shortcuts.inSections"],
        ["shortcuts.closeSettings", ["Escape"]],
      ],
    ],
  ];

  let menus = $state<[string, string][]>([]);
  onMount(async () => {
    menus = await shell.shortcuts().catch(() => []);
  });
</script>

<Dialog title={t("shortcuts.title")} {onclose}>
  <h3>{t("shortcuts.menus")}</h3>
  <table>
    <tbody>
      {#each menus as [what, accelerator] (what)}
        <tr><td>{what}</td><td><kbd>{keys(accelerator)}</kbd></td></tr>
      {/each}
    </tbody>
  </table>
  {#each PAGE as [heading, rows] (heading)}
    <h3>{t(heading)}</h3>
    <table>
      <tbody>
        {#each rows as [what, accelerators, note] (what)}
          <tr>
            <td>{t(what)}</td>
            <td>
              {#each accelerators as accelerator, i (accelerator)}
                {#if i > 0}<span class="or">{t("shortcuts.or")}</span>{/if}<kbd>{keys(accelerator)}</kbd>
              {/each}
              {#if note}<span class="note">{t(note)}</span>{/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/each}
  {#snippet actions()}
    <button class="primary" onclick={onclose}>{t("dialog.close")}</button>
  {/snippet}
</Dialog>

<style>
  h3 {
    font-size: 0.9rem;
    margin: 1rem 0 0.4rem;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  td {
    padding: 0.3rem 0;
  }

  td:last-child {
    text-align: right;
  }

  tr + tr {
    border-top: 1px solid var(--border);
  }

  .or {
    margin: 0 0.35rem;
  }

  .note {
    margin-left: 0.4rem;
    color: var(--text-muted);
  }

  kbd {
    font: inherit;
    font-variant-numeric: tabular-nums;
    padding: 0.1rem 0.45rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
  }
</style>
