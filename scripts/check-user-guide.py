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
APP = REPO_ROOT / "app"
EN_JSON = APP / "src" / "lib" / "i18n" / "en.json"
SIDEBAR = APP / "src" / "lib" / "components" / "Sidebar.svelte"
SETTINGS_PAGE = APP / "src" / "lib" / "components" / "SettingsPage.svelte"
SETTINGS_RS = APP / "src-tauri" / "src" / "settings.rs"
RULES_RS = APP / "src-tauri" / "src" / "library" / "rules.rs"
MENU_RS = APP / "src-tauri" / "src" / "shell" / "menu.rs"

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
        print(f"check-user-guide: {pages} pages cover the sidebar, settings, features and errors")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
