// The visualizations, in the order the picker lists them.

import { bars } from "./renderers/bars";
import { covers } from "./renderers/covers";
import { fifths } from "./renderers/fifths";
import { kaleidoscope } from "./renderers/kaleidoscope";
import { ridges } from "./renderers/ridges";
import { scope } from "./renderers/scope";
import { vectorscope } from "./renderers/vectorscope";
import { vu } from "./renderers/vu";
import type { Visualization } from "./types";

export const VISUALIZATIONS: Visualization[] = [bars, scope, vu, covers, ridges, fifths, vectorscope, kaleidoscope];

export function visualization(id: string): Visualization {
  return VISUALIZATIONS.find((candidate) => candidate.id === id) ?? VISUALIZATIONS[0];
}
