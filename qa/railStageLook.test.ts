import assert from "node:assert/strict";
import { test } from "node:test";
import {
  habitatStops,
  wheelArrow,
  type HabitatDir,
  type HabitatSection,
  type RailEdge,
} from "../src/lib/rail/habitat.ts";
import {
  PIN_H,
  SELECTED_SCALE,
  TILE_H,
  TILE_W,
  WHEEL_COL,
  orderedSlots,
  paintedZoneHit,
  pinnedLook,
  wheelCenter,
  wheelLayout,
  wheelMotion,
  wheelSlot,
} from "../src/lib/rail/stageLook.ts";

const ids = ["a", "b", "c"];
const edges: RailEdge[] = ["left", "right", "top", "bottom"];
const dirs: HabitatDir[] = ["up", "down", "left", "right"];

function axis(edge: RailEdge): { back: HabitatDir; forward: HabitatDir; inward: HabitatDir } {
  if (edge === "right") return { back: "up", forward: "down", inward: "left" };
  if (edge === "top") return { back: "left", forward: "right", inward: "down" };
  if (edge === "bottom") return { back: "left", forward: "right", inward: "up" };
  return { back: "up", forward: "down", inward: "right" };
}

test("W1 open planet is centre", () => {
  assert.equal(wheelCenter(ids, "b", "rhead-a", 0), ids.indexOf("b"));
  assert.equal(wheelCenter(["a", "b", "c"], "c", "rhead-a", 0), 2);
});

test("W2 highlight steps wheel", () => {
  assert.equal(wheelCenter(ids, null, "rhead-c", 0), 2);
  assert.equal(wheelCenter(ids, null, "rraise-c", 1), 2);
  assert.equal(wheelCenter(ids, null, "rtodos-c", 0), 2);
  assert.equal(wheelCenter(ids, null, "rstars", 1), 1);
  assert.equal(wheelCenter(ids, null, "rsettings", 2), 2);
  assert.equal(wheelCenter(ids, null, "rcol-x", 1), 1);
  assert.equal(wheelCenter(ids, null, null, 2), 2);
  assert.equal(wheelCenter(ids, null, "rstars", 9), 2);
  assert.equal(wheelCenter(ids, null, null, -4), 0);
  assert.equal(wheelCenter([], null, "rhead-a", 3), 0);
});

test("W3 selected emphasis", () => {
  const centre = wheelSlot(0);
  assert.ok(centre.scale >= 1.45 && centre.scale <= 1.55);
  assert.equal(centre.scale, SELECTED_SCALE);
  assert.equal(centre.opacity, 1);
  assert.equal(centre.hidden, false);
  let topZ = true;
  const xs: number[] = [];
  for (let k = -3; k <= 3; k++) {
    const slot = wheelSlot(k, "left");
    xs.push(slot.x);
    if (k !== 0) {
      assert.ok(centre.z > slot.z);
      topZ = topZ && centre.z > slot.z;
    }
  }
  assert.equal(topZ, true);
  assert.equal(Math.max(...xs), centre.x);
});

test("W4 falloff", () => {
  for (const k of [1, 2, 3]) {
    const outer = wheelSlot(k);
    const inner = wheelSlot(k - 1);
    assert.ok(outer.scale < inner.scale);
    assert.ok(outer.opacity < inner.opacity);
    assert.equal(outer.scale, wheelSlot(-k).scale);
    assert.equal(outer.opacity, wheelSlot(-k).opacity);
    assert.equal(outer.hidden, false);
  }
  assert.equal(wheelSlot(4).hidden, true);
  assert.equal(wheelSlot(-5).hidden, true);
  assert.equal(wheelSlot(4).opacity, 0);
});

test("W5 follows the curve", () => {
  const ys = [-3, -2, -1, 0, 1, 2, 3].map((k) => wheelSlot(k, "left").y);
  for (let i = 1; i < ys.length; i++) assert.ok(ys[i] > ys[i - 1]);
  assert.equal(wheelSlot(0, "left").rotate, 0);
  assert.ok(wheelSlot(1, "left").rotate > 0);
  assert.ok(wheelSlot(-2, "left").rotate < 0);
  assert.equal(Math.sign(wheelSlot(3, "left").rotate), 1);
  assert.equal(Math.sign(wheelSlot(-1, "left").rotate), -1);
  for (const k of [1, 2, 3]) {
    assert.ok(wheelSlot(k, "left").x < wheelSlot(k - 1, "left").x);
    assert.ok(Math.abs(wheelSlot(k, "left").x - wheelSlot(-k, "left").x) < 1e-9);
  }
});

test("W6 edge mirroring", () => {
  const left = wheelSlot(1, "left");
  const right = wheelSlot(1, "right");
  assert.ok(Math.abs(right.x + left.x) < 1e-9);
  assert.equal(Math.sign(right.rotate), -Math.sign(left.rotate));
  assert.notEqual(Math.sign(right.rotate), Math.sign(left.rotate));
  const top = wheelSlot(1, "top");
  const bottom = wheelSlot(1, "bottom");
  assert.ok(Math.abs(top.x - left.y) < 1e-9);
  assert.ok(Math.abs(top.y - left.x) < 1e-9);
  assert.ok(Math.abs(bottom.x - left.y) < 1e-9);
  assert.ok(Math.abs(bottom.y + left.x) < 1e-9);
  assert.equal(Math.sign(bottom.rotate), -Math.sign(left.rotate));
});

test("W7 pinned never recedes", () => {
  for (const edge of edges) {
    const pin = pinnedLook(edge);
    assert.equal(pin.scale, 1);
    assert.equal(pin.opacity, 1);
    assert.equal(pin.rotate, 0);
    const receded = wheelSlot(2, edge);
    assert.notEqual(pin.scale, receded.scale);
    assert.notEqual(pin.opacity, receded.opacity);
    assert.notEqual(pin.rotate, receded.rotate);
  }
});

test("W8 motion + reduce-motion", () => {
  const full = wheelMotion(false);
  assert.ok(full.durationMs >= 200 && full.durationMs <= 300);
  assert.notEqual(full.swingDeg, 0);
  const reduce = wheelMotion(true);
  assert.ok(reduce.durationMs <= 100);
  assert.equal(reduce.swingDeg, 0);
});

test("W9 order preserved", () => {
  const names = ["m", "a", "z", "b"];
  for (const center of [0, 1, 2, 3]) {
    const rows = orderedSlots(names, center, "left");
    assert.deepEqual(
      rows.map((r) => r.id),
      names,
    );
    for (let i = 1; i < rows.length; i++) assert.ok(rows[i].y > rows[i - 1].y);
  }
});

test("W10 full-screen layout", () => {
  const frameW = 1432;
  const frameH = 875;
  const left = wheelLayout("left", frameW, frameH);
  assert.ok(left.preview.x >= left.wheel.x + left.wheel.w);
  assert.ok(frameW - (left.preview.x + left.preview.w) <= 48);
  assert.ok(left.preview.y <= 48);
  assert.ok(frameH - (left.preview.y + left.preview.h) <= 48);
  assert.ok(left.preview.w > WHEEL_COL);
  assert.ok(left.pinned.y + left.pinned.h <= left.wheel.y);
  assert.equal(left.wheel.y, PIN_H);

  const right = wheelLayout("right", frameW, frameH);
  assert.ok(right.wheel.x > left.wheel.x);
  assert.ok(right.preview.x + right.preview.w <= right.wheel.x);
  assert.ok(right.pinned.y + right.pinned.h <= right.wheel.y);
  assert.ok(frameW - (right.wheel.x + right.wheel.w) <= 48);

  const top = wheelLayout("top", frameW, frameH);
  assert.ok(top.preview.y >= top.wheel.y + top.wheel.h);
  assert.ok(frameH - (top.preview.y + top.preview.h) <= 48);

  const bottom = wheelLayout("bottom", frameW, frameH);
  assert.ok(bottom.preview.y + bottom.preview.h <= bottom.wheel.y);
  assert.ok(bottom.wheel.y > top.wheel.y);

  for (const layout of [left, right, top, bottom]) {
    const tile = layout.selected;
    assert.ok(Math.abs(tile.w - TILE_W * 1.5) < 1e-9);
    assert.ok(Math.abs(tile.h - TILE_H * 1.5) < 1e-9);
    assert.ok(tile.x >= 0 && tile.y >= 0);
    assert.ok(tile.x + tile.w <= frameW + 1e-6);
    assert.ok(tile.y + tile.h <= frameH + 1e-6);
  }
});

test("W11 tile size band", () => {
  assert.ok(TILE_W >= 170 && TILE_W <= 200);
  assert.ok(TILE_H >= 56 && TILE_H <= 68);
});

test("W12 painted-zone hit", () => {
  const layout = wheelLayout("left", 1432, 875);
  assert.equal(
    paintedZoneHit({ x: layout.wheel.x + 8, y: layout.wheel.y + 8 }, layout, false),
    true,
  );
  assert.equal(
    paintedZoneHit({ x: layout.pinned.x + 4, y: layout.pinned.y + 4 }, layout, false),
    true,
  );
  const previewPoint = { x: layout.preview.x + 12, y: layout.preview.y + 12 };
  assert.equal(paintedZoneHit(previewPoint, layout, true), true);
  assert.equal(paintedZoneHit(previewPoint, layout, false), false);
  const gap = { x: layout.wheel.x + layout.wheel.w + 4, y: layout.preview.y + 20 };
  assert.equal(paintedZoneHit(gap, layout, true), false);
  assert.equal(paintedZoneHit({ x: 1420, y: 8 }, layout, true), false);
});

const noneOpen: HabitatSection[] = [
  { pid: "A", open: false, tileIds: ["a1"], snapCols: [["n1"]], hasTodos: true },
  { pid: "B", open: false, tileIds: ["b1"], snapCols: [], hasTodos: false },
];

const oneOpen: HabitatSection[] = [
  {
    pid: "A",
    open: true,
    tileIds: ["a1", "a2"],
    snapCols: [["n1"], ["t1"]],
    hasTodos: true,
  },
  { pid: "B", open: false, tileIds: ["b1"], snapCols: [["bx"]], hasTodos: true },
];

const snapOnly: HabitatSection[] = [
  { pid: "A", open: true, tileIds: [], snapCols: [["n1"]], hasTodos: false },
];

const todosOnly: HabitatSection[] = [
  { pid: "A", open: true, tileIds: [], snapCols: [], hasTodos: true },
];

function reached(
  sections: readonly HabitatSection[],
  edge: RailEdge,
  start: string,
): Set<string> {
  const ids = new Set(habitatStops(sections).map((s) => s.id));
  const seen = new Set<string>();
  const queue = [start];
  while (queue.length > 0) {
    const cur = queue.pop();
    if (!cur || seen.has(cur)) continue;
    seen.add(cur);
    for (const dir of dirs) {
      const next = wheelArrow(sections, cur, dir, edge);
      if (next && ids.has(next) && !seen.has(next)) queue.push(next);
    }
  }
  return seen;
}

test("W13 keyboard reachability", () => {
  const packs = [noneOpen, oneOpen, snapOnly, todosOnly];
  for (const edge of edges) {
    const keys = axis(edge);
    for (const sections of packs) {
      const stops = habitatStops(sections).map((s) => s.id);
      const fromStars = reached(sections, edge, "rstars");
      for (const id of stops) assert.equal(fromStars.has(id), true, `${edge} ${id}`);
      for (const id of stops) {
        assert.equal(reached(sections, edge, id).has("rstars"), true, `${edge} back ${id}`);
      }
    }
    assert.equal(wheelArrow(oneOpen, "rhead-A", keys.forward, edge), "rhead-B");
    assert.notEqual(wheelArrow(oneOpen, "rhead-A", keys.forward, edge), "rraise-A");
    assert.equal(wheelArrow(oneOpen, "rhead-A", keys.inward, edge), "rraise-A");
    assert.equal(wheelArrow(noneOpen, "rhead-A", keys.inward, edge), null);
  }
});
