// The queue's changes as numbered edits (PLAN.md H16): each `queue-changed`
// that changes the list carries its new `listVersion` and either the whole
// list or the edits from the version before. The UI applies the edits to
// its copy, and asks for the whole list when it has missed a version.

/** One change to the list, as `queue/model.rs`'s `Edit` sends it. */
export type ListEdit<T> =
  | { kind: "insert"; at: number; items: T[] }
  | { kind: "remove"; at: number; count: number }
  | { kind: "move"; from: number; count: number; to: number }
  | { kind: "update"; at: number; items: T[] };

/** The list after `edits`, applied in order, as a new array (the list in
    state is never mutated). Throws on an edit that doesn't fit the list,
    which means the copy is out of step: ask for the whole list. */
export function applyEdits<T>(items: readonly T[], edits: readonly ListEdit<T>[]): T[] {
  // Built with slices, not splice(at, 0, ...items): a 50,000-item insert
  // would pass more arguments than JavaScriptCore takes (65,536).
  let list: readonly T[] = items;
  for (const edit of edits) {
    switch (edit.kind) {
      case "insert":
        check(edit.at <= list.length, edit);
        list = [...list.slice(0, edit.at), ...edit.items, ...list.slice(edit.at)];
        break;
      case "remove":
        check(edit.at + edit.count <= list.length, edit);
        list = [...list.slice(0, edit.at), ...list.slice(edit.at + edit.count)];
        break;
      case "move": {
        check(edit.from + edit.count <= list.length, edit);
        const moving = list.slice(edit.from, edit.from + edit.count);
        const rest = [...list.slice(0, edit.from), ...list.slice(edit.from + edit.count)];
        check(edit.to <= rest.length, edit);
        list = [...rest.slice(0, edit.to), ...moving, ...rest.slice(edit.to)];
        break;
      }
      case "update":
        check(edit.at + edit.items.length <= list.length, edit);
        list = [...list.slice(0, edit.at), ...edit.items, ...list.slice(edit.at + edit.items.length)];
        break;
    }
  }
  return list === items ? items.slice() : (list as T[]);
}

function check(fits: boolean, edit: object) {
  if (!fits) throw new RangeError(`queue edit out of range: ${JSON.stringify(edit)}`);
}

/** What to do with a state, given the list version the UI has: take its
    whole list, apply its edits, ask for the whole list (a version was
    missed), or leave the list alone (unchanged, or already applied). */
export function listAction(
  have: number,
  state: { listVersion: number; items: unknown[] | null; edits: unknown[] | null },
): "replace" | "apply" | "resync" | "keep" {
  if (state.items !== null) return state.listVersion >= have ? "replace" : "keep";
  if (state.edits === null || state.listVersion <= have) return "keep";
  return state.listVersion === have + 1 ? "apply" : "resync";
}
