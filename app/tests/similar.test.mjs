// Recommendations' reasons (src/lib/similar.ts, PLAN.md X4 and X5): each
// kind has a message in en.json, with the parameters it names.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { outsideReasonMessage, outsideReasonsText, reasonMessage, reasonsText } from "../src/lib/similar.ts";

const root = new URL("..", import.meta.url).pathname;
const en = JSON.parse(readFileSync(join(root, "src/lib/i18n/en.json"), "utf8"));

const REASONS = [
  { kind: "genre", name: "Jazz" },
  { kind: "era", year: 1994 },
  { kind: "label", name: "Blue Note" },
  { kind: "linked", name: "The Band", relation: "member" },
  { kind: "linked", name: "The Band", relation: "subgroup" },
  { kind: "linked", name: "The Band", relation: "with" },
  { kind: "artist", name: "Quartet" },
  { kind: "composer", name: "Bach" },
  { kind: "together", times: 3 },
  { kind: "loudness" },
];

/** A stand-in for `t`: the English message filled in, plurals by count. */
function t(key, params) {
  let message = en[key];
  if (typeof message === "object") message = params.count === 1 ? message.one : message.other;
  return message.replace(/\{(\w+)\}/g, (whole, name) => String(params[name] ?? whole));
}

test("every reason has a message, and fills each of its placeholders", () => {
  for (const reason of REASONS) {
    const [key, params] = reasonMessage(reason);
    assert.ok(key in en, `${reason.kind}: ${key} isn't in en.json`);
    const text = t(key, params);
    assert.doesNotMatch(text, /\{\w+\}/, `${key}: ${text}`);
  }
});

test("a year is passed as text, so it isn't grouped like a number", () => {
  assert.deepEqual(reasonMessage({ kind: "era", year: 1994 })[1], { year: "1994" });
});

test("reasons read strongest first", () => {
  const text = reasonsText(
    [
      { kind: "genre", name: "Jazz" },
      { kind: "together", times: 1 },
    ],
    t,
  );
  assert.equal(text, "Jazz · played together once");
});

const OUTSIDE_REASONS = [
  { kind: "listenBrainz", name: "Radiohead" },
  { kind: "linked", name: "Radiohead", relation: "member" },
  { kind: "linked", name: "Radiohead", relation: "subgroup" },
  { kind: "linked", name: "Radiohead", relation: "with" },
];

test("every outside reason has a message, and fills each of its placeholders", () => {
  for (const reason of OUTSIDE_REASONS) {
    const [key, params] = outsideReasonMessage(reason);
    assert.ok(key in en, `${reason.kind}: ${key} isn't in en.json`);
    assert.doesNotMatch(t(key, params), /\{\w+\}/, key);
  }
  assert.equal(outsideReasonsText(OUTSIDE_REASONS.slice(0, 2), t), "like Radiohead on ListenBrainz · Radiohead member");
});
