// The multi-selection model (src/lib/selection.ts).
import assert from "node:assert/strict";
import { test } from "node:test";
import { click, dropIndex, emptySelection, moveTo, rowsFor, selectAll } from "../src/lib/selection.ts";

const rows = (selection) => [...selection.rows].sort((a, b) => a - b);

test("a click selects one row; toggle adds and removes; shift selects a range", () => {
  let s = click(emptySelection, 3);
  assert.deepEqual(rows(s), [3]);
  s = click(s, 5, { toggle: true });
  assert.deepEqual(rows(s), [3, 5]);
  s = click(s, 3, { toggle: true });
  assert.deepEqual(rows(s), [5]);
  // The row toggled last is the anchor, as in the Finder.
  s = click(s, 8, { shift: true });
  assert.deepEqual(rows(s), [3, 4, 5, 6, 7, 8]);
  s = click(s, 1, { shift: true });
  assert.deepEqual(rows(s), [1, 2, 3], "a range from the same anchor");
  s = click(s, 9, { shift: true, toggle: true });
  assert.deepEqual(rows(s), [1, 2, 3, 4, 5, 6, 7, 8, 9]);
  assert.deepEqual(rows(click(s, 1)), [1]);
});

test("the keyboard moves the focus and extends with shift", () => {
  let s = moveTo(emptySelection, 0, 10);
  s = moveTo(s, 2, 10, { shift: true });
  assert.deepEqual(rows(s), [0, 1, 2]);
  s = moveTo(s, 99, 10, { shift: true });
  assert.deepEqual(rows(s), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
  s = moveTo(s, -4, 10);
  assert.deepEqual(rows(s), [0]);
  assert.equal(moveTo(s, 3, 0), emptySelection);
  assert.deepEqual(rows(selectAll(3)), [0, 1, 2]);
});

test("acting on a row outside the selection acts on it alone", () => {
  const s = click(click(emptySelection, 4), 1, { toggle: true });
  assert.deepEqual(rowsFor(s, 4), [1, 4]);
  assert.deepEqual(rowsFor(s, 7), [7]);
});

test("dropping a block counts the rows it leaves", () => {
  assert.equal(dropIndex([1, 2], 5), 3);
  assert.equal(dropIndex([6, 7], 2), 2);
  assert.equal(dropIndex([1, 7], 5), 4);
});

