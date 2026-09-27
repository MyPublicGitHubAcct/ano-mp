// Menu items the optional features add to tracks and albums wherever they
// are listed: library radio (O9) and playback preferences (O7).

import { queue } from "$lib/api";
import { appSettings } from "$lib/state/settings.svelte";
import { attempt } from "$lib/state/toasts.svelte";
import { ui, type MenuItem } from "$lib/state/ui.svelte";

export function trackFeatureItems(track: { id: number; title: string }): MenuItem[] {
  const features = appSettings.current.features;
  const items: MenuItem[] = [];
  if (features.libraryRadio) items.push({ label: "Start radio", action: () => attempt(() => queue.startRadio(track.id)) });
  if (features.playbackPreferences) {
    items.push({ label: "Playback preferences…", action: () => (ui.dialog = { kind: "prefs", track, album: null }) });
  }
  return items;
}

export function albumFeatureItems(album: { id: number; title: string }): MenuItem[] {
  const features = appSettings.current.features;
  return features.playbackPreferences
    ? [{ label: "Playback preferences…", action: () => (ui.dialog = { kind: "prefs", track: null, album }) }]
    : [];
}
