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
