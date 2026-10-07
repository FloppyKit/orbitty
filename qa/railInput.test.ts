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
import { spinItems, type SpinItem } from "../src/lib/rail/stageLook.ts";
import { oneFocusOwner, selWrapBox } from "../src/lib/rail/stageFocus.ts";
import { FOCUS_RING } from "../src/themes/host.ts";
import {
  SPIN_CAP_IDLE,
  SPIN_FLICK_MAX,
  archGutter,
  depthOut,
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

test("R12 knobs use the ink mix and stars has no light fills", () => {
  const knob = ruleBody(dial, ".knob {");
  assert.match(knob, /fill:\s*color-mix\(in srgb,\s*var\(--ink\)\s*78%,\s*transparent\)/);
  assert.match(knob, /stroke:\s*var\(--accent\)/);
  assert.doesNotMatch(knob, /var\(--on-accent\)/);
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

test("R6b inward on a non-board layer-2 item does not require a second press to focus", () => {
  const items: SpinItem[] = spinItems(["a"], "__unmapped__");
  const layer2 = [{ kind: "board" as const, stop: "rl2-board", title: "Board" }];
  const state: ArchState = { front: "layer2", pid: "a", center2: 0, pending: null };
  const hit = archArrow(state, items, 2, "right", "left", layer2, null, false, [{ id: "only" }]);
  assert.equal(hit?.focus?.id, "only");
  assert.equal(hit?.state.front, "board");
});
