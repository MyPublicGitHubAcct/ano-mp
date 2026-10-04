// One analysis of what the player plays, decoded from the binary form that
// `visualizer::encode` (app/src-tauri/src/visualizer.rs) documents.

export type Frame = {
  /** No audio is playing; everything is zero. */
  silent: boolean;
  /** 0..1, log-spaced from `lowestHz` to `highestHz`, low first. */
  bands: Float32Array;
  lowestHz: number;
  highestHz: number;
  /** 0..1 per pitch class, C first, the strongest 1. */
  chroma: Float32Array;
  /** Linear 0..1, left and right, over about 40 ms. */
  peak: [number, number];
  rms: [number, number];
  /** -1..1, from a rising zero crossing. */
  left: Float32Array;
  right: Float32Array;
  /** How much louder the spectrum got since the previous frame, 0..1. */
  onset: number;
  beat: boolean;
  /** 0..1 a semitone apart, from MIDI note `lowestNote` (C2) up, on the bands' scale. */
  notes: Float32Array;
  lowestNote: number;
  /** Per band, where it sits between left (-1) and right (+1); 0 for a band too quiet to place. */
  balance: Float32Array;
};

const VERSION = 2;
const HEADER = 48;

export const PITCH_NAMES = ["C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B"];

export function silentFrame(bands = 64, samples = 512, notes = 84): Frame {
  return {
    silent: true,
    bands: new Float32Array(bands),
    lowestHz: 30,
    highestHz: 16000,
    chroma: new Float32Array(12),
    peak: [0, 0],
    rms: [0, 0],
    left: new Float32Array(samples),
    right: new Float32Array(samples),
    onset: 0,
    beat: false,
    notes: new Float32Array(notes),
    lowestNote: 36,
    balance: new Float32Array(bands),
  };
}

/** Decodes `buffer` into `frame`, reusing its arrays when the sizes match. Returns false (leaving `frame`
    as it was) for a buffer it can't read. */
export function decodeFrame(buffer: ArrayBuffer, frame: Frame): boolean {
  if (buffer.byteLength < HEADER) return false;
  const view = new DataView(buffer);
  if (view.getUint8(0) !== VERSION) return false;
  const flags = view.getUint8(1);
  const bandCount = view.getUint16(2, true);
  const samples = view.getUint16(4, true);
  const noteCount = view.getUint8(6);
  const even = (length: number) => length + (length % 2);
  const notesAt = HEADER + even(bandCount);
  const balanceAt = notesAt + even(noteCount);
  const samplesAt = balanceAt + even(bandCount);
  if (buffer.byteLength < samplesAt + 4 * samples) return false;

  frame.silent = (flags & 1) !== 0;
  frame.beat = (flags & 2) !== 0;
  frame.lowestHz = view.getFloat32(8, true);
  frame.highestHz = view.getFloat32(12, true);
  frame.peak[0] = view.getFloat32(16, true);
  frame.peak[1] = view.getFloat32(20, true);
  frame.rms[0] = view.getFloat32(24, true);
  frame.rms[1] = view.getFloat32(28, true);
  frame.onset = view.getFloat32(32, true);
  frame.lowestNote = view.getUint8(7);

  const bytes = new Uint8Array(buffer);
  for (let i = 0; i < 12; i++) frame.chroma[i] = bytes[36 + i] / 255;

  if (frame.bands.length !== bandCount) frame.bands = new Float32Array(bandCount);
  for (let i = 0; i < bandCount; i++) frame.bands[i] = bytes[HEADER + i] / 255;

  if (frame.notes.length !== noteCount) frame.notes = new Float32Array(noteCount);
  for (let i = 0; i < noteCount; i++) frame.notes[i] = bytes[notesAt + i] / 255;

  if (frame.balance.length !== bandCount) frame.balance = new Float32Array(bandCount);
  for (let i = 0; i < bandCount; i++) frame.balance[i] = Math.max(-1, view.getInt8(balanceAt + i) / 127);

  if (frame.left.length !== samples) {
    frame.left = new Float32Array(samples);
    frame.right = new Float32Array(samples);
  }
  const start = samplesAt;
  for (let i = 0; i < samples; i++) {
    frame.left[i] = view.getInt16(start + 2 * i, true) / 32767;
    frame.right[i] = view.getInt16(start + 2 * (samples + i), true) / 32767;
  }
  return true;
}
