-- Cover thumbnails (PLAN.md H17) are files in the image cache, named by the
-- SHA-256 of the picture they were made from. This index says which picture
-- an album (`album-<id>`) or a track (`track-<id>`) shows, so a launch finds
-- its thumbnails without reading the picture again. `origin` (JSON) says
-- where the picture came from and `stamp` what that was like then: a file's
-- size and time and its folder's time (a cover added to the folder changes
-- it), or a download's URL. A stale stamp means looking the picture up
-- again. A cover chosen or downloaded, or the sources changed, deletes rows.
CREATE TABLE art_thumbs (
    key TEXT PRIMARY KEY,
    origin TEXT NOT NULL,
    stamp TEXT NOT NULL,
    hash TEXT NOT NULL
);
