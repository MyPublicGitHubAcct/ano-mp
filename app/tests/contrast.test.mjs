// The theme's colours meet WCAG 2.1 AA contrast (PLAN.md F18), in both
// schemes, as `routes/+layout.svelte` defines them: 4.5:1 for text
// (1.4.3), 3:1 for icons, focus rings and marks (1.4.11).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const layout = readFileSync(new URL("../src/routes/+layout.svelte", import.meta.url), "utf8");

/** The opaque colour tokens of each `:global(:root) { … }` block: light, then dark. */
function schemes() {
  const blocks = [...layout.matchAll(/:global\(:root\)\s*\{([^}]*)\}/g)].map((match) => match[1]);
  assert.equal(blocks.length, 2, "a light and a dark :root block");
  return blocks.map((block) =>
    Object.fromEntries([...block.matchAll(/--([\w-]+):\s*(#[0-9a-f]{6})\s*;/gi)].map((match) => [match[1], match[2]])),
  );
}

function luminance(hex) {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a, b) {
  const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (high + 0.05) / (low + 0.05);
}

const TEXT = 4.5;
const GRAPHIC = 3;

/** [foreground, backgrounds, minimum]. */
const PAIRS = [
  ["text", ["bg", "surface", "surface-2"], TEXT],
  ["text-muted", ["bg", "surface", "surface-2"], TEXT],
  ["accent", ["bg", "surface"], TEXT], // Links and the playing row's name.
  ["danger", ["bg", "surface"], TEXT],
  ["accent-text", ["accent"], TEXT],
  ["text-faint", ["bg", "surface"], GRAPHIC], // Icons and separators only, never text.
  ["heart", ["bg", "surface"], GRAPHIC],
  ["star", ["bg", "surface"], GRAPHIC],
];

test("theme colours meet WCAG AA contrast in light and dark", () => {
  for (const [index, tokens] of schemes().entries()) {
    const scheme = index === 0 ? "light" : "dark";
    for (const [fg, backgrounds, minimum] of PAIRS) {
      for (const bg of backgrounds) {
        assert.ok(tokens[fg] && tokens[bg], `${scheme} defines --${fg} and --${bg}`);
        const ratio = contrast(tokens[fg], tokens[bg]);
        assert.ok(ratio >= minimum, `${scheme}: --${fg} on --${bg} is ${ratio.toFixed(2)}:1, under ${minimum}:1`);
      }
    }
  }
});

test("focus is always visible", () => {
  assert.match(layout, /:global\(:focus-visible\)\s*\{[^}]*outline:\s*2px solid var\(--accent\)/);
});
