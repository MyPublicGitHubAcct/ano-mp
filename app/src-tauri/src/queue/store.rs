//! The saved queue's list, as rows of `queue_items` (migration 011,
//! PLAN.md H16), kept in step with the queue by applying the same edits it
//! sends the frontend (`model::Edit`), so a change writes only the rows it
//! touches.
//!
//! Each row's `ord` places it in the play order, and `original`, while
//! shuffled, in the order before shuffling. Both are sparse: new rows take
//! keys between their neighbours'. When two neighbours' keys get too close
//! to split, every key of that order is written again (`renumber`), which
//! takes many edits at one place to need.
//!
//! The store mirrors the list's uids and keys in memory, files opened from
//! outside the library included (F5): they take a place in the order but
//! are never written, since the OS lets the app open them only until it
//! quits.

use std::collections::HashMap;

use rusqlite::{params, Connection};

use super::model::{Edit, Item, Uid};
use crate::library::Error;

/// Keys closer than this are renumbered before splitting.
const MIN_GAP: f64 = 1e-9;

/// One item as the store knows it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Entry {
    uid: Uid,
    /// Saved: a library track, not a file opened from outside it.
    kept: bool,
    ord: f64,
}

/// What the store reads back at launch.
#[derive(Debug, Default, PartialEq)]
pub struct Loaded {
    /// (uid, track id), in play order.
    pub items: Vec<(Uid, i64)>,
    /// The uids in their order before shuffling, if any row has one.
    pub original: Option<Vec<Uid>>,
}

#[derive(Debug, Default)]
pub struct Store {
    play: Vec<Entry>,
    /// While shuffled: the kept items' original keys, in original order.
    original: Option<Vec<(Uid, f64)>>,
}

impl Store {
    /// The saved list, and a store mirroring it.
    pub fn load(conn: &Connection) -> Result<(Store, Loaded), Error> {
        let mut statement =
            conn.prepare("SELECT uid, track_id, ord, original FROM queue_items ORDER BY ord, uid")?;
        let rows: Vec<(Uid, i64, f64, Option<f64>)> = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)? as Uid,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                ))
            })?
            .collect::<Result<_, _>>()?;
        let mut original: Vec<(Uid, f64)> = rows
            .iter()
            .filter_map(|&(uid, _, _, original)| original.map(|key| (uid, key)))
            .collect();
        original.sort_by(|a, b| a.1.total_cmp(&b.1));
        let store = Store {
            play: rows
                .iter()
                .map(|&(uid, _, ord, _)| Entry {
                    uid,
                    kept: true,
                    ord,
                })
                .collect(),
            original: (!original.is_empty()).then(|| original.clone()),
        };
        let loaded = Loaded {
            items: rows
                .iter()
                .map(|&(uid, track, _, _)| (uid, track))
                .collect(),
            original: (!original.is_empty()).then(|| original.iter().map(|o| o.0).collect()),
        };
        Ok((store, loaded))
    }

    /// Rows whose uid the queue didn't keep (their track left the library
    /// since): deleted, so the rows match the queue again.
    pub fn retain(&mut self, conn: &Connection, kept: &[Uid]) -> Result<(), Error> {
        let keep: std::collections::HashSet<Uid> = kept.iter().copied().collect();
        let gone: Vec<Uid> = self
            .play
            .iter()
            .map(|entry| entry.uid)
            .filter(|uid| !keep.contains(uid))
            .collect();
        if gone.is_empty() {
            return Ok(());
        }
        delete(conn, &gone)?;
        self.play.retain(|entry| keep.contains(&entry.uid));
        if let Some(original) = &mut self.original {
            original.retain(|(uid, _)| keep.contains(uid));
        }
        Ok(())
    }

    /// Writes the whole list: after it was replaced, cleared, shuffled or
    /// put back in order. `original` is the order before shuffling.
    pub fn reset(
        &mut self,
        conn: &Connection,
        items: &[Item],
        original: Option<&[Uid]>,
    ) -> Result<(), Error> {
        self.play = items
            .iter()
            .enumerate()
            .map(|(index, item)| Entry {
                uid: item.uid,
                kept: !item.track.external,
                ord: index as f64,
            })
            .collect();
        let kept: std::collections::HashSet<Uid> = self
            .play
            .iter()
            .filter(|entry| entry.kept)
            .map(|entry| entry.uid)
            .collect();
        self.original = original.map(|uids| {
            uids.iter()
                .filter(|uid| kept.contains(uid))
                .enumerate()
                .map(|(rank, &uid)| (uid, rank as f64))
                .collect()
        });
        let ranks: HashMap<Uid, f64> = self.original.iter().flatten().copied().collect();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM queue_items", [])?;
        {
            let mut insert = tx.prepare_cached(
                "INSERT INTO queue_items (uid, track_id, ord, original) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (entry, item) in self.play.iter().zip(items) {
                if entry.kept {
                    insert.execute(params![
                        entry.uid as i64,
                        item.track.track_id,
                        entry.ord,
                        ranks.get(&entry.uid)
                    ])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Applies the queue's edits, in order, in one transaction.
    pub fn apply(&mut self, conn: &Connection, edits: &[Edit]) -> Result<(), Error> {
        let tx = conn.unchecked_transaction()?;
        // Original keys of items removed in this batch: a block moved by
        // `move_items` comes back as an insert, and keeps its place in the
        // original order.
        let mut removed: HashMap<Uid, f64> = HashMap::new();
        for edit in edits {
            match edit {
                Edit::Insert {
                    at,
                    items,
                    original_at,
                } => self.insert(&tx, *at, items, *original_at, &mut removed)?,
                Edit::Remove { at, count } => {
                    let gone: Vec<Entry> = self.play.drain(*at..*at + *count).collect();
                    let uids: Vec<Uid> = gone.iter().filter(|e| e.kept).map(|e| e.uid).collect();
                    delete(&tx, &uids)?;
                    if let Some(original) = &mut self.original {
                        let set: std::collections::HashSet<Uid> = uids.iter().copied().collect();
                        original.retain(|&(uid, key)| {
                            if set.contains(&uid) {
                                removed.insert(uid, key);
                                false
                            } else {
                                true
                            }
                        });
                    }
                }
                Edit::Move { from, count, to } => {
                    let moving: Vec<Entry> = self.play.drain(*from..*from + *count).collect();
                    let keys = self.keys_at(&tx, *to, moving.len())?;
                    let mut update =
                        tx.prepare_cached("UPDATE queue_items SET ord = ?2 WHERE uid = ?1")?;
                    let moved: Vec<Entry> = moving
                        .into_iter()
                        .zip(keys)
                        .map(|(entry, ord)| Entry { ord, ..entry })
                        .collect();
                    for entry in &moved {
                        if entry.kept {
                            update.execute(params![entry.uid as i64, entry.ord])?;
                        }
                    }
                    self.play.splice(*to..*to, moved);
                }
                // Tags changed; the rows hold only track ids.
                Edit::Update { .. } => {}
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn insert(
        &mut self,
        conn: &Connection,
        at: usize,
        items: &[Item],
        original_at: Option<usize>,
        removed: &mut HashMap<Uid, f64>,
    ) -> Result<(), Error> {
        let keys = self.keys_at(conn, at, items.len())?;
        // Where new items go in the original order: at `original_at` among
        // the queue's items, which the store counts among its kept ones.
        let mut original_keys: HashMap<Uid, f64> = HashMap::new();
        if let Some(original) = &self.original {
            let fresh: Vec<&Item> = items
                .iter()
                .filter(|item| !item.track.external && !removed.contains_key(&item.uid))
                .collect();
            if !fresh.is_empty() {
                let at = original_at.unwrap_or(original.len()).min(original.len());
                let ranks = self.original_keys_at(conn, at, fresh.len())?;
                original_keys.extend(fresh.iter().map(|item| item.uid).zip(ranks));
            }
            for item in items {
                if let Some(key) = removed.remove(&item.uid) {
                    original_keys.insert(item.uid, key);
                }
            }
        }
        let entries: Vec<Entry> = items
            .iter()
            .zip(keys)
            .map(|(item, ord)| Entry {
                uid: item.uid,
                kept: !item.track.external,
                ord,
            })
            .collect();
        {
            let mut insert = conn.prepare_cached(
                "INSERT OR REPLACE INTO queue_items (uid, track_id, ord, original)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (entry, item) in entries.iter().zip(items) {
                if entry.kept {
                    insert.execute(params![
                        entry.uid as i64,
                        item.track.track_id,
                        entry.ord,
                        original_keys.get(&entry.uid)
                    ])?;
                }
            }
        }
        if let Some(original) = &mut self.original {
            for (&uid, &key) in &original_keys {
                let place = original.partition_point(|&(_, k)| k < key);
                original.insert(place, (uid, key));
            }
        }
        self.play.splice(at..at, entries);
        Ok(())
    }

    /// `count` keys for items going in before index `at` of the play order
    /// (after `play[at - 1]`), renumbering it first if they don't fit.
    fn keys_at(&mut self, conn: &Connection, at: usize, count: usize) -> Result<Vec<f64>, Error> {
        if let Some(keys) = split(
            at.checked_sub(1).map(|i| self.play[i].ord),
            self.play.get(at).map(|e| e.ord),
            count,
        ) {
            return Ok(keys);
        }
        renumber(
            conn,
            "ord",
            self.play.iter_mut().map(|e| (e.uid, e.kept, &mut e.ord)),
        )?;
        Ok(split(
            at.checked_sub(1).map(|i| self.play[i].ord),
            self.play.get(at).map(|e| e.ord),
            count,
        )
        .expect("renumbered keys have room"))
    }

    fn original_keys_at(
        &mut self,
        conn: &Connection,
        at: usize,
        count: usize,
    ) -> Result<Vec<f64>, Error> {
        let original = self.original.as_mut().expect("shuffled");
        let bounds = |o: &[(Uid, f64)]| (at.checked_sub(1).map(|i| o[i].1), o.get(at).map(|e| e.1));
        let (low, high) = bounds(original);
        if let Some(keys) = split(low, high, count) {
            return Ok(keys);
        }
        renumber(
            conn,
            "original",
            original.iter_mut().map(|(uid, key)| (*uid, true, key)),
        )?;
        let (low, high) = bounds(original);
        Ok(split(low, high, count).expect("renumbered keys have room"))
    }
}

/// `count` keys strictly between `low` and `high` (either open), evenly
/// spaced; None when they are too close to split.
fn split(low: Option<f64>, high: Option<f64>, count: usize) -> Option<Vec<f64>> {
    let n = count as f64;
    let (low, step) = match (low, high) {
        (None, None) => (-1.0, 1.0),
        (Some(low), None) => (low, 1.0),
        (None, Some(high)) => (high - n - 1.0, 1.0),
        (Some(low), Some(high)) => (low, (high - low) / (n + 1.0)),
    };
    (step > MIN_GAP).then(|| (1..=count).map(|i| low + step * i as f64).collect())
}

/// Gives every entry its index as its key, and writes the kept ones'.
fn renumber<'a>(
    conn: &Connection,
    column: &str,
    entries: impl Iterator<Item = (Uid, bool, &'a mut f64)>,
) -> Result<(), Error> {
    let sql = format!("UPDATE queue_items SET {column} = ?2 WHERE uid = ?1");
    let mut update = conn.prepare(&sql)?;
    for (index, (uid, kept, key)) in entries.enumerate() {
        *key = index as f64;
        if kept {
            update.execute(params![uid as i64, index as f64])?;
        }
    }
    log::debug!("renumbered the queue's {column} keys");
    Ok(())
}

fn delete(conn: &Connection, uids: &[Uid]) -> Result<(), Error> {
    if uids.is_empty() {
        return Ok(());
    }
    let json = serde_json::to_string(uids).expect("uids serialize");
    conn.execute(
        "DELETE FROM queue_items WHERE uid IN (SELECT value FROM json_each(?1))",
        [json],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anomp::PlayerState;
    use crate::library::db;
    use crate::queue::model::{ListLog, Opening, Player, Queue, Request, TrackInfo};

    /// Opens everything at once and plays nothing.
    struct Idle;

    impl Player for Idle {
        fn load(&mut self, _: i64) -> Opening {
            Opening::Done(Ok(()))
        }
        fn set_next(&mut self, _: Option<i64>, _: bool) -> Opening {
            Opening::Done(Ok(()))
        }
        fn cancel(&mut self, _: Request) {}
        fn volume(&self) -> f64 {
            1.0
        }
        fn set_volume(&mut self, _: f64) {}
        fn play(&mut self) -> bool {
            true
        }
        fn pause(&mut self) {}
        fn stop(&mut self) {}
        fn seek(&mut self, _: f64) -> bool {
            true
        }
        fn state(&self) -> PlayerState {
            PlayerState::Paused
        }
        fn position(&self) -> f64 {
            0.0
        }
        fn advance_count(&self) -> i64 {
            0
        }
    }

    fn infos(ids: impl IntoIterator<Item = i64>) -> Vec<TrackInfo> {
        ids.into_iter()
            .map(|track_id| TrackInfo {
                track_id,
                title: format!("t{track_id}"),
                ..TrackInfo::default()
            })
            .collect()
    }

    /// Writes the queue's changes as the host does.
    fn save(conn: &Connection, queue: &mut Queue, store: &mut Store) {
        match queue.take_stored() {
            ListLog::Clean => {}
            ListLog::Reset => store
                .reset(conn, queue.items(), queue.original_order())
                .unwrap(),
            ListLog::Edits(edits) => store.apply(conn, &edits).unwrap(),
        }
    }

    /// The rows read back match the queue: its kept items in order, and
    /// their order before shuffling.
    fn check(conn: &Connection, queue: &Queue) {
        let (_, loaded) = Store::load(conn).unwrap();
        let kept: Vec<(Uid, i64)> = queue
            .items()
            .iter()
            .filter(|item| !item.track.external)
            .map(|item| (item.uid, item.track.track_id))
            .collect();
        assert_eq!(loaded.items, kept);
        let original: Option<Vec<Uid>> = queue.original_order().map(|order| {
            let kept: std::collections::HashSet<Uid> = kept.iter().map(|k| k.0).collect();
            order
                .iter()
                .copied()
                .filter(|uid| kept.contains(uid))
                .collect()
        });
        assert_eq!(loaded.original, original.filter(|o| !o.is_empty()));
    }

    fn uid(queue: &Queue, index: usize) -> Uid {
        queue.items()[index].uid
    }

    #[test]
    fn rows_follow_the_queue_edit_by_edit_on_50_000_items() {
        let conn = db::open_in_memory().unwrap();
        let mut store = Store::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut Idle, infos(1..=50_000), 0, false);
        save(&conn, &mut queue, &mut store);
        check(&conn, &queue);

        queue.add(&mut Idle, infos([7, 8, 9]), true);
        save(&conn, &mut queue, &mut store);
        check(&conn, &queue);
        let gone = [uid(&queue, 2), uid(&queue, 3), uid(&queue, 40_000)];
        queue.remove(&mut Idle, &gone);
        save(&conn, &mut queue, &mut store);
        check(&conn, &queue);
        queue.move_item(&mut Idle, uid(&queue, 49_000), 1);
        save(&conn, &mut queue, &mut store);
        check(&conn, &queue);
        let block = [uid(&queue, 5), uid(&queue, 6), uid(&queue, 25_000)];
        queue.move_items(&mut Idle, &block, 30_000);
        save(&conn, &mut queue, &mut store);
        check(&conn, &queue);
        queue.set_shuffle(&mut Idle, true);
        save(&conn, &mut queue, &mut store);
        check(&conn, &queue);
    }

    #[test]
    fn an_edit_writes_only_its_rows() {
        let conn = db::open_in_memory().unwrap();
        let mut store = Store::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut Idle, infos(1..=1000), 0, false);
        save(&conn, &mut queue, &mut store);
        let changes = |conn: &Connection| -> i64 {
            conn.query_row("SELECT total_changes()", [], |row| row.get(0))
                .unwrap()
        };
        let before = changes(&conn);
        queue.add(&mut Idle, infos([5000, 5001]), true);
        save(&conn, &mut queue, &mut store);
        assert_eq!(changes(&conn) - before, 2);
        let before = changes(&conn);
        queue.move_item(&mut Idle, uid(&queue, 900), 3);
        save(&conn, &mut queue, &mut store);
        assert_eq!(changes(&conn) - before, 1);
    }

    #[test]
    fn keys_too_close_to_split_are_renumbered() {
        let conn = db::open_in_memory().unwrap();
        let mut store = Store::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut Idle, infos(1..=3), 0, false);
        save(&conn, &mut queue, &mut store);
        // Each "play next" goes between the current item and the one after
        // it, halving the gap there: past 30 halvings it is renumbered.
        for id in 100..200 {
            queue.add(&mut Idle, infos([id]), true);
            save(&conn, &mut queue, &mut store);
        }
        check(&conn, &queue);
        assert!(store.play.windows(2).all(|pair| pair[0].ord < pair[1].ord));
    }

    #[test]
    fn files_from_outside_the_library_take_a_place_but_no_row() {
        let conn = db::open_in_memory().unwrap();
        let mut store = Store::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut Idle, infos(1..=4), 0, false);
        save(&conn, &mut queue, &mut store);
        let external = TrackInfo {
            track_id: -1,
            external: true,
            ..TrackInfo::default()
        };
        queue.add(&mut Idle, vec![external], true);
        queue.add(&mut Idle, infos([9]), true);
        save(&conn, &mut queue, &mut store);
        check(&conn, &queue);
        queue.move_item(&mut Idle, uid(&queue, 2), 4);
        save(&conn, &mut queue, &mut store);
        check(&conn, &queue);
    }

    #[test]
    fn a_block_moved_while_shuffled_keeps_its_original_place() {
        let conn = db::open_in_memory().unwrap();
        let mut store = Store::default();
        let mut queue = Queue::new(7);
        queue.replace(&mut Idle, infos(1..=50), 0, false);
        queue.set_shuffle(&mut Idle, true);
        save(&conn, &mut queue, &mut store);
        let block = [uid(&queue, 3), uid(&queue, 4), uid(&queue, 40)];
        queue.move_items(&mut Idle, &block, 10);
        queue.add(&mut Idle, infos([99]), true);
        save(&conn, &mut queue, &mut store);
        check(&conn, &queue);
        // And unshuffling puts them back where the rows say.
        let (_, loaded) = Store::load(&conn).unwrap();
        queue.set_shuffle(&mut Idle, false);
        let order: Vec<Uid> = queue.items().iter().map(|item| item.uid).collect();
        assert_eq!(loaded.original.unwrap(), order);
    }

    #[test]
    fn retain_deletes_the_rows_the_queue_dropped() {
        let conn = db::open_in_memory().unwrap();
        let mut store = Store::default();
        let mut queue = Queue::new(1);
        queue.replace(&mut Idle, infos(1..=5), 0, false);
        save(&conn, &mut queue, &mut store);
        let (mut loaded_store, loaded) = Store::load(&conn).unwrap();
        let kept: Vec<Uid> = loaded
            .items
            .iter()
            .map(|i| i.0)
            .filter(|&u| u != 3)
            .collect();
        loaded_store.retain(&conn, &kept).unwrap();
        let (_, again) = Store::load(&conn).unwrap();
        assert_eq!(again.items.iter().map(|i| i.0).collect::<Vec<_>>(), kept);
    }
}
