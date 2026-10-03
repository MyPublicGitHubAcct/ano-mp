// The queue's numbered edits (src/lib/queueEdits.ts, PLAN.md H16).
import assert from "node:assert/strict";
import { test } from "node:test";
import { applyEdits, listAction } from "../src/lib/queueEdits.ts";

const range = (from, to) => Array.from({ length: to - from }, (_, i) => from + i);

test("each kind of edit changes a copy, in order", () => {
  const list = [1, 2, 3, 4, 5];
  const after = applyEdits(list, [
    { kind: "insert", at: 1, items: [10, 11] },
    { kind: "remove", at: 4, count: 2 },
    { kind: "move", from: 0, count: 2, to: 2 },
    { kind: "update", at: 0, items: [12] },
  ]);
  assert.deepEqual(after, [12, 2, 1, 10, 5]);
  assert.deepEqual(list, [1, 2, 3, 4, 5]);
});

test("a move's target is an index in the list without the moved items", () => {
  assert.deepEqual(applyEdits([1, 2, 3, 4], [{ kind: "move", from: 0, count: 1, to: 3 }]), [2, 3, 4, 1]);
  assert.deepEqual(applyEdits([1, 2, 3, 4], [{ kind: "move", from: 2, count: 2, to: 0 }]), [3, 4, 1, 2]);
});

test("an edit that doesn't fit throws, so the whole list is asked for", () => {
  assert.throws(() => applyEdits([1, 2], [{ kind: "remove", at: 1, count: 2 }]), RangeError);
  assert.throws(() => applyEdits([1, 2], [{ kind: "insert", at: 3, items: [9] }]), RangeError);
  assert.throws(() => applyEdits([1, 2], [{ kind: "move", from: 0, count: 1, to: 2 }]), RangeError);
});

test("50,000 items inserted at once and moved as a block", () => {
  const list = range(0, 50_000);
  const inserted = applyEdits(list, [{ kind: "insert", at: 1, items: range(50_000, 100_000) }]);
  assert.equal(inserted.length, 100_000);
  assert.equal(inserted[1], 50_000);
  assert.equal(inserted[50_001], 1);
  const moved = applyEdits(inserted, [{ kind: "move", from: 1, count: 50_000, to: 50_000 }]);
  assert.deepEqual(moved.slice(0, 3), [0, 1, 2]);
  assert.equal(moved[50_000], 50_000);
});

test("listAction follows the version numbers", () => {
  const edits = (listVersion) => ({ listVersion, items: null, edits: [] });
  assert.equal(listAction(4, edits(5)), "apply");
  assert.equal(listAction(4, edits(7)), "resync");
  assert.equal(listAction(4, edits(4)), "keep");
  assert.equal(listAction(4, { listVersion: 4, items: null, edits: null }), "keep");
  assert.equal(listAction(4, { listVersion: 9, items: [], edits: null }), "replace");
  assert.equal(listAction(4, { listVersion: 3, items: [], edits: null }), "keep");
});
