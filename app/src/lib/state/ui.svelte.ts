// Layout and transient UI state.

import type { ArtistInfo, Playlist } from "$lib/api";
import type { MenuTrack } from "$lib/trackMenu";

/** An item of a context menu: an action, a submenu (`items`), or a separator. */
export type MenuItem =
  | { label: string; action: () => unknown; disabled?: boolean; checked?: boolean; items?: undefined }
  | { label: string; items: MenuItem[]; disabled?: boolean; action?: undefined; checked?: undefined }
  | { separator: true; label?: undefined; action?: undefined; items?: undefined };

export type MainView =
  | "library"
  | "queue"
  | "nowPlaying"
  | "visualizer"
  | "artist"
  | "discography"
  | "settings"
  | "home"
  | "history"
  | "health"
  | "favourites"
  | "playlist";
/** The parts of the settings view. */
export type SettingsSection =
  | "library"
  | "sorting"
  | "display"
  | "appearance"
  | "playback"
  | "equaliser"
  | "effects"
  | "visualizer"
  | "sources"
  | "features"
  | "general"
  | "about";
export type ArtistRef = { id: number; name: string };
export type AlbumRef = { id: number; title: string };
/** What "More like this" (X4) is about. */
export type SimilarSeed = { kind: "track" | "album" | "artist"; id: number; name: string };

/** The modal dialog showing, if any. */
export type Dialog =
  | { kind: "findDetails"; album: AlbumRef }
  | { kind: "chooseCover"; album: AlbumRef }
  | { kind: "findArtist"; artist: ArtistRef; info: ArtistInfo }
  | { kind: "prefs"; track: { id: number; title: string } | null; album: AlbumRef | null }
  /** A smart playlist's rules: a new one (null), or one to edit (F2). */
  | { kind: "smartPlaylist"; playlist: Playlist | null }
  /** Get Info for a track (F16). */
  | { kind: "trackInfo"; trackId: number }
  /** The keyboard shortcuts (F6). */
  | { kind: "shortcuts" }
  /** The third-party notices (Settings › About, PLAN.md §8.2). */
  | { kind: "notices" }
  /** "More like this" for a track, an album or an artist (X4). */
  | { kind: "similar"; seed: SimilarSeed };

/** Views that `back` returns from. */
const OPENED: MainView[] = [
  "nowPlaying",
  "visualizer",
  "artist",
  "discography",
  "settings",
  "home",
  "history",
  "health",
  "favourites",
  "playlist",
];

class Ui {
  /** What the main area shows when not searching: the library browser, the
      queue, the current track with its cover, or an artist's page. */
  mainView = $state<MainView>("library");
  /** The artist the artist and discography views show. */
  artist = $state.raw<ArtistRef | null>(null);
  /** The playlist the playlist view shows. */
  playlistId = $state<number | null>(null);
  /** A playlist just made, whose name is being typed. */
  renaming = $state<number | null>(null);
  /** The queue panel beside the main area (a column when wide, an overlay
      when narrow); hidden while the main area shows the queue or the
      current track. */
  queueOpen = $state(true);
  dialog = $state.raw<Dialog | null>(null);
  /** The part of the settings view showing. */
  settingsSection = $state<SettingsSection>("library");
  /** The views the now-playing, visualizer, artist, discography and settings views were opened from, for `back`. */
  #history: { view: MainView; artist: ArtistRef | null; playlistId: number | null }[] = [];

  get queueInMain() {
    return this.mainView === "queue";
  }

  get nowPlayingInMain() {
    return this.mainView === "nowPlaying";
  }

  get visualizerInMain() {
    return this.mainView === "visualizer";
  }

  get artistInMain() {
    return this.mainView === "artist" && this.artist !== null;
  }

  get discographyInMain() {
    return this.mainView === "discography" && this.artist !== null;
  }

  get settingsInMain() {
    return this.mainView === "settings";
  }

  /** Whether `back` has somewhere to go. */
  get canGoBack() {
    return OPENED.includes(this.mainView);
  }

  /** Shows the library browser in the main area. */
  showLibrary() {
    this.#show("library");
  }

  /** Shows the queue in the main area. */
  showQueue() {
    this.#show("queue");
  }

  /** Shows the current track in the main area. */
  showNowPlaying() {
    this.#open("nowPlaying");
  }

  /** Shows the visualizer in the main area. */
  showVisualizer() {
    this.#open("visualizer");
  }

  /** Shows an artist's page in the main area. */
  showArtist(artist: ArtistRef) {
    if (this.mainView === "artist" && this.artist?.id === artist.id) return;
    this.#open("artist");
    this.artist = artist;
  }

  /** Shows what MusicBrainz lists for an artist that the library doesn't have. */
  showDiscography(artist: ArtistRef) {
    if (this.mainView === "discography" && this.artist?.id === artist.id) return;
    this.#open("discography");
    this.artist = artist;
  }

  /** Shows a view of its own (Home, History, Health, Favourites) in the main area. */
  showView(view: "home" | "history" | "health" | "favourites") {
    this.#open(view);
  }

  /** Shows a playlist in the main area. */
  showPlaylist(playlistId: number) {
    if (this.mainView === "playlist" && this.playlistId === playlistId) return;
    this.#open("playlist");
    this.playlistId = playlistId;
  }

  get playlistInMain() {
    return this.mainView === "playlist" && this.playlistId !== null;
  }

  /** Shows the settings in the main area, at `section` (else where they were left). */
  showSettings(section?: SettingsSection) {
    if (section) this.settingsSection = section;
    this.#open("settings");
  }

  /** Back to the view the now-playing, visualizer, artist, discography or settings view was opened from. */
  back() {
    if (!this.canGoBack) return;
    const previous = this.#history.pop();
    this.mainView = previous?.view ?? "library";
    this.artist = previous?.artist ?? null;
    this.playlistId = previous?.playlistId ?? null;
  }

  /** A view that `back` returns from. */
  #open(view: MainView) {
    if (this.mainView !== view || view === "artist" || view === "discography" || view === "playlist") {
      this.#history = [
        ...this.#history,
        { view: this.mainView, artist: this.artist, playlistId: this.playlistId },
      ].slice(-20);
    }
    this.mainView = view;
    this.sidebarOpen = false;
  }

  /** A view that starts over. */
  #show(view: MainView) {
    this.#history = [];
    this.mainView = view;
    this.sidebarOpen = false;
  }
  /** The sidebar as a drawer, on narrow windows. */
  sidebarOpen = $state(false);
  menu = $state.raw<{ x: number; y: number; items: MenuItem[] } | null>(null);
  searchInput = $state<HTMLInputElement | null>(null);

  openMenu(event: MouseEvent, items: MenuItem[]) {
    event.preventDefault();
    // Opened from the keyboard, the menu gives the focus back when it closes.
    this.menuReturn = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    this.menu = { x: event.clientX, y: event.clientY, items };
  }

  /** Where the focus was when the menu opened. */
  menuReturn: HTMLElement | null = null;

  /** The tracks selected in the list showing, for the menus' Get Info (⌘I). */
  selectedTracks: (() => MenuTrack[]) | null = null;

  /** Announced to screen readers through the page's live region (F18). */
  announcement = $state("");

  announce(text: string) {
    // The same text twice is still announced.
    this.announcement = "";
    queueMicrotask(() => (this.announcement = text));
  }
}

export const ui = new Ui();

/** Reads a per-viewer preference; storage may be unavailable. */
export function loadPreference<T>(key: string, fallback: T): T {
  try {
    const value = localStorage.getItem(`anomp.${key}`);
    return value === null ? fallback : (JSON.parse(value) as T);
  } catch {
    return fallback;
  }
}

export function savePreference(key: string, value: unknown) {
  try {
    localStorage.setItem(`anomp.${key}`, JSON.stringify(value));
  } catch {
    // Not saved: only a convenience.
  }
}
