// Themes (PLAN.md X1): the built-in ones, the CSS custom properties a theme
// becomes, the WCAG checks the editor flags, and the accent taken from the
// current cover. Pure functions, tested by `tests/theme.test.mjs`; the
// appearance store (`state/appearance.svelte.ts`) applies them to `:root`.
//
// A theme is only token values, each validated by `theme.rs`: colours as
// `#rrggbb`, a font from `FONTS`, numbers in range. Nothing the user enters
// becomes CSS text, so the CSP and `validate` stay meaningful.

import type { Density, Font, Theme, ThemePalette } from "./generated/settings";
import builtIn from "./themes.json" with { type: "json" };

/** The built-in themes, the first of which is the default (`theme.rs` reads the same file). */
export const BUILT_IN = builtIn as Theme[];

export const DEFAULT_THEME = BUILT_IN[0];

/** The high-contrast theme, whose colours replace a theme's while the OS asks for more contrast. */
export const HIGH_CONTRAST = BUILT_IN.find((theme) => theme.preset === "highContrast")!;

/** The font stacks behind `Font`; only these ever reach CSS. */
export const FONTS: Record<Font, string> = {
  system: 'system-ui, -apple-system, "Segoe UI", sans-serif',
  rounded: 'ui-rounded, "SF Pro Rounded", system-ui, sans-serif',
  serif: 'ui-serif, "New York", Georgia, "Times New Roman", serif',
  mono: 'ui-monospace, "SF Mono", Menlo, Consolas, monospace',
  humanist: '"Avenir Next", Avenir, "Segoe UI", "Helvetica Neue", sans-serif',
};

/** How much room each density gives, against regular. */
export const DENSITY: Record<Density, number> = { compact: 0.85, regular: 1, roomy: 1.2 };

/** The text size every layout was drawn at, in px; list rows scale from it. */
export const BASE_TEXT_SIZE = 15;
export const TEXT_SIZES = { min: 12, max: 22 };
export const RADII = { min: 0, max: 16 };

/** The colours a theme sets, in the editor's order. */
export const COLOR_TOKENS: (keyof ThemePalette)[] = [
  "bg",
  "surface",
  "surface2",
  "text",
  "textMuted",
  "textFaint",
  "border",
  "accent",
  "accentText",
  "danger",
  "heart",
  "star",
];

export type Rgb = [number, number, number];

export function parseHex(hex: string): Rgb {
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as Rgb;
}

export function toHex(rgb: Rgb): string {
  return (
    "#" +
    rgb
      .map((c) =>
        Math.round(Math.min(255, Math.max(0, c)))
          .toString(16)
          .padStart(2, "0"),
      )
      .join("")
  );
}

/** WCAG 2.1's relative luminance. */
export function luminance(hex: string): number {
  const [r, g, b] = parseHex(hex).map((value) => {
    const c = value / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG 2.1's contrast ratio, 1 to 21. */
export function contrast(a: string, b: string): number {
  const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (high + 0.05) / (low + 0.05);
}

/** 4.5:1 for text (1.4.3), 3:1 for icons, focus rings and marks (1.4.11). */
export const TEXT = 4.5;
export const GRAPHIC = 3;

/** [foreground, backgrounds, minimum]: what each colour is drawn on, and what it needs. */
export const PAIRS: [keyof ThemePalette, (keyof ThemePalette)[], number][] = [
  ["text", ["bg", "surface", "surface2"], TEXT],
  ["textMuted", ["bg", "surface", "surface2"], TEXT],
  ["accent", ["bg", "surface"], TEXT], // Links and the playing row's name.
  ["danger", ["bg", "surface"], TEXT],
  ["accentText", ["accent"], TEXT],
  ["textFaint", ["bg", "surface"], GRAPHIC], // Icons and separators only, never text.
  ["heart", ["bg", "surface"], GRAPHIC],
  ["star", ["bg", "surface"], GRAPHIC],
];

export type ContrastIssue = { fg: keyof ThemePalette; bg: keyof ThemePalette; ratio: number; minimum: number };

/** Each pair of `palette` under WCAG AA, for the editor to flag. */
export function contrastIssues(palette: ThemePalette): ContrastIssue[] {
  return PAIRS.flatMap(([fg, backgrounds, minimum]) =>
    backgrounds
      .map((bg) => ({ fg, bg, ratio: contrast(palette[fg], palette[bg]), minimum }))
      .filter((issue) => issue.ratio < minimum),
  );
}

/** Whether the palette a theme uses is the dark one, given the system's scheme. */
export function isDark(theme: Theme, systemDark: boolean): boolean {
  return theme.scheme === "dark" || (theme.scheme === "system" && systemDark);
}

export type Conditions = {
  /** The system asks for dark mode. */
  systemDark: boolean;
  /** The system asks for more contrast, and the user lets that change the colours. */
  moreContrast: boolean;
  /** The current cover's main colour, when the theme takes its accent from the cover. */
  cover: Rgb | null;
};

/** The palette `theme` shows under `conditions`. */
export function activePalette(theme: Theme, conditions: Conditions): ThemePalette {
  const dark = isDark(theme, conditions.systemDark);
  const source = conditions.moreContrast ? HIGH_CONTRAST : theme;
  const palette = { ...(dark ? source.dark : source.light) };
  if (theme.accentFromCover && conditions.cover && !conditions.moreContrast) {
    Object.assign(palette, coverAccent(conditions.cover, palette));
  }
  return palette;
}

/** The CSS custom properties for `theme` (names without the `--`), and its colour scheme. */
export function tokens(
  theme: Theme,
  conditions: Conditions,
): { scheme: "light" | "dark"; props: Record<string, string> } {
  const dark = isDark(theme, conditions.systemDark);
  const palette = activePalette(theme, conditions);
  const props: Record<string, string> = {};
  for (const token of COLOR_TOKENS) props[kebab(token)] = palette[token];
  const [ar, ag, ab] = parseHex(palette.accent);
  const [tr, tg, tb] = parseHex(palette.text);
  props["hover"] = `rgb(${tr} ${tg} ${tb} / ${dark ? 0.07 : 0.05})`;
  props["selected"] = `rgb(${ar} ${ag} ${ab} / ${dark ? 0.16 : 0.12})`;
  props["shadow"] = `0 6px 24px rgb(0 0 0 / ${dark ? 0.5 : 0.14})`;
  props["font"] = FONTS[theme.font] ?? FONTS.system;
  props["text-size"] = `${theme.textSize}px`;
  props["density"] = String(DENSITY[theme.density] ?? 1);
  props["radius-sm"] = `${Math.round((theme.radius * 2) / 3)}px`;
  props["radius"] = `${theme.radius}px`;
  props["radius-lg"] = `${Math.round((theme.radius * 4) / 3)}px`;
  return { scheme: dark ? "dark" : "light", props };
}

/** `surface2` → `surface-2`, `accentText` → `accent-text`. */
export function kebab(name: string): string {
  return name.replace(/[A-Z0-9]/g, (c) => `-${c.toLowerCase()}`);
}

/** A list row's height for `theme`, from the height it was drawn at (regular density, 15px text). */
export function rowHeight(base: number, theme: Theme): number {
  return Math.round(base * (theme.textSize / BASE_TEXT_SIZE) * (DENSITY[theme.density] ?? 1));
}

/** An accent with the cover's hue that is readable on the palette's backgrounds, and text that is readable
    on it. */
export function coverAccent(cover: Rgb, palette: ThemePalette): Pick<ThemePalette, "accent" | "accentText"> {
  const [hue, saturation] = toHsl(cover);
  const s = Math.min(0.85, Math.max(0.35, saturation));
  const darkBackground = luminance(palette.bg) < 0.2;
  const readable = (hex: string) => contrast(hex, palette.bg) >= TEXT && contrast(hex, palette.surface) >= TEXT;
  // Out from the middle towards the side away from the background, until the text contrast holds.
  let accent = palette.accent;
  for (let step = 0; step <= 50; step++) {
    const l = darkBackground ? 0.55 + step * 0.009 : 0.45 - step * 0.009;
    const candidate = toHex(hsl(hue, s, l));
    if (readable(candidate)) {
      accent = candidate;
      break;
    }
  }
  const accentText =
    [palette.accentText, "#ffffff", "#000000"].find((candidate) => contrast(candidate, accent) >= TEXT) ??
    (contrast("#ffffff", accent) > contrast("#000000", accent) ? "#ffffff" : "#000000");
  return { accent, accentText };
}

/** Whether two themes look the same (names aside). */
export function sameLook(a: Theme, b: Theme): boolean {
  return canonical({ ...a, name: "", preset: "" }) === canonical({ ...b, name: "", preset: "" });
}

/** JSON with each object's keys sorted, so key order doesn't matter. */
function canonical(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  const entries = Object.entries(value).sort(([x], [y]) => (x < y ? -1 : 1));
  return `{${entries.map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`).join(",")}}`;
}

// The same as the visualizer's (`visualizer/util.ts`), which Node's test runner can't import from here.

function hsl(h: number, s: number, l: number): Rgb {
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    return 255 * (l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1)));
  };
  return [f(0), f(8), f(4)];
}

function toHsl([r, g, b]: Rgb): [number, number, number] {
  const [rr, gg, bb] = [r / 255, g / 255, b / 255];
  const max = Math.max(rr, gg, bb);
  const min = Math.min(rr, gg, bb);
  const l = (max + min) / 2;
  const d = max - min;
  if (d === 0) return [0, 0, l];
  const s = d / (1 - Math.abs(2 * l - 1));
  const h = max === rr ? ((gg - bb) / d + 6) % 6 : max === gg ? (bb - rr) / d + 2 : (rr - gg) / d + 4;
  return [h * 60, s, l];
}
