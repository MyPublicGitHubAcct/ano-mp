// The page's uncaught errors and unhandled rejections go to the app's log
// (PLAN.md H9), where the backend redacts and scrubs them like its own.

import { error as logError } from "@tauri-apps/plugin-log";

let installed = false;

function describe(reason: unknown): string {
  if (reason instanceof Error) return reason.stack ?? `${reason.name}: ${reason.message}`;
  try {
    return typeof reason === "string" ? reason : JSON.stringify(reason);
  } catch {
    return Object.prototype.toString.call(reason);
  }
}

/** Starts logging the page's errors; once per page. */
export function logErrors() {
  if (installed || typeof window === "undefined") return;
  installed = true;
  window.addEventListener("error", (event) => {
    const where = event.filename ? ` (${event.filename}:${event.lineno}:${event.colno})` : "";
    void logError(`${describe(event.error ?? event.message)}${where}`).catch(() => {});
  });
  window.addEventListener("unhandledrejection", (event) => {
    void logError(`unhandled rejection: ${describe(event.reason)}`).catch(() => {});
  });
}
