import assert from "node:assert/strict";
import { test } from "node:test";
import {
  beforeIdAt,
  columnArrow,
  columnMode,
  columnPx,
} from "../src/lib/board/planetColumn.ts";

const ids = ["inbox", "today", "done", "pA", "pB"];
const none = { firstCardId: null, firstNeededId: null };

test("C1 down walks the column", () => {
  assert.equal(columnArrow(ids, "today", "down", none), "done");
});

test("C2 up wraps to the bottom", () => {
  assert.equal(columnArrow(ids, "inbox", "up", none), "pB");
  assert.equal(columnArrow(ids, "pB", "down", none), "inbox");
});

test("C3 right drops into the list", () => {
  assert.equal(
    columnArrow(ids, "today", "right", {
      firstCardId: "c1",
      firstNeededId: "n1",
    }),
    "c1",
  );
  assert.equal(
    columnArrow(ids, "today", "right", {
      firstCardId: null,
      firstNeededId: "n1",
    }),
    "n1",
  );
  assert.equal(columnArrow(ids, "today", "right", none), null);
});

test("C4 left does nothing", () => {
  assert.equal(columnArrow(ids, "today", "left", none), null);
  assert.equal(
    columnArrow(ids, "pB", "left", { firstCardId: "c1", firstNeededId: "n1" }),
    null,
  );
});

test("C5 vertical drop target", () => {
  const rows = [
    { id: "A", top: 0, height: 30 },
    { id: "B", top: 30, height: 30 },
    { id: "C", top: 60, height: 30 },
  ];
  assert.equal(beforeIdAt(rows, 10), "A");
  assert.equal(beforeIdAt(rows, 50), "C");
  assert.equal(beforeIdAt(rows, 200), null);
});

test("C6 auto collapse", () => {
  assert.equal(columnMode(500, "auto"), "icons");
  assert.equal(columnMode(900, "auto"), "open");
  assert.equal(columnMode(500, "open"), "open");
  assert.equal(columnMode(900, "icons"), "icons");
});

test("C7 preview clamp width", () => {
  const icons = columnPx("icons");
  const open = columnPx("open");
  assert.ok(icons > 0);
  assert.ok(open > 0);
  assert.ok(icons < open);
});
