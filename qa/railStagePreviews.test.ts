import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  archLayout,
  archPainted,
  archSlot,
  besideRailAdjacent,
  placeBesideRail,
  stagePreviewModel,
  stageStackLayout,
  type ArchCard,
  type ArchRow,
  type PlaceRect,
} from "../src/lib/rail/stageArch.ts";

function card(partial: ArchCard): ArchCard {
  return partial;
}

const pid = "p";

function sample(): { cards: ArchCard[]; rows: ArchRow[] } {
  const cards: ArchCard[] = [
    card({ id: "n-open", projectId: pid, kind: "needed", neededState: "open", createdAt: 2, title: "Ask" }),
    card({ id: "n-later", projectId: pid, kind: "needed", neededState: "later", createdAt: 1, title: "Later" }),
    card({ id: "n-done", projectId: pid, kind: "needed", neededState: "answered", createdAt: 3, title: "Done" }),
    card({ id: "d1", projectId: pid, kind: "todo", status: "doing", createdAt: 1, title: "Doing" }),
    card({ id: "t1", projectId: pid, kind: "todo", status: "todo", createdAt: 2, title: "Todo" }),
    card({ id: "other", projectId: "q", kind: "needed", neededState: "open", createdAt: 1, title: "Nope" }),
  ];
  const rows: ArchRow[] = [
    { id: "w1", title: "Notes", appName: "Notes", iconBase64: "aaa", isFrontmost: true },
    { id: "w2", title: "Mail", appName: "Mail", isMinimized: true },
  ];
  return { cards, rows };
}

test("P1 preview model is a board tile plus window tiles, not todo rows", () => {
  const { cards, rows } = sample();
  const model = stagePreviewModel({ pid, cards, rows });
  assert.equal(model.board.kind, "board");
  assert.equal(model.board.preview, true);
  assert.equal(model.windows.length, 2);
  assert.deepEqual(
    model.windows.map((w) => w.id),
    ["w1", "w2"],
  );
  assert.equal(model.windows[0].iconBase64, "aaa");
  assert.equal(model.windows[0].title, "Notes");
  assert.equal(model.windows[0].appName, "Notes");
  assert.equal(model.windows[0].frontmost, true);
  assert.equal(model.windows[1].minimized, true);
  assert.equal(model.popOut.label, "Pop out");
  const kinds = model.slots.map((it) => it.kind);
  assert.equal(kinds.includes("doing"), false);
  assert.equal(kinds.includes("todo"), false);
  assert.equal(kinds[0], "board");
  assert.equal(
    kinds.filter((k) => k === "window").length,
    rows.length,
  );
});

test("P2 Needed stays when open needed cards exist", () => {
  const { cards, rows } = sample();
  const model = stagePreviewModel({ pid, cards, rows });
  assert.deepEqual(
    model.needed.map((n) => n.id),
    ["n-later", "n-open"],
  );
  assert.equal(
    model.slots.filter((it) => it.kind === "needed").length,
    2,
  );
});

test("P3 empty windows are board and Needed only; overflow is a count", () => {
  const { cards } = sample();
  const empty = stagePreviewModel({ pid, cards, rows: [] });
  assert.equal(empty.windows.length, 0);
  assert.equal(empty.overflow, 0);
  assert.deepEqual(
    empty.slots.map((it) => it.kind),
    ["board", "needed", "needed"],
  );
  const many: ArchRow[] = Array.from({ length: 12 }, (_, i) => ({ id: `w${i}`, title: `W${i}` }));
  const capped = stagePreviewModel({ pid, cards: [], rows: many });
  assert.equal(capped.windows.length, 8);
  assert.equal(capped.overflow, 4);
  assert.equal(capped.needed.length, 0);
  assert.deepEqual(
    capped.windows.map((w) => w.id),
    many.slice(0, 8).map((w) => w.id),
  );
  assert.equal(capped.slots.some((it) => it.kind === "window" && !it.id), false);
});

test("P4 settings place stays on the rail display, beside the rail", () => {
  const primary: PlaceRect = { x: 0, y: 0, w: 1920, h: 1080 };
  const portrait: PlaceRect = { x: 1920, y: 0, w: 900, h: 1600 };
  const frames = [primary, portrait];
  const rail: PlaceRect = { x: 8, y: 80, w: 360, h: 700 };
  const stale: PlaceRect = { x: 2400, y: 200, w: 780, h: 640 };
  const placed = placeBesideRail(rail, { w: 640, h: 480 }, frames, stale);
  assert.ok(placed);
  assert.equal(besideRailAdjacent(placed, rail, frames), true);
  assert.equal(besideRailAdjacent(stale, rail, frames), false);

  const railP: PlaceRect = { x: 1928, y: 40, w: 280, h: 700 };
  const stalePrimary: PlaceRect = { x: 400, y: 120, w: 780, h: 640 };
  const onPortrait = placeBesideRail(railP, { w: 500, h: 400 }, frames, stalePrimary);
  assert.ok(onPortrait);
  assert.equal(besideRailAdjacent(onPortrait, railP, frames), true);

  const tight: PlaceRect = { x: 0, y: 0, w: 900, h: 1600 };
  const crowded = placeBesideRail({ x: 8, y: 40, w: 360, h: 700 }, { w: 780, h: 640 }, [tight], stale);
  assert.ok(crowded);
  assert.equal(besideRailAdjacent(crowded, { x: 8, y: 40, w: 360, h: 700 }, [tight]), true);
  assert.ok(crowded.x + crowded.w <= tight.x + tight.w + 0.5);
});

function framePoint(edge: string, slot: { x: number; y: number }, frame: { w: number; h: number }) {
  if (edge === "right") return { x: frame.w + slot.x, y: frame.h / 2 + slot.y };
  if (edge === "top") return { x: frame.w / 2 + slot.x, y: slot.y };
  if (edge === "bottom") return { x: frame.w / 2 + slot.x, y: frame.h + slot.y };
  return { x: slot.x, y: frame.h / 2 + slot.y };
}

test("P5 preview tiles and arch slots sit in the painted region", () => {
  const frame = { w: 1040, h: 760 };
  const lay = archLayout("left", frame.w, frame.h);
  const stacks = stageStackLayout("left", frame.w, frame.h, 3, 2);
  const tiles = [stacks.board, stacks.needed, ...stacks.windows];
  for (const tile of tiles) {
    assert.ok(tile.w > 8 && tile.h > 8, `${tile.w}x${tile.h}`);
    const samples = [
      { x: tile.x + 2, y: tile.y + 2 },
      { x: tile.x + tile.w / 2, y: tile.y + tile.h / 2 },
      { x: tile.x + tile.w - 3, y: tile.y + tile.h - 3 },
    ];
    for (const point of samples) {
      assert.equal(
        archPainted(lay, point, { front: "layer2", pid: "p", revealed: true }),
        true,
        `${point.x},${point.y}`,
      );
    }
  }
  const centre = framePoint("left", archSlot(0, "left", 1, frame), frame);
  assert.equal(
    archPainted(lay, centre, { front: "arch", pid: null, revealed: true }),
    true,
  );
  const settings = stageStackLayout("left", frame.w, frame.h, 0, 0).settings;
  const mid = { x: settings.x + settings.w / 2, y: settings.y + settings.h / 2 };
  assert.equal(
    archPainted(lay, mid, { front: "arch", pid: null, revealed: true, embed: "settings" }),
    true,
  );
  assert.equal(
    archPainted(lay, mid, { front: "arch", pid: null, revealed: true }),
    false,
  );
});

test("P6 wiring: embed or beside-rail, no toggle, no feature save, hover lift stays inner", () => {
  const rail = readFileSync(new URL("../src/lib/RailPage.svelte", import.meta.url), "utf8");
  const list = readFileSync(new URL("../src/lib/rail/List.svelte", import.meta.url), "utf8");
  const css = ["rail-chrome.css", "rail-rows.css", "rail-stage.css", "rail-arch.css"].map((name) => readFileSync(new URL(`../src/lib/rail/${name}`, import.meta.url), "utf8")).join("");
  const toggle = readFileSync(new URL("../src-tauri/src/rail/rail_toggle.rs", import.meta.url), "utf8");
  const drawers = readFileSync(new URL("../src-tauri/src/windows/drawers.rs", import.meta.url), "utf8");
  const openAt = rail.indexOf("function openStageDest");
  const open = rail.slice(openAt, openAt + 700);
  assert.match(open, /dest_show/);
  assert.match(open, /stars-beside/);
  assert.match(open, /stageEmbed = "settings"/);
  assert.equal(open.includes("toggle_labeled"), false);
  assert.equal(open.includes("persist("), false);
  assert.equal(open.includes("save_state"), false);
  const pop = rail.slice(rail.indexOf("function popOutSettings"), rail.indexOf("function popOutSettings") + 500);
  assert.match(pop, /settings-beside/);
  assert.equal(pop.includes("persist("), false);
  assert.match(toggle, /"settings-beside"/);
  assert.match(toggle, /"stars-beside"/);
  assert.match(toggle, /show_drawer_beside_rail/);
  assert.match(drawers, /fn place_beside_rail_display/);
  assert.equal(/\.arch-slot:hover\s*[,{][^}]*transform\s*:/.test(css), false);
  assert.match(css, /\.arch-slot:hover \.arch-vis/);
  assert.match(list, /data-stage-board/);
  assert.match(list, /data-win-preview/);
  assert.match(list, /data-stage-settings/);
  assert.equal(list.includes('kind === "todo"'), false);
  assert.equal(list.includes('kind === "doing"'), false);
  const parked = css.slice(css.indexOf(".rail.stage-shell.stage-parked"), css.indexOf(".rail.stage-shell.stage-parked") + 420);
  assert.equal(/stage-parked \.scroll/.test(parked), false);
});
