import { defineConfig, searchForWorkspaceRoot } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
// @ts-expect-error type error without @types/node package
import { mkdirSync, writeFileSync } from "node:fs";
const host = process.env.TAURI_DEV_HOST;

// Where the client build lists the npm packages its code came from, which
// scripts/make-notices.py reads: only these ship in the app (PLAN.md §8.2).
const BUNDLED_PACKAGES = ".svelte-kit/output/bundled-packages.json";

/** Records the npm packages whose modules end up in the client bundle. */
function bundledPackages() {
  /** @type {Set<string>} */
  const names = new Set();
  return {
    name: "anomp-bundled-packages",
    apply: /** @type {const} */ ("build"),
    /**
     * @this {{ environment?: { config: { consumer: string } } }}
     * @param {unknown} _options
     * @param {Record<string, { type: string, moduleIds?: string[], modules?: object }>} bundle
     */
    generateBundle(_options, bundle) {
      if (this.environment?.config.consumer !== "client") return;
      for (const output of Object.values(bundle)) {
        if (output.type !== "chunk") continue;
        for (const id of output.moduleIds ?? Object.keys(output.modules ?? {})) {
          const match = /node_modules\/((?:@[^/]+\/)?[^/]+)\//.exec(id.replaceAll("\\", "/"));
          if (match) names.add(match[1]);
        }
      }
      mkdirSync(".svelte-kit/output", { recursive: true });
      writeFileSync(BUNDLED_PACKAGES, JSON.stringify([...names].sort(), null, 2) + "\n");
    },
  };
}

// https://vite.dev/config/
export default defineConfig(({ command }) => ({
  plugins: [sveltekit(), bundledPackages()],

  // The developer tools (/dev, PLAN.md H2) are in `vite dev` and in the
  // Tauri CLI's debug builds, which also register their commands
  // (`debug_assertions`); a release build leaves them out.
  define: {
    __DEV_TOOLS__: JSON.stringify(command === "serve" || process.env.TAURI_ENV_DEBUG === "true"),
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    // The user guide's pages, which the Help window bundles (routes/help).
    fs: {
      allow: [searchForWorkspaceRoot(process.cwd()), "../docs/user-guide"],
    },
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
