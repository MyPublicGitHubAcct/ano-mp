-- Recommendations from outside the library (PLAN.md X5) the user said no
-- to, by MusicBrainz artist id, so they aren't suggested again. The name
-- is as it was suggested, for Settings to list.
CREATE TABLE outside_dismissed (
    musicbrainz_id TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    dismissed_at   INTEGER NOT NULL
) STRICT;
