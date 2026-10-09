// Landing on a found feature's control (PLAN.md X9): scrolled into view,
// highlighted for a moment (`.found`, styled in +layout.svelte, without
// the fade under Reduce Motion) and focused, so VoiceOver reads its label.

const FOCUSABLE = "input:not([type='hidden']), select, textarea, button, [href], [tabindex]";
const HIGHLIGHT_MS = 2000;

/** The control in `element` to focus: itself, the choice made in a group, or the first control in it. */
function controlIn(element: HTMLElement): HTMLElement | null {
  if (element.matches(FOCUSABLE)) return element;
  return (
    element.querySelector<HTMLElement>("input:checked, [aria-checked='true']") ??
    element.querySelector<HTMLElement>(FOCUSABLE)
  );
}

/** Scrolls to `element`, highlights its row (a `.field`, `.switch` or `.card` around it) and focuses its control; a disabled control's row takes the focus instead. */
export function landOn(element: HTMLElement) {
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const row = element.closest<HTMLElement>(".field, .switch, .card") ?? element;
  row.scrollIntoView({ block: "center", behavior: reduce ? "auto" : "smooth" });
  row.classList.add("found");
  setTimeout(() => row.classList.remove("found"), HIGHLIGHT_MS);

  const control = controlIn(element);
  if (control && !control.matches(":disabled")) {
    control.focus({ preventScroll: true });
    return;
  }
  if (!row.hasAttribute("tabindex")) {
    row.tabIndex = -1;
    row.addEventListener("blur", () => row.removeAttribute("tabindex"), { once: true });
  }
  row.focus({ preventScroll: true });
}

/** Lands on the element `id` once it is drawn (a section may draw its controls after loading), trying for about a second; else on `fallback`'s. Returns a function that stops trying. */
export function landWhenDrawn(id: string, fallback: string | null, done: () => void): () => void {
  let frames = 0;
  let frame = requestAnimationFrame(function attempt() {
    const element = document.getElementById(id);
    if (element || ++frames >= 60) {
      const target = element ?? (fallback ? document.getElementById(fallback) : null);
      if (target) landOn(target);
      done();
      return;
    }
    frame = requestAnimationFrame(attempt);
  });
  return () => cancelAnimationFrame(frame);
}
