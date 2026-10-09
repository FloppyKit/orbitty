# qa

Source scans. Not a GUI walk.

Run `node --test qa/*.test.ts` from the repo root. Each file reads source with `readFileSync` and asserts on the text. A scan that used to read one svelte or rust file as a blob joins the split parts with `""` in the old order. The six rail CSS tests read `@import` paths from `src/lib/rail/rail.css` and join those files with `""`.

Do not add a test in a map change. Names stay stable.
