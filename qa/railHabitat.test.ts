import assert from "node:assert/strict";
import { test } from "node:test";
import {
  boardSnapshot,
  habitatArrow,
  habitatConfirm,
  habitatStops,
  normalizeCols,
  planetCount,
  raisePlan,
  tiles,
  toggleCol,
  type HabitatCard,
  type HabitatSection,
} from "../src/lib/rail/habitat.ts";

function card(partial: HabitatCard): HabitatCard {
  return partial;
}

const ring: HabitatSection[] = [
  {
    pid: "A",
    open: true,
    tileIds: ["a1", "a2"],
    snapCols: [["n1"], ["t1"]],
    hasTodos: true,
  },
  {
    pid: "B",
    open: false,
    tileIds: ["b1"],
    snapCols: [["bx"]],
    hasTodos: true,
  },
];

test("H1 default is Todo only", () => {
  assert.deepEqual(normalizeCols(undefined), ["todo"]);
  assert.deepEqual(normalizeCols("x"), ["todo"]);
  const snap = boardSnapshot(
    [
      card({
        id: "n",
        projectId: "A",
        kind: "needed",
        neededState: "open",
        createdAt: 1,
        title: "Need",
      }),
      card({
        id: "t",
        projectId: "A",
        kind: "todo",
        status: "todo",
        createdAt: 2,
        title: "Task",
      }),
    ],
    "A",
    normalizeCols(undefined),
  );
  assert.deepEqual(
    snap.map((c) => c.id),
    ["needed", "todo"],
  );
});

test("H2 Needed always shows", () => {
  const snap = boardSnapshot(
    [
      card({
        id: "n",
        projectId: "A",
        kind: "needed",
        neededState: "open",
        createdAt: 1,
      }),
    ],
    "A",
    [],
  );
  assert.deepEqual(
    snap.map((c) => c.id),
    ["needed"],
  );
  assert.deepEqual(normalizeCols(["needed", "done"]), ["done"]);
  assert.deepEqual(normalizeCols([]), []);
  const cols = ["todo", "doing"];
  assert.deepEqual(toggleCol(cols, "needed"), ["todo", "doing"]);
  assert.deepEqual(cols, ["todo", "doing"]);
});

test("H3 Needed rule", () => {
  const cards = [
    card({
      id: "later",
      projectId: "A",
      kind: "needed",
      neededState: "later",
      createdAt: 1,
      lastTouchedAt: 5,
    }),
    card({
      id: "open",
      projectId: "A",
      kind: "needed",
      neededState: "open",
      createdAt: 9,
      lastTouchedAt: 9,
    }),
    card({
      id: "answered",
      projectId: "A",
      kind: "needed",
      neededState: "answered",
      createdAt: 2,
    }),
    card({
      id: "other",
      projectId: "B",
      kind: "needed",
      neededState: "open",
      createdAt: 1,
    }),
  ];
  const snap = boardSnapshot(cards, "A", []);
  const needed = snap.find((c) => c.id === "needed");
  assert.ok(needed);
  assert.deepEqual(
    needed.items.map((c) => c.id),
    ["open", "later"],
  );
  const empty = boardSnapshot(cards, "C", ["todo"]);
  assert.equal(
    empty.some((c) => c.id === "needed"),
    false,
  );
});

test("H4 user picks columns", () => {
  assert.deepEqual(toggleCol(["doing"], "todo"), ["todo", "doing"]);
  assert.deepEqual(toggleCol(["todo", "doing"], "doing"), ["todo"]);
  assert.deepEqual(normalizeCols(["done", "todo", "todo"]), ["todo", "done"]);
});

test("H5 column contents", () => {
  const cards: HabitatCard[] = [
    card({
      id: "need-doing",
      projectId: "A",
      kind: "needed",
      neededState: "open",
      status: "doing",
      createdAt: 1,
    }),
    card({
      id: "other-doing",
      projectId: "B",
      kind: "window",
      status: "doing",
      createdAt: 1,
      windowId: "wb",
    }),
    card({
      id: "d-late",
      projectId: "A",
      kind: "window",
      status: "doing",
      createdAt: 10,
      windowId: "w1",
    }),
    card({
      id: "d-early",
      projectId: "A",
      kind: "todo",
      status: "doing",
      createdAt: 4,
    }),
    card({ id: "c1", projectId: "A", kind: "todo", status: "done", completedAt: 1, createdAt: 1 }),
    card({ id: "c2", projectId: "A", kind: "todo", status: "done", completedAt: 5, createdAt: 2 }),
    card({ id: "c3", projectId: "A", kind: "todo", status: "done", completedAt: 3, createdAt: 3 }),
    card({ id: "c4", projectId: "A", kind: "todo", status: "done", completedAt: 4, createdAt: 4 }),
    card({ id: "c5", projectId: "A", kind: "todo", status: "done", completedAt: 2, createdAt: 5 }),
    card({ id: "loose", projectId: "A", kind: "todo", status: "todo", createdAt: 3 }),
    card({ id: "loose-doing", projectId: "A", kind: "todo", status: "doing", createdAt: 8 }),
    card({ id: "loose-done", projectId: "A", kind: "todo", status: "done", completedAt: 9, createdAt: 1 }),
    card({ id: "linked", projectId: "A", kind: "window", status: "todo", windowId: "w9", createdAt: 1 }),
    card({ id: "b-loose", projectId: "B", kind: "todo", status: "todo", createdAt: 1 }),
  ];
  const snap = boardSnapshot(cards, "A", ["doing", "done"], 4);
  const doing = snap.find((c) => c.id === "doing");
  const done = snap.find((c) => c.id === "done");
  assert.ok(doing && done);
  assert.deepEqual(
    doing.items.map((c) => c.id),
    ["d-early", "loose-doing", "d-late"],
  );
  assert.equal(doing.more, 0);
  assert.equal(doing.items.some((c) => c.id === "d-late" && c.windowId === "w1"), true);
  assert.deepEqual(
    done.items.map((c) => c.id),
    ["loose-done", "c2", "c4", "c3"],
  );
  assert.equal(done.more, 2);
  assert.equal(done.items.length, 4);
  assert.equal(planetCount(cards, "A"), 3);
});

test("H6 raise restores and ends on frontmost", () => {
  const plan = raisePlan([
    { id: "w1", isMinimized: true, isFrontmost: false },
    { id: "w2", isMinimized: false, isFrontmost: true },
    { id: "w3", isMinimized: false, isFrontmost: false },
  ]);
  assert.deepEqual(plan.restore, ["w1"]);
  assert.deepEqual(plan.focus, ["w1", "w3", "w2"]);
});

test("H7 raise skips asleep and caps", () => {
  const one = raisePlan([
    { id: "w1", sleeping: true, isMinimized: true },
    { id: "w2", isMinimized: false, isFrontmost: false },
  ]);
  assert.deepEqual(one.focus, ["w2"]);
  assert.deepEqual(one.restore, []);

  const rows = Array.from({ length: 20 }, (_, i) => ({
    id: `w${i}`,
    isMinimized: false,
    isFrontmost: i === 19,
  }));
  const many = raisePlan(rows);
  assert.equal(many.focus.length, 12);
  assert.equal(many.focus[many.focus.length - 1], "w19");
  assert.equal(many.restore.length, 0);
});

test("H8 tiles overflow", () => {
  const rows = [
    { id: "a", isMinimized: false },
    { id: "b", isMinimized: true },
    { id: "c", sleeping: true, isMinimized: true },
    { id: "d" },
    { id: "e" },
    { id: "f", isMinimized: true },
    { id: "g" },
    { id: "h" },
    { id: "i", sleeping: true },
  ];
  const view = tiles(rows, 6);
  assert.equal(view.shown.length, 6);
  assert.equal(view.more, 3);
  assert.deepEqual(
    view.shown.map((t) => t.state),
    ["open", "stowed", "asleep", "open", "open", "stowed"],
  );
  assert.deepEqual(
    view.shown.map((t) => t.id),
    ["a", "b", "c", "d", "e", "f"],
  );
});

test("H9 ring order", () => {
  const stops = habitatStops(ring);
  assert.deepEqual(
    stops.map((s) => s.id),
    [
      "rstars",
      "rsettings",
      "rcol-todo",
      "rcol-doing",
      "rcol-done",
      "rhead-A",
      "rraise-A",
      "rtile-a1",
      "rtile-a2",
      "rsnap-n1",
      "rsnap-t1",
      "rtodos-A",
      "rhead-B",
    ],
  );
  assert.deepEqual(
    stops.map((s) => s.group),
    ["front", "front", "cols", "cols", "cols", "A", "A", "A", "A", "A", "A", "A", "B"],
  );
  assert.equal(
    stops.some((s) => s.id === "rtile-b1" || s.id === "rsnap-bx" || s.id === "rtodos-B" || s.id === "rraise-B"),
    false,
  );
});

test("H10 arrows", () => {
  assert.equal(habitatArrow(ring, "rhead-A", "down"), "rraise-A");
  assert.equal(habitatArrow(ring, "rraise-A", "down"), "rtile-a1");
  assert.equal(habitatArrow(ring, "rtile-a1", "up"), "rraise-A");
  assert.equal(habitatArrow(ring, "rtodos-A", "down"), "rhead-B");
  assert.equal(habitatArrow(ring, "rhead-B", "up"), "rtodos-A");
  assert.equal(habitatArrow(ring, "rtile-a1", "right"), "rtile-a2");
  assert.equal(habitatArrow(ring, "rtile-a2", "right"), null);
  assert.equal(habitatArrow(ring, "rhead-B", "down"), null);
  assert.equal(habitatArrow(ring, "rsnap-n1", "right"), "rsnap-t1");
  assert.equal(habitatArrow(ring, "rsnap-t1", "right"), null);
});

test("E1 head confirm is expand", () => {
  assert.equal(habitatConfirm("rhead-A"), "expand");
  assert.notEqual(habitatConfirm("rhead-A"), "raise");
  assert.equal(habitatConfirm("rcol-todo"), "toggleCol");
  assert.equal(habitatConfirm("rtile-a1"), "focusTile");
  assert.equal(habitatConfirm("rsnap-n1"), "openSnap");
  assert.equal(habitatConfirm("rtodos-A"), "todos");
  assert.equal(habitatConfirm("rhead-"), null);
  assert.equal(habitatConfirm("nope"), null);
});

test("E2 raise confirm is raise", () => {
  assert.equal(habitatConfirm("rraise-A"), "raise");
  assert.notEqual(habitatConfirm("rraise-A"), "expand");
  assert.equal(habitatConfirm("rraise-"), null);
});

test("E3 raise stop only when open", () => {
  const stops = habitatStops(ring);
  const ids = stops.map((s) => s.id);
  const head = ids.indexOf("rhead-A");
  assert.ok(head >= 0);
  assert.equal(ids[head + 1], "rraise-A");
  assert.equal(ids[head + 2], "rtile-a1");
  assert.deepEqual(
    stops.filter((s) => s.group === "B").map((s) => s.id),
    ["rhead-B"],
  );
});

test("E4 arrows through raise", () => {
  assert.equal(habitatArrow(ring, "rhead-A", "down"), "rraise-A");
  assert.equal(habitatArrow(ring, "rraise-A", "down"), "rtile-a1");
  assert.equal(habitatArrow(ring, "rtile-a1", "up"), "rraise-A");

  const snapOnly: HabitatSection[] = [
    { pid: "A", open: true, tileIds: [], snapCols: [["n1"]], hasTodos: false },
  ];
  assert.equal(habitatArrow(snapOnly, "rraise-A", "down"), "rsnap-n1");
  assert.equal(habitatArrow(snapOnly, "rsnap-n1", "up"), "rraise-A");

  const todosOnly: HabitatSection[] = [
    { pid: "A", open: true, tileIds: [], snapCols: [], hasTodos: true },
  ];
  assert.equal(habitatArrow(todosOnly, "rraise-A", "down"), "rtodos-A");
  assert.equal(habitatArrow(todosOnly, "rtodos-A", "up"), "rraise-A");
});
