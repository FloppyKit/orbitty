import assert from "node:assert/strict";
import test from "node:test";
import { armOnShrink } from "../src/lib/rail/nudgeTrigger.ts";

test("a shrink we just heard arms that window", () => {
  assert.equal(armOnShrink("9:1", null, new Set(), [], null), "9:1");
});

test("a claimed or already-asked window does not arm", () => {
  assert.equal(armOnShrink("9:1", null, new Set(["9:1"]), [], null), null);
  assert.equal(armOnShrink("9:1", null, new Set(), ["9:1"], null), null);
  assert.equal(armOnShrink("9:1", null, new Set(), ["bundle|t|1"], "bundle|t|1"), null);
});

test("a prompt already up is left alone", () => {
  assert.equal(armOnShrink("9:2", "9:1", new Set(), [], null), "9:1");
});
