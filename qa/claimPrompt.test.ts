import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

test("claim prompt is a pane, not a rail card", () => {
  const list = src("src/lib/rail/List.svelte");
  assert.equal(list.includes('class="nudge"'), false);
  assert.equal(list.includes("nudgeWindow"), false);

  const page = src("src/routes/+page.svelte");
  assert.match(page, /windowLabel === "claim-prompt"/);
  assert.match(page, /<ClaimPromptPage \/>/);

  const panes = src("src-tauri/src/windows/panes.rs");
  assert.match(panes, /"claim-prompt"/);
  assert.match(panes, /place_beside_rail_display/);
  assert.match(panes, /fn show_claim_prompt/);

  const ui = src("src/lib/ClaimPromptPage.svelte");
  assert.match(ui, /Put \$\{arm.appName\}/);
  assert.match(ui, /on a planet/);
  assert.match(ui, /claim_prompt_act/);
  assert.match(ui, /claim_prompt_later/);
  assert.match(src("src-tauri/capabilities/default.json"), /claim-prompt/);
  assert.match(ui, /nudgeClaim/);
  assert.match(ui, /nudgeCreate/);
  assert.match(ui, /<select/);
  assert.equal(ui.includes('class="nudge"'), false);
  assert.equal(ui.includes("{#each projects as p (p.id)}\n        <button"), false);
});
