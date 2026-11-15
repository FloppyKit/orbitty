// 55c: a small status dot above every painted agent droid whose seat door
// gives a word (`seat_outcomes`), in the same red / ochre / warm colors the
// old antenna used. No door word = no dot, so the flag-off look is today's.
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

function src(rel: string): string {
  return readFileSync(join(root, rel), "utf8");
}

function between(text: string, start: string, end: string): string {
  const at = text.indexOf(start);
  assert.ok(at >= 0, start);
  const stop = text.indexOf(end, at + start.length);
  assert.ok(stop > at, end);
  return text.slice(at, stop);
}

function walk(rel: string): string[] {
  const abs = join(root, rel);
  const out: string[] = [];
  for (const name of readdirSync(abs)) {
    const child = join(rel, name);
    if (statSync(join(root, child)).isDirectory()) out.push(...walk(child));
    else out.push(child);
  }
  return out;
}

function walkerDrawFields(): string[] {
  const struct = between(src("src-tauri/src/habitat/habitat_mac/draw_data.rs"), "struct WalkerDraw {", "\n}");
  return [...struct.matchAll(/pub\(crate\)\s+(\w+):/g)].map((m) => m[1]);
}

test("every_walkerdraw_field_is_read", () => {
  // At base `droid_glance` was carried on every WalkerDraw and never read:
  // the painted kit ignored the color 55b computed. The dot reads it.
  const world = src("src-tauri/src/habitat/habitat_mac/draw_world.rs");
  const fields = walkerDrawFields();
  assert.ok(fields.includes("droid_glance"), "droid_glance field");
  assert.ok(fields.includes("painted_droid"), "painted_droid field");
  assert.ok(fields.includes("door_word"), "door_word field");
  for (const field of fields) {
    assert.ok(world.includes(`w.${field}`), `draw_world never reads w.${field}`);
  }
});

test("draw_world_calls_status_pip", () => {
  const world = src("src-tauri/src/habitat/habitat_mac/draw_world.rs");
  const loop = between(world, "for w in &d.walkers {", "if let Some((bx, by)) = d.ball");
  const blit = loop.indexOf("draw_nsimage(img, art");
  const pip = loop.indexOf("status_pip(");
  const badge = loop.indexOf("if w.icon != 0");
  assert.ok(blit >= 0, "sprite blit");
  assert.ok(badge > blit, "badge after the blit");
  assert.ok(pip > blit, "status_pip draws after the sprite");
  assert.ok(pip < badge, "status_pip draws before the badge");
  const call = loop.slice(pip, loop.indexOf(")", loop.indexOf("w.door_word", pip)) + 1);
  assert.match(call, /w\.painted_droid/);
  assert.match(call, /w\.droid_glance/);
  assert.match(call, /w\.door_word/);
});

test("draw_sets_painted_droid_from_kit_frame", () => {
  const draw = src("src-tauri/src/habitat/habitat_mac/draw.rs");
  const literal = between(draw, "Some(WalkerDraw {", "})");
  // A painted kit frame drew it: `b.droid.is_some()` alone would put a dot
  // on the fallback droid, which already shows its own baked antenna.
  assert.match(draw, /painted_droid: b\.droid\.is_some\(\) && painted != 0/);
  assert.match(draw, /door_word: b\.door_word/);
  // The hoisted local still comes from the kit frame pointer, and the
  // painted branch still wins over the baked fallback.
  const hoist = between(draw, "let painted = b", "Some(WalkerDraw {");
  assert.match(hoist, /droid_pack::frame_ptr\(/);
  assert.match(literal, /if painted != 0 \{\s*painted\s*\}/);
  assert.ok(draw.indexOf("let painted = b") < draw.indexOf("painted_droid:"));
});

test("door_word_reaches_being", () => {
  const layout = src("src-tauri/src/habitat/habitat_mac/layout.rs");
  const make = between(layout, "pub(crate) fn make_being", "\n}");
  assert.match(make, /door_word: door_word\(entry\)/);

  const sync = src("src-tauri/src/habitat/habitat_mac/sync.rs");
  const fast = between(sync, "// Pose-only fast path", "Some((mut walkers, dots))");
  assert.match(fast, /let word = door_word\(e\)/);
  const set = fast.indexOf("b.door_word = word;");
  assert.ok(set > 0, "fast path stores the word");
  // A word that appears or vanishes with an unchanged glance must still
  // redraw, or the dot goes stale until the next pose change.
  assert.match(fast.slice(set, set + 240), /any_pose = true/);
});

// The walker-draw files 55c touches. Regression guard: green at base too.
const DRAW_FILES = [
  "src-tauri/src/habitat/habitat_mac/pose.rs",
  "src-tauri/src/habitat/habitat_mac/pose_tests.rs",
  "src-tauri/src/habitat/habitat_mac/types.rs",
  "src-tauri/src/habitat/habitat_mac/layout.rs",
  "src-tauri/src/habitat/habitat_mac/sync.rs",
  "src-tauri/src/habitat/habitat_mac/todo_tree.rs",
  "src-tauri/src/habitat/habitat_mac/draw.rs",
  "src-tauri/src/habitat/habitat_mac/draw_data.rs",
  "src-tauri/src/habitat/habitat_mac/draw_world.rs",
];

test("walker_files_never_call_the_door", () => {
  const banned = ["outcome::all", "seat_outcomes", "seatOutcomes", "peek::view", "herdr::"];
  for (const rel of DRAW_FILES) {
    const text = src(rel);
    for (const needle of banned) {
      assert.equal(text.includes(needle), false, `${rel} names ${needle}`);
    }
  }
});

test("habitat_still_never_calls_door", () => {
  // 55b's guard, still true for the new dot code: the habitat reads the
  // rail snapshot's word and never the door or the peek itself.
  for (const rel of walk("src-tauri/src/habitat/habitat_mac")) {
    const text = src(rel);
    for (const needle of ["outcome::all", "peek::view", "seat_outcomes", "seatOutcomes"]) {
      assert.equal(text.includes(needle), false, `${rel} names ${needle}`);
    }
  }
});

test("no_home_path_literal_in_the_new_files", () => {
  const home = "/User" + "s/";
  for (const rel of ["qa/walkerStatusDot.test.ts"]) {
    assert.equal(src(rel).includes(home), false, rel);
  }
});
