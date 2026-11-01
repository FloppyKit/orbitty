import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import {
  MAX_ROWS,
  catalogue,
  clearAfterRun,
  launcherItems,
  launcherKey,
  noteRecent,
  pickIndex,
  rankItems,
  type LaunchCtx,
  type LaunchRow,
} from "../src/lib/companion/launcher.ts";

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

function titles(rows: LaunchRow[]): string[] {
  return rows.map((row) => row.title);
}

function registeredCommands(): Set<string> {
  const lib = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
  const start = lib.indexOf("generate_handler!");
  assert.ok(start >= 0, "generate_handler");
  const end = lib.indexOf("]))", start);
  assert.ok(end > start);
  const names = new Set<string>();
  for (const raw of lib.slice(start, end).split("\n")) {
    const line = raw.replace(/\/\/.*$/, "").trim().replace(/,$/, "");
    if (!line || line.includes("generate_handler")) continue;
    const name = line.split("::").pop()?.trim() ?? "";
    if (/^[A-Za-z0-9_]+$/.test(name)) names.add(name);
  }
  return names;
}

test("D3 invoke rows are registered commands and planets are one row each", () => {
  const cmds = registeredCommands();
  assert.ok(cmds.has("dest_show"));
  assert.equal(cmds.has("dest_open"), false);
  const rows = catalogue(ctx());
  const invokes = rows.filter((row) => row.run.t === "invoke");
  assert.ok(invokes.length > 0);
  for (const row of invokes) {
    assert.equal(row.run.t, "invoke");
    assert.ok(cmds.has(row.run.cmd), row.run.cmd);
    assert.notEqual(row.run.cmd, "list_windows");
    assert.notEqual(row.run.cmd, "focus_window");
    assert.notEqual(row.run.cmd, "dest_open");
  }
  assert.equal(rows.filter((row) => row.id.startsWith("goto-")).length, 2);
  assert.equal(rows.filter((row) => row.id.startsWith("locate-")).length, 2);
  assert.equal(rows.filter((row) => row.id.startsWith("claimto-")).length, 2);
  assert.equal(rows.some((row) => row.id === "open-smallest"), false);
  const withTodo = catalogue(
    ctx({ smallestTodo: { cardId: "c1", text: "buy milk" } }),
  );
  assert.equal(withTodo.filter((row) => row.id === "open-smallest").length, 1);
});

test("D3 host rows drop while a host transition runs", () => {
  const idle = catalogue(ctx());
  const busy = catalogue(ctx({ hostBusy: true }));
  assert.equal(idle.filter((row) => row.run.t === "host").length, 3);
  assert.equal(busy.filter((row) => row.run.t === "host").length, 0);
});

test("D3 labels stay plain", () => {
  const banned = /\b(rail|dest|host)\b/i;
  for (const row of catalogue(ctx({ smallestTodo: { cardId: "c1", text: "buy milk" } }))) {
    assert.doesNotMatch(row.title, banned, row.title);
    if (row.subtitle) assert.doesNotMatch(row.subtitle, banned, row.subtitle);
  }
});

test("D4 rankItems boa, st, slash cl, empty suggestions", () => {
  const rows = catalogue(ctx());
  const boa = rankItems("boa", rows);
  assert.equal(boa[0]?.title, "Open Board");
  const again = rankItems("boa", rows);
  assert.deepEqual(titles(boa), titles(again));

  const st = rankItems("st", rows);
  const starsAt = st
    .map((row, index) => (/stars/i.test(row.title) ? index : -1))
    .filter((index) => index >= 0);
  const settingsAt = st.findIndex((row) => row.title === "Settings");
  assert.ok(starsAt.length >= 3, titles(st).join(" | "));
  assert.ok(settingsAt > Math.max(...starsAt), titles(st).join(" | "));

  const slash = rankItems("/cl", rows);
  assert.ok(slash.length > 0);
  assert.ok(slash.every((row) => /claim/i.test(row.title)), titles(slash).join(" | "));
  assert.equal(slash.some((row) => row.kind === "ask"), false);

  const empty = rankItems("", rows);
  assert.deepEqual(titles(empty), [
    "Open Board",
    "Open Stars",
    "Planets",
    "Claim front window",
  ]);
});

test("D4 slash st is stars rows only", () => {
  const rows = catalogue(ctx());
  const slash = rankItems("/st", rows);
  assert.ok(slash.length > 0);
  assert.ok(slash.every((row) => /stars/i.test(row.title)), titles(slash).join(" | "));
  assert.equal(slash.some((row) => row.title === "Settings"), false);
});

test("D4 suggestions put the smallest todo and a recent row first", () => {
  const rows = catalogue(ctx({ smallestTodo: { cardId: "c1", text: "buy milk" } }));
  assert.equal(rankItems("", rows)[0]?.title, "Open smallest todo");
  const recent = noteRecent([], "open-stars");
  assert.equal(rankItems("", rows, recent)[0]?.id, "open-stars");
  assert.equal(noteRecent(recent, "ask")[0], "open-stars");
});

test("D5 ask and answer stay row 1 and slash hides ask", () => {
  const ask = launcherItems(ctx({ query: "hello there" }));
  assert.equal(ask[0]?.kind, "ask");
  assert.equal(ask[0]?.run.t, "send");
  assert.match(ask[0]?.title ?? "", /Ask Orbitty: hello there/);
  const boa = launcherItems(ctx({ query: "boa" }));
  assert.equal(boa[0]?.kind, "ask");
  assert.equal(boa[1]?.title, "Open Board");

  const armed = launcherItems(
    ctx({ query: "the answer", armed: { id: "n1", title: "TEST 27p-a ask" } }),
  );
  assert.equal(armed[0]?.kind, "answer");
  assert.equal(armed[0]?.title, "Answer: TEST 27p-a ask");
  assert.deepEqual(armed[0]?.run, { t: "answer", id: "n1" });
  assert.equal(armed.some((row) => row.kind === "ask"), false);

  const slash = launcherItems(ctx({ query: "/cl" }));
  assert.equal(slash.some((row) => row.kind === "ask"), false);
  assert.ok(slash.every((row) => /claim/i.test(row.title)));

  const idle = launcherItems(
    ctx({ armed: { id: "n1", title: "Bring milk" } }),
  );
  assert.equal(idle[0]?.kind, "answer");
  assert.equal(idle[1]?.title, "Open Board");
});

test("D5 list caps at 50", () => {
  const planets = Array.from({ length: 40 }, (_, i) => ({
    id: `p${i}`,
    name: `Alpha ${i}`,
  }));
  const items = launcherItems(ctx({ query: "a", planets }));
  assert.equal(items.length, MAX_ROWS);
  assert.equal(items[0]?.kind, "ask");
});

test("D6 launcherKey wrap, shift enter, composing, digits, esc", () => {
  const base = {
    query: "boa",
    selected: 0,
    count: 8,
    visibleStart: 0,
    stash: null,
  };
  const down = launcherKey(base, { key: "ArrowDown" }, []);
  assert.equal(down.state.selected, 1);
  assert.equal(down.effect, "none");
  const up = launcherKey(base, { key: "ArrowUp" }, []);
  assert.equal(up.state.selected, 7);
  assert.equal(up.state.visibleStart, 3);
  const enter = launcherKey(base, { key: "Enter" }, []);
  assert.equal(enter.effect, "run");
  assert.equal(enter.runIndex, 0);
  const shift = launcherKey(base, { key: "Enter", shift: true }, []);
  assert.equal(shift.effect, "newline");
  assert.equal(shift.runIndex, undefined);
  const composing = launcherKey(base, { key: "Enter", composing: true }, []);
  assert.equal(composing.effect, "ignore");
  const scrolled = launcherKey(
    { ...base, selected: 4, visibleStart: 3 },
    { key: "1", meta: true },
    [],
  );
  assert.equal(scrolled.effect, "run");
  assert.equal(scrolled.runIndex, 3);
  const fifth = launcherKey(
    { ...base, visibleStart: 3 },
    { key: "5", meta: true },
    [],
  );
  assert.equal(fifth.effect, "run");
  assert.equal(fifth.runIndex, 7);
  const missing = launcherKey(
    { ...base, count: 3, visibleStart: 0 },
    { key: "5", meta: true },
    [],
  );
  assert.equal(missing.effect, "ignore");

  const moved = launcherKey({ ...base, selected: 3 }, { key: "Escape" }, []);
  assert.equal(moved.effect, "none");
  assert.equal(moved.state.selected, 0);
  assert.equal(moved.state.query, "boa");
  const cleared = launcherKey(moved.state, { key: "Escape" }, []);
  assert.equal(cleared.state.query, "");
  assert.equal(cleared.state.stash, "boa");
  assert.equal(cleared.effect, "none");
  const blurred = launcherKey(cleared.state, { key: "Escape" }, []);
  assert.equal(blurred.effect, "blur");
  const restored = launcherKey(cleared.state, { key: "z", meta: true }, []);
  assert.equal(restored.state.query, "boa");
  assert.equal(restored.state.stash, null);
});

test("D6 tab completes the selected title", () => {
  const titlesFor = ["Ask Orbitty: ka", "Go to Kansas"];
  const done = launcherKey(
    { query: "ka", selected: 1, count: 2, visibleStart: 0, stash: null },
    { key: "Tab" },
    titlesFor,
  );
  assert.equal(done.effect, "complete");
  assert.equal(done.complete, "Go to Kansas");
  assert.equal(done.state.query, "Go to Kansas");
});

test("D9 clearAfterRun keeps a chat draft chosen by arrows", () => {
  assert.equal(
    clearAfterRun({
      kind: "invoke",
      query: "boa",
      title: "Open Board",
      byArrow: true,
    }),
    true,
  );
  assert.equal(
    clearAfterRun({
      kind: "invoke",
      query: "please summarize this for me",
      title: "Open Board",
      byArrow: true,
    }),
    false,
  );
  assert.equal(
    clearAfterRun({
      kind: "invoke",
      query: "/cl",
      title: "Claim front window",
      byArrow: true,
    }),
    true,
  );
  assert.equal(
    clearAfterRun({ kind: "send", query: "hello", title: "Ask Orbitty: hello", byArrow: false }),
    true,
  );
  assert.equal(
    clearAfterRun({
      kind: "answer",
      query: "yes",
      title: "Answer: ask",
      byArrow: false,
    }),
    true,
  );
});

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

test("D1 docked order is title, chat, then the dock", () => {
  const page = src("src/lib/CompanionPage.svelte");
  const titleAt = page.indexOf('mode="title"');
  const chatAt = page.indexOf('class="dockchat"');
  const dockAt = page.indexOf("<CompanionDock");
  assert.ok(titleAt > 0 && titleAt < chatAt && chatAt < dockAt);
  const spiritInChat = page.indexOf("<CompSpirit", chatAt);
  assert.ok(spiritInChat > chatAt && spiritInChat < dockAt);
  assert.match(page, /\{#if expanded && sprite\}[\s\S]*\bcorner\b/);
  assert.equal(page.includes('sprite && !expanded && (hosted === "off" || spiritHold)'), false);
  const row = src("src/lib/WindowTitleRow.svelte");
  assert.match(row, /"mac" \| "custom" \| "title"/);
  assert.match(row, /\{#if mode === "mac"\}[\s\S]*class="wtr-lights"/);
  assert.match(row, /\{#if mode === "custom"\}[\s\S]*class="winx"/);
  assert.match(row, /if \(mode === "title"\) return/);
});

test("D2 dests are gone and toggleDest stays on the chord", () => {
  const dock = src("src/lib/CompanionDock.svelte");
  const page = src("src/lib/CompanionPage.svelte");
  assert.equal(dock.includes("Dests"), false);
  assert.equal(dock.includes("showDests"), false);
  assert.equal(existsSync(new URL("../src/lib/companion/Dests.svelte", import.meta.url)), false);
  assert.match(page, /function toggleDest/);
  assert.match(page, /orbitty:toggle-host/);
});

test("D5 reducer runs sendLine for the ask row", () => {
  const dock = src("src/lib/CompanionDock.svelte");
  const page = src("src/lib/CompanionPage.svelte");
  assert.match(dock, /launcherKey\(/);
  assert.match(page, /action\.t === "send"/);
  assert.match(page, /await sendLine\(\)/);
});

test("D7 combobox, listbox, and option roles", () => {
  const mouth = src("src/lib/companion/Mouth.svelte");
  const launcher = src("src/lib/companion/Launcher.svelte");
  assert.match(mouth, /role="combobox"/);
  assert.match(mouth, /aria-activedescendant/);
  assert.match(launcher, /role="listbox"/);
  assert.match(launcher, /role="option"/);
  assert.match(launcher, /aria-selected/);
});

test("D8 arming keeps the draft and still answers", () => {
  const needed = src("src/lib/companion/pageNeeded.svelte.ts");
  const focus = needed.slice(
    needed.indexOf("export function onNeededFocus"),
    needed.indexOf("export function composerField"),
  );
  assert.doesNotMatch(focus, /draft\s*=/);
  assert.match(needed, /needed_answer/);
  const page = src("src/lib/CompanionPage.svelte");
  assert.match(page, /orbitty:needed-focus/);
  assert.match(page, /onNeededFocus\(/);
});

test("D10 list height is reserved inside the measured dock", () => {
  const launcher = src("src/lib/companion/Launcher.svelte");
  const style = launcher.slice(launcher.indexOf("<style>"), launcher.indexOf("</style>"));
  assert.match(style, /\.launcher\s*\{[^}]*height:\s*150px/);
  assert.doesNotMatch(style, /\.launcher\s*\{[^}]*height:\s*auto/);
  const dock = src("src/lib/CompanionDock.svelte");
  assert.ok(dock.indexOf('class="ghostdock"') < dock.indexOf("<DockLauncher"));
  assert.match(src("src/lib/CompanionPage.svelte"), /class="docktitle starstrip"/);
  const hug = src("src/lib/companion/pageHug.svelte.ts");
  assert.match(hug, /querySelector\("\.ghostdock"\)/);
  assert.match(hug, /\.starstrip/);
});

test("D11 launcher styles use theme vars only", () => {
  const launcher = src("src/lib/companion/Launcher.svelte");
  const style = launcher.slice(launcher.indexOf("<style>"), launcher.indexOf("</style>"));
  assert.equal(/#[0-9a-fA-F]{3,8}/.test(style), false, style);
  assert.equal(/rgb\(/.test(style), false, style);
  assert.match(style, /var\(--accent\)/);
  assert.match(style, /var\(--focus-ring\)/);
});

test("one_word_prefix_preselects_command", () => {
  const items = launcherItems(ctx({ query: "boa" }));
  const picked = pickIndex("boa", items);
  assert.equal(items[0]?.kind, "ask");
  assert.equal(picked, 1);
  assert.equal(items[picked]?.title, "Open Board");
  assert.equal(items[picked]?.kind, "command");
  const up = launcherKey(
    { query: "boa", selected: picked, count: items.length, visibleStart: 0, stash: null },
    { key: "ArrowUp" },
    items.map((row) => row.title),
  );
  assert.equal(up.state.selected, 0);
  assert.equal(up.state.query, "boa");

  const armed = launcherItems(
    ctx({ query: "boa", armed: { id: "n1", title: "Bring milk" } }),
  );
  assert.equal(armed[0]?.kind, "answer");
  assert.equal(pickIndex("boa", armed), 0);

  assert.equal(pickIndex("yes", launcherItems(ctx({ query: "yes" }))), 0);
  assert.equal(pickIndex("ok", launcherItems(ctx({ query: "ok" }))), 0);
  assert.equal(pickIndex("bd", launcherItems(ctx({ query: "bd" }))), 0);
});

test("two_words_keep_ask_first", () => {
  for (const query of ["what is on the board today", "open board", "?boa", "? what"]) {
    const items = launcherItems(ctx({ query }));
    assert.equal(items[0]?.kind, "ask", query);
    assert.equal(pickIndex(query, items), 0, query);
  }
});

test("esc_with_selection_only_collapses", () => {
  const state = {
    query: "hello boa",
    selected: 2,
    count: 6,
    visibleStart: 1,
    stash: "kept",
  };
  const result = launcherKey(state, { key: "Escape", hasSelection: true }, []);
  assert.equal(result.effect, "collapse-selection");
  assert.equal(result.state.query, "hello boa");
  assert.equal(result.state.selected, 2);
  assert.equal(result.state.visibleStart, 1);
  assert.equal(result.state.stash, "kept");
  assert.equal(result.runIndex, undefined);
});

test("esc_ladder_unchanged_without_selection", () => {
  const base = {
    query: "boa",
    selected: 0,
    count: 8,
    visibleStart: 0,
    stash: null,
  };
  const moved = launcherKey(
    { ...base, selected: 3 },
    { key: "Escape", hasSelection: false },
    [],
  );
  assert.equal(moved.effect, "none");
  assert.equal(moved.state.selected, 0);
  assert.equal(moved.state.query, "boa");
  assert.equal(moved.state.stash, null);
  const cleared = launcherKey(moved.state, { key: "Escape" }, []);
  assert.equal(cleared.state.query, "");
  assert.equal(cleared.state.stash, "boa");
  assert.equal(cleared.effect, "none");
  const blurred = launcherKey(cleared.state, { key: "Escape" }, []);
  assert.equal(blurred.effect, "blur");
  const restored = launcherKey(cleared.state, { key: "z", meta: true }, []);
  assert.equal(restored.state.query, "boa");
  assert.equal(restored.state.stash, null);
});

test("D12 global shortcuts stay byte-identical", () => {
  const lib = src("src-tauri/src/lib.rs");
  assert.match(lib, /Shortcut::new\(Some\(Modifiers::SUPER \| Modifiers::SHIFT\), Code::KeyO\)/);
  assert.match(lib, /Shortcut::new\(Some\(Modifiers::SUPER \| Modifiers::SHIFT\), Code::KeyY\)/);
  assert.match(lib, /Shortcut::new\(Some\(Modifiers::SUPER \| Modifiers::SHIFT\), Code::KeyN\)/);
});
