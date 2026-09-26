// Layout and transient UI state.

import type { ArtistInfo } from "$lib/api";

export type MenuItem = { label: string; action: () => unknown; disabled?: boolean };

export type MainView = "library" | "queue" | "nowPlaying" | "artist" | "services";
export type ArtistRef = { id: number; name: string };
export type AlbumRef = { id: number; title: string };

/** The modal dialog showing, if any. */
export type Dialog =
  | { kind: "findDetails"; album: AlbumRef }
  | { kind: "chooseCover"; album: AlbumRef }
  | { kind: "findArtist"; artist: ArtistRef; info: ArtistInfo };

/** Views that `back` returns from. */
const OPENED: MainView[] = ["nowPlaying", "artist", "services"];

class Ui {
  /** What the main area shows when not searching: the library browser, the
      queue, the current track with its cover, or an artist's page. */
  mainView = $state<MainView>("library");
  /** The artist the artist view shows. */
  artist = $state.raw<ArtistRef | null>(null);
  /** The queue panel beside the main area (a column when wide, an overlay
      when narrow); hidden while the main area shows the queue or the
      current track. */
  queueOpen = $state(true);
  dialog = $state.raw<Dialog | null>(null);
  /** The views the now-playing, artist and services views were opened from, for `back`. */
  #history: { view: MainView; artist: ArtistRef | null }[] = [];

  get queueInMain() {
    return this.mainView === "queue";
  }

  get nowPlayingInMain() {
    return this.mainView === "nowPlaying";
  }

  get artistInMain() {
    return this.mainView === "artist" && this.artist !== null;
  }

  get servicesInMain() {
    return this.mainView === "services";
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

  /** Shows an artist's page in the main area. */
  showArtist(artist: ArtistRef) {
    if (this.mainView === "artist" && this.artist?.id === artist.id) return;
    this.#open("artist");
    this.artist = artist;
  }

  /** Shows the online sources' settings and status in the main area. */
  showServices() {
    this.#open("services");
  }

  /** Back to the view the now-playing, artist or services view was opened from. */
  back() {
    if (!this.canGoBack) return;
    const previous = this.#history.pop();
    this.mainView = previous?.view ?? "library";
    this.artist = previous?.artist ?? null;
  }

  /** A view that `back` returns from. */
  #open(view: MainView) {
    if (this.mainView !== view || view === "artist") {
      this.#history = [...this.#history, { view: this.mainView, artist: this.artist }].slice(-20);
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
    this.menu = { x: event.clientX, y: event.clientY, items };
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
