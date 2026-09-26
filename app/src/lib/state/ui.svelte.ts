// Layout and transient UI state.

export type MenuItem = { label: string; action: () => unknown; disabled?: boolean };

class Ui {
  /** What the main area shows when not searching: the library browser, the
      queue, or the current track with its cover. */
  mainView = $state<"library" | "queue" | "nowPlaying">("library");
  /** The queue panel beside the main area (a column when wide, an overlay
      when narrow); hidden while the main area shows the queue or the
      current track. */
  queueOpen = $state(true);
  /** Where leaving the now-playing view goes back to. */
  #beforeNowPlaying: "library" | "queue" = "library";

  get queueInMain() {
    return this.mainView === "queue";
  }

  get nowPlayingInMain() {
    return this.mainView === "nowPlaying";
  }

  /** Shows the queue in the main area. */
  showQueue() {
    this.mainView = "queue";
    this.sidebarOpen = false;
  }

  /** Shows the current track in the main area. */
  showNowPlaying() {
    if (this.mainView !== "nowPlaying") this.#beforeNowPlaying = this.mainView;
    this.mainView = "nowPlaying";
    this.sidebarOpen = false;
  }

  /** Back to the view the now-playing view was opened from. */
  leaveNowPlaying() {
    if (this.mainView === "nowPlaying") this.mainView = this.#beforeNowPlaying;
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
