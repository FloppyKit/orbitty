// 27p-c part a: fuzzy match tiers, frequency ranking, ghost completion,
// Cmd-1..9. Pure launcher behaviour + source scans for the paint.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  TIER,
  catalogue,
  ghostSuffix,
  launcherItems,
  launcherKey,
  noteUse,
  pickIndex,
  rankItems,
  type LaunchCtx,
  type LaunchRow,
  type Usage,
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

function row(id: string, title: string): LaunchRow {
  return {
    id,
    title,
    glyph: "·",
    kind: "command",
    run: { t: "invoke", cmd: "dest_show", args: { dest: "launchpad" } },
  };
}

function titles(rows: LaunchRow[]): string[] {
  return rows.map((r) => r.title);
}

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

function used(entries: [string, number][]): Usage {
  let usage: Usage = {};
  let clock = 1000;
  for (const [id, count] of entries) {
    for (let i = 0; i < count; i += 1) {
      clock += 1;
      usage = noteUse(usage, id, clock);
    }
  }
  return usage;
}

test("tiers run exact, word-start, initials, subsequence, typo", () => {
  assert.deepEqual(
    [
      TIER.exact,
      TIER.wordStart,
      TIER.initials,
      TIER.subsequence,
      TIER.typo,
    ],
    [0, 1, 2, 3, 4],
  );
});

test("initials beat a plain subsequence hit", () => {
  // "ss" is the word initials of System Settings; the other row only
  // carries s…s as a subsequence.
  const rows = [row("boss", "Boss says no"), row("sys", "System Settings")];
  const ranked = rankItems("ss", rows);
  assert.equal(ranked[0]?.title, "System Settings", titles(ranked).join(" | "));
  assert.equal(ranked[1]?.title, "Boss says no");
});

test("initials reach the real catalogue (ob is Open Board)", () => {
  const rows = catalogue(ctx());
  assert.equal(rankItems("ob", rows)[0]?.title, "Open Board");
  // Initials never preselect: Return keeps its base meaning for a word.
  assert.equal(pickIndex("ob", launcherItems(ctx({ query: "ob" }))), 0);
});

test("subsequence still finds Open Board (opbrd)", () => {
  const ranked = rankItems("opbrd", catalogue(ctx()));
  assert.equal(ranked[0]?.title, "Open Board", titles(ranked).join(" | "));
});

test("one typo matches a 4+ letter query (swap and drop)", () => {
  const rows = catalogue(ctx());
  for (const query of ["baord", "boadr", "bord"]) {
    assert.ok(
      titles(rankItems(query, rows)).includes("Open Board"),
      `${query}: ${titles(rankItems(query, rows)).join(" | ")}`,
    );
  }
  // Swapped pair in the middle is the same one-typo rule.
  const synthetic = [row("quartz", "Quartz Live")];
  assert.equal(rankItems("qurtaz", synthetic).length, 0, "swap twice is two typos");
  assert.ok(titles(rankItems("quratz", synthetic)).includes("Quartz Live"));
});

test("no typo tier under four letters", () => {
  const rows = [row("map", "Map View")];
  assert.equal(rankItems("mop", rows).length, 0, "3 letters never typo-match");
  assert.equal(rankItems("mopx", rows).length, 0, "no match, no crash");
});

test("typo ranks below a subsequence hit", () => {
  const typoRow = row("board", "Open Board");
  const subseqRow = row("boats", "Boats and rafts");
  const ranked = rankItems("boadr", [typoRow, subseqRow]);
  assert.deepEqual(titles(ranked), ["Boats and rafts", "Open Board"]);
});

test("three runs lift an equal-tier row, ties keep catalogue order", () => {
  const rows = [row("one", "Stars one"), row("two", "Stars two")];
  assert.deepEqual(titles(rankItems("st", rows)), ["Stars one", "Stars two"]);
  const usage = used([["two", 3]]);
  assert.deepEqual(titles(rankItems("st", rows, usage)), ["Stars two", "Stars one"]);
  const both = used([
    ["one", 3],
    ["two", 1],
  ]);
  assert.deepEqual(titles(rankItems("st", rows, both)), ["Stars one", "Stars two"]);
});

test("usage never lifts a lower tier above an exact hit", () => {
  const rows = [row("exact", "Stars"), row("sub", "Sitar strings")];
  const usage = used([["sub", 9]]);
  assert.deepEqual(titles(rankItems("stars", rows, usage)), ["Stars", "Sitar strings"]);
});

test("base hits unchanged: boa, slash st, slash cl", () => {
  const rows = catalogue(ctx());
  assert.equal(rankItems("boa", rows)[0]?.title, "Open Board");
  const st = rankItems("/st", rows);
  assert.ok(st.length > 0 && st.every((r) => /stars/i.test(r.title)), titles(st).join(" | "));
  const cl = rankItems("/cl", rows);
  assert.ok(cl.length > 0 && cl.every((r) => /claim/i.test(r.title)), titles(cl).join(" | "));
});

test("noteUse counts, keeps recency, and never counts ask or answer", () => {
  let usage: Usage = {};
  usage = noteUse(usage, "open-board", 10);
  usage = noteUse(usage, "open-board", 20);
  assert.deepEqual(usage["open-board"], { count: 2, usedAt: 20 });
  const untouched = noteUse(usage, "ask", 30);
  assert.equal(untouched, usage);
  const answered = noteUse(usage, "answer:n1", 30);
  assert.equal(answered, usage);
  // Map stays bounded in memory.
  let many: Usage = {};
  for (let i = 0; i < 60; i += 1) many = noteUse(many, `cmd-${i}`, i + 1);
  assert.ok(Object.keys(many).length <= 40, String(Object.keys(many).length));
  assert.equal(many["cmd-59"]?.count, 1);
});

test("ghost shows the rest of the top match after the caret", () => {
  const items = launcherItems(ctx({ query: "open b" }));
  assert.equal(ghostSuffix("open b", items), "oard");
  assert.equal(ghostSuffix("OPEN B", items), "oard", "case-insensitive prefix");
  assert.equal(ghostSuffix("open bo", items), "ard");
  assert.equal(ghostSuffix("open board", items), "", "no ghost on an exact title");
});

test("no ghost when caret is not at end, composing, slash, ask only", () => {
  const items = launcherItems(ctx({ query: "open b" }));
  assert.equal(ghostSuffix("open b", items, false), "");
  assert.equal(ghostSuffix("open b", items, true, true), "");
  const slash = launcherItems(ctx({ query: "/op" }));
  assert.equal(ghostSuffix("/op", slash), "");
  const askOnly = launcherItems(ctx({ query: "zzz qqq" }));
  assert.equal(askOnly[0]?.kind, "ask");
  assert.equal(ghostSuffix("zzz qqq", askOnly), "");
  // Armed answer row first still ghosts off the top command row.
  const armed = launcherItems(ctx({ query: "open b", armed: { id: "n1", title: "Test" } }));
  assert.equal(armed[0]?.kind, "answer");
  assert.equal(ghostSuffix("open b", armed), "oard");
});

test("Tab takes the ghost; Return never does", () => {
  const items = launcherItems(ctx({ query: "open b" }));
  const names = items.map((r) => r.title);
  const state = { query: "open b", selected: 0, count: items.length, visibleStart: 0, stash: null };
  const tab = launcherKey(state, { key: "Tab", ghost: "oard", ghostTitle: "Open Board" }, names);
  assert.equal(tab.effect, "complete");
  assert.equal(tab.complete, "Open Board");
  assert.equal(tab.state.query, "Open Board");

  const enter = launcherKey(state, { key: "Enter", ghost: "oard" }, names);
  assert.equal(enter.effect, "run");
  assert.equal(enter.runIndex, 0);
  assert.equal(enter.complete, undefined);
  assert.equal(enter.state.query, "open b", "Return never types the ghost");

  // No ghost: Tab keeps today's complete-the-selected-title.
  const plain = launcherKey(
    { query: "ka", selected: 1, count: 2, visibleStart: 0, stash: null },
    { key: "Tab" },
    ["Ask Orbitty: ka", "Go to Kansas"],
  );
  assert.equal(plain.complete, "Go to Kansas");
});

test("Cmd-1..9 run visibleStart + N - 1, past the end ignored", () => {
  const state = { query: "st", selected: 0, count: 20, visibleStart: 3, stash: null };
  for (let n = 1; n <= 9; n += 1) {
    const result = launcherKey(state, { key: String(n), meta: true }, []);
    assert.equal(result.effect, "run", `Cmd-${n}`);
    assert.equal(result.runIndex, 3 + n - 1, `Cmd-${n}`);
  }
  const short = { ...state, count: 4 };
  assert.equal(launcherKey(short, { key: "5", meta: true }, []).effect, "ignore");
  assert.equal(launcherKey(short, { key: "9", meta: true }, []).effect, "ignore");
  // Cmd-Opt-digit stays the global tile chord: not handled here.
  const opt = launcherKey(state, { key: "2", meta: true, alt: true }, []);
  assert.equal(opt.effect, "none");
  assert.equal(opt.runIndex, undefined);
  // Shift is not part of the quick keys.
  assert.equal(launcherKey(state, { key: "2", meta: true, shift: true }, []).effect, "run");
});

test("empty query with usage leads with the most-used row", () => {
  const rows = catalogue(ctx());
  const usage = used([["open-stars", 2]]);
  assert.equal(rankItems("", rows, usage)[0]?.id, "open-stars");
  assert.equal(rankItems("", rows)[0]?.id, "open-board");
});

test("dock paints the hint, the ghost, and the 1-9 keys", () => {
  const launcher = src("src/lib/companion/Launcher.svelte");
  assert.match(launcher, /launcher-key/, "per-row number hint");
  assert.match(launcher, /\{i - visibleStart \+ 1\}/, "hint numbers the visible row");
  assert.match(launcher, /< 9/, "hints stop at nine");

  const dock = src("src/lib/CompanionDock.svelte");
  assert.match(dock, /ghostSuffix\(/, "dock asks the pure helper");
  assert.match(dock, /ghost=\{|ghost,\s*\n/, "ghost passed to the mouth");
  assert.match(dock, /\/\^\[1-9\]\$\/\.test\(e\.key\)/, "preventDefault covers 1-9");
  assert.match(dock, /noteUse\(/, "usage counted on a run");

  const mouth = src("src/lib/companion/Mouth.svelte");
  assert.match(mouth, /class="fieldghost"[^>]*aria-hidden="true"|aria-hidden="true"[^>]*class="fieldghost"/);
  assert.match(mouth, /\{ghost\}/);
  assert.match(mouth, /variant !== "line"/, "ghost is dock-only");
});

test("ghost and hint styles stay on theme vars", () => {
  const css = src("src/lib/companion/dock.css");
  const ghost = css.slice(css.indexOf(".fieldghost"), css.indexOf(".fieldghost") + 2000);
  assert.ok(ghost.length > 0, "fieldghost rules exist");
  assert.equal(/#[0-9a-fA-F]{3,8}/.test(ghost), false, ghost);
  assert.equal(/rgb\(/.test(ghost), false, ghost);
  assert.match(ghost, /var\(--dim\)/);
  const launcher = src("src/lib/companion/Launcher.svelte");
  const style = launcher.slice(launcher.indexOf("<style>"), launcher.indexOf("</style>"));
  const hint = style.slice(style.indexOf(".launcher-key"));
  assert.ok(hint.length > 0, "launcher-key style exists");
  assert.equal(/#[0-9a-fA-F]{3,8}/.test(hint), false, hint);
  assert.equal(/rgb\(/.test(hint), false, hint);
});
