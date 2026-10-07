import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { parse } from "svelte/compiler";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

type AstNode = {
  type?: string;
  name?: string;
  start?: number;
  end?: number;
  data?: string;
  attributes?: AstNode[];
  fragment?: AstNode;
  nodes?: AstNode[];
  consequent?: AstNode;
  alternate?: AstNode | null;
  body?: AstNode;
  fallback?: AstNode;
  pending?: AstNode | null;
  then?: AstNode | null;
  catch?: AstNode | null;
  test?: AstNode;
  expression?: AstNode;
  [key: string]: unknown;
};

const ATTR_TYPES = new Set([
  "Attribute",
  "BindDirective",
  "ClassDirective",
  "SpreadAttribute",
  "StyleDirective",
  "LetDirective",
  "OnDirective",
  "TransitionDirective",
  "AnimateDirective",
  "UseDirective",
  "AttachTag",
]);

function childrenOf(node: AstNode): AstNode[] {
  const out: AstNode[] = [];
  for (const key of [
    "fragment",
    "nodes",
    "consequent",
    "alternate",
    "body",
    "fallback",
    "pending",
    "then",
    "catch",
    "attributes",
  ]) {
    const v = node[key];
    if (!v) continue;
    if (Array.isArray(v)) out.push(...(v as AstNode[]));
    else if (typeof v === "object") out.push(v as AstNode);
  }
  return out;
}

function walk(node: AstNode | null | undefined, fn: (node: AstNode, parent: AstNode | null) => void, parent: AstNode | null = null) {
  if (!node || typeof node !== "object") return;
  fn(node, parent);
  for (const child of childrenOf(node)) walk(child, fn, node);
}

function classTokens(el: AstNode): string[] {
  const names: string[] = [];
  for (const attr of el.attributes ?? []) {
    if (attr.type === "Attribute" && attr.name === "class") {
      const value = attr.value as unknown;
      const parts = Array.isArray(value) ? value : value ? [value] : [];
      for (const part of parts as AstNode[]) {
        if (part.type === "Text" && typeof part.data === "string") {
          names.push(...part.data.split(/\s+/).filter(Boolean));
        }
      }
    }
    if (attr.type === "ClassDirective" && typeof attr.name === "string") names.push(attr.name);
  }
  return names;
}

function hasClass(el: AstNode, token: string): boolean {
  return classTokens(el).includes(token);
}

function fragmentOf(file: string): AstNode {
  const ast = parse(file, { modern: true }) as unknown as AstNode;
  assert.equal(ast.fragment?.type, "Fragment");
  return ast.fragment as AstNode;
}

function paintedListTitle(file: string): number {
  const root = fragmentOf(file);
  let n = 0;
  walk(root, (node, parent) => {
    if (node.type !== "ExpressionTag") return;
    if (parent && ATTR_TYPES.has(parent.type ?? "")) return;
    const slice = file.slice(node.start ?? 0, node.end ?? 0);
    if (slice.includes("listTitle")) n += 1;
  });
  return n;
}

function banner(file: string): AstNode | null {
  const root = fragmentOf(file);
  let found: AstNode | null = null;
  walk(root, (node) => {
    if (node.type === "RegularElement" && hasClass(node, "k-banner")) found = node;
  });
  return found;
}

function textUnder(node: AstNode): string {
  let text = "";
  walk(node, (n) => {
    if (n.type === "Text" && typeof n.data === "string") text += n.data;
  });
  return text;
}

function hasDescendant(node: AstNode, pred: (n: AstNode) => boolean): boolean {
  let ok = false;
  walk(node, (n) => {
    if (pred(n)) ok = true;
  });
  return ok;
}

test("G1 Chrome has no title-row classes", () => {
  const file = src("src/lib/board/Chrome.svelte");
  const root = fragmentOf(file);
  const banned = ["k-heads", "k-titlerow", "k-title", "planet-chip"];
  walk(root, (node) => {
    if (node.type !== "RegularElement" && node.type !== "Component") return;
    for (const token of classTokens(node)) {
      assert.equal(banned.includes(token), false, token);
    }
  });
});

test("G2b list title once via WindowTitleRow; no Orbitty Lists; embedded banner remains", () => {
  const chrome = src("src/lib/board/Chrome.svelte");
  const kanban = src("src/lib/KanbanPage.svelte");
  const row = src("src/lib/WindowTitleRow.svelte");
  assert.equal(chrome.includes("Orbitty Lists"), false);
  assert.equal(kanban.includes("Orbitty Lists"), false);
  assert.equal(chrome.includes("data-tauri-drag-region"), false);
  const titles = chrome.match(/title=\{listTitle\}/g) ?? [];
  assert.equal(titles.length, 1);
  assert.match(chrome, /<WindowTitleRow[\s\S]*title=\{listTitle\}/);
  const bar = banner(chrome);
  assert.ok(bar, "k-banner");
  assert.equal(textUnder(bar).includes("Launchpad"), true);
  assert.equal(textUnder(bar).includes("Orbitty Lists"), false);
  assert.equal(
    hasDescendant(bar, (n) => n.type === "RegularElement" && hasClass(n, "winx")),
    true,
    "embedded winx",
  );
  const drag = (bar.attributes ?? []).some(
    (attr) => attr.type === "Attribute" && attr.name === "onmousedown",
  );
  assert.equal(drag, true);
  assert.match(row, /onmousedown=\{onRowMouseDown\}/);
  assert.match(row, /startPaneDrag/);
  assert.equal(row.includes("data-tauri-drag-region"), false);
  assert.match(row, /\{#if mode === "custom"\}/);
});

test("G3 Done actor chips survive", () => {
  const file = src("src/lib/board/Chrome.svelte");
  const root = fragmentOf(file);
  let rendered = false;
  walk(root, (node) => {
    if (node.type === "RenderTag" && file.slice(node.start ?? 0, node.end ?? 0).includes("doneActorChips")) {
      rendered = true;
    }
  });
  assert.equal(rendered, true);
});

test("G4 rail mode renders neither PlanetColumn nor BoardChrome", () => {
  const file = src("src/lib/KanbanPage.svelte");
  const root = fragmentOf(file);
  const guarded = new Set<string>();
  walk(root, (node) => {
    if (node.type !== "IfBlock" || !node.test || !node.consequent) return;
    const testSrc = file.slice(node.test.start ?? 0, node.test.end ?? 0);
    if (!testSrc.includes("railPlanet") || !testSrc.includes("hidden")) return;
    walk(node.consequent, (inner) => {
      if (inner.type === "Component" && (inner.name === "PlanetColumn" || inner.name === "BoardChrome")) {
        guarded.add(inner.name as string);
      }
    });
  });
  assert.equal(guarded.has("PlanetColumn"), true);
  assert.equal(guarded.has("BoardChrome"), true);
});
