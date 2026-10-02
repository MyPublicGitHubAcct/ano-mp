// Opening web links in the browser rather than the webview (PLAN.md H3).
import { openUrl } from "@tauri-apps/plugin-opener";
import { webLink } from "$lib/links";
import { attempt } from "$lib/state/toasts.svelte";

/** Opens `url` in the browser if it is a web link (`webLink`); a failure shows as a toast. */
export function openWebLink(url: string) {
  const link = webLink(url);
  if (link) attempt(() => openUrl(link));
}

/** An `<a>`'s click handler: opens its `href` in the browser, never in the webview. */
export function openLink(event: MouseEvent) {
  event.preventDefault();
  const anchor = event.currentTarget as HTMLAnchorElement;
  if (anchor.hasAttribute("href")) openWebLink(anchor.href);
}
