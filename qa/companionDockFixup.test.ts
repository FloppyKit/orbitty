import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { launcherItems, pickIndex, refitComposer, type LaunchCtx } from "../src/lib/companion/launcher.ts";

function ctx(over: Partial<LaunchCtx> = {}): LaunchCtx {
  return {
    query: "",
    armed: null,
    planets: [
      { id: "p-kansas", name: "Kansas" },
      { id: "p-orbit", name: "Orbit" },
    ],
    hosted: "off",
    hostBusy: false,
    agent: "spark",
    agents: [
      { id: "spark", label: "Spark" },
      { id: "grok", label: "Work" },
    ],
    mode: "spark",
    speak: false,
    sprite: true,
    captions: true,
    head: false,
    prompts: false,
    nextCount: 0,
    smallestTodo: null,
    planetsHidden: false,
    ...over,
  };
}

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

test("send_button_runs_selected_row", () => {
  const mouth = src("src/lib/companion/Mouth.svelte");
  const form = mouth.slice(mouth.indexOf("<form"), mouth.indexOf("{#if attach}"));
  assert.match(form, /variant === "line"/);
  assert.match(form, /sendLine\(\)/);
  assert.match(form, /onSubmit\?\.\(\)/);
  assert.match(mouth, /type="submit"/);
  const dock = src("src/lib/CompanionDock.svelte");
  assert.match(dock, /onSubmit=\{\(\) => void runIndex\(selected, byArrow\)\}/);
  const expanded = dock.slice(dock.indexOf("function onFieldKeydown"), dock.indexOf("const MAX_EXTRA_LINES"));
  assert.match(expanded, /if \(expanded\) \{[\s\S]*sendLine\(\)/);
});

test("send_button_ask_row_sends", () => {
  const query = "what is on the board today";
  const items = launcherItems(ctx({ query }));
  assert.equal(pickIndex(query, items), 0);
  assert.equal(items[0]?.kind, "ask");
  assert.equal(items[0]?.run.t, "send");
  const page = src("src/lib/CompanionPage.svelte");
  assert.match(page, /action\.t === "send" \|\| action\.t === "answer"/);
  assert.match(page, /await sendLine\(\)/);
  const needed = src("src/lib/companion/pageNeeded.svelte.ts");
  assert.match(needed, /needed_answer/);
});

test("refit_after_companion_off", () => {
  const shared = {
    needed: 220,
    oneLineH: 36,
    lineH: 18,
    expanded: false,
    hostedOff: true,
  };
  const off = refitComposer({ ...shared, sprite: false });
  const on = refitComposer({ ...shared, sprite: true });
  assert.equal(off.height, 220);
  assert.equal(off.scroll, false);
  assert.equal(off.composerExtra, 184);
  assert.deepEqual(off, on);

  const capped = refitComposer({
    needed: 400,
    oneLineH: 36,
    lineH: 18,
    expanded: true,
    hostedOff: true,
    sprite: false,
  });
  assert.equal(capped.scroll, true);
  assert.equal(capped.height, 36 + 5 * 18);
  assert.equal(capped.composerExtra, 0);

  const dock = src("src/lib/CompanionDock.svelte");
  const grow = dock.slice(dock.indexOf("function growField"), dock.indexOf("let dockDragging"));
  assert.match(grow, /refitComposer\(/);
  assert.match(grow, /sprite/);
  assert.match(grow, /0px/);
  assert.match(grow, /\$effect\(\(\) => \{[\s\S]*\bsprite\b[\s\S]*growField\(\)/);
  assert.equal(src("src/lib/CompanionPage.svelte").includes("function reportHug"), true);
});
