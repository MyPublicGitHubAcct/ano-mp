// ESLint for the frontend (PLAN.md H13): the recommended rules of ESLint,
// typescript-eslint and eslint-plugin-svelte, with Prettier owning layout
// (eslint-config-prettier turns off the rules that would fight it).
// `npm run lint`; scripts/check-all.py runs it.
import js from "@eslint/js";
import prettier from "eslint-config-prettier";
import svelte from "eslint-plugin-svelte";
import { defineConfig } from "eslint/config";
import globals from "globals";
import ts from "typescript-eslint";
import svelteConfig from "./svelte.config.js";

export default defineConfig(
  { ignores: ["build/", ".svelte-kit/", "src-tauri/", "src/lib/generated/"] },
  js.configs.recommended,
  ts.configs.recommended,
  svelte.configs.recommended,
  prettier,
  svelte.configs.prettier,
  {
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
        // vite.config.js `define` (app.d.ts).
        __DEV_TOOLS__: "readonly",
      },
    },
  },
  {
    files: ["**/*.svelte", "**/*.svelte.ts", "**/*.svelte.js"],
    languageOptions: {
      parserOptions: {
        extraFileExtensions: [".svelte"],
        parser: ts.parser,
        svelteConfig,
      },
    },
  },
  {
    rules: {
      // Nearly every <a> is a web link that `openLink` opens in the browser,
      // and the app has no base path (Tauri serves it at /), so resolve()
      // would change nothing for the two internal ones (/dev and back).
      "svelte/no-navigation-without-resolve": ["error", { ignoreLinks: true }],
    },
  },
);
