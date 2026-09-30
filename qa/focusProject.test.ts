import assert from "node:assert/strict";
import fs from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { resolveFocus } from "../src/lib/board/focusProject.ts";

const planets = ["A", "B"];
const kanbanPath = fileURLToPath(
  new URL("../src/lib/KanbanPage.svelte", import.meta.url),
);

/** Drop // and block comments so a commented-out line cannot satisfy a pin.
 * Strings and template literals are left intact, including their interpolations. */
function stripComments(src: string): string {
  let out = "";
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    const next = src[i + 1];
    if (c === "/" && next === "/") {
      while (i < src.length && src[i] !== "\n") i++;
      continue;
    }
    if (c === "/" && next === "*") {
      i += 2;
      while (i < src.length && !(src[i] === "*" && src[i + 1] === "/")) i++;
      i = Math.min(src.length, i + 2);
      continue;
    }
    if (c === '"' || c === "'" || c === "`") {
      const end = skipString(src, i);
      out += src.slice(i, end);
      i = end;
      continue;
    }
    out += c;
    i++;
  }
  return out;
}

function skipString(src: string, i: number): number {
  const q = src[i];
  i++;
  while (i < src.length) {
    if (src[i] === "\\") {
      i += 2;
      continue;
    }
    if (q === "`" && src[i] === "$" && src[i + 1] === "{") {
      i += 2;
      let depth = 1;
      while (i < src.length && depth > 0) {
        if (src[i] === '"' || src[i] === "'" || src[i] === "`") {
          i = skipString(src, i);
          continue;
        }
        if (src[i] === "/" && src[i + 1] === "/") {
          while (i < src.length && src[i] !== "\n") i++;
          continue;
        }
        if (src[i] === "/" && src[i + 1] === "*") {
          i += 2;
          while (i < src.length && !(src[i] === "*" && src[i + 1] === "/")) i++;
          i = Math.min(src.length, i + 2);
          continue;
        }
        if (src[i] === "{") depth++;
        else if (src[i] === "}") depth--;
        i++;
      }
      continue;
    }
    if (src[i] === q) return i + 1;
    i++;
  }
  return i;
}

function braceBody(src: string, openIndex: number): string {
  if (src[openIndex] !== "{") {
    throw new Error(`expected { at ${openIndex}`);
  }
  let depth = 0;
  let i = openIndex;
  while (i < src.length) {
    const c = src[i];
    if (c === '"' || c === "'" || c === "`") {
      i = skipString(src, i);
      continue;
    }
    if (c === "{") depth++;
    else if (c === "}") {
      depth--;
      if (depth === 0) return src.slice(openIndex + 1, i);
    }
    i++;
  }
  throw new Error("unclosed brace");
}

function functionBody(src: string, name: string): string {
  const re = new RegExp(`(?:async\\s+)?function\\s+${name}\\s*\\(`);
  const m = re.exec(src);
  if (!m) throw new Error(`missing function ${name}`);
  let i = m.index + m[0].length;
  let depth = 1;
  while (i < src.length && depth > 0) {
    if (src[i] === "(") depth++;
    else if (src[i] === ")") depth--;
    i++;
  }
  while (i < src.length && src[i] !== "{") i++;
  return braceBody(src, i);
}

function arrowBody(src: string, marker: string): string {
  const at = src.indexOf(marker);
  assert.ok(at >= 0, `missing ${marker}`);
  const brace = src.indexOf("{", at);
  assert.ok(brace >= 0, `missing body for ${marker}`);
  return braceBody(src, brace);
}

/** True when `index` sits in an `if (!embedded) { ... }` block. */
function indexInsideNotEmbedded(src: string, index: number): boolean {
  const re = /if\s*\(\s*!embedded\s*\)\s*\{/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(src))) {
    const open = m.index + m[0].length - 1;
    const body = braceBody(src, open);
    const start = open + 1;
    const end = start + body.length;
    if (index >= start && index < end) return true;
  }
  return false;
}

function kanban(): string {
  return stripComments(fs.readFileSync(kanbanPath, "utf8"));
}

test("stale pending A + manual pick B + reload stays on B", () => {
  // Board click A is one-shot once A is in the project list.
  const afterA = resolveFocus({
    pending: "A",
    selected: "inbox",
    projectIds: planets,
  });
  assert.equal(afterA.selected, "A");
  assert.equal(afterA.pending, null);

  // Manual planet-tab pick clears pending, then a reload must stay on B.
  const picked = resolveFocus({
    pending: null,
    selected: "B",
    projectIds: planets,
  });
  assert.equal(picked.selected, "B");
  assert.equal(picked.pending, null);

  const reloaded = resolveFocus({
    pending: picked.pending,
    selected: picked.selected,
    projectIds: planets,
  });
  assert.equal(reloaded.selected, "B");
  assert.equal(reloaded.pending, null);
});

test("board click B after A selects B", () => {
  const afterA = resolveFocus({
    pending: "A",
    selected: "inbox",
    projectIds: planets,
  });
  const afterB = resolveFocus({
    pending: "B",
    selected: afterA.selected,
    projectIds: planets,
  });
  assert.equal(afterB.selected, "B");
  assert.equal(afterB.pending, null);

  const reloaded = resolveFocus({
    pending: afterB.pending,
    selected: afterB.selected,
    projectIds: planets,
  });
  assert.equal(reloaded.selected, "B");
  assert.equal(reloaded.pending, null);
});

test("pending id missing from projects stays until it appears, then clears", () => {
  const waiting = resolveFocus({
    pending: "A",
    selected: "inbox",
    projectIds: [],
  });
  assert.equal(waiting.selected, "A");
  assert.equal(waiting.pending, "A");

  const landed = resolveFocus({
    pending: waiting.pending,
    selected: waiting.selected,
    projectIds: planets,
  });
  assert.equal(landed.selected, "A");
  assert.equal(landed.pending, null);
});

test("P1 K1 loadState calls applyPendingFocus and does not assign selected from pending", () => {
  const src = kanban();
  assert.equal(src.includes("selected = pendingFocusProject"), false);
  const body = functionBody(src, "loadState");
  assert.match(body, /applyPendingFocus\(\s*\)/);
});

test("P2 K5 kanban pane-show calls applyPendingFocus", () => {
  const src = kanban();
  const marker = 'listenPaneVis("kanban"';
  const at = src.indexOf(marker);
  assert.ok(at >= 0, marker);
  const afterMarker = src.slice(at + marker.length);
  const next = afterMarker.search(/\blisten(?:PaneVis)?\s*(?:<[^>]*>)?\s*\(/);
  const region = next === -1 ? afterMarker : afterMarker.slice(0, next);
  assert.match(region, /applyPendingFocus\(\s*\)/);
  const cb = arrowBody(src, marker);
  assert.match(cb, /applyPendingFocus\(\s*\)/);
});

test("P3 K2 K3 take_board_focus follows the focus listener and feeds applyFocusProject", () => {
  const src = kanban();
  const listenAt = src.indexOf('listen<string>("orbitty:focus-project"');
  const needle = 'invoke<string | null>("take_board_focus")';
  const invokeAt = src.indexOf(needle);
  assert.ok(listenAt >= 0, "focus-project listener");
  assert.ok(invokeAt > listenAt, "typed take_board_focus after the listener");
  assert.equal(src.split(needle).length - 1, 1, "one typed take_board_focus");
  assert.match(src.slice(invokeAt, invokeAt + 220), /applyFocusProject\s*\(/);
});

test("P4 K4 onPlanetTabClick clears pending focus", () => {
  const body = functionBody(kanban(), "onPlanetTabClick");
  assert.match(body, /pendingFocusProject\s*=\s*null/);
});

test("N3 listener consumes the slot and embedded skips the mount take", () => {
  const src = kanban();
  const cb = arrowBody(src, 'listen<string>("orbitty:focus-project"');
  const consumeAt = cb.indexOf('void invoke("take_board_focus")');
  assert.ok(consumeAt >= 0, "listener consumes take_board_focus");
  assert.equal(
    indexInsideNotEmbedded(cb, consumeAt),
    true,
    "only the kanban window consumes the slot",
  );

  const mountAt = src.indexOf('invoke<string | null>("take_board_focus")');
  assert.ok(mountAt >= 0, "mount take");
  assert.equal(
    indexInsideNotEmbedded(src, mountAt),
    true,
    "embedded instances skip the mount take",
  );
  assert.match(src.slice(mountAt, mountAt + 220), /applyFocusProject\s*\(/);
});

test("EDGE ghost pending drops when the loaded list lacks the id", () => {
  const ghost = resolveFocus({
    pending: "X",
    selected: "inbox",
    projectIds: planets,
    projectsLoaded: true,
  });
  assert.equal(ghost.pending, null);
  assert.equal(ghost.selected, "inbox");

  // Inbox pick after the drop, then a reload, stays on Inbox.
  const picked = resolveFocus({
    pending: ghost.pending,
    selected: "inbox",
    projectIds: planets,
    projectsLoaded: true,
  });
  assert.equal(picked.selected, "inbox");
  assert.equal(picked.pending, null);

  const reloaded = resolveFocus({
    pending: picked.pending,
    selected: picked.selected,
    projectIds: planets,
    projectsLoaded: true,
  });
  assert.equal(reloaded.selected, "inbox");
  assert.equal(reloaded.pending, null);

  const apply = functionBody(kanban(), "applyPendingFocus");
  assert.match(apply, /projectsLoaded:\s*projects\.length\s*>\s*0/);
});

test("not loaded yet keeps pending until the list contains it", () => {
  const waiting = resolveFocus({
    pending: "B",
    selected: "inbox",
    projectIds: [],
  });
  assert.equal(waiting.selected, "B");
  assert.equal(waiting.pending, "B");

  const landed = resolveFocus({
    pending: waiting.pending,
    selected: waiting.selected,
    projectIds: planets,
    projectsLoaded: true,
  });
  assert.equal(landed.selected, "B");
  assert.equal(landed.pending, null);

  // Event / mount take runs before the first loadState. It must not treat
  // an empty list as a loaded one.
  const eventPath = functionBody(kanban(), "applyFocusProject");
  assert.doesNotMatch(eventPath, /projectsLoaded/);
});
