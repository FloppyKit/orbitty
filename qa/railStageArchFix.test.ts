import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  ARCH_FRAME,
  ARCH_ITEM_BOX,
  ARCH_SPAN_DEG,
  CLAIM_FLASH_MS,
  EDGE_GRACE_MS,
  EDGE_HIDE_MS,
  EDGE_SLOP,
  STAGE_INK,
  STAGE_INK_DIM,
  STAGE_PLATTER_RGBA,
  archCentreChanged,
  archEnter,
  archLayout,
  archOrbitPath,
  archPainted,
  archPoint,
  archSlot,
  archZone,
  claimLanded,
  mirrorLayout,
  previewCardOuter,
  previewCardScale,
  wheelZone,
  edgeHover,
  edgeLeaveSide,
  stageContrast,
  type ArchFrame,
  type ArchState,
  type EdgeHoverState,
  type Layer2Item,
} from "../src/lib/rail/stageArch.ts";
import { wheelLayout, type ZoneRect } from "../src/lib/rail/stageLook.ts";
import { svgPathPoints } from "./svgPath.ts";

function railSrc() {
  const names = ["pageRows", "pageStage", "pagePointer", "pageRing", "pageClaim", "pageBeing", "pageDrag"];
  const parts = names.map((n) => readFileSync(new URL(`../src/lib/rail/${n}.svelte.ts`, import.meta.url), "utf8"));
  parts.push(readFileSync(new URL("../src/lib/RailPage.svelte", import.meta.url), "utf8"));
  return parts.join("");
}

function railCss() {
  const sheet = readFileSync(new URL("../src/lib/rail/rail.css", import.meta.url), "utf8");
  const names = [...sheet.matchAll(/@import\s+"\.\/([^"]+)"/g)].map((m) => m[1]);
  return names.map((name) => readFileSync(new URL(`../src/lib/rail/${name}`, import.meta.url), "utf8")).join("");
}

const STEP = (ARCH_SPAN_DEG / 4) * (Math.PI / 180);
const edges = ["left", "right", "top", "bottom"] as const;
const frames: ArchFrame[] = [
  { w: 1040, h: 760 },
  { w: 892, h: 760 },
  { w: 1000, h: 700 },
  { w: 900, h: 600 },
];

function fromEdge(edge: string, slot: { x: number; y: number }): number {
  if (edge === "right") return -slot.x;
  if (edge === "top") return slot.y;
  if (edge === "bottom") return -slot.y;
  return slot.x;
}

function framePoint(
  edge: string,
  slot: { x: number; y: number },
  frame: ArchFrame,
): { x: number; y: number } {
  if (edge === "right") return { x: frame.w + slot.x, y: frame.h / 2 + slot.y };
  if (edge === "top") return { x: frame.w / 2 + slot.x, y: slot.y };
  if (edge === "bottom") return { x: frame.w / 2 + slot.x, y: frame.h + slot.y };
  return { x: slot.x, y: frame.h / 2 + slot.y };
}

function boxOf(c: { x: number; y: number }, w: number, h: number): ZoneRect {
  return { x: c.x - w / 2, y: c.y - h / 2, w, h };
}

function contains(outer: ZoneRect, inner: ZoneRect): boolean {
  return (
    inner.x >= outer.x - 1e-6 &&
    inner.y >= outer.y - 1e-6 &&
    inner.x + inner.w <= outer.x + outer.w + 1e-6 &&
    inner.y + inner.h <= outer.y + outer.h + 1e-6
  );
}

function pathPoints(d: string): { x: number; y: number }[] {
  const nums = d.match(/-?\d+(?:\.\d+)?/g)?.map(Number) ?? [];
  const pts: { x: number; y: number }[] = [];
  for (let i = 0; i + 1 < nums.length; i += 2) pts.push({ x: nums[i], y: nums[i + 1] });
  return pts;
}

function nearest(pts: { x: number; y: number }[], p: { x: number; y: number }): number {
  let best = Infinity;
  for (const q of pts) {
    const d = Math.hypot(q.x - p.x, q.y - p.y);
    if (d < best) best = d;
  }
  return best;
}

test("F1 edge placement", () => {
  for (const edge of edges) {
    for (const frame of frames) {
      const centre = archSlot(0, edge, 1, frame);
      const dist = fromEdge(edge, centre);
      assert.ok(dist >= 150 && dist <= 240, `${edge} centre ${dist}`);
      const ring2 = fromEdge(edge, archSlot(0, edge, 2, frame));
      assert.ok(ring2 >= 360 && ring2 <= 540, `${edge} ring2 ${ring2}`);
      for (let k = -4; k <= 4; k++) {
        const slot = archSlot(k, edge, 1, frame);
        assert.equal(slot.hidden, false);
        const d = fromEdge(edge, slot);
        assert.ok(d >= 40 && d <= 300, `${edge} k=${k} ${d}`);
        const pt = framePoint(edge, slot, frame);
        if (edge === "left" || edge === "right") {
          assert.ok(Math.abs(pt.x - frame.w / 2) > 120, `${edge} ${frame.w} k=${k}`);
        } else {
          assert.ok(Math.abs(pt.y - frame.h / 2) > 120, `${edge} ${frame.h} k=${k}`);
        }
      }
    }
  }
});

test("F2 fits", () => {
  const side = ARCH_ITEM_BOX * 1.18 * 1.06;
  const l2w = 148 * 1.06;
  const l2h = 64 * 1.06;
  for (const edge of edges) {
    for (const frame of frames) {
      for (let k = -4; k <= 4; k++) {
        for (const ring of [1, 2] as const) {
          const slot = archSlot(k, edge, ring, frame);
          assert.equal(slot.hidden, false);
          const c = framePoint(edge, slot, frame);
          const b = ring === 1 ? boxOf(c, side, side) : boxOf(c, l2w, l2h);
          assert.ok(contains({ x: 0, y: 0, w: frame.w, h: frame.h }, b), `${edge} ${frame.w}x${frame.h} r${ring} k=${k}`);
        }
      }
    }
  }
});

test("F3 on the line", () => {
  for (const edge of edges) {
    for (const frame of frames) {
      for (const ring of [1, 2] as const) {
        const d = archOrbitPath(edge, ring, frame);
        const pts = svgPathPoints(d);
        assert.ok(d.length > 0);
        assert.ok(pts.length >= 16, `${edge} points ${pts.length}`);
        const edgeSlot = archSlot(4, edge, ring, frame);
        const along = edge === "top" || edge === "bottom" ? "x" : "y";
        const reach = Math.max(...pts.map((p) => Math.abs(p[along])));
        assert.ok(reach > Math.abs(edgeSlot[along]) + 0.5, `${edge} r${ring} reach`);
        for (let k = -4; k <= 4; k++) {
          const slot = archSlot(k, edge, ring, frame);
          const pt = archPoint(k * STEP, edge, ring, frame);
          assert.ok(Math.hypot(slot.x - pt.x, slot.y - pt.y) <= 0.5);
          assert.ok(nearest(pts, { x: slot.x, y: slot.y }) <= 1, `${edge} r${ring} k=${k}`);
        }
      }
    }
  }
  assert.ok(svgPathPoints(archOrbitPath("left", 1, ARCH_FRAME)).length >= 16);
});

test("F4 zones match drawn items", () => {
  const side = ARCH_ITEM_BOX * 1.18 * 1.06;
  const l2w = 148 * 1.06;
  const l2h = 64 * 1.06;
  for (const edge of ["left", "right", "top"] as const) {
    for (const frame of [
      [1040, 760],
      [1000, 700],
    ] as const) {
      const lay = archLayout(edge, frame[0], frame[1]);
      const preview = wheelLayout(edge, frame[0], frame[1]).preview;
      for (const rect of [lay.arch, lay.layer2, lay.board]) {
        assert.ok(rect.x >= -1e-6 && rect.y >= -1e-6);
        assert.ok(rect.x + rect.w <= frame[0] + 1e-6);
        assert.ok(rect.y + rect.h <= frame[1] + 1e-6);
      }
      assert.deepEqual(lay.board, preview);
      if (edge === "left") assert.ok(lay.layer2.x >= lay.arch.x + lay.arch.w - 1e-6);
      if (edge === "right") assert.ok(lay.layer2.x + lay.layer2.w <= lay.arch.x + 1e-6);
      if (edge === "top") assert.ok(lay.layer2.y >= lay.arch.y + lay.arch.h - 1e-6);
    }
  }
  for (const edge of edges) {
    for (const frame of frames) {
      const lay = archLayout(edge, frame.w, frame.h);
      for (let k = -4; k <= 4; k++) {
        const s1 = archSlot(k, edge, 1, frame);
        const c1 = framePoint(edge, s1, frame);
        const b1 = boxOf(c1, side, side);
        assert.ok(contains(lay.arch, b1), `${edge} arch k=${k}`);
        assert.equal(archZone(lay, c1, "arch"), "arch");
        assert.equal(archZone(lay, c1, "layer2"), "arch");
        const s2 = archSlot(k, edge, 2, frame);
        const c2 = framePoint(edge, s2, frame);
        const b2 = boxOf(c2, l2w, l2h);
        assert.ok(contains(lay.layer2, b2), `${edge} layer2 k=${k}`);
        assert.equal(archZone(lay, c2, "layer2"), "layer2");
      }
    }
  }
});

test("F5 painted, no dead strip", () => {
  const frame = { w: 1040, h: 760 };
  const lay = archLayout("left", frame.w, frame.h);
  assert.ok(Math.abs(lay.arch.x + lay.arch.w - lay.layer2.x) < 1e-6, "arch meets layer2");
  const outer = lay.layer2.x + lay.layer2.w;
  const heights = [40, 200, 380, 560, 720];
  for (const y of heights) {
    for (let x = 0; x < outer - 1; x += 4) {
      assert.equal(
        archPainted(lay, { x, y }, { front: "layer2", pid: "p", revealed: true }),
        true,
        `${x},${y}`,
      );
    }
    for (let x = 2; x < lay.arch.w - 1; x += 4) {
      assert.equal(
        archPainted(lay, { x, y }, { front: "arch", pid: null, revealed: true }),
        true,
      );
    }
  }
  const boardOnly = { x: outer + EDGE_SLOP + 8, y: 400 };
  assert.equal(archPainted(lay, boardOnly, { front: "board", pid: "p", revealed: true }), true);
  assert.equal(archPainted(lay, boardOnly, { front: "arch", pid: "p", revealed: true }), false);
  assert.equal(archPainted(lay, boardOnly, { front: "layer2", pid: null, revealed: true }), false);
  const grown = { x: lay.arch.w + EDGE_SLOP - 1, y: 400 };
  const past = { x: lay.arch.w + EDGE_SLOP + 2, y: 400 };
  assert.equal(archPainted(lay, grown, { front: "arch", pid: null, revealed: true }), true);
  assert.equal(archPainted(lay, past, { front: "arch", pid: null, revealed: true }), false);
});

test("F6 edge hysteresis", () => {
  assert.ok(EDGE_HIDE_MS >= 450 && EDGE_HIDE_MS <= 800);
  assert.ok(EDGE_GRACE_MS >= 700 && EDGE_GRACE_MS <= 1500);
  const parked: EdgeHoverState = { revealed: false, revealedAt: 0, outsideSince: null };
  const t = 10_000;
  const revealed = edgeHover(parked, { kind: "move", painted: false }, t);
  assert.equal(revealed.act, "reveal");
  assert.equal(revealed.state.revealed, true);
  assert.equal(revealed.state.revealedAt, t);

  let s = revealed.state;
  s = edgeHover(s, { kind: "leave", side: "edge" }, t + 10).state;
  for (let now = t + 10; now <= t + 3000; now += 100) {
    const tick = edgeHover(s, { kind: "tick" }, now);
    assert.equal(tick.act, null);
    s = tick.state;
  }

  const fresh = edgeHover(parked, { kind: "move", painted: true }, t).state;
  const outside = edgeHover(fresh, { kind: "move", painted: false }, t + 1000);
  assert.equal(outside.act, null);
  assert.equal(outside.state.outsideSince, t + 1000);
  const early = edgeHover(outside.state, { kind: "tick" }, t + 1000 + EDGE_HIDE_MS - 1);
  assert.equal(early.act, null);
  const parkedAct = edgeHover(outside.state, { kind: "tick" }, t + 1000 + EDGE_HIDE_MS);
  assert.equal(parkedAct.act, "park");

  const back = edgeHover(outside.state, { kind: "move", painted: true }, t + 1000 + 50);
  assert.equal(back.state.outsideSince, null);
  assert.equal(edgeHover(back.state, { kind: "tick" }, t + 1000 + EDGE_HIDE_MS).act, null);

  const left = edgeHover(fresh, { kind: "leave", side: "away" }, t + 100);
  assert.equal(left.act, null);
  assert.equal(edgeHover(left.state, { kind: "tick" }, t + EDGE_GRACE_MS - 1).act, null);

  let flap = fresh;
  for (let i = 0; i < 20; i++) {
    const at = t + i * 100;
    const leave = edgeHover(flap, { kind: "leave", side: "edge" }, at);
    assert.equal(leave.act, null);
    const again = edgeHover(leave.state, { kind: "move", painted: true }, at + 40);
    assert.notEqual(again.act, "park");
    flap = again.state;
  }

  assert.equal(edgeLeaveSide("left", 0, 10, 800, 600), "edge");
  assert.equal(edgeLeaveSide("left", -4, 10, 800, 600), "edge");
  assert.equal(edgeLeaveSide("left", 12, 10, 800, 600), "away");
  assert.equal(edgeLeaveSide("right", 800, 10, 800, 600), "edge");
  assert.equal(edgeLeaveSide("right", 790, 10, 800, 600), "away");
  assert.equal(edgeLeaveSide("top", 10, 0, 800, 600), "edge");
  assert.equal(edgeLeaveSide("top", 10, 8, 800, 600), "away");
  assert.equal(edgeLeaveSide("bottom", 10, 600, 800, 600), "edge");
  assert.equal(edgeLeaveSide("bottom", 10, 580, 800, 600), "away");
  assert.equal(edgeLeaveSide("nope", -1, 10, 800, 600), "edge");
  void CLAIM_FLASH_MS;
});

test("F7 select keeps layer 2", () => {
  const layer2: Layer2Item[] = [
    { kind: "board", stop: "rl2-board", title: "Open board" },
    { kind: "needed", stop: "rl2-n", id: "n", title: "Need" },
  ];
  const entered = archEnter({ front: "arch", pid: null, center2: 0, pending: null }, {
    stop: "rhead-b",
    kind: "planet",
    pid: "b",
  }, layer2);
  const kept = archCentreChanged(entered, { stop: "rhead-b", kind: "planet", pid: "b" });
  assert.equal(kept.front, "layer2");
  assert.equal(kept.pid, "b");
  const frame = { w: 1040, h: 760 };
  const lay = archLayout("left", frame.w, frame.h);
  const l2w = 148 * 1.06;
  const l2h = 64 * 1.06;
  for (let k = -4; k <= 4; k++) {
    const c = framePoint("left", archSlot(k, "left", 2, frame), frame);
    const b = boxOf(c, l2w, l2h);
    for (let x = b.x + 1; x < b.x + b.w - 1; x += 12) {
      for (let y = b.y + 1; y < b.y + b.h - 1; y += 12) {
        assert.equal(
          archPainted(lay, { x, y }, { front: "layer2", pid: "b", revealed: true }),
          true,
          `${k} ${x},${y}`,
        );
      }
    }
  }
  const held: ArchState = { front: "layer2", pid: "a", center2: 2, pending: null };
  const stars = archEnter(held, { stop: "rstars", kind: "stars" });
  assert.deepEqual(stars, held);
  assert.notEqual(stars, held);
});

test("F8 claim target", () => {
  const a = { windowId: "w1", projectId: "a", lastTouchedAt: 10 };
  const b = { windowId: "w2", projectId: "b", lastTouchedAt: 20 };
  assert.equal(claimLanded(null, [a]), null);
  assert.deepEqual(claimLanded([], [a]), { windowId: "w1", projectId: "a" });
  assert.deepEqual(claimLanded([a], [{ ...a, projectId: "b", lastTouchedAt: 30 }]), {
    windowId: "w1",
    projectId: "b",
  });
  assert.deepEqual(claimLanded([], [a, { ...b, lastTouchedAt: 5 }]), { windowId: "w1", projectId: "a" });
  assert.deepEqual(claimLanded([], [{ ...a, lastTouchedAt: 1 }, b]), { windowId: "w2", projectId: "b" });
  assert.equal(claimLanded([a, b], [a]), null);
  assert.equal(claimLanded([a, b], [a, b]), null);
});

test("F9 legibility", () => {
  assert.ok(stageContrast(STAGE_INK, STAGE_PLATTER_RGBA, "white") >= 4.5);
  assert.ok(stageContrast(STAGE_INK, STAGE_PLATTER_RGBA, "black") >= 4.5);
  assert.ok(stageContrast(STAGE_INK_DIM, STAGE_PLATTER_RGBA, "white") >= 4.5);
  assert.ok(stageContrast(STAGE_INK_DIM, STAGE_PLATTER_RGBA, "black") >= 4.5);
  const css = railCss();
  for (const chunk of css.split("}")) {
    if (!chunk.includes(".arch-platter") && !chunk.includes(".layer2-unmapped")) continue;
    assert.equal(chunk.includes("light-dark("), false, chunk.slice(0, 120));
  }
});

test("F10 wiring guards", () => {
  const rail = railSrc();
  const list = readFileSync(new URL("../src/lib/rail/List.svelte", import.meta.url), "utf8");
  const css = railCss();
  const over = rail.slice(rail.indexOf("function overPainted"), rail.indexOf("function overPainted") + 700);
  assert.match(over, /archPainted\(/);
  assert.equal(over.includes("paintedZoneHit"), false);
  const dest = rail.slice(rail.indexOf("function openStageDest"), rail.indexOf("function openStageDest") + 400);
  assert.match(dest, /dest_show/);
  assert.equal(dest.includes("toggle_labeled"), false);
  const hitAt = list.indexOf('class="stage-hit"');
  const winBranch = list.indexOf("rail.windows.length === 0");
  assert.ok(hitAt >= 0 && winBranch >= 0);
  assert.ok(hitAt < winBranch, "stage-hit sits inside the windows-only branch");
  assert.match(css, /\.arch-slot:hover \.arch-vis[\s\S]*?transform/);
  assert.equal(/\.arch-slot:hover\s*[,{][^}]*transform\s*:/.test(css), false);
  const gear = list.slice(list.indexOf("snippet gearGlyph"), list.indexOf("snippet gearGlyph") + 900);
  const path = gear.match(/d="([^"]+)"/);
  assert.ok(path);
  const verts = path[1].match(/-?\d+(?:\.\d+)?/g) ?? [];
  assert.ok(verts.length >= 32, `gear vertices ${verts.length / 2}`);
  assert.equal(list.includes("persist("), false);
  assert.equal(list.includes("save_state"), false);
});

test("wheelZone is the front layer anywhere inside the frame", () => {
  const frame = { w: 1040, h: 760 };
  const lay = archLayout("left", frame.w, frame.h);
  const archPt = { x: lay.arch.x + 1, y: frame.h / 2 };
  const gap = { x: lay.layer2.x + lay.layer2.w + 12, y: 8 };
  assert.equal(archZone(lay, archPt, "layer2"), "arch");
  assert.equal(archZone(lay, gap, "layer2"), "none");
  assert.equal(wheelZone(frame, archPt, "layer2", false), "layer2");
  assert.equal(wheelZone(frame, gap, "layer2", false), "layer2");
  assert.equal(wheelZone(frame, gap, "arch", false), "arch");
  assert.equal(wheelZone(frame, { x: -1, y: 10 }, "layer2", false), "none");
  assert.equal(wheelZone(frame, { x: frame.w, y: 10 }, "arch", false), "none");
  assert.equal(wheelZone(frame, { x: 10, y: 10 }, "board", false), "none");
  assert.equal(wheelZone(frame, archPt, "layer2", true), "none");
});

test("preview card is 1.5x and stays inside the beside panel", () => {
  const expanded = mirrorLayout("left", ARCH_FRAME.w, ARCH_FRAME.h).expanded;
  const box = previewCardScale(expanded);
  const outer = previewCardOuter(box);
  assert.equal(box.scale, 2);
  assert.equal(box.cardW, 600);
  assert.equal(box.thumbH, 320);
  assert.equal(box.icon, 192);
  assert.equal(box.pad, 56);
  assert.equal(box.gap, 32);
  assert.equal(box.font, 36);
  assert.ok(outer.w <= expanded.w, `width ${outer.w} > ${expanded.w}`);
  assert.ok(outer.h <= expanded.h, `height ${outer.h} > ${expanded.h}`);
  assert.ok(box.cardW >= 300);
  assert.ok(box.thumbH >= 160);
  assert.ok(box.icon >= 96);
  const css = readFileSync(new URL("../src/lib/rail/rail-arch.css", import.meta.url), "utf8");
  const card = css.slice(css.indexOf(".l2-window-card {"), css.indexOf(".l2-needed-card"));
  assert.match(card, /max-width:\s*var\(--pv-card,\s*600px\)/);
  assert.match(card, /padding:\s*var\(--pv-pad,\s*56px\)/);
  assert.match(card, /gap:\s*var\(--pv-gap,\s*32px\)/);
  assert.match(card, /font:\s*600 var\(--pv-font,\s*36px\)/);
  assert.match(css, /\.l2-thumb \{[\s\S]*?max-height:\s*var\(--pv-thumb,\s*320px\)/);
  assert.match(css, /\.l2-card-icon,[\s\S]*?\.l2-app-icon \{[\s\S]*?width:\s*var\(--pv-icon,\s*192px\);[\s\S]*?height:\s*var\(--pv-icon,\s*192px\);/);
  assert.match(css, /\.arch-slot \.l2-app-icon \{[\s\S]*?width:\s*44px;[\s\S]*?height:\s*44px;/);
  assert.match(css, /\.l2-slot \{[\s\S]*?width:\s*148px;[\s\S]*?height:\s*64px;/);
  assert.match(css, /\.l2-hit \{[\s\S]*?width:\s*148px;\s*height:\s*64px;/);
});

test("preview card is min(2x, fit) beside the rail", () => {
  const big = mirrorLayout("left", 1040, 760).expanded;
  const two = previewCardScale(big);
  const twoOuter = previewCardOuter(two);
  assert.equal(two.scale, 2);
  assert.ok(twoOuter.w <= big.w);
  assert.ok(twoOuter.h <= big.h);
  const mid = mirrorLayout("left", 892, 760).expanded;
  const midBox = previewCardScale(mid);
  const midOuter = previewCardOuter(midBox);
  assert.ok(midBox.scale < 2);
  assert.ok(midOuter.w <= mid.w);
  assert.ok(midOuter.h <= mid.h);
  const small = mirrorLayout("left", 400, 300).expanded;
  const fit = previewCardScale(small);
  const fitOuter = previewCardOuter(fit);
  assert.ok(fit.scale < 2);
  assert.ok(fit.scale < midBox.scale);
  assert.ok(fitOuter.w <= small.w);
  assert.ok(fitOuter.h <= small.h);
  assert.ok(fit.cardW < two.cardW);
  const list = readFileSync(new URL("../src/lib/rail/List.svelte", import.meta.url), "utf8");
  assert.match(list, /previewCardScale\(expanded\)/);
  assert.match(list, /style=\{previewVars\(mirror\.expanded\)\}/);
});
