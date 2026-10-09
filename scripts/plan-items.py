#!/usr/bin/env python3
"""Writes docs/plan-items.md, the index of PLAN.md's numbered items (Phase
7c D4): each item's number, name, status and where it is documented.

The items, by series, in plan order:

- O (§4.6's table, Phase 6b), F (§4.7's table, Phase 6c), H (Phase 7's
  table), X (Phase 7b), D (Phase 7c) and N (Phase 7d): a table row whose
  first cell is the number, its name in the second cell;
- X, D and N also have an item line, "- [ ] **X1 Name.** …", and M
  (§9.2) has only its line, "- [x] M1 Name (…): …".
Every item of a series has each of the series' sources.

An item's status is its table's status column where it has one (O's
"Decision", H's "Status"); otherwise its item line: the first clause after
the name, or "Done <date>" for a ticked line that says no more, or "Open"
for an unticked one ("Proposal" for an unticked N item). F's, with neither,
is its record's **Decision** line ("built 2026-09-27").

An item's record is the place in docs/design/ that names it: a heading
("## X1 Themes", with its item line under it) or an unindented item line
("- **H1 …**", "- [x] M1 …"), linked by the heading it is under. An item
whose status says it is done or built must have one; an open item or a
proposal may not have one yet. A record is moved word for word when its
item is finished, so its own tick is history and isn't compared.

Problems, each listed: a number defined twice in the plan or recorded
twice in docs/design/, an item missing its series' table row or item line,
a done item without a record, an item without a status, and an item whose
table status and item line disagree on whether it is done. The page isn't
written while there are any.

Without --check, rewrites docs/plan-items.md. With --check, changes
nothing and exits non-zero on a problem or a stale page.

Usage: scripts/plan-items.py [--check]
"""

import argparse
import collections
import dataclasses
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
PLAN = "PLAN.md"
DESIGN = "docs/design"
PAGE = "docs/plan-items.md"

# Each series in plan order: its letter, what it is, where it lives, and
# the plan's sources every item of it has (a table row, an item line).
SERIES = {
    "O": ("Optional features", "Phase 6b, §4 #6", ("row",)),
    "F": ("Expected features", "Phase 6c, §4 #7", ("row",)),
    "H": ("Hardening", "Phase 7", ("row",)),
    "X": ("Personalisation, effects and discovery", "Phase 7b", ("row", "line")),
    "D": ("Documentation", "Phase 7c", ("row", "line")),
    "N": ("More features, proposed", "Phase 7d", ("row", "line")),
    "M": ("Maintenance scripts", "§9.2", ("line",)),
}
# Table columns shown beside the status, when a series' table has them.
EXTRA_COLUMNS = ("Priority", "Kind")

NUMBER = re.compile(r"^([A-Z])(\d+)$")
HEADING = re.compile(r"^(#{1,6})\s+(.*?)(?:\s+#+)?\s*$")
# An unindented item line: its tick, then its number and name.
ITEM_LINE = re.compile(r"^- (?:\[( |x)\] )?(\*\*)?([A-Z])(\d+) (?=[A-Z(])")
DATE = re.compile(r"\d{4}-\d{2}-\d{2}")
# The words an item line's status clause starts with.
STATUS_WORD = re.compile(r"(Done|Built|Written|Added|Checked|Accepted|Decided|Open)\b")
# A clause ends at a colon, a sentence's end, a parenthesis or a comma.
CLAUSE_END = re.compile(r":|\.(?:\s|$)| \(|,")
DECISION = re.compile(
    r"\*\*Decision:\*\*\s+(?:accepted and )?(built)\s+(\d{4}-\d{2}-\d{2})", re.IGNORECASE
)


@dataclasses.dataclass(frozen=True)
class Place:
    """Where something is in a Markdown file: the file, its line, and the
    heading it is under (its text and anchor)."""

    path: str
    line: int
    heading: str
    anchor: str


@dataclasses.dataclass
class Row:
    """An item's row in a plan table."""

    name: str
    status: str | None
    extra: dict
    place: Place


@dataclasses.dataclass
class Line:
    """An item's line (a list item naming it): its tick (None if it has
    none), its name, the text after the name, and where it is."""

    ticked: bool | None
    name: str
    rest: str
    place: Place


@dataclasses.dataclass
class Item:
    number: str
    name: str
    status: str
    extra: dict
    plan: Place
    record: Place | None


def slug(heading):
    """GitHub's anchor for a heading's text (as check-docs.py makes it)."""
    text = re.sub(r"!?\[([^\]]*)\]\([^)]*\)", r"\1", heading).strip().lower()
    text = re.sub(r"[^\w\- ]", "", text)
    return text.replace(" ", "-")


def sections(path, text):
    """Each line of a Markdown text with the heading it is under: a list of
    (line number, line, Place of its heading), fenced code left out."""
    out, seen, inside = [], collections.Counter(), False
    current = Place(path, 0, "", "")
    for number, line in enumerate(text.split("\n"), 1):
        if line.lstrip().startswith("```"):
            inside = not inside
            continue
        if inside:
            continue
        match = HEADING.match(line)
        if match:
            title = match.group(2)
            base = slug(title)
            anchor = base if seen[base] == 0 else f"{base}-{seen[base]}"
            seen[base] += 1
            current = Place(path, number, title, anchor)
        out.append((number, line, current))
    return out


def cells(line):
    return [cell.strip() for cell in line.strip().strip("|").split("|")]


def number_key(number):
    return (list(SERIES).index(number[0]), int(number[1:]))


def table_rows(path, text):
    """Each item row of the text's tables: a list of (number, Row)."""
    rows, header = [], None
    for number, line, heading in sections(path, text):
        if not line.startswith("|"):
            header = None
            continue
        row = cells(line)
        if header is None:
            header = row
            continue
        match = NUMBER.match(row[0])
        if header[0] != "#" or not match or match.group(1) not in SERIES:
            continue
        columns = dict(zip(header, row))
        status = next(
            (
                value
                for key, value in columns.items()
                if key == "Status" or key.startswith("Decision")
            ),
            None,
        )
        extra = {key: columns[key] for key in EXTRA_COLUMNS if key in columns}
        place = dataclasses.replace(heading, line=number)
        rows.append((row[0], Row(row[1], status, extra, place)))
    return rows


def item_lines(path, text):
    """Each unindented line naming an item, with the lines continuing it:
    a list of (number, Line)."""
    found, lines = [], sections(path, text)
    for index, (number, line, heading) in enumerate(lines):
        match = ITEM_LINE.match(line)
        if not match or match.group(3) not in SERIES:
            continue
        body = [line[match.end() :]]
        for _, more, _ in lines[index + 1 :]:
            if not more.startswith("  "):
                break
            body.append(more.strip())
        text = " ".join(body)
        if match.group(2):
            name, _, rest = text.partition("**")
            name = name.strip().rstrip(".")
        else:
            name = re.split(r" \(|[.:]", text, maxsplit=1)[0]
            rest = text[len(name) :].lstrip(".:")
        ticked = None if match.group(1) is None else match.group(1) == "x"
        place = dataclasses.replace(heading, line=number)
        found.append((match.group(3) + match.group(4), Line(ticked, name, rest.strip(), place)))
    return found


def line_status(rest, ticked):
    """An item line's status: its first clause if that starts with a status
    word; else "Done" (with a date in a leading parenthesis) or "Open"."""
    lead = re.match(r"\(([^)]*)\)[.:]?\s*", rest)
    when = DATE.search(lead.group(1)) if lead else None
    if lead:
        rest = rest[lead.end() :]
    clause = CLAUSE_END.split(rest, maxsplit=1)[0].strip()
    said = clause if STATUS_WORD.match(clause) else None
    if ticked:
        return said or (f"Done {when.group()}" if when else "Done")
    if said is None:
        return "Open"
    if said.startswith("Done"):
        return "Open, part done"
    return "Open: " + said[0].lower() + said[1:]


def is_done(status):
    """Whether a status says done (True), open (False) or neither (None)."""
    if re.match(r"(Done|Built)\b", status):
        return True
    if re.match(r"(Open|Proposal)\b", status):
        return False
    return None


def defined_once(found, where, problems):
    """The found pairs as a dict, a number found twice reported."""
    out = {}
    for number, value in found:
        if number in out:
            problems.append(
                f"{number} is {where} twice: {value.place.path}:{out[number].place.line} "
                f"and :{value.place.line}"
            )
        else:
            out[number] = value
    return out


def record_decision(text, number):
    """The **Decision** line's status in the record's item line at a line
    number, or None."""
    lines = text.split("\n")[number - 1 :]
    body = [lines[0]]
    for more in lines[1:]:
        if not more.startswith("  "):
            break
        body.append(more.strip())
    match = DECISION.search(" ".join(body))
    return f"Built {match.group(2)}" if match else None


def design_records(design_texts):
    """Each item recorded in the design notes, as (number, Line): a heading
    naming it (with the item line under it, if any, folded in) or an item
    line."""
    found = []
    for path, text in sorted(design_texts.items()):
        headed = {}
        for _, line, heading in sections(path, text):
            match = HEADING.match(line)
            words = match.group(2).split(" ", 1) if match else []
            if len(words) == 2 and NUMBER.match(words[0]) and words[0][0] in SERIES:
                headed[words[0]] = heading
                found.append((words[0], Line(None, words[1], "", heading)))
        for number, line in item_lines(path, text):
            if number not in headed or headed[number].anchor != line.place.anchor:
                found.append((number, line))
    return found


def collect(plan_text, design_texts):
    """Every item in plan order, and every problem found."""
    problems = []
    rows = defined_once(table_rows(PLAN, plan_text), "in a plan table", problems)
    lines = defined_once(item_lines(PLAN, plan_text), "an item line in the plan", problems)
    records = defined_once(design_records(design_texts), "recorded in docs/design/", problems)

    for letter, (_, _, sources) in SERIES.items():
        numbers = {n for n in (*rows, *lines) if n[0] == letter}
        if not numbers:
            problems.append(f"no {letter} items found in {PLAN}")
        for source, name, kind in ((rows, "row", "table row"), (lines, "line", "item line")):
            if name in sources:
                for number in sorted(numbers - set(source), key=number_key):
                    problems.append(f"{number} has no {kind} in {PLAN}, as {letter} items do")

    items = []
    for number in sorted({*rows, *lines}, key=number_key):
        row, line, record = rows.get(number), lines.get(number), records.get(number)
        decision = record and record_decision(design_texts[record.place.path], record.place.line)
        if row and row.status:
            status = row.status
        elif line and number[0] == "N" and not line.ticked:
            status = "Proposal"
        elif line and line.ticked is not None:
            status = line_status(line.rest, line.ticked)
        elif decision:
            status = decision
        else:
            problems.append(
                f"{number} has no status in {PLAN} or a **Decision** line in its record"
            )
            continue

        done = is_done(status)
        if line and line.ticked is not None and done is not None and done != line.ticked:
            tick = "ticked" if line.ticked else "unticked"
            problems.append(f"{number}'s table says {status!r} but its item line is {tick}")
        if record is None and (done or (line and line.ticked)):
            problems.append(f"{number} is done ({status}) but nothing in {DESIGN}/ records it")

        name = row.name if row else line.name
        plan = line.place if line and "line" in SERIES[number[0]][2] else row.place
        extra = row.extra if row else {}
        items.append(Item(number, name, status, extra, plan, record and record.place))
    return items, problems


def cell(text):
    return text.replace("|", "\\|")


def link(place, base, named=False):
    """A link to the heading a place is under, its text the heading's (after
    the file's name, if named)."""
    file = place.path.rpartition("/")[2]
    text = f"{file}: {place.heading}" if named else place.heading
    return f"[{text}]({base}{file}#{place.anchor})"


def number_range(items):
    return items[0].number if len(items) == 1 else f"{items[0].number}–{items[-1].number}"


def render(items):
    """docs/plan-items.md's text."""
    by_series = collections.defaultdict(list)
    for item in items:
        by_series[item.number[0]].append(item)
    letters = "; ".join(
        f"{letter}, {what.lower()} ({where})" for letter, (what, where, _) in SERIES.items()
    )
    out = [
        "# Plan items",
        "",
        (
            "Every numbered item of [PLAN.md](../PLAN.md), so a number cited in the "
            "docs, the code or a commit can be looked up here: its name, its status "
            "in the plan's own words (its table's status column where there is one, "
            "otherwise its item line), the plan's section that defines it, and its "
            "record in [`docs/design/`](design/) (its design, steps as built and "
            "known limits) once there is one. The letters are the plan's series: "
            f"{letters}. This page is generated by `scripts/plan-items.py` from the "
            "plan and the design notes, so don't edit it: change the plan, then "
            "rerun the script (`check-all.py --quick` fails while the page is "
            "stale)."
        ),
    ]
    for letter, (what, where, _) in SERIES.items():
        group = by_series[letter]
        if not group:
            continue
        extras = [key for key in EXTRA_COLUMNS if any(key in item.extra for item in group)]
        out += ["", f"## {number_range(group)}: {what} ({where})", ""]
        out.append("| # | " + " | ".join(["Name", *extras, "Status", "Plan", "Record"]) + " |")
        out.append("|---" * (5 + len(extras)) + "|")
        for item in group:
            values = [item.number, item.name, *(item.extra.get(k, "") for k in extras)]
            values += [item.status, link(item.plan, "../")]
            values.append(link(item.record, "design/", named=True) if item.record else "—")
            out.append("| " + " | ".join(cell(value) for value in values) + " |")
    return "\n".join(out) + "\n"


def read_sources(root=REPO_ROOT):
    plan = (root / PLAN).read_text("utf-8")
    design = {
        path.relative_to(root).as_posix(): path.read_text("utf-8")
        for path in sorted((root / DESIGN).glob("*.md"))
    }
    return plan, design


def run(root=REPO_ROOT, check=False, out=print):
    """Checks the plan and writes (or, with check, compares) the page;
    returns the exit code."""
    items, problems = collect(*read_sources(root))
    page = root / PAGE
    if not problems:
        text = render(items)
        current = page.read_text("utf-8") if page.exists() else None
        if text != current:
            if check:
                problems.append(f"{PAGE} is stale: run scripts/plan-items.py")
            else:
                page.write_text(text, "utf-8")
                out(f"plan-items: wrote {PAGE}")
    for problem in problems:
        out(f"plan-items: {problem}")
    if problems:
        return 1
    out(f"plan-items: {len(items)} items indexed")
    return 0


def main():
    parser = argparse.ArgumentParser(description="Write the index of PLAN.md's items.")
    parser.add_argument("--check", action="store_true", help="change nothing; fail on a difference")
    args = parser.parse_args()
    return run(check=args.check)


if __name__ == "__main__":
    sys.exit(main())
