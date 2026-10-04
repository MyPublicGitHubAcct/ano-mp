// Resonance: cymatics with the phase portrait drawn over it, so the plate
// shows which notes ring and the attractor the timbre they ring with.

import { t } from "$lib/i18n";
import { overlay } from "../combine";
import type { Visualization } from "../types";
import { cymatics } from "./cymatics";
import { portrait } from "./portrait";

export const resonance: Visualization = {
  id: "resonance",
  get name() {
    return t("viz.resonance.name");
  },
  get description() {
    return t("viz.resonance.description");
  },
  create: overlay(cymatics, portrait),
};
