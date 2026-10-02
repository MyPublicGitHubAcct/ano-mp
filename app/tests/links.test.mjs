// Web links the app opens (src/lib/links.ts, PLAN.md H3).
import assert from "node:assert/strict";
import { test } from "node:test";
import { webLink } from "../src/lib/links.ts";

test("https links pass through", () => {
  assert.equal(webLink("https://musicbrainz.org/artist/x"), "https://musicbrainz.org/artist/x");
  assert.equal(webLink("https://en.wikipedia.org/wiki/A_(b)?c=d#e"), "https://en.wikipedia.org/wiki/A_(b)?c=d#e");
});

test("http links are upgraded to https", () => {
  assert.equal(webLink("http://example.com/band"), "https://example.com/band");
  assert.equal(webLink("HTTP://Example.COM"), "https://example.com/");
  assert.equal(webLink("http://example.com:80/"), "https://example.com/");
  assert.equal(webLink("http://example.com:8080/a"), "https://example.com:8080/a");
});

test("anything else is not a link", () => {
  for (const url of [
    null,
    undefined,
    "",
    "example.com",
    "/relative",
    "javascript:alert(1)",
    "file:///etc/passwd",
    "mailto:band@example.com",
    "ftp://example.com/",
    "data:text/html,hi",
    "tauri://localhost/",
    "anomp-art://localhost/album-1",
    "https://user:pass@example.com/",
    "http://user@example.com/",
  ]) {
    assert.equal(webLink(url), null, String(url));
  }
});
