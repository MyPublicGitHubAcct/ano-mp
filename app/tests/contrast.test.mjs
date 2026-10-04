// Every built-in theme's colours meet WCAG 2.1 AA contrast (PLAN.md F18,
// X1), in both schemes: 4.5:1 for text (1.4.3), 3:1 for icons, focus
// rings and marks (1.4.11). The pairs are `theme.ts`'s `PAIRS`, which the
// theme editor also flags.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { BUILT_IN, contrastIssues } from "../src/lib/theme.ts";

const layout = readFileSync(new URL("../src/routes/+layout.svelte", import.meta.url), "utf8");

test("built-in themes meet WCAG AA contrast in light and dark", () => {
  for (const theme of BUILT_IN) {
    for (const scheme of ["light", "dark"]) {
      const issues = contrastIssues(theme[scheme]).map(
        ({ fg, bg, ratio, minimum }) => `${fg} on ${bg} is ${ratio.toFixed(2)}:1, under ${minimum}:1`,
      );
      assert.deepEqual(issues, [], `${theme.preset} (${scheme})`);
    }
  }
});

test("focus is always visible", () => {
  assert.match(layout, /:global\(:focus-visible\)\s*\{[^}]*outline:\s*2px solid var\(--accent\)/);
});
