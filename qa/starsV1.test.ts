import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";
import {
  starsColumns,
  starsLayout,
  starsScopeTargets,
} from "../src/lib/stars/starsLayout.ts";
import {
  HANDLE_R,
  STARS_SPOKES,
  handleHitRadius,
  parseRating,
  radarDraft,
  ratingFromKey,
  snapOnSpoke,
  RADAR_R,
} from "../src/lib/stars/radarGeom.ts";
import {
  folioFace,
  pageCovers,
  reflectTarget,
  stripCells,
  type FolioPage,
} from "../src/lib/stars/folioDay.ts";
import { buildDocs, docsInRange, type SkyDoc } from "../src/lib/skyBag.ts";

const root = new URL("..", import.meta.url);
function src(rel: string): string {
  return readFileSync(new URL(rel, root), "utf8");
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

test("S1 starsLayout boundaries and column px", () => {
  assert.equal(starsLayout(1200), "three");
  assert.equal(starsLayout(900), "three");
  assert.equal(starsLayout(899), "dots");
  assert.equal(starsLayout(600), "dots");
  assert.equal(starsLayout(599), "single");
  assert.equal(starsLayout(320), "single");
  const three = starsColumns("three");
  const dots = starsColumns("dots");
  const single = starsColumns("single");
  assert.equal(three.planet, 168);
  assert.equal(three.right, 300);
  assert.equal(three.rightMin, 260);
  assert.equal(three.rightMax, 340);
  assert.equal(three.middle, "minmax(420px, 1fr)");
  assert.equal(dots.planet, 72);
  assert.equal(dots.right, 240);
  assert.equal(dots.rightMin, 220);
  assert.equal(dots.rightMax, 260);
  assert.equal(dots.middle, "minmax(0, 1fr)");
  assert.equal(single.planet, 0);
  assert.equal(single.right, 0);
});

test("S15 dots columns fit 600 through 899", () => {
  const dots = starsColumns("dots");
  assert.deepEqual(dots, {
    planet: 72,
    right: 240,
    rightMin: 220,
    rightMax: 260,
    middle: "minmax(0, 1fr)",
  });
  const template = `${dots.planet}px ${dots.middle} clamp(${dots.rightMin}px, ${dots.right}px, ${dots.rightMax}px)`;
  assert.equal(template, "72px minmax(0, 1fr) clamp(220px, 240px, 260px)");
  const three = starsColumns("three");
  assert.deepEqual(three, {
    planet: 168,
    right: 300,
    rightMin: 260,
    rightMax: 340,
    middle: "minmax(420px, 1fr)",
  });
  const page = src("src/lib/StarsPage.svelte");
  assert.ok(
    page.includes(
      "`${cols.planet}px ${cols.middle} clamp(${cols.rightMin}px, ${cols.right}px, ${cols.rightMax}px)`",
    ),
  );
  const bodyAt = page.indexOf(".s-body {");
  const bodyCss = page.slice(bodyAt, page.indexOf("}", bodyAt));
  assert.match(bodyCss, /gap:\s*10px/);
  assert.match(bodyCss, /padding:\s*10px 12px 12px/);
  assert.match(bodyCss, /overflow:\s*hidden/);
  const middleMin = dots.middle === "minmax(0, 1fr)" ? 0 : Number(/^minmax\((\d+)px/.exec(dots.middle)?.[1]);
  assert.equal(middleMin, 0);
  const gaps = 10 * 2;
  const padding = 12 + 12;
  const need = dots.planet + middleMin + dots.rightMin + gaps + padding;
  const marked = [600, 640, 775, 899];
  for (const w of marked) {
    assert.equal(starsLayout(w), "dots", String(w));
    assert.ok(72 + 220 + gaps + padding <= w, `marked ${w}`);
  }
  for (let w = 600; w <= 899; w++) {
    assert.equal(starsLayout(w), "dots", String(w));
    assert.ok(need <= w, `${need} <= ${w}`);
  }
  assert.equal(starsLayout(599), "single");
  assert.equal(starsLayout(900), "three");
});

test("S16 stars-save fill is mixed accent", () => {
  const page = src("src/lib/StarsPage.svelte");
  const at = page.indexOf(".stars-save:not(:disabled)");
  assert.ok(at > 0);
  const rule = page.slice(at, page.indexOf("}", at));
  assert.equal(/background:\s*var\(--accent\)\s*;/.test(rule), false);
  assert.match(rule, /background:\s*color-mix\(in srgb,\s*var\(--accent\)\s*24%,\s*transparent\)\s*;/);
  assert.match(rule, /color:\s*var\(--ink\)\s*;/);
  assert.match(rule, /border-color:\s*var\(--accent\)\s*;/);
});

test("S2 planet scope reaches trackers only", () => {
  const t = starsScopeTargets("p-mars");
  assert.equal(t.trackers, "p-mars");
  assert.equal(t.radar, "overall");
  assert.equal(t.journal, "overall");
  assert.equal(t.sky, "overall");
  const page = src("src/lib/StarsPage.svelte");
  assert.match(page, /starsScopeTargets\(trackerScope\)\.trackers/);
  assert.match(page, /scope:\s*"overall"/);
  for (const tag of ["RadarDial", "AlmanacFolio", "ReflectSheet"]) {
    const at = page.indexOf(`<${tag}`);
    assert.ok(at > 0, tag);
    const close = page.indexOf(">", at);
    const open = page.slice(at, close);
    assert.equal(open.includes("scope"), false, tag);
  }
});

test("S3 five spokes, Terroir, Self practice", () => {
  assert.deepEqual(
    STARS_SPOKES.map((s) => s.key),
    ["sleep", "terroir", "athleticism", "relationships", "self"],
  );
  assert.deepEqual(
    STARS_SPOKES.map((s) => s.label),
    ["Sleep", "Terroir", "Athleticism", "Relationships", "Self practice"],
  );
  assert.equal(STARS_SPOKES.length, 5);
});

test("S4 radarDraft does not fill the past and mount does not save", () => {
  const today = "2026-10-07";
  const draft = radarDraft(today, today, {});
  assert.equal(draft.draft, true);
  assert.deepEqual(draft.values, {
    sleep: 3,
    terroir: 3,
    athleticism: 3,
    relationships: 3,
    self: 3,
  });
  const saved = { sleep: 4, terroir: 2, athleticism: 5, relationships: 1, self: 3 };
  const kept = radarDraft(today, today, { [today]: saved });
  assert.equal(kept.draft, false);
  assert.equal(kept.values?.sleep, 4);
  const past = radarDraft("2026-10-01", today, {});
  assert.equal(past.values, null);
  assert.equal(past.draft, false);
  const page = src("src/lib/StarsPage.svelte");
  const heads = ["onMount(async", "function loadState", "function syncStarsDraft"] as const;
  for (const head of heads) {
    const at = page.indexOf(head);
    assert.ok(at >= 0, head);
    const body = braceBody(page, at);
    if (head === "onMount(async") {
      assert.equal(body.includes("persist("), false);
      assert.equal(body.includes("saveCheckin"), false);
    }
    for (const banned of ["saveStarsToday", "save_state", "setAverageWindow("]) {
      assert.equal(body.includes(banned), false, `${head} contains ${banned}`);
    }
  }
});

test("S5 slider handles and hit radius", () => {
  const dial = src("src/lib/stars/RadarDial.svelte");
  assert.match(dial, /role="slider"/);
  assert.match(dial, /aria-valuemin="1"/);
  assert.match(dial, /aria-valuemax="5"/);
  assert.match(dial, /aria-valuenow=/);
  assert.match(dial, /aria-valuetext=/);
  assert.match(dial, /tabindex="0"/);
  assert.ok(HANDLE_R >= 7);
  for (const scale of [0.5, 1, 2, 3]) {
    assert.ok(handleHitRadius(scale) * scale >= 14, String(scale));
  }
  assert.equal(/class="knob"[\s\S]{0,120}r="3"/.test(dial), false);
});

test("S6 keys, parseRating, snapOnSpoke", () => {
  assert.equal(ratingFromKey(3, "ArrowLeft"), 2);
  assert.equal(ratingFromKey(3, "ArrowDown"), 2);
  assert.equal(ratingFromKey(1, "ArrowLeft"), 1);
  assert.equal(ratingFromKey(3, "ArrowRight"), 4);
  assert.equal(ratingFromKey(3, "ArrowUp"), 4);
  assert.equal(ratingFromKey(5, "ArrowUp"), 5);
  assert.equal(ratingFromKey(4, "Home"), 1);
  assert.equal(ratingFromKey(2, "End"), 5);
  assert.equal(ratingFromKey(1, "4"), 4);
  assert.equal(ratingFromKey(1, "1"), 1);
  assert.equal(ratingFromKey(1, "5"), 5);
  assert.equal(ratingFromKey(1, "6"), null);
  assert.equal(ratingFromKey(1, "0"), null);
  assert.equal(parseRating(" 4 "), 4);
  assert.equal(parseRating("4.6"), null);
  assert.equal(parseRating("0"), null);
  assert.equal(parseRating("x"), null);
  assert.equal(snapOnSpoke(0), 1);
  assert.equal(snapOnSpoke(RADAR_R), 5);
  assert.equal(snapOnSpoke((3 / 5) * RADAR_R), 3);
});

test("S7 pageCovers and folioFace", () => {
  const single: FolioPage = { createdAt: 2, dayKey: "2026-10-03" };
  assert.equal(pageCovers(single, "2026-10-03"), true);
  assert.equal(pageCovers(single, "2026-10-04"), false);
  const span: FolioPage = { createdAt: 3, dayKey: "2026-10-01", dayKeyEnd: "2026-10-03" };
  assert.equal(pageCovers(span, "2026-10-01"), true);
  assert.equal(pageCovers(span, "2026-10-02"), true);
  assert.equal(pageCovers(span, "2026-10-03"), true);
  assert.equal(pageCovers(span, "2026-09-30"), false);
  assert.equal(pageCovers(span, "2026-10-04"), false);
  const legacy: FolioPage = { createdAt: new Date(2026, 9, 3, 15).getTime() };
  assert.equal(pageCovers(legacy, "2026-10-03"), true);
  assert.equal(pageCovers(legacy, "2026-10-04"), false);
  const face = folioFace("2026-10-02", [span, single]);
  assert.equal(face.kind, "journal");
  if (face.kind === "journal") assert.equal(face.pages[0]?.createdAt, 3);
  assert.equal(folioFace("2026-10-04", [span]).kind, "cloud");
});

test("S8 strip marks, span bar, no future cells", () => {
  const today = "2026-10-07";
  const pages: FolioPage[] = [
    { createdAt: 1, dayKey: "2026-10-01", dayKeyEnd: "2026-10-03" },
    { createdAt: 2, dayKey: "2026-10-05" },
  ];
  const words = new Set(["2026-10-04"]);
  const span = stripCells(
    "2026-10-02",
    "custom",
    "2026-10-01",
    "2026-10-05",
    pages,
    (d) => words.has(d),
    today,
  );
  const byDay = Object.fromEntries(span.map((c) => [c.day, c]));
  assert.equal(byDay["2026-10-01"].mark, "page");
  assert.equal(byDay["2026-10-01"].spanRole, "start");
  assert.equal(byDay["2026-10-02"].spanRole, "mid");
  assert.equal(byDay["2026-10-03"].spanRole, "end");
  assert.equal(byDay["2026-10-04"].mark, "cloud");
  assert.equal(byDay["2026-10-04"].spanRole, null);
  assert.equal(byDay["2026-10-05"].mark, "page");
  assert.equal(byDay["2026-10-05"].spanRole, "solo");
  assert.equal(byDay["2026-10-02"].selected, true);
  for (const c of span) assert.ok(c.day <= today, c.day);
  const day = stripCells("2026-10-07", "day", "2026-10-07", "2026-10-07", [], () => false, today);
  assert.equal(day.length, 14);
  assert.equal(day.some((c) => c.day > today), false);
  assert.equal(day[day.length - 1].day, today);
  assert.equal(day[day.length - 1].selected, true);
});

test("S9 reflectTarget writes one span page", () => {
  const today = "2026-10-07";
  const day = reflectTarget("day", "2026-10-03", "2026-10-03", "2026-10-03", today);
  assert.equal(day.ok, true);
  if (day.ok) {
    assert.equal(day.dayKey, "2026-10-03");
    assert.equal(day.dayKeyEnd, undefined);
    assert.equal(day.scope, "overall");
  }
  const week = reflectTarget("week", "2026-10-07", "2026-10-01", "2026-10-07", today);
  assert.equal(week.ok, true);
  if (week.ok) {
    assert.equal(week.dayKey, "2026-10-01");
    assert.equal(week.dayKeyEnd, "2026-10-07");
    assert.equal(week.scope, "overall");
    assert.equal(Array.isArray(week), false);
  }
  const month = reflectTarget("month", today, "2026-09-08", today, today);
  assert.equal(month.ok && month.dayKeyEnd, today);
  const custom = reflectTarget("custom", today, "2026-10-02", "2026-10-06", today);
  assert.equal(custom.ok && custom.dayKey, "2026-10-02");
  assert.equal(custom.ok && custom.dayKeyEnd, "2026-10-06");
  const future = reflectTarget("day", "2026-10-08", "2026-10-08", "2026-10-08", today);
  assert.equal(future.ok, false);
  const year = reflectTarget("year", today, "2025-10-08", today, today);
  assert.equal(year.ok, false);
  if (!year.ok) assert.match(year.hint, /week, month or custom/);
});

test("S10 sky bag range and no bot lines", () => {
  const inside =  new Date(2026, 9, 3, 12).getTime() / 1000;
  const outside = new Date(2026, 9, 1, 12).getTime() / 1000;
  const docs: SkyDoc[] = [
    { entries: [{ token: "inside", weight: 1 }], at: inside, stamped: true },
    { entries: [{ token: "outside", weight: 1 }], at: outside, stamped: true },
  ];
  const kept = docsInRange(docs, "2026-10-02", "2026-10-04");
  assert.deepEqual(kept.map((d) => d.entries[0].token), ["inside"]);
  const state = {
    kanbanCards: [
      {
        status: "done",
        title: "Terrace harvest",
        completedAt: inside,
        lastTouchedAt: inside,
        projectId: "p1",
      },
    ],
    projects: [{ id: "p1", name: "Mars" }],
    companion: {
      turns: [
        { role: "you", text: "terrace coffee please" },
        { role: "them", text: "robothelper reply" },
      ],
      lastUserLine: "",
    },
  };
  const all = buildDocs(state);
  const tokens = all.flatMap((d) => d.entries.map((e) => e.token));
  assert.equal(tokens.includes("robothelper"), false);
  assert.equal(tokens.includes("terrace"), true);
  const ranged = buildDocs(state, { from: "2026-10-03", to: "2026-10-03" });
  const rangedTokens = ranged.flatMap((d) => d.entries.map((e) => e.token));
  assert.equal(rangedTokens.includes("robothelper"), false);
  assert.equal(rangedTokens.includes("coffee"), false);
  assert.equal(rangedTokens.includes("terrace"), true);
  const sky = src("src/lib/SkyField.svelte");
  assert.match(sky, /:\s*buildDocs\(s\)/);
  assert.match(sky, /buildDocs\(s, range, filter\)/);
});

test("S11 palette guard", () => {
  const files = readdirSync(new URL("../src/lib/stars/", import.meta.url))
    .filter((n) => n.endsWith(".svelte"))
    .map((n) => src(`src/lib/stars/${n}`));
  files.push(src("src/lib/StarsPage.svelte"));
  const styles = files
    .map((file) => [...file.matchAll(/<style>([\s\S]*?)<\/style>/g)].map((m) => m[1]).join("\n"))
    .join("\n");
  const stripped = styles
    .replace(/rgba\(\s*0\s*,\s*0\s*,\s*0\s*,[^)]+\)/g, "")
    .replace(/transparent/g, "");
  assert.equal(/#[0-9a-fA-F]{3,8}/.test(stripped), false, stripped);
  assert.equal(/rgba?\(/.test(stripped), false, stripped);
  assert.equal(/color-scheme:\s*light/.test(styles), false);
  const stars = src("src/lib/StarsPage.svelte");
  const block = stars.slice(stars.indexOf(".stars {"), stars.indexOf("}", stars.indexOf(".stars {")));
  assert.equal(/--bg\s*:/.test(block), false);
  assert.equal(/--panel\s*:/.test(block), false);
  assert.equal(/--ink\s*:/.test(block), false);
  assert.equal(/--paper\s*:/.test(block), false);
});

test("S12 embed banner and own-width layout", () => {
  const page = src("src/lib/StarsPage.svelte");
  assert.equal(page.includes("NudgeStrip"), false);
  assert.equal(page.includes("window.innerWidth"), false);
  assert.match(page, /new ResizeObserver/);
  const embedded = page.slice(page.indexOf("{#if embedded}"), page.indexOf("{:else}"));
  assert.match(embedded, /class="s-banner"/);
  assert.match(embedded, /class="winx"/);
  assert.match(embedded, /onClose\?\.\(\)/);
});

test("S13 tracker controls show on hover and focus", () => {
  const css = [...src("src/lib/stars/StarsTrackers.svelte").matchAll(/<style>([\s\S]*?)<\/style>/g)]
    .map((m) => m[1])
    .join("\n");
  assert.match(css, /\.face:hover\s+\.overlay/);
  assert.match(css, /\.face:focus-within\s+\.overlay/);
  const overlay = css.slice(css.indexOf(".overlay {"), css.indexOf("}", css.indexOf(".overlay {")));
  assert.equal(/display\s*:\s*none/.test(overlay), false);
  assert.match(src("src/lib/stars/StarsTrackers.svelte"), /scope === "overall"/);
});

test("S14 planet column All first and the old reorder write", () => {
  const col = src("src/lib/stars/StarsPlanetColumn.svelte");
  const page = src("src/lib/StarsPage.svelte");
  assert.ok(col.indexOf("All planets") < col.indexOf("{#each projects"));
  assert.match(col, /columnArrow/);
  assert.match(col, /beforeIdAt/);
  assert.match(page, /function commitPlanetOrder/);
  assert.match(page, /onReorderCommit=\{commitPlanetOrder\}/);
});
