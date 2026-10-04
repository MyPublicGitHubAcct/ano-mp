// The visualizations, in the order the picker lists them: Phase 5's, then
// X3's (PLAN.md Phase 7b), its two combinations last.

import { bars } from "./renderers/bars";
import { covers } from "./renderers/covers";
import { cymatics } from "./renderers/cymatics";
import { fifths } from "./renderers/fifths";
import { harmonograph } from "./renderers/harmonograph";
import { harmony } from "./renderers/harmony";
import { kaleidoscope } from "./renderers/kaleidoscope";
import { portrait } from "./renderers/portrait";
import { recurrence } from "./renderers/recurrence";
import { resonance } from "./renderers/resonance";
import { rhythm } from "./renderers/rhythm";
import { ridges } from "./renderers/ridges";
import { scope } from "./renderers/scope";
import { spiral } from "./renderers/spiral";
import { stage } from "./renderers/stage";
import { tonnetz } from "./renderers/tonnetz";
import { vectorscope } from "./renderers/vectorscope";
import { vu } from "./renderers/vu";
import type { Visualization } from "./types";

export const VISUALIZATIONS: Visualization[] = [
  bars,
  scope,
  vu,
  covers,
  ridges,
  fifths,
  vectorscope,
  kaleidoscope,
  tonnetz,
  recurrence,
  cymatics,
  portrait,
  spiral,
  harmonograph,
  rhythm,
  stage,
  resonance,
  harmony,
];

export function visualization(id: string): Visualization {
  return VISUALIZATIONS.find((candidate) => candidate.id === id) ?? VISUALIZATIONS[0];
}
