// The menu bar's items that are the page's to do (`shell/menu.rs`'s
// `PAGE_ITEMS`, PLAN.md F6), run for the menu's `menu` events
// (routes/+page.svelte) and for a found feature (PLAN.md X9), which this
// opens wherever `lib/find.ts` says.

import { ask, open } from "@tauri-apps/plugin-dialog";
import { queue, shell } from "$lib/api";
import type { FindView, PageAction, Place } from "$lib/find";
import { t } from "$lib/i18n";
import { collection } from "$lib/state/collection.svelte";
import { library } from "$lib/state/library.svelte";
import { player } from "$lib/state/player.svelte";
import { attempt, toasts } from "$lib/state/toasts.svelte";
import { ui } from "$lib/state/ui.svelte";
import { AUDIO_EXTENSIONS } from "$lib/workbench";

/** Shows a view in the main area, leaving the search. */
export function showView(view: FindView) {
  library.query = "";
  switch (view) {
    case "nowPlaying":
      return ui.showNowPlaying();
    case "visualizer":
      return ui.showVisualizer();
    case "queue":
      return ui.showQueue();
    case "library":
      return ui.showLibrary();
    case "settings":
      return ui.showSettings();
    default:
      return ui.showView(view);
  }
}

/** Shows the current track: its album in the browser, and the track in it. */
function goToCurrent() {
  const item = player.currentItem;
  if (!item) return;
  library.query = "";
  if (item.albumId !== null && item.album !== null) {
    const shown = library.showAlbum({ id: item.albumId, title: item.album, albumArtist: null, albumArtistId: null });
    if (shown) return;
  }
  ui.showNowPlaying();
}

/** Plays the current track's album, or the search box, and so on: a menu bar item the page does. */
export async function runPageAction(id: PageAction | string) {
  ui.menu = null;
  switch (id as PageAction) {
    case "settings":
      return showView("settings");
    case "add-folder":
      return library.addFolder();
    case "open-files": {
      const paths = await open({
        multiple: true,
        filters: [{ name: t("drop.audioFiles"), extensions: AUDIO_EXTENSIONS }],
      });
      if (paths && paths.length > 0) await attempt(() => queue.openFiles(paths));
      return;
    }
    case "new-playlist":
      return collection.newPlaylist();
    case "new-smart-playlist":
      ui.dialog = { kind: "smartPlaylist", playlist: null };
      return;
    case "import-playlist":
      return collection.importPlaylist();
    case "export-playlist": {
      const playlist = ui.playlistInMain ? collection.byId(ui.playlistId) : null;
      if (playlist) return collection.exportPlaylist(playlist);
      return toasts.show(t("menuAction.openPlaylistFirst"), "info");
    }
    case "export-data":
      return collection.exportData();
    case "import-data": {
      const settings = await ask(t("menuAction.importSettings"), {
        title: t("menuAction.importTitle"),
        okLabel: t("menuAction.importWithSettings"),
        cancelLabel: t("menuAction.importWithout"),
      });
      return collection.importData(settings);
    }
    case "find":
      ui.searchInput?.focus();
      ui.searchInput?.select();
      return;
    case "get-info": {
      const track = ui.selectedTracks?.()[0];
      const trackId = track?.id ?? (player.currentItem?.external ? undefined : player.currentItem?.trackId);
      if (trackId !== undefined) ui.dialog = { kind: "trackInfo", trackId };
      return;
    }
    case "go-to-current":
      return goToCurrent();
    case "show-home":
      return showView("home");
    case "show-library":
      return showView("library");
    case "show-favourites":
      return showView("favourites");
    case "show-now-playing":
      return showView("nowPlaying");
    case "show-queue":
      return showView("queue");
    case "show-visualizer":
      return showView("visualizer");
    case "toggle-queue":
      ui.queueOpen = !ui.queueOpen;
      return;
    // The menu bar's own item opens it in Rust; a found feature comes here.
    case "mini-player":
      return attempt(shell.toggleMiniPlayer);
    case "shortcuts":
      ui.dialog = { kind: "shortcuts" };
      return;
  }
}

/** Opens a found feature where `lib/find.ts`'s `target` says: a view (focused, for VoiceOver), a Settings control, a menu item's action, the sidebar's playlists, or the sleep timer (over the results, which stay). */
export function openFound(place: Place) {
  switch (place.kind) {
    case "view":
      showView(place.view);
      requestAnimationFrame(() => document.getElementById("main")?.focus());
      return;
    case "settings":
      library.query = "";
      return ui.showSettings(place.section, place.control);
    case "action": {
      // Started first, so Get Info still sees the selection the results had.
      const done = runPageAction(place.action);
      if (place.action !== "find") library.query = "";
      return done;
    }
    case "playlists":
      library.query = "";
      return ui.revealPlaylists();
    case "sleepTimer":
      return ui.revealSleepTimer();
  }
}
