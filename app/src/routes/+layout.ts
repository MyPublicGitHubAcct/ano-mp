// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import { logErrors } from "$lib/logErrors";
import { appSettings } from "$lib/state/settings.svelte";

export const ssr = false;

/** The settings, which much of the UI reads as it renders. */
export async function load() {
  logErrors();
  await appSettings.load();
}
