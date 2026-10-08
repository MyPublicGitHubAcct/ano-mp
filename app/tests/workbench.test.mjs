// The effects workbench's choices (src/lib/workbench.ts, PLAN.md X8).
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  AUDIO_EXTENSIONS,
  canUseCurrent,
  isAudioFile,
  keepLoop,
  loopOwner,
  playAction,
  recordAction,
  takeSpan,
} from "../src/lib/workbench.ts";

test("only audio files are taken, whatever their case", () => {
  assert.ok(isAudioFile("/Music/a.flac"));
  assert.ok(isAudioFile("/Music/B.MP3"));
  assert.ok(isAudioFile("C:\\Music\\c.Opus"));
  assert.ok(!isAudioFile("/Music/cover.jpg"));
  assert.ok(!isAudioFile("/Music/flac"), "no extension");
  assert.ok(!isAudioFile("/Music/.flac"), "a hidden file named only its extension");
  assert.ok(!isAudioFile("/Music.flac/notes"), "the dot is in a folder's name");
  assert.ok(AUDIO_EXTENSIONS.every((extension) => extension === extension.toLowerCase()));
});

test("Play plays the file, goes back to it, or opens it again", () => {
  assert.equal(playAction(null, 3, [3]), "none");
  assert.equal(playAction(5, 5, [3, 5]), "toggle");
  assert.equal(playAction(5, 3, [3, 5]), "jump", "something else plays; the file is still queued");
  assert.equal(playAction(5, 3, [3]), "reopen", "the file left the queue");
  assert.equal(playAction(5, null, []), "reopen");
});

const ready = {
  fileUid: 5,
  currentUid: 5,
  loaded: true,
  recordingOn: true,
  recording: false,
  folder: "/Recordings",
};

test("Record takes the file once it plays, asking for a folder first", () => {
  assert.equal(recordAction(ready), "take");
  assert.equal(recordAction({ ...ready, folder: null }), "chooseFolder");
});

test("Record explains, rather than acting, while it can't record", () => {
  assert.equal(recordAction({ ...ready, recordingOn: false }), "recordingOff", "never turns recording on");
  assert.equal(recordAction({ ...ready, fileUid: null }), "notReady");
  assert.equal(recordAction({ ...ready, currentUid: 3 }), "notReady", "another item plays");
  assert.equal(recordAction({ ...ready, loaded: false }), "notReady", "still opening");
});

test("Record stops any recording running", () => {
  assert.equal(recordAction({ ...ready, recording: true }), "stop");
  assert.equal(recordAction({ ...ready, recording: true, currentUid: 3 }), "stop", "one started elsewhere");
});

test("a take is the whole file, or once round the loop", () => {
  assert.deepEqual(takeSpan(200, null), { from: 0, to: 200, loop: false });
  assert.deepEqual(takeSpan(200, [10, 20.5]), { from: 10, to: 20.5, loop: true });
});

test("the loop shown is the current item's, read once it is open", () => {
  assert.equal(loopOwner(5, true), 5);
  assert.equal(loopOwner(6, false), null, "a file still opening has no loop yet");
  assert.equal(loopOwner(null, true), null);
  // A file dropped while the last one looped: what the engine said of the
  // last one, arriving after the new one is current, is dropped.
  assert.ok(!keepLoop(5, loopOwner(6, false)));
  assert.ok(!keepLoop(5, loopOwner(6, true)));
  assert.ok(keepLoop(6, loopOwner(6, true)));
  assert.ok(!keepLoop(null, null));
});

test("the playing track can be taken unless nothing plays or it is the file already", () => {
  assert.ok(canUseCurrent(null, 3), "no file yet");
  assert.ok(canUseCurrent(5, 3), "another file");
  assert.ok(!canUseCurrent(3, 3), "the file is playing");
  assert.ok(!canUseCurrent(null, null), "nothing plays");
  assert.ok(!canUseCurrent(5, null));
});
