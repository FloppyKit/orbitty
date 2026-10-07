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
  assert.equal(root.rootEsc, "hide");
  const sheet = focusStep(at("Z2"), "Escape", world({ reflectOpen: true }));
  assert.equal(sheet.rootEsc, "reflect");
  const z0 = focusStep(at("Z0", 0), "Escape", w);
  assert.equal(z0.pos.zone, "Z2");
  const z1 = focusStep(at("Z1", 2), "Escape", w);
  assert.equal(z1.pos.zone, "Z2");
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
