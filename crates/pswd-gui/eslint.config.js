import js from "@eslint/js";
import svelte from "eslint-plugin-svelte";
import globals from "globals";
import ts from "typescript-eslint";

export default ts.config(
  { ignores: ["dist/", "coverage/", "src-tauri/"] },
  js.configs.recommended,
  ts.configs.strict,
  svelte.configs.recommended,
  {
    // `__APP_VERSION__` : remplacé par Vite à la construction (`vite.config.ts`).
    languageOptions: { globals: { ...globals.browser, ...globals.node, __APP_VERSION__: "readonly" } },
  },
  {
    files: ["**/*.svelte", "**/*.svelte.ts"],
    languageOptions: { parserOptions: { parser: ts.parser } },
  },
);
