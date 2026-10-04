// The user guide's parser (src/lib/guide.ts, PLAN.md Phase 7c D1), and
// every page of the real guide through it.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { guidePages, linkTarget, parseInline, parsePage, plain, slug } from "../src/lib/guide.ts";

const GUIDE = new URL("../../docs/user-guide/", import.meta.url);

test("slugs are GitHub's", () => {
  assert.equal(slug("Folders that can't be found"), "folders-that-cant-be-found");
  assert.equal(slug("Match the device’s sample rate"), "match-the-devices-sample-rate");
  assert.equal(slug("Albums as one file: cue sheets and chapters"), "albums-as-one-file-cue-sheets-and-chapters");
  assert.equal(slug("2. Finding music"), "2-finding-music");
});

test("inline markup", () => {
  assert.deepEqual(parseInline("Press **⌘F** or `x:y` *now* a\\|b", "a.md"), [
    { kind: "text", text: "Press " },
    { kind: "strong", children: [{ kind: "text", text: "⌘F" }] },
    { kind: "text", text: " or " },
    { kind: "code", text: "x:y" },
    { kind: "text", text: " " },
    { kind: "em", children: [{ kind: "text", text: "now" }] },
    { kind: "text", text: " a|b" },
  ]);
  const [link] = parseInline("[**Settings**](11-settings.md#sorting)", "a.md");
  assert.deepEqual(link.target, { kind: "page", page: "11-settings.md", anchor: "sorting" });
  assert.equal(plain(link.children), "Settings");
});

test("where links lead", () => {
  assert.deepEqual(linkTarget("#queue", "03.md"), { kind: "page", page: "03.md", anchor: "queue" });
  assert.deepEqual(linkTarget("01-a.md", "03.md"), { kind: "page", page: "01-a.md", anchor: null });
  assert.deepEqual(linkTarget("https://musicbrainz.org", "03.md"), { kind: "web", url: "https://musicbrainz.org" });
  for (const href of ["../../PLAN.md", "javascript:alert(1)", "file:///etc", "pic.png"]) {
    assert.deepEqual(linkTarget(href, "03.md"), { kind: "none" }, href);
  }
});

test("blocks: headings, paragraphs, nested lists with a table, comments dropped", () => {
  const page = parsePage(
    "x.md",
    [
      "# Title",
      "",
      "<!-- Screenshot:",
      "  something. -->",
      "",
      "A paragraph",
      "over two lines.",
      "",
      "1. First step,",
      "   continued.",
      "",
      "   | A | B |",
      "   |---|---|",
      "   | 1 | 2 |",
      "",
      "2. Second step:",
      "   - a bullet",
      "     - a nested one",
      "",
      "## Title",
    ].join("\n"),
  );
  assert.equal(page.title, "Title");
  const [title, paragraph, list, again] = page.blocks;
  assert.deepEqual(title, { kind: "heading", level: 1, id: "title", children: [{ kind: "text", text: "Title" }] });
  assert.equal(again.id, "title-1");
  assert.equal(plain(paragraph.children), "A paragraph over two lines.");
  assert.equal(list.kind, "list");
  assert.equal(list.ordered, true);
  assert.equal(list.items.length, 2);
  const [step, table] = list.items[0];
  assert.equal(plain(step.children), "First step, continued.");
  assert.equal(table.kind, "table");
  assert.deepEqual(
    table.rows.map((row) => row.map(plain)),
    [["1", "2"]],
  );
  const [, bullets] = list.items[1];
  assert.equal(bullets.kind, "list");
  assert.equal(bullets.ordered, false);
  assert.equal(bullets.items[0][1].kind, "list");
});

/** Every text of `blocks`, and every page link, depth first. */
function walk(blocks, texts, links) {
  const inline = (inlines) => {
    for (const item of inlines) {
      if (item.kind === "text") texts.push(item.text);
      if (item.kind === "link") links.push(item.target);
      if ("children" in item) inline(item.children);
    }
  };
  for (const block of blocks) {
    if (block.kind === "heading" || block.kind === "paragraph") inline(block.children);
    if (block.kind === "list") block.items.forEach((item) => walk(item, texts, links));
    if (block.kind === "table") [block.head, ...block.rows].flat().forEach(inline);
  }
}

test("every page of the guide parses fully, and its links lead somewhere", () => {
  const files = Object.fromEntries(
    readdirSync(GUIDE)
      .filter((name) => name.endsWith(".md"))
      .map((name) => [name, readFileSync(new URL(name, GUIDE), "utf8")]),
  );
  const pages = guidePages(files);
  assert.equal(pages[0].name, "README.md");
  assert.equal(pages.length, Object.keys(files).length);
  const ids = new Map(
    pages.map((page) => {
      const found = [];
      const collect = (blocks) => {
        for (const block of blocks) {
          if (block.kind === "heading") found.push(block.id);
          if (block.kind === "list") block.items.forEach(collect);
        }
      };
      collect(page.blocks);
      return [page.name, new Set(found)];
    }),
  );
  for (const page of pages) {
    const texts = [];
    const links = [];
    walk(page.blocks, texts, links);
    for (const text of texts) {
      assert.doesNotMatch(text, /\*\*|\]\(|<!--|^#|```/, `${page.name}: markup left in “${text}”`);
    }
    for (const target of links) {
      assert.notEqual(target.kind, "none", `${page.name}: a link the Help window can't follow`);
      if (target.kind !== "page") continue;
      assert.ok(ids.has(target.page), `${page.name}: no page ${target.page}`);
      if (target.anchor) assert.ok(ids.get(target.page).has(target.anchor), `${page.name}: no #${target.anchor}`);
    }
  }
});
