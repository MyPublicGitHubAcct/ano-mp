// Layout and navigation (`state/ui.svelte.ts`): which view the main area
// shows, going back to where a view was opened from, the settings' section
// and control, announcements, and per-viewer preferences.

import { describe, expect, test } from "vitest";
import { fakeBackend } from "./backend";

async function load() {
  fakeBackend();
  return import("$lib/state/ui.svelte");
}

const artist = (id: number) => ({ id, name: `Artist ${id}` });

describe("back", () => {
  test("returns through the views opened, to the library", async () => {
    const { ui } = await load();
    ui.showNowPlaying();
    ui.showArtist(artist(1));
    ui.showArtist(artist(2));
    ui.showPlaylist(5);
    expect([ui.mainView, ui.playlistInMain]).toEqual(["playlist", true]);
    ui.back();
    expect([ui.mainView, ui.artist?.id]).toEqual(["artist", 2]);
    ui.back();
    expect([ui.mainView, ui.artist?.id]).toEqual(["artist", 1]);
    ui.back();
    expect([ui.mainView, ui.artist]).toEqual(["nowPlaying", null]);
    ui.back();
    expect([ui.mainView, ui.canGoBack]).toEqual(["library", false]);
    ui.back();
    expect(ui.mainView).toBe("library");
  });

  test("opening what is already shown adds no step", async () => {
    const { ui } = await load();
    ui.showArtist(artist(1));
    ui.showArtist(artist(1));
    ui.showSettings();
    ui.showSettings("effects");
    ui.back();
    expect([ui.mainView, ui.artist?.id]).toEqual(["artist", 1]);
    ui.back();
    expect(ui.mainView).toBe("library");
  });

  test("the library and the queue start over", async () => {
    const { ui } = await load();
    ui.showView("home");
    ui.showView("history");
    ui.showQueue();
    expect([ui.queueInMain, ui.canGoBack]).toEqual([true, false]);
    ui.showView("favourites");
    ui.back();
    expect(ui.mainView).toBe("queue");
  });

  test("remembers the last twenty steps", async () => {
    const { ui } = await load();
    for (let id = 1; id <= 25; id++) ui.showArtist(artist(id));
    let steps = 0;
    while (ui.canGoBack) {
      ui.back();
      steps++;
    }
    expect(steps).toBe(21);
  });
});

describe("views", () => {
  test("opening one closes the sidebar's drawer", async () => {
    const { ui } = await load();
    ui.sidebarOpen = true;
    ui.showVisualizer();
    expect([ui.visualizerInMain, ui.sidebarOpen]).toEqual([true, false]);
  });

  test("the settings open at a section, and land on a control once", async () => {
    const { ui } = await load();
    ui.showSettings("effects", "setting-freeze");
    expect([ui.settingsInMain, ui.settingsSection, ui.settingsControl]).toEqual([true, "effects", "setting-freeze"]);
    ui.showSettings();
    expect([ui.settingsSection, ui.settingsControl]).toEqual(["effects", null]);
  });

  test("revealing the playlists opens the sidebar each time", async () => {
    const { ui } = await load();
    ui.revealPlaylists();
    ui.sidebarOpen = false;
    ui.revealPlaylists();
    ui.revealSleepTimer();
    expect([ui.playlistsReveal, ui.sidebarOpen, ui.sleepTimerReveal]).toEqual([2, true, 1]);
  });
});

test("the same announcement twice is announced twice", async () => {
  const { ui } = await load();
  const { watch, flush } = await import("./reactive.svelte");
  const heard: string[] = [];
  const reader = watch(() => heard.push(ui.announcement));
  ui.announce("Added");
  await Promise.resolve();
  flush();
  ui.announce("Added");
  await Promise.resolve();
  flush();
  expect(heard.filter((text) => text === "Added")).toHaveLength(2);
  reader.stop();
});

describe("preferences", () => {
  test("round-trip through storage, with a fallback", async () => {
    const { loadPreference, savePreference } = await load();
    expect(loadPreference("ruleId", "album-artist")).toBe("album-artist");
    savePreference("ruleId", "genre");
    expect(loadPreference("ruleId", "album-artist")).toBe("genre");
  });

  test("something unreadable in storage reads as the fallback", async () => {
    const { loadPreference } = await load();
    localStorage.setItem("anomp.ruleId", "{not json");
    expect(loadPreference("ruleId", "album-artist")).toBe("album-artist");
  });
});
