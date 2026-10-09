// Finding the app's features from the search box (PLAN.md X9): an index of
// the views, the Settings sections and the settings in them, the menu
// bar's page items and the optional features' switches, each by the label
// `en.json` gives it, a few synonyms (`find.synonyms.<id>`) and where it
// lives. Pure, so `npm test` can check the matching, the ranking, where a
// result opens and that nothing is missing (tests/find.test.mjs); the
// search results show it (SearchResults.svelte) and `lib/pageActions.ts`
// opens a result.

import type { FeatureSettings } from "./api";
import type { MessageKey } from "./i18n";
import type { SettingsSection } from "./state/ui.svelte";

/** A field of `FeatureSettings`: an optional feature's switch or option. */
export type FeatureKey = keyof FeatureSettings;

/** The views a result opens. */
export type FindView =
  | "home"
  | "favourites"
  | "artists"
  | "history"
  | "health"
  | "workbench"
  | "nowPlaying"
  | "visualizer"
  | "queue"
  | "library"
  | "settings";

/** The menu bar's items that are the page's to do (`shell/menu.rs`'s `PAGE_ITEMS`), by id. */
export type PageAction =
  | "settings"
  | "add-folder"
  | "open-files"
  | "new-playlist"
  | "new-smart-playlist"
  | "import-playlist"
  | "export-playlist"
  | "export-data"
  | "import-data"
  | "find"
  | "get-info"
  | "go-to-current"
  | "show-home"
  | "show-library"
  | "show-favourites"
  | "show-now-playing"
  | "show-queue"
  | "show-visualizer"
  | "toggle-queue"
  | "mini-player"
  | "shortcuts";

/** Where a result opens: a view, a control in Settings (by its element id), a menu item's action, the sidebar's playlists, or the player bar's sleep timer. */
export type Place =
  | { kind: "view"; view: FindView }
  | { kind: "settings"; section: SettingsSection; control: string }
  | { kind: "action"; action: PageAction }
  | { kind: "playlists" }
  | { kind: "sleepTimer" };

export type FindEntry = {
  /** Unique; its synonyms are `find.synonyms.<id>`. */
  id: string;
  label: MessageKey;
  /** Where it lives, outermost first ("Settings", "Playback"). */
  where: MessageKey[];
  place: Place;
  /** Shown only while one of these features is on; else it opens at the first one's switch. */
  needs?: FeatureKey[];
  /** The feature switch or option it is. */
  feature?: FeatureKey;
  /** The Settings section it is. */
  section?: SettingsSection;
  /** The menu bar item it also is. */
  menu?: PageAction;
};

const SETTINGS: MessageKey = "settings.title";
const SIDEBAR: MessageKey = "find.where.sidebar";

/** The Settings sections' names, as `SettingsPage.svelte`'s `ALL_SECTIONS` gives them. */
export const SECTION_NAMES: Record<SettingsSection, MessageKey> = {
  general: "settings.general",
  library: "settings.library",
  sorting: "settings.sorting",
  display: "settings.display",
  appearance: "settings.appearance",
  playback: "settings.playback",
  equaliser: "settings.equaliser",
  effects: "settings.effects",
  recording: "settings.recording",
  visualizer: "settings.visualizer",
  sources: "settings.sources",
  features: "settings.features",
  about: "settings.about",
};

/** Sections Settings shows only while a feature is on (`SettingsPage.svelte`'s `SECTIONS`). */
const SECTION_NEEDS: Partial<Record<SettingsSection, FeatureKey[]>> = {
  appearance: ["themes"],
  recording: ["recording"],
};

/** A Settings section's tab: where a section opens. */
export const sectionTab = (section: SettingsSection) => `settings-tab-${section}`;

/** The element id of a feature's switch or option (`FeaturesOptions.svelte`, Updates' in `AboutOptions.svelte`). */
export const switchControl = (feature: FeatureKey) => `feature-${feature}`;

const view = (
  id: string,
  label: MessageKey,
  place: Place,
  extra: Pick<FindEntry, "needs" | "menu"> = {},
): FindEntry => ({ id: `view.${id}`, label, where: [SIDEBAR], place, ...extra });

const section = (id: SettingsSection): FindEntry => ({
  id: `section.${id}`,
  label: SECTION_NAMES[id],
  where: [SETTINGS],
  place: { kind: "settings", section: id, control: sectionTab(id) },
  needs: SECTION_NEEDS[id],
  section: id,
});

/** A setting in a section, at the element `control` (`setting-<name>` unless the component already had an id). */
const setting = (
  inSection: SettingsSection,
  name: string,
  label: MessageKey,
  control = `setting-${name}`,
  needs = SECTION_NEEDS[inSection],
): FindEntry => ({
  id: `setting.${name}`,
  label,
  where: [SETTINGS, SECTION_NAMES[inSection]],
  place: { kind: "settings", section: inSection, control },
  needs,
});

const action = (id: PageAction, label: MessageKey, menu: MessageKey): FindEntry => ({
  id: `action.${id}`,
  label,
  where: [menu],
  place: { kind: "action", action: id },
  menu: id,
});

/** A feature's switch or option, in Settings › Features unless it says otherwise. */
const toggle = (
  feature: FeatureKey,
  label: MessageKey = `feature.${feature}` as MessageKey,
  extra: { needs?: FeatureKey[]; in?: SettingsSection } = {},
): FindEntry => {
  const inSection = extra.in ?? "features";
  return {
    id: `switch.${feature}`,
    label,
    where: [SETTINGS, SECTION_NAMES[inSection]],
    place: { kind: "settings", section: inSection, control: switchControl(feature) },
    needs: extra.needs,
    feature,
  };
};

/** The effects, by their `EffectType` ids (`effects.<id>` in en.json). */
const EFFECTS = ["reverb", "echo", "chorus", "flanger", "phaser", "tremolo", "lofi", "freeze"] as const;

/** Every entry, in the order results tie in: the sidebar, then each Settings section with its settings, the menu bar, and the switches. The /dev page (H2) has none. */
export const INDEX: FindEntry[] = [
  view(
    "home",
    "sidebar.home",
    { kind: "view", view: "home" },
    {
      needs: ["recentlyAdded", "onThisDay", "listeningHistory"],
      menu: "show-home",
    },
  ),
  view("favourites", "sidebar.favourites", { kind: "view", view: "favourites" }, { menu: "show-favourites" }),
  view("artists", "sidebar.artists", { kind: "view", view: "artists" }),
  view("history", "sidebar.history", { kind: "view", view: "history" }, { needs: ["listeningHistory"] }),
  view("health", "sidebar.health", { kind: "view", view: "health" }, { needs: ["healthReport"] }),
  view("workbench", "sidebar.workbench", { kind: "view", view: "workbench" }, { needs: ["effectsWorkbench"] }),
  view("nowPlaying", "sidebar.nowPlaying", { kind: "view", view: "nowPlaying" }, { menu: "show-now-playing" }),
  view("visualizer", "sidebar.visualizer", { kind: "view", view: "visualizer" }, { menu: "show-visualizer" }),
  view("queue", "queue.title", { kind: "view", view: "queue" }, { menu: "show-queue" }),
  view("library", "library.title", { kind: "view", view: "library" }, { menu: "show-library" }),
  view("playlists", "sidebar.playlists", { kind: "playlists" }),
  view("folders", "sidebar.folders", { kind: "settings", section: "library", control: sectionTab("library") }),
  view("sources", "sidebar.sources", { kind: "settings", section: "sources", control: sectionTab("sources") }),
  view("settings", "sidebar.settings", { kind: "view", view: "settings" }, { menu: "settings" }),
  // Also in the Controls menu, whose items Rust runs (not PAGE_ITEMS).
  { id: "bar.sleepTimer", label: "bar.sleep", where: ["find.where.bar"], place: { kind: "sleepTimer" } },

  section("general"),
  setting("general", "menuBarControls", "general.menuBarControls"),
  setting("general", "miniPlayerOnTop", "general.onTop"),
  setting("general", "trackNotifications", "general.trackNotifications"),

  section("library"),
  setting("library", "rescanAtLaunch", "folders.rescanAtLaunch"),
  setting("library", "watchFolders", "folders.watch"),

  section("sorting"),
  setting("sorting", "libraryViews", "sort.views"),
  setting("sorting", "skippedWords", "sort.skipped"),

  section("display"),
  setting("display", "trackColumns", "display.columns"),
  setting("display", "albumFacts", "display.releaseFacts"),
  setting("display", "descriptions", "display.descriptions"),
  setting("display", "sidebarNowPlaying", "display.sidebarNowPlaying"),

  section("appearance"),
  setting("appearance", "theme", "appearance.themes"),
  setting("appearance", "colourScheme", "appearance.scheme", "theme-scheme"),
  setting("appearance", "accentFromCover", "appearance.accentFromCover"),
  setting("appearance", "systemContrast", "appearance.systemContrast"),
  setting("appearance", "font", "appearance.font", "theme-font"),
  setting("appearance", "textSize", "appearance.textSize", "theme-size"),
  setting("appearance", "density", "appearance.density", "theme-density"),
  setting("appearance", "corners", "appearance.radius", "theme-radius"),

  section("playback"),
  setting("playback", "outputDevice", "playback.device"),
  setting("playback", "bufferSize", "playback.bufferSize"),
  setting("playback", "replayGain", "playback.replayGain"),
  setting("playback", "replayGainPreamp", "eq.preamp"),
  setting("playback", "untagged", "playback.untagged"),
  setting("playback", "preventClipping", "playback.preventClipping"),
  setting("playback", "crossfade", "playback.crossfadeTitle"),

  section("equaliser"),
  setting("equaliser", "equaliserOn", "eq.enabled"),
  setting("equaliser", "equaliserFollow", "eq.follow"),
  setting("equaliser", "equaliserPreset", "eq.presetLabel"),
  setting("equaliser", "equaliserPreamp", "eq.preamp"),
  setting("equaliser", "equaliserBands", "eq.bands"),

  section("effects"),
  // Out of reach (inert) while the effects are off.
  setting("effects", "effectsPreset", "effects.preset", "setting-effectsPreset", ["effects"]),
  ...EFFECTS.map((id) => setting("effects", id, `effects.${id}`, `effect-${id}`, ["effects"])),

  section("recording"),
  setting("recording", "recordingFolder", "recording.folder"),
  setting("recording", "recordingFormat", "recording.format"),
  // One shows at a time, as the format is lossy or not, so they share an id.
  setting("recording", "recordingBitrate", "recording.bitrate", "setting-recordingQuality"),
  setting("recording", "recordingBits", "recording.bits", "setting-recordingQuality"),
  setting("recording", "recordingCueSheet", "recording.cueSheet"),

  section("visualizer"),
  setting("visualizer", "visualization", "vizOptions.visualization"),
  setting("visualizer", "visualizerCycle", "vizOptions.cycle"),
  setting("visualizer", "coverWall", "vizOptions.coverWall"),
  setting("visualizer", "frameRate", "vizOptions.frameRate"),
  setting("visualizer", "sensitivity", "vizOptions.sensitivity"),
  setting("visualizer", "coverColours", "vizOptions.coverColours"),
  setting("visualizer", "calm", "vizOptions.calm"),

  section("sources"),
  setting("sources", "onlineServices", "services.online"),
  setting("sources", "autoMatch", "services.autoMatch"),
  setting("sources", "sourceList", "services.sources"),
  setting("sources", "sourceOrder", "services.order"),

  section("features"),

  section("about"),
  setting("about", "notices", "about.notices"),
  setting("about", "checkUpdates", "updates.check"),
  setting("about", "showLogs", "about.showLogs"),
  setting("about", "copyDiagnostics", "about.copyDiagnostics"),
  setting("about", "detailedLogging", "about.detailedLogging"),

  action("add-folder", "find.menu.addFolder", "find.where.file"),
  action("open-files", "find.menu.openFiles", "find.where.file"),
  action("new-playlist", "sidebar.newPlaylist", "find.where.file"),
  action("new-smart-playlist", "sidebar.newSmartPlaylist", "find.where.file"),
  action("import-playlist", "sidebar.importPlaylist", "find.where.file"),
  action("export-playlist", "find.menu.exportPlaylist", "find.where.file"),
  action("import-data", "find.menu.importData", "find.where.file"),
  action("export-data", "find.menu.exportData", "find.where.file"),
  action("get-info", "menu.getInfo", "find.where.file"),
  action("find", "find.menu.find", "find.where.edit"),
  action("go-to-current", "find.menu.goToCurrent", "find.where.controls"),
  action("toggle-queue", "find.menu.toggleQueue", "find.where.view"),
  action("mini-player", "general.miniPlayer", "find.where.view"),
  action("shortcuts", "general.showShortcuts", "find.where.help"),

  toggle("loudnessAnalysis"),
  toggle("waveformSeekBar"),
  toggle("segueShuffle"),
  toggle("skipSilence", "features.skipSilence"),
  toggle("skipSilenceAfter", "features.skipAfter"),
  toggle("signalPath"),
  toggle("matchSampleRate"),
  toggle("practiceMode"),
  toggle("effects"),
  toggle("recording"),
  toggle("effectsWorkbench"),
  toggle("crossfeed", "features.crossfeed"),
  toggle("crossfeedHeadphonesOnly", "features.headphonesOnly"),
  toggle("healthReport"),
  toggle("cueSheets"),
  toggle("classical"),
  toggle("playbackPreferences"),
  toggle("lyrics"),
  toggle("listeningHistory"),
  toggle("listenbrainz", "features.listenBrainz"),
  toggle("recentlyPlayed"),
  toggle("topPlayed"),
  toggle("libraryRadio"),
  toggle("radioAfterQueue"),
  toggle("recentlyAdded"),
  toggle("onThisDay"),
  toggle("moreInGenre"),
  toggle("recommendations"),
  toggle("outsideRecommendations"),
  toggle("similarArtists"),
  toggle("themes"),
  toggle("remoteControl", "features.remoteSwitch"),
  toggle("remotePort", "features.port", { needs: ["remoteControl"] }),
  toggle("updateCheck", "updates.automatic", { in: "about" }),
];

/** `text` without case or accents: "Égaliseur" → "egaliseur". */
export function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
}

/** The words of `text`, folded. */
export function words(text: string): string[] {
  return fold(text)
    .split(/[^\p{L}\p{N}]+/u)
    .filter((word) => word !== "");
}

/** A query's words, or null while it has fewer than two letters. */
export function queryWords(query: string): string[] | null {
  const found = words(query);
  return found.join("").length >= 2 ? found : null;
}

/**
 * How `query` matches `text`: 0 when its first word starts `text`, 1 when
 * every word starts some word of `text` but not like that, null when one
 * doesn't.
 */
export function matchIn(query: string[], text: string): 0 | 1 | null {
  const target = words(text);
  if (!query.every((word) => target.some((candidate) => candidate.startsWith(word)))) return null;
  return target[0]?.startsWith(query[0]) ? 0 : 1;
}

/** Whether an entry's feature is on: true, false, or null when it isn't a feature's on-off switch. */
function switchState(entry: FindEntry, features: FeatureSettings): boolean | null {
  if (!entry.feature) return null;
  const value = features[entry.feature];
  return typeof value === "boolean" ? value : null;
}

/** Whether an entry is hidden now: every feature it needs is off. */
export function isOff(entry: FindEntry, features: FeatureSettings): boolean {
  return entry.needs !== undefined && !entry.needs.some((key) => Boolean(features[key]));
}

/** Where opening an entry goes: its place, or, while it is hidden, the switch of the feature it needs. Never turns anything on (PLAN.md §4 #6). */
export function target(entry: FindEntry, features: FeatureSettings, index: FindEntry[] = INDEX): Place {
  const feature = entry.needs?.[0];
  if (feature === undefined || !isOff(entry, features)) return entry.place;
  return index.find((candidate) => candidate.id === `switch.${feature}`)?.place ?? entry.place;
}

/** One result: the entry, where it opens, and whether it is on or off (null: neither applies). */
export type Found = { entry: FindEntry; target: Place; state: "on" | "off" | null };

/** Text for a message key, or null when the catalogue has none (an entry without synonyms). */
export type Lookup = (key: string) => string | null;

const placeKey = (place: Place) => JSON.stringify(place);

/** An entry's synonyms, from `find.synonyms.<id>` (comma-separated). */
export function synonyms(entry: FindEntry, text: Lookup): string[] {
  const list = text(`find.synonyms.${entry.id}`);
  return list === null
    ? []
    : list
        .split(",")
        .map((synonym) => synonym.trim())
        .filter(Boolean);
}

/**
 * The entries `query` finds, best first: a match in the label before one
 * in a synonym, and one in a synonym before one only in where it lives
 * (with the label: "playback crossfade"), each from its first word before
 * later ones, then in the index's order. A result that opens where an
 * earlier one does is left out. Empty below two letters.
 */
export function find(query: string, text: Lookup, features: FeatureSettings, index: FindEntry[] = INDEX): Found[] {
  const wanted = queryWords(query);
  if (wanted === null) return [];
  const ranked: { found: Found; rank: number; order: number }[] = [];
  index.forEach((entry, order) => {
    const label = text(entry.label) ?? entry.label;
    const place = [...entry.where.map((key) => text(key) ?? key), label].join(" ");
    const best = (texts: string[]) =>
      texts.reduce<number | null>((rank, candidate) => {
        const match = matchIn(wanted, candidate);
        return match === null ? rank : Math.min(rank ?? match, match);
      }, null);
    const tiers = [best([label]), best(synonyms(entry, text)), best([place])];
    const tier = tiers.findIndex((match) => match !== null);
    const match = tiers[tier];
    if (match === undefined || match === null) return;
    const on = switchState(entry, features);
    const state = isOff(entry, features) ? "off" : on === null ? null : on ? "on" : "off";
    ranked.push({
      found: { entry, target: target(entry, features, index), state },
      rank: tier * 2 + match,
      order,
    });
  });
  ranked.sort((a, b) => a.rank - b.rank || a.order - b.order);
  const seen = new Set<string>();
  return ranked
    .map(({ found }) => found)
    .filter((found) => {
      const key = placeKey(found.target);
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
}
