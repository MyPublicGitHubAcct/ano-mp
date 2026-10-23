// Effects for the state tests, as a component's would be. Import this
// module in the test, after `setup.ts` has reset the registry, as the
// stores are: a static import would hold another copy of Svelte's runtime
// than the stores', whose signals its effects couldn't see.

import { flushSync } from "svelte";

/** Runs pending effects (the stores' Svelte's `flushSync`). */
export const flush = flushSync;

export type Watch = { readonly runs: number; stop: () => void };

/** Runs `read` in an `$effect` under `$effect.root`, now and whenever what it
    read changes (after `flush`), counting the runs; `stop` ends it. */
export function watch(read: () => unknown): Watch {
  let runs = 0;
  const stop = $effect.root(() => {
    $effect(() => {
      read();
      runs++;
    });
  });
  flushSync();
  return {
    get runs() {
      return runs;
    },
    stop,
  };
}
