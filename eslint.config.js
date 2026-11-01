import svelte from "eslint-plugin-svelte";
import * as svelteParser from "svelte-eslint-parser";
import tseslint from "typescript-eslint";

export default [
  {
    // qa and the Rust tree are not this rule set. The lint script
    // names src svelte and ts; these dirs stay off the path.
    ignores: [
      "node_modules/**",
      "dist/**",
      "build/**",
      ".svelte-kit/**",
      "src-tauri/**",
      "qa/**",
    ],
  },
  {
    files: ["src/**/*.svelte"],
    plugins: { svelte },
    languageOptions: {
      parser: svelteParser,
      parserOptions: {
        parser: tseslint.parser,
      },
    },
    processor: "svelte/svelte",
  },
  {
    files: ["src/**/*.ts"],
    languageOptions: {
      parser: tseslint.parser,
    },
  },
];
