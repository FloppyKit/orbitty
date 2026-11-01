import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  blockedLabel,
  isBlocked,
  openBlockerIds,
  waitingTitle,
  type BlockerCard,
} from "../src/lib/board/blockers.ts";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

const cards: BlockerCard[] = [
  { id: "c", title: "Waiter", status: "todo", kind: "todo" },
  { id: "a", title: "Alpha", status: "todo", kind: "todo" },
  { id: "b", title: "Beta", status: "doing", kind: "todo" },
  { id: "d", title: "Done one", status: "done", kind: "todo" },
];

test("isBlocked counts todo and doing blockers", () => {
  assert.equal(isBlocked({ c: ["a"] }, cards, cards[0]), true);
  assert.equal(isBlocked({ c: ["b"] }, cards, cards[0]), true);
  assert.deepEqual(openBlockerIds({ c: ["a", "b"] }, cards, "c"), ["a", "b"]);
});

test("done and missing blockers are clear", () => {
  assert.equal(isBlocked({ c: ["d"] }, cards, cards[0]), false);
  assert.equal(isBlocked({ c: ["missing"] }, cards, cards[0]), false);
  assert.equal(isBlocked({ c: ["d", "missing"] }, cards, cards[0]), false);
  assert.deepEqual(openBlockerIds({ c: ["d", "missing", "a"] }, cards, "c"), ["a"]);
  assert.equal(isBlocked({}, cards, cards[0]), false);
});

test("blockedLabel text", () => {
  assert.equal(blockedLabel(1), "Blocked · 1");
  assert.equal(blockedLabel(2), "Blocked · 2");
  assert.equal(waitingTitle({ c: ["a", "b"] }, cards, "c"), "Waiting on: Alpha, Beta");
});

test("the row chip and the doing drag gate", () => {
  const col = src("src/lib/board/Columns.svelte");
  assert.match(col, /class="b-blocked"/);
  assert.match(col, /card\.status !== "done"/);
  assert.match(col, /blockedLabel\(/);
  assert.match(col, /waitingTitle\(/);
  const css = src("src/lib/board/board.css");
  assert.match(css, /\.b-blocked \{/);
  const file = src("src/lib/board/pageMutate.svelte.ts");
  const start = file.indexOf("export function moveCard(");
  assert.notEqual(start, -1);
  const next = file.indexOf("\nexport function ", start + 10);
  const body = file.slice(start, next === -1 ? undefined : next);
  const done = body.indexOf('target === "done"');
  const gate = body.indexOf('target === "doing"');
  assert.ok(done !== -1 && gate > done, "done still completes; doing is the gate");
  assert.match(body, /isBlocked\(/);
  assert.match(body, /return;/);
});
