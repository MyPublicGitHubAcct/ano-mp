// Vitest for the rune modules (`src/lib/state/*.svelte.ts`, PLAN.md H13),
// which plain `node --test` can't compile: `npm run test:state`. The files
// are `*.spec.ts`, since Node's test runner takes `*.test.ts` files under
// `tests/` for its own (`npm test`), and this config picks up only these.
//
// SvelteKit's plugin compiles the runes and resolves `$lib`; Svelte's
// browser build (`conditions`) runs `$effect`s, which its server build
// never does; jsdom gives Tauri's IPC mocks a `window`.

import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [sveltekit()],
  resolve: { conditions: ["browser"] },
  define: { __DEV_TOOLS__: "false" },
  test: {
    include: ["tests/state/**/*.spec.ts"],
    environment: "jsdom",
    setupFiles: ["tests/state/setup.ts"],
  },
});
