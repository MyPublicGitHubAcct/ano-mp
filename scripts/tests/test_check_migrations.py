import pytest


@pytest.fixture
def migrations(script):
    return script("check-migrations")


def test_numbering(migrations):
    assert migrations.numbering_problems(["001_a.sql", "002_b.sql"]) == []
    assert migrations.numbering_problems(["001_a.sql", "003_c.sql"]) == [
        "migration 002 missing or repeated (found 003)"
    ]
    assert migrations.numbering_problems(["001_a.sql", "001_b.sql"]) == [
        "migration 002 missing or repeated (found 001)"
    ]
    assert migrations.numbering_problems(["002_b.sql"]) == [
        "migration 001 missing or repeated (found 002)"
    ]
    assert migrations.numbering_problems(["001_a.sql", "2_Bad.sql"]) == [
        "2_Bad.sql: not named NNN_lower_case.sql"
    ]


def test_listed_migrations(migrations):
    text = """
    const MIGRATIONS: &[&str] = &[
        include_str!("migrations/001_a.sql"),
        include_str!("migrations/002_b.sql"),
    ];
    """
    assert migrations.listed_migrations(text) == ["001_a.sql", "002_b.sql"]
    assert migrations.listed_migrations("fn main() {}") is None


def test_listing_problems(migrations):
    names = ["001_a.sql", "002_b.sql"]
    assert migrations.listing_problems(names, ["001_a.sql", "002_b.sql"]) == []
    assert migrations.listing_problems(names, ["002_b.sql", "001_a.sql"]) == [
        "MIGRATIONS is not in numeric order"
    ]
    assert migrations.listing_problems(names, ["001_a.sql", "003_c.sql"]) == [
        "002_b.sql: not listed in MIGRATIONS",
        "003_c.sql: listed in MIGRATIONS but doesn't exist",
    ]
    assert migrations.listing_problems(names, ["001_a.sql", "001_a.sql", "002_b.sql"]) == [
        "001_a.sql: listed more than once in MIGRATIONS"
    ]
    assert migrations.listing_problems(names, None) == ["db.rs: MIGRATIONS not found"]


def test_fts_warnings(migrations):
    alter = "ALTER TABLE tracks ADD COLUMN mood TEXT;"
    with_trigger = alter + "\nDROP TRIGGER tracks_search_update;"
    acknowledged = "-- fts: mood isn't indexed.\n" + alter
    rebuild = "CREATE TABLE albums_new (id INTEGER);"
    unrelated = "ALTER TABLE playlists ADD COLUMN colour TEXT;"
    warnings = migrations.fts_warnings(
        {
            "003_old.sql": alter,
            "010_alter.sql": alter,
            "011_trigger.sql": with_trigger,
            "012_ack.sql": acknowledged,
            "013_rebuild.sql": rebuild,
            "014_other.sql": unrelated,
        },
        reviewed_up_to=9,
    )
    assert [w.split(":")[0] for w in warnings] == ["010_alter.sql", "013_rebuild.sql"]


def test_shipped_changes_names_each_file(migrations, monkeypatch):
    monkeypatch.setattr(
        migrations,
        "git",
        lambda *args: "app/src-tauri/src/library/migrations/002_b.sql\n",
    )
    assert migrations.shipped_changes("v0.1.0") == [
        "002_b.sql: changed since v0.1.0, where it had shipped"
    ]


def test_real_tree_passes(migrations):
    files = {path.name: path for path in migrations.MIGRATIONS_DIR.glob("*.sql")}
    listed = migrations.listed_migrations(migrations.DB_RS.read_text("utf-8"))
    assert len(files) >= 9
    assert migrations.numbering_problems(files) == []
    assert migrations.listing_problems(list(files), listed) == []
    sql = {name: path.read_text("utf-8") for name, path in files.items()}
    assert migrations.fts_warnings(sql) == []
