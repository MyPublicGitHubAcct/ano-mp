<script lang="ts">
  // The keyboard shortcuts (PLAN.md F6): the menus' (which work wherever
  // the focus is), and the page's own, with ⌘ written as the platform has it.
  import { onMount } from "svelte";
  import { shell } from "$lib/api";
  import { t, type MessageKey } from "$lib/i18n";
  import Dialog from "./Dialog.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const mac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

  /** "CmdOrCtrl+Shift+O" as the platform writes it: "⇧⌘O", or "Ctrl+Shift+O". */
  function keys(accelerator: string) {
    const parts = accelerator.split("+");
    const key = parts.pop() ?? "";
    const names: Record<string, string> = mac
      ? { Right: "→", Left: "←", Up: "↑", Down: "↓", Period: ".", Comma: ",", Slash: "/", Space: t("shortcuts.space") }
      : { Right: "→", Left: "←", Up: "↑", Down: "↓", Period: ".", Comma: ",", Slash: "/", Space: t("shortcuts.space") };
    const shown = names[key] ?? key;
    if (!mac) return [...parts.map((part) => (part === "CmdOrCtrl" ? "Ctrl" : part)), shown].join("+");
    const symbols: Record<string, string> = { Alt: "⌥", Shift: "⇧", CmdOrCtrl: "⌘", Ctrl: "⌃" };
    const order = ["Ctrl", "Alt", "Shift", "CmdOrCtrl"];
    return (
      parts
        .sort((a, b) => order.indexOf(a) - order.indexOf(b))
        .map((part) => symbols[part] ?? part)
        .join("") + shown
    );
  }

  const PAGE: [MessageKey, string][] = [
    ["shortcuts.seekBack", "Left"],
    ["shortcuts.seekForward", "Right"],
    ["shortcuts.select", "Shift+Up"],
    ["shortcuts.selectAll", "CmdOrCtrl+A"],
    ["shortcuts.menu", "Shift+F10"],
    ["shortcuts.remove", "Delete"],
    ["shortcuts.back", "Escape"],
    ["shortcuts.nextVisualization", "V"],
    ["shortcuts.fullScreenVisualizer", "F"],
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
  <h3>{t("shortcuts.lists")}</h3>
  <table>
    <tbody>
      {#each PAGE as [what, accelerator] (what)}
        <tr><td>{t(what)}</td><td><kbd>{keys(accelerator)}</kbd></td></tr>
      {/each}
    </tbody>
  </table>
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

  kbd {
    font: inherit;
    font-variant-numeric: tabular-nums;
    padding: 0.1rem 0.45rem;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--surface-2);
  }
</style>
