// The user guide in the Help window (PLAN.md Phase 7c D1): its Markdown
// pages (docs/user-guide/), bundled with the app, parsed into a tree the
// Help page renders with ordinary elements, never as HTML, so nothing in a
// page can become markup or script. Pure, so plain `node --test` covers it.
//
// Only what the guide uses is understood: headings, paragraphs, bullet and
// numbered lists (nested, with tables or paragraphs inside), tables,
// **bold**, *emphasis*, `code` and links; HTML comments (the screenshot
// markers) are dropped. Anything else shows as its text.

export type Inline =
  | { kind: "text"; text: string }
  | { kind: "strong"; children: Inline[] }
  | { kind: "em"; children: Inline[] }
  | { kind: "code"; text: string }
  | { kind: "link"; target: LinkTarget; children: Inline[] };

/** Where a link goes: another page of the guide (and a heading on it), a
    web page, or nowhere the Help window can follow (shown as text). */
export type LinkTarget =
  { kind: "page"; page: string; anchor: string | null } | { kind: "web"; url: string } | { kind: "none" };

export type Block =
  | { kind: "heading"; level: number; id: string; children: Inline[] }
  | { kind: "paragraph"; children: Inline[] }
  | { kind: "list"; ordered: boolean; start: number; items: Block[][] }
  | { kind: "table"; head: Inline[][]; rows: Inline[][][] };

export type Page = { name: string; title: string; blocks: Block[] };

/** The page the guide opens on: its index. */
export const INDEX = "README.md";

/** A heading's anchor as GitHub makes it, so links between the pages work
    in both places: lower case, punctuation dropped, spaces as hyphens. */
export function slug(text: string): string {
  return text
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}_\- ]/gu, "")
    .replaceAll(" ", "-");
}

/** The plain text of `inlines`, for slugs and titles. */
export function plain(inlines: Inline[]): string {
  return inlines.map((inline) => ("children" in inline ? plain(inline.children) : inline.text)).join("");
}

/** Where `href`, written on a page of the guide, leads. */
export function linkTarget(href: string, page: string): LinkTarget {
  if (/^https?:\/\//i.test(href)) return { kind: "web", url: href };
  const match = /^([\w.-]*\.md)?(?:#(.*))?$/.exec(href);
  if (!match || (match[1] === undefined && match[2] === undefined)) return { kind: "none" };
  return { kind: "page", page: match[1] ?? page, anchor: match[2] || null };
}

/** The inline content of one paragraph, heading or cell. */
export function parseInline(text: string, page: string): Inline[] {
  const out: Inline[] = [];
  let plainText = "";
  const flush = () => {
    if (plainText) out.push({ kind: "text", text: plainText });
    plainText = "";
  };
  let i = 0;
  while (i < text.length) {
    const rest = text.slice(i);
    let match: RegExpExecArray | null;
    if (rest.startsWith("\\") && rest.length > 1 && /[\\`*_[\]()#|!-]/.test(rest[1])) {
      plainText += rest[1];
      i += 2;
    } else if ((match = /^`([^`]+)`/.exec(rest))) {
      flush();
      out.push({ kind: "code", text: match[1] });
      i += match[0].length;
    } else if ((match = /^\*\*(.+?)\*\*/s.exec(rest))) {
      flush();
      out.push({ kind: "strong", children: parseInline(match[1], page) });
      i += match[0].length;
    } else if ((match = /^\*([^*\s](?:[^*]*[^*\s])?)\*/.exec(rest))) {
      flush();
      out.push({ kind: "em", children: parseInline(match[1], page) });
      i += match[0].length;
    } else if ((match = /^\[([^\]]+)\]\(([^)\s]+)\)/.exec(rest))) {
      flush();
      out.push({ kind: "link", target: linkTarget(match[2], page), children: parseInline(match[1], page) });
      i += match[0].length;
    } else {
      plainText += text[i];
      i += 1;
    }
  }
  flush();
  return out;
}

const BULLET = /^([-*+]) +/;
const NUMBER = /^(\d+)[.)] +/;
const HEADING = /^(#{1,6}) +(.*?)\s*#*\s*$/;

const indentOf = (line: string) => line.length - line.trimStart().length;
const isTableRow = (line: string) => line.trim().startsWith("|");
const isTableRule = (line: string) => /^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)*\|?\s*$/.test(line);

/** A table row's cells, `\|` kept inside a cell. */
function cells(line: string): string[] {
  const row = line
    .trim()
    .replace(/^\|/, "")
    .replace(/(?<!\\)\|$/, "");
  return row.split(/(?<!\\)\|/).map((cell) => cell.trim());
}

/** `markdown` without HTML comments. */
function uncommented(markdown: string): string {
  return markdown.replace(/<!--[\s\S]*?-->/g, "");
}

/** The blocks of `lines` (already outdented to their container). */
function parseBlocks(lines: string[], page: string, ids: Map<string, number>): Block[] {
  const blocks: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (line.trim() === "") {
      i += 1;
      continue;
    }
    const heading = HEADING.exec(line);
    if (heading) {
      const children = parseInline(heading[2], page);
      const base = slug(plain(children));
      const seen = ids.get(base) ?? 0;
      ids.set(base, seen + 1);
      blocks.push({ kind: "heading", level: heading[1].length, id: seen ? `${base}-${seen}` : base, children });
      i += 1;
      continue;
    }
    if (isTableRow(line) && i + 1 < lines.length && isTableRule(lines[i + 1])) {
      const head = cells(line).map((cell) => parseInline(cell, page));
      i += 2;
      const rows: Inline[][][] = [];
      while (i < lines.length && isTableRow(lines[i])) {
        rows.push(cells(lines[i]).map((cell) => parseInline(cell, page)));
        i += 1;
      }
      blocks.push({ kind: "table", head, rows });
      continue;
    }
    const bullet = BULLET.exec(line);
    const number = NUMBER.exec(line);
    if (bullet || number) {
      const ordered = number !== null;
      const items: Block[][] = [];
      const start = number ? Number(number[1]) : 1;
      while (i < lines.length) {
        const marker = (ordered ? NUMBER : BULLET).exec(lines[i]);
        if (!marker) break;
        const width = marker[0].length;
        const body = [lines[i].slice(width)];
        i += 1;
        // The item's own lines: indented ones, and blank lines followed by one.
        while (i < lines.length) {
          if (lines[i].trim() === "") {
            const next = lines.slice(i).find((candidate) => candidate.trim() !== "");
            if (next === undefined || indentOf(next) < width) break;
            body.push("");
          } else if (indentOf(lines[i]) >= Math.min(width, 2)) {
            body.push(lines[i].slice(Math.min(indentOf(lines[i]), width)));
          } else {
            break;
          }
          i += 1;
        }
        items.push(parseBlocks(body, page, ids));
        while (i < lines.length && lines[i].trim() === "") {
          const next = lines.slice(i).find((candidate) => candidate.trim() !== "");
          if (next === undefined || !(ordered ? NUMBER : BULLET).test(next)) break;
          i += 1;
        }
      }
      blocks.push({ kind: "list", ordered, start, items });
      continue;
    }
    // A paragraph: up to a blank line or the start of another block.
    const text = [line.trim()];
    i += 1;
    while (
      i < lines.length &&
      lines[i].trim() !== "" &&
      !HEADING.test(lines[i]) &&
      !BULLET.test(lines[i]) &&
      !NUMBER.test(lines[i]) &&
      !(isTableRow(lines[i]) && i + 1 < lines.length && isTableRule(lines[i + 1]))
    ) {
      text.push(lines[i].trim());
      i += 1;
    }
    blocks.push({ kind: "paragraph", children: parseInline(text.join(" "), page) });
  }
  return blocks;
}

/** The page `name` from its Markdown. Its title is its first heading's. */
export function parsePage(name: string, markdown: string): Page {
  const lines = uncommented(markdown).replaceAll("\r\n", "\n").split("\n");
  const blocks = parseBlocks(lines, name, new Map());
  const first = blocks.find((block) => block.kind === "heading");
  return { name, title: first?.kind === "heading" ? plain(first.children) : name, blocks };
}

/** The pages in the index's order (the pages it links, then any it
    doesn't), the index first. `files` are the pages' Markdown by name. */
export function guidePages(files: Record<string, string>): Page[] {
  const pages = new Map(Object.entries(files).map(([name, text]) => [name, parsePage(name, text)]));
  const order: string[] = [];
  const add = (name: string) => {
    if (pages.has(name) && !order.includes(name)) order.push(name);
  };
  add(INDEX);
  const visit = (blocks: Block[]) => {
    for (const block of blocks) {
      if (block.kind === "list") block.items.forEach(visit);
      if (block.kind === "paragraph") visitInline(block.children);
    }
  };
  const visitInline = (inlines: Inline[]) => {
    for (const inline of inlines) {
      if (inline.kind === "link" && inline.target.kind === "page") add(inline.target.page);
      if ("children" in inline) visitInline(inline.children);
    }
  };
  const index = pages.get(INDEX);
  if (index) visit(index.blocks);
  [...pages.keys()].sort().forEach(add);
  return order.map((name) => pages.get(name)!);
}
