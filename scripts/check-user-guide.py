#!/usr/bin/env python3
"""Checks that the user guide (docs/user-guide/, PLAN.md Phase 7c D1)
covers the app as it is.

- The index (README.md) links every page of the guide.
- Every sidebar item (Sidebar.svelte's views, items and headings, and the
  built-in library views in library/rules.rs) is named in the guide, as
  en.json words it.
- Every Settings section (SettingsPage.svelte's ALL_SECTIONS) is a
  heading in the Settings chapter.
- Every field of FeatureSettings (settings.rs) has a label in en.json
  (`feature.<field>`, or SWITCH_LABELS) that the guide names.
- Every menu item with a page action (shell/menu.rs's PAGE_ITEMS) is
  named in the guide.
- Every `error.<code>` message in en.json is in the errors appendix,
  each `{placeholder}` written as "…".
- The shortcuts appendix matches the menus and the shortcuts sheet (F6):
  its "Everywhere" table has a row for each menu accelerator (menu.rs,
  under the item's name, with the same keys) and each of `shortcuts()`'s
  pairs, and no other; its "no shortcut" list names each PAGE_ITEMS item
  without one, and only menu items without one; and its other tables are
  ShortcutsDialog.svelte's PAGE sections, row for row. Keys are written as
  the sheet's `keys()` writes them on macOS ("Alt+CmdOrCtrl+R" as "⌥⌘R").
  The keys components handle in their own keydown handlers aren't read:
  the sheet's PAGE list stands for them.

Names are compared as the UI shows them, with runs of white space
(Markdown's line breaks) as one space. The settings reference
(settings-reference.md) is generated and checked by `cargo test`, not
here.

Lists every problem it finds and exits non-zero if there are any. Changes
nothing.

Usage: scripts/check-user-guide.py
"""

import argparse
import json
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
GUIDE = REPO_ROOT / "docs" / "user-guide"
INDEX = "README.md"
SETTINGS_CHAPTER = "11-settings.md"
ERRORS_APPENDIX = "appendix-d-errors.md"
SHORTCUTS_APPENDIX = "appendix-a-shortcuts.md"
APP = REPO_ROOT / "app"
EN_JSON = APP / "src" / "lib" / "i18n" / "en.json"
SIDEBAR = APP / "src" / "lib" / "components" / "Sidebar.svelte"
SETTINGS_PAGE = APP / "src" / "lib" / "components" / "SettingsPage.svelte"
SETTINGS_RS = APP / "src-tauri" / "src" / "settings.rs"
RULES_RS = APP / "src-tauri" / "src" / "library" / "rules.rs"
MENU_RS = APP / "src-tauri" / "src" / "shell" / "menu.rs"
SHORTCUTS_DIALOG = APP / "src" / "lib" / "components" / "ShortcutsDialog.svelte"

# FeatureSettings fields whose label isn't `feature.<field>`.
SWITCH_LABELS = {
    "skipSilence": "features.skipSilence",
    "skipSilenceAfter": "features.skipAfter",
    "listenbrainz": "features.listenBrainz",
    "crossfeed": "features.crossfeed",
    "crossfeedHeadphonesOnly": "features.headphonesOnly",
    "remoteControl": "features.remoteSwitch",
    "remotePort": "features.port",
    "updateCheck": "updates.automatic",
}

# How ShortcutsDialog's `keys()` writes a key on macOS: a symbol, or a
# message from en.json. A key in neither is written as it is ("V", "F10").
KEY_SYMBOLS = {
    "Right": "→",
    "Left": "←",
    "Up": "↑",
    "Down": "↓",
    "Period": ".",
    "Comma": ",",
    "Slash": "/",
}
KEY_MESSAGES = {
    "Space": "shortcuts.space",
    "Escape": "shortcuts.esc",
    "Return": "shortcuts.return",
    "Click": "shortcuts.click",
}
# Its modifiers, in the order macOS writes them.
MODIFIERS = {"Ctrl": "⌃", "Alt": "⌥", "Shift": "⇧", "CmdOrCtrl": "⌘"}


def squeeze(text):
    """`text` with each run of white space as one space."""
    return re.sub(r"\s+", " ", text)


def message(messages, key):
    """The message `key`, a plural's "other" form, or None."""
    value = messages.get(key)
    if isinstance(value, dict):
        value = value.get("other")
    return value


def camel_case(name):
    head, *rest = name.split("_")
    return head + "".join(part.capitalize() for part in rest)


def sidebar_keys(svelte):
    """The message keys of the sidebar's views, items and headings."""
    keys = re.findall(r'name: t\("([^"]+)"\)', svelte)
    keys += re.findall(r'<Icon [^>]*/>\s*\{t\("([^"]+)"\)\}', svelte)
    keys += re.findall(r'<h2>\{t\("([^"]+)"\)\}</h2>', svelte)
    return list(dict.fromkeys(keys))


def default_views(rules_rs):
    """The names of the built-in library views (`default_rules`)."""
    match = re.search(
        r"pub fn default_rules\(\)(.*?)^\s*\}\s*$", rules_rs, re.DOTALL | re.MULTILINE
    )
    if match is None:
        return None
    return re.findall(r'SortRule::new\(\s*"[^"]+",\s*"([^"]+)"', match.group(1))


def settings_sections(svelte):
    """The message keys of the Settings sections' names, in order."""
    match = re.search(r"const ALL_SECTIONS[^=]*=\s*\[(.*?)\];", svelte, re.DOTALL)
    if match is None:
        return None
    return re.findall(r'name: "([^"]+)"', match.group(1))


def feature_fields(settings_rs):
    """FeatureSettings' fields, in camel case."""
    match = re.search(r"pub struct FeatureSettings \{(.*?)\n\}", settings_rs, re.DOTALL)
    if match is None:
        return None
    return [
        camel_case(name) for name in re.findall(r"^\s*pub (\w+):", match.group(1), re.MULTILINE)
    ]


def menu_items(menu_rs):
    """The texts of the menu items the page handles."""
    match = re.search(r"const PAGE_ITEMS[^=]*=\s*\[(.*?)\];", menu_rs, re.DOTALL)
    if match is None:
        return None
    return re.findall(r'\(\s*"[^"]+",\s*"([^"]+)"', match.group(1))


def menu_accelerators(menu_rs):
    """(text, accelerator) of every menu item with a shortcut: PAGE_ITEMS'
    and those built with `.accelerator(…)`; None if PAGE_ITEMS is missing."""
    match = re.search(r"const PAGE_ITEMS[^=]*=\s*\[(.*?)\];", menu_rs, re.DOTALL)
    if match is None:
        return None
    pairs = re.findall(r'\(\s*"[^"]+",\s*"([^"]+)",\s*Some\("([^"]+)"\)', match.group(1))
    for built in re.finditer(
        r'MenuItemBuilder::with_id\(\s*"[^"]+",\s*"([^"]+)"\s*\)(.*?)\.build\(',
        menu_rs,
        re.DOTALL,
    ):
        accelerator = re.search(r'\.accelerator\(\s*"([^"]+)"\s*\)', built.group(2))
        if accelerator:
            pairs.append((built.group(1), accelerator.group(1)))
    return pairs


def menu_names(menu_rs):
    """Every menu item's and submenu's text written in menu.rs."""
    names = re.findall(r'MenuItemBuilder::with_id\(\s*"[^"]+",\s*"([^"]+)"', menu_rs)
    names += re.findall(r'SubmenuBuilder::new\(\s*app,\s*"([^"]+)"', menu_rs)
    match = re.search(r"const PAGE_ITEMS[^=]*=\s*\[(.*?)\];", menu_rs, re.DOTALL)
    if match:
        names += re.findall(r'\(\s*"[^"]+",\s*"([^"]+)"', match.group(1))
    return set(names)


def sheet_menu_shortcuts(menu_rs):
    """`shortcuts()`'s (what, accelerator) pairs, PAGE_ITEMS' included;
    None if it is missing."""
    match = re.search(r"pub fn shortcuts\(\)(.*?)^\}", menu_rs, re.DOTALL | re.MULTILINE)
    if match is None:
        return None
    pairs = re.findall(r'\(\s*"([^"]+)",\s*"([^"]+)"\s*\)', match.group(1))
    if "PAGE_ITEMS" in match.group(1):
        items = re.search(r"const PAGE_ITEMS[^=]*=\s*\[(.*?)\];", menu_rs, re.DOTALL)
        if items:
            pairs += re.findall(r'\(\s*"[^"]+",\s*"([^"]+)",\s*Some\("([^"]+)"\)', items.group(1))
    return pairs


def dialog_key_names(svelte):
    """The keys `keys()`'s `names` map renames; None if it is missing."""
    match = re.search(r"const names[^=]*=\s*\{(.*?)\};", svelte, re.DOTALL)
    if match is None:
        return None
    return re.findall(r"^\s*(\w+):", match.group(1), re.MULTILINE)


def dialog_sections(svelte):
    """ShortcutsDialog's PAGE: [(heading key, [(what key, [accelerators],
    note key or None)])]; None if it is missing."""
    match = re.search(r"const PAGE[^=]*=\s*\[(.*?)^  \];", svelte, re.DOTALL | re.MULTILINE)
    if match is None:
        return None
    sections = []
    for token in re.finditer(
        r'\[ *"([\w.]+)", *\[([^\]\n]*)\](?:, *"([\w.]+)")? *\]|^\s*"([\w.]+)",\s*$',
        match.group(1),
        re.MULTILINE,
    ):
        what, accelerators, note, heading = token.groups()
        if heading:
            sections.append((heading, []))
        elif sections:
            sections[-1][1].append((what, re.findall(r'"([^"]+)"', accelerators), note))
    return sections


def key_text(accelerator, messages):
    """`accelerator` as the sheet writes it on macOS: "⇧⌘O", "⌘-click"."""
    *parts, key = accelerator.split("+")
    if key in KEY_MESSAGES:
        shown = message(messages, KEY_MESSAGES[key]) or key
    else:
        shown = KEY_SYMBOLS.get(key, key)
    order = list(MODIFIERS)
    parts.sort(key=lambda part: order.index(part) if part in order else -1)
    return (
        "".join(MODIFIERS.get(part, part) for part in parts)
        + ("-" if key == "Click" else "")
        + shown
    )


def appendix_tables(text):
    """Each `##` section's table rows (header and rule left out), as lists
    of cells, by heading; and each section's text, by heading."""
    tables, texts = {}, {}
    for section in re.split(r"^## ", text, flags=re.MULTILINE)[1:]:
        heading, _, body = section.partition("\n")
        heading = heading.strip()
        texts[heading] = body
        rows = [
            [squeeze(cell).strip() for cell in line.strip().strip("|").split("|")]
            for line in body.splitlines()
            if line.startswith("|")
        ]
        tables[heading] = [row for row in rows[1:] if not all(set(c) <= {"-"} for c in row)]
    return tables, texts


def menu_shortcut_problems(menu_rs, messages, tables, texts):
    accelerators = menu_accelerators(menu_rs)
    sheet = sheet_menu_shortcuts(menu_rs)
    if accelerators is None or sheet is None:
        return ["menu.rs: PAGE_ITEMS or shortcuts() not found"]
    heading = message(messages, "shortcuts.menus")
    if heading not in tables:
        return [f"{SHORTCUTS_APPENDIX} has no “{heading}” section"]
    rows = [row for row in tables[heading] if len(row) == 3]
    by_item = {}
    for action, menu, keys in rows:
        by_item.setdefault(menu.split("›")[-1].strip(), []).append(keys)
    problems = []
    sheet_keys = {(what, key_text(accelerator, messages)) for what, accelerator in sheet}
    for text, accelerator in accelerators:
        keys = key_text(accelerator, messages)
        if keys not in by_item.get(text, []):
            listed = " or ".join(by_item.get(text, [])) or "nothing"
            problems.append(
                f"the menu item “{text}” is {keys}, but {SHORTCUTS_APPENDIX} lists {listed}"
            )
        if not any(keys == shown for _, shown in sheet_keys):
            problems.append(f"the menu item “{text}” ({keys}) isn't in menu.rs's shortcuts()")
    for what, keys in sorted(sheet_keys):
        if [what, keys] not in [[action, shown] for action, _, shown in rows]:
            problems.append(
                f"the shortcuts sheet's “{what}” ({keys}) isn't in {SHORTCUTS_APPENDIX}"
            )
    names = menu_names(menu_rs)
    for action, menu, keys in rows:
        item = menu.split("›")[-1].strip()
        if item not in names:
            problems.append(f"{SHORTCUTS_APPENDIX} lists “{menu}”, which menu.rs has no item for")
        elif (action, keys) not in sheet_keys:
            problems.append(
                f"{SHORTCUTS_APPENDIX} lists “{action}” as {keys}, which the menus don't have"
            )

    none = re.search(r"have no shortcut:(.*?)(?:\n\n|$)", texts[heading], re.DOTALL)
    unlisted = set(re.findall(r"\*\*(.+?)\*\*", squeeze(none.group(1)))) if none else set()
    items = re.search(r"const PAGE_ITEMS[^=]*=\s*\[(.*?)\];", menu_rs, re.DOTALL)
    for text in re.findall(r'\(\s*"[^"]+",\s*"([^"]+)",\s*None\s*\)', items.group(1)):
        if text not in unlisted:
            problems.append(
                f"the menu item “{text}” has no shortcut, and {SHORTCUTS_APPENDIX} doesn't say so"
            )
    with_keys = {text for text, _ in accelerators}
    for text in sorted(unlisted):
        if text not in names:
            problems.append(
                f"{SHORTCUTS_APPENDIX} says “{text}” has no shortcut, but menu.rs has no such item"
            )
        elif text in with_keys:
            problems.append(f"{SHORTCUTS_APPENDIX} says “{text}” has no shortcut, but it has one")
    return problems


def page_shortcut_problems(svelte, messages, tables):
    sections = dialog_sections(svelte)
    names = dialog_key_names(svelte)
    if sections is None or names is None:
        return ["ShortcutsDialog.svelte: PAGE or keys()'s names not found"]
    problems = [
        f"ShortcutsDialog.svelte's keys() renames {name}, which check-user-guide.py doesn't know"
        for name in names
        if name not in KEY_SYMBOLS and name not in KEY_MESSAGES
    ]
    headings = {message(messages, "shortcuts.menus")}
    for heading_key, rows in sections:
        heading = message(messages, heading_key) or heading_key
        headings.add(heading)
        if heading not in tables:
            problems.append(f"{SHORTCUTS_APPENDIX} has no “{heading}” section")
            continue
        listed = {row[0]: row[-1] for row in tables[heading]}
        expected = set()
        for what_key, accelerators, note_key in rows:
            what = message(messages, what_key) or what_key
            keys = " or ".join(key_text(accelerator, messages) for accelerator in accelerators)
            if note_key:
                keys += " " + (message(messages, note_key) or note_key)
            expected.add(what)
            if what not in listed:
                problems.append(
                    f"the sheet's “{what}” ({keys}) isn't in “{heading}” in {SHORTCUTS_APPENDIX}"
                )
            elif listed[what] != keys:
                problems.append(
                    f"{SHORTCUTS_APPENDIX} lists “{what}” as {listed[what]}, the sheet as {keys}"
                )
        problems += [
            f"{SHORTCUTS_APPENDIX} lists “{what}” in “{heading}”, which the sheet doesn't"
            for what in listed
            if what not in expected
        ]
    problems += [
        f"{SHORTCUTS_APPENDIX}'s table “{heading}” isn't a section of the sheet"
        for heading, rows in tables.items()
        if rows and heading not in headings
    ]
    return problems


def error_entry(text):
    """An error message as the appendix writes it: placeholders as "…"."""
    return re.sub(r"\{[^}]+\}", "…", text)


def index_problems(pages, index_text):
    linked = set(re.findall(r"\]\(([^)#]+\.md)(?:#[^)]*)?\)", index_text))
    return [f"{INDEX} doesn't link {page}" for page in sorted(pages) if page not in linked]


def named_problems(what, names, guide_text):
    text = squeeze(guide_text)
    return [
        f"{what} “{name}” isn't named in the guide" for name in names if squeeze(name) not in text
    ]


def section_problems(names, chapter_text):
    headings = {
        squeeze(h).strip() for h in re.findall(r"^#{2,3} (.+)$", chapter_text, re.MULTILINE)
    }
    return [
        f"the Settings section “{name}” has no heading in {SETTINGS_CHAPTER}"
        for name in names
        if name not in headings
    ]


def feature_problems(fields, messages, guide_text):
    problems = []
    names = []
    for field in fields:
        key = SWITCH_LABELS.get(field, f"feature.{field}")
        label = message(messages, key)
        if label is None:
            problems.append(f"the feature {field} has no label {key} in en.json")
        else:
            names.append(label)
    return problems + named_problems("the feature", names, guide_text)


def error_problems(messages, appendix_text):
    text = squeeze(appendix_text)
    return [
        f"{key} (“{error_entry(message(messages, key))}”) isn't in {ERRORS_APPENDIX}"
        for key in sorted(messages)
        if key.startswith("error.") and squeeze(error_entry(message(messages, key))) not in text
    ]


def problems_in(read):
    """Every problem, reading each source with `read(path)`; None if missing."""
    pages = {path.name: read(path) for path in sorted(GUIDE.glob("*.md"))}
    if INDEX not in pages:
        return [f"docs/user-guide/{INDEX} is missing"]
    guide = "\n".join(text for text in pages.values())
    messages = json.loads(read(EN_JSON))
    problems = index_problems(set(pages) - {INDEX}, pages[INDEX])

    keys = sidebar_keys(read(SIDEBAR))
    problems += [
        f"Sidebar.svelte: en.json has no {key}" for key in keys if not message(messages, key)
    ]
    sidebar = [message(messages, key) for key in keys if message(messages, key)]
    problems += named_problems("the sidebar item", sidebar, guide)
    views = default_views(read(RULES_RS))
    if views is None:
        problems.append("rules.rs: default_rules not found")
    else:
        problems += named_problems("the library view", views, guide)

    sections = settings_sections(read(SETTINGS_PAGE))
    if sections is None:
        problems.append("SettingsPage.svelte: ALL_SECTIONS not found")
    elif SETTINGS_CHAPTER not in pages:
        problems.append(f"docs/user-guide/{SETTINGS_CHAPTER} is missing")
    else:
        names = [message(messages, key) or key for key in sections]
        problems += section_problems(names, pages[SETTINGS_CHAPTER])

    fields = feature_fields(read(SETTINGS_RS))
    if fields is None:
        problems.append("settings.rs: FeatureSettings not found")
    else:
        problems += feature_problems(fields, messages, guide)

    items = menu_items(read(MENU_RS))
    if items is None:
        problems.append("menu.rs: PAGE_ITEMS not found")
    else:
        problems += named_problems("the menu item", items, guide)

    if SHORTCUTS_APPENDIX not in pages:
        problems.append(f"docs/user-guide/{SHORTCUTS_APPENDIX} is missing")
    else:
        tables, texts = appendix_tables(pages[SHORTCUTS_APPENDIX])
        problems += menu_shortcut_problems(read(MENU_RS), messages, tables, texts)
        problems += page_shortcut_problems(read(SHORTCUTS_DIALOG), messages, tables)

    if ERRORS_APPENDIX not in pages:
        problems.append(f"docs/user-guide/{ERRORS_APPENDIX} is missing")
    else:
        problems += error_problems(messages, pages[ERRORS_APPENDIX])
    return problems


def main():
    parser = argparse.ArgumentParser(description="Check the user guide covers the app.")
    parser.parse_args()
    problems = problems_in(lambda path: path.read_text("utf-8"))
    for problem in problems:
        print(f"check-user-guide: {problem}")
    if not problems:
        pages = len(list(GUIDE.glob("*.md")))
        print(
            f"check-user-guide: {pages} pages cover the sidebar, settings, features, shortcuts and errors"
        )
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
