import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  archLabelBox,
  archMarbleBox,
  archOrbitArc,
  archOrbitPath,
  archOrbitPoint,
  archOrbitSamples,
  archPainted,
  archLayout,
  archSlot,
  ARCH_IDLE,
  mirrorLayout,
  neededAct,
  thumbShowsImage,
  type ArchFrame,
} from "../src/lib/rail/stageArch.ts";
import type { ZoneRect } from "../src/lib/rail/stageLook.ts";

const edges = ["left", "right", "top", "bottom"] as const;
const frames: ArchFrame[] = [
  { w: 892, h: 760 },
  { w: 1040, h: 760 },
  { w: 1040, h: 752 },
];
const css = readFileSync(new URL("../src/lib/rail/rail.css", import.meta.url), "utf8");
const page = readFileSync(new URL("../src/lib/RailPage.svelte", import.meta.url), "utf8");
const list = readFileSync(new URL("../src/lib/rail/List.svelte", import.meta.url), "utf8");

function hit(a: ZoneRect, b: ZoneRect): boolean {
  return a.x < b.x + b.w - 0.01 && a.x + a.w > b.x + 0.01 && a.y < b.y + b.h - 0.01 && a.y + a.h > b.y + 0.01;
}

function gap(a: ZoneRect, b: ZoneRect): number {
  const dx = Math.max(a.x - (b.x + b.w), b.x - (a.x + a.w), 0);
  const dy = Math.max(a.y - (b.y + b.h), b.y - (a.y + a.h), 0);
  return Math.hypot(dx, dy);
}

function centre(box: ZoneRect): { x: number; y: number } {
  return { x: box.x + box.w / 2, y: box.y + box.h / 2 };
}

function framePoint(edge: string, slot: { x: number; y: number }, frame: ArchFrame): { x: number; y: number } {
  if (edge === "right") return { x: frame.w + slot.x, y: frame.h / 2 + slot.y };
  if (edge === "top") return { x: frame.w / 2 + slot.x, y: slot.y };
  if (edge === "bottom") return { x: frame.w / 2 + slot.x, y: frame.h + slot.y };
  return { x: slot.x, y: frame.h / 2 + slot.y };
}

test("P1 labels sit tight and do not overlap", () => {
  const archHit = css.slice(css.indexOf(".rail.stage-shell .arch-hit {"), css.indexOf(".rail.stage-shell .arch-hit {") + 400);
  assert.match(archHit, /position:\s*absolute/);
  assert.doesNotMatch(archHit, /flex-direction:\s*column/);
  assert.match(css, /\.arch-name\s*\{[^}]*position:\s*absolute/);
  for (const edge of edges) {
    for (const frame of frames) {
      for (const ring of [1, 2] as const) {
        const expanded = mirrorLayout(edge, frame.w, frame.h).expanded;
        const avoid = [expanded];
        const labels = new Map<number, ZoneRect>();
        const marbles = new Map<number, ZoneRect>();
        for (let k = -4; k <= 4; k++) {
          if (archSlot(k, edge, ring, frame).hidden) continue;
          const marble = archMarbleBox(k, edge, ring, frame, k === 0);
          const slot = archSlot(k, edge, ring, frame);
          const pt = framePoint(edge, slot, frame);
          const c = centre(marble);
          assert.ok(Math.abs(c.x - pt.x) <= 1 && Math.abs(c.y - pt.y) <= 1, `${edge} r${ring} k=${k} marble`);
          const label = archLabelBox(k, edge, ring, frame, { avoid });
          assert.ok(label.side === "below" || label.side === "inboard");
          const g = gap(marble, label);
          assert.ok(g <= 6.01 && g > 0, `${edge} ${frame.w} r${ring} k=${k} gap ${g}`);
          labels.set(k, label);
          marbles.set(k, marble);
        }
        for (const [k, label] of labels) {
          for (const [kk, marble] of marbles) {
            if (k !== kk) assert.ok(!hit(label, marble), `${edge} r${ring} label ${k} marble ${kk}`);
          }
          for (const [kk, other] of labels) {
            if (k !== kk) assert.ok(!hit(label, other), `${edge} r${ring} labels ${k}/${kk}`);
          }
          if (ring === 1) {
            assert.ok(!hit(label, expanded), `${edge} ${frame.w} label ${k} expanded`);
          }
        }
      }
    }
  }
});

test("P2 orbit is a smooth arc through the screen edge", () => {
  for (const edge of edges) {
    for (const frame of frames) {
      const arc = archOrbitArc(edge, 1, frame);
      assert.equal(archOrbitPath(edge, 1, frame), arc.d);
      assert.match(arc.d, /A[\d.]+ [\d.]+ 0 1 1 /);
      const pts = archOrbitSamples(arc, 160);
      let prev = 0;
      for (let i = 1; i < pts.length - 1; i++) {
        const a = Math.atan2(pts[i].y - pts[i - 1].y, pts[i].x - pts[i - 1].x);
        const b = Math.atan2(pts[i + 1].y - pts[i].y, pts[i + 1].x - pts[i].x);
        let turn = Math.abs(b - a);
        if (turn > Math.PI) turn = 2 * Math.PI - turn;
        assert.ok(turn <= (4 * Math.PI) / 180 + 1e-6, `${edge} turn ${turn}`);
        prev = turn;
      }
      assert.ok(prev >= 0);
      for (let k = -4; k <= 4; k++) {
        const slot = archSlot(k, edge, 1, frame);
        let best = Infinity;
        for (let i = 0; i <= 720; i++) {
          const t = arc.t0 + ((arc.t1 - arc.t0) * i) / 720;
          const p = archOrbitPoint(arc, t);
          best = Math.min(best, Math.hypot(p.x - slot.x, p.y - slot.y));
        }
        assert.ok(best <= 1, `${edge} k=${k} ${best}`);
      }
      const tail = archOrbitPoint(arc, arc.t0);
      const tailB = archOrbitPoint(arc, arc.t1);
      if (edge === "left") {
        assert.ok(tail.x <= 0.5 && tailB.x <= 0.5, `left tails ${tail.x} ${tailB.x}`);
      } else if (edge === "right") {
        assert.ok(tail.x >= -0.5 && tailB.x >= -0.5, `right tails ${tail.x}`);
      } else if (edge === "top") {
        assert.ok(tail.y <= 0.5 && tailB.y <= 0.5, `top tails ${tail.y}`);
      } else {
        assert.ok(tail.y >= -0.5 && tailB.y >= -0.5, `bottom tails ${tail.y}`);
      }
    }
  }
  assert.match(list, /stage-orbit-fade/);
  assert.match(list, /stop-opacity="0"/);
});

test("P3 unmapped uses the expanded rect", () => {
  assert.doesNotMatch(css, /inset:\s*48px/);
  assert.match(list, /class="layer2-unmapped" style=\{rectStyle\(mirror\.expanded\)\}/);
  for (const edge of edges) {
    for (const frame of frames) {
      const lay = mirrorLayout(edge, frame.w, frame.h);
      const painted = archPainted(
        archLayout(edge, frame.w, frame.h),
        { x: lay.expanded.x + 8, y: lay.expanded.y + 8 },
        { front: "layer2", pid: "__unmapped__", revealed: true, edge, frame },
      );
      assert.equal(painted, true);
      for (let k = -4; k <= 4; k++) {
        const slot = archSlot(k, edge, 1, frame);
        if (slot.hidden) continue;
        const marble = archMarbleBox(k, edge, 1, frame, k === 0);
        assert.ok(!hit(marble, lay.expanded), `${edge} k=${k} marble in expanded`);
      }
    }
  }
  assert.ok(ARCH_IDLE);
});

test("P4 stars embed beside the strip", () => {
  const start = page.indexOf("function openStageDest");
  const body = page.slice(start, start + 700);
  assert.match(body, /stageEmbed = "settings"/);
  assert.match(body, /stageEmbed = "stars"/);
  assert.match(body, /dest_show/);
  assert.match(body, /stars-beside/);
  const own = page.slice(start, page.indexOf("function popOutStars"));
  assert.doesNotMatch(own, /dest_show/);
  assert.match(list, /<StarsPage[\s\S]*embedded/);
  assert.match(list, /data-stage-stars/);
  assert.match(page, /function popOutStars\(\) \{[\s\S]*rail_stage_set[\s\S]*stars-beside/);
});

test("P5 needed click opens the mouth", () => {
  const act = neededAct({ kind: "needed", id: "n1", title: "call back" });
  assert.deepEqual(act, { kind: "mouth", id: "n1", title: "call back" });
  assert.equal(neededAct({ kind: "window", id: "w", title: "x" }), null);
  assert.match(list, /arm_companion_mouth|armNeededMouth/);
  assert.match(list, /item\.kind === "needed"\) rail\.armNeededMouth/);
  assert.match(page, /arm_companion_mouth/);
  assert.match(list, />Answer</);
});

test("P6 a raise parks and grace is retried", () => {
  assert.match(page, /reason:\s*"raised"/);
  assert.match(page, /function scheduleGraceRecheck/);
  assert.match(page, /revealGraceUntil/);
  const listen = page.slice(page.indexOf("orbitty:rail-stage"));
  assert.match(listen.slice(0, 900), /source === "edge"/);
  assert.match(listen.slice(0, 900), /pointerInside = true/);
  assert.match(listen.slice(0, 900), /EDGE_GRACE_MS/);
  assert.match(page, /edgePointerSeed = false/);
});

test("P7 thumbnails only for a centred layer-2 window", () => {
  assert.equal(thumbShowsImage("ok"), true);
  assert.equal(thumbShowsImage("denied"), false);
  assert.match(list, /class="l2-thumb"/);
  assert.match(list, /Screen Recording needed for previews/);
  const fx = list.slice(list.indexOf("$effect(() => {"), list.indexOf("$effect(() => {") + 900);
  assert.match(fx, /front !== "layer2"/);
  assert.match(fx, /window_thumb/);
  assert.doesNotMatch(fx, /persist\(|save_state|localStorage/);
  assert.match(list, /data-win-preview/);
});

test("P8 stage shadows are light and strokes are gone", () => {
  const shell = css.slice(css.indexOf(".rail.stage-shell {"));
  const blocks = shell.split(/\}/g);
  for (const block of blocks) {
    if (!/box-shadow\s*:/.test(block)) continue;
    if (/0\s+0\s+0\s+\d/.test(block)) continue;
    const decls = block.match(/box-shadow\s*:[^;]+/g) ?? [];
    for (const decl of decls) {
      if (/var\(--stage-shadow\)/.test(decl)) continue;
      if (/none/.test(decl)) continue;
      const alphas = [...decl.matchAll(/\/\s*(0?\.\d+|\d+)\)|,\s*(0?\.\d+)\s*\)/g)].map((m) => Number(m[1] ?? m[2]));
      for (const a of alphas) assert.ok(a <= 0.14, decl);
      const blurs = [...decl.matchAll(/(-?\d+(?:\.\d+)?)px/g)].map((m) => Number(m[1]));
      if (blurs.length >= 3) assert.ok(blurs[2] <= 16, decl);
    }
  }
  const hit = shell.slice(shell.indexOf(".l2-hit {"), shell.indexOf(".l2-hit {") + 500);
  assert.match(hit, /border:\s*0/);
  assert.doesNotMatch(hit, /border:\s*0\.5px/);
  assert.match(shell, /--stage-shadow:\s*0 4px 14px rgb\(0 0 0 \/ 0\.12\)/);
});
