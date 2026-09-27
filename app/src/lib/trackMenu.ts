// The menu for tracks wherever they're listed (PLAN.md F4): one track, or
// a selection of several. Play, play next and add to the queue; add to a
// playlist; heart and rate; Get Info, the artist and album, and show the
// file in the Finder; library radio and playback preferences (O7, O9).

import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { queue, type Track } from "$lib/api";
import { count, t } from "$lib/i18n";
import { trackFeatureItems } from "$lib/featureMenu";
import { collection } from "$lib/state/collection.svelte";
import { library } from "$lib/state/library.svelte";
import { attempt } from "$lib/state/toasts.svelte";
import { ui, type MenuItem } from "$lib/state/ui.svelte";

export type MenuTrack = Pick<
  Track,
  "id" | "title" | "artist" | "artistId" | "album" | "albumId" | "albumArtist" | "favourite" | "rating"
> & { path?: string };

/** "Add to Playlist ▸": a new one, then each playlist that takes tracks.
    `trackIds` are the tracks, or how to find them when one is chosen (an
    album's, say). */
export function playlistItems(trackIds: number[] | (() => Promise<number[]>)): MenuItem[] {
  const ids = typeof trackIds === "function" ? trackIds : async () => trackIds;
  const lists = collection.playlists.filter((playlist) => playlist.rules === null);
  return [
    { label: t("menu.newPlaylist"), action: async () => collection.newPlaylist(await ids()) },
    ...(lists.length > 0 ? [{ separator: true } as MenuItem] : []),
    ...lists.map((playlist) => ({
      label: playlist.name,
      action: async () => collection.addTo(playlist, await ids()),
    })),
  ];
}

/** "Rate ▸": no stars to five, the current one checked. */
export function ratingItems(trackIds: number[], current: number | null): MenuItem[] {
  return [
    ...[1, 2, 3, 4, 5].map((stars) => ({
      label: "★".repeat(stars),
      checked: current === stars,
      action: () => collection.setRating(trackIds, stars),
    })),
    { separator: true },
    { label: t("menu.clearRating"), action: () => collection.setRating(trackIds, null) },
  ];
}

/** The menu for `tracks`, most often the selection. `play` plays them in
    their context (from this track in the album, say); without it they're
    played as a list. */
export function trackMenu(tracks: MenuTrack[], play?: () => unknown): MenuItem[] {
  const ids = tracks.map((track) => track.id);
  const one = tracks.length === 1 ? tracks[0] : null;
  const allHearted = tracks.every((track) => track.favourite);
  const rating = tracks.every((track) => track.rating === tracks[0]?.rating) ? (tracks[0]?.rating ?? null) : null;
  const items: MenuItem[] = [
    {
      label: one ? t("menu.play") : t("menu.playCount", { count: tracks.length }),
      action: play ?? (() => attempt(() => queue.play(ids, 0))),
    },
    { label: t("menu.playNext"), action: () => attempt(() => queue.add(ids, true)) },
    { label: t("menu.addToQueue"), action: () => attempt(() => queue.add(ids, false)) },
    { label: t("menu.addToPlaylist"), items: playlistItems(ids) },
    { separator: true },
    {
      label: allHearted ? t("heart.removeShort") : t("heart.addShort"),
      action: () => collection.setFavourite("track", ids, !allHearted),
    },
    { label: t("menu.rate"), items: ratingItems(ids, rating) },
  ];
  if (one) {
    items.push(
      { separator: true },
      { label: t("menu.getInfo"), action: () => (ui.dialog = { kind: "trackInfo", trackId: one.id }) },
    );
    if (one.artistId !== null && one.artist !== null) {
      const artist = { id: one.artistId, name: one.artist };
      items.push({
        label: t("menu.goToArtist"),
        action: () => {
          library.query = "";
          ui.showArtist(artist);
        },
      });
    }
    if (one.albumId !== null && one.album !== null) {
      const album = { id: one.albumId, title: one.album, albumArtist: one.albumArtist, albumArtistId: null };
      items.push({ label: t("menu.goToAlbum"), action: () => library.showAlbum(album) });
    }
    if (one.path) {
      const path = one.path;
      items.push({ label: t("menu.showInFinder"), action: () => attempt(() => revealItemInDir(path)) });
    }
    const features = trackFeatureItems({ id: one.id, title: one.title ?? "" });
    if (features.length > 0) items.push({ separator: true }, ...features);
  }
  return items;
}

/** "3 tracks" for a drag. */
export const dragLabel = (n: number) => count("count.tracks", n);
