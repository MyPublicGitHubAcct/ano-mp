// The developer page is in debug builds only (PLAN.md H2). In a release
// build `__DEV_TOOLS__` is false, so the import is dead code and its chunk
// (with the calls to the debug-only commands) isn't emitted; /dev goes to
// the player instead.
import { redirect } from "@sveltejs/kit";

export async function load() {
  if (__DEV_TOOLS__) {
    return { DevPage: (await import("$lib/components/dev/DevPage.svelte")).default };
  }
  redirect(307, "/");
}
