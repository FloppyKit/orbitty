import assert from "node:assert/strict";
import { test } from "node:test";
import {
  classifyKey,
  surfaceGroups,
  tabMove,
  tabTarget,
} from "../src/lib/ringKeys.ts";

const surfaces = [
  { id: "needed", stops: ["n1"] },
  { id: "tabs", stops: ["t1", "t2"] },
  { id: "todo", stops: ["c1", "c2"] },
];

test("T1 tab jumps to the next surface's first stop", () => {
  // t1 is not the last stop in its group; the next surface starts at c1.
  assert.equal(tabTarget(surfaces, "t1", 1), "c1");
});

test("T2 shift+tab goes back and wraps", () => {
  assert.equal(tabTarget(surfaces, "n1", -1), "c1");
});

test("T3 empty surfaces are skipped", () => {
  const groups = [
    { id: "needed", stops: [] as string[] },
    { id: "tabs", stops: ["t1"] },
    { id: "todo", stops: ["c1"] },
  ];
  assert.equal(tabTarget(groups, "c1", 1), "t1");
});

test("T4 no highlight yet picks an end", () => {
  assert.equal(tabTarget(surfaces, null, 1), "n1");
  assert.equal(tabTarget(surfaces, null, -1), "c1");
});

test("T5 Shift+Tab reaches the ring", () => {
  assert.equal(classifyKey({ key: "Tab", shiftKey: true }), "tab-prev");
  assert.equal(classifyKey({ key: "Tab" }), "tab-next");
});

test("T6 a field keeps its Tab, arrows, and Esc", () => {
  assert.equal(classifyKey({ key: "Tab", inField: true }), null);
  assert.equal(classifyKey({ key: "Tab", shiftKey: true, inField: true }), null);
  assert.equal(classifyKey({ key: "ArrowDown", inField: true }), null);
  // inField is an input, textarea, or select. Esc stays with the field.
  assert.equal(classifyKey({ key: "Escape", inField: true }), null);
});

test("T7 chords are not the ring", () => {
  assert.equal(classifyKey({ key: "Tab", metaKey: true }), null);
  assert.equal(classifyKey({ key: "Tab", ctrlKey: true }), null);
  assert.equal(classifyKey({ key: "Tab", altKey: true }), null);
  assert.equal(
    classifyKey({ key: "ArrowLeft", metaKey: true, altKey: true }),
    null,
  );
});

test("T8 existing verbs unchanged", () => {
  assert.deepEqual(classifyKey({ key: "ArrowUp" }), { arrow: "up" });
  assert.deepEqual(classifyKey({ key: "ArrowDown" }), { arrow: "down" });
  assert.deepEqual(classifyKey({ key: "ArrowLeft" }), { arrow: "left" });
  assert.deepEqual(classifyKey({ key: "ArrowRight" }), { arrow: "right" });
  assert.equal(classifyKey({ key: "Enter" }), "confirm");
  assert.equal(classifyKey({ key: "Enter", shiftKey: true }), "grab");
  assert.equal(
    classifyKey({ key: "Enter", shiftKey: true, inField: true }),
    null,
  );
  assert.equal(classifyKey({ key: "Escape" }), "back");
  assert.equal(classifyKey({ key: "Escape", shiftKey: true }), null);
  assert.equal(classifyKey({ key: "ArrowUp", shiftKey: true }), null);
});

test("T9 Tab is never Esc", () => {
  assert.notEqual(classifyKey({ key: "Tab" }), "back");
  assert.notEqual(classifyKey({ key: "Tab", shiftKey: true }), "back");
});

test("T10 fewer than two surfaces leaves Tab alone", () => {
  const one = surfaceGroups([
    { id: "n1", group: "needed" },
    { id: "n2", group: "needed" },
    { id: "loose" },
  ]);
  assert.deepEqual(one, [{ id: "needed", stops: ["n1", "n2"] }]);
  assert.deepEqual(tabMove(one, "n1", 1), { eat: false, id: null });
  assert.deepEqual(tabMove(one, "n1", -1), { eat: false, id: null });
  assert.deepEqual(tabMove([], null, 1), { eat: false, id: null });
});

test("T11 an active id outside every group picks an end", () => {
  assert.equal(tabTarget(surfaces, "outside", 1), "n1");
  assert.equal(tabTarget(surfaces, "outside", -1), "c1");
});

test("T12 two surfaces eat Tab and jump", () => {
  const groups = surfaceGroups([
    { id: "n1", group: "needed" },
    { id: "c1", group: "todo" },
  ]);
  assert.deepEqual(tabMove(groups, "n1", 1), { eat: true, id: "c1" });
  assert.deepEqual(tabMove(groups, "c1", -1), { eat: true, id: "n1" });
});
