-- What the optional features keep (PLAN.md §4.6). The app never writes
-- the user's files: everything it learns or is told goes here, keyed by
-- track, album or artist id, which rescans keep.

-- One pass over each track's audio (O1–O4, library/analysis.rs). A row
-- is for the file as it was: a different size or modification time means
-- the track is analysed again.
CREATE TABLE track_analysis (
    track_id         INTEGER PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    file_size        INTEGER NOT NULL,
    file_mtime_ns    INTEGER NOT NULL,
    analysed_at      INTEGER NOT NULL,
    -- Why the file couldn't be decoded; the measurements are NULL then.
    error            TEXT,
    -- Seconds decoded.
    duration         REAL,
    -- EBU R128 integrated loudness in LUFS; NULL for digital silence.
    loudness         REAL,
    -- Linear, 1 is full scale.
    sample_peak      REAL,
    true_peak        REAL,
    -- How many 400 ms blocks fell in each 0.5 LU from -70 LUFS: 16-bit
    -- little-endian counts, trailing empty bins left off. Albums are gated
    -- over their tracks' blocks from these.
    histogram        BLOB,
    -- Seconds below -60 dBFS at the start and the end, and the longest
    -- silence inside (a hidden track's gap).
    leading_silence  REAL,
    trailing_silence REAL,
    gap_start        REAL,
    gap_length       REAL,
    -- dBFS RMS of the first and last 50 ms; NULL for digital silence.
    start_level      REAL,
    end_level        REAL,
    -- Where the spectrum stops, as a lossy encoder leaves it; NULL if it doesn't.
    cutoff_hz        REAL,
    -- The waveform for the seek bar: a (min, max) pair of signed bytes
    -- (-127..127 for -1..1) per slice.
    envelope         BLOB
) STRICT;

-- An album's loudness, gated over all of its analysed tracks, and its
-- largest true peak.
CREATE TABLE album_analysis (
    album_id INTEGER PRIMARY KEY REFERENCES albums (id) ON DELETE CASCADE,
    loudness REAL,
    peak     REAL,
    -- How many of its tracks it covers.
    tracks   INTEGER NOT NULL
) STRICT;

-- Listening history (O8): a row per play that counted (half the track, or
-- four minutes).
CREATE TABLE plays (
    id        INTEGER PRIMARY KEY,
    track_id  INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    -- Unix seconds when the track started.
    played_at INTEGER NOT NULL,
    -- How long it was listened to, in seconds.
    seconds   REAL NOT NULL
) STRICT;

CREATE INDEX plays_track ON plays (track_id);
CREATE INDEX plays_time ON plays (played_at);

-- Listens waiting to be sent to ListenBrainz while it can't be reached, as
-- the JSON it takes.
CREATE TABLE listens_pending (
    id        INTEGER PRIMARY KEY,
    listen    TEXT NOT NULL,
    queued_at INTEGER NOT NULL
) STRICT;

-- The user's own rules for playing a track or an album (O7). NULL leaves a
-- value to the album (for a track) or the default. Trims are seconds off
-- the start and the end.
CREATE TABLE track_prefs (
    track_id    INTEGER PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    skip        INTEGER,
    gain_offset REAL,
    trim_start  REAL,
    trim_end    REAL
) STRICT;

CREATE TABLE album_prefs (
    album_id      INTEGER PRIMARY KEY REFERENCES albums (id) ON DELETE CASCADE,
    skip          INTEGER,
    never_shuffle INTEGER,
    gain_offset   REAL
) STRICT;

-- Phones paired with the LAN remote (O14): only a hash of each device's
-- token is kept.
CREATE TABLE remote_devices (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    token_hash BLOB NOT NULL UNIQUE,
    paired_at  INTEGER NOT NULL,
    last_seen  INTEGER
) STRICT;
