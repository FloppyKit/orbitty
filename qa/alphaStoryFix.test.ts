import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { HARNESS_GROUP, HARNESSES, LMSTUDIO_OFF } from "../src/lib/modelSetupCopy.ts";

test("model setup is a solid card inside the chat, not a floating overlay", () => {
  const page = readFileSync("src/lib/CompanionPage.svelte", "utf8");
  assert.doesNotMatch(page, /<ModelSetup/);
  const caption = readFileSync("src/lib/CaptionPane.svelte", "utf8");
  assert.match(caption, /<ModelSetup cue=\{botLine\} \/>/);
  const setup = readFileSync("src/lib/ModelSetup.svelte", "utf8");
  assert.match(setup, /class="model-setup-card"/);
  assert.match(setup, /--bg-solid/);
  assert.match(setup, /aria-label="Base URL"/);
  assert.match(setup, /\{HARNESS_GROUP\}/);
  assert.match(setup, /Look again/);
  assert.equal(HARNESS_GROUP, "Agent harnesses");
  assert.equal(HARNESSES.length, 5);
  for (const h of HARNESSES) {
    assert.ok(h.line.length > 12, h.id);
  }
  assert.match(LMSTUDIO_OFF, /Developer > Start Server/);
  assert.match(LMSTUDIO_OFF, /1234/);
  assert.match(LMSTUDIO_OFF, /Look again/);
});
