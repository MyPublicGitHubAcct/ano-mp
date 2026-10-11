import pytest


@pytest.fixture
def guide(script):
    return script("check-user-guide")


def test_sidebar_keys(guide):
    svelte = """
      { id: "home" as const, name: t("sidebar.home"), icon: "home" as const, on: true },
      <span class="with-icon"><Icon name="wave" size="1.1rem" /> {t("sidebar.visualizer")}</span>
        <Icon name="chevron" size="0.9rem" />
        {t("library.title")}
      <h2>{t("sidebar.folders")}</h2>
      <button title={t("sidebar.addFolder")}>
    """
    assert guide.sidebar_keys(svelte) == [
        "sidebar.home",
        "sidebar.visualizer",
        "library.title",
        "sidebar.folders",
    ]


def test_default_views_are_only_the_built_in_ones(guide):
    rules = """
    pub fn default_rules() -> Vec<SortRule> {
        vec![
            SortRule::new("album-artist", "Album artist", &[AlbumArtist, Album], &order),
            SortRule::new(
                "genre",
                "Genre",
                &[Genre],
                &order,
            ),
        ]
    }

    fn test_rule() -> SortRule {
        SortRule::new("songs", "Songs", &[], &[])
    }
    """
    assert guide.default_views(rules) == ["Album artist", "Genre"]
    assert guide.default_views("fn other() {}") is None


def test_settings_sections(guide):
    svelte = """
  const ALL_SECTIONS: { id: SettingsSection; name: MessageKey; about: MessageKey }[] = [
    { id: "general", name: "settings.general", about: "settings.generalAbout" },
    { id: "sources", name: "settings.sources", about: "settings.sourcesAbout" },
  ];
    """
    assert guide.settings_sections(svelte) == ["settings.general", "settings.sources"]
    assert guide.settings_sections("<script></script>") is None


def test_feature_fields(guide):
    rust = """
pub struct FeatureSettings {
    /// O1: measures loudness.
    pub loudness_analysis: bool,
    pub remote_port: u16,
}
    """
    assert guide.feature_fields(rust) == ["loudnessAnalysis", "remotePort"]
    assert guide.feature_fields("struct Other {}") is None


def test_menu_items(guide):
    rust = """
const PAGE_ITEMS: [(&str, &str, Option<&str>); 2] = [
    ("settings", "Settings…", Some("CmdOrCtrl+,")),
    (
        "add-folder",
        "Add Folder to Library…",
        Some("CmdOrCtrl+Shift+O"),
    ),
];
    """
    assert guide.menu_items(rust) == ["Settings…", "Add Folder to Library…"]


def test_names_are_found_across_line_breaks(guide):
    text = "Turn on **Keep segues\ntogether in shuffle** first."
    assert guide.named_problems("the feature", ["Keep segues together in shuffle"], text) == []
    assert guide.named_problems("the feature", ["Lyrics"], text) == [
        "the feature “Lyrics” isn't named in the guide"
    ]


def test_sections_must_be_headings(guide):
    chapter = "# Settings\n\n## General\n\nThe Library section…\n\n### Online sources\n"
    assert guide.section_problems(["General", "Online sources", "Library"], chapter) == [
        "the Settings section “Library” has no heading in 11-settings.md"
    ]


def test_features_need_a_label(guide):
    messages = {"feature.lyrics": "Lyrics", "features.port": "Port"}
    assert guide.feature_problems(["lyrics", "remotePort"], messages, "Lyrics and Port") == []
    assert guide.feature_problems(["newThing"], messages, "") == [
        "the feature newThing has no label feature.newThing in en.json"
    ]


def test_errors_are_matched_with_placeholders_as_ellipses(guide):
    messages = {
        "error.openTimedOut": "The file took more than {seconds} s to open",
        "error.notSmart": "Not a smart playlist",
        "error.counted": {"one": "{count} thing", "other": "{count} things"},
        "queue.title": "Queue",
    }
    appendix = "**The file took more than … s to\nopen**\n\n**… things**\n"
    assert guide.error_problems(messages, appendix) == [
        "error.notSmart (“Not a smart playlist”) isn't in appendix-d-errors.md"
    ]


def test_the_index_links_every_page(guide):
    index = "1. [Start](01-start.md)\n- [Errors](appendix-d-errors.md#folders)\n"
    pages = {"01-start.md", "02-more.md", "appendix-d-errors.md"}
    assert guide.index_problems(pages, index) == ["README.md doesn't link 02-more.md"]


def test_the_real_guide_covers_the_app(guide):
    assert guide.problems_in(lambda path: path.read_text("utf-8")) == []


SHORTCUT_MESSAGES = {
    "shortcuts.menus": "Everywhere",
    "shortcuts.space": "Space",
    "shortcuts.esc": "Esc",
    "shortcuts.return": "Return",
    "shortcuts.click": "click",
    "shortcuts.lists": "In lists",
    "shortcuts.select": "Select more",
    "shortcuts.menu": "Open the menu",
    "shortcuts.orRightClick": "(or right-click)",
}

MENU_RS = """
const PAGE_ITEMS: [(&str, &str, Option<&str>); 3] = [
    ("settings", "Settings…", Some("CmdOrCtrl+,")),
    ("import-playlist", "Import Playlist…", None),
    (
        "add-folder",
        "Add Folder to Library…",
        Some("CmdOrCtrl+Shift+O"),
    ),
];
    let record = CheckMenuItemBuilder::with_id("record", "Record")
        .accelerator("Alt+CmdOrCtrl+R")
        .enabled(crate::settings::current(app).features.recording)
        .build(app)?;
    let shuffle = CheckMenuItemBuilder::with_id("shuffle", "Shuffle").build(app)?;
    let play = MenuItemBuilder::with_id("play", "Play").build(app)?;
    let controls = SubmenuBuilder::new(app, "Controls");

pub fn shortcuts() -> Vec<(&'static str, &'static str)> {
    let mut list: Vec<(&str, &str)> = vec![
        ("Play or pause", "Space"),
        ("Record", "Alt+CmdOrCtrl+R"),
    ];
    list.extend(PAGE_ITEMS.iter().filter_map(|(_, text, accelerator)| None));
    list
}
"""

APPENDIX = """# Appendix A

## Everywhere

| Action | Menu | Keys |
|---|---|---|
| Play or pause | Controls › Play | Space |
| Record | Controls › Record | ⌥⌘R |
| Settings… | ano-mp › Settings… | ⌘, |
| Add Folder to Library… | File › Add Folder to Library… | ⇧⌘O |

These menu items have no shortcut: **Import Playlist…** and
**Shuffle**.

## In lists

| Action | Keys |
|---|---|
| Select more | ⇧↑ or ⇧↓ |
| Open the menu | ⇧F10 (or right-click) |
"""

DIALOG = """
    const names: Record<string, string> = {
      Right: "→",
      Space: t("shortcuts.space"),
      Return: mac ? t("shortcuts.return") : t("shortcuts.enter"),
    };
  const PAGE: [MessageKey, Row[]][] = [
    [
      "shortcuts.lists",
      [
        ["shortcuts.select", ["Shift+Up", "Shift+Down"]],
        ["shortcuts.menu", ["Shift+F10"], "shortcuts.orRightClick"],
      ],
    ],
  ];
"""


def test_keys_are_written_as_the_sheet_writes_them_on_macos(guide):
    def keys(accelerator):
        return guide.key_text(accelerator, SHORTCUT_MESSAGES)

    assert keys("CmdOrCtrl+Shift+O") == "⇧⌘O"
    assert keys("Alt+CmdOrCtrl+Period") == "⌥⌘."
    assert keys("Shift+Alt+Ctrl+CmdOrCtrl+Right") == "⌃⌥⇧⌘→"
    assert keys("CmdOrCtrl+Click") == "⌘-click"
    assert keys("Space") == "Space"
    assert keys("Escape") == "Esc"
    assert keys("Shift+V") == "⇧V"


def test_menu_accelerators_come_from_page_items_and_builders(guide):
    assert guide.menu_accelerators(MENU_RS) == [
        ("Settings…", "CmdOrCtrl+,"),
        ("Add Folder to Library…", "CmdOrCtrl+Shift+O"),
        ("Record", "Alt+CmdOrCtrl+R"),
    ]
    assert guide.menu_accelerators("fn other() {}") is None
    assert {"Record", "Shuffle", "Play", "Controls", "Import Playlist…"} <= guide.menu_names(
        MENU_RS
    )


def test_the_sheets_menu_shortcuts_include_page_items(guide):
    assert guide.sheet_menu_shortcuts(MENU_RS) == [
        ("Play or pause", "Space"),
        ("Record", "Alt+CmdOrCtrl+R"),
        ("Settings…", "CmdOrCtrl+,"),
        ("Add Folder to Library…", "CmdOrCtrl+Shift+O"),
    ]


def test_dialog_sections(guide):
    assert guide.dialog_sections(DIALOG) == [
        (
            "shortcuts.lists",
            [
                ("shortcuts.select", ["Shift+Up", "Shift+Down"], None),
                ("shortcuts.menu", ["Shift+F10"], "shortcuts.orRightClick"),
            ],
        )
    ]
    assert guide.dialog_key_names(DIALOG) == ["Right", "Space", "Return"]


def test_matching_shortcuts_have_no_problems(guide):
    tables, texts = guide.appendix_tables(APPENDIX)
    assert tables["In lists"] == [
        ["Select more", "⇧↑ or ⇧↓"],
        ["Open the menu", "⇧F10 (or right-click)"],
    ]
    assert guide.menu_shortcut_problems(MENU_RS, SHORTCUT_MESSAGES, tables, texts) == []
    assert guide.page_shortcut_problems(DIALOG, SHORTCUT_MESSAGES, tables) == []


def test_a_changed_or_removed_menu_shortcut_is_found(guide):
    def problems(menu_rs):
        tables, texts = guide.appendix_tables(APPENDIX)
        return guide.menu_shortcut_problems(menu_rs, SHORTCUT_MESSAGES, tables, texts)

    changed = MENU_RS.replace('"Alt+CmdOrCtrl+R"', '"Shift+CmdOrCtrl+R"')
    assert problems(changed) == [
        "the menu item “Record” is ⇧⌘R, but appendix-a-shortcuts.md lists ⌥⌘R",
        "the shortcuts sheet's “Record” (⇧⌘R) isn't in appendix-a-shortcuts.md",
        "appendix-a-shortcuts.md lists “Record” as ⌥⌘R, which the menus don't have",
    ]
    removed = MENU_RS.replace('Some("CmdOrCtrl+,")', "None")
    assert problems(removed) == [
        "appendix-a-shortcuts.md lists “Settings…” as ⌘,, which the menus don't have",
        "the menu item “Settings…” has no shortcut, and appendix-a-shortcuts.md doesn't say so",
    ]
    added = MENU_RS.replace(
        '"Shuffle").build', '"Shuffle")\n        .accelerator("CmdOrCtrl+S")\n        .build'
    )
    assert problems(added) == [
        "the menu item “Shuffle” is ⌘S, but appendix-a-shortcuts.md lists nothing",
        "the menu item “Shuffle” (⌘S) isn't in menu.rs's shortcuts()",
        "appendix-a-shortcuts.md says “Shuffle” has no shortcut, but it has one",
    ]


def test_the_page_keys_must_match_the_sheet_both_ways(guide):
    tables, _ = guide.appendix_tables(APPENDIX)
    changed = DIALOG.replace('["Shift+F10"]', '["F10"]').replace(
        '["shortcuts.select", ["Shift+Up", "Shift+Down"]],\n', ""
    )
    assert guide.page_shortcut_problems(changed, SHORTCUT_MESSAGES, tables) == [
        (
            "appendix-a-shortcuts.md lists “Open the menu” as ⇧F10 (or right-click), "
            "the sheet as F10 (or right-click)"
        ),
        "appendix-a-shortcuts.md lists “Select more” in “In lists”, which the sheet doesn't",
    ]
    unknown = DIALOG.replace('Right: "→",', 'Right: "→",\n      Tab: "⇥",')
    assert guide.page_shortcut_problems(unknown, SHORTCUT_MESSAGES, tables) == [
        "keys.ts's keys() renames Tab, which check-user-guide.py doesn't know"
    ]
