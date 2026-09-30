import assert from "node:assert/strict";
import { test } from "node:test";
import { resolveFocus } from "../src/lib/board/focusProject.ts";

const planets = ["A", "B"];

test("stale pending A + manual pick B + reload stays on B", () => {
  // Board click A is one-shot once A is in the project list.
  const afterA = resolveFocus({
    pending: "A",
    selected: "inbox",
    projectIds: planets,
  });
  assert.equal(afterA.selected, "A");
  assert.equal(afterA.pending, null);

  // Manual planet-tab pick clears pending, then a reload must stay on B.
  const picked = resolveFocus({
    pending: null,
    selected: "B",
    projectIds: planets,
  });
  assert.equal(picked.selected, "B");
  assert.equal(picked.pending, null);

  const reloaded = resolveFocus({
    pending: picked.pending,
    selected: picked.selected,
    projectIds: planets,
  });
  assert.equal(reloaded.selected, "B");
  assert.equal(reloaded.pending, null);
});

test("board click B after A selects B", () => {
  const afterA = resolveFocus({
    pending: "A",
    selected: "inbox",
    projectIds: planets,
  });
  const afterB = resolveFocus({
    pending: "B",
    selected: afterA.selected,
    projectIds: planets,
  });
  assert.equal(afterB.selected, "B");
  assert.equal(afterB.pending, null);

  const reloaded = resolveFocus({
    pending: afterB.pending,
    selected: afterB.selected,
    projectIds: planets,
  });
  assert.equal(reloaded.selected, "B");
  assert.equal(reloaded.pending, null);
});

test("pending id missing from projects stays until it appears, then clears", () => {
  const waiting = resolveFocus({
    pending: "A",
    selected: "inbox",
    projectIds: [],
  });
  assert.equal(waiting.selected, "A");
  assert.equal(waiting.pending, "A");

  const landed = resolveFocus({
    pending: waiting.pending,
    selected: waiting.selected,
    projectIds: planets,
  });
  assert.equal(landed.selected, "A");
  assert.equal(landed.pending, null);
});
