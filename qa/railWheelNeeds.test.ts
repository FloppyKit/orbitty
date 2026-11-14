import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { planetAbbrevs, type HabitatCard } from "../src/lib/rail/habitat.ts";
import { mirrorItems, mirrorStart, type ArchCard, type ArchRow } from "../src/lib/rail/stageArch.ts";

function card(partial: HabitatCard): HabitatCard {
  return partial;
}

function balanced(src: string, at: number, open: string, close: string): string {
  let depth = 0;
  let i = at;
  while (i < src.length) {
    const nextOpen = src.indexOf(open, i);
    const nextClose = src.indexOf(close, i);
    if (nextClose < 0) return src.slice(at);
    if (nextOpen >= 0 && nextOpen < nextClose) {
      depth++;
      i = nextOpen + open.length;
    } else {
      depth--;
      i = nextClose + close.length;
      if (depth === 0) return src.slice(at, i);
    }
  }
  return src.slice(at);
}

function fnBody(src: string, name: string): string {
  const at = src.indexOf(`function ${name}`);
  assert.ok(at >= 0, name);
  const open = src.indexOf("{", at);
  let depth = 0;
  for (let i = open; i < src.length; i++) {
    const ch = src[i];
    if (ch === "{") depth++;
    else if (ch === "}") {
      depth--;
      if (depth === 0) return src.slice(at, i + 1);
    }
  }
  return "";
}

function buttons(src: string, className: string): string[] {
  const out: string[] = [];
  const needle = `class="${className}"`;
  let from = 0;
  while (from < src.length) {
    const at = src.indexOf(needle, from);
    if (at < 0) break;
    const start = src.lastIndexOf("<button", at);
    const end = src.indexOf("</button>", at);
    if (start < 0 || end < 0) break;
    out.push(src.slice(start, end));
    from = end + "</button>".length;
  }
  return out;
}

test("planetAbbrevs_needed_then_doing_then_todo_cap3", () => {
  const pid = "p";
  const cards: HabitatCard[] = [
    card({
      id: "n-open",
      projectId: pid,
      kind: "needed",
      neededState: "open",
      createdAt: 2,
      title: "Open need",
    }),
    card({
      id: "n-later",
      projectId: pid,
      kind: "needed",
      neededState: "later",
      createdAt: 1,
      lastTouchedAt: 5,
      title: "Later need",
    }),
    card({
      id: "n-answered",
      projectId: pid,
      kind: "needed",
      neededState: "answered",
      createdAt: 3,
      title: "Answered",
    }),
    card({
      id: "n-skipped",
      projectId: pid,
      kind: "needed",
      neededState: "skipped",
      createdAt: 4,
      title: "Skipped",
    }),
    card({
      id: "d-early",
      projectId: pid,
      kind: "todo",
      status: "doing",
      createdAt: 4,
      title: "Doing A",
    }),
    card({
      id: "d-late",
      projectId: pid,
      kind: "todo",
      status: "doing",
      createdAt: 10,
      title: "Doing B",
    }),
    card({
      id: "t-early",
      projectId: pid,
      kind: "todo",
      status: "todo",
      createdAt: 11,
      title: "Todo A",
    }),
    card({
      id: "t-late",
      projectId: pid,
      kind: "todo",
      status: "todo",
      createdAt: 12,
      title: "Todo B",
    }),
    card({
      id: "done",
      projectId: pid,
      kind: "todo",
      status: "done",
      createdAt: 1,
      completedAt: 9,
      title: "Done card",
    }),
    card({
      id: "other",
      projectId: "q",
      kind: "todo",
      status: "todo",
      createdAt: 1,
      title: "Other planet",
    }),
  ];
  const got = planetAbbrevs(cards, pid);
  assert.equal(got.rows.length, 3);
  assert.deepEqual(
    got.rows.map((row) => row.title),
    ["Open need", "Later need", "Doing A"],
  );
  assert.equal(got.rows[0].kind, "needed");
  assert.equal(got.rows[1].kind, "needed");
  assert.equal(got.rows[1].neededState, "later");
  assert.equal(got.rows[2].status, "doing");
  assert.equal(got.more, 3);
  assert.equal(
    got.rows.some((row) => row.title === "Done card" || row.title === "Answered" || row.title === "Skipped"),
    false,
  );
});

test("planetAbbrevs_empty_when_no_open_work", () => {
  const got = planetAbbrevs(
    [
      card({
        id: "done",
        projectId: "A",
        kind: "todo",
        status: "done",
        title: "Done card",
        completedAt: 1,
      }),
      card({
        id: "answered",
        projectId: "A",
        kind: "needed",
        neededState: "answered",
        title: "Answered",
      }),
      card({
        id: "skipped",
        projectId: "A",
        kind: "needed",
        neededState: "skipped",
        title: "Skipped",
      }),
      card({
        id: "elsewhere",
        projectId: "B",
        kind: "todo",
        status: "todo",
        title: "Elsewhere",
      }),
    ],
    "A",
  );
  assert.deepEqual(got.rows, []);
  assert.equal(got.more, 0);
});

test("railWheelNeeds level-1 abbrevs stay outside the planet hit and preview todos only", () => {
  const list = readFileSync(new URL("../src/lib/rail/List.svelte", import.meta.url), "utf8");
  const page = readFileSync(new URL("../src/lib/RailPage.svelte", import.meta.url), "utf8");
  const eachAt = list.indexOf("{#each items as item, i (item.stop)}");
  assert.ok(eachAt >= 0);
  const each = balanced(list, eachAt, "{#each", "{/each}");
  assert.equal(each.includes('class="arch-abbrevs"'), false);
  assert.equal(each.includes("ul.arch-abbrevs"), false);
  assert.equal(each.includes("BoardSnapshot"), false);
  assert.equal(each.includes("BoardPreview"), false);
  assert.equal(each.includes("{#if i === center}"), false);
  const ifAt = each.indexOf('item.kind === "planet" && item.pid && !archHidden');
  const columnAt = each.search(/<KanbanPage|<BoardColumns/);
  assert.ok(ifAt >= 0 && columnAt > ifAt);
  const prelude = each.slice(ifAt, columnAt);
  assert.match(prelude, /archHidden\(i\)/);
  assert.match(prelude, /front === "arch"/);
  assert.doesNotMatch(prelude, /i === center/);
  const hits = buttons(list, "arch-hit");
  assert.equal(hits.length, 2);
  for (const hit of hits) {
    assert.equal(hit.includes("arch-abbrev"), false);
    assert.equal(hit.includes("KanbanPage"), false);
    assert.equal(hit.includes("BoardColumns"), false);
    assert.equal(hit.includes("openAbbrev"), false);
  }
  assert.match(each, /onclick=\{\(\) => onTileClick\(item, i\)\}/);
  assert.match(each, /openAbbrev/);
  assert.doesNotMatch(each, /openDetail/);
  assert.doesNotMatch(each, /load_state|loadState/);
  const click = fnBody(list, "onTileClick");
  assert.match(click, /archEnter/);
  assert.doesNotMatch(click, /openAbbrev|abbrevPick|toggleDoing|completeTodo|planetAbbrevs/);
  const opener = fnBody(list, "openAbbrev");
  assert.match(opener, /abbrevPick/);
  assert.doesNotMatch(opener, /archEnter|archDo|layer2|toggleDoing|completeTodo/);
  const wheelAt = list.indexOf("Scroll never changes depth");
  const listenAt = list.indexOf("el.addEventListener", wheelAt);
  const wheel = list.slice(wheelAt, listenAt);
  const archZone = wheel.slice(wheel.indexOf('zone === "arch"'), wheel.indexOf("archScroll2"));
  assert.match(archZone, /return;/);
  assert.doesNotMatch(archZone, /archEnter|front:\s*"layer2"/);
  const nAt = list.indexOf('class="l1-needed-preview"');
  const nEnd = list.indexOf("l1-needed-preview-end");
  assert.ok(nAt >= 0 && nEnd > nAt);
  const neededPreview = list.slice(nAt, nEnd);
  assert.doesNotMatch(
    neededPreview,
    /toggleDoing|completeTodo|armNeededMouth|openKanbanFocus|laterNeeded|Answer|Later|Open in board|Start|Park|Mark done/,
  );
  const tAt = list.indexOf('class="l1-todo-preview"');
  const tEnd = list.indexOf("l1-todo-preview-end");
  assert.ok(tAt >= 0 && tEnd > tAt);
  const todoPreview = list.slice(tAt, tEnd);
  assert.match(todoPreview, /toggleDoing/);
  assert.match(todoPreview, /completeTodo/);
  assert.match(todoPreview, />Start</);
  assert.match(todoPreview, />Park</);
  assert.match(todoPreview, />Mark done</);
  assert.doesNotMatch(todoPreview, /armNeededMouth|openKanbanFocus|laterNeeded|>Answer<|>Open in board</);
  assert.match(page, /toggleDoingBody/);
  assert.match(page, /planetAbbrevs/);
});

test("mirrorItems_order_is_habitat_board_needed_then_windows", () => {
  const pid = "p";
  const cards: ArchCard[] = [
    { id: "n-open", projectId: pid, kind: "needed", neededState: "open", createdAt: 2, title: "Ask", neededWhy: "because" },
    { id: "n-later", projectId: pid, kind: "needed", neededState: "later", createdAt: 1, title: "Later" },
    { id: "n-done", projectId: pid, kind: "needed", neededState: "answered", createdAt: 3, title: "Done" },
    { id: "d1", projectId: pid, kind: "todo", status: "doing", createdAt: 1, title: "Doing" },
    { id: "t1", projectId: pid, kind: "todo", status: "todo", createdAt: 2, title: "Todo" },
  ];
  const rows: ArchRow[] = Array.from({ length: 12 }, (_, i) => ({
    id: `w${i}`,
    title: `W${i}`,
    sleeping: i % 3 === 0,
  }));
  const items = mirrorItems({ pid, cards, rows });
  assert.equal(items[0].kind, "habitat");
  assert.equal(items[0].stop, "rl2-habitat");
  assert.equal(items[1].kind, "board");
  assert.equal(items[1].stop, "rl2-board");
  const needed = items.filter((it) => it.kind === "needed");
  assert.deepEqual(
    needed.map((n) => n.id),
    ["n-later", "n-open"],
  );
  assert.equal(needed[1].why, "because");
  const windows = items.filter((it) => it.kind === "window");
  assert.equal(windows.length, 12);
  assert.deepEqual(
    windows.map((w) => w.id),
    rows.map((r) => r.id),
  );
  assert.equal(windows[0].sleeping, true);
  assert.equal(
    items.some((it) => it.kind === "doing" || it.kind === "todo" || it.kind === "more"),
    false,
  );
  assert.equal(
    items.some((it) => it.kind === "scene" || it.stop === "rl2-scene"),
    false,
  );
  assert.equal(mirrorStart(items), 0);
  assert.equal(items[mirrorStart(items)].kind, "habitat");
});

test("railWheelNeeds level-2 habitat then board then needed then windows", () => {
  const list = readFileSync(new URL("../src/lib/rail/List.svelte", import.meta.url), "utf8");
  const ring = readFileSync(new URL("../src/lib/rail/pageRing.svelte.ts", import.meta.url), "utf8");
  const place = readFileSync(new URL("../src/lib/rail/stagePlace.ts", import.meta.url), "utf8");
  assert.match(list, /item\.kind === "habitat" && archState\.pid\) rail\.raisePlanet\(archState\.pid\)/);
  assert.match(list, /centred2\.kind === "habitat"/);
  assert.match(list, /rail\.raisePlanet\(archState\.pid\)/);
  const select = fnBody(list, "selectLayer");
  assert.match(select, /kind === "board"/);
  assert.match(select, /enterBoard/);
  assert.match(list, /raiseWindow\(centred2\)/);
  const neededAt = list.indexOf('class="l2-needed-card"');
  const neededEnd = list.indexOf("</div>", neededAt);
  assert.ok(neededAt >= 0 && neededEnd > neededAt);
  const needed = list.slice(neededAt, neededEnd);
  assert.match(needed, />Answer</);
  assert.match(needed, />Open in board</);
  assert.match(needed, /armNeededMouth/);
  assert.match(needed, /openKanbanFocus/);
  assert.equal(list.includes("rl2-scene"), false);
  assert.equal(list.includes('kind === "scene"'), false);
  assert.equal(list.includes("archLayer2Items"), false);
  assert.match(list, /mirrorItems\(/);
  const confirm = fnBody(ring, "confirmLayer2Stop");
  assert.match(confirm, /kind === "habitat"/);
  assert.match(confirm, /raisePlanet/);
  assert.match(confirm, /kind === "needed"/);
  assert.match(confirm, /armNeededMouth/);
  assert.match(confirm, /kind === "board"/);
  assert.match(confirm, /enterBoard/);
  assert.match(confirm, /wakeSleeper/);
  assert.match(confirm, /focusRow/);
  assert.doesNotMatch(confirm, /rl2-scene|kind === "scene"/);
  assert.equal(place.includes("rl2-scene"), false);
  assert.equal(place.includes('kind === "scene"'), false);
  assert.match(place, /stop: "rl2-habitat"/);
});

test("railWheelNeeds_level1_column_reuses_board_without_preview", () => {
  const kanban = readFileSync(new URL("../src/lib/KanbanPage.svelte", import.meta.url), "utf8");
  const list = readFileSync(new URL("../src/lib/rail/List.svelte", import.meta.url), "utf8");
  const lists = readFileSync(new URL("../src/routes/+page.svelte", import.meta.url), "utf8");
  const well = readFileSync(new URL("../src/lib/companion/Well.svelte", import.meta.url), "utf8");
  assert.match(kanban, /columnsOnly\s*=\s*false/);
  const previewAt = kanban.indexOf("<BoardPreview");
  assert.ok(previewAt >= 0);
  const gate = kanban.lastIndexOf("columnsOnly", previewAt);
  assert.ok(gate >= 0);
  assert.match(kanban.slice(gate, previewAt), /!columnsOnly/);
  const l2 = list.indexOf("railPlanet={boardPlanet}");
  assert.ok(l2 >= 0);
  const l2Tag = list.lastIndexOf("<KanbanPage", l2);
  const l2End = list.indexOf("/>", l2);
  assert.ok(l2Tag >= 0 && l2End > l2Tag);
  const l2Mount = list.slice(l2Tag, l2End);
  assert.match(l2Mount, /embedded/);
  assert.doesNotMatch(l2Mount, /columnsOnly/);
  const eachAt = list.indexOf("{#each items as item, i (item.stop)}");
  assert.ok(eachAt >= 0);
  const each = balanced(list, eachAt, "{#each", "{/each}");
  assert.match(each, /columnsOnly/);
  assert.match(each, /<KanbanPage|<BoardColumns/);
  assert.doesNotMatch(each, /BoardPreview|BoardSnapshot|class="arch-abbrevs"/);
  const mountAt = kanban.indexOf("onMount(async () => {");
  const listenAt = kanban.indexOf('listen("state:changed", loadState)', mountAt);
  assert.ok(mountAt >= 0 && listenAt > mountAt);
  assert.match(kanban.slice(mountAt, listenAt), /if \(columnsOnly\) return/);
  assert.equal(lists.includes("columnsOnly"), false);
  assert.equal(well.includes("columnsOnly"), false);
});

function fnBodyNamed(src: string, signature: string): string {
  const at = src.indexOf(signature);
  assert.ok(at >= 0, signature);
  const open = src.indexOf("{", at);
  let depth = 0;
  for (let i = open; i < src.length; i++) {
    const ch = src[i];
    if (ch === "{") depth++;
    else if (ch === "}") {
      depth--;
      if (depth === 0) return src.slice(at, i + 1);
    }
  }
  return "";
}

test("onTileClick_opens_habitat_and_enters_level2", () => {
  const list = readFileSync(new URL("../src/lib/rail/List.svelte", import.meta.url), "utf8");
  const ring = readFileSync(new URL("../src/lib/rail/pageRing.svelte.ts", import.meta.url), "utf8");
  const pointer = readFileSync(new URL("../src/lib/rail/pagePointer.svelte.ts", import.meta.url), "utf8");
  const click = fnBody(list, "onTileClick");
  const planetAt = click.indexOf('item.kind === "planet"');
  const unmappedAt = click.indexOf('item.kind === "unmapped"');
  assert.ok(planetAt >= 0 && unmappedAt > planetAt);
  const planet = click.slice(planetAt, unmappedAt);
  assert.match(planet, /archEnter/);
  assert.match(planet, /raisePlanet/);
  assert.doesNotMatch(planet, /openAbbrev/);
  const open = fnBodyNamed(pointer, "export function raisePlanet(");
  assert.match(open, /set_planets_hidden/);
  assert.match(open, /habitat:expand/);
  assert.match(open, /bring_planet_front/);
  assert.match(open, /locate_planet/);
  assert.doesNotMatch(open, /bring_all_front/);
  assert.match(list, /item\.kind === "habitat" && archState\.pid\) rail\.raisePlanet\(archState\.pid\)/);
  assert.match(list, /archState\.pid && rail\.raisePlanet\(archState\.pid\)/);
  const confirm = fnBody(ring, "confirmLayer2Stop");
  assert.match(confirm, /kind === "habitat"/);
  assert.match(confirm, /raisePlanet/);
  assert.match(confirm, /kind === "needed"/);
  assert.match(confirm, /armNeededMouth/);
  assert.match(confirm, /kind === "board"/);
  assert.match(confirm, /enterBoard/);
  assert.match(confirm, /wakeSleeper/);
  assert.match(confirm, /focusRow/);
  assert.doesNotMatch(confirm, /rl2-scene|kind === "scene"/);
  assert.equal(list.includes("rl2-scene"), false);
  assert.equal(list.includes('kind === "scene"'), false);
});
