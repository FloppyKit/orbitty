import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  archArrow,
  archConfirm,
  archEnter,
  archOrbitPath,
  archPainted,
  archLayout,
  archSlot,
  ARCH_IDLE,
  mirrorDepth,
  mirrorItems,
  mirrorLayout,
  mirrorSlot,
  mirrorStart,
  type ArchCard,
  type ArchFrame,
  type ArchRow,
  type ArchState,
  type Layer2Item,
} from "../src/lib/rail/stageArch.ts";
import { spinItems, type SpinItem, type ZoneRect } from "../src/lib/rail/stageLook.ts";

function card(partial: ArchCard): ArchCard {
  return partial;
}

function overlap(a: ZoneRect, b: ZoneRect): boolean {
  return a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;
}

function pathPoints(d: string): { x: number; y: number }[] {
  const pts: { x: number; y: number }[] = [];
  const re = /[ML](-?\d+(?:\.\d+)?) (-?\d+(?:\.\d+)?)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(d))) pts.push({ x: Number(m[1]), y: Number(m[2]) });
  return pts;
}

function segDist(
  p: { x: number; y: number },
  a: { x: number; y: number },
  b: { x: number; y: number },
): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len2 = dx * dx + dy * dy;
  if (len2 < 1e-12) return Math.hypot(p.x - a.x, p.y - a.y);
  let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2;
  t = Math.max(0, Math.min(1, t));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

function distToPath(p: { x: number; y: number }, pts: { x: number; y: number }[]): number {
  let best = Infinity;
  for (let i = 1; i < pts.length; i++) best = Math.min(best, segDist(p, pts[i - 1], pts[i]));
  return best;
}

const edges = ["left", "right", "top", "bottom"] as const;

test("M1 mirrorItems is board, needed, then every window including asleep", () => {
  const pid = "p";
  const cards: ArchCard[] = [
    card({ id: "n-open", projectId: pid, kind: "needed", neededState: "open", createdAt: 2, title: "Ask", neededWhy: "because" }),
    card({ id: "n-later", projectId: pid, kind: "needed", neededState: "later", createdAt: 1, title: "Later" }),
    card({ id: "n-done", projectId: pid, kind: "needed", neededState: "answered", createdAt: 3, title: "Done" }),
    card({ id: "d1", projectId: pid, kind: "todo", status: "doing", createdAt: 1, title: "Doing" }),
    card({ id: "t1", projectId: pid, kind: "todo", status: "todo", createdAt: 2, title: "Todo" }),
  ];
  const rows: ArchRow[] = Array.from({ length: 12 }, (_, i) => ({
    id: `w${i}`,
    title: `W${i}`,
    sleeping: i % 3 === 0,
  }));
  const items = mirrorItems({ pid, cards, rows });
  assert.equal(items[0].kind, "board");
  assert.equal(items[0].stop, "rl2-board");
  const needed = items.filter((it) => it.kind === "needed");
  assert.deepEqual(needed.map((n) => n.id), ["n-later", "n-open"]);
  assert.equal(needed[1].why, "because");
  const windows = items.filter((it) => it.kind === "window");
  assert.equal(windows.length, 12);
  assert.deepEqual(windows.map((w) => w.id), rows.map((r) => r.id));
  assert.equal(windows[0].sleeping, true);
  assert.equal(items.some((it) => it.kind === "doing" || it.kind === "todo"), false);
  assert.equal(mirrorStart(items), 0);
});

test("M2 layer-2 slots sit on ring 1 and on the orbit", () => {
  const frames: ArchFrame[] = [
    { w: 892, h: 760 },
    { w: 1040, h: 760 },
    { w: 1040, h: 752 },
  ];
  for (const edge of edges) {
    for (const frame of frames) {
      const d = archOrbitPath(edge, 1, frame);
      const pts = pathPoints(d);
      assert.ok(pts.length > 4, edge);
      for (let i = 0; i < 9; i++) {
        const center = 3;
        const slot = mirrorSlot(i, center, edge, frame);
        const expect = archSlot(i - center, edge, 1, frame);
        assert.ok(Math.hypot(slot.x - expect.x, slot.y - expect.y) <= 0.5, `${edge} k`);
        const ring2 = archSlot(i - center, edge, 2, frame);
        assert.ok(Math.hypot(slot.x - ring2.x, slot.y - ring2.y) > 1, "not ring 2");
        if (!slot.hidden) {
          assert.ok(distToPath({ x: slot.x, y: slot.y }, pts) <= 1, `${edge} line`);
        }
      }
    }
  }
});

test("M3 expanded clears the arch and meets the size floors", () => {
  function boxes(edge: string, frame: ArchFrame): ZoneRect[] {
    const out: ZoneRect[] = [];
    for (let k = -4; k <= 4; k++) {
      const slot = archSlot(k, edge, 1, frame);
      if (slot.hidden) continue;
      const size = 88 * slot.scale * 1.06;
      const c = framePoint(edge, slot, frame);
      out.push({ x: c.x - size / 2, y: c.y - size / 2, w: size, h: size });
    }
    return out;
  }
  const checks: [string, number, number, number, number][] = [
    ["left", 892, 760, 560, 600],
    ["right", 892, 760, 560, 600],
    ["left", 1040, 760, 560, 600],
    ["right", 1040, 760, 560, 600],
    ["top", 1040, 752, 900, 440],
    ["bottom", 1040, 752, 900, 440],
  ];
  for (const [edge, w, h, minW, minH] of checks) {
    const frame = { w, h };
    const m = mirrorLayout(edge, w, h);
    assert.ok(m.expanded.w >= minW && m.expanded.h >= minH, `${edge} ${m.expanded.w}x${m.expanded.h}`);
    for (const box of boxes(edge, frame)) {
      assert.equal(overlap(m.expanded, box), false, `${edge} expanded`);
      assert.equal(overlap(m.back, box), false, `${edge} back`);
    }
  }
});

function framePoint(edge: string, slot: { x: number; y: number }, frame: ArchFrame) {
  if (edge === "right") return { x: frame.w + slot.x, y: frame.h / 2 + slot.y };
  if (edge === "top") return { x: frame.w / 2 + slot.x, y: slot.y };
  if (edge === "bottom") return { x: frame.w / 2 + slot.x, y: frame.h + slot.y };
  return { x: slot.x, y: frame.h / 2 + slot.y };
}

test("M4 enter centres Board; arrows spin layer 2; none reach front board", () => {
  const items: Layer2Item[] = [
    { kind: "board", stop: "rl2-board", title: "Board" },
    { kind: "needed", stop: "rl2-n-1", id: "n", title: "Need" },
    { kind: "window", stop: "rl2-w-a", id: "a", title: "Asleep", sleeping: true },
    { kind: "window", stop: "rl2-w-b", id: "b", title: "Min", minimized: true },
    { kind: "window", stop: "rl2-w-c", id: "c", title: "Open" },
  ];
  const planets = spinItems(["p"], "__unmapped__");
  const centre = planets.findIndex((it) => it.kind === "planet");
  const entered = archEnter(ARCH_IDLE, planets[centre] as SpinItem, items);
  assert.equal(entered.front, "layer2");
  assert.equal(entered.center2, 0);
  assert.equal(items[entered.center2].kind, "board");
  let state: ArchState = entered;
  const forward = archArrow(state, planets, centre, "down", "left", items);
  assert.ok(forward);
  assert.equal(forward.state.front, "layer2");
  assert.equal(forward.state.center2, 1);
  assert.equal(items[forward.state.center2].kind, "needed");
  state = forward.state;
  const back = archArrow({ ...state, center2: 0 }, planets, centre, "up", "left", items);
  assert.ok(back);
  assert.equal(back.state.front, "arch");
  const end: ArchState = { front: "layer2", pid: "p", center2: items.length - 1, pending: null };
  const stuck = archArrow(end, planets, centre, "down", "left", items);
  assert.ok(stuck);
  assert.equal(stuck.state.center2, items.length - 1);
  assert.equal(stuck.state.front, "layer2");
  for (const dir of ["up", "down", "left", "right"]) {
    for (let i = 0; i < items.length; i++) {
      const s: ArchState = { front: "layer2", pid: "p", center2: i, pending: null };
      const next = archArrow(s, planets, centre, dir, "left", items);
      if (next) assert.notEqual(next.state.front, "board");
    }
  }
  assert.equal(archConfirm(items[2]), "wake");
  assert.equal(archConfirm(items[3]), "restore");
  assert.equal(archConfirm(items[4]), "focus");
  const rail = readFileSync(new URL("../src/lib/RailPage.svelte", import.meta.url), "utf8");
  const applyAt = rail.indexOf("const applyArrow");
  const apply = rail.slice(applyAt, applyAt + 900);
  assert.equal(apply.includes("popOutBoard"), false);
});

test("M5 expanded and back are painted; old misses stay misses", () => {
  const frame = { w: 1040, h: 760 };
  const lay = archLayout("left", frame.w, frame.h);
  const m = mirrorLayout("left", frame.w, frame.h);
  const opts = {
    front: "layer2" as const,
    pid: "p",
    revealed: true,
    edge: "left",
    frame,
  };
  for (const zone of [m.expanded, m.back]) {
    const samples = [
      { x: zone.x + 1, y: zone.y + 1 },
      { x: zone.x + zone.w / 2, y: zone.y + zone.h / 2 },
      { x: zone.x + zone.w - 2, y: zone.y + zone.h - 2 },
    ];
    for (const point of samples) {
      if (point.x < 0 || point.y < 0) continue;
      assert.equal(archPainted(lay, point, opts), true, `${point.x},${point.y}`);
    }
  }
  const mid = { x: lay.board.x + lay.board.w / 2, y: lay.board.y + lay.board.h / 2 };
  assert.equal(archPainted(lay, mid, { front: "arch", pid: null, revealed: true, edge: "left", frame }), false);
  assert.equal(archPainted(lay, mid, { front: "layer2", pid: null, revealed: true, edge: "left", frame }), false);
  assert.equal(archPainted(lay, mid, { front: "arch", pid: "p", revealed: true, edge: "left", frame }), false);
  const arch = readFileSync(new URL("../src/lib/rail/stageArch.ts", import.meta.url), "utf8");
  assert.match(arch, /zones\.push\(mirror\.expanded, mirror\.back\)/);
});

test("M6 hover does not switch fronts; reduced mirror depth is instant", () => {
  const rail = readFileSync(new URL("../src/lib/RailPage.svelte", import.meta.url), "utf8");
  const moveAt = rail.indexOf("function onStagePointerMove");
  const move = rail.slice(moveAt, rail.indexOf("function onStagePointerDown"));
  assert.equal(move.includes("pokeArch"), false);
  assert.equal(move.includes("archPointer"), false);
  const reduced = mirrorDepth("layer2", 1, true);
  assert.equal(reduced.blur, 0);
  assert.equal(reduced.durationMs, 0);
  assert.ok(reduced.scale < 1);
  assert.ok(reduced.shift >= 100);
  const front = mirrorDepth("layer2", 2, false);
  assert.equal(front.scale, 1);
  assert.equal(front.shift, 0);
});

test("M7 orbit is above the scrim and stage chrome is gone", () => {
  const list = readFileSync(new URL("../src/lib/rail/List.svelte", import.meta.url), "utf8");
  const css = readFileSync(new URL("../src/lib/rail/rail.css", import.meta.url), "utf8");
  const chrome = readFileSync(new URL("../src/lib/rail/Chrome.svelte", import.meta.url), "utf8");
  const arch = readFileSync(new URL("../src/lib/rail/stageArch.ts", import.meta.url), "utf8");
  assert.equal(list.includes("stage-stacks"), false);
  assert.match(list, /class="stage-orbit"/);
  assert.match(list, /width=\{frameW\}/);
  assert.match(list, /height=\{frameH\}/);
  const orbitAt = list.indexOf('class="stage-orbit"');
  const slotsAt = list.indexOf('class="arch-list"');
  assert.ok(orbitAt >= 0 && slotsAt > orbitAt);
  const between = list.slice(orbitAt, slotsAt);
  assert.equal(between.includes("backdrop-filter"), false);
  assert.match(css, /\.stage-orbit path[\s\S]*stroke:\s*rgb\(255 255 255 \/ 0\.55\)/);
  assert.match(css, /systemLight[\s\S]*\.stage-orbit path[\s\S]*rgb\(0 0 0 \/ 0\.38\)/);
  assert.equal(chrome.includes("wordmark"), false);
  assert.match(chrome, /\{#if rail\.embedded\}/);
  assert.equal(chrome.includes("{:else}"), false);
  for (const chunk of css.split("}")) {
    if (!chunk.includes(".arch-platter") && !chunk.includes(".layer2-platter")) continue;
    assert.equal(/\bborder\s*:/.test(chunk), false, chunk.slice(0, 80));
    assert.equal(chunk.includes("box-shadow"), false);
    assert.equal(chunk.includes("backdrop-filter"), false);
    assert.equal(chunk.includes("0.78"), false);
  }
  const hit = css.slice(css.indexOf(".rail.stage-shell .stage-hit"), css.indexOf(".rail.stage-shell .stage-hit") + 280);
  assert.match(hit, /background:\s*transparent/);
  assert.equal(hit.includes("rgb(28 28 32"), false);
  assert.equal(arch.includes("persist"), false);
  assert.equal(arch.includes("save_state"), false);
  assert.equal(list.includes("persist("), false);
  assert.equal(list.includes("save_state"), false);
});

test("M8 parked pointer does not reveal; edge source seeds grace", () => {
  const rail = readFileSync(new URL("../src/lib/RailPage.svelte", import.meta.url), "utf8");
  const moveAt = rail.indexOf("function onStagePointerMove");
  const move = rail.slice(moveAt, rail.indexOf("function onStagePointerDown"));
  assert.equal(move.includes("showStage"), false);
  assert.match(move, /stageRevealed/);
  const listenAt = rail.indexOf('listen<{ revealed?: boolean; source?: string }>("orbitty:rail-stage"');
  assert.ok(listenAt >= 0);
  const listen = rail.slice(listenAt, listenAt + 700);
  assert.match(listen, /source === "edge"/);
  assert.match(listen, /pointerInside = true/);
  assert.match(listen, /EDGE_GRACE_MS/);
  const hideAt = rail.indexOf("function maybeAutoHide");
  const hide = rail.slice(hideAt, hideAt + 320);
  assert.match(hide, /revealGraceUntil/);
  assert.match(hide, /EDGE_GRACE_MS|revealGraceUntil/);
});
