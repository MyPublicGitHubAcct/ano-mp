// Multi-selection over a list of rows (PLAN.md F4), as Finder and Music do
// it: a click selects one row, ⌘-click (Ctrl elsewhere) adds or removes
// one, Shift-click selects the range from the anchor, and the arrow keys
// move the focus, extending the selection with Shift. Pure, so it's
// tested with plain `node --test`.

export type Selection = {
  /** Selected row indices. */
  rows: ReadonlySet<number>;
  /** Where a Shift-click or Shift-arrow range starts. */
  anchor: number;
  /** The row the keyboard is on. */
  focus: number;
};

export const emptySelection: Selection = { rows: new Set(), anchor: -1, focus: -1 };

export type Modifiers = { shift?: boolean; toggle?: boolean };

function range(from: number, to: number) {
  const rows = new Set<number>();
  for (let i = Math.min(from, to); i <= Math.max(from, to); i++) rows.add(i);
  return rows;
}

/** The selection after a click on row `index`. */
export function click(selection: Selection, index: number, modifiers: Modifiers = {}): Selection {
  if (modifiers.shift && selection.anchor >= 0) {
    const rows = range(selection.anchor, index);
    if (modifiers.toggle) for (const row of selection.rows) rows.add(row);
    return { rows, anchor: selection.anchor, focus: index };
  }
  if (modifiers.toggle) {
    const rows = new Set(selection.rows);
    if (rows.has(index)) rows.delete(index);
    else rows.add(index);
    return { rows, anchor: index, focus: index };
  }
  return { rows: new Set([index]), anchor: index, focus: index };
}

/** The selection after the keyboard moves the focus to `index` (clamped to
    `count` rows), extending it with Shift. */
export function moveTo(selection: Selection, index: number, count: number, modifiers: Modifiers = {}): Selection {
  if (count === 0) return emptySelection;
  const focus = Math.max(0, Math.min(count - 1, index));
  if (modifiers.shift) {
    const anchor = selection.anchor >= 0 ? selection.anchor : focus;
    return { rows: range(anchor, focus), anchor, focus };
  }
  return { rows: new Set([focus]), anchor: focus, focus };
}

/** Every row selected. */
export const selectAll = (count: number): Selection =>
  count === 0 ? emptySelection : { rows: range(0, count - 1), anchor: 0, focus: count - 1 };

/** The selected rows in order; for a row that isn't selected (right-clicked
    or dragged), that row alone. */
export function rowsFor(selection: Selection, index: number): number[] {
  if (!selection.rows.has(index)) return [index];
  return [...selection.rows].sort((a, b) => a - b);
}

/** Where a block of `moving` rows lands when dropped in gap `gap` (0 to
    `count`), as an index in the list after the move: the gap counts the
    moving rows, the result doesn't. */
export function dropIndex(moving: readonly number[], gap: number) {
  return gap - moving.filter((row) => row < gap).length;
}
