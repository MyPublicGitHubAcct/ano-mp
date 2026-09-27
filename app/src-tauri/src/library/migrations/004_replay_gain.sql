-- ReplayGain tags (PLAN.md Phase 6): gains in dB, peaks linear (1 is full
-- scale); NULL when the file has none. Opus R128 gains are stored converted
-- to ReplayGain's reference level.
ALTER TABLE tracks ADD COLUMN replaygain_track_gain REAL;
ALTER TABLE tracks ADD COLUMN replaygain_track_peak REAL;
ALTER TABLE tracks ADD COLUMN replaygain_album_gain REAL;
ALTER TABLE tracks ADD COLUMN replaygain_album_peak REAL;

-- Files scanned before this don't have them read yet: the next scan
-- re-reads every file, since no size or modification time matches -1.
UPDATE tracks SET file_mtime_ns = -1;
