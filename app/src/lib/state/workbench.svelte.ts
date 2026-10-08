// The effects workbench (PLAN.md X8): the file chosen, and Play and Record
// for it. The file plays through the queue (after the current item, as a
// file opened from the Finder); a take records it once with X6's recorder.
// The file is chosen, dropped, or the track playing taken as it is. Kept
// while the page is closed, so going back finds the file and a take
// still running.

import { open } from "@tauri-apps/plugin-dialog";
import { queue, workbench as api, type WorkbenchFile } from "$lib/api";
import { t } from "$lib/i18n";
import { AUDIO_EXTENSIONS, canUseCurrent, playAction, recordAction } from "$lib/workbench";
import { player } from "./player.svelte";
import { recording } from "./recording.svelte";
import { appSettings } from "./settings.svelte";
import { attempt, toasts } from "./toasts.svelte";

class WorkbenchStore {
  file = $state.raw<WorkbenchFile | null>(null);
  /** A file is being opened. */
  opening = $state(false);
  /** A take started here is being recorded. */
  taking = $state(false);
  /** The file the last take was saved as. */
  saved = $state<string | null>(null);

  /** The file is the queue's current item. */
  get current() {
    return this.file !== null && player.currentItem?.uid === this.file.uid;
  }

  get playAction() {
    return playAction(
      this.file?.uid ?? null,
      player.currentItem?.uid ?? null,
      player.items.map((item) => item.uid),
    );
  }

  get recordAction() {
    return recordAction({
      fileUid: this.file?.uid ?? null,
      currentUid: player.currentItem?.uid ?? null,
      loaded: player.loaded,
      recordingOn: appSettings.current.features.recording,
      recording: recording.recording,
      folder: recording.state.folder,
    });
  }

  /** Asks for a file with the open dialog, and plays it. */
  async choose() {
    const path = await open({
      multiple: false,
      title: t("workbench.choose"),
      filters: [{ name: t("drop.audioFiles"), extensions: AUDIO_EXTENSIONS }],
    });
    if (typeof path === "string") await this.open(path);
  }

  /** Plays the file at `path` (chosen or dropped). */
  open(path: string) {
    return this.#take(() => api.open(path));
  }

  /** Whether "Use the Playing Track" has a track to take. */
  get canUseCurrent() {
    return canUseCurrent(this.file?.uid ?? null, player.currentItem?.uid ?? null);
  }

  /** Takes the track playing, as it plays, as the file. */
  useCurrent() {
    return this.#take(() => api.current());
  }

  /** Makes the file `open` resolves to the workbench's, once at a time. */
  async #take(open: () => Promise<WorkbenchFile>) {
    if (this.opening) return;
    this.opening = true;
    try {
      const file = await attempt(open);
      if (file) {
        this.file = file;
        this.saved = null;
      }
    } finally {
      this.opening = false;
    }
  }

  async play() {
    const file = this.file;
    switch (this.playAction) {
      case "toggle":
        return player.toggle();
      case "jump":
        if (file) await attempt(() => queue.jump(file.uid));
        return;
      case "reopen":
        if (file) await this.#take(() => api.reopen(file.trackId));
        return;
    }
  }

  async record() {
    const action = this.recordAction;
    if (action === "stop") {
      // Stopped by hand, the recording's own toast says where it went.
      const take = this.taking;
      this.taking = false;
      await recording.toggle();
      if (take && !recording.recording) this.saved = recording.state.fileName;
      return;
    }
    if (action === "chooseFolder" && !(await recording.chooseFolder())) return;
    if (action !== "take" && action !== "chooseFolder") return;
    const file = this.file;
    if (!file) return;
    this.taking = true;
    this.saved = null;
    const state = await attempt(() => api.take(file.uid));
    if (!state) this.taking = false;
  }

  /** Whether a recording ran when `recordingChanged` last heard. */
  #wasRecording = false;

  /** The recording started or stopped: a take that stopped is saved. */
  recordingChanged(recordingNow: boolean, fileName: string | null) {
    const stopped = this.#wasRecording && !recordingNow;
    this.#wasRecording = recordingNow;
    // A take just asked for hasn't started yet: only a stop ends it.
    if (!stopped || !this.taking) return;
    this.taking = false;
    this.saved = fileName;
    if (fileName) toasts.show(t("recording.saved", { files: fileName }), "info");
  }
}

export const workbench = new WorkbenchStore();
