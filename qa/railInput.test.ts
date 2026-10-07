import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";
import { SPIN_STEP_PX } from "../src/lib/rail/stageLook.ts";
import {
  ARCH_DWELL_MS,
  ARCH_IDLE,
  archArrow,
  archLabelBox,
  archMarbleBox,
  archPointer,
  enterBoard,
  type ArchState,
} from "../src/lib/rail/stageArch.ts";
import { spinCenterFor, spinItems, type SpinItem } from "../src/lib/rail/stageLook.ts";
import { oneFocusOwner, selWrapBox } from "../src/lib/rail/stageFocus.ts";
import { FOCUS_RING } from "../src/themes/host.ts";
import {
  SPIN_CAP_IDLE,
  SPIN_FLICK_MAX,
  archGutter,
  centreStep,
  depthOut,
  depthOutStop,
  type CentreCause,
  emptyDismiss,
  hitBox,
  inputIntent,
  railCrumb,
  spinClickTarget,
  spinStepsCap,
} from "../src/lib/rail/railInput.ts";
import { columnNext, segmentNext, withinNext } from "../src/lib/stars/starsFocus.ts";

const list = readFileSync(new URL("../src/lib/rail/List.svelte", import.meta.url), "utf8");
const page = readFileSync(new URL("../src/lib/RailPage.svelte", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/lib/rail/rail.css", import.meta.url), "utf8");
const chrome = readFileSync(new URL("../src/lib/rail/Chrome.svelte", import.meta.url), "utf8");
const host = readFileSync(new URL("../src/themes/host.ts", import.meta.url), "utf8");
const root = readFileSync(new URL("../src/routes/+page.svelte", import.meta.url), "utf8");
const dial = readFileSync(new URL("../src/lib/stars/RadarDial.svelte", import.meta.url), "utf8");
const starsPage = readFileSync(new URL("../src/lib/StarsPage.svelte", import.meta.url), "utf8");

function wheelBody(): string {
  const at = list.indexOf("const onWheel = (e: WheelEvent)");
  const end = list.indexOf('el.addEventListener("wheel"', at);
  assert.ok(at >= 0 && end > at);
  return list.slice(at, end);
}

function ruleBody(src: string, selector: string): string {
  const at = src.indexOf(selector);
  assert.ok(at >= 0, selector);
  const open = src.indexOf("{", at);
  const close = src.indexOf("}", open);
  return src.slice(open + 1, close);
}

test("R1 wheel never changes depth and a 600px flick is at most 3 steps", () => {
  const wheel = wheelBody();
  assert.equal(wheel.includes("bringArch("), false);
  assert.equal(wheel.includes("archBack("), false);
  assert.equal(wheel.includes("archForward("), false);
  assert.match(wheel, /spinStepsCap\(/);
  const unbound = Math.trunc(600 / SPIN_STEP_PX);
  assert.ok(unbound > SPIN_FLICK_MAX, `unbound ${unbound}`);
  const flick = spinStepsCap(SPIN_CAP_IDLE, { deltaX: 0, deltaY: 600, deltaMode: 0 }, 1_000);
  assert.ok(Math.abs(flick.steps) <= 3);
  assert.ok(Math.abs(flick.steps) >= 1);
  const line = spinStepsCap(SPIN_CAP_IDLE, { deltaX: 0, deltaY: 5, deltaMode: 1 }, 2_000);
  assert.equal(line.steps, 1);
  const pageEv = spinStepsCap(SPIN_CAP_IDLE, { deltaX: -9, deltaY: 0, deltaMode: 2 }, 3_000);
  assert.equal(pageEv.steps, -1);
  let state = SPIN_CAP_IDLE;
  let total = 0;
  for (let i = 0; i < 6; i++) {
    const out = spinStepsCap(state, { deltaX: 0, deltaY: 100, deltaMode: 0 }, 5_000 + i * 10);
    state = out.state;
    total += Math.abs(out.steps);
  }
  assert.ok(total <= 3, `burst ${total}`);
});

test("R2 inputIntent maps in, out, and move", () => {
  assert.equal(inputIntent({ kind: "key", key: "Enter" }), "in");
  assert.equal(inputIntent({ kind: "click", onTarget: true }), "in");
  assert.equal(inputIntent({ kind: "pointer", button: 4 }), "in");
  assert.equal(inputIntent({ kind: "key", key: "Escape" }), "out");
  assert.equal(inputIntent({ kind: "pointer", button: 3 }), "out");
  assert.equal(inputIntent({ kind: "click", empty: true }), "out");
  for (const key of ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"]) {
    assert.equal(inputIntent({ kind: "key", key }), "move", key);
  }
  assert.equal(inputIntent({ kind: "hover" }), "move");
  assert.equal(inputIntent({ kind: "wheel" }), "move");
  assert.equal(inputIntent({ kind: "pointer", button: 0 }), "none");
  assert.equal(inputIntent({ kind: "click" }), "none");
});

test("R3 archPointer dwell never changes front", () => {
  const layer: ArchState = { front: "layer2", pid: "a", center2: 1, pending: null };
  const t = 8_000;
  const early = archPointer(layer, "arch", t);
  assert.equal(early.front, "layer2");
  assert.equal(early.pending?.zone, "arch");
  const fired = archPointer(early, "arch", t + ARCH_DWELL_MS + 500);
  assert.equal(fired.front, "layer2");
  assert.equal(fired.pid, "a");
  const arch: ArchState = { front: "arch", pid: "a", center2: 0, pending: null };
  const armed = archPointer(arch, "layer2", t);
  const stayed = archPointer(armed, "layer2", t + ARCH_DWELL_MS * 4);
  assert.equal(stayed.front, "arch");
});

test("R4 Esc out is one level and the root hides", () => {
  assert.deepEqual(depthOut("board"), { front: "layer2", hide: false });
  assert.deepEqual(depthOut("layer2"), { front: "arch", hide: false });
  assert.deepEqual(depthOut("arch"), { front: "arch", hide: true });
  const esc = page.slice(page.indexOf("function onRailEscape"), page.indexOf("function reclaimRailRing"));
  assert.equal(esc.includes("archBack(archBack"), false);
  assert.match(esc, /depthOut\(/);
  assert.match(esc, /parkStage\(/);
});

test("R5 hit box outsets every edge by 8px and empty dismiss is pointerup", () => {
  const frame = { w: 1040, h: 760 };
  for (const edge of ["left", "right", "top", "bottom"] as const) {
    for (const k of [-2, 0, 2]) {
      const marble = archMarbleBox(k, edge, 1, frame, k === 0);
      const label = archLabelBox(k, edge, 1, frame, { avoid: [] });
      const box = hitBox(marble, label);
      const wrap = selWrapBox(marble, label, 8);
      assert.deepEqual(box, wrap);
      for (const part of [marble, label]) {
        assert.ok(box.x <= part.x - 8 + 0.01, `${edge} ${k} x`);
        assert.ok(box.y <= part.y - 8 + 0.01, `${edge} ${k} y`);
        assert.ok(box.x + box.w >= part.x + part.w + 8 - 0.01, `${edge} ${k} r`);
        assert.ok(box.y + box.h >= part.y + part.h + 8 - 0.01, `${edge} ${k} b`);
      }
    }
  }
  const empty = { dragPx: 0, overHit: false, overPanel: false, animating: false };
  assert.equal(emptyDismiss({ phase: "pointerdown", ...empty }), false);
  assert.equal(emptyDismiss({ phase: "pointerup", ...empty }), true);
  assert.equal(emptyDismiss({ phase: "pointerup", ...empty, dragPx: 4 }), false);
  assert.equal(emptyDismiss({ phase: "pointerup", ...empty, overHit: true }), false);
  assert.equal(emptyDismiss({ phase: "pointerup", ...empty, overPanel: true }), false);
  assert.equal(emptyDismiss({ phase: "pointerup", ...empty, animating: true }), false);
  assert.equal(spinClickTarget(true, false), "landing");
  assert.equal(spinClickTarget(true, true), "item");
  const down = page.slice(page.indexOf("function onStagePointerDown"), page.indexOf("function onStagePointerUp"));
  assert.equal(down.includes("stageFocus(false)"), false);
  assert.match(page, /function onStagePointerUp/);
  assert.match(page, /emptyDismiss\(/);
});

test("R6 one R from layer 2 lands on the board with focus", () => {
  const items = spinItems(["a"], "__unmapped__");
  const layer2 = [
    { kind: "board" as const, stop: "rl2-board", title: "Board" },
    { kind: "window" as const, stop: "rl2-w", title: "Win" },
  ];
  const onBoard: ArchState = { front: "layer2", pid: "a", center2: 0, pending: null };
  const cards = [{ id: "c1", status: "todo" }];
  const once = archArrow(onBoard, items, 2, "right", "left", layer2, "rl2-board", false, cards);
  assert.equal(once?.state.front, "board");
  assert.equal(once?.walkIn, "board");
  assert.equal(once?.focus?.kind, "card");
  assert.equal(once?.focus?.id, "c1");
  const empty = enterBoard(onBoard, []);
  assert.equal(empty?.state.front, "board");
  assert.equal(empty?.focus.kind, "add");
  const notLayer = enterBoard({ ...onBoard, front: "arch" }, cards);
  assert.equal(notLayer, null);
  const apply = page.slice(page.indexOf("if (result.walkIn)"), page.indexOf("archDo(result.state)") + 400);
  assert.match(apply, /focusBoardLanding\(/);
});

test("R7 Stars columns, segments, and the embedded outer edge", () => {
  for (const layout of ["three", "dots"] as const) {
    assert.deepEqual(columnNext(layout, "planets", "right"), { col: "middle", exit: false });
    assert.deepEqual(columnNext(layout, "middle", "right"), { col: "trackers", exit: false });
    assert.deepEqual(columnNext(layout, "middle", "left"), { col: "planets", exit: false });
    assert.deepEqual(columnNext(layout, "planets", "left", { embedded: true, outward: "left" }), {
      col: "planets",
      exit: true,
    });
    assert.deepEqual(columnNext(layout, "trackers", "right", { embedded: true, outward: "left" }), {
      col: "trackers",
      exit: false,
    });
    assert.equal(columnNext(layout, "planets", "left", { embedded: false, outward: "left" }).exit, false);
  }
  assert.deepEqual(columnNext("single", "planets", "right", { embedded: true, outward: "left" }), {
    col: "planets",
    exit: false,
  });
  assert.equal(segmentNext("checkin", "right"), "trackers");
  assert.equal(segmentNext("trackers", "right"), "journal");
  assert.equal(segmentNext("journal", "right"), "journal");
  assert.equal(segmentNext("checkin", "left"), "checkin");
  assert.equal(withinNext("planets", 0, "up", 4), 3);
  assert.equal(withinNext("trackers", 3, "down", 4), 0);
  assert.equal(withinNext("middle", 0, "down", 2), 1);
  assert.match(starsPage, /columnNext\(/);
  assert.match(starsPage, /onRingExit\?\.\(\)/);
});

test("R8 name pills are solid, 14px semibold, not uppercase, with a dot", () => {
  const name = ruleBody(css, ".rail.stage-shell .arch-name {");
  assert.match(name, /background:\s*color-mix\(in srgb,\s*var\(--panel\)\s*90%,\s*transparent\)/);
  assert.match(name, /color:\s*var\(--ink\)/);
  assert.match(name, /font:\s*600\s+14px\/1\.2/);
  assert.match(name, /text-transform:\s*none/);
  assert.match(name, /-apple-system,\s*"SF Pro Text"/);
  assert.doesNotMatch(name, /uppercase/);
  const dot = ruleBody(css, ".rail.stage-shell .arch-dot {");
  assert.match(dot, /width:\s*8px/);
  assert.match(dot, /height:\s*8px/);
  assert.match(list, /class="arch-dot"/);
  assert.match(list, /planetCss\(/);
});

test("R9 gutter is a quarter of the preview and the selected slot scales 1.06", () => {
  assert.equal(archGutter(220), 55);
  assert.ok(Math.abs(archGutter(180) - 45) < 1e-9);
  assert.equal(archGutter(0), 0);
  assert.match(list, /archGutter\(/);
  assert.match(css, /\.is-selected \.arch-vis/);
  assert.match(css, /transform:\s*scale\(1\.06\)/);
  assert.match(list, /class="l2-app-icon"/);
});

test("R10 one focus ring token and one owner", () => {
  assert.equal(FOCUS_RING, "0 0 0 2px var(--bg), 0 0 0 4px var(--accent)");
  assert.match(host, /FOCUS_RING = "0 0 0 2px var\(--bg\), 0 0 0 4px var\(--accent\)"/);
  assert.match(root, /--focus-ring:\s*0 0 0 2px var\(--bg\), 0 0 0 4px var\(--accent\)/);
  assert.match(css, /box-shadow:\s*var\(--focus-ring\)/);
  for (const file of [
    "src/lib/StarsPage.svelte",
    "src/lib/stars/StarsPlanetColumn.svelte",
    "src/lib/stars/StarsTrackers.svelte",
    "src/lib/stars/RadarDial.svelte",
  ]) {
    const src = readFileSync(new URL(`../${file}`, import.meta.url), "utf8");
    assert.match(src, /var\(--focus-ring\)/, file);
  }
  assert.equal(/display:\s*block/.test(css.slice(css.indexOf(".arch-ringlet"), css.indexOf(".arch-ringlet") + 900)), false);
  assert.equal(oneFocusOwner([true, false, false]), true);
  assert.equal(oneFocusOwner([true, true]), false);
  assert.equal(oneFocusOwner([false, false]), true);
});

test("R11 breadcrumb strings", () => {
  const names = { p1: "Orbitty" };
  assert.equal(railCrumb("arch", null, names), "Planets");
  assert.equal(railCrumb("layer2", "p1", names), "Planets › Orbitty");
  assert.equal(railCrumb("board", "p1", names), "Planets › Orbitty › Board");
  assert.equal(railCrumb("layer2", "missing", names), "Planets › Planet");
  assert.match(chrome, /railCrumbParts\(/);
  assert.match(chrome, /class:was=\{!part\.current\}/);
  assert.match(css, /\.rail-crumb \.was/);
  assert.match(css, /var\(--dim\)/);
});

function parseCssColor(raw: string): { r: number; g: number; b: number; a: number } {
  const s = raw.trim();
  const hex = s.match(/^#([0-9a-f]{3}|[0-9a-f]{6})$/i);
  if (hex) {
    let h = hex[1];
    if (h.length === 3) h = [...h].map((c) => c + c).join("");
    return {
      r: Number.parseInt(h.slice(0, 2), 16),
      g: Number.parseInt(h.slice(2, 4), 16),
      b: Number.parseInt(h.slice(4, 6), 16),
      a: 1,
    };
  }
  const rgb = s.match(/^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)(?:[\s,/]+([\d.]+))?\s*\)$/i);
  assert.ok(rgb, s);
  return {
    r: Number(rgb[1]),
    g: Number(rgb[2]),
    b: Number(rgb[3]),
    a: rgb[4] === undefined ? 1 : Number(rgb[4]),
  };
}

/** Premultiplied `color-mix(in srgb, ink p%, other)`. */
function mixSrgb(
  ink: { r: number; g: number; b: number; a: number },
  other: { r: number; g: number; b: number; a: number },
  pInk: number,
): { r: number; g: number; b: number; a: number } {
  const p2 = 1 - pInk;
  const a = pInk * ink.a + p2 * other.a;
  const ch = (c1: number, a1: number, c2: number, a2: number) =>
    a <= 0 ? 0 : (pInk * c1 * a1 + p2 * c2 * a2) / a;
  return {
    r: ch(ink.r, ink.a, other.r, other.a),
    g: ch(ink.g, ink.a, other.g, other.a),
    b: ch(ink.b, ink.a, other.b, other.a),
    a,
  };
}

function compositeOver(
  fg: { r: number; g: number; b: number; a: number },
  bg: { r: number; g: number; b: number; a: number },
): { r: number; g: number; b: number; a: number } {
  const a = fg.a + bg.a * (1 - fg.a);
  const ch = (f: number, b: number) => (a <= 0 ? 0 : (f * fg.a + b * bg.a * (1 - fg.a)) / a);
  return { r: ch(fg.r, bg.r), g: ch(fg.g, bg.g), b: ch(fg.b, bg.b), a };
}

function relLuminance(c: { r: number; g: number; b: number }): number {
  const lin = (u: number) => {
    const x = u / 255;
    return x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
}

function quotedField(block: string, name: string): string {
  const m = block.match(new RegExp(`${name}:\\s*"([^"]+)"`));
  assert.ok(m, name);
  return m[1];
}

/** Opaque knob: mix ink into panel, then composite any leftover alpha onto the panel. */
function knobLuminance(inkRaw: string, panelRaw: string, pInk: number): number {
  const ink = parseCssColor(inkRaw);
  const panel = parseCssColor(panelRaw);
  return relLuminance(compositeOver(mixSrgb(ink, panel, pInk), panel));
}

test("R12 knobs use the ink mix and stars has no light fills", () => {
  const knob = ruleBody(dial, ".knob {");
  assert.match(knob, /fill:\s*color-mix\(in srgb,\s*var\(--ink\)\s*40%,\s*var\(--panel\)\)/);
  assert.match(knob, /stroke:\s*var\(--accent\)/);
  assert.doesNotMatch(knob, /var\(--on-accent\)/);
  assert.doesNotMatch(knob, /78%/);
  const darkAt = host.indexOf("systemDark:");
  const darkBlock = host.slice(darkAt, host.indexOf("colorScheme:", darkAt));
  const darkInk = quotedField(darkBlock, "ink");
  const darkPanel = quotedField(darkBlock, "panel");
  const term = root.slice(root.indexOf('data-theme="terminal"'), root.indexOf("color-scheme:", root.indexOf('data-theme="terminal"')));
  const termInk = term.match(/--ink:\s*([^;]+);/)?.[1]?.trim() ?? "";
  const termPanel = term.match(/--panel:\s*([^;]+);/)?.[1]?.trim() ?? "";
  assert.equal(darkInk, "rgba(255, 255, 255, 0.85)");
  assert.equal(darkPanel, "#2C2C2E");
  assert.equal(termInk, "#c8f0d0");
  assert.equal(termPanel, "#12201a");
  const darkL = knobLuminance(darkInk, darkPanel, 0.4);
  const termL = knobLuminance(termInk, termPanel, 0.4);
  assert.ok(darkL < 0.3, `systemDark knob L ${darkL}`);
  assert.ok(termL < 0.3, `terminal knob L ${termL}`);
  const oldDark = relLuminance(
    compositeOver(mixSrgb(parseCssColor(darkInk), { r: 0, g: 0, b: 0, a: 0 }, 0.78), parseCssColor(darkPanel)),
  );
  assert.ok(oldDark >= 0.3, `78% formula must stay above the guard, got ${oldDark}`);
  assert.ok(relLuminance(parseCssColor("#ffffff")) >= 0.3);
  const dir = new URL("../src/lib/stars/", import.meta.url);
  const files = readdirSync(dir).filter((n) => n.endsWith(".svelte") || n.endsWith(".css"));
  const light: string[] = [];
  for (const name of files) {
    const src = readFileSync(new URL(name, dir), "utf8");
    const styleAt = src.indexOf("<style");
    const body = styleAt >= 0 ? src.slice(styleAt) : src;
    if (/fill:\s*var\(--on-accent\)/.test(body)) light.push(`${name} on-accent`);
    if (/fill:\s*#fff\b/i.test(body)) light.push(`${name} #fff`);
    if (/fill:\s*white\b/.test(body)) light.push(`${name} white`);
    if (/fill:\s*rgb\(\s*255[\s,]+255[\s,]+255/.test(body)) light.push(`${name} rgb`);
  }
  assert.deepEqual(light, []);
});

function fnBody(src: string, name: string, nextName: string): string {
  const at = src.indexOf(`function ${name}`);
  const end = src.indexOf(`function ${nextName}`, at);
  assert.ok(at >= 0 && end > at, name);
  return src.slice(at, end);
}

/** background / background-color values that are light slabs. Gradients at alpha ≤ 0.55 that fade to transparent stay. */
function lightBackgrounds(cssSrc: string): string[] {
  const src = cssSrc.replace(/\/\*[\s\S]*?\*\//g, "");
  const hits: string[] = [];
  const re = /(^|[\s;}])(background(?:-color)?)\s*:\s*([^;]*);/g;
  for (const m of src.matchAll(re)) {
    let value = m[3].replace(/var\(--planet,\s*#fff(?:fff)?\)/gi, "var(--planet)");
    const where = `${m[2]}: ${value.trim()}`;
    const bare = value.replace(/color-mix\((?:[^)(]+|\([^)]*\))*\)/gi, "");
    const lightRgb = (body: string): boolean => {
      const rgbRe = /rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)(?:[\s,/]+(\d*\.?\d+))?\s*\)/gi;
      for (const c of body.matchAll(rgbRe)) {
        const ch = [Number(c[1]), Number(c[2]), Number(c[3])];
        const alpha = c[4] === undefined ? 1 : Number(c[4]);
        if (ch.every((n) => n >= 230) && alpha >= 0.5) return true;
      }
      if (/(^|[^#\w])#fff\b/i.test(body) || /#ffffff\b/i.test(body)) return true;
      if (/(^|[^\w-])white\b/i.test(body)) return true;
      return false;
    };
    if (/gradient\s*\(/i.test(value)) {
      if (!/transparent/i.test(value)) {
        if (lightRgb(bare)) hits.push(where);
        continue;
      }
      const rgbRe = /rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)(?:[\s,/]+(\d*\.?\d+))?\s*\)/gi;
      for (const c of value.matchAll(rgbRe)) {
        const ch = [Number(c[1]), Number(c[2]), Number(c[3])];
        const alpha = c[4] === undefined ? 1 : Number(c[4]);
        if (ch.every((n) => n >= 230) && alpha > 0.55) hits.push(where);
      }
      const stripped = bare.replace(/rgba?\([^)]*\)/gi, "");
      if (/(^|[^#\w])#fff\b/i.test(stripped) || /#ffffff\b/i.test(stripped) || /(^|[^\w-])white\b/i.test(stripped)) {
        hits.push(where);
      }
      continue;
    }
    if (lightRgb(bare)) hits.push(where);
  }
  return hits;
}

const PANEL_TINT = /background:\s*color-mix\(in srgb,\s*var\(--panel\)\s*92%,\s*transparent\)/;

test("B1 Esc and walkOut call sites step one level", () => {
  const esc = fnBody(page, "onRailEscape", "reclaimRailRing");
  const at = esc.indexOf('arch.front !== "arch"');
  const branch = esc.slice(at, esc.indexOf("return true;", at));
  assert.match(branch, /const step = depthOut\(arch\.front\)/);
  assert.match(branch, /front:\s*step\.front/);
  assert.doesNotMatch(branch, /front:\s*"arch"/);
  const walk = fnBody(page, "walkOut", "closeStageEmbed");
  assert.match(walk, /if \(fromBoard\) archDo\(\{ \.\.\.arch, front: "layer2", pending: null \}\)/);
  assert.doesNotMatch(walk, /front:\s*"arch"/);
});

test("B2 empty dismiss is pointerup and overPanel is its own hit", () => {
  const down = fnBody(page, "onStagePointerDown", "onStagePointerUp");
  assert.equal(down.includes("onRailEscape"), false);
  assert.equal(down.includes("emptyDismiss"), false);
  const up = fnBody(page, "onStagePointerUp", "onStageAux");
  assert.match(up, /phase:\s*"pointerup"/);
  assert.match(up, /dragPx:\s*drag,/);
  assert.doesNotMatch(up, /dragPx:\s*0/);
  assert.match(up, /const animating = wheelAnimating\(\)/);
  assert.match(up, /\n\s*animating,\n/);
  assert.doesNotMatch(up, /animating:\s*false/);
  assert.match(up, /closest\("\.stage-settings, \.l2-expanded, \.l2-board, \.chrome"\)/);
  assert.match(up, /overPanel,/);
  assert.doesNotMatch(up, /overPanel:\s*hit\b/);
});

test("B3 Stars arrows call columnNext and spoke Esc returns to the column", () => {
  const key = starsPage.slice(starsPage.indexOf("function onStarsKey"), starsPage.indexOf("const showCheckin"));
  const arrow = key.indexOf('if (e.key === "ArrowLeft" || e.key === "ArrowRight")');
  assert.ok(arrow >= 0);
  const before = key.slice(arrow, key.indexOf("columnNext(", arrow));
  assert.doesNotMatch(before, /(^|\n)\s*return\s*;/);
  assert.match(key, /const next = columnNext\(/);
  assert.match(key, /focusCol = next\.col/);
  assert.match(key, /focusStarsCol\(next\.col\)/);
  const exit = key.slice(key.indexOf("if (next.exit)"), key.indexOf("if (next.col"));
  assert.match(exit, /onRingExit\?\.\(\)/);
  const cb = starsPage.slice(
    starsPage.indexOf("detachMenuKeys = menuRing.attach"),
    starsPage.indexOf('invoke("hide_window"'),
  );
  assert.match(cb, /spokeLive = false/);
  assert.ok(cb.indexOf("spokeLive = false") < cb.length);
});

test("B4 mouse Back and Forward bind pointerup and auxclick", () => {
  const mouse = fnBody(page, "takeMouseButton", "onStagePointerDown");
  assert.match(mouse, /button !== 3 && button !== 4/);
  assert.match(mouse, /inputIntent\(\{\s*kind:\s*"pointer",\s*button\s*\}\)/);
  assert.match(page, /onpointerup=\{onStagePointerUp\}/);
  assert.match(page, /onauxclick=\{onStageAux\}/);
});

test("B5 depth-out keeps the came-from planet and does not open beside", () => {
  const items = spinItems(["came-from"], "__unmapped__");
  const at = spinCenterFor(items, depthOutStop("came-from"), 0);
  assert.equal(items[0].kind, "stars");
  assert.equal(items[at].pid, "came-from");
  assert.equal(items[at].kind, "planet");
  assert.notEqual(at, 0);
  assert.equal(depthOutStop("came-from"), items[at].stop);
  assert.equal(depthOutStop(null), null);
  assert.equal(depthOut("layer2").front, "arch");
  const esc = fnBody(page, "onRailEscape", "reclaimRailRing");
  const branchAt = esc.indexOf('arch.front !== "arch"');
  const branch = esc.slice(branchAt, esc.indexOf("return true;", branchAt));
  assert.match(branch, /reselectStop = backStop/);
  assert.match(branch, /depthOutStop\(arch\.pid\)/);
  assert.match(branch, /centreCause = "depthOut"/);
  assert.ok(branch.indexOf("reselectStop") < branch.indexOf("const step = depthOut"));
  assert.doesNotMatch(branch, /applyBeside\(/);
  assert.doesNotMatch(branch, /stageEmbed\s*=/);
});

test("B5 centreStep depthOut does not apply and the stored cause is spin", () => {
  assert.deepEqual(centreStep("depthOut"), { apply: false, next: "spin" });
  assert.deepEqual(centreStep("spin"), { apply: true, next: "spin" });
});

test("B5 sequence depthOut centre change then a later centre change applies", () => {
  let cause: CentreCause = "depthOut";
  const first = centreStep(cause);
  assert.equal(first.apply, false);
  cause = first.next;
  const later = centreStep(cause);
  assert.equal(later.apply, true);
  assert.equal(later.next, "spin");
});

/** Brace-matched body starting at the first `{` at or after `from`. Comments and strings are stripped first. */
function braceBody(src: string, from: number): string {
  const open = src.indexOf("{", from);
  assert.ok(open >= 0);
  let depth = 0;
  for (let i = open; i < src.length; i++) {
    if (src[i] === "{") depth++;
    else if (src[i] === "}") {
      depth--;
      if (depth === 0) return src.slice(open, i + 1);
    }
  }
  assert.fail("unclosed brace");
}

/** Spaces out comments and string literals (templates included). Newlines are kept. */
function stripCommentsAndStrings(src: string): string {
  const out: string[] = [];
  const blank = (from: number, to: number) => {
    for (let k = from; k < to; k++) out.push(src[k] === "\n" ? "\n" : " ");
  };
  const skipString = (i: number): number => {
    const q = src[i];
    let j = i + 1;
    while (j < src.length) {
      if (src[j] === "\\") {
        j += 2;
        continue;
      }
      if (q === "`" && src[j] === "$" && src[j + 1] === "{") {
        j += 2;
        let nest = 1;
        while (j < src.length && nest > 0) {
          if (src[j] === "'" || src[j] === '"' || src[j] === "`") {
            j = skipString(j);
            continue;
          }
          if (src[j] === "{") nest++;
          else if (src[j] === "}") nest--;
          if (nest > 0) j++;
        }
        continue;
      }
      if (src[j] === q) return j + 1;
      j++;
    }
    return j;
  };
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    const n = src[i + 1];
    if (c === "/" && n === "/") {
      const end = src.indexOf("\n", i);
      const stop = end < 0 ? src.length : end;
      blank(i, stop);
      i = stop;
      continue;
    }
    if (c === "/" && n === "*") {
      const end = src.indexOf("*/", i + 2);
      const stop = end < 0 ? src.length : end + 2;
      blank(i, stop);
      i = stop;
      continue;
    }
    if (c === "'" || c === '"' || c === "`") {
      const stop = skipString(i);
      blank(i, stop);
      i = stop;
      continue;
    }
    out.push(c);
    i++;
  }
  return out.join("");
}

/** Brace, paren, and ternary depth of `src[from, to)`. `?.` and `??` are not ternaries. */
function scanDepths(src: string, from: number, to: number): { brace: number; paren: number; tern: number } {
  let brace = 0;
  let paren = 0;
  let tern = 0;
  for (let i = from; i < to; i++) {
    const c = src[i];
    const n = src[i + 1];
    if (c === "{") brace++;
    else if (c === "}") brace--;
    else if (c === "(") paren++;
    else if (c === ")") paren--;
    else if (c === "?") {
      if (n === "?" || n === ".") {
        i++;
        continue;
      }
      tern++;
    } else if (c === ":" && tern > 0 && n !== ":" && src[i - 1] !== ":") tern--;
  }
  return { brace, paren, tern };
}

test("B5 List centre effect resets through centreStep after the guard", () => {
  const centre = list.slice(list.indexOf("const onWheel = !!active"), list.indexOf("if (rail.spinCenter"));
  const stepAt = centre.indexOf("const step = centreStep(rail.centreCause)");
  const guardAt = centre.indexOf("if (step.apply)");
  const besideAt = centre.indexOf("rail.applyBeside(");
  const resetAt = centre.indexOf("rail.centreCause = step.next");
  assert.ok(stepAt >= 0, "centreStep call");
  assert.ok(guardAt > stepAt, "guard after centreStep");
  assert.ok(besideAt > guardAt, "applyBeside inside the guard");
  assert.ok(resetAt > besideAt, "reset after the guard");
  assert.equal(centre.includes('centreCause = "depthOut"'), false);
  assert.equal(centre.includes('centreCause = "spin"'), false);

  // Verify probe floor (repro/probe_test.ts): one `centreCause =` write, after the centre-change block.
  assert.equal((centre.match(/centreCause\s*=/g) ?? []).length, 1);
  const br = centre.indexOf("if (next !== center) {");
  const close = centre.indexOf("\n    }\n", br);
  assert.ok(br >= 0 && close > br);
  assert.ok(centre.indexOf("rail.centreCause = step.next") > close);

  const stripped = stripCommentsAndStrings(list);
  const marker = stripped.indexOf("const step = centreStep(rail.centreCause)");
  assert.ok(marker >= 0, "centreStep call in stripped source");
  const effectAt = stripped.lastIndexOf("$effect", marker);
  assert.ok(effectAt >= 0 && effectAt < marker);
  const body = braceBody(stripped, effectAt);
  assert.equal(body.includes("if (!booted)"), true);
  assert.equal(body.includes("const pid = archState"), false);
  const writeRe = /centreCause\s*(=|\|\|=|\?\?=|&&=)(?!=)/g;
  const writes = [...body.matchAll(writeRe)];
  assert.equal(writes.length, 1, "exactly one centreCause write");
  const found = writes[0];
  assert.ok(found.index !== undefined);
  assert.equal(found[1], "=");
  const exact = "rail.centreCause = step.next";
  const rel = found.index - "rail.".length;
  assert.ok(rel >= 0);
  assert.equal(/[A-Za-z0-9_$]/.test(body[rel - 1] ?? ""), false);
  assert.equal(body.slice(rel, rel + exact.length), exact);
  const after = body[rel + exact.length] ?? "";
  assert.equal(/[A-Za-z0-9_$.]/.test(after), false);
  const stepRel = body.indexOf("const step = centreStep(rail.centreCause)");
  assert.ok(stepRel >= 0 && rel > stepRel, "write after centreStep");
  const ifRel = body.indexOf("if (next !== center)");
  assert.ok(ifRel >= 0 && ifRel > stepRel);
  const ifBlock = braceBody(body, ifRel);
  const ifOpen = body.indexOf("{", ifRel);
  const ifEnd = ifOpen + ifBlock.length;
  assert.ok(rel >= ifEnd, "write after the centre-change block");
  const depths = scanDepths(body, 1, rel);
  assert.equal(depths.brace, 0, "write at brace depth 0");
  assert.equal(depths.paren, 0, "write not inside a call or callback");
  assert.equal(depths.tern, 0, "write not inside a ternary");
  let boundary = rel - 1;
  while (boundary >= 0 && /\s/.test(body[boundary])) boundary--;
  assert.ok(
    boundary >= 0 && (body[boundary] === ";" || body[boundary] === "{" || body[boundary] === "}"),
    "write is its own statement at depth 0",
  );
});

test("N2 rail backgrounds are dark panel tints", () => {
  assert.match(ruleBody(css, ".rail.stage-shell .spin-board {"), PANEL_TINT);
  assert.match(ruleBody(css, ".rail.stage-shell .spin-board {"), /color:\s*var\(--ink\)/);
  assert.match(ruleBody(css, ".rail.stage-shell .arch-slot .spin-newproj {"), PANEL_TINT);
  assert.match(ruleBody(css, ".rail.stage-shell .spin-newproj .mini {"), PANEL_TINT);
  assert.match(ruleBody(css, ".rail.stage-shell .spin-newproj .mini {"), /color:\s*var\(--ink\)/);
  assert.match(ruleBody(css, ".rail.stage-shell .stage-board-tile {"), PANEL_TINT);
  assert.match(ruleBody(css, ".rail.stage-shell .stage-board-tile {"), /color:\s*var\(--ink\)/);
  const light = ruleBody(css, ':root[data-theme="systemLight"] .rail.stage-shell .l2-expanded {');
  assert.match(light, /background:\s*rgb\(28\s+28\s+32\s*\/\s*0\.94\)/);
  assert.match(light, /color:\s*#f5f5f7/);
  assert.doesNotMatch(light, /255\s+255\s+255/);
  assert.deepEqual(lightBackgrounds(css), []);
});

test("R6b inward on a non-board layer-2 item does not require a second press to focus", () => {
  const items: SpinItem[] = spinItems(["a"], "__unmapped__");
  const layer2 = [{ kind: "board" as const, stop: "rl2-board", title: "Board" }];
  const state: ArchState = { front: "layer2", pid: "a", center2: 0, pending: null };
  const hit = archArrow(state, items, 2, "right", "left", layer2, null, false, [{ id: "only" }]);
  assert.equal(hit?.focus?.id, "only");
  assert.equal(hit?.state.front, "board");
});
