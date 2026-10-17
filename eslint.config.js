import svelte from "eslint-plugin-svelte";
import * as svelteParser from "svelte-eslint-parser";
import tseslint from "typescript-eslint";

const glassSyntax = [
  "error",
  {
    selector: "Identifier[name='GLASS_CARD_SELECTORS']",
    message: "Glass collection is the literal [data-glass] query.",
  },
  {
    selector:
      "CallExpression[callee.property.name='querySelectorAll'][arguments.0.value!='[data-glass]']",
    message: "querySelectorAll must take the literal [data-glass].",
  },
];

export default [
  {
    // qa and the Rust tree are not this rule set. The lint script names
    // the glass module and the flush page only, so the rest of src is
    // not parsed: the default parser cannot read it, and this round
    // does not reformat it.
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
    files: ["src/lib/glass/**/*.ts"],
    languageOptions: {
      parser: tseslint.parser,
      sourceType: "module",
    },
    rules: {
      "no-restricted-syntax": glassSyntax,
    },
  },
  {
    files: ["src/routes/+page.svelte"],
    plugins: { svelte },
    languageOptions: {
      parser: svelteParser,
      parserOptions: {
        parser: tseslint.parser,
      },
    },
    processor: "svelte/svelte",
    rules: {
      "no-restricted-syntax": glassSyntax,
    },
  },
];
