import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";

const looseAny = /: any\b|any\[\]|\.\.\.args: any/;

test("rail page scopes have no loose any", () => {
  const dir = new URL("../src/lib/rail/", import.meta.url);
  const files = readdirSync(dir)
    .filter((name) => /^page.*\.svelte\.ts$/.test(name))
    .sort();
  assert.deepEqual(files, [
    "pageBeing.svelte.ts",
    "pageClaim.svelte.ts",
    "pageDrag.svelte.ts",
    "pagePointer.svelte.ts",
    "pageRing.svelte.ts",
    "pageRows.svelte.ts",
    "pageStage.svelte.ts",
  ]);
  for (const name of files) {
    const text = readFileSync(new URL(name, dir), "utf8");
    assert.equal(looseAny.test(text), false, name);
  }
});
