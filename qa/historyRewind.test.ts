import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

test("the rail adopts disk planets on state:changed unless a local edit waits", () => {
  const file = src("src/lib/RailPage.svelte");
  const start = file.indexOf('listen("state:changed"');
  assert.notEqual(start, -1);
  const body = file.slice(start, file.indexOf("\n    });", start));
  const drag = body.indexOf("if (windowDragHot())");
  const before = body.indexOf("const projectsBefore = projects;");
  const load = body.indexOf('await invoke<PersistedState>("load_state")', before);
  const adopt = body.indexOf(
    "if (!persistDeferred && projects === projectsBefore) projects = s.projects ?? projects;",
  );
  assert.ok(drag !== -1 && before > drag, "not on the drag-hot path");
  assert.ok(load > before && adopt > load, "captured before the await, adopted after");
});

import { readdirSync, statSync } from "node:fs";
import {
  alsoRemove,
  driftHeadline,
  eventWords,
  restoreConfirm,
  rewindConfirm,
  rowWords,
  shown,
  when,
  who,
  type Drift,
  type HistoryEvent,
  type Plan,
} from "../src/lib/history/words.ts";

const named = (n: number) => Array.from({ length: n }, (_, i) => ({ id: `c-${i}`, title: `Card ${i}` }));

function plan(over: Partial<Plan>): Plan {
  return {
    at: 0,
    cards: [],
    revive: [],
    added: [],
    removeAdded: false,
    planets: [],
    planetsRevive: [],
    planetsAdded: [],
    removePlanets: false,
    skipped: [],
    ...over,
  };
}

function ev(over: Partial<HistoryEvent>): HistoryEvent {
  return { id: "x", what: "card", op: "set", target: "c-1", title: "X", changes: [], ...over };
}

test("the confirm strings", () => {
  const p = plan({
    cards: named(14).map((n) => ({ ...n, fields: [] })),
    revive: named(2),
    added: named(3),
  });
  assert.equal(
    rewindConfirm("5:21 PM", p),
    "Rewind the board to 5:21 PM? 14 cards change. 2 deleted cards come back. 3 cards added since then stay.",
  );
  assert.equal(alsoRemove(3), "Also remove those 3 cards");
  const fields = ["title", "notes", "status", "links"].map((field) => ({ field, from: null, to: null }));
  assert.equal(
    restoreConfirm("5:21 PM", plan({ cards: [{ id: "c-1", title: "X", fields }] })),
    "Restore this card to how it was at 5:21 PM? Title, notes and 2 more fields change.",
  );
});

test("times and who read in plain words", () => {
  const now = new Date(2026, 9, 13, 18, 0);
  assert.equal(when(new Date(2026, 9, 13, 17, 21).getTime() / 1000, now), "5:21 PM");
  assert.equal(when(new Date(2026, 9, 12, 9, 4).getTime() / 1000, now), "Yesterday 9:04 AM");
  assert.equal(when(new Date(2026, 9, 3, 0, 5).getTime() / 1000, now), "Oct 3 12:05 AM");
  const names = ["user", "ctl", "agent", "rule", "app", "rewind", "repair"].map(who);
  assert.deepEqual(names, ["You", "orbittyctl", "Agent", "Rule", "Orbitty", "Rewind", "Repair"]);
});

test("rows read in plain words and one save folds", () => {
  const ch = (field: string, from: unknown, to: unknown) => [{ field, from, to }];
  assert.equal(eventWords(ev({ changes: ch("title", "Old", "New") })), 'Renamed "Old" to "New"');
  assert.equal(eventWords(ev({ changes: ch("status", "todo", "doing") })), 'Moved "X" to Doing');
  assert.equal(eventWords(ev({ changes: ch("notes", "a", "b") })), 'Changed notes on "X"');
  assert.equal(eventWords(ev({ changes: ch("tagIds", ["t-1"], ["t-1", "t-2"]) })), 'Added a tag to "X"');
  assert.equal(eventWords(ev({ changes: ch("links", null, []) })), 'Links changed on "X"');
  assert.equal(eventWords(ev({ what: "deleted" })), 'Deleted "X"');
  assert.equal(
    eventWords(ev({ what: "planet", title: "B", changes: ch("name", "A", "B") })),
    'Renamed planet "A" to "B"',
  );
  assert.equal(eventWords(ev({ what: "checkpoint" })), "Checkpoint");
  const links = Array.from({ length: 46 }, (_, i) => ev({ id: `l-${i}`, changes: ch("links", [], null) }));
  assert.equal(rowWords({ id: "l-45", at: 0, actor: "app", events: links }), "Links changed on 46 cards");
  assert.equal(shown("links", [{ url: "https://example.com/a", label: "doc" }]), "doc");
  assert.equal(shown("notes", "two\nlines"), "two lines");
});

test("the drift headline matches Drift::headline", () => {
  const d: Drift = {
    cards: named(12).map((n) => ({ ...n, fields: [] })),
    missing: [],
    unknown: [],
    planets: [],
    planetsMissing: [],
    planetsUnknown: [],
    missedWrite: false,
  };
  assert.equal(driftHeadline(d), "12 cards differ from the saved history");
  const rust = src("src-tauri/src/relay/rewind.rs");
  assert.ok(rust.includes('format!("{} {verb} from the saved history"'));
});

test("Settings has a History tab with the drift review, list and undo", () => {
  const page = src("src/lib/SettingsPage.svelte");
  assert.match(page, /onclick=\{\(\) => \(section = "history"\)\}>History<\/button/);
  assert.ok(page.includes('{:else if section === "history"}\n      <SettingsHistory />'));
  const tab = src("src/lib/SettingsHistory.svelte");
  for (const s of [
    "Board matches its history.",
    ">Check now<",
    ">Review<",
    ">Repair from history<",
    ">Keep the board as it is<",
    ">Rewind board to here<",
    ">Bring back<",
    ">Undo last rewind<",
    ">Show more<",
    ">Cancel<",
    '"Bring back" : "Rewind"',
    "now: {shown(f.field, f.now)} · history: {shown(f.field, f.history)}",
    "const STEP = 50;",
    "{MISSED_WRITE}",
    "{alsoRemove(a.plan.added.length)}",
    "{rewindConfirm(when(a.at), a.plan)}",
  ]) {
    assert.ok(tab.includes(s), s);
  }
  assert.equal(/\bconfirm\(|plugin-dialog|\bask\(/.test(tab), false, "the confirm is in the page");
});

test("the card preview has a History button beside Edit", () => {
  const p = src("src/lib/board/Preview.svelte");
  const hist = p.indexOf('onclick={() => toggleHistory(card)}>History</button>');
  const edit = p.indexOf('onclick={() => (previewEdit = true)}>Edit</button>');
  assert.ok(hist !== -1 && edit > hist && edit - hist < 160, "History sits right before Edit");
  for (const s of [">Restore to this<", ">Restore<", ">Cancel<", "{restoreConfirm(when(histAsk.at), histAsk.plan)}"]) {
    assert.ok(p.includes(s), s);
  }
  assert.ok(p.includes('invoke<HistoryPage>("history_list", { card: card.id, limit: 50 })'));
});

test("the seven history commands are registered", () => {
  const lib = src("src-tauri/src/lib.rs");
  const list = lib.slice(lib.indexOf("generate_handler!["), lib.indexOf("]))", lib.indexOf("generate_handler![")));
  for (const c of ["list", "drift", "plan", "apply", "repair", "keep", "undo"]) {
    assert.ok(list.includes(`relay::rewind::history_${c},`), c);
  }
});

function svelteFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = `${dir}/${name}`;
    if (statSync(p).isDirectory()) return svelteFiles(p);
    return name.endsWith(".svelte") ? [p] : [];
  });
}

test("no .svelte file shows a kind number or a line id", () => {
  const root = new URL("../src", import.meta.url).pathname;
  for (const f of svelteFiles(root)) {
    const text = readFileSync(f, "utf8");
    assert.equal(/\b42[3-6]\b/.test(text), false, `${f} names a history kind`);
  }
  for (const rel of ["src/lib/SettingsHistory.svelte", "src/lib/board/Preview.svelte"]) {
    const file = src(rel);
    const markup = file
      .replace(/<script[\s\S]*?<\/script>/g, "")
      .replace(/<style[\s\S]*?<\/style>/g, "");
    const shownExpr = [...markup.matchAll(/(^|[^=$])\{([^#/:@][^{}]*)\}/g)].map((m) => m[2].trim());
    const ids = shownExpr.filter((e) => /\.(id|to|row|target|kind)$/.test(e));
    assert.deepEqual(ids, [], rel);
  }
});
