import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

test("alpha spirit only: no face pack or face picker", () => {
  const conf = readFileSync("src-tauri/tauri.conf.json", "utf8");
  const cargo = readFileSync("src-tauri/Cargo.toml", "utf8");
  assert.match(cargo, /^alpha = \[\]$/m);
  const companion = [...conf.matchAll(/"\.\.\/brand\/companion\/[^"]+"/g)].map((m) => m[0]);
  assert.ok(companion.length > 0);
  for (const path of companion) {
    assert.match(path, /companion\/spirit\//);
    assert.doesNotMatch(path, /face/i);
  }
  const src = readFileSync("src/lib/CompanionPage.svelte", "utf8");
  assert.doesNotMatch(src, /FacePicker|face-pack|face_pack/);
  const setup = readFileSync("src/lib/ModelSetup.svelte", "utf8");
  assert.match(setup, /https:\/\/ollama.com\/download/);
});
