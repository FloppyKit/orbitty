import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { parse } from "svelte/compiler";
import { columnMode, columnPx } from "../src/lib/board/planetColumn.ts";
import {
  SPIN_SETTLE_MS,
  SPIN_STEP_PX,
  spinAccumulate,
  spinArrow,
  spinCenterFor,
  spinConfirm,
  spinInitial,
  spinItems,
  spinJump,
  spinOffset,
  spinPane,
  spinStep,
  wheelLayout,
  wheelSlot,
  WHEEL_COL,
  WHEEL_ROW,
  type SpinDir,
  type SpinItem,
} from "../src/lib/rail/stageLook.ts";

const U = "__unmapped__";
const edges = ["left", "right", "top", "bottom"] as const;
const dirs: SpinDir[] = ["up", "down", "left", "right"];

function axis(edge: string): { back: SpinDir; forward: SpinDir; inward: SpinDir; outward: SpinDir } {
  if (edge === "right") return { back: "up", forward: "down", inward: "left", outward: "right" };
  if (edge === "top") return { back: "left", forward: "right", inward: "down", outward: "up" };
  if (edge === "bottom") return { back: "left", forward: "right", inward: "up", outward: "down" };
  return { back: "up", forward: "down", inward: "right", outward: "left" };
}

test("P1 spin order", () => {
  const stops = spinItems(["a", "b", "c"], U).map((it) => it.stop);
  assert.deepEqual(stops, [
    "rstars",
    "rsettings",
    "rhead-a",
    "rhead-b",
    "rhead-c",
    `rhead-${U}`,
    "rnewproj",
  ]);
  const shuffled = ["m", "a", "z"];
  const planets = spinItems(shuffled, U)
    .filter((it) => it.kind === "planet")
    .map((it) => it.pid);
  assert.deepEqual(planets, shuffled);
  assert.equal(spinItems(shuffled, U)[0].stop, "rstars");
  assert.equal(spinItems(shuffled, U)[1].stop, "rsettings");
});

test("P2 wrap step", () => {
  for (const n of [4, 7]) {
    assert.equal(spinStep(n - 1, 1, n), 0);
    assert.equal(spinStep(0, -1, n), n - 1);
    assert.equal(spinStep(2, n, n), 2);
    assert.equal(spinStep(2, -n, n), 2);
  }
  assert.equal(spinStep(3, 1, 0), 0);
  assert.equal(spinStep(4, -1, 1), 0);
});

test("P3 shortest offset", () => {
  const n = 7;
  const c = 0;
  for (let i = 0; i < n; i++) {
    const k = spinOffset(i, c, n);
    assert.ok(k > -n / 2 && k <= n / 2, `k ${k}`);
  }
  assert.equal(spinOffset(c, c, n), 0);
  assert.equal(spinOffset(6, 0, n), -1);
  assert.equal(spinOffset(1, 0, n), 1);
  assert.equal(spinOffset(0, 3, n), spinOffset(0, 3, n));
  assert.equal(spinOffset(0, 3, n), -3);
  assert.equal(spinOffset(4, 3, n), 1);
});

test("P4 wrapped neighbours", () => {
  const items = spinItems(["a", "b", "c"], U);
  const n = items.length;
  assert.ok(n >= 5);
  const c = items.findIndex((it) => it.stop === "rstars");
  assert.equal(c, 0);
  const kNew = spinOffset(items.findIndex((it) => it.stop === "rnewproj"), c, n);
  const kSet = spinOffset(items.findIndex((it) => it.stop === "rsettings"), c, n);
  assert.equal(kNew, -1);
  assert.equal(kSet, 1);
  assert.equal(wheelSlot(kNew).hidden, false);
  assert.equal(wheelSlot(kSet).hidden, false);
  const centre = wheelSlot(spinOffset(c, c, n));
  assert.ok(centre.scale >= 1.45 && centre.scale <= 1.55);
  assert.equal(wheelSlot(4).hidden, true);
  assert.equal(wheelSlot(-4).hidden, true);
  const wide = spinItems(["a", "b", "c", "d", "e", "f", "g", "h"], U);
  const far = spinOffset(4, 0, wide.length);
  assert.ok(Math.abs(far) > 3);
  assert.equal(wheelSlot(far).hidden, true);
});

test("P5 wrap jump", () => {
  assert.equal(spinJump(3, -3), true);
  assert.equal(spinJump(1, 0), false);
  assert.equal(spinJump(-1, 1), true);
  assert.equal(spinJump(0, 0), false);
  assert.equal(spinJump(2, 1), false);
});

test("P6 scroll accumulate + snap", () => {
  const zero = { residual: 0, lastAt: 0 };
  let out = spinAccumulate(zero, { deltaX: 0, deltaY: SPIN_STEP_PX - 10, deltaMode: 0 }, 10);
  assert.equal(out.steps, 0);
  assert.ok(Math.abs(out.state.residual) < SPIN_STEP_PX);
  out = spinAccumulate(out.state, { deltaX: 0, deltaY: 20, deltaMode: 0 }, 20);
  assert.equal(out.steps, 1);

  const mid = spinAccumulate(zero, { deltaX: 0, deltaY: 30, deltaMode: 0 }, 100);
  assert.equal(mid.steps, 0);
  const rev = spinAccumulate(mid.state, { deltaX: 0, deltaY: -10, deltaMode: 0 }, 110);
  assert.equal(rev.steps, 0);
  assert.ok(rev.state.residual < 0);
  assert.ok(Math.abs(rev.state.residual) < SPIN_STEP_PX);

  const line = spinAccumulate(
    { residual: 40, lastAt: 0 },
    { deltaX: 0, deltaY: 12, deltaMode: 1 },
    200,
  );
  assert.equal(line.steps, 1);
  assert.equal(line.state.residual, 0);
  const page = spinAccumulate(zero, { deltaX: -4, deltaY: 0, deltaMode: 2 }, 210);
  assert.equal(page.steps, -1);

  const huge = spinAccumulate(zero, { deltaX: 0, deltaY: 5000, deltaMode: 0 }, 300);
  assert.equal(huge.steps, 2);

  const dirty = { residual: 40, lastAt: 1000 };
  const settled = spinAccumulate(
    dirty,
    { deltaX: 0, deltaY: 0, deltaMode: 0 },
    1000 + SPIN_SETTLE_MS + 1,
  );
  assert.equal(settled.steps, 0);
  assert.equal(settled.state.residual, 0);
});

test("P7 keyboard spin + reachability", () => {
  for (const count of [0, 1, 5]) {
    const ids = Array.from({ length: count }, (_, i) => `p${i}`);
    const items = spinItems(ids, U);
    for (const edge of edges) {
      const ax = axis(edge);
      const last = items[items.length - 1].stop;
      assert.equal(spinArrow(items, "rnewproj", ax.forward, edge), "rstars");
      assert.equal(spinArrow(items, "rstars", ax.back, edge), "rnewproj");
      assert.equal(spinArrow(items, "rstars", ax.inward, edge), null);
      assert.equal(spinArrow(items, "rstars", ax.outward, edge), null);
      assert.equal(spinArrow(items, last, ax.forward, edge), "rstars");
      if (edge === "left" || edge === "right") {
        assert.equal(ax.forward, "down");
        assert.equal(ax.back, "up");
      } else {
        assert.equal(ax.forward, "right");
        assert.equal(ax.back, "left");
      }
      const seen = new Set<string>();
      const q = ["rstars"];
      while (q.length) {
        const id = q.pop() as string;
        if (seen.has(id)) continue;
        seen.add(id);
        for (const dir of dirs) {
          const next = spinArrow(items, id, dir, edge);
          if (next) q.push(next);
        }
      }
      for (const it of items) assert.equal(seen.has(it.stop), true, `${edge} ${it.stop}`);
    }
  }
});

test("P8 centre sync", () => {
  const items = spinItems(["a", "b", "c"], U);
  assert.equal(
    spinCenterFor(items, "rhead-b", 0),
    items.findIndex((it) => it.stop === "rhead-b"),
  );
  assert.equal(spinCenterFor(items, null, 1), 1);
  assert.equal(spinCenterFor(items, "nope", 9), items.length - 1);
  assert.equal(spinCenterFor(items, "nope", -3), 0);

  const two = spinItems(["a", "b"], U);
  assert.equal(spinInitial(two, "b"), two.findIndex((it) => it.kind === "planet" && it.pid === "b"));
  const none = spinItems([], U);
  assert.equal(spinInitial(none, null), none.findIndex((it) => it.kind === "unmapped"));
  const some = spinItems(["a", "b", "c"], U);
  assert.equal(spinInitial(some, null), 2);
  assert.equal(spinInitial(some, "missing"), 2);
});

test("P9 board pane", () => {
  const items = spinItems(["p1"], U);
  const by = (kind: SpinItem["kind"]) => items.find((it) => it.kind === kind) as SpinItem;
  assert.deepEqual(spinPane(by("planet")), { kind: "board", selected: "p1" });
  assert.deepEqual(spinPane(by("stars")), { kind: "board", selected: "inbox" });
  assert.deepEqual(spinPane(by("settings")), { kind: "board", selected: "inbox" });
  assert.deepEqual(spinPane(by("add")), { kind: "board", selected: "inbox" });
  assert.deepEqual(spinPane(by("unmapped")), { kind: "unclaimed" });
});

test("P10 board embed without planet column", () => {
  assert.equal(columnMode(1200, "hidden"), "hidden");
  assert.equal(columnMode(300, "hidden"), "hidden");
  assert.equal(columnPx("hidden"), 0);
  assert.equal(columnPx("open"), 168);
  assert.equal(columnPx("icons"), 72);
  assert.equal(columnMode(500, "auto"), "icons");
  assert.equal(columnMode(640, "auto"), "open");
  assert.equal(columnMode(900, "open"), "open");
  assert.equal(columnMode(100, "icons"), "icons");
});

test("P11 small window still fits", () => {
  const left = wheelLayout("left", 1040, 760);
  assert.ok(left.preview.x >= WHEEL_COL);
  assert.ok(left.preview.y >= 0);
  assert.ok(left.preview.x + left.preview.w <= 1040);
  assert.ok(left.preview.y + left.preview.h <= 760);
  assert.ok(left.preview.w >= 560);
  assert.ok(left.preview.h >= 600);

  const right = wheelLayout("right", 1040, 760);
  assert.ok(right.preview.x >= 0);
  assert.ok(right.preview.x + right.preview.w <= right.wheel.x);
  assert.ok(right.wheel.x >= right.preview.x + right.preview.w);
  assert.ok(right.preview.w >= 560);
  assert.ok(right.preview.h >= 600);
  assert.ok(right.preview.y + right.preview.h <= 760);

  const top = wheelLayout("top", 1040, 760);
  assert.ok(top.preview.y >= WHEEL_ROW);
  assert.ok(top.preview.h >= 400);
  assert.ok(top.preview.x >= 0);
  assert.ok(top.preview.x + top.preview.w <= 1040);
  assert.ok(top.preview.y + top.preview.h <= 760);
});

test("P12 fresh-profile first claim", () => {
  const items = spinItems([], U);
  assert.deepEqual(
    items.map((it) => it.stop),
    ["rstars", "rsettings", `rhead-${U}`, "rnewproj"],
  );
  const start = spinInitial(items, null);
  assert.equal(items[start].kind, "unmapped");
  assert.deepEqual(spinPane(items[start]), { kind: "unclaimed" });
  assert.equal(spinArrow(items, items[start].stop, "down", "left"), "rnewproj");
  assert.equal(spinConfirm("rnewproj"), "newProject");
  assert.equal(spinConfirm("rhead-p1"), "raise");
  assert.equal(spinConfirm("rstars"), "stars");
  assert.equal(items.some((it) => it.stop === "rnewproj"), true);
});

test("P3 even count keeps the positive half", () => {
  // n = 4, d = n/2. `>` keeps +2; `>=` wraps to -2 (V10).
  assert.equal(spinOffset(2, 0, 4), 2);
  assert.notEqual(spinOffset(2, 0, 4), -2);
});

test("P6 step and settle bounds", () => {
  assert.ok(SPIN_STEP_PX >= 48 && SPIN_STEP_PX <= 72, String(SPIN_STEP_PX));
  assert.ok(SPIN_SETTLE_MS >= 120 && SPIN_SETTLE_MS <= 180, String(SPIN_SETTLE_MS));
});

test("P12 unmapped centre and settings confirm", () => {
  assert.equal(spinConfirm("rhead-__unmapped__"), "centre");
  assert.notEqual(spinConfirm("rhead-__unmapped__"), "raise");
  assert.equal(spinConfirm("rsettings"), "settings");
  assert.notEqual(spinConfirm("rsettings"), "stars");
});

type AstNode = {
  type?: string;
  name?: string;
  start?: number;
  end?: number;
  operator?: string;
  test?: AstNode;
  consequent?: AstNode;
  alternate?: AstNode | null;
  fragment?: AstNode;
  nodes?: AstNode[];
  attributes?: AstNode[];
  [key: string]: unknown;
};

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

function childrenOf(node: AstNode): AstNode[] {
  const out: AstNode[] = [];
  for (const key of ["fragment", "nodes", "consequent", "alternate", "attributes"]) {
    const v = node[key];
    if (!v) continue;
    if (Array.isArray(v)) out.push(...(v as AstNode[]));
    else if (typeof v === "object") out.push(v as AstNode);
  }
  return out;
}

function walk(node: AstNode | null | undefined, fn: (node: AstNode) => void) {
  if (!node || typeof node !== "object") return;
  fn(node);
  for (const child of childrenOf(node)) walk(child, fn);
}

function fragmentOf(file: string): AstNode {
  const ast = parse(file, { modern: true }) as unknown as AstNode;
  assert.equal(ast.fragment?.type, "Fragment");
  return ast.fragment as AstNode;
}

const G4_TEST = 'railPlanet == null && planetColMode !== "hidden"';

test("G4 railPlanet conjunction guards column and chrome", () => {
  const file = src("src/lib/KanbanPage.svelte");
  const root = fragmentOf(file);
  const guarded = new Set<string>();
  walk(root, (node) => {
    if (node.type !== "IfBlock" || !node.test || !node.consequent) return;
    const testSrc = file.slice(node.test.start ?? 0, node.test.end ?? 0);
    if (node.test.type !== "LogicalExpression" || node.test.operator !== "&&") return;
    if (testSrc !== G4_TEST) return;
    walk(node.consequent, (inner) => {
      if (inner.type === "Component" && (inner.name === "PlanetColumn" || inner.name === "BoardChrome")) {
        guarded.add(inner.name as string);
      }
    });
  });
  assert.equal(guarded.has("PlanetColumn"), true);
  assert.equal(guarded.has("BoardChrome"), true);
});

test("Done-chips guard is not a dead if", () => {
  const file = src("src/lib/board/Chrome.svelte");
  const root = fragmentOf(file);
  let render = false;
  let chips = false;
  function walkIf(node: AstNode | null | undefined, ifs: string[]) {
    if (!node || typeof node !== "object") return;
    const next =
      node.type === "IfBlock" && node.test
        ? [...ifs, file.slice(node.test.start ?? 0, node.test.end ?? 0).trim()]
        : ifs;
    if (node.type === "RenderTag") {
      const slice = file.slice(node.start ?? 0, node.end ?? 0);
      if (slice.includes("doneActorChips")) {
        render = true;
        assert.equal(next.includes("false"), false, next.join(" | "));
        assert.equal(next[next.length - 1], 'selected === "done"');
      }
    }
    if (node.type === "Attribute" && node.name === "chips") {
      const slice = file.slice(node.start ?? 0, node.end ?? 0);
      if (slice.includes("doneActorChips")) {
        chips = true;
        assert.equal(next.includes("false"), false, next.join(" | "));
        assert.equal(slice, 'chips={selected === "done" ? doneActorChips : undefined}');
      }
    }
    for (const child of childrenOf(node)) walkIf(child, next);
  }
  walkIf(root, []);
  assert.equal(render, true);
  assert.equal(chips, true);
});

function rustFn(file: string, name: string): string {
  const start = file.indexOf(name);
  assert.ok(start >= 0, name);
  const brace = file.indexOf("{", start);
  let depth = 0;
  for (let i = brace; i < file.length; i++) {
    if (file[i] === "{") depth += 1;
    else if (file[i] === "}") {
      depth -= 1;
      if (depth === 0) return file.slice(start, i + 1);
    }
  }
  throw new Error(`unclosed ${name}`);
}

test("unknown-edge size uses the left arm", () => {
  // 1440×875 cannot tell left from top (both 1040×760). 1000×700 can:
  // left/unknown = (992, 700); top = (1000, 692). The `_` arm is that left formula.
  const file = src("src-tauri/src/state/rail_geom.rs");
  const body = rustFn(file, "pub fn rail_wheel_size");
  const docAt = file.lastIndexOf("Unknown edges are left", file.indexOf("pub fn rail_wheel_size"));
  assert.ok(docAt >= 0);
  assert.match(
    body,
    /"top" \| "bottom" => \(vw\.min\(RAIL_SPIN_W\), \(vh - 8\.0\)\.max\(0\.0\)\.min\(RAIL_SPIN_H\)\)/,
  );
  assert.match(
    body,
    /_ => \(\(vw - 8\.0\)\.max\(0\.0\)\.min\(RAIL_SPIN_W\), vh\.min\(RAIL_SPIN_H\)\)/,
  );
  const armRe = /^\s*("[^"]+"(?:\s*\|\s*"[^"]+")*|_)\s*=>/gm;
  const arms = [...body.matchAll(armRe)].map((m) => m[1]);
  assert.deepEqual(arms, ['"top" | "bottom"', "_"]);
});
