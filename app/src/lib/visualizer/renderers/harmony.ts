// Harmony: the pitch spiral beside the Tonnetz, the notes sounding and the
// chords they make.

import { t } from "$lib/i18n";
import { sideBySide } from "../combine";
import type { Visualization } from "../types";
import { spiral } from "./spiral";
import { tonnetz } from "./tonnetz";

export const harmony: Visualization = {
  id: "harmony",
  get name() {
    return t("viz.harmony.name");
  },
  get description() {
    return t("viz.harmony.description");
  },
  create: sideBySide(spiral, tonnetz),
};
