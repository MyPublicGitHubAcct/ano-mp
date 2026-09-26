// Layout and transient UI state.

export type MenuItem = { label: string; action: () => unknown; disabled?: boolean };

export type MainView = "library" | "queue" | "nowPlaying" | "artist";
export type ArtistRef = { id: number; name: string };

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
  /** The views the now-playing and artist views were opened from, for `back`. */
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

  /** Back to the view the now-playing or artist view was opened from. */
  back() {
    if (this.mainView !== "nowPlaying" && this.mainView !== "artist") return;
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
