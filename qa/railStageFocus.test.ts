import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { cssRuleBlocks, stripComments } from "./cssChain.ts";
import {
  archHalo,
  besideFront,
  besideNext,
  keyOwnerNext,
  outwardDir,
  pickFromKind,
  selWrapBox,
  type KeyOwner,
} from "../src/lib/rail/stageFocus.ts";
import {
  ARCH_IDLE,
  archArrow,
  archLabelBox,
  archLayer2Items,
  archMarbleBox,
  archScroll2,
  type ArchState,
} from "../src/lib/rail/stageArch.ts";
import { spinItems, type SpinItem } from "../src/lib/rail/stageLook.ts";

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

const list = readFileSync(new URL("../src/lib/rail/List.svelte", import.meta.url), "utf8");
const page = railSrc();
const css = railCss();
const stars = readFileSync(new URL("../src/lib/StarsPage.svelte", import.meta.url), "utf8");
const settings = readFileSync(new URL("../src/lib/SettingsPage.svelte", import.meta.url), "utf8");
const kanban = readFileSync(new URL("../src/lib/KanbanPage.svelte", import.meta.url), "utf8");
const polish = readFileSync(new URL("./railStagePolish.test.ts", import.meta.url), "utf8");
const menuKeys = readFileSync(new URL("../src/lib/menuKeys.svelte.ts", import.meta.url), "utf8");

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

function fnBody(src: string, name: string): string {
  const at = src.indexOf(`function ${name}`);
  assert.ok(at >= 0, name);
  return braceBody(src, at);
}

test("F1 besideNext replaces a different owner and keeps the same one", () => {
  assert.equal(besideNext("stars", "planet"), null);
  assert.equal(besideNext("stars", "settings"), "settings");
  assert.equal(besideNext("settings", "unmapped"), null);
  assert.equal(besideFront("unmapped"), "unmapped");
  assert.equal(besideNext("board", "stars"), "stars");
  for (const owner of ["stars", "settings", "unmapped", "board"] as const) {
    assert.equal(besideNext(owner, owner), owner);
  }
  assert.equal(besideNext(null, "planet"), null);
  assert.equal(besideNext("stars", "add"), null);
  assert.equal(besideFront("stars"), "arch");
  assert.equal(besideFront("settings"), "arch");
  assert.equal(besideFront("board"), "board");
  assert.equal(besideFront("planet"), "layer2");
});

test("F2 selection paths route through applyBeside", () => {
  const click = fnBody(list, "onTileClick");
  const planet = click.slice(click.indexOf('item.kind === "planet"'), click.indexOf('item.kind === "unmapped"'));
  const unmapped = click.slice(click.indexOf('item.kind === "unmapped"'), click.indexOf("spinConfirm"));
  assert.match(planet, /applyBeside\(\s*"planet"\s*\)/);
  assert.match(unmapped, /applyBeside\(\s*"unmapped"\s*\)/);
  assert.match(click, /applyBeside\(\s*"stars"\s*\)/);
  assert.match(click, /applyBeside\(\s*"settings"\s*\)/);
  const wheel = list.slice(list.indexOf("const onWheel = (e: WheelEvent)"), list.indexOf('el.addEventListener("wheel"'));
  assert.match(wheel, /applyBeside\(/);
  assert.match(list, /rail\.applyBeside\(pickFromKind\(list\[next\]\?\.kind\)\)/);
  const arrowAt = page.indexOf("const applyArrow = (activeId: string, dir: string)");
  const apply = page.slice(arrowAt, page.indexOf("menuRing.setStops(stops)", arrowAt));
  assert.match(apply, /applyBeside\(/);
  assert.match(apply, /walkInto\(/);
  const open = fnBody(page, "openStageDest");
  assert.match(open, /applyBeside\(/);
  assert.doesNotMatch(open, /stageEmbed\s*=/);
  const beside = fnBody(page, "applyBeside");
  assert.match(beside, /besideNext\(/);
  assert.match(beside, /clearArchToFront|archBack/);
  assert.match(beside, /hideBesidePopouts\(/);
  const popouts = fnBody(page, "hideBesidePopouts");
  assert.match(popouts, /hide_window/);
  assert.match(popouts, /stars-beside/);
  assert.match(popouts, /settings-beside/);
  assert.match(fnBody(page, "archDo"), /applyBeside\(/);
  const archDo = fnBody(page, "archDo");
  assert.match(archDo, /next\.pid !== prevPid/);
  const confirm = fnBody(page, "confirmHabitatStop");
  assert.match(confirm, /applyBeside\(\s*"planet"\s*\)/);
  assert.match(confirm, /applyBeside\(\s*"unmapped"\s*\)/);
});

test("F3 keyOwnerNext is exactly one owner", () => {
  const owners: KeyOwner[] = ["rail", "stars", "settings", "unmapped", "board"];
  const targets = ["stars", "settings", "unmapped", "board"] as const;
  for (const owner of owners) {
    for (const target of targets) {
      assert.equal(keyOwnerNext(owner, { type: "walkIn", target }), target);
    }
    assert.equal(keyOwnerNext(owner, { type: "walkOut" }), "rail");
    assert.equal(keyOwnerNext(owner, { type: "panelClosed" }), "rail");
    assert.equal(keyOwnerNext(owner, { type: "select", other: "settings" }), "rail");
    const next = keyOwnerNext(owner, { type: "walkOut" });
    assert.equal(owners.includes(next), true);
    assert.equal(typeof next, "string");
  }
});

test("F4 embedded rings attach on walk-in only", () => {
  const mountAt = stars.indexOf("onMount(async () =>");
  assert.ok(mountAt > 0);
  const mount = braceBody(stars, mountAt);
  assert.equal(mount.includes("menuRing.attach"), false);
  assert.equal(stars.split("menuRing.attach").length - 1, 1);
  const guardAt = stars.indexOf("if (!embedded || !ringActive) return");
  assert.ok(stars.indexOf("menuRing.attach(", guardAt) > guardAt);
  assert.match(stars, /if\s*\(\s*!embedded\s*\|\|\s*!ringActive\s*\)\s*return/);
  assert.match(stars, /onRingExit\?\.\(\)/);
  assert.match(stars, /menuRing\.onEdge/);
  assert.match(settings, /if\s*\(\s*!embedded\s*\|\|\s*!ringActive\s*\)\s*return;\s*const root = settingsRoot;[\s\S]*?menuRing\.attach\(/);
  assert.match(settings, /onRingExit\?\.\(\)/);
  assert.equal(settings.includes("menuRing.attach"), true);
  assert.match(kanban, /if\s*\(\s*railPlanet\s*==\s*null\s*\)/);
  assert.match(kanban, /if\s*\(\s*!ringActive\s*\)\s*return/);
  assert.match(kanban, /onRingExit\?\.\(\)/);
  const walk = fnBody(page, "walkOut");
  assert.match(walk, /reclaimRailRing\(/);
  assert.match(walk, /keyOwnerNext\(/);
  assert.match(page, /reselectStop/);
  assert.match(page, /menuRing\.select\(id\)/);
  assert.match(list, /ringActive=\{rail\.keyOwner === "stars"\}/);
  assert.match(list, /ringActive=\{rail\.keyOwner === "settings"\}/);
  assert.match(list, /ringActive=\{rail\.keyOwner === "board"\}/);
  assert.match(list, /onRingExit=\{\(\) => rail\.walkOut\(\)\}/);
});

test("F5 archArrow walks in and leaves coach arrows alone", () => {
  const items = spinItems(["a"], "__unmapped__");
  const at = (kind: SpinItem["kind"]) => items.findIndex((it) => it.kind === kind);
  const layer2 = archLayer2Items({
    pid: "a",
    cards: [],
    rows: [],
  });
  const starsHit = archArrow(ARCH_IDLE, items, at("stars"), "right", "left", layer2);
  assert.equal(starsHit?.walkIn, "stars");
  const settingsHit = archArrow(ARCH_IDLE, items, at("settings"), "right", "left", layer2);
  assert.equal(settingsHit?.walkIn, "settings");
  const unmappedHit = archArrow(ARCH_IDLE, items, at("unmapped"), "right", "left", layer2);
  assert.equal(unmappedHit?.walkIn, "unmapped");
  assert.equal(unmappedHit?.state.front, "layer2");
  const planetHit = archArrow(ARCH_IDLE, items, at("planet"), "right", "left", layer2);
  assert.equal(planetHit?.state.front, "layer2");
  assert.equal(planetHit?.walkIn ?? null, null);
  const onBoard: ArchState = { front: "layer2", pid: "a", center2: 0, pending: null };
  const boardHit = archArrow(onBoard, items, at("planet"), "right", "left", layer2);
  assert.equal(boardHit?.state.front, "board");
  assert.equal(boardHit?.walkIn, "board");
  const coach = archArrow(ARCH_IDLE, items, at("planet"), "down", "left", layer2, "rcoach-claim", true);
  assert.ok(coach);
  assert.equal(coach.walkIn ?? null, null);
  assert.notEqual(coach.stop, null);
});

test("F6 layer 2 spins like layer 1", () => {
  const n = 5;
  const back = archScroll2({ front: "layer2", pid: "a", center2: 0, pending: null }, -1, n);
  assert.equal(back.front, "layer2");
  assert.equal(back.center2, n - 1);
  const fwd = archScroll2({ front: "layer2", pid: "a", center2: n - 1, pending: null }, 1, n);
  assert.equal(fwd.front, "layer2");
  assert.equal(fwd.center2, 0);
  const items = spinItems(["a"], "__unmapped__");
  const layer2 = [
    { kind: "board" as const, stop: "rl2-board", title: "Board" },
    { kind: "needed" as const, stop: "rl2-n", title: "Need" },
    { kind: "window" as const, stop: "rl2-w", title: "Win" },
  ];
  const end: ArchState = { front: "layer2", pid: "a", center2: layer2.length - 1, pending: null };
  const wrapped = archArrow(end, items, 2, "down", "left", layer2);
  assert.equal(wrapped?.state.front, "layer2");
  assert.equal(wrapped?.state.center2, 0);
  const start: ArchState = { front: "layer2", pid: "a", center2: 0, pending: null };
  const backed = archArrow(start, items, 2, "up", "left", layer2);
  assert.equal(backed?.state.front, "layer2");
  assert.equal(backed?.state.center2, layer2.length - 1);
  const out = archArrow(start, items, 2, "left", "left", layer2);
  assert.equal(out?.state.front, "arch");
  const offsetFn = fnBody(list, "l2Offset");
  assert.match(offsetFn, /spinOffset\(index,\s*archState\?\.center2 \?\? 0,\s*layer2\.length\)/);
  assert.match(list, /spinJump\(prevL2/);
});

test("F7 archHalo ignores hover and lights only the centre or active item", () => {
  const kinds = ["planet", "stars", "settings", "unmapped", "add", "window", "needed", "board"];
  for (const kind of kinds) {
    assert.equal(archHalo(kind, { center: false, active: false }), "none", kind);
    const on = archHalo(kind, { center: true, active: false });
    const active = archHalo(kind, { center: false, active: true });
    assert.equal(on, "ring", kind);
    assert.equal(active, "ring", kind);
  }
  const src = readFileSync(new URL("../src/lib/rail/stageFocus.ts", import.meta.url), "utf8");
  const halo = fnBody(src, "archHalo");
  assert.equal(halo.includes("hover"), false);
});

test("F8 selWrapBox covers marble and label on every edge", () => {
  const edges = ["left", "right", "top", "bottom"] as const;
  const frame = { w: 1040, h: 760 };
  for (const edge of edges) {
    for (const k of [-2, 0, 2]) {
      const marble = archMarbleBox(k, edge, 1, frame, k === 0);
      const label = archLabelBox(k, edge, 1, frame, { avoid: [] });
      const wrap = selWrapBox(marble, label);
      for (const box of [marble, label]) {
        assert.ok(wrap.x <= box.x - 6 + 0.01, `${edge} ${k} x`);
        assert.ok(wrap.y <= box.y - 6 + 0.01, `${edge} ${k} y`);
        assert.ok(wrap.x + wrap.w >= box.x + box.w + 6 - 0.01, `${edge} ${k} r`);
        assert.ok(wrap.y + wrap.h >= box.y + box.h + 6 - 0.01, `${edge} ${k} b`);
      }
      assert.equal(label.side === "below" || label.side === "inboard", true);
    }
  }
  const blocks = cssRuleBlocks(css);
  const rule = blocks.find((b) => /\.arch-sel\b/.test(b.selector) && /border-radius/.test(b.body));
  assert.ok(rule);
  assert.match(rule.body, /border-radius:\s*999px/);
  const stripped = rule.body.replace(/var\(\s*--focus\s*,\s*#6e4db0\s*\)/g, "var(--focus)");
  assert.doesNotMatch(stripped, /#[0-9a-fA-F]{3,8}/);
  assert.match(rule.body, /var\(\s*--focus/);
  assert.match(rule.body, /color-mix/);
  const style = fnBody(list, "selStyle");
  assert.match(style, /archLabelBox\(k, edge, ring, frame, \{ avoid \}\)/);
  assert.match(style, /selWrapBox\(marble, label\)/);
});

function lastSubject(part: string): string {
  const bits = part.trim().split(/\s+/);
  return bits[bits.length - 1] ?? "";
}

function propsOf(body: string): string[] {
  return [...body.matchAll(/(?:^|[;{\s])([a-z-]+)\s*:/g)].map((m) => m[1]);
}

test("F9 no rectangles on arch tiles", () => {
  assert.equal(/class="arch-hit"[\s\S]{0,240}mkey-sel/.test(list), false);
  assert.equal((list.match(/class="arch-hit"/g) ?? []).length, 2);
  assert.match(list, /class:mkey-sel=\{rail\.menuRing\.activeId === COACH_CLAIM_STOP\}/);
  const blocks = cssRuleBlocks(stripComments(css));
  for (const block of blocks) {
    const scoped = /\.arch-hit\b|\.arch-slot\b|\.arch-vis\b|\.arch-back\b|\.arch-sel\b|\.arch-ringlet\b|\.arch-marble\b|\.arch-glyph\b/.test(
      block.selector,
    );
    if (!scoped) continue;
    if (/outline\s*:/.test(block.body)) assert.match(block.body, /outline:\s*none/, block.selector);
    if (/border\s*:/.test(block.body) && !/border-radius\s*:/.test(block.body.replace(/border-radius/g, ""))) {
      const borderOk = /border:\s*(?:0|none)\b/.test(block.body) || /\.arch-ringlet\b/.test(block.selector);
      assert.equal(borderOk, true, block.selector);
    }
    if (/box-shadow\s*:/.test(block.body)) {
      const none = /box-shadow:\s*none/.test(block.body);
      const round = /\.arch-marble\b|\.arch-sel\b/.test(block.selector);
      assert.equal(none || round, true, block.selector);
    }
    const hoverish = /:hover\b|\.reduce-motion\b|:has\(\.arch-hit:active\)/.test(block.selector);
    if (hoverish) {
      for (const prop of propsOf(block.body)) {
        const shadowOnRound = prop === "box-shadow" && /\.arch-marble\b|\.arch-sel\b/.test(block.selector);
        assert.ok(
          shadowOnRound || ["transform", "filter", "animation", "transition"].includes(prop),
          `${block.selector} ${prop}`,
        );
      }
    }
  }
  const hit = blocks.find((b) => lastSubject(b.selector.split(",")[0] ?? "") === ".arch-hit" && /width:\s*88px/.test(b.body));
  assert.ok(hit);
  assert.match(hit.body, /border-radius:\s*50%/);
});

test("F10 ringlet is hidden unless halo or focus; glow is round", () => {
  const blocks = cssRuleBlocks(stripComments(css));
  const ringlet = blocks.find((b) => lastSubject(b.selector) === ".arch-ringlet" && /display\s*:/.test(b.body));
  assert.ok(ringlet);
  assert.match(ringlet.body, /display:\s*none/);
  const show = blocks.find(
    (b) => /data-halo="ring"/.test(b.selector) && /focus-visible/.test(b.selector) && /display:\s*block/.test(b.body),
  );
  assert.equal(show, undefined);
  const ring = blocks.find((b) => /\.arch-sel\b/.test(b.selector) && /var\(\s*--focus-ring\s*\)/.test(b.body));
  assert.ok(ring);
  const glow = blocks.find((b) => b.selector.includes(".arch-vis::before") && /width\s*:/.test(b.body));
  assert.ok(glow);
  const width = /width:\s*(\d+)px/.exec(glow.body);
  const height = /height:\s*(\d+)px/.exec(glow.body);
  assert.ok(width && height);
  assert.equal(width[1], height[1]);
  assert.match(glow.body, /border-radius:\s*50%/);
});

test("F11 geometry subjects stay on the known list", () => {
  const blocks = cssRuleBlocks(stripComments(css));
  const guarded = new Set([".arch-hit", ".arch-vis", ".l2-vis"]);
  for (const block of blocks) {
    for (const part of block.selector.split(",")) {
      const raw = lastSubject(part);
      if (raw.includes("::")) continue;
      const subject = raw.replace(/\[[^\]]*\]/g, "").replace(/:[^[\s]+/g, "");
      if (!guarded.has(subject)) continue;
      const interesting = propsOf(block.body).filter((p) =>
        p === "left" || p === "top" || p === "inset" || p === "transform" || p.startsWith("margin"),
      );
      if (!interesting.length) continue;
      if (subject === ".arch-hit") {
        const extra = interesting.filter((p) => p !== "left" && p !== "top" && p !== "margin");
        assert.deepEqual(extra, [], block.selector);
        if (interesting.includes("margin")) {
          for (const decl of block.body.matchAll(/margin(?:-[a-z]+)?\s*:\s*([^;]+)/g)) {
            assert.equal(decl[1].trim(), "0", block.selector);
          }
        }
        if (interesting.includes("left") || interesting.includes("top")) {
          assert.match(block.body, /width:\s*88px/);
        }
        assert.doesNotMatch(block.body, /transform\s*:/);
      } else if (interesting.includes("transform")) {
        assert.match(block.body, /transform:\s*(?:none|scale\(1\.06\)|scale\(0\.97\))\s*;/);
      } else {
        assert.ok(block.selector.includes(".arch-vis") && block.selector.includes(".l2-vis"), block.selector);
        assert.doesNotMatch(block.selector, /:hover|:active|reduce-motion/);
        assert.doesNotMatch(block.body, /translate\s*\(/);
      }
    }
    for (const part of block.selector.split(",")) {
      const raw = lastSubject(part);
      if (raw.includes("::")) continue;
      const subject = raw.replace(/\[[^\]]*\]/g, "").replace(/:[^[\s]+/g, "");
      if (![".arch-marble-wrap", ".arch-glyph", ".l2-app-icon"].includes(subject)) continue;
      if (!/transform\s*:/.test(block.body)) continue;
      assert.match(block.body, /translate\(\s*-50%\s*,\s*-50%\s*\)/, block.selector);
    }
  }
});

test("F12 polish survivors stay in the polish test", () => {
  assert.ok(polish.includes("only the in-flight guard returns before setTimeout"));
  assert.ok(polish.includes("ring is not reassigned before archLabelBox"));
  assert.ok(polish.includes("mirror\\.expanded"));
  const style = fnBody(list, "labelStyle");
  assert.equal(style.slice(0, style.indexOf("archLabelBox(")).includes("ring ="), false);
  assert.match(style, /const avoid = \[mirror\.expanded\];/);
  const grace = fnBody(page, "scheduleGraceRecheck");
  const timeoutAt = grace.indexOf("setTimeout(");
  assert.ok(timeoutAt > 0);
  const head = grace.slice(0, timeoutAt);
  assert.equal((head.match(/\breturn\b/g) ?? []).length, 1);
});

test("F13 preview card shrinks onto stage tokens", () => {
  const blocks = cssRuleBlocks(stripComments(css));
  const thumb = blocks.find((b) => b.selector.includes(".l2-thumb") && /max-height/.test(b.body));
  assert.ok(thumb);
  const maxH = Number(/max-height:\s*(?:var\(--pv-thumb,\s*)?(\d+)px/.exec(thumb.body)?.[1]);
  assert.equal(maxH, 320);
  assert.match(thumb.body, /max-width:\s*100%/);
  const card = blocks.find((b) => b.selector.includes(".l2-window-card") && /max-width/.test(b.body));
  assert.ok(card);
  const maxW = Number(/max-width:\s*(?:var\(--pv-card,\s*)?(\d+)px/.exec(card.body)?.[1]);
  assert.equal(maxW, 600);
  assert.match(card.body, /flex:\s*none/);
  assert.match(card.body, /align-self:\s*center/);
  assert.match(card.body, /background:\s*transparent/);
  assert.match(card.body, /box-shadow:\s*0 0 0 2px color-mix\(in srgb, var\(--planet, #8a8f98\) 70%, transparent\)/);
  assert.doesNotMatch(card.body, /--stage-platter/);
  assert.match(card.body, /var\(\s*--stage-ink/);
  assert.doesNotMatch(card.body, /rgba\(\s*248\s*,\s*246\s*,\s*252/);
  assert.match(css, /\.l2-expanded:has\(> \.l2-window-card\)[\s\S]*?background:\s*transparent/);
  assert.match(list, /Screen Recording needed for previews/);
});

test("S5 outwardDir is the walk-out arrow for every edge", () => {
  assert.equal(outwardDir("right"), "right");
  assert.equal(outwardDir("left"), "left");
  assert.equal(outwardDir("top"), "up");
  assert.equal(outwardDir("bottom"), "down");
});

test("S8 pickFromKind keeps add and the other kinds", () => {
  for (const kind of ["planet", "stars", "settings", "unmapped", "board", "add"] as const) {
    assert.equal(pickFromKind(kind), kind);
  }
  assert.equal(pickFromKind("nope"), "other");
  assert.equal(pickFromKind(undefined), "other");
});

test("W3 MenuRing onEdge eats the key at the end of the ring", () => {
  const handleAt = menuKeys.indexOf("private handleKey(e: KeyboardEvent)");
  assert.ok(handleAt > 0);
  const handle = braceBody(menuKeys, handleAt);
  const edgeAt = handle.indexOf("this.onEdge?.(dir)");
  assert.ok(edgeAt > 0);
  assert.match(handle, /if\s*\(\s*atEnd\s*&&\s*this\.onEdge\?\.\(\s*dir\s*\)\s*\)\s*return true;/);
  assert.ok(edgeAt < handle.indexOf("this.move(delta)"));
});

test("W4 board outer column calls onRingExit", () => {
  assert.match(
    kanban,
    /if\s*\(\s*exitActive\s*&&\s*dir\s*===\s*exitDir\s*&&\s*\(\s*dir\s*===\s*"left"\s*\|\|\s*dir\s*===\s*"right"\s*\)\s*\)\s*\{[\s\S]*?if\s*\(\s*ci\s*\+\s*step\s*<\s*0\s*\|\|\s*ci\s*\+\s*step\s*>=\s*cols\.length\s*\)\s*\{\s*exit\?\.\(\);\s*return true;/,
  );
});

test("W5 rail Esc walks out of Unmapped", () => {
  const esc = fnBody(page, "onRailEscape");
  assert.match(esc, /if\s*\(\s*(?:scope\.)?keyOwner\s*===\s*"unmapped"\s*\)\s*\{\s*(?:scope\.)?walkOut\(\);\s*return true;\s*\}/);
  const walk = fnBody(page, "walkOut");
  assert.match(walk, /reselectStop\s*=\s*(?:scope\.)?ownerStop\((?:scope\.)?keyOwner\)/);
  assert.match(walk, /(?:scope\.)?keyOwnerNext\(\s*(?:scope\.)?keyOwner\s*,\s*\{\s*type:\s*"walkOut"\s*\}\s*\)/);
  assert.match(fnBody(page, "ownerStop"), /owner === "unmapped"\) return `rhead-\$\{(?:scope\.)?UNMAPPED_KEY\}`/);
});

test("FR2 Enter on a planet stop calls applyBeside", () => {
  const confirm = fnBody(page, "confirmHabitatStop");
  assert.match(confirm, /if\s*\(\s*(?:scope\.)?spin\s*===\s*"raise"\s*\)\s*\{\s*(?:scope\.)?applyBeside\(\s*"planet"\s*\)/);
});
