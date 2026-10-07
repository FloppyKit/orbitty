import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { stripTypeScriptTypes } from "node:module";
import { test } from "node:test";
import {
  starsColumns,
  starsGridTemplate,
  starsLayout,
} from "../src/lib/stars/starsLayout.ts";
import {
  AUTOSAVE_MS,
  autosaveDue,
  columnNext,
  focusOrigin,
  focusStep,
  segmentNext,
  starsFieldGate,
  zoneColumn,
  type FocusWorld,
  type StarsFocus,
} from "../src/lib/stars/starsFocus.ts";
import {
  HISTORY_DAY_CAP,
  HISTORY_SPAN_CAP,
  trackerSessions,
  trackerWeek,
  withCounterHistory,
  withTimerStop,
} from "../src/lib/stars/trackerHistory.ts";
import type { Tracker } from "../src/lib/persist.ts";
import { STARS_SPOKES, radarDraft, ratingFromKey } from "../src/lib/stars/radarGeom.ts";
import { classifyKey, surfaceGroups, tabMove } from "../src/lib/ringKeys.ts";

const root = new URL("..", import.meta.url);
function src(rel: string): string {
  return readFileSync(new URL(rel, root), "utf8");
}

function styleOf(rel: string): string {
  return [...src(rel).matchAll(/<style>([\s\S]*?)<\/style>/g)].map((m) => m[1]).join("\n");
}

function braceBody(text: string, from: number): string {
  const open = text.indexOf("{", from);
  assert.ok(open >= 0);
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    if (text[i] === "{") depth++;
    else if (text[i] === "}") {
      depth--;
      if (depth === 0) return text.slice(open, i + 1);
    }
  }
  throw new Error("unbalanced");
}

function world(over: Partial<FocusWorld> = {}): FocusWorld {
  return {
    layout: "three",
    counts: { Z0: 4, Z1: 3, Z2: 5, Z3: 3, Z4: 2, Z5: 4, Z6: 3, Z7: 3 },
    trackerCols: 2,
    cardControls: [4, 0],
    cardNumberCtl: [0, -1],
    spokeItems: 5,
    embedded: false,
    outward: "left",
    reflectOpen: false,
    segment: "checkin",
    ...over,
  };
}

function at(zone: StarsFocus["zone"], index = 0, depth: 0 | 1 | 2 = 0, ctl = 0): StarsFocus {
  const pos = focusOrigin(zone);
  return { ...pos, index, depth, ctl };
}

test("T1 journal is its own column and trackers stay in the radar column", () => {
  assert.equal(starsLayout(900), "three");
  assert.equal(starsLayout(640), "dots");
  assert.equal(starsLayout(599), "single");
  const three = starsColumns("three");
  const dots = starsColumns("dots");
  assert.equal(three.middle, "minmax(360px, 1fr)");
  assert.equal(three.journal, "minmax(320px, 0.9fr)");
  assert.equal(starsGridTemplate("three"), `168px ${three.middle} ${three.journal}`);
  assert.equal(starsGridTemplate("dots"), `72px ${dots.middle} ${dots.journal}`);
  assert.equal(starsGridTemplate("single"), "minmax(0, 1fr)");
  const layoutSrc = src("src/lib/stars/starsLayout.ts");
  assert.equal(layoutSrc.includes("journalColumn"), false);
  const gaps = 20;
  const padding = 24;
  const need = dots.planet + dots.middleMin + dots.journalMin + gaps + padding;
  for (let w = 600; w <= 899; w++) {
    assert.equal(starsLayout(w), "dots");
    assert.ok(need <= w, `${need} <= ${w}`);
  }
  assert.ok(168 + 360 + 320 + gaps + padding <= 900);
});

test("T2 folio is outside the radar column and trackers follow the day lines", () => {
  const page = src("src/lib/StarsPage.svelte");
  const midAt = page.indexOf('class="mid-col"');
  const journalAt = page.indexOf('class="journal-col"');
  assert.ok(midAt > 0 && journalAt > midAt);
  const mid = page.slice(midAt, journalAt);
  assert.equal(mid.includes("<AlmanacFolio"), false);
  assert.ok(mid.indexOf("<DayPositives") < mid.indexOf("<StarsTrackers"));
  assert.ok(page.indexOf("<AlmanacFolio") > journalAt);
  assert.match(page, /data-stars-col="middle"/);
  assert.match(page, /data-stars-col="trackers"/);
});

test("T3 save chip is gone and autosave waits for a real edit", () => {
  assert.equal(AUTOSAVE_MS, 600);
  assert.equal(autosaveDue(false), false);
  assert.equal(autosaveDue(true), true);
  const page = src("src/lib/StarsPage.svelte");
  assert.equal(page.includes("Save today"), false);
  assert.equal(page.includes("stars-save"), false);
  assert.equal(page.includes(">Edit</button>"), false);
  const save = braceBody(page, page.indexOf("async function saveStarsToday"));
  assert.match(save, /if \(!autosaveDue\(draftDirty\)\) return/);
  const schedule = braceBody(page, page.indexOf("function scheduleAutosave"));
  assert.match(schedule, /AUTOSAVE_MS/);
  assert.match(page, /function flushAutosave/);
  assert.match(page, /document\.hidden/);
  assert.match(page, /moveDay\(/);
  for (const head of ["onMount(async", "function loadState", "function syncStarsDraft"]) {
    const body = braceBody(page, page.indexOf(head));
    assert.equal(body.includes("saveStarsToday"), false, head);
    assert.equal(body.includes("save_state"), false, head);
  }
  const script = page.slice(page.indexOf("<script"), page.indexOf("</script>"));
  assert.equal(script.includes("$effect(") && script.slice(script.indexOf("$effect")).includes("saveStarsToday")
    ? script.split("$effect").slice(1).some((chunk) => chunk.split("});")[0]?.includes("saveStarsToday"))
    : false, false);
});

// T3b (Verify 45a37c5 fix): run the real StarsPage autosave functions against
// fake timers and a fake save_state, so a dropped dirty flag, a dirty sync,
// a missing day-change / hide / close flush or a lost debounce goes red.
function autosaveHarness() {
  const page = src("src/lib/StarsPage.svelte");
  const script = page.slice(page.indexOf("<script"), page.indexOf("</script>"));
  const decl = (head: string): string => {
    const at = script.indexOf(head);
    assert.ok(at >= 0, head);
    assert.equal(script.indexOf(head, at + 1), -1, `${head} once`);
    const open = script.indexOf("{", at);
    return script.slice(at, open) + braceBody(script, at) + ";\n";
  };
  const heads = [
    "function moveDay(",
    "function scheduleAutosave(",
    "function flushAutosave(",
    "function noteEdit(",
    "function syncStarsDraft(",
    "function setSpoke(",
    "function setWin(",
    "function filled(",
    "async function saveStarsToday(",
    "async function loadState(",
    "function hideStars(",
    "const onVisibility = ",
  ];
  const code = stripTypeScriptTypes(heads.map(decl).join("\n"));
  const today = "2026-10-07";
  let now = 0;
  let seq = 0;
  const timers = new Map<number, { at: number; fn: () => void }>();
  let disk: Record<string, unknown> = {};
  const writes: { day: string; row: Record<string, unknown> }[] = [];
  const S: Record<string, unknown> = {
    cursorDay: today,
    draftDirty: false,
    saveTimer: undefined,
    starsDraft: {},
    starsSavedDay: "",
    savedLabel: "",
    starsState: { promptsOptIn: false, checkins: {} },
    bagState: null,
    avgMode: false,
    embedded: false,
    hideLabel: "stars",
    onClose: undefined,
    onReshow: () => {},
    themeId: "",
    windowOpacity: 1,
    KNOWN_THEMES: new Set(["systemLight", "systemDark", "terminal"]),
    osThemeId: () => "systemDark",
    clampWindowOpacity: (v: number) => v,
    planetDragging: false,
    projects: [],
    trackerScope: "overall",
    starsLoaded: false,
    document: { hidden: false },
    get isToday() {
      return S.cursorDay === today;
    },
    get dayEditable() {
      return S.cursorDay === today && !S.avgMode;
    },
    todayKey: () => today,
    emptyCheckin: () => ({ sleep: 0, terroir: 0, athleticism: 0, relationships: 0, self: 0 }),
    radarDraft,
    STARS_SPOKES,
    autosaveDue,
    AUTOSAVE_MS,
    clockLabel: () => "11:52",
    setTimeout: (fn: () => void, ms: number) => {
      timers.set(++seq, { at: now + ms, fn });
      return seq;
    },
    clearTimeout: (id: number) => {
      timers.delete(id);
    },
    invoke: async (cmd: string, args?: { state: { stars: { checkins: Record<string, Record<string, unknown>> } } }) => {
      if (cmd === "load_state") return structuredClone({ stars: { promptsOptIn: false, checkins: disk } });
      if (cmd === "save_state" && args) {
        const before = disk;
        disk = structuredClone(args.state.stars.checkins);
        for (const [day, row] of Object.entries(disk)) {
          if (JSON.stringify(before[day]) !== JSON.stringify(row)) writes.push({ day, row: row as Record<string, unknown> });
        }
      }
      return undefined;
    },
  };
  const fns = new Function(
    "S",
    `with (S) {\n${code}\nreturn { moveDay, flushAutosave, syncStarsDraft, loadState, setSpoke, setWin, hideStars, onVisibility };\n}`,
  )(S) as Record<string, (...a: unknown[]) => unknown>;
  const settle = async () => {
    for (let i = 0; i < 5; i++) await new Promise((r) => setImmediate(r));
  };
  const advance = async (ms: number) => {
    const end = now + ms;
    for (;;) {
      const due = [...timers].filter(([, t]) => t.at <= end).sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      timers.delete(due[0]);
      now = due[1].at;
      due[1].fn();
      await settle();
    }
    now = end;
  };
  return { S, fns, writes, advance, settle, today, yesterday: "2026-10-06" };
}

test("T3b autosave runs on a real edit only: debounced, flushed on day change / hide / close", async () => {
  // Mount / load / sync / navigation / hide / close never write the untouched draft.
  {
    const h = autosaveHarness();
    await h.fns.loadState();
    h.fns.syncStarsDraft();
    assert.equal((h.S.starsDraft as Record<string, number>).sleep, 3, "draft starts at 3");
    assert.equal(h.S.draftDirty, false, "sync leaves the draft clean");
    h.fns.flushAutosave();
    (h.S.document as { hidden: boolean }).hidden = true;
    h.fns.onVisibility();
    h.fns.hideStars();
    h.fns.moveDay(h.yesterday);
    h.fns.moveDay(h.today);
    await h.advance(5000);
    assert.equal(h.writes.length, 0, "untouched draft never saves");
  }
  // One edit: nothing before 600 ms, one write at 600 ms.
  {
    const h = autosaveHarness();
    h.fns.syncStarsDraft();
    h.fns.setSpoke("sleep", 4);
    assert.equal(h.S.draftDirty, true, "an edit marks the draft dirty");
    await h.advance(599);
    assert.equal(h.writes.length, 0, "debounced");
    await h.advance(1);
    assert.equal(h.writes.length, 1, "first real edit autosaves");
    assert.equal(h.writes[0].day, h.today);
    assert.equal(h.writes[0].row.sleep, 4);
    assert.equal(h.S.draftDirty, false);
    await h.advance(5000);
    assert.equal(h.writes.length, 1, "one write per edit burst");
  }
  // A state:changed reload mid-debounce keeps the pending edit and still saves it.
  {
    const h = autosaveHarness();
    h.fns.syncStarsDraft();
    h.fns.setSpoke("terroir", 2);
    await h.advance(300);
    await h.fns.loadState();
    assert.equal((h.S.starsDraft as Record<string, number>).terroir, 2, "reload keeps the edit");
    await h.advance(600);
    assert.equal(h.writes.length, 1, "reload does not cancel the autosave");
    assert.equal(h.writes[0].row.terroir, 2);
  }
  // A burst of edits restarts the 600 ms window and lands as one write.
  {
    const h = autosaveHarness();
    h.fns.syncStarsDraft();
    h.fns.setSpoke("sleep", 4);
    await h.advance(400);
    h.fns.setWin("win1", "walked");
    await h.advance(400);
    assert.equal(h.writes.length, 0, "timer restarts on each edit");
    await h.advance(200);
    assert.equal(h.writes.length, 1);
    assert.equal(h.writes[0].row.sleep, 4);
    assert.equal(h.writes[0].row.win1, "walked");
  }
  // Day change, hide and close flush the pending edit at once, then the timer is spent.
  for (const leave of ["day", "hide", "close"] as const) {
    const h = autosaveHarness();
    h.fns.syncStarsDraft();
    h.fns.setSpoke("self", 5);
    if (leave === "day") h.fns.moveDay(h.yesterday);
    if (leave === "hide") {
      (h.S.document as { hidden: boolean }).hidden = true;
      h.fns.onVisibility();
    }
    if (leave === "close") h.fns.hideStars();
    await h.settle();
    assert.equal(h.writes.length, 1, `${leave} flushes the edit`);
    assert.equal(h.writes[0].day, h.today, leave);
    assert.equal(h.writes[0].row.self, 5, leave);
    await h.advance(5000);
    assert.equal(h.writes.length, 1, `${leave}: no second write`);
  }
  // Past days stay read-only: no write.
  {
    const h = autosaveHarness();
    h.fns.syncStarsDraft();
    h.fns.moveDay(h.yesterday);
    h.fns.setSpoke("sleep", 5);
    await h.advance(5000);
    assert.equal(h.writes.length, 0, "past day edit is refused");
  }
});

test("T4 tracker card order, ink value, neutral bars, round buttons", () => {
  const card = src("src/lib/stars/TrackerCard.svelte");
  const top = card.indexOf('class="top"');
  const val = card.indexOf('class="val');
  const bars = card.indexOf('class="bars"');
  const foot = card.indexOf('class="foot"');
  assert.ok(top > 0 && top < val && val < bars && bars < foot);
  assert.equal(card.includes("planetCss"), false);
  assert.equal(card.includes("var(--p)"), false);
  const css = styleOf("src/lib/stars/TrackerCard.svelte");
  const valRule = css.slice(css.indexOf(".val,"), css.indexOf("}", css.indexOf(".val,")));
  assert.match(valRule, /color:\s*var\(--ink\)/);
  assert.match(valRule, /font-variant-numeric:\s*tabular-nums/);
  assert.match(css, /\.bar\.today[\s\S]*color-mix\(in srgb,\s*var\(--ink\)\s*70%/);
  assert.match(css, /\.round[\s\S]*border:\s*none/);
  assert.match(css, /\.round[\s\S]*border-radius:\s*50%/);
  assert.match(src("src/lib/stars/StarsTrackers.svelte"), /minmax\(150px,\s*1fr\)/);
});

test("T4b PlanetChip is the only round sphere", () => {
  const css = styleOf("src/lib/stars/PlanetChip.svelte");
  const rule = css.slice(css.indexOf(".planet-chip {"), css.indexOf("}", css.indexOf(".planet-chip {")));
  assert.match(rule, /width:\s*var\(--chip\)/);
  assert.match(rule, /height:\s*var\(--chip\)/);
  assert.match(rule, /min-width:\s*var\(--chip\)/);
  assert.match(rule, /min-height:\s*var\(--chip\)/);
  assert.match(rule, /aspect-ratio:\s*1\s*\/\s*1/);
  assert.match(rule, /border-radius:\s*50%/);
  assert.match(rule, /flex:\s*none/);
  assert.match(rule, /radial-gradient/);
  assert.match(rule, /var\(--p\)/);
  assert.equal(/#fff\b|white/i.test(rule), false);
  assert.equal(/border-radius:\s*99px/.test(css), false);
  const files = readdirSync(new URL("../src/lib/stars/", import.meta.url)).filter((n) => n.endsWith(".svelte"));
  files.push("../StarsPage.svelte");
  for (const name of files) {
    const rel = name.startsWith("..") ? "src/lib/StarsPage.svelte" : `src/lib/stars/${name}`;
    const text = src(rel);
    if (name === "PlanetChip.svelte") {
      assert.match(text, /planetCss\(/);
      continue;
    }
    assert.equal(text.includes("planetCss("), false, rel);
    assert.equal(/style="background:\s*\{planetCss/.test(text), false, rel);
  }
  for (const host of ["StarsTrackers.svelte", "StarsPlanetColumn.svelte", "LedgerStack.svelte"]) {
    assert.match(src(`src/lib/stars/${host}`), /<PlanetChip/);
  }
});

test("T5 controls reveal by opacity on hover and focus-within", () => {
  const css = styleOf("src/lib/stars/TrackerCard.svelte");
  assert.match(css, /\.card:hover\s+\.foot/);
  assert.match(css, /\.card:focus-within\s+\.foot/);
  assert.equal(/display\s*:\s*none/.test(css), false);
  const foot = css.slice(css.indexOf(".foot {"), css.indexOf("}", css.indexOf(".foot {")));
  assert.match(foot, /opacity:\s*0/);
});

test("T6 tracker week, sessions, and capped history writes", () => {
  const bare: Tracker = { id: "t", label: "Steps", value: 4 };
  assert.deepEqual(trackerWeek(bare, "2026-10-07"), [0, 0, 0, 0, 0, 0, 0]);
  const counted = withCounterHistory(bare, "2026-10-07", 9);
  assert.equal(counted.value, 9);
  assert.equal(counted.history?.days?.["2026-10-07"], 9);
  assert.equal(trackerWeek(counted, "2026-10-07")[6], 9);
  const running: Tracker = { id: "t", label: "Sit", value: 30, kind: "timer", runningSince: 1_000 };
  const stopped = withTimerStop(running, "2026-10-07", 11_000);
  assert.equal(stopped.runningSince, undefined);
  assert.equal(stopped.history?.days?.["2026-10-07"], 10);
  assert.deepEqual(trackerSessions(stopped, "2026-10-07"), [[1_000, 11_000]]);
  assert.deepEqual(trackerSessions(stopped, "2026-10-06"), []);
  let many = stopped;
  for (let i = 0; i < 30; i++) {
    many = withTimerStop({ ...many, kind: "timer", runningSince: i * 1000 }, "2026-10-07", i * 1000 + 500);
  }
  assert.ok((many.history?.sessions?.spans.length ?? 0) <= HISTORY_SPAN_CAP);
  let days = bare;
  for (let i = 0; i < HISTORY_DAY_CAP + 5; i++) {
    const key = `2026-01-${String((i % 28) + 1).padStart(2, "0")}`;
    days = withCounterHistory(days, `2025-01-${String(i + 1).padStart(2, "0")}`, i);
    void key;
  }
  assert.ok(Object.keys(days.history?.days ?? {}).length <= HISTORY_DAY_CAP);
  const oldest = Object.keys(days.history?.days ?? {}).sort()[0];
  assert.notEqual(oldest, "2025-01-01");
});

// T6b (Verify 45a37c5 fix): the real StarsPage bump / set / stop handlers write
// history through trackerHistory, and a second stop on the same day adds.
test("T6b bump / set / stop write history and same-day stops accumulate", () => {
  const first = withTimerStop({ id: "t", label: "Sit", value: 0, kind: "timer", runningSince: 1_000 }, "2026-10-07", 11_000);
  const second = withTimerStop({ ...first, runningSince: 20_000 }, "2026-10-07", 25_000);
  assert.equal(second.history?.days?.["2026-10-07"], 15, "stop adds to today's seconds");
  assert.deepEqual(trackerSessions(second, "2026-10-07"), [[1_000, 11_000], [20_000, 25_000]]);
  const nextDay = withTimerStop({ ...second, runningSince: 90_000 }, "2026-10-08", 95_000);
  assert.deepEqual(trackerSessions(nextDay, "2026-10-08"), [[90_000, 95_000]], "a new day starts a new session list");
  assert.equal(nextDay.history?.days?.["2026-10-07"], 15);
  assert.equal(nextDay.history?.days?.["2026-10-08"], 5);

  const page = src("src/lib/StarsPage.svelte");
  const script = page.slice(page.indexOf("<script"), page.indexOf("</script>"));
  const decl = (head: string): string => {
    const at = script.indexOf(head);
    assert.ok(at >= 0, head);
    assert.equal(script.indexOf(head, at + 1), -1, `${head} once`);
    return script.slice(at, script.indexOf("{", at)) + braceBody(script, at) + ";\n";
  };
  const code = stripTypeScriptTypes(
    ["function mapTracker(", "function bumpTracker(", "function setTrackerValue(", "function stopTimer("].map(decl).join("\n"),
  );
  let list: { id: string; trackers?: Tracker[] }[] = [];
  const S: Record<string, unknown> = {
    todayKey: () => "2026-10-07",
    withCounterHistory,
    withTimerStop,
    Date: { now: () => 31_000 },
    patchProjects: (fn: (l: typeof list) => typeof list) => {
      list = fn(list);
      return Promise.resolve();
    },
  };
  const fns = new Function("S", `with (S) {\n${code}\nreturn { bumpTracker, setTrackerValue, stopTimer };\n}`)(S) as Record<
    string,
    (...a: unknown[]) => unknown
  >;
  const tr = () => list[0]?.trackers?.[0];
  list = [{ id: "p", trackers: [{ id: "c", label: "Steps", value: 4 }] }];
  fns.bumpTracker("p", "c", 1);
  assert.equal(tr()?.value, 5);
  assert.equal(tr()?.history?.days?.["2026-10-07"], 5, "bump writes days[today]");
  fns.setTrackerValue("p", "c", "12");
  assert.equal(tr()?.history?.days?.["2026-10-07"], 12, "typed set writes days[today]");
  list = [{ id: "p", trackers: [{ id: "c", label: "Sit", value: 10, kind: "timer", runningSince: 1_000, history: { days: { "2026-10-07": 10 } } }] }];
  fns.stopTimer("p", "c");
  assert.equal(tr()?.runningSince, undefined);
  assert.equal(tr()?.history?.days?.["2026-10-07"], 40, "stop adds the session seconds");
  assert.deepEqual(trackerSessions(tr() as Tracker, "2026-10-07"), [[1_000, 31_000]]);
});

test("T7 zones move one level and the embedded edge exits", () => {
  const w = world();
  const right = focusStep(at("Z1", 1), "ArrowRight", w);
  assert.equal(right.column, true);
  assert.equal(right.pos.zone, "Z2");
  assert.equal(right.exit, false);
  const into = focusStep(at("Z2", 0), "Enter", w);
  assert.equal(into.pos.depth, 1);
  assert.equal(into.activate, false);
  const out = focusStep(into.pos, "Escape", w);
  assert.equal(out.pos.depth, 0);
  assert.equal(out.pos.zone, "Z2");
  assert.equal(out.rootEsc, null);
  const root = focusStep(out.pos, "Escape", w);
  assert.equal(root.pos.zone, "Z0");
  assert.equal(root.rootEsc, null);
  const sheet = focusStep(at("Z2"), "Escape", world({ reflectOpen: true }));
  assert.equal(sheet.rootEsc, "reflect");
  const sheetDeep = focusStep({ ...at("Z2"), depth: 1 }, "Escape", world({ reflectOpen: true }));
  assert.equal(sheetDeep.rootEsc, "reflect");
  assert.equal(sheetDeep.pos.depth, 1);
  const z0 = focusStep(at("Z0", 0), "Escape", w);
  assert.equal(z0.rootEsc, "hide");
  assert.equal(z0.pos.zone, "Z0");
  const z0sheet = focusStep(at("Z0", 0), "Escape", world({ reflectOpen: true }));
  assert.equal(z0sheet.rootEsc, "reflect");
  for (const zone of ["Z1", "Z2", "Z3", "Z4", "Z5", "Z6", "Z7"] as const) {
    const stepped = focusStep(at(zone, zone === "Z1" ? 2 : 0), "Escape", w);
    assert.equal(stepped.pos.zone, "Z0", `${zone} steps out to Z0`);
    assert.equal(stepped.rootEsc, null, `${zone} does not hide`);
  }
  const card = focusStep(at("Z4", 0), "Enter", w);
  assert.equal(card.pos.depth, 1);
  assert.equal(card.pos.ctl, 0);
  const ctl = focusStep(card.pos, "ArrowRight", w);
  assert.equal(ctl.pos.depth, 1);
  assert.equal(ctl.pos.ctl, 1);
  const typed = focusStep({ ...card.pos, ctl: 0 }, "Enter", w);
  assert.equal(typed.pos.depth, 2);
  const one = focusStep(typed.pos, "Escape", w);
  assert.equal(one.pos.depth, 1, "Esc drops one level");
  assert.notEqual(one.pos.depth, 0);
  const add = focusStep(at("Z4", 1), "Enter", w);
  assert.equal(add.activate, true);
  const exit = focusStep(at("Z1", 0), "ArrowLeft", world({ embedded: true, outward: "left" }));
  assert.equal(exit.exit, true);
  assert.equal(columnNext("three", "planets", "left", { embedded: true, outward: "left" }).exit, true);
  const stayed = focusStep(at("Z1", 0), "ArrowLeft", world({ embedded: true, outward: "right" }));
  assert.equal(stayed.exit, false);
  const up = focusStep(at("Z2", 0), "ArrowUp", w);
  assert.equal(up.pos.zone, "Z0");
  const back = focusStep(up.pos, "ArrowDown", w);
  assert.equal(back.pos.zone, "Z2");
  const down = focusStep(at("Z3", 2), "ArrowDown", w);
  assert.equal(down.pos.zone, "Z4");
  const single = focusStep(at("Z2", 0), "ArrowRight", world({ layout: "single", segment: "checkin" }));
  assert.equal(single.segment, "trackers");
  assert.equal(single.pos.zone, "Z4");
  const home = focusStep(at("Z1", 2), "Home", w);
  assert.equal(home.pos.index, 0);
  const end = focusStep(at("Z1", 0), "End", w);
  assert.equal(end.pos.index, 2);
  const rings = ["src/lib/StarsPage.svelte", ...readdirSync(new URL("../src/lib/stars/", import.meta.url))
    .filter((n) => n.endsWith(".svelte"))
    .map((n) => `src/lib/stars/${n}`)];
  let owners = 0;
  for (const rel of rings) {
    const css = styleOf(rel);
    owners += css.match(/box-shadow:\s*var\(--focus-ring\)/g)?.length ?? 0;
  }
  assert.equal(owners, 1);
  const page = src("src/lib/StarsPage.svelte");
  assert.match(page, /removeAttribute\("data-stars-focus"\)/);
  assert.match(page, /setAttribute\("data-stars-focus", "1"\)/);
  assert.match(page, /onRingExit\?\.\(\)/);
});

// T7b (Verify 45a37c5 fix): ← among card controls, every zone tag is enumerable,
// and a radar handle only edits after Return (live), never while arrowing past it.
test("T7b card controls move both ways, every zone tag is enumerable, handles edit only when live", () => {
  const w = world();
  const card = focusStep(at("Z4", 0), "Enter", w);
  const right = focusStep(card.pos, "ArrowRight", w);
  assert.equal(right.pos.ctl, 1);
  const left = focusStep(right.pos, "ArrowLeft", w);
  assert.equal(left.pos.depth, 1, "← stays among the card controls");
  assert.equal(left.pos.ctl, 0, "← moves to the previous control");
  const files = [
    "src/lib/StarsPage.svelte",
    "src/lib/SkyField.svelte",
    ...readdirSync(new URL("../src/lib/stars/", import.meta.url))
      .filter((n) => n.endsWith(".svelte") && n !== "ReflectSheet.svelte")
      .map((n) => `src/lib/stars/${n}`),
  ];
  for (const rel of files) {
    for (const tag of src(rel).match(/<[a-z]+\b[^>]*data-focus-zone[^>]*>/g) ?? []) {
      assert.ok(/data-focus-(item|ctl)\b/.test(tag), `${rel} ${tag.slice(0, 80)} is never enumerated`);
    }
  }
  const page = src("src/lib/StarsPage.svelte");
  assert.match(page, /<RadarDial[\s\S]*?live=\{spokeLive\}/);
  const dial = src("src/lib/stars/RadarDial.svelte");
  const at0 = dial.indexOf("function onHandleKey(");
  assert.ok(at0 >= 0);
  const code = stripTypeScriptTypes(dial.slice(at0, dial.indexOf("{", at0)) + braceBody(dial, at0));
  for (const live of [false, true]) {
    const sets: [string, number][] = [];
    const S = {
      editable: true,
      avgMode: false,
      live,
      draft: false,
      ratingFromKey,
      spokeValue: () => 3,
      onSet: (k: string, v: number) => sets.push([k, v]),
    };
    const onHandleKey = new Function("S", `with (S) {\n${code}\nreturn onHandleKey;\n}`)(S) as (e: unknown, k: string) => void;
    for (const key of ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"]) {
      onHandleKey({ key, preventDefault() {} }, "sleep");
    }
    if (live) assert.equal(sets.length, 4, "live handle edits with arrows");
    else assert.equal(sets.length, 0, "arrowing past a handle never edits it");
  }
});

test("T8 every interactive stars control declares a focus zone", () => {
  const files = [
    "src/lib/StarsPage.svelte",
    "src/lib/SkyField.svelte",
    ...readdirSync(new URL("../src/lib/stars/", import.meta.url))
      .filter((n) => n.endsWith(".svelte") && n !== "ReflectSheet.svelte")
      .map((n) => `src/lib/stars/${n}`),
  ];
  const zones = new Set(["Z0", "Z1", "Z2", "Z3", "Z4", "Z5", "Z6", "Z7"]);
  for (const rel of files) {
    const text = src(rel);
    const tags = text.match(/<(?:button|input|select)\b[^>]*>|<g\b[^>]*role="slider"[^>]*>/g) ?? [];
    for (const tag of tags) {
      assert.match(tag, /data-focus-zone/, `${rel} ${tag.slice(0, 80)}`);
      const named = tag.match(/data-focus-zone="(Z[0-7])"/);
      if (named) assert.ok(zones.has(named[1] ?? ""));
    }
  }
});

test("T9 tracker styles stay on tokens and do not grow a shadow", () => {
  const css = [styleOf("src/lib/stars/TrackerCard.svelte"), styleOf("src/lib/stars/StarsTrackers.svelte"), styleOf("src/lib/stars/PlanetChip.svelte")].join("\n");
  const stripped = css.replace(/rgba\(\s*0\s*,\s*0\s*,\s*0\s*,[^)]+\)/g, "").replace(/transparent/g, "");
  assert.equal(/#[0-9a-fA-F]{3,8}/.test(stripped), false);
  assert.equal(/rgba?\(/.test(stripped), false);
  assert.equal(/0\s+18px\s+36px/.test(css), false);
  assert.equal(/drop-shadow\(\s*0\s+18px\s+36px/.test(css), false);
  for (const block of css.match(/box-shadow:\s*([^;]+);/g) ?? []) {
    const nums = [...block.matchAll(/(\d+(?:\.\d+)?)px/g)].map((m) => Number(m[1]));
    const blur = nums.length >= 3 ? nums[2] : 0;
    assert.ok((blur ?? 0) <= 8, block);
  }
});

// T9b (Verify 45a37c5 fix): D-T / D-C / D-G surface values exactly, so a light bar,
// a brighter card, a glow, a bigger or brighter chip, or a filled Terminal card goes red.
test("T9b tracker card, bars and chip keep the D-T / D-C values; no glow under trackers", () => {
  const card = styleOf("src/lib/stars/TrackerCard.svelte");
  const rule = (css: string, sel: string): string => {
    const at = css.indexOf(`${sel} {`);
    assert.ok(at >= 0, sel);
    return css.slice(at, css.indexOf("}", at));
  };
  assert.match(rule(card, "\n  .card"), /background:\s*var\(--panel-fill\);/);
  const hover = rule(card, "\n  .card:hover");
  assert.match(hover, /background:\s*color-mix\(in srgb,\s*var\(--ink\)\s*4%,\s*var\(--panel-fill\)\);/);
  for (const m of card.matchAll(/translateY\(\s*(-?[\d.]+)px\s*\)/g)) assert.ok(Math.abs(Number(m[1])) <= 1, "at most 1px lift");
  const bar = rule(card, "\n  .bar");
  assert.match(bar, /background:\s*var\(--dim\);/);
  assert.match(bar, /opacity:\s*0\.55;/);
  assert.match(rule(card, "\n  .bar.today"), /background:\s*color-mix\(in srgb,\s*var\(--ink\)\s*70%,\s*transparent\);/);
  const empty = rule(card, "\n  .bar.empty");
  assert.match(empty, /background:\s*var\(--dim\);/);
  assert.match(rule(card, "\n  .sess"), /color-mix\(in srgb,\s*var\(--ink\)\s*70%,\s*transparent\)/);
  const term = rule(card, ':global(:root[data-theme="terminal"]) .card');
  assert.match(term, /background:\s*transparent;/);
  const css = [card, styleOf("src/lib/stars/StarsTrackers.svelte"), styleOf("src/lib/stars/PlanetChip.svelte")].join("\n");
  for (const m of css.matchAll(/box-shadow:\s*([^;]+);/g)) {
    for (const layer of (m[1] ?? "").split(/,(?![^(]*\))/)) {
      const lens = layer.replace(/\b(rgba?|var|color-mix)\([^)]*\)/g, " ").match(/-?\d*\.?\d+(px)?\b/g) ?? [];
      const blur = Number((lens[2] ?? "0").replace("px", ""));
      assert.ok(blur <= 8, `blur ${blur} in ${m[0]}`);
    }
  }
  const chip = rule(styleOf("src/lib/stars/PlanetChip.svelte"), "\n  .planet-chip");
  assert.match(
    chip.replace(/\s+/g, " "),
    /radial-gradient\( circle at 34% 30%, color-mix\(in srgb, var\(--p\) 78%, var\(--ink\) 22%\) 0 18%, var\(--p\) 46%, color-mix\(in srgb, var\(--p\) 62%, black\) 100% \)/,
  );
  const termChip = styleOf("src/lib/stars/PlanetChip.svelte").replace(/\s+/g, " ");
  assert.match(termChip, /:global\(:root\[data-theme="terminal"\]\) \.planet-chip \{ background: var\(--p\); box-shadow: none; \}/);
  assert.match(src("src/lib/stars/PlanetChip.svelte"), /size = 10\b/);
  for (const rel of ["src/lib/stars/TrackerCard.svelte", "src/lib/stars/StarsTrackers.svelte", "src/lib/stars/StarsPlanetColumn.svelte", "src/lib/stars/LedgerStack.svelte"]) {
    for (const m of src(rel).matchAll(/<PlanetChip\b[^>]*?size=\{(\d+)\}/g)) assert.ok(Number(m[1]) <= 12, `${rel} chip ${m[1]}px`);
  }
});

// FR2: run the real MenuRing capture listener and the real bubble
// onStarsKey in source order, then dispatch DOM-like keydown events.
type KeyListener = { type: string; fn: (e: KeyEvt) => void; capture: boolean };
type KeyEvt = {
  key: string;
  shiftKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  target: { closest?: (sel: string) => unknown };
  defaultPrevented: boolean;
  preventDefault: () => void;
  stopImmediatePropagation: () => void;
};

function loadMenuRing(win: { addEventListener: Function; removeEventListener: Function }, document: { querySelector: () => null }) {
  let text = src("src/lib/menuKeys.svelte.ts");
  text = text.replace(/^import[^\n]*\n/gm, "");
  text = text.replace(/\$state<[^>]*>\(/g, "$state(");
  text = stripTypeScriptTypes(text);
  text = text.replace(/\bexport\s+/g, "");
  text = text.replace(/\bprivate\s+/g, "");
  const MenuRing = new Function(
    "untrack",
    "$state",
    "classifyKey",
    "surfaceGroups",
    "tabMove",
    "window",
    "document",
    "CSS",
    `${text}\nreturn MenuRing;`,
  )(
    (fn: () => unknown) => fn(),
    (v: unknown) => v,
    classifyKey,
    surfaceGroups,
    tabMove,
    win,
    document,
    { escape: (s: string) => s },
  ) as new () => { setStops: (s: unknown[]) => void; attach: (cb?: () => boolean) => () => void; select: (id: string) => void; onEdge: ((dir: string) => boolean) | null; activeId: string | null };
  return MenuRing;
}

function pageScript(): string {
  const page = src("src/lib/StarsPage.svelte");
  return page.slice(page.indexOf("<script"), page.indexOf("</script>"));
}

function blockBody(text: string, at: number, asyncFn = false): string {
  const raw = braceBody(text, at);
  const wrapped = stripTypeScriptTypes(`${asyncFn ? "async " : ""}function __b() ${raw}`);
  const open = wrapped.indexOf("{");
  return wrapped.slice(open);
}

async function bootStarsKeys(opts: { embedded: boolean; ringActive: boolean }) {
  const listeners: KeyListener[] = [];
  const win = {
    listeners,
    addEventListener(type: string, fn: (e: KeyEvt) => void, options?: boolean | { capture?: boolean }) {
      const capture = options === true || (typeof options === "object" && !!options?.capture);
      listeners.push({ type, fn, capture });
    },
    removeEventListener(type: string, fn: (e: KeyEvt) => void) {
      const i = listeners.findIndex((l) => l.type === type && l.fn === fn);
      if (i >= 0) listeners.splice(i, 1);
    },
  };
  const document = {
    hidden: false,
    addEventListener() {},
    removeEventListener() {},
    querySelector: () => null,
  };
  const MenuRing = loadMenuRing(win, document);
  const menuRing = new MenuRing();
  const log: string[] = [];
  const averages: unknown[] = [];
  let exits = 0;
  let closes = 0;
  const resolvers: ((v: () => void) => void)[] = [];
  const S: Record<string, unknown> = {
    embedded: opts.embedded,
    ringActive: opts.ringActive,
    outwardDir: "left",
    reflectOpen: false,
    spokeLive: false,
    focusCol: "planets",
    focusPos: focusOrigin("Z1"),
    mainEl: null,
    starsFieldGate,
    layout: "three",
    segment: "checkin",
    navView: "day",
    NAV_ORDER: ["day", "week", "month", "year", "custom"],
    avgFrom: "",
    avgTo: "",
    cursorDay: "2026-10-07",
    hideLabel: "stars",
    draftDirty: false,
    saveTimer: undefined,
    detachMenuKeys: undefined,
    unlisten: undefined,
    unlistenVis: undefined,
    menuRing,
    window: win,
    document,
    Element: class Element {},
    STARS_SPOKES,
    focusStep,
    columnNext,
    segmentNext,
    zoneColumn,
    autosaveDue,
    shiftDay: () => "2026-09-07",
    setAverageWindow: (...args: unknown[]) => {
      averages.push(args);
    },
    bumpSpoke: () => {},
    saveFromRing: () => {},
    focusStarsCol: () => {},
    applyStarsFocus: () => {},
    activateStarsFocus: () => {},
    onRingExit: () => {
      exits += 1;
    },
    onClose: () => {
      closes += 1;
    },
    onVisibility: () => {},
    syncStarsDraft: () => {},
    loadState: async () => {},
    listenPaneVis: async () => () => {},
    listen: () => new Promise((res) => resolvers.push(res as (v: () => void) => void)),
    invoke: (cmd: string) => {
      log.push(cmd);
      return Promise.resolve();
    },
    saveStarsToday: async () => {
      log.push("save");
    },
    clearTimeout: () => {},
    setTimeout: () => 1,
    focusWorld: () =>
      world({
        embedded: !!S.embedded,
        reflectOpen: !!S.reflectOpen,
        outward: "left",
        segment: S.segment as "checkin",
        layout: S.layout as "three",
      }),
  };
  const script = pageScript();
  const heads = [
    "function cycleView(",
    "function flushAutosave(",
    "function hideStars(",
    "function applyStep(",
    "function focusElement(",
    "function fieldKind(",
    "function onStarsKey(",
  ];
  const code = stripTypeScriptTypes(
    heads
      .map((head) => {
        const at = script.indexOf(head);
        assert.ok(at >= 0, head);
        const open = script.indexOf("{", at);
        return script.slice(at, open) + braceBody(script, at) + "\n";
      })
      .join("\n"),
  );
  const fns = new Function(
    "S",
    `with (S) {\n${code}\nreturn { cycleView, flushAutosave, hideStars, applyStep, onStarsKey };\n}`,
  )(S) as Record<string, (...a: unknown[]) => unknown>;
  Object.assign(S, fns);

  const page = src("src/lib/StarsPage.svelte");
  const stopsAt = page.lastIndexOf("$effect(() =>", page.indexOf("const stops: RingStop[]"));
  const mountAt = page.indexOf("onMount(async () =>");
  const ringAt = page.lastIndexOf("$effect(() =>", page.indexOf("if (!embedded || !ringActive) return"));
  const keyAt = page.lastIndexOf("onMount(", page.indexOf('window.addEventListener("keydown", onStarsKey)'));
  const blocks = [
    { at: stopsAt, kind: "stops" },
    { at: mountAt, kind: "mount" },
    { at: ringAt, kind: "ring" },
    { at: keyAt, kind: "key" },
  ].sort((a, b) => a.at - b.at);
  let mountP: Promise<unknown> = Promise.resolve();
  for (const block of blocks) {
    const body = blockBody(page, block.at, block.kind === "mount");
    if (block.kind === "mount") {
      mountP = new Function("S", `with (S) { return (async () => ${body})(); }`)(S) as Promise<unknown>;
    } else {
      new Function("S", `with (S) { ${body} }`)(S);
    }
  }
  for (const res of resolvers) res(() => {});
  await mountP;

  const starsKey = fns.onStarsKey as (e: KeyEvt) => void;
  function owner(fn: (e: KeyEvt) => void): string {
    return fn === starsKey ? "stars" : "ring";
  }
  function registration() {
    return listeners
      .filter((l) => l.type === "keydown")
      .map((l) => ({ owner: owner(l.fn), capture: l.capture }));
  }
  function dispatch(key: string): string[] {
    let stopped = false;
    const event: KeyEvt = {
      key,
      shiftKey: false,
      metaKey: false,
      ctrlKey: false,
      altKey: false,
      target: {},
      defaultPrevented: false,
      preventDefault() {
        this.defaultPrevented = true;
      },
      stopImmediatePropagation() {
        stopped = true;
      },
    };
    const caps = listeners.filter((l) => l.type === "keydown" && l.capture);
    const bub = listeners.filter((l) => l.type === "keydown" && !l.capture);
    const trace: string[] = [];
    for (const group of [caps, bub]) {
      for (const l of group) {
        trace.push(`${owner(l.fn)}:${l.capture ? "capture" : "bubble"}`);
        l.fn(event);
        if (stopped) return trace;
      }
    }
    return trace;
  }
  return {
    S,
    log,
    averages,
    exits: () => exits,
    closes: () => closes,
    registration,
    dispatch,
  };
}

test("FR2 standalone map owns arrows Return and Esc; embedded ring still exits", async () => {
  const stand = await bootStarsKeys({ embedded: false, ringActive: false });
  assert.deepEqual(stand.registration(), [{ owner: "stars", capture: false }], "standalone registers only the bubble map");
  assert.deepEqual(stand.dispatch("a"), ["stars:bubble"], "standalone propagation is the bubble listener");
  assert.equal((stand.S.focusPos as { index: number }).index, 0);
  stand.dispatch("ArrowDown");
  stand.dispatch("ArrowDown");
  assert.equal((stand.S.focusPos as { zone: string; index: number }).zone, "Z1");
  assert.equal((stand.S.focusPos as { index: number }).index, 2, "Down walks the zone");
  assert.equal(stand.S.navView, "day", "Down does not cycle the stars-nav view");
  assert.equal(stand.averages.length, 0);
  stand.dispatch("ArrowUp");
  stand.dispatch("ArrowUp");
  stand.dispatch("ArrowUp");
  assert.equal((stand.S.focusPos as { zone: string }).zone, "Z0", "Up leaves the zone for Z0");
  assert.equal(stand.S.navView, "day");
  stand.S.focusPos = focusOrigin("Z1");
  stand.dispatch("ArrowRight");
  assert.equal((stand.S.focusPos as { zone: string }).zone, "Z2", "Right moves to the next zone");
  assert.equal(stand.S.navView, "day");
  stand.dispatch("Enter");
  assert.equal((stand.S.focusPos as { zone: string; depth: number }).zone, "Z2");
  assert.equal((stand.S.focusPos as { depth: number }).depth, 1, "Return enters the zone");
  assert.equal(stand.S.navView, "day", "Return does not cycle the stars-nav view");
  stand.dispatch("Escape");
  assert.equal((stand.S.focusPos as { zone: string; depth: number }).depth, 0, "Esc leaves the spoke");
  assert.equal((stand.S.focusPos as { zone: string }).zone, "Z2");
  assert.equal(stand.log.includes("hide_window"), false);
  stand.dispatch("Escape");
  assert.equal((stand.S.focusPos as { zone: string }).zone, "Z0");
  assert.equal(stand.log.includes("hide_window"), false, "Esc at Z2 does not hide");
  stand.S.focusPos = { ...focusOrigin("Z4"), depth: 1 };
  stand.S.reflectOpen = true;
  stand.dispatch("Escape");
  assert.equal(stand.S.reflectOpen, false, "an open Reflect sheet closes first");
  assert.equal((stand.S.focusPos as { depth: number }).depth, 1, "the live control stays until the sheet is gone");
  assert.equal(stand.log.includes("hide_window"), false);
  for (const zone of ["Z1", "Z2", "Z3", "Z4", "Z5", "Z6", "Z7"] as const) {
    stand.S.focusPos = focusOrigin(zone);
    stand.S.reflectOpen = false;
    stand.S.spokeLive = false;
    stand.dispatch("Escape");
    assert.equal((stand.S.focusPos as { zone: string }).zone, "Z0", `${zone} steps out one level`);
    assert.equal(stand.log.includes("hide_window"), false, `${zone} does not hide`);
  }
  stand.S.focusPos = focusOrigin("Z0");
  stand.dispatch("Escape");
  assert.equal(stand.log.at(-1), "hide_window", "hide only at Z0 depth 0");
  assert.equal(stand.closes(), 0);

  const forced = await bootStarsKeys({ embedded: false, ringActive: true });
  assert.deepEqual(forced.registration(), [{ owner: "stars", capture: false }], "ringActive alone does not attach the standalone ring");
  forced.dispatch("ArrowDown");
  forced.dispatch("ArrowDown");
  assert.equal(forced.S.navView, "day", "forced ringActive still does not cycle the view");
  assert.equal((forced.S.focusPos as { index: number }).index, 2);
  forced.S.focusPos = focusOrigin("Z5");
  forced.dispatch("Escape");
  assert.equal((forced.S.focusPos as { zone: string }).zone, "Z0");
  assert.equal(forced.log.includes("hide_window"), false);

  const host = await bootStarsKeys({ embedded: true, ringActive: true });
  assert.deepEqual(
    host.registration(),
    [
      { owner: "ring", capture: true },
      { owner: "stars", capture: false },
    ],
    "registration order: capture ring, then bubble map",
  );
  assert.deepEqual(host.dispatch("a"), ["ring:capture", "stars:bubble"], "propagation runs capture before bubble");
  const before = host.S.focusPos;
  assert.deepEqual(host.dispatch("ArrowLeft"), ["ring:capture"], "the ring eats the outward arrow");
  assert.equal(host.exits(), 1, "outer edge calls onRingExit");
  assert.equal(host.S.focusPos, before, "the bubble map does not run");
  assert.equal(host.closes(), 0);
  assert.equal(host.log.includes("hide_window"), false);
  host.S.reflectOpen = true;
  assert.deepEqual(host.dispatch("Escape"), ["ring:capture"]);
  assert.equal(host.S.reflectOpen, false, "embedded Reflect still closes first");
  assert.equal(host.exits(), 1, "Reflect does not walk out");
  host.S.spokeLive = true;
  assert.deepEqual(host.dispatch("Escape"), ["ring:capture"]);
  assert.equal(host.S.spokeLive, false, "a live spoke still exits before walk-out");
  assert.equal(host.exits(), 1);
  assert.deepEqual(host.dispatch("Escape"), ["ring:capture"]);
  assert.equal(host.exits(), 2, "embedded Esc at rest calls onRingExit");
  assert.equal(host.closes(), 0);
  assert.equal(host.log.includes("hide_window"), false);
  assert.equal(host.S.focusPos, before);
});

// FR3: real onStarsKey / add-form handlers against a DOM built from the
// component markup. Svelte's scope class is one extra class on a non-:global rule.
class DomEl {
  tagName: string;
  parent: DomEl | null = null;
  children: DomEl[] = [];
  attrs: Record<string, string> = {};
  classes = new Set<string>();
  classList: { contains: (c: string) => boolean; add: (c: string) => void };
  listeners: Record<string, Array<(e: Record<string, unknown>) => void>> = {};
  value = "";
  options: { value: string; text: string }[] = [];
  selectedIndex = 0;
  text = "";
  doc: { body: DomEl; activeElement: DomEl } | null = null;
  constructor(tag: string) {
    this.tagName = tag.toUpperCase();
    this.classList = {
      contains: (c) => this.classes.has(c),
      add: (c) => this.classes.add(c),
    };
  }
  setAttribute(name: string, value: string) {
    this.attrs[name] = value;
    if (name === "class") {
      this.classes = new Set(value.split(/\s+/).filter(Boolean));
    }
  }
  getAttribute(name: string) {
    return this.attrs[name] ?? null;
  }
  hasAttribute(name: string) {
    return Object.prototype.hasOwnProperty.call(this.attrs, name);
  }
  removeAttribute(name: string) {
    delete this.attrs[name];
  }
  append(child: DomEl) {
    child.parent = this;
    this.children.push(child);
  }
  closest(sel: string): DomEl | null {
    let node: DomEl | null = this;
    while (node) {
      if (node.matches(sel)) return node;
      node = node.parent;
    }
    return null;
  }
  matches(sel: string) {
    return sel.split(",").some((part) => matchCompound(this, part.trim()));
  }
  querySelector(sel: string) {
    return this.querySelectorAll(sel)[0] ?? null;
  }
  querySelectorAll(sel: string): DomEl[] {
    const out: DomEl[] = [];
    const walk = (node: DomEl) => {
      for (const child of node.children) {
        if (child.matches(sel)) out.push(child);
        walk(child);
      }
    };
    walk(this);
    return out;
  }
  getBoundingClientRect() {
    return { left: 0, top: 0, width: 0, height: 0, right: 0, bottom: 0 };
  }
  focus() {
    if (this.doc) this.doc.activeElement = this;
  }
  click() {
    for (const fn of this.listeners.click ?? []) fn({ currentTarget: this, target: this });
  }
}

function matchCompound(el: DomEl, sel: string) {
  if (!sel) return false;
  let i = 0;
  let saw = false;
  while (i < sel.length) {
    if (sel[i] === " ") {
      i += 1;
      continue;
    }
    saw = true;
    if (sel[i] === ".") {
      let j = i + 1;
      while (j < sel.length && /[\w-]/.test(sel[j])) j += 1;
      if (!el.classes.has(sel.slice(i + 1, j))) return false;
      i = j;
      continue;
    }
    if (sel[i] === "[") {
      const end = sel.indexOf("]", i);
      const body = sel.slice(i + 1, end);
      const eq = body.indexOf("=");
      if (eq < 0) {
        if (!el.hasAttribute(body)) return false;
      } else {
        const name = body.slice(0, eq);
        let raw = body.slice(eq + 1).trim();
        if ((raw.startsWith('"') && raw.endsWith('"')) || (raw.startsWith("'") && raw.endsWith("'"))) raw = raw.slice(1, -1);
        if (el.getAttribute(name) !== raw) return false;
      }
      i = end + 1;
      continue;
    }
    if (/[A-Za-z*]/.test(sel[i])) {
      let j = i + 1;
      while (j < sel.length && /[\w-]/.test(sel[j])) j += 1;
      const tag = sel.slice(i, j);
      if (tag !== "*" && el.tagName !== tag.toUpperCase()) return false;
      i = j;
      continue;
    }
    return false;
  }
  return saw;
}

type DomDoc = { body: DomEl; activeElement: DomEl };

function adopt(el: DomEl, doc: DomDoc) {
  el.doc = doc;
  return el;
}

function attrsFrom(raw: string): Record<string, string> {
  const attrs: Record<string, string> = {};
  const re = /([:@A-Za-z_][\w:.-]*)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|(\{[^}]*\})))?/g;
  for (const m of raw.matchAll(re)) {
    attrs[m[1]] = m[2] ?? m[3] ?? m[4] ?? "";
  }
  return attrs;
}

function tagNodes(chunk: string, doc: DomDoc): DomEl[] {
  const nodes: DomEl[] = [];
  const re = /<([A-Za-z][\w-]*)([^>]*?)>/g;
  for (const m of chunk.matchAll(re)) {
    const tag = m[1] ?? "";
    if (tag[0] !== tag[0].toLowerCase()) continue;
    if (tag === "option") continue;
    const node = adopt(new DomEl(tag), doc);
    const attrs = attrsFrom(m[2] ?? "");
    for (const [name, value] of Object.entries(attrs)) node.setAttribute(name, value);
    const rest = chunk.slice((m.index ?? 0) + m[0].length);
    const cut = rest.indexOf("<");
    node.text = (cut < 0 ? rest : rest.slice(0, cut)).replace(/\s+/g, " ").trim();
    nodes.push(node);
  }
  return nodes;
}

function specOf(selector: string): [number, number, number] {
  const srcSel = selector.trim().startsWith(":global(")
    ? selector.trim().replace(/^:global\(/, "").replace(/\)\s*$/, "")
    : selector.trim();
  let ids = 0;
  let cls = 0;
  let elem = 0;
  let i = 0;
  while (i < srcSel.length) {
    if (srcSel.startsWith(":not(", i)) {
      let depth = 0;
      let j = i + 4;
      for (; j < srcSel.length; j += 1) {
        if (srcSel[j] === "(") depth += 1;
        else if (srcSel[j] === ")") {
          depth -= 1;
          if (depth === 0) break;
        }
      }
      const inner = specOf(srcSel.slice(i + 5, j));
      ids += inner[0];
      cls += inner[1];
      elem += inner[2];
      i = j + 1;
      continue;
    }
    if (srcSel.startsWith(":root", i)) {
      cls += 1;
      i += 5;
      continue;
    }
    if (srcSel[i] === "#") {
      ids += 1;
      i += 1;
      while (i < srcSel.length && /[\w-]/.test(srcSel[i])) i += 1;
      continue;
    }
    if (srcSel[i] === ".") {
      cls += 1;
      i += 1;
      while (i < srcSel.length && /[\w-]/.test(srcSel[i])) i += 1;
      continue;
    }
    if (srcSel[i] === "[") {
      cls += 1;
      i = srcSel.indexOf("]", i) + 1;
      continue;
    }
    if (/[a-zA-Z*]/.test(srcSel[i] ?? "")) {
      if (srcSel[i] !== "*") elem += 1;
      i += 1;
      while (i < srcSel.length && /[\w-]/.test(srcSel[i])) i += 1;
      continue;
    }
    i += 1;
  }
  return [ids, cls, elem];
}

function beats(a: [number, number, number], b: [number, number, number]) {
  for (let i = 0; i < 3; i += 1) if (a[i] !== b[i]) return a[i] > b[i];
  return false;
}

function sliceBetween(text: string, start: string, end: string) {
  const i = text.indexOf(start);
  assert.ok(i >= 0, start);
  const j = text.indexOf(end, i + start.length);
  assert.ok(j > i, end);
  return text.slice(i + start.length, j);
}

function loadBodies(file: string, heads: string[]) {
  const text = file.endsWith(".svelte") ? pageScriptFrom(file) : src(file);
  return stripTypeScriptTypes(
    heads
      .map((head) => {
        const at = text.indexOf(head);
        assert.ok(at >= 0, `${file} ${head}`);
        let paren = text.indexOf("(", at);
        let depth = 0;
        for (; paren < text.length; paren += 1) {
          if (text[paren] === "(") depth += 1;
          else if (text[paren] === ")") {
            depth -= 1;
            if (depth === 0) break;
          }
        }
        return text.slice(at, paren + 1) + " " + braceBody(text, paren) + "\n";
      })
      .join("\n"),
  );
}

function pageScriptFrom(rel: string) {
  const file = src(rel);
  return file.slice(file.indexOf("<script"), file.indexOf("</script>"));
}

function bootAddForm() {
  const added: { id: string; name: string; kind: string }[] = [];
  const doc: DomDoc = { body: new DomEl("body"), activeElement: new DomEl("body") };
  adopt(doc.body, doc);
  doc.activeElement = doc.body;
  const main = adopt(new DomEl("div"), doc);
  main.classList.add("stars");
  main.setAttribute("class", "stars");
  doc.body.append(main);
  const projects = [
    { id: "p1", name: "Alpha" },
    { id: "p2", name: "Beta" },
  ];
  const S: Record<string, unknown> = {
    Element: DomEl,
    document: doc,
    mainEl: main,
    layout: "three",
    segment: "checkin",
    embedded: false,
    reflectOpen: false,
    spokeLive: false,
    focusCol: "middle",
    outwardDir: "left",
    focusPos: { ...focusOrigin("Z4"), index: 0, depth: 0, ctl: 0 },
    starsFieldGate,
    focusStep,
    columnNext,
    segmentNext,
    zoneColumn,
    closeAddForm: null,
    adding: false,
    label: "",
    kind: "number",
    planetId: "p1",
    scope: "overall",
    projects,
    onAdd: (id: string, name: string, kind: string) => added.push({ id, name, kind }),
    onRingExit: () => {},
    focusStarsCol: () => {},
    hideStars: () => {},
    tick: () => {
      render();
      return Promise.resolve();
    },
  };

  const pageCode = loadBodies("src/lib/StarsPage.svelte", [
    "function focusWorld(",
    "function focusElement(",
    "function applyStarsFocus(",
    "function activateStarsFocus(",
    "function applyStep(",
    "function fieldKind(",
    "function onRegisterAdd(",
    "async function landAddForm(",
    "async function onAddFocus(",
    "function onStarsKey(",
  ]);
  const trackerCode = loadBodies("src/lib/stars/StarsTrackers.svelte", [
    "function openAdd(",
    "function closeAdd(",
    "function commitAdd(",
    "function onAddFormKey(",
    "function publishAddForm(",
  ]);
  const fns = new Function(
    "S",
    `with (S) {\n${pageCode}\n${trackerCode}\nreturn { focusWorld, focusElement, applyStarsFocus, activateStarsFocus, applyStep, fieldKind, onRegisterAdd, landAddForm, onAddFocus, onStarsKey, openAdd, closeAdd, commitAdd, onAddFormKey, publishAddForm };\n}`,
  )(S) as Record<string, (...a: unknown[]) => unknown>;
  Object.assign(S, fns);
  S.onRegister = fns.onRegisterAdd;
  (fns.publishAddForm as () => void)();

  const trackers = src("src/lib/stars/StarsTrackers.svelte");
  const formChunk = sliceBetween(trackers, "{#if adding}", "{:else}");
  const elseChunk = sliceBetween(trackers, "{:else}", "{/if}");

  function wire(node: DomEl) {
    const click = node.getAttribute("onclick") ?? "";
    if (click.includes("openAdd")) node.listeners.click = [() => (fns.openAdd as () => void)()];
    if (click.includes("commitAdd")) node.listeners.click = [() => (fns.commitAdd as () => boolean)()];
    if (node.tagName === "SELECT" && (node.getAttribute("aria-label") ?? "").includes("Planet")) {
      node.options = projects.map((p) => ({ value: p.id, text: p.name }));
      node.selectedIndex = Math.max(0, node.options.findIndex((o) => o.value === S.planetId));
      node.value = node.options[node.selectedIndex]?.value ?? "";
    }
    if (node.tagName === "SELECT" && (node.getAttribute("aria-label") ?? "").includes("kind")) {
      node.options = [
        { value: "number", text: "Number" },
        { value: "timer", text: "Timer" },
      ];
      node.selectedIndex = node.options.findIndex((o) => o.value === S.kind);
      if (node.selectedIndex < 0) node.selectedIndex = 0;
      node.value = node.options[node.selectedIndex]?.value ?? "";
    }
  }

  function render() {
    const prev = doc.activeElement;
    main.children = [];
    if (!S.adding) {
      const btn = tagNodes(elseChunk, doc).find((n) => n.tagName === "BUTTON");
      assert.ok(btn, "add button markup");
      btn.text = "+ Add";
      wire(btn);
      main.append(btn);
    } else {
      const nodes = tagNodes(formChunk, doc);
      const form = nodes[0];
      assert.ok(form && form.tagName === "DIV", "add form markup");
      for (const child of nodes.slice(1)) {
        wire(child);
        form.append(child);
      }
      main.append(form);
    }
    if (!prev || !contains(main, prev)) doc.activeElement = doc.body;
  }

  function controls(): DomEl[] {
    return [...main.querySelectorAll("[data-focus-ctl]")];
  }

  async function press(node: DomEl, key: string) {
    const event = {
      key,
      shiftKey: false,
      metaKey: false,
      ctrlKey: false,
      altKey: false,
      target: node,
      defaultPrevented: false,
      stopped: false,
      preventDefault() {
        this.defaultPrevented = true;
      },
      stopPropagation() {
        this.stopped = true;
      },
    };
    const form = node.closest("[data-add-form]");
    const wired =
      (node.getAttribute("onkeydown") ?? "").includes("onAddFormKey") ||
      (form?.getAttribute("onkeydown") ?? "").includes("onAddFormKey");
    if (wired) (fns.onAddFormKey as (e: typeof event) => void)(event);
    if (!event.stopped) (fns.onStarsKey as (e: typeof event) => void)(event);
    if (!event.defaultPrevented && node.tagName === "SELECT" && key.startsWith("Arrow") && node.options.length) {
      const dir = key === "ArrowUp" || key === "ArrowLeft" ? -1 : 1;
      node.selectedIndex = Math.max(0, Math.min(node.options.length - 1, node.selectedIndex + dir));
      node.value = node.options[node.selectedIndex]?.value ?? node.value;
    }
    await Promise.resolve();
    await Promise.resolve();
    return event;
  }

  render();
  return { S, fns, main, doc, added, render, controls, press, formChunk };
}

function contains(root: DomEl, node: DomEl) {
  let cur: DomEl | null = node;
  while (cur) {
    if (cur === root) return true;
    cur = cur.parent;
  }
  return false;
}

function pos(S: Record<string, unknown>) {
  return S.focusPos as { zone: string; depth: number; ctl: number; index: number };
}

test("FR3 Return on + Add focuses the first field", async () => {
  const boot = bootAddForm();
  const add = boot.main.querySelector("button.add") as DomEl;
  assert.ok(add, "+ Add is in the markup");
  add.focus();
  boot.S.focusPos = { ...focusOrigin("Z4"), index: 0, depth: 0, ctl: 0 };
  await boot.press(add, "Enter");
  const active = boot.doc.activeElement;
  assert.notEqual(active, boot.doc.body, "focus never lands on BODY");
  assert.equal(active.tagName, "SELECT");
  assert.match(active.getAttribute("aria-label") ?? "", /Planet/);
  assert.equal(pos(boot.S).depth, 1);
  assert.equal(pos(boot.S).ctl, 0);
  assert.equal(boot.S.adding, true);
});

test("FR3 Esc from each add-form field closes and restores + Add", async () => {
  const labels = ["Planet for this tracker", "New tracker name", "Tracker kind", "Add"];
  for (const label of labels) {
    const boot = bootAddForm();
    boot.S.adding = true;
    boot.render();
    const fields = boot.controls();
    assert.equal(fields.length, 4, "planet, name, kind, Add");
    const field = fields.find((el) => (el.getAttribute("aria-label") ?? "") === label || el.text === label);
    assert.ok(field, label);
    const ctl = fields.indexOf(field);
    field.focus();
    boot.S.focusPos = { ...focusOrigin("Z4"), index: 0, depth: 1, ctl };
    await boot.press(field, "Escape");
    assert.equal(boot.S.adding, false, `${label} Esc closes the form`);
    const active = boot.doc.activeElement;
    assert.notEqual(active, boot.doc.body, `${label} Esc does not leave BODY`);
    assert.equal(active.tagName, "BUTTON");
    assert.equal(active.classList.contains("add"), true, `${label} Esc returns to + Add`);
    assert.equal(active.text, "+ Add");
    assert.equal(pos(boot.S).depth, 0);
  }
});

test("FR3 planet select arrows change the value and are not prevented", async () => {
  const boot = bootAddForm();
  boot.S.adding = true;
  boot.render();
  const planet = boot.controls()[0];
  assert.equal(planet.tagName, "SELECT");
  planet.focus();
  boot.S.focusPos = { ...focusOrigin("Z4"), depth: 1, ctl: 0 };
  const down = await boot.press(planet, "ArrowDown");
  assert.equal(down.defaultPrevented, false, "the guard does not prevent the select arrow");
  assert.equal(planet.value, "p2", "ArrowDown changes the planet");
  assert.equal(boot.doc.activeElement, planet, "Up/Down stay on the select");
  const right = await boot.press(planet, "ArrowRight");
  assert.equal(right.defaultPrevented, false, "Left/Right are not prevented on the select");
  assert.notEqual(boot.doc.activeElement, boot.doc.body);
  assert.equal(boot.doc.activeElement.tagName, "INPUT", "ArrowRight still reaches the name");
});

test("FR3 Tab and arrows reach planet, name, kind and Add", async () => {
  const boot = bootAddForm();
  boot.S.adding = true;
  boot.render();
  const [planet, name, kind, add] = boot.controls();
  planet.focus();
  boot.S.focusPos = { ...focusOrigin("Z4"), depth: 1, ctl: 0 };
  await boot.press(planet, "Tab");
  assert.equal(boot.doc.activeElement, name, "Tab reaches the name");
  await boot.press(name, "Tab");
  assert.equal(boot.doc.activeElement, kind, "Tab reaches the kind");
  await boot.press(kind, "Tab");
  assert.equal(boot.doc.activeElement, add, "Tab reaches Add");
  assert.notEqual(boot.doc.activeElement, boot.doc.body);

  boot.S.focusPos = { ...focusOrigin("Z4"), depth: 1, ctl: 0 };
  planet.focus();
  await boot.press(planet, "ArrowRight");
  assert.equal(boot.doc.activeElement, name, "ArrowRight reaches the name");
  await boot.press(name, "ArrowRight");
  assert.equal(boot.doc.activeElement, kind, "ArrowRight reaches the kind");
  await boot.press(kind, "ArrowRight");
  assert.equal(boot.doc.activeElement, add, "ArrowRight reaches Add");
  await boot.press(add, "ArrowLeft");
  assert.equal(boot.doc.activeElement, kind, "ArrowLeft reaches back to kind");
  assert.notEqual(boot.doc.activeElement, boot.doc.body);

  const outside = adopt(new DomEl("input"), boot.doc);
  outside.setAttribute("type", "text");
  boot.main.append(outside);
  outside.focus();
  const held = { ...focusOrigin("Z3"), depth: 1 as const, ctl: 0 };
  boot.S.focusPos = held;
  const eaten = await boot.press(outside, "ArrowRight");
  assert.equal(eaten.defaultPrevented, false, "a text field outside the add form keeps its arrow");
  assert.equal(pos(boot.S).zone, "Z3");
  assert.equal(pos(boot.S).depth, 1);
});

test("FR3 Enter in the name submits or moves to Add and never leaves focus on BODY", async () => {
  const empty = bootAddForm();
  empty.S.adding = true;
  empty.S.label = " ";
  empty.render();
  const name = empty.controls()[1];
  name.focus();
  empty.S.focusPos = { ...focusOrigin("Z4"), depth: 1, ctl: 1 };
  await empty.press(name, "Enter");
  assert.equal(empty.S.adding, true, "an empty name does not submit");
  assert.equal(empty.added.length, 0);
  assert.notEqual(empty.doc.activeElement, empty.doc.body);
  assert.equal(empty.doc.activeElement.tagName, "BUTTON");
  assert.equal(empty.doc.activeElement.text, "Add");
  assert.equal(pos(empty.S).depth, 1);

  const filled = bootAddForm();
  filled.S.adding = true;
  filled.S.label = "Steps";
  filled.render();
  const named = filled.controls()[1];
  named.focus();
  filled.S.focusPos = { ...focusOrigin("Z4"), depth: 1, ctl: 1 };
  await filled.press(named, "Enter");
  assert.equal(filled.added.length, 1);
  assert.equal(filled.added[0]?.name, "Steps");
  assert.equal(filled.S.adding, false, "Enter submits and closes");
  assert.notEqual(filled.doc.activeElement, filled.doc.body, "submit does not leave focus on BODY");
  assert.equal(filled.doc.activeElement.tagName, "BUTTON");
  assert.equal(filled.doc.activeElement.text, "+ Add");
});

test("FR3 focus ring specificity beats the hum and selected rings with no new colour", () => {
  const pageCss = styleOf("src/lib/StarsPage.svelte");
  const rules = [...pageCss.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .map((m) => ({ sel: (m[1] ?? "").trim(), body: (m[2] ?? "").replace(/\s+/g, " ").trim() }))
    .filter((rule) => rule.sel.includes("data-stars-focus"));
  assert.equal(rules.length, 2);
  const base = rules.find((rule) => !rule.sel.includes("terminal"));
  const term = rules.find((rule) => rule.sel.includes("terminal"));
  assert.ok(base && term);
  const humCss = styleOf("src/lib/stars/TrackerCard.svelte");
  const cellCss = styleOf("src/lib/stars/AlmanacFolio.svelte");
  const humAt = humCss.indexOf(".round.hum {");
  const cellAt = cellCss.indexOf(".cell.selected {");
  assert.ok(humAt >= 0 && cellAt >= 0);
  const humSel = humCss.slice(humCss.lastIndexOf("\n", humAt) + 1, humCss.indexOf("{", humAt)).trim();
  const cellSel = cellCss.slice(cellCss.lastIndexOf("\n", cellAt) + 1, cellCss.indexOf("{", cellAt)).trim();
  const hum = specOf(humSel);
  const cell = specOf(cellSel);
  hum[1] += 1;
  cell[1] += 1;
  const baseSpec = specOf(base.sel);
  const termSpec = specOf(term.sel);
  assert.deepEqual(hum, [0, 3, 0]);
  assert.deepEqual(cell, [0, 3, 0]);
  assert.ok(beats(baseSpec, hum), `base ${baseSpec} beats .round.hum ${hum}`);
  assert.ok(beats(baseSpec, cell), `base ${baseSpec} beats .cell.selected ${cell}`);
  assert.ok(beats(termSpec, hum), `terminal ${termSpec} beats .round.hum`);
  assert.ok(beats(termSpec, cell), `terminal ${termSpec} beats .cell.selected`);
  assert.ok(baseSpec[0] >= 1 && termSpec[0] >= 1, "both rules carry the id-level bump");
  assert.equal(base.body, "outline: none; box-shadow: var(--focus-ring);");
  assert.equal(term.body, "box-shadow: none; outline: 1px solid var(--ink); background: transparent;");
  for (const body of [base.body, term.body]) {
    assert.equal(/#[0-9a-fA-F]{3,8}|rgba?\(|hsla?\(|\bwhite\b/i.test(body), false, body);
  }
});
