// The effects workbench's choices (PLAN.md X8), apart from the page so
// they can be tested (tests/workbench.test.mjs): what Play and Record do
// as the queue and the recording stand, and what a take records.

/** The file name extensions the open dialogs offer: what the app plays. */
export const AUDIO_EXTENSIONS = [
  "mp3",
  "flac",
  "m4a",
  "m4b",
  "aac",
  "ogg",
  "oga",
  "opus",
  "wav",
  "aif",
  "aiff",
  "aifc",
  "wma",
  "wv",
  "ape",
];

/** Whether `path` ends in one of `AUDIO_EXTENSIONS`, in any case. */
export function isAudioFile(path: string): boolean {
  const dot = path.lastIndexOf(".");
  const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return dot > slash + 1 && AUDIO_EXTENSIONS.includes(path.slice(dot + 1).toLowerCase());
}

/**
 * What Play does: play or pause the file (it is the current item), go
 * back to it (it is still queued), open it again (it left the queue), or
 * nothing (no file yet).
 */
export type PlayAction = "toggle" | "jump" | "reopen" | "none";

export function playAction(fileUid: number | null, currentUid: number | null, queued: readonly number[]): PlayAction {
  if (fileUid === null) return "none";
  if (fileUid === currentUid) return "toggle";
  return queued.includes(fileUid) ? "jump" : "reopen";
}

/** Whether "Use the Playing Track" has a track to take: one is current, and it isn't the file already. */
export function canUseCurrent(fileUid: number | null, currentUid: number | null): boolean {
  return currentUid !== null && currentUid !== fileUid;
}

/** Where the queue and the recording stand, for `recordAction`. */
export type RecordState = {
  fileUid: number | null;
  currentUid: number | null;
  /** The current item is open (not still opening). */
  loaded: boolean;
  /** The `recording` feature's switch. */
  recordingOn: boolean;
  /** A recording runs, the workbench's or another. */
  recording: boolean;
  /** The recordings' folder, once chosen. */
  folder: string | null;
};

/**
 * What Record does: stop the recording running (a take, or one started
 * elsewhere); nothing, because recording is off or the file isn't playing
 * (the page says why); ask for the recordings' folder first; or record a
 * take.
 */
export type RecordAction = "stop" | "recordingOff" | "notReady" | "chooseFolder" | "take";

export function recordAction(state: RecordState): RecordAction {
  if (state.recording) return "stop";
  if (!state.recordingOn) return "recordingOff";
  if (state.fileUid === null || state.fileUid !== state.currentUid || !state.loaded) return "notReady";
  return state.folder === null ? "chooseFolder" : "take";
}

/** What a take records: the whole file, or once round the A–B loop. */
export function takeSpan(duration: number, loop: readonly [number, number] | null) {
  return loop ? { from: loop[0], to: loop[1], loop: true } : { from: 0, to: duration, loop: false };
}

/**
 * Whose A–B loop `LoopControls` shows: the current item's, once it is
 * open. The engine clears a loop when another track loads, and until then
 * still reports the last one's, so the loop is read again for each item
 * once it is open, and none is shown while one opens.
 */
export function loopOwner(currentUid: number | null, loaded: boolean): number | null {
  return loaded ? currentUid : null;
}

/** Whether a loop read for item `askedFor` still belongs to `owner` (`loopOwner`) when it arrives. */
export function keepLoop(askedFor: number | null, owner: number | null): boolean {
  return askedFor !== null && askedFor === owner;
}
