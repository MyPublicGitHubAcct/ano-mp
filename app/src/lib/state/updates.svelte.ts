// Newer releases (PLAN.md §8.2): the last check, from Settings › About's
// button or the automatic checks (off by default), and a toast when an
// automatic check finds a newer version. The download is the release's
// page on GitHub; nothing installs itself.

import { on, onAll, updates as api, type UpdateCheck } from "$lib/api";
import { t } from "$lib/i18n";
import { attempt, toasts } from "./toasts.svelte";

class UpdatesStore {
  /** The last check that worked since launch; null before one. */
  last = $state.raw<UpdateCheck | null>(null);
  checking = $state(false);

  connect() {
    const stop = onAll([
      on("update-available", (check) => {
        this.last = check;
        toasts.show(t("updates.available", { version: check.latest ?? "" }), "info", 12000);
      }),
    ]);
    api.status().then(
      (check) => (this.last ??= check),
      () => {},
    );
    return stop;
  }

  async check() {
    this.checking = true;
    try {
      const check = await attempt(() => api.check());
      if (check) this.last = check;
    } finally {
      this.checking = false;
    }
  }
}

export const updates = new UpdatesStore();
