import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { habitatConfirm } from "../src/lib/rail/habitat.ts";
import {
  COACH_LINE,
  coachDue,
  coachHoldsRail,
  coachStops,
  emptyDeskShowsWheel,
  normalizeStory,
  type CoachStory,
} from "../src/lib/rail/coach.ts";
import {
  spinArrow,
  spinConfirm,
  spinItems,
  spinStops,
  type SpinDir,
} from "../src/lib/rail/stageLook.ts";

const U = "__unmapped__";
const edges = ["left", "right", "top", "bottom"] as const;
const dirs: SpinDir[] = ["up", "down", "left", "right"];

function axis(edge: string): { back: SpinDir; forward: SpinDir; inward: SpinDir; outward: SpinDir } {
  if (edge === "right") return { back: "up", forward: "down", inward: "left", outward: "right" };
  if (edge === "top") return { back: "left", forward: "right", inward: "down", outward: "up" };
  if (edge === "bottom") return { back: "left", forward: "right", inward: "up", outward: "down" };
  return { back: "up", forward: "down", inward: "right", outward: "left" };
}

const armed: CoachStory = { v: 1, skipped: false, coachDone: false };

function due(over: Partial<Parameters<typeof coachDue>[0]> = {}) {
  return coachDue({
    trusted: true,
    stateLoaded: true,
    claimsCount: 0,
    story: armed,
    embedded: false,
    ...over,
  });
}

test("first-run state renders coach line", () => {
  assert.equal(due(), true);
  assert.equal(COACH_LINE, "Claim a window to make a walker.");
  assert.equal(coachHoldsRail(true), true);
  assert.equal(coachHoldsRail(false), false);
  const list = readFileSync("src/lib/rail/List.svelte", "utf8");
  assert.match(list, /\{COACH_LINE\}/);
  const css = readFileSync("src/lib/rail/rail-stage.css", "utf8");
  assert.match(css, /\.coach-line \{[^}]*top:\s*32px/);
});

test("K1 coach due only when trusted", () => {
  assert.equal(due({ trusted: true }), true);
  assert.equal(due({ trusted: false }), false);
});

test("K2 claims hide the coach", () => {
  assert.equal(due({ claimsCount: 0 }), true);
  assert.equal(due({ claimsCount: 1 }), false);
  assert.equal(due({ claimsCount: 4 }), false);
});

test("K3 coachDone hides the coach", () => {
  assert.equal(due(), true);
  assert.equal(due({ story: { ...armed, coachDone: true } }), false);
});

test("K4 skipped and v0 hide the coach", () => {
  assert.equal(due({ story: { ...armed, skipped: true } }), false);
  assert.equal(due({ story: { ...armed, v: 0 } }), false);
  assert.equal(due({ story: armed }), true);
});

test("K5 embedded never shows the coach", () => {
  assert.equal(due({ embedded: false }), true);
  assert.equal(due({ embedded: true }), false);
});

test("K6 coach stops exist only when due", () => {
  assert.deepEqual(coachStops(false), []);
  assert.deepEqual(
    coachStops(true).map((s) => s.id),
    ["rcoach-claim", "rcoach-skip"],
  );
  const items = spinItems(["p"], U);
  const off = spinStops(items, false).map((s) => s.id);
  const on = spinStops(items, true).map((s) => s.id);
  assert.equal(off.includes("rcoach-claim"), false);
  assert.equal(off.includes("rcoach-skip"), false);
  assert.equal(on.includes("rcoach-claim"), true);
  assert.equal(on.includes("rcoach-skip"), true);
  assert.equal(on.includes("rstars"), true);
});

test("K7 confirm mapping keeps existing ids and adds coach stops", () => {
  assert.equal(spinConfirm("rstars"), "stars");
  assert.equal(spinConfirm("rsettings"), "settings");
  assert.equal(spinConfirm("rnewproj"), "newProject");
  assert.equal(spinConfirm("rhead-__unmapped__"), "centre");
  assert.equal(spinConfirm("rhead-p1"), "raise");
  assert.equal(spinConfirm("nope"), null);
  assert.equal(spinConfirm("rcoach-claim"), "coachClaim");
  assert.equal(spinConfirm("rcoach-skip"), "coachSkip");
  assert.equal(habitatConfirm("rstars"), "stars");
  assert.equal(habitatConfirm("rsettings"), "settings");
  assert.equal(habitatConfirm("rhead-p1"), "expand");
});

test("K8 spin nav from rstars reaches both coach stops and back", () => {
  const items = spinItems(["p0"], U);
  for (const edge of edges) {
    const ax = axis(edge);
    assert.equal(spinArrow(items, "rstars", ax.outward, edge, false), null);
    assert.equal(spinArrow(items, "rstars", ax.outward, edge, true), "rcoach-claim");
    assert.equal(spinArrow(items, "rcoach-claim", ax.forward, edge, true), "rcoach-skip");
    assert.equal(spinArrow(items, "rcoach-skip", ax.back, edge, true), "rcoach-claim");
    assert.equal(spinArrow(items, "rcoach-claim", ax.inward, edge, true), "rstars");
    const seen = new Set<string>();
    const q = ["rstars"];
    while (q.length) {
      const id = q.pop() as string;
      if (seen.has(id)) continue;
      seen.add(id);
      for (const dir of dirs) {
        const next = spinArrow(items, id, dir, edge, true);
        if (next) q.push(next);
      }
    }
    assert.equal(seen.has("rcoach-claim"), true, edge);
    assert.equal(seen.has("rcoach-skip"), true, edge);
    assert.equal(seen.has("rstars"), true, edge);
    for (const it of items) assert.equal(seen.has(it.stop), true, `${edge} ${it.stop}`);
  }
});

test("K9 zero windows and zero projects on an armed story shows the wheel", () => {
  assert.equal(emptyDeskShowsWheel({ windows: 0, projects: 0, story: armed }), true);
  assert.equal(
    emptyDeskShowsWheel({ windows: 0, projects: 0, story: { ...armed, skipped: true } }),
    false,
  );
  assert.equal(emptyDeskShowsWheel({ windows: 1, projects: 0, story: armed }), false);
  assert.equal(emptyDeskShowsWheel({ windows: 0, projects: 1, story: armed }), false);
  const items = spinItems([], U);
  const stops = spinStops(items, true).map((s) => s.id);
  assert.equal(stops.includes("rnewproj"), true);
  assert.equal(due(), true);
});

const STARTER_STORY_KEYS = [
  "v",
  "skipped",
  "coachDone",
  "seededProjectId",
  "commsBuilt",
  "starsBuilt",
  "introDone",
];

test("K10 normalizeStory keeps the starterStory key list", () => {
  const raw = {
    v: 3,
    skipped: true,
    coachDone: true,
    seededProjectId: "planet-a",
    commsBuilt: true,
    starsBuilt: false,
    introDone: true,
  };
  const out = normalizeStory(raw);
  assert.deepEqual(Object.keys(out), STARTER_STORY_KEYS);
  assert.deepEqual(out, {
    v: 3,
    skipped: true,
    coachDone: true,
    seededProjectId: "planet-a",
    commsBuilt: true,
    starsBuilt: false,
    introDone: true,
  });
});
