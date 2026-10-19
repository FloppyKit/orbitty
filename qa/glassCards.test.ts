import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { nudgePick } from "../src/lib/rail/nudgeTrigger.ts";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

test("glass Claim chip appears on a new minimize of an unclaimed window", () => {
  const win = { id: "w1", stableKey: "sk1", isMinimized: true };
  const none = new Set<string>();
  assert.equal(nudgePick([win], none, none, [])?.id, "w1");
  assert.equal(nudgePick([win], new Set(["w1"]), none, []), undefined);
  assert.equal(nudgePick([win], none, new Set(["w1"]), []), undefined);
  assert.equal(nudgePick([win], none, none, ["w1"]), undefined);
  assert.equal(nudgePick([win], none, none, ["sk1"]), undefined);
  assert.equal(nudgePick([{ ...win, isMinimized: false }], none, none, []), undefined);
  const drag = src("src/lib/rail/pageDrag.svelte.ts");
  const fn = drag.slice(
    drag.indexOf("export function checkNudge"),
    drag.indexOf("export async function openSettings"),
  );
  assert.match(fn, /if \(!scope\.nudgePrimed\)/);
  assert.match(fn, /nudgePick\(/);
  assert.equal(fn.includes("flatMap"), false);
});

test("glass Claim one nudge sits below the overlay strip", () => {
  const stage = src("src/lib/rail/rail-stage.css");
  assert.match(stage, /\.rail\.stage-shell \.nudge \{[\s\S]*?margin-top:\s*28px/);
  const list = src("src/lib/rail/List.svelte");
  assert.match(list, /Claim\s+one\?/);
  assert.equal(list.includes("data-tauri-drag-region"), false);
  assert.equal(stage.includes("data-tauri-drag-region"), false);
});

test("glass main window has no native title bar", () => {
  const conf = src("src-tauri/tauri.conf.json");
  assert.match(conf, /"decorations": false/);
  assert.equal(conf.includes("titleBarStyle"), false);
  const glass = src("src-tauri/src/windows/glass.rs");
  assert.equal(glass.includes("clear_titlebar_separator"), false);
  assert.equal(glass.includes("setTitlebarSeparatorStyle"), false);
  const chrome = src("src/lib/rail/rail-chrome.css").match(/\.chrome \{[^}]*\}/);
  assert.ok(chrome);
  assert.equal(chrome[0].includes("border-bottom"), false);
  const wtr = src("src/lib/WindowTitleRow.svelte").match(/\.wtr \{[^}]*\}/);
  assert.ok(wtr);
  assert.equal(wtr[0].includes("border-bottom"), false);
  assert.equal(src("src/lib/rail/rail-chrome.css").includes("data-tauri-drag-region"), false);
});
