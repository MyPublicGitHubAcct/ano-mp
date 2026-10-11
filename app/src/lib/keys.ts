// Keyboard shortcuts as the platform writes them (PLAN.md F6): the
// shortcuts sheet's rows and the messages that name a shortcut
// (`{keys}`). Accelerators are written as Tauri's menus take them
// ("CmdOrCtrl+Shift+O"). scripts/check-user-guide.py reads `names` below.
import { t } from "$lib/i18n";
import { commandKey } from "$lib/platform";

/** "CmdOrCtrl+Shift+O" as the platform writes it: "⇧⌘O", or "Ctrl+Shift+O";
    "CmdOrCtrl+Click" as "⌘-click", or "Ctrl+click". */
export function keys(accelerator: string): string {
  const parts = accelerator.split("+");
  const key = parts.pop() ?? "";
  const names: Record<string, string> = {
    Right: "→",
    Left: "←",
    Up: "↑",
    Down: "↓",
    Period: ".",
    Comma: ",",
    Slash: "/",
    Space: t("shortcuts.space"),
    Escape: t("shortcuts.esc"),
    Return: commandKey ? t("shortcuts.return") : t("shortcuts.enter"),
    Click: t("shortcuts.click"),
  };
  const shown = names[key] ?? key;
  if (!commandKey) return [...parts.map((part) => (part === "CmdOrCtrl" ? "Ctrl" : part)), shown].join("+");
  const symbols: Record<string, string> = { Alt: "⌥", Shift: "⇧", CmdOrCtrl: "⌘", Ctrl: "⌃" };
  const order = ["Ctrl", "Alt", "Shift", "CmdOrCtrl"];
  return (
    parts
      .sort((a, b) => order.indexOf(a) - order.indexOf(b))
      .map((part) => symbols[part] ?? part)
      .join("") +
    (key === "Click" ? "-" : "") +
    shown
  );
}

/** The Help window's Back: ⌘[ where shortcuts use ⌘, else Alt+←, as
    browsers have them. */
export const HELP_BACK = commandKey ? "CmdOrCtrl+[" : "Alt+Left";

/** Whether `event` is the Help window's Back (HELP_BACK). */
export function isHelpBack(event: KeyboardEvent): boolean {
  return commandKey ? event.metaKey && event.key === "[" : event.altKey && event.key === "ArrowLeft";
}
