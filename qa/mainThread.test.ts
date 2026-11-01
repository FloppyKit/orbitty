// R4: Orbitty's main agent has one thread. Source scan only.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

/** Brace body starting at `{`, skipping comments and strings. */
function braceBody(file: string, open: number): string {
  let i = open;
  let depth = 0;
  while (i < file.length) {
    const c = file[i];
    const n = file[i + 1];
    if (c === "/" && n === "/") {
      const nl = file.indexOf("\n", i);
      i = nl < 0 ? file.length : nl + 1;
      continue;
    }
    if (c === "/" && n === "*") {
      const end = file.indexOf("*/", i + 2);
      i = end < 0 ? file.length : end + 2;
      continue;
    }
    if (c === '"') {
      i++;
      while (i < file.length && file[i] !== '"') {
        if (file[i] === "\\") i += 2;
        else i++;
      }
      i++;
      continue;
    }
    if (c === "{") depth++;
    else if (c === "}") {
      depth--;
      if (depth === 0) return file.slice(open, i + 1);
    }
    i++;
  }
  throw new Error("unbalanced brace");
}

/** Brace body of `fn name`, skipping comments and strings. */
function fnBody(file: string, name: string): string {
  const key = `fn ${name}`;
  const at = file.indexOf(key);
  assert.ok(at >= 0, key);
  const open = file.indexOf("{", at);
  assert.ok(open > at, key);
  return braceBody(file, open);
}

function tsFn(file: string, name: string): string {
  const key = `function ${name}`;
  const at = file.indexOf(key);
  assert.ok(at >= 0, key);
  const open = file.indexOf("{", at);
  assert.ok(open > at, key);
  return braceBody(file, open);
}

test("main_thread_single", () => {
  const model = src("src-tauri/src/state/models_companion.rs");
  const heal = fnBody(model, "heal_modes");
  const push = fnBody(model, "push_turn");
  const switched = fnBody(model, "switch_mode");

  assert.match(heal, /if self\.turns\.is_empty\(\)/);
  const assign = heal.indexOf("self.turns = t.clone()");
  const empty = heal.indexOf("if self.turns.is_empty()");
  assert.ok(empty >= 0 && assign > empty, "turns assignment is the empty-thread heal");
  assert.equal(heal.indexOf("self.turns =", assign + 1), -1);

  const outside = model.replace(heal, "");
  assert.doesNotMatch(outside, /\.turns\s*=/, "turns assignment outside heal_modes");
  assert.doesNotMatch(model, /turns\.clear\(\)/);
  assert.doesNotMatch(switched, /\.turns\s*=/);
  assert.doesNotMatch(switched, /turns\.clear\(\)/);
  assert.match(switched, /self\.mode = new_mode\.to_string\(\)/);
  assert.match(model, /Append one turn to the single companion thread/);
  assert.match(model, /Same `turns`/);

  assert.match(push, /self\.turns\.push\(/);
  assert.match(push, /push_archived/);
  const withoutPush = model.replace(push, "");
  assert.doesNotMatch(withoutPush, /\.turns\.push\(/);
  assert.equal(model.split("push_archived").length - 1, 1);

  const roll = src("src-tauri/src/companion/mouth_roll.rs");
  assert.match(roll, /Never wipe turns/);
  assert.doesNotMatch(roll, /turns\.clear\(\)/);
  assert.doesNotMatch(roll, /\.turns\s*=/);
  assert.doesNotMatch(roll, /\.turns\.push\(/);

  const setMode = fnBody(src("src-tauri/src/coordinator/commands.rs"), "set_companion_mode");
  assert.match(setMode, /switch_mode\(/);
  assert.doesNotMatch(setMode, /turns\.clear\(\)/);
  assert.doesNotMatch(setMode, /\.turns\s*=/);
});

test("main_thread_ui_never_resets", () => {
  const page = src("src/lib/CompanionPage.svelte");
  assert.doesNotMatch(page, /turns\s*=\s*\[\]/);
  const expand = tsFn(src("src/lib/companion/pageExpand.svelte.ts"), "toggleExpanded");
  assert.doesNotMatch(expand, /turns\s*=\s*\[\]/);
  assert.doesNotMatch(expand, /\.turns\s*=/);
  const pick = tsFn(src("src/lib/CompanionDock.svelte"), "pickAgent");
  assert.doesNotMatch(pick, /turns/);
  assert.match(page, /expandKey\(/);
  const handler = tsFn(page, "onPaneKey");
  assert.doesNotMatch(handler, /turns\s*=\s*\[\]/);
});
