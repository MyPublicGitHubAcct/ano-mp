// Grouping releases by their MusicBrainz release group types, as the
// artist page and the discography page list them. Sections are named in
// English here (the names are stored, e.g. which are collapsed); pages show
// them through `sectionName`.

import { t, type MessageKey } from "$lib/i18n";

export type Typed = { releaseType: string | null; secondaryTypes: string[] };

/** Sections in the order pages show them. */
export const SECTIONS = ["Albums", "EPs", "Singles", "Live albums", "Compilations", "Soundtracks", "Other releases"];

const NAMES: Record<string, MessageKey> = {
  Albums: "releases.albums",
  EPs: "releases.eps",
  Singles: "releases.singles",
  "Live albums": "releases.live",
  Compilations: "releases.compilations",
  Soundtracks: "releases.soundtracks",
  "Other releases": "releases.other",
};

/** A section's name as the UI shows it. */
export const sectionName = (section: string) => (NAMES[section] ? t(NAMES[section]) : section);

const SPOKEN = ["Interview", "Spokenword", "Audiobook", "Audio drama"];

/** The section for a release; one without a type goes in `untyped`. */
export function sectionOf(release: Typed, untyped = "Albums") {
  const secondary = release.secondaryTypes;
  if (secondary.includes("Live")) return "Live albums";
  if (secondary.includes("Compilation")) return "Compilations";
  if (secondary.includes("Soundtrack")) return "Soundtracks";
  if (secondary.some((type) => SPOKEN.includes(type))) return "Other releases";
  switch (release.releaseType) {
    case null:
      return untyped;
    case "Album":
      return "Albums";
    case "EP":
      return "EPs";
    case "Single":
      return "Singles";
    default:
      return "Other releases";
  }
}

/** `releases` grouped into the sections that have any, in order. */
export function bySection<T extends Typed>(releases: T[], untyped = "Albums") {
  const grouped = new Map<string, T[]>();
  for (const release of releases) {
    const section = sectionOf(release, untyped);
    grouped.set(section, [...(grouped.get(section) ?? []), release]);
  }
  return SECTIONS.filter((name) => grouped.has(name)).map((name) => ({ name, releases: grouped.get(name)! }));
}
