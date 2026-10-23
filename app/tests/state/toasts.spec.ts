// Toasts (`state/toasts.svelte.ts`): shown, dismissed on time or by hand,
// at most four, and `attempt` turning a command's error into one.

import { describe, expect, test, vi } from "vitest";
import { errorText, t } from "$lib/i18n";
import { fakeBackend } from "./backend";
import { queueFixtures, serverFixtures } from "./fixtures";

async function load() {
  fakeBackend();
  return import("$lib/state/toasts.svelte");
}

const texts = (list: { text: string }[]) => list.map((toast) => toast.text);

describe("toasts", () => {
  test("go after their time", async () => {
    vi.useFakeTimers();
    const { toasts } = await load();
    toasts.show("a");
    toasts.show("b", "info", 1000);
    expect(toasts.list.map((toast) => [toast.text, toast.kind])).toEqual([
      ["a", "error"],
      ["b", "info"],
    ]);
    vi.advanceTimersByTime(1000);
    expect(texts(toasts.list)).toEqual(["a"]);
    vi.advanceTimersByTime(5000);
    expect(toasts.list).toEqual([]);
  });

  test("keep the newest four", async () => {
    const { toasts } = await load();
    for (const text of ["1", "2", "3", "4", "5"]) toasts.show(text);
    expect(texts(toasts.list)).toEqual(["2", "3", "4", "5"]);
  });

  test("can be dismissed by hand", async () => {
    const { toasts } = await load();
    toasts.show("a");
    toasts.show("b");
    toasts.dismiss(toasts.list[0].id);
    expect(texts(toasts.list)).toEqual(["b"]);
  });
});

describe("attempt", () => {
  test("resolves to what the command did, showing nothing", async () => {
    const { attempt, toasts } = await load();
    expect(await attempt(async () => 7)).toBe(7);
    expect(toasts.list).toEqual([]);
  });

  test("shows a coded error in the user's language", async () => {
    const { attempt, toasts } = await load();
    const { jellyfinOff } = serverFixtures;
    expect(await attempt(() => Promise.reject(jellyfinOff))).toBeUndefined();
    expect(texts(toasts.list)).toEqual([errorText(jellyfinOff)]);
    expect(errorText(jellyfinOff)).toBe(t("error.featureOff", JSON.parse(jellyfinOff).params));

    const [timedOut] = queueFixtures.skipped.skipped;
    await attempt(() => Promise.reject(timedOut.error));
    expect(texts(toasts.list)[1]).toBe(t("error.openTimedOut", { seconds: 20 }));
  });

  test("shows a plain error's own text", async () => {
    const { attempt, toasts } = await load();
    await attempt(() => Promise.reject(new Error("The library is not available")));
    expect(texts(toasts.list)).toEqual(["The library is not available"]);
  });
});
