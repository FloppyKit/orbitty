import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { sampleProofs } from "../src/lib/lifesprite/fixtures.ts";
import {
  KIND_DOING,
  KIND_DONE,
  KIND_TODO,
  foldBoard,
  type BoardProof,
} from "../src/lib/lifesprite/fold.ts";
import { proof } from "../src/lib/lifesprite/fixtures.ts";

function titles(events: BoardProof[], projectId: string, column: "todo" | "doing" | "done") {
  const board = foldBoard(events);
  const project = board.find((p) => p.projectId === projectId);
  assert.ok(project, `missing project ${projectId}`);
  return project[column].map((c) => ({ cardId: c.cardId, title: c.title }));
}

test("sample lines fold into columns and keep the mint title", () => {
  const board = foldBoard(sampleProofs());
  assert.deepEqual(
    board.map((p) => p.projectId),
    ["p-kitchen", "p-yard"],
  );
  assert.deepEqual(titles(sampleProofs(), "p-kitchen", "todo"), [
    { cardId: "c-bread", title: "Slice the bread" },
  ]);
  assert.deepEqual(titles(sampleProofs(), "p-kitchen", "doing"), [
    { cardId: "c-bowls", title: "Warm the bowls" },
  ]);
  assert.deepEqual(titles(sampleProofs(), "p-kitchen", "done"), []);
  assert.deepEqual(titles(sampleProofs(), "p-yard", "doing"), [
    { cardId: "c-step", title: "Sweep the step" },
  ]);
  assert.deepEqual(titles(sampleProofs(), "p-yard", "done"), [
    { cardId: "c-rake", title: "c-rake" },
  ]);
  const ids = board.flatMap((p) => [...p.todo, ...p.doing, ...p.done].map((c) => c.cardId));
  assert.equal(ids.includes("c-nch"), false);
  assert.equal(
    board.some((p) =>
      [...p.todo, ...p.doing, ...p.done].some((c) => c.title === "Renamed bowls"),
    ),
    false,
  );
});

test("a move that arrives before its mint keeps the mint title and the later column", () => {
  const mint = proof({
    kind: KIND_TODO,
    created_at: 10,
    cardId: "c-1",
    project: "p-1",
    content: { cardId: "c-1", title: "First title" },
  });
  const move = proof({
    kind: KIND_DOING,
    created_at: 20,
    cardId: "c-1",
    project: "p-1",
    extraTags: [["e", mint.id ?? ""]],
    content: { from: "todo", to: "doing", cardId: "c-1" },
  });
  const board = foldBoard([move, mint]);
  assert.equal(board.length, 1);
  assert.deepEqual(board[0]?.todo, []);
  assert.deepEqual(board[0]?.doing, [{ cardId: "c-1", title: "First title" }]);
});

test("a move for an unknown card shows the card id", () => {
  const move = proof({
    kind: KIND_DONE,
    created_at: 5,
    cardId: "c-missing",
    project: "p-1",
    content: { from: "doing", to: "done", cardId: "c-missing" },
  });
  const board = foldBoard([move]);
  assert.deepEqual(board[0]?.done, [{ cardId: "c-missing", title: "c-missing" }]);
  assert.equal(JSON.parse(move.content ?? "{}").title, undefined);
});

test("kind 31200 is not replayed into a column or a title", () => {
  const mint = proof({
    kind: KIND_TODO,
    created_at: 1,
    cardId: "c-1",
    project: "p-1",
    content: { cardId: "c-1", title: "Keep me" },
  });
  const runtime = proof({
    kind: 31200,
    created_at: 2,
    cardId: "c-1",
    t: "local-only",
    project: "p-1",
    extraTags: [["e-type", "board-delete"]],
    content: { cardId: "c-1", title: "Gone" },
  });
  const other = proof({
    kind: 31200,
    created_at: 3,
    cardId: "c-only-runtime",
    t: "local-only",
    extraTags: [["e-type", "needed-open"]],
    content: { cardId: "c-only-runtime", title: "Needs a person" },
  });
  const taggedOrbitty = proof({
    kind: 31200,
    created_at: 4,
    cardId: "c-weird",
    project: "p-1",
    content: { cardId: "c-weird", title: "Still not a column" },
  });
  const board = foldBoard([runtime, other, taggedOrbitty, mint]);
  assert.equal(board.length, 1);
  assert.deepEqual(board[0]?.todo, [{ cardId: "c-1", title: "Keep me" }]);
});

test("a later title on another 420 does not replace the mint title", () => {
  const first = proof({
    kind: KIND_TODO,
    created_at: 1,
    cardId: "c-1",
    project: "p-1",
    content: { cardId: "c-1", title: "Minted" },
  });
  const second = proof({
    kind: KIND_TODO,
    created_at: 9,
    cardId: "c-1",
    project: "p-1",
    content: { cardId: "c-1", title: "Edited later" },
  });
  const board = foldBoard([second, first]);
  assert.deepEqual(board[0]?.todo, [{ cardId: "c-1", title: "Minted" }]);
});

test("out-of-order lines follow created_at, and projects stay apart", () => {
  const late = proof({
    kind: KIND_DONE,
    created_at: 30,
    cardId: "c-a",
    project: "p-b",
    content: { from: "doing", to: "done", cardId: "c-a" },
  });
  const earlyMint = proof({
    kind: KIND_TODO,
    created_at: 1,
    cardId: "c-a",
    project: "p-b",
    content: { cardId: "c-a", title: "Alpha" },
  });
  const other = proof({
    kind: KIND_TODO,
    created_at: 2,
    cardId: "c-b",
    project: "p-a",
    content: { cardId: "c-b", title: "Beta" },
  });
  const mid = proof({
    kind: KIND_DOING,
    created_at: 15,
    cardId: "c-a",
    project: "p-b",
    content: { from: "todo", to: "doing", cardId: "c-a" },
  });
  const board = foldBoard([late, other, mid, earlyMint]);
  assert.deepEqual(
    board.map((p) => p.projectId),
    ["p-b", "p-a"],
  );
  const pb = board[0];
  assert.deepEqual(pb?.todo, []);
  assert.deepEqual(pb?.doing, []);
  assert.deepEqual(pb?.done, [{ cardId: "c-a", title: "Alpha" }]);
  assert.deepEqual(board[1]?.todo, [{ cardId: "c-b", title: "Beta" }]);
});

test("a doing mint lands in Doing, and a card with no project tag is its own group", () => {
  const doingMint = proof({
    kind: KIND_DOING,
    created_at: 4,
    cardId: "c-hot",
    content: { cardId: "c-hot", title: "Already going" },
  });
  deleteProject(doingMint);
  const board = foldBoard([doingMint]);
  assert.equal(board[0]?.projectId, null);
  assert.deepEqual(board[0]?.doing, [{ cardId: "c-hot", title: "Already going" }]);
});

test("t=nch and a proof with no card id are skipped", () => {
  const nch = proof({
    kind: KIND_TODO,
    created_at: 1,
    cardId: "c-x",
    t: "nch",
    content: { cardId: "c-x", title: "Other chain" },
  });
  const blank: BoardProof = {
    id: "1".repeat(64),
    pubkey: "ab".repeat(32),
    created_at: 2,
    kind: KIND_TODO,
    tags: [["t", "orbitty"]],
    content: "{}",
    sig: "cd".repeat(64),
  };
  assert.deepEqual(foldBoard([nch, blank]), []);
});

function deleteProject(ev: BoardProof) {
  ev.tags = (ev.tags ?? []).filter((tag) => tag[0] !== "project");
}

test("lifesprite sources do not store an nsec or call the Mac save path", () => {
  const lib = fileURLToPath(new URL("../src/lib/lifesprite/", import.meta.url));
  const page = fileURLToPath(new URL("../src/routes/lifesprite/+page.svelte", import.meta.url));
  const files = fs.readdirSync(lib).map((name) => path.join(lib, name));
  files.push(page);
  const banned = [
    "localStorage",
    "sessionStorage",
    "indexedDB",
    "save_state",
    "event_keys",
    "habitat_mac",
    "@tauri-apps",
    "console.log",
    "console.debug",
    "writeFile",
  ];
  for (const file of files) {
    const text = fs.readFileSync(file, "utf8");
    for (const word of banned) {
      assert.equal(text.includes(word), false, `${path.basename(file)} mentions ${word}`);
    }
  }
});
