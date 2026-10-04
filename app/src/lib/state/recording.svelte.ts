// Recording what is played (PLAN.md X6), shared by the now-playing bar and
// Settings › Recording: whether a recording runs and for how long, the
// folder recordings go in, and starting and stopping. The backend says
// when a recording starts or stops (also from the Controls menu, or on an
// error); while one runs the store asks for its length every second.

import { open } from "@tauri-apps/plugin-dialog";
import { on, onAll, recording as api, type RecordingKind, type RecordingState } from "$lib/api";
import { errorText, t } from "$lib/i18n";
import { attempt, toasts } from "./toasts.svelte";

const POLL_MS = 1000;

class RecordingStore {
  state = $state.raw<RecordingState>({
    recording: false,
    seconds: 0,
    overruns: 0,
    fileName: null,
    folder: null,
    formats: [],
  });
  /** A start or stop is on its way. */
  busy = $state(false);
  #timer: ReturnType<typeof setInterval> | null = null;

  get recording() {
    return this.state.recording;
  }

  /** Whether this build can write `kind`. */
  available(kind: RecordingKind) {
    return this.state.formats.includes(kind);
  }

  connect() {
    const stop = onAll([
      on("recording", (state) => this.#set({ ...state, folder: state.folder ?? this.state.folder })),
      on("recording-failed", (stopped) => {
        if (stopped.error) toasts.show(errorText(stopped.error));
      }),
    ]);
    void this.refresh();
    return () => {
      stop();
      this.#poll(false);
    };
  }

  async refresh() {
    const state = await api.status().catch(() => null);
    if (state) this.#set(state);
  }

  /** Asks for a folder and keeps it; resolves to whether one was picked. */
  async chooseFolder(): Promise<boolean> {
    const path = await open({ multiple: false, directory: true, title: t("recording.chooseFolder") });
    if (typeof path !== "string") return false;
    const state = await attempt(() => api.setFolder(path));
    if (state) this.#set(state);
    return state !== undefined;
  }

  /** Starts recording (asking for a folder first if there is none), or stops. */
  async toggle() {
    if (this.busy) return;
    this.busy = true;
    try {
      if (this.state.recording) {
        const stopped = await attempt(() => api.stop());
        if (stopped) {
          toasts.show(t("recording.saved", { files: stopped.files.join(", ") }), "info");
          if (stopped.overruns > 0) toasts.show(t("recording.overruns", { count: stopped.overruns }));
        }
      } else {
        if (!this.state.folder && !(await this.chooseFolder())) return;
        const state = await attempt(() => api.start());
        if (state) this.#set(state);
      }
    } finally {
      this.busy = false;
    }
  }

  #set(state: RecordingState) {
    this.state = state;
    this.#poll(state.recording);
  }

  #poll(on: boolean) {
    if (on && this.#timer === null) {
      this.#timer = setInterval(() => void this.refresh(), POLL_MS);
    } else if (!on && this.#timer !== null) {
      clearInterval(this.#timer);
      this.#timer = null;
    }
  }
}

export const recording = new RecordingStore();
