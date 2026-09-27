// Dragging tracks within the window (PLAN.md F4): onto a playlist or the
// queue in the sidebar, or onto the queue panel. Pointer events rather than
// HTML drag and drop, as the queue's own reordering does, so it works on
// touch screens (Phase 8) and isn't taken over by the window's file drop.
//
// A drag starts once the pointer has moved a few pixels with a button
// down on a row. Drop targets are elements with `data-drop="<id>"`, which
// register what they accept and do with `dropTargets`; the one under the
// pointer is highlighted (it gets `data-drop-over`).

export type DragPayload =
  /** Tracks, found when dropped (an album's are fetched then). */
  | { kind: "tracks"; trackIds: () => Promise<number[]> }
  /** Queue items, by uid, from the queue panel. */
  | { kind: "queue"; uids: number[] }
  /** Entries of a playlist, from its view. */
  | { kind: "playlist"; playlistId: number; itemIds: number[]; trackIds: number[] };

export type DropTarget = {
  accepts: (payload: DragPayload) => boolean;
  drop: (payload: DragPayload, event: PointerEvent, element: HTMLElement) => void;
};

/** Pixels the pointer moves before a press becomes a drag. */
const THRESHOLD = 6;

export const dropTargets = new Map<string, DropTarget>();

/** Registers a drop target while a component is mounted; returns the
    function that removes it. */
export function registerDropTarget(id: string, target: DropTarget) {
  dropTargets.set(id, target);
  return () => {
    if (dropTargets.get(id) === target) dropTargets.delete(id);
  };
}

class Drag {
  /** The drag under way, with the pointer's position and the target under it. */
  current = $state.raw<{ payload: DragPayload; label: string; x: number; y: number; over: string | null } | null>(
    null,
  );
  #pending: { payload: DragPayload; label: string; x: number; y: number; pointerId: number } | null = null;
  #overElement: HTMLElement | null = null;

  /** Call on pointerdown on something draggable; the drag begins if the
      pointer moves far enough before it's released. */
  press(event: PointerEvent, payload: () => { payload: DragPayload; label: string } | null) {
    if (event.button !== 0) return;
    const start = { x: event.clientX, y: event.clientY, pointerId: event.pointerId };
    const move = (e: PointerEvent) => {
      if (e.pointerId !== start.pointerId) return;
      if (!this.#pending && !this.current) {
        if (Math.hypot(e.clientX - start.x, e.clientY - start.y) < THRESHOLD) return;
        const made = payload();
        if (!made) return stop();
        this.#pending = { ...made, ...start };
        this.current = { ...made, x: e.clientX, y: e.clientY, over: null };
        document.body.classList.add("dragging-items");
      }
      this.#track(e);
    };
    const up = (e: PointerEvent) => {
      if (e.pointerId !== start.pointerId) return;
      if (this.current) this.#drop(e);
      stop();
    };
    const cancel = () => stop();
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") stop();
    };
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
      window.removeEventListener("keydown", key, true);
      this.#setOver(null);
      this.#pending = null;
      this.current = null;
      document.body.classList.remove("dragging-items");
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
    window.addEventListener("keydown", key, true);
  }

  /** The drop target under the point, if it accepts the payload. */
  #targetAt(x: number, y: number): HTMLElement | null {
    const payload = this.current?.payload;
    if (!payload) return null;
    const element = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-drop]") ?? null;
    const id = element?.dataset.drop;
    return id && dropTargets.get(id)?.accepts(payload) ? element : null;
  }

  #track(event: PointerEvent) {
    if (!this.current) return;
    const element = this.#targetAt(event.clientX, event.clientY);
    this.#setOver(element);
    this.current = { ...this.current, x: event.clientX, y: event.clientY, over: element?.dataset.drop ?? null };
  }

  #setOver(element: HTMLElement | null) {
    if (this.#overElement === element) return;
    this.#overElement?.removeAttribute("data-drop-over");
    element?.setAttribute("data-drop-over", "");
    this.#overElement = element;
  }

  #drop(event: PointerEvent) {
    const payload = this.current?.payload;
    const element = this.#targetAt(event.clientX, event.clientY);
    const id = element?.dataset.drop;
    if (payload && element && id) dropTargets.get(id)?.drop(payload, event, element);
  }
}

export const drag = new Drag();
