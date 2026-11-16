// 42b: a per-planet scene button on rail level 2. Entering sets every other
// window aside and puts that planet's claimed windows back in their saved
// layout; leaving restores the desk exactly as it was. Flag planet_scenes,
// default off.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  autoHideWanted,
  sceneEnter,
  sceneLeave,
  SCENE_TOLERANCE,
  type SceneSession,
  type SceneWindowRow,
} from "../src/lib/rail/habitat.ts";
// Namespace on purpose: a missing hold symbol then fails its own assertion
// instead of taking the whole file down with a link error.
import * as habitatScene from "../src/lib/rail/habitat.ts";
import { mirrorItems, stagePreviewModel } from "../src/lib/rail/stagePlace.ts";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

function src(rel: string): string {
  return readFileSync(join(root, rel), "utf8");
}

/** One function body, from its `function name(` to the first column-0 `}`. */
function fnText(rel: string, name: string): string {
  const body = src(rel);
  const at = body.indexOf(`function ${name}(`);
  assert.ok(at >= 0, `${name} present`);
  const end = body.indexOf("\n}", at);
  assert.ok(end > at, `${name} has a body`);
  return body.slice(at, end);
}

function win(partial: Partial<SceneWindowRow> & { id: string }): SceneWindowRow {
  return {
    stableKey: `${partial.id}|Window|1`,
    frame: { x: 100, y: 100, w: 400, h: 300 },
    minimized: false,
    claimed: false,
    onScreen: true,
    frontmost: false,
    ...partial,
  };
}

function rect(row: SceneWindowRow): { x: number; y: number; w: number; h: number } {
  const f = row.frame;
  assert.ok(f);
  return { x: f.x, y: f.y, w: f.w, h: f.h };
}

const sample = () => ({
  pid: "p1",
  cards: [],
  rows: [
    { id: "w1", title: "Notes" },
    { id: "w2", title: "Text" },
  ],
});

test("scene_enter_minimizes_unclaimed_only", () => {
  const claimed = win({ id: "a", claimed: true });
  const other = win({ id: "b" });
  const { session, plan } = sceneEnter(null, "kansas", [claimed, other], []);
  assert.equal(session.pid, "kansas");
  assert.deepEqual(plan.minimize, ["b"]);
  assert.deepEqual(plan.focus, ["a"]);
  assert.deepEqual(
    session.snapshot.map((s) => s.id),
    ["a", "b"],
  );
  // The claimed window keeps its frame when nothing is saved for it.
  assert.deepEqual(plan.frames, []);
});

test("scene_restores_minimized_claimed", () => {
  const claimed = win({ id: "a", claimed: true, minimized: true });
  const { plan } = sceneEnter(null, "kansas", [claimed], [
    { stableKey: "a|Window|1", x: 40, y: 60, w: 800, h: 500 },
  ]);
  assert.deepEqual(plan.restore, ["a"]);
  assert.deepEqual(plan.frames, [{ id: "a", x: 40, y: 60, w: 800, h: 500 }]);
});

test("scene_other_space_left_alone", () => {
  const otherSpace = win({ id: "b", onScreen: false });
  const secondDisplay = win({ id: "c", onScreen: false, minimized: true });
  const { session, plan } = sceneEnter(null, "kansas", [otherSpace, secondDisplay], []);
  assert.deepEqual(plan.minimize, []);
  // Still in the snapshot: leave must not strand it.
  assert.deepEqual(
    session.snapshot.map((s) => s.id),
    ["b", "c"],
  );
});

test("scene_claimed_other_space_left_alone", () => {
  // Needs Matt 4: a claimed window on another Space or a gone display is left
  // alone — not moved, not raised, not pulled out of the Dock.
  const far = win({
    id: "far",
    claimed: true,
    onScreen: false,
    frame: { x: 40, y: 60, w: 800, h: 500 },
  });
  const farMin = win({ id: "far-min", claimed: true, onScreen: false, minimized: true });
  const near = win({ id: "near", claimed: true });
  const { session, plan } = sceneEnter(null, "kansas", [far, farMin, near], [
    { stableKey: "far|Window|1", x: 10, y: 20, w: 300, h: 200 },
  ]);
  assert.deepEqual(plan.frames, []);
  assert.deepEqual(plan.focus, ["near"]);
  assert.deepEqual(plan.restore, []);
  assert.deepEqual(plan.minimize, []);
  // Still in the snapshot: leave must not strand it.
  assert.deepEqual(
    session.snapshot.map((s) => s.id),
    ["far", "far-min", "near"],
  );
});

test("scene_switch_keeps_original_snapshot", () => {
  const first = [
    win({ id: "a", claimed: true, frame: { x: 10, y: 20, w: 300, h: 200 } }),
    win({ id: "b", frame: { x: 500, y: 400, w: 600, h: 400 } }),
  ];
  const enter = sceneEnter(null, "kansas", first, []);
  const opened: SceneSession = { pid: "kansas", snapshot: enter.session.snapshot };
  // The user dragged the claimed window inside the scene.
  const moved = [
    win({ id: "a", claimed: true, frame: { x: 900, y: 30, w: 500, h: 700 } }),
    win({ id: "b" }),
  ];
  const next = sceneEnter(opened, "saturn", moved, []);
  assert.deepEqual(next.session.snapshot, opened.snapshot);
  assert.equal(next.session.snapshot[0].x, 10);
});

test("scene_leave_restores_snapshot_frames", () => {
  const before = [
    win({ id: "a", claimed: true, frame: { x: 10, y: 20, w: 300, h: 200 }, frontmost: true }),
    win({ id: "b", frame: { x: 500, y: 400, w: 600, h: 400 } }),
  ];
  const { session } = sceneEnter(null, "kansas", before, []);
  const open = { pid: "kansas", snapshot: session.snapshot };
  const inScene = [
    win({ id: "a", claimed: true, frame: { x: 900, y: 30, w: 500, h: 700 } }),
    win({ id: "b", minimized: true }),
  ];
  const { plan, save } = sceneLeave(open, inScene);
  assert.deepEqual(plan.restore, ["b"]);
  const byId = new Map(plan.frames.map((f) => [f.id, f]));
  assert.deepEqual(byId.get("a"), { id: "a", x: 10, y: 20, w: 300, h: 200 });
  assert.deepEqual(byId.get("b"), { id: "b", x: 500, y: 400, w: 600, h: 400 });
  // ... and the planet keeps the layout the user left inside the scene.
  assert.deepEqual(save, [{ stableKey: "a|Window|1", x: 900, y: 30, w: 500, h: 700 }]);
  // Focus returns to the window that had it before the scene.
  assert.deepEqual(plan.focus, ["a"]);
});

test("scene_leave_reminimizes_what_was_minimized_before", () => {
  // Leave restores minimized state, not just frames: a claimed window that
  // was in the Dock before the scene goes back into the Dock.
  const before = [
    win({ id: "claimed-min", claimed: true, minimized: true, frame: { x: 40, y: 60, w: 800, h: 500 } }),
    win({ id: "other" }),
  ];
  const { session } = sceneEnter(null, "kansas", before, [
    { stableKey: "claimed-min|Window|1", x: 10, y: 20, w: 300, h: 200 },
  ]);
  assert.equal(session.snapshot[0].minimized, true);
  // Inside the scene the window is up at its saved frame.
  const inScene = [
    win({ id: "claimed-min", claimed: true, frame: { x: 10, y: 20, w: 300, h: 200 } }),
    win({ id: "other", minimized: true }),
  ];
  const { plan } = sceneLeave({ pid: "kansas", snapshot: session.snapshot }, inScene);
  assert.deepEqual(plan.minimize, ["claimed-min"]);
  assert.deepEqual(plan.restore, ["other"]);
  const byId = new Map(plan.frames.map((f) => [f.id, f]));
  assert.deepEqual(byId.get("claimed-min"), { id: "claimed-min", x: 40, y: 60, w: 800, h: 500 });
  // A window that was up and stayed up is neither stowed nor restored.
  assert.equal(plan.minimize.includes("other"), false);
  assert.equal(plan.restore.includes("claimed-min"), false);
});

test("scene_leave_reminimize_needs_the_caller", () => {
  // plan.minimize is only worth anything if leaveScene applies it, and after
  // the frame writes so the Dock box is the pre-scene box.
  const body = src("src/lib/rail/pagePointer.svelte.ts");
  const at = body.indexOf("export async function leaveScene");
  assert.ok(at >= 0);
  const fn = body.slice(at, body.indexOf("\n}", at));
  assert.match(fn, /plan\.minimize/);
  const frameWrite = fn.indexOf('invoke("set_window_frame"');
  const stow = fn.indexOf('invoke("minimize_window"');
  assert.ok(frameWrite >= 0, "leave writes the pre-scene frames");
  assert.ok(stow > frameWrite, "the re-minimize comes after the frame writes");
});

test("scene_leave_skips_closed_window", () => {
  const before = [win({ id: "a" }), win({ id: "b" })];
  const { session } = sceneEnter(null, "kansas", before, []);
  const open = { pid: "kansas", snapshot: session.snapshot };
  // b was minimized and moved during the scene; a is gone (closed).
  const { plan } = sceneLeave(open, [
    win({ id: "b", minimized: true, frame: { x: 640, y: 80, w: 500, h: 400 } }),
  ]);
  assert.deepEqual(plan.restore, ["b"]);
  assert.deepEqual(
    plan.frames.map((f) => f.id),
    ["b"],
  );
  assert.equal(
    plan.restore.includes("a"),
    false,
    "a closed window is skipped, never a reason to abort",
  );
});

test("scene_new_window_left_alone", () => {
  const before = [win({ id: "a" })];
  const { session } = sceneEnter(null, "kansas", before, []);
  const open = { pid: "kansas", snapshot: session.snapshot };
  const later = win({ id: "new", minimized: true, frame: { x: 1200, y: 50, w: 300, h: 200 } });
  const { plan, save } = sceneLeave(open, [win({ id: "a" }), later]);
  assert.equal(plan.restore.includes("new"), false);
  assert.equal(
    plan.frames.some((f) => f.id === "new"),
    false,
  );
  assert.equal(
    save.some((s) => s.stableKey.startsWith("new")),
    false,
  );
});

test("scene_plan_table", () => {
  // One row per input shape; every column the rail reads.
  const rows: SceneWindowRow[] = [
    win({ id: "claimed", claimed: true }),
    win({ id: "claimed-min", claimed: true, minimized: true }),
    win({ id: "other" }),
    win({ id: "other-min", minimized: true }),
    win({ id: "far-space", onScreen: false }),
    win({ id: "noframe", frame: null }),
  ];
  const { plan } = sceneEnter(null, "kansas", rows, []);
  assert.deepEqual(plan.minimize, ["other"]);
  assert.deepEqual(plan.restore, ["claimed-min"]);
  assert.equal(plan.frames.length, 0);
  assert.deepEqual(plan.focus, ["claimed", "claimed-min"]);
  // Two pixels of drift is not a move.
  assert.equal(SCENE_TOLERANCE, 2);
  const near = sceneEnter(null, "kansas", [win({ id: "n", claimed: true, frame: { x: 102, y: 98, w: 400, h: 302 } })], [
    { stableKey: "n|Window|1", x: 100, y: 100, w: 400, h: 300 },
  ]);
  assert.deepEqual(near.plan.frames, []);
  const drifted = sceneEnter(null, "kansas", [win({ id: "n", claimed: true, frame: { x: 140, y: 100, w: 400, h: 300 } })], [
    { stableKey: "n|Window|1", x: 100, y: 100, w: 400, h: 300 },
  ]);
  assert.deepEqual(drifted.plan.frames, [{ id: "n", x: 100, y: 100, w: 400, h: 300 }]);
});

test("scene_tile_hidden_flag_off", () => {
  const { pid, cards, rows } = sample();
  const model = stagePreviewModel({ pid, cards, rows });
  assert.equal(
    model.slots.some((it) => (it.kind as string) === "scene"),
    false,
  );
  assert.equal(
    mirrorItems({ pid, cards, rows }).some((it) => (it.kind as string) === "scene"),
    false,
  );
  assert.deepEqual(
    mirrorItems({ pid, cards, rows }).map((it) => it.kind),
    ["habitat", "board", "window", "window"],
  );
});

test("scene_tile_after_habitat_flag_on", () => {
  const { pid, cards, rows } = sample();
  const items = mirrorItems({ pid, cards, rows, scene: { title: "kansas", active: false } });
  assert.deepEqual(
    items.map((it) => it.kind),
    ["habitat", "scene", "board", "window", "window"],
  );
  const tile = items[1];
  assert.equal(tile.stop, "rl2-scene");
  assert.equal(tile.title, "kansas");
  assert.equal(tile.active, false);
});

test("scene_tile_names_planet_and_state", () => {
  const { pid, cards, rows } = sample();
  const out = mirrorItems({ pid, cards, rows, scene: { title: "kansas", active: false } })[1];
  const inside = mirrorItems({ pid, cards, rows, scene: { title: "kansas", active: true } })[1];
  // F5: the tile carries the planet and whether the scene is in or out.
  assert.equal(out.title, "kansas");
  assert.equal(out.active, false);
  assert.equal(inside.active, true);
  const list = src("src/lib/rail/List.svelte");
  assert.match(list, /item\.kind === "scene"/);
  assert.match(list, /active \? "in" : "out"/);
});

test("no_tiler_names", () => {
  const banned = /yabai|aerospace|rectangle|amethyst/i;
  for (const rel of [
    "src/lib/rail/stagePlace.ts",
    "src/lib/rail/pagePointer.svelte.ts",
    "src/lib/rail/List.svelte",
    "src/lib/rail/habitat.ts",
    "src-tauri/src/windows/scene.rs",
    "src-tauri/src/windows/windows.rs",
    "src-tauri/src/windows/windows_mac.rs",
    "src-tauri/src/flags.rs",
    "src-tauri/src/state/mod.rs",
  ]) {
    assert.equal(banned.test(src(rel)), false, rel);
  }
});

test("planet_scenes_flag_defaults_off", () => {
  // The scene moves and minimizes real windows, so the row must ship off and
  // stay off the verify sweep until a hand turns it on.
  const flags = src("src-tauri/src/flags.rs");
  const at = flags.indexOf('id: "planet_scenes"');
  assert.ok(at >= 0);
  const row = flags.slice(at, flags.indexOf("},", at));
  assert.match(row, /default: false/);
  assert.match(row, /auto_on_verify: false/);
  assert.match(row, /remove_after: "2027-01-14"/);
});

test("planetScenes_optional_in_state", () => {
  // One optional state.json field, additive: an older save has no key.
  assert.match(src("src/lib/persist.ts"), /planetScenes\?:/);
  const state = src("src-tauri/src/state/mod.rs");
  assert.match(state, /pub planet_scenes: Option<HashMap<String, PlanetScene>>/);
  assert.match(state, /fn planet_scenes_optional_in_an_old_save/);
  // A rail save omits it; the merge must carry the disk value forward.
  assert.match(src("src-tauri/src/state/merge.rs"), /out\.planet_scenes\.is_none\(\)/);
  assert.match(
    src("src-tauri/src/state/merge.rs"),
    /out\.planet_scenes = disk\.planet_scenes\.clone\(\)/,
  );
});

// ---- FIX2: a scene toggle must not hand focus to another app ----------
// Entering or leaving a scene called restore_window, which is focus_window in
// windows_mac.rs: focusing a window activates its app, the rail blurred, and
// the auto-hide parked it under a second so it could not be clicked. The
// toggle now unminimizes without activating and holds auto-hide across the
// desk settle.

test("scene_toggle_never_activates_another_app", () => {
  const rel = "src/lib/rail/pagePointer.svelte.ts";
  for (const name of ["enterScene", "leaveScene"]) {
    const fn = fnText(rel, name);
    // The raise order is still known; only its timing changed.
    assert.match(fn, /plan\.focus/, `${name} still reads the raise order`);
    // Unminimize with the command that does not activate.
    assert.match(fn, /invoke\("deminiaturize_window"/, `${name} deminiaturizes`);
    // Neither command may run inside the toggle: both activate the window.
    assert.doesNotMatch(fn, /invoke\("restore_window"/, `${name} must not activate`);
    assert.doesNotMatch(fn, /invoke\("focus_window"/, `${name} must not activate`);
    // Held, not dropped.
    assert.match(fn, /sceneDeferFocus\(/, `${name} defers the raise`);
    // And the rail is held open across the activation the toggle used to cause.
    assert.match(fn, /armSceneHold\(/, `${name} holds auto-hide`);
  }
});

test("scene_deferred_raise_lands_when_the_pointer_leaves", () => {
  const rel = "src/lib/rail/pagePointer.svelte.ts";
  const leaveHook = fnText(rel, "onStageMouseLeave");
  assert.match(
    leaveHook,
    /sceneFlushFocus/,
    "the deferred raise flushes when the pointer leaves the rail",
  );
  // Sprite click and row raise keep restore_window + focus_window on purpose.
  const raise = fnText(rel, "raisePlanetWindows");
  assert.match(raise, /invoke\("restore_window"/);
  assert.match(raise, /invoke\("focus_window"/);
});

test("scene_deminiaturize_command_is_registered", () => {
  // The rail cannot call a function that is not a Tauri command: the Rust
  // fn exists in windows_mac.rs, the wrapper and the handler entry are what
  // make invoke("deminiaturize_window") resolve.
  const lib = src("src-tauri/src/lib.rs");
  assert.match(lib, /#\[tauri::command\]\s*\nfn deminiaturize_window\(id: String\)/);
  const handler = lib.slice(lib.indexOf("generate_handler!["));
  const list = handler.slice(0, handler.indexOf("])"));
  assert.match(list, /deminiaturize_window/);
  // The stub keeps the non-mac build compiling.
  assert.match(src("src-tauri/src/windows/windows_stub.rs"), /fn deminiaturize_window/);
});

test("scene_toggle_holds_auto_hide_for_three_seconds", () => {
  const hold = habitatScene as unknown as {
    SCENE_HOLD_MS?: number;
    armSceneHold?: (now?: number, ms?: number) => void;
    clearSceneHold?: () => void;
  };
  const holdMs = hold.SCENE_HOLD_MS ?? 0;
  assert.ok(holdMs >= 3000, "the dogfood check is three seconds");
  assert.equal(typeof hold.armSceneHold, "function", "the toggle arms a hold");
  assert.equal(typeof hold.clearSceneHold, "function");
  const armSceneHold = hold.armSceneHold!;
  const clearSceneHold = hold.clearSceneHold!;
  const outside = { railAutoHideOnBlur: true, focusInside: false, pointerInside: false };
  clearSceneHold();
  // Without a toggle the rule is today's: outside focus and pointer hides.
  assert.equal(autoHideWanted(outside), true);
  const t0 = 1_000_000;
  armSceneHold(t0);
  assert.equal(autoHideWanted({ ...outside, now: t0 + 1 }), false);
  assert.equal(autoHideWanted({ ...outside, now: t0 + 2999 }), false);
  // The grace ends; the existing rules take over unchanged.
  assert.equal(autoHideWanted({ ...outside, now: t0 + holdMs }), true);
  // The pref still wins inside the grace, and focus still wins after it.
  assert.equal(
    autoHideWanted({ ...outside, railAutoHideOnBlur: false, now: t0 + 1 }),
    false,
  );
  assert.equal(
    autoHideWanted({ ...outside, railAutoHideOnBlur: false, now: t0 + holdMs }),
    false,
  );
  assert.equal(
    autoHideWanted({ ...outside, focusInside: true, now: t0 + holdMs }),
    false,
  );
  clearSceneHold();
  assert.equal(autoHideWanted(outside), true);
});
