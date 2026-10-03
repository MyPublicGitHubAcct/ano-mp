-- The saved play queue as rows (PLAN.md H16). The `player.queue` setting
-- held the whole list as JSON, rewritten on every change (350 KB for
-- 50,000 tracks). Each row is an item: `uid` is its id in the queue, `ord`
-- places it in the play order and `original`, while shuffled, in the order
-- before shuffling (NULL otherwise). Both are sparse keys, so adding or
-- moving items writes only their rows. No foreign key to `tracks`: the
-- queue leaves out a track the library lost when it next loads.
CREATE TABLE queue_items (
    uid INTEGER PRIMARY KEY,
    track_id INTEGER NOT NULL,
    ord REAL NOT NULL,
    original REAL
);

CREATE INDEX queue_items_ord ON queue_items (ord);

-- The list moves out of the setting, once. What stays there is small: the
-- current item (an index in the play order), the position, repeat, the
-- volume, and now whether shuffle is on (an empty queue can be shuffled).
INSERT INTO queue_items (uid, track_id, ord)
SELECT t.key + 1, t.value, t.key
FROM settings s, json_each(s.value, '$.tracks') t
WHERE s.key = 'player.queue' AND json_valid(s.value) AND t.type = 'integer';

-- `original` listed indices into `tracks` in their order before shuffling.
UPDATE queue_items SET original = o.rank
FROM (
    SELECT o.key AS rank, o.value AS item
    FROM settings s, json_each(s.value, '$.original') o
    WHERE s.key = 'player.queue' AND json_valid(s.value) AND o.type = 'integer'
) AS o
WHERE queue_items.uid = o.item + 1;

UPDATE settings
SET value = json_set(
    json_remove(value, '$.tracks', '$.original'),
    '$.shuffled',
    json(CASE WHEN json_type(value, '$.original') = 'array' THEN 'true' ELSE 'false' END)
)
WHERE key = 'player.queue' AND json_valid(value);
