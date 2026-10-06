import assert from "node:assert/strict";
import { test } from "node:test";
import {
  RAIL_AUTO_HIDE_PREF,
  RAIL_REVEAL_ACTION,
  ROW_CHROME_PREF,
  applyPlanetOpen,
  autoHideWanted,
  chordAction,
  habitatConfirm,
  habitatStops,
  normalizeRailEdge,
  revealWanted,
  stageCollapsedIds,
  stageFront,
  stagePrefs,
  stageRoles,
  stageStripIds,
  wheelArrow,
  type HabitatSection,
} from "../src/lib/rail/habitat.ts";

const collapsed: HabitatSection[] = [
  { pid: "A", open: false, tileIds: ["a1"], snapCols: [["n1"]], hasTodos: true },
  { pid: "B", open: false, tileIds: ["b1"], snapCols: [], hasTodos: false },
];

const openA: HabitatSection[] = [
  { pid: "A", open: true, tileIds: ["a1"], snapCols: [["n1"]], hasTodos: true },
  { pid: "B", open: false, tileIds: ["b1"], snapCols: [], hasTodos: false },
];

test("S1 default edge LEFT", () => {
  assert.equal(normalizeRailEdge(undefined), "left");
  assert.equal(normalizeRailEdge("nope"), "left");
  assert.equal(normalizeRailEdge(""), "left");
  assert.equal(stagePrefs(undefined).railEdge, "left");
  assert.equal(stagePrefs({}).railEdge, "left");
  assert.equal(normalizeRailEdge("right"), "right");
  assert.equal(normalizeRailEdge("top"), "top");
  assert.equal(normalizeRailEdge("bottom"), "bottom");
  assert.equal(revealWanted(true, false), true);
  assert.equal(revealWanted(false, true), true);
  assert.equal(revealWanted(false, false), false);
});

test("S2 strip order", () => {
  assert.deepEqual(stageStripIds(["A", "B"]), ["stars", "settings", "A", "B"]);
  const ids = habitatStops(collapsed).map((s) => s.id);
  const icons = ids.filter(
    (id) => id === "rstars" || id === "rsettings" || id.startsWith("rhead-"),
  );
  assert.deepEqual(icons, ["rstars", "rsettings", "rhead-A", "rhead-B"]);
  assert.equal(wheelArrow(collapsed, "rhead-A", "up", "left"), "rsettings");
  assert.equal(wheelArrow(collapsed, "rsettings", "up", "left"), "rstars");
  assert.equal(wheelArrow(collapsed, "rhead-A", "down", "left"), "rhead-B");
});

test("S3 one open planet", () => {
  let open = applyPlanetOpen(null, ["A", "B"], "A");
  assert.equal(open, "A");
  assert.deepEqual(
    stageRoles(["A", "B"], open).map((r) => r.role),
    ["open", "receded"],
  );
  open = applyPlanetOpen(open, ["A", "B"], "B");
  assert.equal(open, "B");
  assert.deepEqual(
    stageRoles(["A", "B"], open).map((r) => [r.id, r.role]),
    [
      ["A", "receded"],
      ["B", "open"],
    ],
  );
  assert.equal(
    stageRoles(["A", "B"], open).filter((r) => r.role === "open").length,
    1,
  );
  assert.equal(applyPlanetOpen("B", ["A", "B"], "B"), null);
  assert.deepEqual(
    stageRoles(["A", "B"], null).map((r) => r.role),
    ["icon", "icon"],
  );
  assert.deepEqual(stageCollapsedIds([], ["A", "B"]), ["A", "B"]);
  assert.deepEqual(stageCollapsedIds(["B"], ["A", "B", "C"]), ["B", "C"]);
});

test("S4 Stars+Settings front", () => {
  const stack = stageFront("A");
  assert.deepEqual(
    stack.front.map((f) => f.id),
    ["stars", "settings"],
  );
  assert.ok(stack.front.every((f) => f.z > stack.openZ));
  const stops = habitatStops(openA).map((s) => s.id);
  assert.ok(stops.includes("rstars"));
  assert.ok(stops.includes("rsettings"));
  assert.equal(wheelArrow(openA, "rhead-A", "up", "left"), "rsettings");
  assert.equal(wheelArrow(openA, "rsettings", "up", "left"), "rstars");
  assert.equal(wheelArrow(openA, "rstars", "down", "left"), "rsettings");
});

test("S5 raise only when expanded", () => {
  const shut = habitatStops(collapsed).map((s) => s.id);
  assert.equal(shut.includes("rraise-A"), false);
  assert.equal(shut.includes("rraise-B"), false);
  const open = habitatStops(openA).map((s) => s.id);
  assert.equal(open.includes("rraise-A"), true);
  assert.equal(wheelArrow(openA, "rhead-A", "right", "left"), "rraise-A");
  assert.equal(wheelArrow(openA, "rhead-A", "down", "left"), "rhead-B");
  assert.equal(habitatConfirm("rraise-A"), "raise");
  assert.notEqual(habitatConfirm("rhead-A"), "raise");
  assert.equal(habitatConfirm("rhead-A"), "expand");
});

test("S6 auto-hide pref", () => {
  assert.notEqual(RAIL_AUTO_HIDE_PREF, ROW_CHROME_PREF);
  assert.equal(RAIL_AUTO_HIDE_PREF, "railAutoHideOnBlur");
  assert.equal(ROW_CHROME_PREF, "autohideRowChrome");
  const split = stagePrefs({ railAutoHideOnBlur: false, autohideRowChrome: true });
  assert.equal(split.railAutoHideOnBlur, false);
  assert.equal(split.autohideRowChrome, true);
  assert.equal(stagePrefs(undefined).railAutoHideOnBlur, true);
  assert.equal(stagePrefs({ autohideRowChrome: false }).railAutoHideOnBlur, true);
  assert.equal(
    autoHideWanted({
      railAutoHideOnBlur: false,
      focusInside: false,
      pointerInside: false,
    }),
    false,
  );
  assert.equal(
    autoHideWanted({
      railAutoHideOnBlur: true,
      focusInside: false,
      pointerInside: false,
    }),
    true,
  );
  assert.equal(
    autoHideWanted({
      railAutoHideOnBlur: true,
      focusInside: true,
      pointerInside: false,
    }),
    false,
  );
  assert.equal(
    autoHideWanted({
      railAutoHideOnBlur: true,
      focusInside: false,
      pointerInside: true,
    }),
    false,
  );
});

test("S7 hotkey owner", () => {
  assert.equal(chordAction("cmd+shift+o"), RAIL_REVEAL_ACTION);
  assert.equal(chordAction("⌘⇧O"), "rail-reveal");
  assert.notEqual(chordAction("cmd+shift+o"), "companion-summon");
  assert.equal(chordAction("cmd+alt+o"), "companion-open");
  assert.equal(habitatConfirm("rstars"), "stars");
  assert.equal(habitatConfirm("rsettings"), "settings");
});
