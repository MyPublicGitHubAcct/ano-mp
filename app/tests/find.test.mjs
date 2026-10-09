// Finding a feature (src/lib/find.ts, PLAN.md X9): the matching, the
// ranking, where a result opens, and that the index misses nothing the
// user guide's check lists (scripts/check-user-guide.py's sources: the
// sidebar, the Settings sections, FeatureSettings and the menu's
// PAGE_ITEMS), with every control it points at in its section.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { find, fold, INDEX, matchIn, queryWords, SECTION_NAMES, target } from "../src/lib/find.ts";

const root = new URL("..", import.meta.url).pathname;
const read = (path) => readFileSync(join(root, path), "utf8");
const en = JSON.parse(read("src/lib/i18n/en.json"));
const text = (key) => {
  const message = en[key];
  return typeof message === "string" ? message : (message?.other ?? null);
};

/** FeatureSettings' fields, camel-cased, with their Rust types, as settings.rs declares them. */
const TYPES = (() => {
  const body = read("src-tauri/src/settings.rs").match(/pub struct FeatureSettings \{([\s\S]*?)\n\}/)[1];
  return [...body.matchAll(/^\s*pub (\w+): (\w+)/gm)].map(([, name, type]) => [
    name.replace(/_(\w)/g, (_, letter) => letter.toUpperCase()),
    type,
  ]);
})();
const FIELDS = TYPES.map(([field]) => field);

/** Every switch off (and the other options at a value of their type). */
const allOff = Object.fromEntries(
  TYPES.map(([field, type]) => [field, type === "bool" ? false : type === "CrossfeedLevel" ? "off" : 1]),
);
const withOn = (...on) => ({ ...allOff, ...Object.fromEntries(on.map((field) => [field, true])) });

const first = (query, features = allOff) => find(query, text, features)[0];

test("matching ignores case and accents, from the start of any word", () => {
  assert.equal(fold("Égaliseur"), "egaliseur");
  assert.equal(matchIn(queryWords("ÉQ"), "Equaliser"), 0);
  assert.equal(matchIn(queryWords("fade"), "Crossfade"), null, "not inside a word");
  assert.equal(matchIn(queryWords("clip"), "Prevent clipping"), 1, "a later word");
  assert.equal(matchIn(queryWords("dark mode"), "dark mode, light mode"), 0);
  assert.equal(matchIn(queryWords("mode dark"), "dark mode"), 1, "each word anywhere");
  assert.equal(matchIn(queryWords("dark x"), "dark mode"), null, "every word must match");
});

test("nothing is found below two letters", () => {
  assert.equal(queryWords("e"), null);
  assert.equal(queryWords(" é "), null);
  assert.deepEqual(find("e", text, allOff), []);
  assert.ok(find("eq", text, allOff).length > 0);
});

test("a label match ranks above a synonym, and a synonym above the place", () => {
  const index = [
    { id: "place", label: "x.place", where: ["x.alpha"], place: { kind: "view", view: "home" } },
    { id: "synonym", label: "x.synonym", where: [], place: { kind: "view", view: "queue" } },
    { id: "later", label: "x.later", where: [], place: { kind: "view", view: "library" } },
    { id: "label", label: "x.alpha", where: [], place: { kind: "view", view: "artists" } },
  ];
  const messages = {
    "x.place": "Place",
    "x.alpha": "Alpha",
    "x.synonym": "Synonym",
    "x.later": "Later alpha",
    "find.synonyms.synonym": "beta, alpha",
  };
  const found = find("alp", (key) => messages[key] ?? null, allOff, index).map((result) => result.entry.id);
  assert.deepEqual(found, ["label", "later", "synonym", "place"], "a later word of the label ranks below its first");
});

test("the exit's queries put the right place first", () => {
  const cross = first("cross");
  assert.equal(cross.entry.id, "setting.crossfade");
  assert.deepEqual(cross.target, { kind: "settings", section: "playback", control: "setting-crossfade" });

  assert.equal(first("eq").entry.id, "section.equaliser");
  assert.deepEqual(first("eq").target, { kind: "settings", section: "equaliser", control: "settings-tab-equaliser" });

  // Recording off: the section shows as off and opens at its switch.
  const record = first("record");
  assert.equal(record.entry.id, "section.recording");
  assert.equal(record.state, "off");
  assert.deepEqual(record.target, { kind: "settings", section: "features", control: "feature-recording" });
  assert.deepEqual(first("record", withOn("recording")).target, {
    kind: "settings",
    section: "recording",
    control: "settings-tab-recording",
  });

  const theme = first("theme");
  assert.equal(theme.entry.id, "setting.theme");
  assert.equal(theme.state, "off");
  assert.deepEqual(theme.target, { kind: "settings", section: "features", control: "feature-themes" });
  assert.deepEqual(first("theme", withOn("themes")).target, {
    kind: "settings",
    section: "appearance",
    control: "setting-theme",
  });
});

test("synonyms find what their words don't", () => {
  assert.equal(first("dark mode").entry.id, "section.appearance");
  assert.equal(first("scrobble").entry.id, "switch.listenbrainz");
  assert.equal(first("tags").entry.id, "action.get-info");
  assert.deepEqual(first("sleep").target, { kind: "sleepTimer" }, "the player bar's, not in a list the guide checks");
  assert.equal(first("bedtime").entry.id, "bar.sleepTimer");
});

test("a switch says whether it is on", () => {
  assert.equal(first("lyrics").state, "off");
  assert.equal(first("lyrics", withOn("lyrics")).state, "on");
  assert.equal(first("lyrics").entry.feature, "lyrics");
  assert.equal(first("skip after").state, null, "a number, not a switch");
  assert.equal(first("crossfade").state, null, "a setting, always there");
});

test("a result opens at its view, its control, its action or the playlists", () => {
  const entry = (id) => INDEX.find((candidate) => candidate.id === id);
  assert.deepEqual(target(entry("view.favourites"), allOff), { kind: "view", view: "favourites" });
  assert.deepEqual(target(entry("action.get-info"), allOff), { kind: "action", action: "get-info" });
  assert.deepEqual(target(entry("view.playlists"), allOff), { kind: "playlists" });
  assert.deepEqual(target(entry("setting.preventClipping"), allOff), {
    kind: "settings",
    section: "playback",
    control: "setting-preventClipping",
  });
  assert.deepEqual(target(entry("switch.updateCheck"), allOff), {
    kind: "settings",
    section: "about",
    control: "feature-updateCheck",
  });
});

test("a feature that is off opens at its switch, and finding it turns nothing on", () => {
  const entry = (id) => INDEX.find((candidate) => candidate.id === id);
  const features = Object.freeze({ ...allOff });
  const atSwitch = (feature) => ({ kind: "settings", section: "features", control: `feature-${feature}` });
  assert.deepEqual(target(entry("view.home"), features), atSwitch("recentlyAdded"));
  assert.deepEqual(target(entry("view.home"), withOn("onThisDay")), { kind: "view", view: "home" });
  assert.deepEqual(target(entry("view.history"), features), atSwitch("listeningHistory"));
  assert.deepEqual(target(entry("view.workbench"), features), atSwitch("effectsWorkbench"));
  assert.deepEqual(target(entry("setting.font"), features), atSwitch("themes"));
  assert.deepEqual(target(entry("setting.reverb"), features), atSwitch("effects"));
  assert.deepEqual(target(entry("switch.remotePort"), features), atSwitch("remoteControl"));
  assert.deepEqual(target(entry("switch.remotePort"), withOn("remoteControl")), atSwitch("remotePort"));
  for (const query of ["record", "theme", "home", "reverb", "port"]) find(query, text, features);
  assert.deepEqual(features, allOff);
});

test("no two results open in the same place", () => {
  for (const query of ["home", "settings", "record", "theme", "library", "queue", "pl", "on"]) {
    const places = find(query, text, allOff).map((result) => JSON.stringify(result.target));
    assert.equal(new Set(places).size, places.length, query);
  }
  assert.equal(find("home", text, withOn("recentlyAdded")).filter((r) => r.entry.label === "sidebar.home").length, 1);
});

test("the developer page never appears", () => {
  const sidebar = read("src/lib/components/Sidebar.svelte");
  assert.ok(sidebar.includes("sidebar.developerTools"), "the sidebar still has its link");
  assert.ok(!INDEX.some((entry) => entry.label === "sidebar.developerTools"));
  for (const query of ["developer", "dev", "tools"]) {
    assert.ok(!find(query, text, allOff).some((result) => result.entry.label === "sidebar.developerTools"));
  }
});

test("entries are unique, and every label, place and synonym is in en.json", () => {
  const ids = INDEX.map((entry) => entry.id);
  assert.equal(new Set(ids).size, ids.length);
  for (const entry of INDEX) {
    for (const key of [entry.label, ...entry.where]) assert.ok(key in en, `${entry.id}: en.json lacks ${key}`);
  }
  for (const [key, value] of Object.entries(en)) {
    if (!key.startsWith("find.synonyms.")) continue;
    const id = key.slice("find.synonyms.".length);
    assert.ok(ids.includes(id), `${key} is for no entry`);
    assert.ok(
      value.split(",").every((synonym) => synonym.trim() !== ""),
      key,
    );
  }
});

test("every sidebar item has an entry, hidden as the sidebar hides it", () => {
  const sidebar = read("src/lib/components/Sidebar.svelte");
  // The same patterns as check-user-guide.py's sidebar_keys.
  const keys = new Set([
    ...[...sidebar.matchAll(/name: t\("([^"]+)"\)/g)].map((m) => m[1]),
    ...[...sidebar.matchAll(/<Icon [^>]*\/>\s*\{t\("([^"]+)"\)\}/g)].map((m) => m[1]),
    ...[...sidebar.matchAll(/<h2>\{t\("([^"]+)"\)\}<\/h2>/g)].map((m) => m[1]),
    ...[...sidebar.matchAll(/<Icon name="chevron"[^>]*\/>\s*\{t\("([^"]+)"\)\}/g)].map((m) => m[1]),
  ]);
  assert.ok(keys.size >= 14, `found the sidebar's items (${keys.size})`);
  for (const key of keys) {
    assert.ok(
      INDEX.some((entry) => entry.label === key && entry.id.startsWith("view.")),
      `no entry for the sidebar's ${key}`,
    );
  }
  // A view the sidebar shows only while features are on needs them.
  const views = [...sidebar.matchAll(/id: "(\w+)" as const,[\s\S]*?\son: ([^,}\n]+)/g)];
  assert.ok(views.length >= 6, `found the sidebar's views (${views.length})`);
  for (const [, id, on] of views) {
    const needs = on.trim() === "true" ? undefined : [...on.matchAll(/f\.(\w+)/g)].map((m) => m[1]);
    assert.deepEqual(INDEX.find((entry) => entry.id === `view.${id}`)?.needs, needs, id);
  }
});

test("every Settings section has an entry, hidden as Settings hides it", () => {
  const page = read("src/lib/components/SettingsPage.svelte");
  const all = page.match(/const ALL_SECTIONS[^=]*=\s*\[([\s\S]*?)\];/)[1];
  const sections = [...all.matchAll(/id: "(\w+)", name: "([^"]+)"/g)];
  assert.equal(sections.length, Object.keys(SECTION_NAMES).length);
  for (const [, id, name] of sections) {
    assert.equal(SECTION_NAMES[id], name, id);
    assert.ok(
      INDEX.some((entry) => entry.section === id),
      `no entry for the section ${id}`,
    );
  }
  const hidden = Object.fromEntries(
    [...page.matchAll(/candidate\.id !== "(\w+)" \|\| appSettings\.current\.features\.(\w+)/g)].map((m) => [
      m[1],
      [m[2]],
    ]),
  );
  assert.ok(Object.keys(hidden).length >= 2, "found the hidden sections");
  for (const entry of INDEX.filter((candidate) => candidate.place.kind === "settings" && !candidate.feature)) {
    const needs = hidden[entry.place.section];
    if (needs) assert.deepEqual(entry.needs, needs, entry.id);
  }
});

test("every FeatureSettings field has a switch entry, by the label the guide's check uses", () => {
  const script = read("../scripts/check-user-guide.py");
  const labels = Object.fromEntries(
    [...script.match(/SWITCH_LABELS = \{([\s\S]*?)\}/)[1].matchAll(/"(\w+)": "([\w.]+)"/g)].map((m) => [m[1], m[2]]),
  );
  assert.ok(FIELDS.length > 30, "found the fields");
  for (const field of FIELDS) {
    const entry = INDEX.find((candidate) => candidate.feature === field);
    assert.ok(entry, `no entry for the feature ${field}`);
    assert.equal(entry.label, labels[field] ?? `feature.${field}`, field);
  }
});

test("every menu item with a page action has an entry, and the page runs each", () => {
  const menu = read("src-tauri/src/shell/menu.rs");
  const items = [...menu.match(/const PAGE_ITEMS[^=]*=\s*\[([\s\S]*?)\];/)[1].matchAll(/\(\s*"([^"]+)",\s*"/g)].map(
    (m) => m[1],
  );
  assert.ok(items.length >= 20, "found the menu's items");
  const actions = read("src/lib/pageActions.ts");
  for (const id of items) {
    assert.ok(
      INDEX.some((entry) => entry.menu === id),
      `no entry for the menu item ${id}`,
    );
    assert.ok(actions.includes(`case "${id}":`), `pageActions.ts doesn't run ${id}`);
  }
});

test("every control an entry opens at is in its section", () => {
  const page = read("src/lib/components/SettingsPage.svelte");
  const imports = Object.fromEntries(
    [...page.matchAll(/import (\w+) from "\.\/([\w/]+\.svelte)";/g)].map((m) => [m[1], m[2]]),
  );
  const components = Object.fromEntries(
    [...page.matchAll(/section\.id === "(\w+)"\}\s*<(\w+)/g)].map((m) => [m[1], imports[m[2]]]),
  );
  components.sources = imports[page.match(/\{:else\}\s*<(\w+)/)[1]];
  assert.equal(Object.keys(components).length, Object.keys(SECTION_NAMES).length, "found each section's component");
  for (const entry of INDEX) {
    if (entry.place.kind !== "settings") continue;
    const { section, control } = entry.place;
    if (control.startsWith("settings-tab-")) {
      assert.equal(control, `settings-tab-${section}`, entry.id);
      assert.ok(page.includes('id="settings-tab-{candidate.id}"'));
      continue;
    }
    const source = read(`src/lib/components/${components[section]}`);
    if (source.includes(`id="${control}"`)) continue;
    // Drawn in a loop: `id="feature-{item.key}"` over a list with the key, `id="effect-{id}"` over the effects.
    const [, prefix, name] = control.match(/^(\w+-)(\w+)$/);
    assert.ok(source.includes(`id="${prefix}{`), `${entry.id}: no element ${control} in ${components[section]}`);
    if (prefix === "feature-") assert.ok(source.includes(`key: "${name}"`), `${entry.id}: no switch ${name}`);
    else assert.ok(`effects.${name}` in en, `${entry.id}: no effect ${name}`);
  }
});
