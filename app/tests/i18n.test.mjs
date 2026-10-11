// The message catalogue (PLAN.md F19): well-formed entries, and a message
// for every error code the Rust side sends (`src-tauri/src/coded.rs`), and
// a form per platform for text that names macOS (`platform.ts`).
// Keys the UI uses are checked by `npm run check`, since `t` takes only
// keys `en.json` has.
import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { detect } from "../src/lib/platform.ts";

const root = new URL("..", import.meta.url).pathname;
const en = JSON.parse(readFileSync(join(root, "src/lib/i18n/en.json"), "utf8"));

function rustFiles(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return rustFiles(path);
    return path.endsWith(".rs") ? [path] : [];
  });
}

test("every message is text, or plural forms with an `other`", () => {
  for (const [key, message] of Object.entries(en)) {
    if (typeof message === "string") continue;
    assert.equal(typeof message, "object", key);
    assert.equal(typeof message.other, "string", `${key} has no "other" form`);
    for (const [form, text] of Object.entries(message)) {
      assert.ok(["zero", "one", "two", "few", "many", "other"].includes(form), `${key}: ${form}`);
      assert.equal(typeof text, "string", `${key}.${form}`);
    }
  }
});

test("every error code the backend sends has a message", () => {
  const codes = new Set();
  for (const path of rustFiles(join(root, "src-tauri/src"))) {
    const text = readFileSync(path, "utf8");
    for (const match of text.matchAll(/\bcoded\(\s*"(\w+)"/g)) codes.add(match[1]);
  }
  assert.ok(codes.has("trackGone") && codes.has("featureOff"), "found the codes");
  for (const code of codes) assert.ok(`error.${code}` in en, `en.json lacks error.${code}`);
});

test("a message's placeholders are simple names", () => {
  for (const [key, message] of Object.entries(en)) {
    const texts = typeof message === "string" ? [message] : Object.values(message);
    for (const text of texts) {
      const opened = (text.match(/\{/g) ?? []).length;
      const names = (text.match(/\{\w+\}/g) ?? []).length;
      assert.equal(opened, names, `${key}: ${text}`);
    }
  }
});

const PLATFORMS = ["ios", "linux", "windows"];
const placeholders = (message) =>
  [
    ...new Set(
      (typeof message === "string" ? [message] : Object.values(message)).flatMap(
        (text) => text.match(/\{\w+\}/g) ?? [],
      ),
    ),
  ].sort();

test("a platform's form of a message matches the message", () => {
  for (const [key, message] of Object.entries(en)) {
    const at = key.lastIndexOf("@");
    if (at < 0) continue;
    const base = key.slice(0, at);
    assert.ok(PLATFORMS.includes(key.slice(at + 1)), `${key}: not a platform`);
    assert.ok(base in en, `${key} has no ${base}`);
    assert.equal(typeof message, typeof en[base], `${key} and ${base} differ in kind`);
    assert.deepEqual(placeholders(message), placeholders(en[base]), `${key}'s placeholders`);
  }
});

test("a message that names macOS has a form for Windows and Linux", () => {
  // Shortcuts go in a {keys} parameter (keys.ts) instead of a ⌘.
  const macOnly = /\bmacOS\b|\bMac\b|Mac’|\bFinder\b|iCloud|keychain|⌘|⌥/;
  for (const [key, message] of Object.entries(en)) {
    if (key.includes("@")) continue;
    const texts = typeof message === "string" ? [message] : Object.values(message);
    if (!texts.some((text) => macOnly.test(text))) continue;
    for (const platform of ["linux", "windows"]) {
      const form = en[`${key}@${platform}`];
      assert.ok(form !== undefined, `${key} names macOS but has no ${key}@${platform}`);
      const formTexts = typeof form === "string" ? [form] : Object.values(form);
      assert.ok(!formTexts.some((text) => macOnly.test(text)), `${key}@${platform} still names macOS`);
    }
  }
});

test("the platform comes from navigator.platform and touch points", () => {
  assert.equal(detect("MacIntel"), "macos");
  assert.equal(detect("MacIntel", 5), "ios");
  assert.equal(detect("iPhone"), "ios");
  assert.equal(detect("Win32"), "windows");
  assert.equal(detect("Linux x86_64"), "linux");
});
