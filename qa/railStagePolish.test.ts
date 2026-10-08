import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  archLabelBox,
  archMarbleBox,
  archOrbitArc,
  archOrbitPath,
  archPainted,
  archLayout,
  archSlot,
  ARCH_IDLE,
  L2_RING,
  mirrorLayout,
  neededAct,
  thumbShowsImage,
  type ArchFrame,
} from "../src/lib/rail/stageArch.ts";
import type { ZoneRect } from "../src/lib/rail/stageLook.ts";
import { archCssChain, stripComments } from "./cssChain.ts";
import { svgPathPoints } from "./svgPath.ts";

const edges = ["left", "right", "top", "bottom"] as const;
const frames: ArchFrame[] = [
  { w: 892, h: 760 },
  { w: 1040, h: 760 },
  { w: 1040, h: 752 },
];
const css = ["rail-chrome.css", "rail-rows.css", "rail-stage.css", "rail-arch.css"].map((name) => readFileSync(new URL(`../src/lib/rail/${name}`, import.meta.url), "utf8")).join("");
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

function maxTurnDeg(pts: { x: number; y: number }[]): number {
  let prev: number | null = null;
  let worst = 0;
  for (let i = 1; i < pts.length; i++) {
    const dx = pts[i].x - pts[i - 1].x;
    const dy = pts[i].y - pts[i - 1].y;
    if (dx * dx + dy * dy < 0.25 * 0.25) continue;
    const h = Math.atan2(dy, dx);
    if (prev != null) {
      let turn = Math.abs(h - prev);
      if (turn > Math.PI) turn = 2 * Math.PI - turn;
      if (turn > worst) worst = turn;
    }
    prev = h;
  }
  return (worst * 180) / Math.PI;
}

/** Brace-matched body starting at the first `{` at or after `from`. */
function braceBody(src: string, from: number): string {
  const open = src.indexOf("{", from);
  if (open < 0) return "";
  let depth = 0;
  let quote: string | null = null;
  for (let i = open; i < src.length; i++) {
    const c = src[i];
    if (quote) {
      if (c === "\\") {
        i++;
        continue;
      }
      if (c === quote) quote = null;
      continue;
    }
    if (c === "'" || c === '"' || c === "`") {
      quote = c;
      continue;
    }
    if (c === "{") depth++;
    else if (c === "}") {
      depth--;
      if (depth === 0) return src.slice(open, i + 1);
    }
  }
  return "";
}

function splitArgs(inner: string): string[] {
  const args: string[] = [];
  let cur = "";
  let depth = 0;
  let quote: string | null = null;
  for (let i = 0; i < inner.length; i++) {
    const c = inner[i];
    if (quote) {
      cur += c;
      if (c === "\\") {
        cur += inner[++i] ?? "";
        continue;
      }
      if (c === quote) quote = null;
      continue;
    }
    if (c === "'" || c === '"' || c === "`") {
      quote = c;
      cur += c;
      continue;
    }
    if (c === "(" || c === "{" || c === "[") {
      depth++;
      cur += c;
      continue;
    }
    if (c === ")" || c === "}" || c === "]") {
      depth--;
      cur += c;
      continue;
    }
    if (c === "," && depth === 0) {
      args.push(cur.trim());
      cur = "";
      continue;
    }
    cur += c;
  }
  if (cur.trim()) args.push(cur.trim());
  return args;
}

function callsTo(src: string, name: string): string[][] {
  const out: string[][] = [];
  const needle = `${name}(`;
  let i = 0;
  while (i < src.length) {
    const at = src.indexOf(needle, i);
    if (at < 0) break;
    const prev = at > 0 ? src[at - 1] : "";
    if (/[A-Za-z0-9_$]/.test(prev)) {
      i = at + needle.length;
      continue;
    }
    let depth = 1;
    let j = at + needle.length;
    let quote: string | null = null;
    for (; j < src.length && depth > 0; j++) {
      const c = src[j];
      if (quote) {
        if (c === "\\") {
          j++;
          continue;
        }
        if (c === quote) quote = null;
        continue;
      }
      if (c === "'" || c === '"' || c === "`") {
        quote = c;
        continue;
      }
      if (c === "(") depth++;
      else if (c === ")") depth--;
    }
    out.push(splitArgs(src.slice(at + needle.length, j - 1)));
    i = j;
  }
  return out;
}

function fnBody(src: string, name: string): string {
  const at = src.indexOf(`function ${name}`);
  assert.ok(at >= 0, `function ${name}`);
  return braceBody(src, at);
}

/** Ring is the identifier passed through. A literal 1 or 2 fails. */
function assertRingPassed(body: string, fn: string, where: string) {
  const calls = callsTo(body, fn);
  assert.ok(calls.length >= 1, `${where} calls ${fn}`);
  for (const args of calls) {
    assert.equal(args[2], "ring", `${where} ${fn} ring arg ${args[2]}`);
  }
}

function assertAvoidPassed(body: string, where: string) {
  assert.match(body, /const avoid\s*=\s*\[\s*mirror\.expanded\s*\]/, `${where} keeps avoid`);
  const calls = callsTo(body, "archLabelBox");
  assert.ok(calls.length >= 1, `${where} calls archLabelBox`);
  for (const args of calls) {
    assert.ok(
      args.some((a) => a.replace(/\s/g, "") === "{avoid}"),
      `${where} passes { avoid }`,
    );
  }
}

test("P1 labels sit tight and do not overlap", () => {
  const cssSrc = stripComments(css);
  const listSrc = stripComments(list);
  const chain = archCssChain(css);
  assert.ok(Math.abs(chain.dx) <= 1 && Math.abs(chain.dy) <= 1, `arch chain net (${chain.dx}, ${chain.dy})`);
  assert.equal(chain.visTranslate, false, "arch-vis positioning rule has no centring translate");
  assert.equal(chain.icons.length, 3);
  for (const icon of chain.icons) {
    assert.equal(icon.left, 0, `${icon.subject} left`);
    assert.equal(icon.top, 0, `${icon.subject} top`);
    assert.equal(icon.centred, true, `${icon.subject} translate(-50%, -50%)`);
    const off = Math.hypot(chain.dx + icon.left, chain.dy + icon.top);
    assert.ok(off <= 1, `${icon.subject} centre ${off}px from slot`);
  }
  const archAt = cssSrc.indexOf(".rail.stage-shell .arch-hit {");
  const archHit = cssSrc.slice(archAt, archAt + 400);
  assert.match(archHit, /position:\s*absolute/);
  assert.doesNotMatch(archHit, /flex-direction:\s*column/);
  assert.match(cssSrc, /\.arch-name\s*\{[^}]*position:\s*absolute/);
  const l2At = listSrc.indexOf("{#each layer2");
  const l2 = listSrc.slice(l2At, listSrc.indexOf("{/each}", l2At));
  assert.match(l2, /slotStyle\(l2Offset\(i\), L2_RING,/);
  assert.equal((l2.match(/labelSide\(l2Offset\(i\), L2_RING\)/g) ?? []).length, 2);
  assert.match(l2, /labelStyle\(l2Offset\(i\), L2_RING\)/);
  assert.doesNotMatch(l2, /(?:slotStyle|labelSide|labelStyle)\(l2Offset\(i\),\s*2\b/);
  assert.equal(L2_RING, 1);
  for (const edge of edges) {
    for (const frame of frames) {
      const expanded = mirrorLayout(edge, frame.w, frame.h).expanded;
      for (const ring of [1, 2] as const) {
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
          if (ring === L2_RING) {
            assert.ok(!hit(label, expanded), `${edge} ${frame.w} label ${k} expanded`);
          }
        }
      }
      const l2Labels = new Map<number, ZoneRect>();
      const l2Marbles = new Map<number, ZoneRect>();
      for (let k = -4; k <= 4; k++) {
        if (archSlot(k, edge, L2_RING, frame).hidden) continue;
        const marble = archMarbleBox(k, edge, L2_RING, frame, k === 0);
        const label = archLabelBox(k, edge, L2_RING, frame, { avoid: [expanded] });
        assert.ok(!hit(label, expanded), `${edge} ${frame.w}x${frame.h} l2 k=${k} expanded`);
        l2Labels.set(k, label);
        l2Marbles.set(k, marble);
      }
      for (const [k, label] of l2Labels) {
        for (const [kk, marble] of l2Marbles) {
          if (k !== kk) assert.ok(!hit(label, marble), `${edge} l2 label ${k} marble ${kk}`);
        }
        for (const [kk, other] of l2Labels) {
          if (k !== kk) assert.ok(!hit(label, other), `${edge} l2 labels ${k}/${kk}`);
        }
      }
    }
  }
  for (const layer of [
    { name: "layer1", ring: 1 as const },
    { name: "layer2", ring: L2_RING },
  ]) {
    for (const edge of edges) {
      for (const frame of frames) {
        const expanded = mirrorLayout(edge, frame.w, frame.h).expanded;
        for (let k = -4; k <= 4; k++) {
          const slot = archSlot(k, edge, layer.ring, frame);
          if (slot.hidden) continue;
          const box = archLabelBox(k, edge, layer.ring, frame, { avoid: [expanded] });
          const marble = archMarbleBox(k, edge, layer.ring, frame, k === 0);
          const pt = framePoint(edge, slot, frame);
          const lab = {
            x: pt.x + chain.dx + (box.x - (marble.x + marble.w / 2)),
            y: pt.y + chain.dy + (box.y - (marble.y + marble.h / 2)),
            w: box.w,
            h: box.h,
          };
          const where = `${layer.name} ${edge} ${frame.w}x${frame.h} k=${k}`;
          assert.ok(lab.x >= -0.01, `${where} x ${lab.x}`);
          assert.ok(lab.y >= -0.01, `${where} y ${lab.y}`);
          assert.ok(lab.x + lab.w <= frame.w + 0.01, `${where} right ${lab.x + lab.w}`);
          assert.ok(lab.y + lab.h <= frame.h + 0.01, `${where} bottom ${lab.y + lab.h}`);
          assert.ok(!hit(lab, expanded), `${where} expanded`);
        }
      }
    }
  }
  const styleBody = fnBody(listSrc, "labelStyle");
  const sideBody = fnBody(listSrc, "labelSide");
  assertRingPassed(styleBody, "archLabelBox", "labelStyle");
  assertRingPassed(styleBody, "archMarbleBox", "labelStyle");
  assertRingPassed(sideBody, "archLabelBox", "labelSide");
  assertAvoidPassed(styleBody, "labelStyle");
  assertAvoidPassed(sideBody, "labelSide");
  const beforeLabel = styleBody.slice(0, styleBody.indexOf("archLabelBox(k, edge, ring,"));
  assert.equal(beforeLabel.includes("ring ="), false, "ring is not reassigned before archLabelBox");
  assert.match(styleBody, /const avoid = \[mirror\.expanded\];/);
});

test("P2 orbit is a smooth arc through the screen edge", () => {
  const sampler = readFileSync(new URL("./svgPath.ts", import.meta.url), "utf8");
  assert.doesNotMatch(sampler, /stageArch/);
  assert.match(sampler, /perArc = 256/);
  for (const edge of edges) {
    for (const frame of frames) {
      for (const ring of [1, 2] as const) {
        const arc = archOrbitArc(edge, ring, frame);
        assert.equal(archOrbitPath(edge, ring, frame), arc.d);
        assert.match(arc.d, /A/);
        const drawn = /A([\d.]+) ([\d.]+) /.exec(arc.d);
        assert.ok(drawn);
        assert.ok(Math.abs(Number(drawn[1]) - arc.rx) < 1e-3, `${edge} rx field`);
        assert.ok(Math.abs(Number(drawn[2]) - arc.ry) < 1e-3, `${edge} ry field`);
        const pts = svgPathPoints(arc.d);
        assert.ok(pts.length >= 16, `${edge} r${ring} points`);
        const turn = maxTurnDeg(pts);
        assert.ok(turn <= 4, `${edge} r${ring} turn ${turn}`);
        for (let k = -4; k <= 4; k++) {
          const slot = archSlot(k, edge, ring, frame);
          const best = distToPath({ x: slot.x, y: slot.y }, pts);
          assert.ok(best <= 1, `${edge} ${frame.w}x${frame.h} r${ring} k=${k} ${best}`);
        }
        const tail = pts[0];
        const tailB = pts[pts.length - 1];
        if (ring === 1) {
          if (edge === "left") {
            assert.ok(tail.x <= 0.5 && tailB.x <= 0.5, `left tails ${tail.x} ${tailB.x}`);
          } else if (edge === "right") {
            assert.ok(tail.x >= -0.5 && tailB.x >= -0.5, `right tails ${tail.x} ${tailB.x}`);
          } else if (edge === "top") {
            assert.ok(tail.y <= 0.5 && tailB.y <= 0.5, `top tails ${tail.y} ${tailB.y}`);
          } else {
            assert.ok(tail.y >= -0.5 && tailB.y >= -0.5, `bottom tails ${tail.y} ${tailB.y}`);
          }
        }
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
  const own = page.slice(start, page.indexOf("function popOutStars"));
  assert.match(own, /applyBeside\(label\)/);
  assert.doesNotMatch(own, /stageEmbed\s*=/);
  assert.doesNotMatch(own, /dest_show/);
  const beside = page.slice(page.indexOf("function applyBeside"), page.indexOf("function walkInto"));
  assert.match(beside, /besideNext\(/);
  assert.match(beside, /stageEmbed = "settings"/);
  assert.match(beside, /stageEmbed = "stars"/);
  assert.match(beside, /clearArchToFront\(\)/);
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
  const pageSrc = stripComments(page);
  assert.match(pageSrc, /reason:\s*"raised"/);
  assert.match(pageSrc, /function scheduleGraceRecheck/);
  const hide = braceBody(pageSrc, pageSrc.indexOf("function maybeAutoHide"));
  const grace = braceBody(hide, hide.indexOf("revealGraceUntil"));
  const call = grace.indexOf("scheduleGraceRecheck()");
  const ret = grace.indexOf("return");
  assert.ok(call >= 0 && ret > call, "grace branch calls scheduleGraceRecheck() before return");
  const recheck = fnBody(pageSrc, "scheduleGraceRecheck");
  const recheckTimeout = recheck.indexOf("setTimeout(");
  assert.ok(recheckTimeout >= 0, "scheduleGraceRecheck has setTimeout");
  const recheckHead = recheck.slice(0, recheckTimeout);
  const earlyReturns = recheckHead.match(/\breturn\b/g) ?? [];
  assert.equal(earlyReturns.length, 1, "only the in-flight guard returns before setTimeout");
  assert.match(recheckHead, /if\s*\(\s*graceRecheck\s*\)\s*return/);
  const recheckCb = braceBody(recheck, recheckTimeout);
  assert.ok(recheckCb.includes("maybeAutoHide()"), "scheduleGraceRecheck callback calls maybeAutoHide()");
  const listenAt = pageSrc.indexOf("orbitty:rail-stage");
  const listen = braceBody(pageSrc, pageSrc.indexOf("=>", listenAt));
  const edge = braceBody(listen, listen.indexOf('source === "edge"'));
  const timeoutAt = edge.indexOf("setTimeout(");
  assert.ok(timeoutAt >= 0, "edge branch has setTimeout");
  const cb = braceBody(edge, timeoutAt);
  const seed = cb.indexOf("edgePointerSeed = false");
  const inside = cb.indexOf("pointerInside = false");
  const hideCall = cb.indexOf("maybeAutoHide()");
  assert.ok(seed >= 0 && inside > seed && hideCall > inside, "seed expiry clears and rechecks");
  const cbAt = edge.indexOf(cb, timeoutAt);
  assert.match(edge.slice(cbAt + cb.length, cbAt + cb.length + 40), /^\s*,\s*EDGE_GRACE_MS\s*\)/);
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
