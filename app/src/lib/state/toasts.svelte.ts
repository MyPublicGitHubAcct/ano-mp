// Short messages at the bottom of the window: errors from commands, and
// tracks the queue had to skip. Coded errors from the backend show in the
// user's language (`errorText`, PLAN.md F19).

import { errorText } from "$lib/i18n";

export type Toast = { id: number; text: string; kind: "error" | "info" };

let nextId = 1;

class Toasts {
  list = $state<Toast[]>([]);

  show(text: string, kind: Toast["kind"] = "error", ms = 6000) {
    const id = nextId++;
    this.list = [...this.list.slice(-3), { id, text, kind }];
    setTimeout(() => this.dismiss(id), ms);
  }

  dismiss(id: number) {
    this.list = this.list.filter((toast) => toast.id !== id);
  }
}

export const toasts = new Toasts();

/** Runs a command, showing its error (if any) as a toast. */
export async function attempt<T>(action: () => Promise<T>): Promise<T | undefined> {
  try {
    return await action();
  } catch (error) {
    toasts.show(errorText(error));
    return undefined;
  }
}
