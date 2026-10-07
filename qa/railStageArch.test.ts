import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  ARCH_DWELL_MS,
  ARCH_IDLE,
  archArrow,
  archBack,
  archCentreChanged,
  archConfirm,
  archDepth,
  archEnter,
  archFocusPid,
  archForward,
  archInitial,
  archLayer2Items,
  archLayout,
  archOpenBoard,
  archPointer,
  archScroll2,
  archSlot,
  archStart2,
  mirrorStart,
  type ArchCard,
  type ArchRow,
  type ArchState,
  type Layer2Item,
} from "../src/lib/rail/stageArch.ts";
import {
  spinArrow,
  spinInitial,
  spinItems,
  wheelLayout,
  type SpinDir,
  type SpinItem,
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

function planet(pid: string): SpinItem {
  return { stop: `rhead-${pid}`, kind: "planet", pid };
}

function card(partial: ArchCard): ArchCard {
  return partial;
}

test("S1 arch slots", () => {
  const centre = archSlot(0, "left", 1);
  assert.ok(centre.scale >= 1.1 && centre.scale <= 1.25);
  assert.ok(archSlot(1).opacity < archSlot(0).opacity);
  assert.ok(archSlot(3).opacity <= 0.6);
  assert.ok(archSlot(1).scale < archSlot(0).scale);
  let prevScale = Infinity;
  let prevOpacity = Infinity;
  for (const k of [0, 1, 2, 3, 4, 5]) {
    const slot = archSlot(k, "left", 1);
    const mirror = archSlot(-k, "left", 1);
    for (const edge of edges) {
      for (const ring of [1, 2] as const) {
        for (const kk of [k, -k]) {
          const turned = archSlot(kk, edge, ring);
          assert.ok(!("rotate" in turned) || turned.rotate === 0, `${edge} r${ring} k=${kk}`);
        }
      }
    }
    assert.ok(slot.scale <= prevScale, `scale |k|=${k}`);
    assert.ok(slot.opacity <= prevOpacity, `opacity |k|=${k}`);
    assert.equal(mirror.scale, slot.scale);
    assert.equal(mirror.opacity, slot.opacity);
    prevScale = slot.scale;
    prevOpacity = slot.opacity;
    assert.equal(archSlot(k, "right", 1).x, -slot.x);
    assert.equal(archSlot(k, "right", 1).y, slot.y);
    const top = archSlot(k, "top", 1);
    const bottom = archSlot(k, "bottom", 1);
    assert.equal(top.x, slot.y);
    assert.equal(top.y, slot.x);
    assert.equal(bottom.x, slot.y);
    assert.equal(bottom.y, -slot.x);
    assert.ok(archSlot(k, "left", 2).x > slot.x, `ring2 in front k=${k}`);
  }
  assert.equal(archSlot(4, "left", 1).hidden, false);
  assert.equal(archSlot(5, "left", 1).hidden, true);
  assert.equal(archSlot(-5, "left", 1).hidden, true);
});

test("S2 enter", () => {
  const layer2 = archLayer2Items({
    pid: "a",
    cards: [card({ id: "n", projectId: "a", kind: "needed", neededState: "open", createdAt: 1 })],
    rows: [],
  });
  const entered = archEnter(ARCH_IDLE, planet("a"), layer2);
  assert.equal(entered.front, "layer2");
  assert.equal(entered.pid, "a");
  assert.equal(entered.center2, mirrorStart(layer2));
  assert.equal(mirrorStart(layer2), layer2.findIndex((it) => it.kind === "board"));
  assert.equal(archStart2(layer2), 1);
  const unmapped = archEnter(ARCH_IDLE, { stop: `rhead-${U}`, kind: "unmapped", pid: U });
  assert.equal(unmapped.front, "layer2");
  assert.equal(unmapped.pid, U);
  const held: ArchState = { front: "layer2", pid: "a", center2: 2, pending: { zone: "arch", since: 4 } };
  for (const item of [
    { stop: "rstars", kind: "stars" as const },
    { stop: "rsettings", kind: "settings" as const },
    { stop: "rnewproj", kind: "add" as const },
  ]) {
    const next = archEnter(held, item);
    assert.deepEqual(next, held);
    assert.notEqual(next, held);
  }
  const startList: Layer2Item[] = [
    { kind: "board", stop: "rl2-board" },
    { kind: "needed", stop: "rl2-n", id: "n" },
    { kind: "window", stop: "rl2-w", id: "w" },
  ];
  assert.equal(archStart2(startList), 1);
  assert.equal(archStart2([{ kind: "board", stop: "rl2-board" }]), 0);
  assert.equal(archStart2([]), 0);
  assert.equal(archEnter(ARCH_IDLE, planet("a"), startList).center2, 0);
  assert.equal(startList[archEnter(ARCH_IDLE, planet("a"), startList).center2].kind, "board");
});

test("S3 back + forward", () => {
  let s: ArchState = { front: "board", pid: "a", center2: 3, pending: null };
  s = archBack(s);
  assert.equal(s.front, "layer2");
  assert.equal(s.pid, "a");
  assert.equal(s.center2, 3);
  s = archBack(s);
  assert.equal(s.front, "arch");
  assert.equal(s.pid, "a");
  s = archBack(s);
  assert.equal(s.front, "arch");
  assert.equal(s.pid, "a");
  const fwd = archForward(s);
  assert.equal(fwd.front, "layer2");
  assert.equal(fwd.pid, "a");
  const idle = archForward(ARCH_IDLE);
  assert.equal(idle.front, "arch");
  assert.equal(idle.pid, null);
});

test("S4 one planet at a time", () => {
  const layerA = archLayer2Items({
    pid: "a",
    cards: [card({ id: "na", projectId: "a", kind: "needed", neededState: "open", createdAt: 1 })],
    rows: [],
  });
  const layerB = archLayer2Items({ pid: "b", cards: [], rows: [{ id: "w" }] });
  let s = archEnter(ARCH_IDLE, planet("a"), layerA);
  s = archEnter(s, planet("b"), layerB);
  assert.equal(s.pid, "b");
  assert.equal(s.front, "layer2");
  assert.equal(s.center2, mirrorStart(layerB));
  assert.equal(layerB[s.center2]?.kind, "board");
  const stars = spinItems(["a", "b", "c"], U).find((it) => it.kind === "stars");
  const away = archCentreChanged(s, stars);
  assert.equal(away.pid, null);
  assert.equal(away.front, "arch");
  const other = archCentreChanged(s, planet("c"));
  assert.equal(other.pid, null);
  assert.equal(other.front, "arch");
  const stay = archCentreChanged(s, planet("b"));
  assert.equal(stay.pid, "b");
  assert.equal(stay.front, "layer2");
  assert.equal(stay.center2, s.center2);
});

test("S5 layer-2 items", () => {
  const pid = "p";
  const cards: ArchCard[] = [
    card({ id: "n-late", projectId: pid, kind: "needed", neededState: "later", createdAt: 10, status: "todo", title: "later" }),
    card({ id: "n-open", projectId: pid, kind: "needed", neededState: "open", createdAt: 20, status: "todo", title: "open" }),
    card({ id: "n-ans", projectId: pid, kind: "needed", neededState: "answered", createdAt: 1, status: "todo" }),
    card({ id: "d-new", projectId: pid, kind: "todo", status: "doing", createdAt: 30, title: "d2" }),
    card({ id: "d-old", projectId: pid, kind: "todo", status: "doing", createdAt: 15, title: "d1" }),
    card({ id: "t1", projectId: pid, kind: "todo", status: "todo", createdAt: 40, title: "t1" }),
    card({ id: "t2", projectId: pid, kind: "todo", status: "todo", createdAt: 25, title: "t2" }),
    card({ id: "t3", projectId: pid, kind: "todo", status: "todo", createdAt: 35, title: "t3" }),
    card({ id: "win-card", projectId: pid, kind: "window", status: "todo", windowId: "w9", createdAt: 1 }),
    card({ id: "done", projectId: pid, kind: "todo", status: "done", createdAt: 1 }),
    card({ id: "other", projectId: "q", kind: "todo", status: "todo", createdAt: 1 }),
    card({ id: "other-n", projectId: "q", kind: "needed", neededState: "open", createdAt: 1, status: "todo" }),
  ];
  const rows: ArchRow[] = [
    { id: "w1" },
    { id: "w2", sleeping: true, isMinimized: true },
    { id: "w3", isMinimized: true },
  ];
  const items = archLayer2Items({ pid, cards, rows });
  assert.deepEqual(
    items.map((it) => it.kind),
    ["board", "needed", "needed", "window", "window", "window", "doing", "doing", "todo", "todo", "todo"],
  );
  assert.deepEqual(
    items.filter((it) => it.kind === "needed").map((it) => it.id),
    ["n-late", "n-open"],
  );
  assert.deepEqual(
    items.filter((it) => it.kind === "window").map((it) => it.id),
    ["w1", "w2", "w3"],
  );
  assert.deepEqual(
    items.filter((it) => it.kind === "doing").map((it) => it.id),
    ["d-old", "d-new"],
  );
  assert.deepEqual(
    items.filter((it) => it.kind === "todo").map((it) => it.id),
    ["t2", "t3", "t1"],
  );
  assert.equal(items[0].stop, "rl2-board");
  assert.equal(items.some((it) => it.id === "win-card" || it.id === "done" || it.id === "other" || it.id === "n-ans"), false);

  const manyRows = Array.from({ length: 12 }, (_, i) => ({ id: `w${i}` }));
  const winCap = archLayer2Items({ pid, cards: [], rows: manyRows });
  assert.equal(winCap.filter((it) => it.kind === "window").length, 8);
  assert.equal(winCap[winCap.length - 1].kind, "more");
  assert.equal(winCap[winCap.length - 1].count, 4);

  const todos: ArchCard[] = [
    ...Array.from({ length: 6 }, (_, i) =>
      card({ id: `d${i}`, projectId: pid, kind: "todo", status: "doing", createdAt: i }),
    ),
    ...Array.from({ length: 9 }, (_, i) =>
      card({ id: `t${i}`, projectId: pid, kind: "todo", status: "todo", createdAt: 100 + i }),
    ),
  ];
  const todoCap = archLayer2Items({ pid, cards: todos, rows: [] });
  const body = todoCap.filter((it) => it.kind === "doing" || it.kind === "todo");
  assert.equal(body.length, 10);
  assert.equal(body.filter((it) => it.kind === "doing").length, 6);
  assert.deepEqual(
    body.filter((it) => it.kind === "doing").map((it) => it.id),
    ["d0", "d1", "d2", "d3", "d4", "d5"],
  );
  assert.equal(body.findIndex((it) => it.kind === "todo"), 6);
  assert.equal(todoCap[todoCap.length - 1].kind, "more");

  const needed = Array.from({ length: 7 }, (_, i) =>
    card({ id: `n${i}`, projectId: pid, kind: "needed", neededState: "open", createdAt: i, status: "todo" }),
  );
  const shown = archLayer2Items({ pid, cards: needed, rows: [], maxWindows: 1, maxTodos: 1 });
  assert.equal(shown.filter((it) => it.kind === "needed").length, 7);
});

test("S6 confirm", () => {
  const win = (sleeping: boolean, minimized: boolean): Layer2Item => ({
    kind: "window",
    stop: "rl2-w-1",
    id: "1",
    sleeping,
    minimized,
  });
  assert.equal(archConfirm(win(false, false)), "focus");
  assert.equal(archConfirm(win(false, true)), "restore");
  assert.equal(archConfirm(win(true, true)), "wake");
  for (const kind of ["todo", "doing", "needed", "more", "board"] as const) {
    assert.equal(archConfirm({ kind, stop: "s" }), "board");
  }
});

test("S7 recede depth", () => {
  const front = archDepth("arch", 1, false);
  assert.equal(front.scale, 1);
  assert.equal(front.opacity, 1);
  assert.equal(front.blur, 0);
  const front2 = archDepth("layer2", 2, false);
  assert.equal(front2.scale, 1);
  assert.equal(front2.opacity, 1);
  assert.equal(front2.blur, 0);
  const rec = archDepth("layer2", 1, false);
  assert.ok(rec.scale <= 0.9);
  assert.ok(rec.opacity <= 0.6);
  assert.ok(rec.blur >= 2 && rec.blur <= 6);
  const rec2 = archDepth("board", 2, false);
  assert.ok(rec2.scale <= 0.9 && rec2.opacity <= 0.6 && rec2.blur >= 2 && rec2.blur <= 6);
  const reduced = archDepth("layer2", 1, true);
  assert.equal(reduced.blur, 0);
  assert.equal(reduced.durationMs, 0);
  assert.equal(reduced.scale, rec.scale);
  assert.equal(reduced.opacity, rec.opacity);
  assert.ok(front.durationMs >= 240 && front.durationMs <= 480);
  assert.ok(rec.durationMs >= 240 && rec.durationMs <= 480);
});

test("S8 view-only guard", () => {
  const src = readFileSync(new URL("../src/lib/rail/stageArch.ts", import.meta.url), "utf8");
  for (const banned of ["@tauri-apps", "invoke(", "persist", "save_state", "collapsedSectionIds"]) {
    assert.equal(src.includes(banned), false, banned);
  }
  const frozen = Object.freeze({
    front: "layer2" as const,
    pid: "a",
    center2: 1,
    pending: Object.freeze({ zone: "arch" as const, since: 3 }),
  });
  const before = JSON.stringify(frozen);
  const item = planet("b");
  const layer2 = Object.freeze([
    Object.freeze({ kind: "board" as const, stop: "rl2-board" }),
    Object.freeze({ kind: "needed" as const, stop: "rl2-n-1", id: "1" }),
  ]);
  const outs = [
    archEnter(frozen, item, layer2),
    archBack(frozen),
    archForward(frozen),
    archCentreChanged(frozen, planet("c")),
    archOpenBoard(frozen),
    archScroll2(frozen, 1, 4),
    archPointer(frozen, "arch", 3),
    archInitial([item], 0, 1),
  ];
  const arrow = archArrow(frozen, [item, { stop: "rstars", kind: "stars" }], 0, "down", "left", layer2);
  assert.ok(arrow);
  outs.push(arrow.state);
  for (const out of outs) {
    assert.notEqual(out, frozen);
    assert.equal(JSON.stringify(frozen), before);
  }
  assert.equal(JSON.stringify(layer2), JSON.stringify([
    { kind: "board", stop: "rl2-board" },
    { kind: "needed", stop: "rl2-n-1", id: "1" },
  ]));
});

test("S9 layout fits", () => {
  for (const edge of ["left", "right", "top"] as const) {
    for (const [w, h] of [
      [1040, 760],
      [1000, 700],
    ] as const) {
      const lay = archLayout(edge, w, h);
      const preview = wheelLayout(edge, w, h).preview;
      for (const rect of [lay.arch, lay.layer2, lay.board]) {
        assert.ok(rect.x >= 0 && rect.y >= 0 && rect.w >= 0 && rect.h >= 0, edge);
        assert.ok(rect.x + rect.w <= w + 1e-6, `${edge} x`);
        assert.ok(rect.y + rect.h <= h + 1e-6, `${edge} y`);
      }
      assert.deepEqual(lay.board, preview);
      if (edge === "left") assert.ok(lay.layer2.x >= lay.arch.x + lay.arch.w - 1e-6);
      if (edge === "right") assert.ok(lay.layer2.x + lay.layer2.w <= lay.arch.x + 1e-6);
      if (edge === "top") assert.ok(lay.layer2.y >= lay.arch.y + lay.arch.h - 1e-6);
    }
  }
});

test("S10 keyboard", () => {
  const items = spinItems(["a", "b"], U);
  const centre = items.findIndex((it) => it.kind === "planet" && it.pid === "a");
  const layer2 = archLayer2Items({
    pid: "a",
    cards: [
      card({ id: "n", projectId: "a", kind: "needed", neededState: "open", createdAt: 1, title: "n" }),
      card({ id: "d", projectId: "a", kind: "todo", status: "doing", createdAt: 2, title: "d" }),
      card({ id: "t", projectId: "a", kind: "todo", status: "todo", createdAt: 3, title: "t" }),
    ],
    rows: [{ id: "w" }],
  });
  assert.equal(layer2[0].kind, "board");
  for (const edge of edges) {
    const ax = axis(edge);
    const inward = archArrow(ARCH_IDLE, items, centre, ax.inward, edge, layer2);
    assert.ok(inward, edge);
    assert.equal(inward.state.front, "layer2");
    assert.equal(inward.state.pid, "a");
    const onBoard: ArchState = { front: "layer2", pid: "a", center2: 0, pending: null };
    const opened = archArrow(onBoard, items, centre, ax.inward, edge, layer2);
    assert.ok(opened, edge);
    assert.equal(opened.state.front, "board", edge);
    assert.equal(opened.walkIn, "board", edge);
    assert.equal(opened.state.pid, "a", edge);
    const boardFront: ArchState = { front: "board", pid: "a", center2: 0, pending: null };
    const back1 = archArrow(boardFront, items, centre, ax.outward, edge, layer2);
    assert.ok(back1);
    assert.equal(back1.state.front, "layer2");
    assert.equal(back1.state.pid, "a");
    const back2 = archArrow(back1.state, items, centre, ax.outward, edge, layer2);
    assert.ok(back2);
    assert.equal(back2.state.front, "arch");
    assert.equal(back2.state.pid, "a");

    const last = items.length - 1;
    assert.equal(
      archArrow(ARCH_IDLE, items, last, ax.forward, edge, layer2)?.stop,
      spinArrow(items, items[last].stop, ax.forward, edge),
    );
    assert.equal(
      archArrow(ARCH_IDLE, items, 0, ax.back, edge, layer2)?.stop,
      spinArrow(items, items[0].stop, ax.back, edge),
    );

    const end: ArchState = { front: "layer2", pid: "a", center2: layer2.length - 1, pending: null };
    const fwd = archArrow(end, items, centre, ax.forward, edge, layer2);
    assert.ok(fwd);
    assert.equal(fwd.state.front, "layer2");
    assert.equal(fwd.state.center2, 0, edge);
    const start: ArchState = { front: "layer2", pid: "a", center2: 0, pending: null };
    const backed = archArrow(start, items, centre, ax.back, edge, layer2);
    assert.ok(backed);
    assert.equal(backed.state.front, "layer2", edge);
    assert.equal(backed.state.center2, layer2.length - 1, edge);
    const out = archArrow(start, items, centre, ax.outward, edge, layer2);
    assert.ok(out);
    assert.equal(out.state.front, "arch", edge);

    const seen = new Set<number>();
    const queue = [0];
    let reachedArch = false;
    while (queue.length) {
      const i = queue.shift() as number;
      if (seen.has(i)) continue;
      seen.add(i);
      const state: ArchState = { front: "layer2", pid: "a", center2: i, pending: null };
      for (const dir of dirs) {
        const next = archArrow(state, items, centre, dir, edge, layer2);
        if (!next) continue;
        if (next.state.front === "arch") reachedArch = true;
        if (next.stop) {
          const at = layer2.findIndex((it) => it.stop === next.stop);
          if (at >= 0) queue.push(at);
        }
      }
    }
    assert.equal(seen.size, layer2.length, edge);
    assert.equal(reachedArch, true, edge);
  }
  const scrolled = archScroll2({ front: "layer2", pid: "a", center2: 0, pending: null }, -1, 5);
  assert.equal(scrolled.front, "layer2");
  assert.equal(scrolled.pid, "a");
  assert.equal(scrolled.center2, 4);
  const wrapped = archScroll2({ front: "layer2", pid: "a", center2: 4, pending: null }, 1, 5);
  assert.equal(wrapped.front, "layer2");
  assert.equal(wrapped.center2, 0);
  const spun = archScroll2({ front: "layer2", pid: "a", center2: 1, pending: null }, 9, 4);
  assert.equal(spun.front, "layer2");
  assert.equal(spun.center2, 2);

  const stale: ArchState = { front: "arch", pid: "a", center2: 0, pending: null };
  assert.equal(archFocusPid(stale, planet("b")), "b");
  const pidNull: ArchState = { front: "arch", pid: null, center2: 0, pending: null };
  assert.equal(archFocusPid(pidNull, planet("b")), "b");
  assert.equal(archFocusPid(pidNull, { stop: `rhead-${U}`, kind: "unmapped", pid: U }), U);
  assert.equal(archFocusPid(pidNull, { stop: "rstars", kind: "stars" }), null);
  const onLayer: ArchState = { front: "layer2", pid: "a", center2: 1, pending: null };
  assert.equal(archFocusPid(onLayer, planet("b")), "a");
  const onBoardFront: ArchState = { front: "board", pid: "a", center2: 1, pending: null };
  assert.equal(archFocusPid(onBoardFront, planet("b")), "a");
  const centredList = archLayer2Items({
    pid: "b",
    cards: [card({ id: "nb", projectId: "b", kind: "needed", neededState: "open", createdAt: 1, title: "n" })],
    rows: [],
  });
  const centreB = items.findIndex((it) => it.kind === "planet" && it.pid === "b");
  const inwardB = archArrow(stale, items, centreB, "right", "left", centredList);
  assert.ok(inwardB);
  assert.equal(inwardB.state.front, "layer2");
  assert.equal(inwardB.state.pid, "b");
  assert.equal(inwardB.state.center2, mirrorStart(centredList));
  assert.equal(centredList[inwardB.state.center2]?.kind, "board");
});

test("S11 pointer dwell + fresh", () => {
  assert.ok(ARCH_DWELL_MS >= 120 && ARCH_DWELL_MS <= 240);
  const layer: ArchState = { front: "layer2", pid: "a", center2: 1, pending: null };
  const t = 5000;
  const early = archPointer(layer, "arch", t);
  assert.equal(early.front, "layer2");
  assert.equal(early.pending?.zone, "arch");
  assert.equal(early.pending?.since, t);
  const mid = archPointer(early, "arch", t + ARCH_DWELL_MS - 1);
  assert.equal(mid.front, "layer2");
  const fired = archPointer(early, "arch", t + ARCH_DWELL_MS);
  assert.equal(fired.front, "layer2");
  assert.equal(fired.pid, "a");
  assert.equal(fired.pending?.zone, "arch");
  const left = archPointer(early, "layer2", t + ARCH_DWELL_MS);
  assert.equal(left.front, "layer2");
  assert.equal(left.pending, null);
  const gap = archPointer(early, "none", t + 10_000);
  assert.equal(gap.front, "layer2");
  assert.equal(gap.pending, null);
  const direct = archPointer(layer, "none", t + 10_000);
  assert.equal(direct.front, "layer2");
  const withPid: ArchState = { front: "arch", pid: "a", center2: 2, pending: null };
  const arm = archPointer(withPid, "layer2", 100);
  const back = archPointer(arm, "layer2", 100 + ARCH_DWELL_MS);
  assert.equal(back.front, "arch");
  assert.equal(back.pid, "a");
  const bare: ArchState = { front: "arch", pid: null, center2: 0, pending: null };
  const armBare = archPointer(bare, "layer2", 100);
  const stay = archPointer(armBare, "layer2", 100 + ARCH_DWELL_MS + 80);
  assert.equal(stay.front, "arch");
  assert.equal(stay.pid, null);

  const freshItems = spinItems([], U);
  const freshAt = spinInitial(freshItems, null);
  const fresh = archInitial(freshItems, freshAt, 0);
  assert.equal(fresh.front, "layer2");
  assert.equal(fresh.pid, U);
  const veteran = spinItems(["a"], U);
  const vetAt = spinInitial(veteran, null);
  const idle = archInitial(veteran, vetAt, 1);
  assert.deepEqual(idle, { front: "arch", pid: null, center2: 0, pending: null });
});
