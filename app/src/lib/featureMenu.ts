// Menu items the optional features add to tracks, albums and artists
// wherever they are listed: library radio (O9), playback preferences (O7)
// and "More like this" (X4; for an artist, X5's outside artists too).

import { t } from "$lib/i18n";
import { queue } from "$lib/api";
import { appSettings } from "$lib/state/settings.svelte";
import { attempt } from "$lib/state/toasts.svelte";
import { ui, type MenuItem, type SimilarSeed } from "$lib/state/ui.svelte";

function moreLikeThis(seed: SimilarSeed): MenuItem[] {
  const features = appSettings.current.features;
  const on = features.recommendations || (seed.kind === "artist" && features.outsideRecommendations);
  return on ? [{ label: t("menu.moreLikeThis"), action: () => (ui.dialog = { kind: "similar", seed }) }] : [];
}

export function trackFeatureItems(track: { id: number; title: string }): MenuItem[] {
  const features = appSettings.current.features;
  const items: MenuItem[] = [];
  if (features.libraryRadio)
    items.push({ label: t("menu.startRadio"), action: () => attempt(() => queue.startRadio(track.id)) });
  items.push(...moreLikeThis({ kind: "track", id: track.id, name: track.title }));
  if (features.playbackPreferences) {
    items.push({
      label: t("menu.playbackPreferences"),
      action: () => (ui.dialog = { kind: "prefs", track, album: null }),
    });
  }
  return items;
}

export function albumFeatureItems(album: { id: number; title: string }): MenuItem[] {
  const features = appSettings.current.features;
  const items = moreLikeThis({ kind: "album", id: album.id, name: album.title });
  if (features.playbackPreferences)
    items.push({
      label: t("menu.playbackPreferences"),
      action: () => (ui.dialog = { kind: "prefs", track: null, album }),
    });
  return items;
}

export function artistFeatureItems(artist: { id: number; name: string }): MenuItem[] {
  return moreLikeThis({ kind: "artist", id: artist.id, name: artist.name });
}
