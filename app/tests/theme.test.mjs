// Themes (src/lib/theme.ts): the built-ins, the tokens a theme becomes,
// the editor's contrast flags and the accent from the cover.
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  BUILT_IN,
  COLOR_TOKENS,
  DEFAULT_THEME,
  HIGH_CONTRAST,
  TEXT,
  contrast,
  contrastIssues,
  coverAccent,
  kebab,
  rowHeight,
  sameLook,
  tokens,
} from "../src/lib/theme.ts";

const plain = { systemDark: false, moreContrast: false, cover: null };

test("built-in themes are distinct and complete", () => {
  assert.equal(DEFAULT_THEME.preset, "system");
  const presets = BUILT_IN.map((theme) => theme.preset);
  assert.equal(new Set(presets).size, presets.length);
  for (const theme of BUILT_IN) {
    for (const palette of [theme.light, theme.dark]) {
      assert.deepEqual(Object.keys(palette).sort(), [...COLOR_TOKENS].sort(), theme.preset);
      for (const color of Object.values(palette)) assert.match(color, /^#[0-9a-f]{6}$/, theme.preset);
    }
  }
});

test("a theme becomes custom properties for its scheme", () => {
  const light = tokens(DEFAULT_THEME, plain);
  assert.equal(light.scheme, "light");
  assert.equal(light.props.bg, DEFAULT_THEME.light.bg);
  assert.equal(light.props["surface-2"], DEFAULT_THEME.light.surface2);
  assert.equal(light.props["accent-text"], DEFAULT_THEME.light.accentText);
  assert.equal(light.props["text-size"], "15px");
  assert.equal(light.props.radius, "6px");
  assert.equal(light.props["radius-lg"], "8px");
  assert.equal(light.props["radius-sm"], "4px");
  assert.equal(light.props.density, "1");
  assert.match(light.props.font, /system-ui/);

  const dark = tokens(DEFAULT_THEME, { ...plain, systemDark: true });
  assert.equal(dark.scheme, "dark");
  assert.equal(dark.props.bg, DEFAULT_THEME.dark.bg);

  // A theme with its own scheme ignores the system's.
  const forcedLight = BUILT_IN.find((theme) => theme.preset === "light");
  assert.equal(tokens(forcedLight, { ...plain, systemDark: true }).scheme, "light");
});

test("the system's request for more contrast swaps in the high-contrast colours, keeping the rest", () => {
  const theme = { ...DEFAULT_THEME, font: "serif", textSize: 18 };
  const { props } = tokens(theme, { ...plain, moreContrast: true });
  assert.equal(props.text, HIGH_CONTRAST.light.text);
  assert.equal(props["text-size"], "18px");
  assert.match(props.font, /serif/);
});

test("names become CSS names", () => {
  assert.equal(kebab("surface2"), "surface-2");
  assert.equal(kebab("textMuted"), "text-muted");
  assert.equal(kebab("bg"), "bg");
});

test("contrast follows WCAG, and the editor flags a failing pair", () => {
  assert.equal(contrast("#000000", "#ffffff").toFixed(1), "21.0");
  assert.equal(contrast("#777777", "#777777"), 1);
  const palette = { ...DEFAULT_THEME.light, textMuted: "#b0b0b0" };
  const issues = contrastIssues(palette);
  assert.ok(issues.length > 0);
  assert.ok(issues.every((issue) => issue.fg === "textMuted" && issue.minimum === TEXT));
});

test("the cover's accent is readable on the theme, in both schemes", () => {
  for (const cover of [
    [250, 230, 40], // A pale yellow: too light on white.
    [20, 30, 120], // A deep blue: too dark on black.
    [128, 128, 128], // Grey.
  ]) {
    for (const palette of [DEFAULT_THEME.light, DEFAULT_THEME.dark, HIGH_CONTRAST.dark]) {
      const { accent, accentText } = coverAccent(cover, palette);
      assert.ok(contrast(accent, palette.bg) >= TEXT, `${accent} on ${palette.bg}`);
      assert.ok(contrast(accent, palette.surface) >= TEXT, `${accent} on ${palette.surface}`);
      assert.ok(contrast(accentText, accent) >= TEXT, `${accentText} on ${accent}`);
    }
  }
  const midnight = BUILT_IN.find((theme) => theme.accentFromCover);
  const { props } = tokens(midnight, { ...plain, cover: [200, 40, 40] });
  assert.notEqual(props.accent, midnight.dark.accent);
  // Not while the system asks for more contrast.
  const calm = tokens(midnight, { ...plain, cover: [200, 40, 40], moreContrast: true });
  assert.equal(calm.props.accent, HIGH_CONTRAST.dark.accent);
});

test("rows grow with the text and the density", () => {
  assert.equal(rowHeight(40, DEFAULT_THEME), 40);
  assert.equal(rowHeight(40, { ...DEFAULT_THEME, textSize: 18 }), 48);
  assert.equal(rowHeight(40, { ...DEFAULT_THEME, density: "compact" }), 34);
});

test("themes compare by look, not name", () => {
  assert.ok(sameLook(DEFAULT_THEME, { ...DEFAULT_THEME, name: "Mine", preset: "custom" }));
  assert.ok(!sameLook(DEFAULT_THEME, { ...DEFAULT_THEME, radius: 7 }));
  assert.ok(!sameLook(DEFAULT_THEME, { ...DEFAULT_THEME, light: { ...DEFAULT_THEME.light, bg: "#000000" } }));
  const reordered = Object.fromEntries(Object.entries(DEFAULT_THEME).reverse());
  assert.ok(sameLook(DEFAULT_THEME, reordered));
});
