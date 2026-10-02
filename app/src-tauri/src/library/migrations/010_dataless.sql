-- Cloud placeholders (PLAN.md H12): with "Optimize Mac Storage", iCloud
-- Drive keeps dataless files whose contents download only when read. A
-- track whose file was a placeholder when last seen (by a scan, which
-- records it without reading it, or by the analysis, which leaves it) is
-- marked here, and every scan checks it again, reading its tags once the
-- file has been downloaded (which changes neither its size nor its time).

-- fts: no search index covers the new column, so the triggers stay as they are.
ALTER TABLE tracks ADD COLUMN dataless INTEGER NOT NULL DEFAULT 0;
