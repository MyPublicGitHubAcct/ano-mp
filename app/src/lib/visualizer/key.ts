// Estimates the key from pitch-class energy (Krumhansl–Schmuckler): the
// major or minor key whose tonal hierarchy (Krumhansl & Kessler's probe-tone
// ratings) correlates best with it.

const MAJOR = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR = [6.33, 2.68, 3.52, 5.38, 2.6, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

export const KEY_NAMES = ["C", "D♭", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B"];

export type Key = {
  /** Pitch class of the tonic, C = 0. */
  tonic: number;
  minor: boolean;
  /** Pearson correlation with the key's profile, -1..1. */
  strength: number;
};

function correlation(a: ArrayLike<number>, b: ArrayLike<number>, rotation: number) {
  let meanA = 0;
  let meanB = 0;
  for (let i = 0; i < 12; i++) {
    meanA += a[i] / 12;
    meanB += b[i] / 12;
  }
  let product = 0;
  let squaresA = 0;
  let squaresB = 0;
  for (let i = 0; i < 12; i++) {
    const x = a[(i + rotation) % 12] - meanA;
    const y = b[i] - meanB;
    product += x * y;
    squaresA += x * x;
    squaresB += y * y;
  }
  return squaresA === 0 || squaresB === 0 ? 0 : product / Math.sqrt(squaresA * squaresB);
}

/** The best-fitting key for 12 pitch-class weights (C first), or null if they're all equal (silence). */
export function estimateKey(chroma: ArrayLike<number>): Key | null {
  let best: Key | null = null;
  for (let tonic = 0; tonic < 12; tonic++) {
    for (const minor of [false, true]) {
      const strength = correlation(chroma, minor ? MINOR : MAJOR, tonic);
      if (best === null || strength > best.strength) best = { tonic, minor, strength };
    }
  }
  return best !== null && best.strength > 0 ? best : null;
}

export function keyName(key: Key) {
  return `${KEY_NAMES[key.tonic]} ${key.minor ? "minor" : "major"}`;
}
