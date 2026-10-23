// Dragging tracks within the window (`state/drag.svelte.ts`, F4): a press
// becomes a drag past a few pixels, the target under the pointer is
// highlighted if it accepts the payload and gets it on release, and Esc
// cancels. jsdom has no layout, so `elementFromPoint` is the test's.

import { afterEach, describe, expect, test, vi } from "vitest";
import type { DragPayload } from "$lib/state/drag.svelte";
import { fakeBackend } from "./backend";

/** A pointer event of the primary button and pointer 1 (jsdom may lack `PointerEvent`). */
function pointer(type: string, x: number, y: number, pointerId = 1): PointerEvent {
  const event = new MouseEvent(type, { clientX: x, clientY: y, button: 0, bubbles: true });
  Object.defineProperty(event, "pointerId", { value: pointerId });
  return event as PointerEvent;
}

const payload: DragPayload = { kind: "queue", uids: [3] };

// A drag a test leaves going would hear the next test's pointer.
afterEach(() => void window.dispatchEvent(pointer("pointercancel", 0, 0)));

async function load() {
  fakeBackend();
  const { drag, registerDropTarget } = await import("$lib/state/drag.svelte");
  const target = document.createElement("div");
  target.dataset.drop = "queue";
  document.body.replaceChildren(target);
  document.elementFromPoint = (x: number) => (x >= 100 ? target : document.body);
  const drop = vi.fn();
  const accepts = vi.fn((given: DragPayload) => given.kind === "queue");
  const unregister = registerDropTarget("queue", { accepts, drop });
  drag.press(pointer("pointerdown", 0, 0), () => ({ payload, label: "1 track" }));
  return { drag, target, drop, accepts, unregister };
}

describe("a press", () => {
  test("becomes a drag only once the pointer has moved far enough", async () => {
    const { drag } = await load();
    window.dispatchEvent(pointer("pointermove", 3, 3));
    expect(drag.current).toBeNull();
    window.dispatchEvent(pointer("pointermove", 10, 0));
    expect(drag.current).toMatchObject({ payload, label: "1 track", x: 10, y: 0, over: null });
    expect(document.body.classList.contains("dragging-items")).toBe(true);
  });

  test("of another pointer is no drag", async () => {
    const { drag } = await load();
    window.dispatchEvent(pointer("pointermove", 50, 0, 2));
    expect(drag.current).toBeNull();
  });
});

describe("a drag", () => {
  test("highlights the target under the pointer and drops on it", async () => {
    const { drag, target, drop } = await load();
    window.dispatchEvent(pointer("pointermove", 120, 0));
    expect(drag.current?.over).toBe("queue");
    expect(target.hasAttribute("data-drop-over")).toBe(true);
    window.dispatchEvent(pointer("pointermove", 50, 0));
    expect(target.hasAttribute("data-drop-over")).toBe(false);
    window.dispatchEvent(pointer("pointermove", 130, 0));
    window.dispatchEvent(pointer("pointerup", 130, 0));
    expect(drop).toHaveBeenCalledOnce();
    expect(drop.mock.calls[0][0]).toEqual(payload);
    expect([drag.current, target.hasAttribute("data-drop-over")]).toEqual([null, false]);
    expect(document.body.classList.contains("dragging-items")).toBe(false);
  });

  test("passes over a target that doesn't accept it", async () => {
    const { drag, drop, accepts } = await load();
    accepts.mockReturnValue(false);
    window.dispatchEvent(pointer("pointermove", 120, 0));
    expect(drag.current?.over).toBeNull();
    window.dispatchEvent(pointer("pointerup", 120, 0));
    expect(drop).not.toHaveBeenCalled();
  });

  test("Esc cancels it, and the window's own Esc never hears it", async () => {
    const { drag, drop } = await load();
    const back = vi.fn();
    window.addEventListener("keydown", back);
    window.dispatchEvent(pointer("pointermove", 120, 0));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }));
    expect(drag.current).toBeNull();
    expect(back).not.toHaveBeenCalled();
    window.dispatchEvent(pointer("pointerup", 120, 0));
    expect(drop).not.toHaveBeenCalled();
    window.removeEventListener("keydown", back);
  });
});

test("a target removed is no longer dropped on, but only its own registration goes", async () => {
  const { drag, drop, unregister } = await load();
  const { dropTargets, registerDropTarget } = await import("$lib/state/drag.svelte");
  const other = { accepts: () => true, drop: vi.fn() };
  registerDropTarget("queue", other);
  unregister();
  expect(dropTargets.get("queue")).toBe(other);
  window.dispatchEvent(pointer("pointermove", 120, 0));
  window.dispatchEvent(pointer("pointerup", 120, 0));
  expect([drop.mock.calls.length, other.drop.mock.calls.length]).toEqual([0, 1]);
  expect(drag.current).toBeNull();
});
