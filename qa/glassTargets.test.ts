import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

function src(rel: string): string {
  return readFileSync(join(root, rel), "utf8");
}

function svelteRels(): string[] {
  const out: string[] = [];
  const walk = (dir: string) => {
    for (const name of readdirSync(dir)) {
      if (name === "node_modules" || name === ".svelte-kit") continue;
      const abs = join(dir, name);
      if (statSync(abs).isDirectory()) walk(abs);
      else if (name.endsWith(".svelte")) out.push(relative(root, abs));
    }
  };
  walk(join(root, "src"));
  return out;
}

function openTagsWithDataGlass(text: string): string[] {
  const out: string[] = [];
  const re = /\bdata-glass\b/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text))) {
    const lt = text.lastIndexOf("<", m.index);
    const gt = text.indexOf(">", m.index);
    if (lt < 0 || gt < 0 || text[lt + 1] === "/" || text[lt + 1] === "!") continue;
    out.push(text.slice(lt, gt + 1));
  }
  return out;
}

function classTokens(tag: string): string[] {
  const m = tag.match(/\bclass="([^"]*)"/);
  if (!m) return [];
  return m[1].split(/\s+/).filter((token) => token.length > 0);
}

/** Interior of each data-glass element, using matching close tags. */
function dataGlassInteriors(text: string): string[] {
  const interiors: string[] = [];
  const re = /\bdata-glass\b/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text))) {
    const lt = text.lastIndexOf("<", m.index);
    const gt = text.indexOf(">", m.index);
    if (lt < 0 || gt < 0 || text[lt + 1] === "/" || text[lt + 1] === "!") continue;
    const name = /^<([A-Za-z][\w:-]*)/.exec(text.slice(lt, gt + 1))?.[1];
    if (!name) continue;
    let depth = 1;
    let j = gt + 1;
    while (j < text.length && depth > 0) {
      const nextOpen = text.indexOf(`<${name}`, j);
      const nextClose = text.indexOf(`</${name}`, j);
      if (nextClose < 0) break;
      const openIsTag =
        nextOpen >= 0 &&
        nextOpen < nextClose &&
        /[\s>/]/.test(text[nextOpen + 1 + name.length] ?? " ");
      if (openIsTag) {
        depth += 1;
        j = nextOpen + name.length + 1;
        continue;
      }
      depth -= 1;
      if (depth === 0) interiors.push(text.slice(gt + 1, nextClose));
      j = nextClose + name.length + 2;
    }
    re.lastIndex = gt + 1;
  }
  return interiors;
}

const TAGS: Record<string, string[]> = {
  "src/lib/board/Columns.svelte": ["b-card"],
  "src/lib/board/PlanetColumn.svelte": ["planet-col"],
  "src/lib/rail/List.svelte": ["l2-window-card", "l2-needed-card"],
  "src/lib/stars/TrackerCard.svelte": ["card"],
  "src/lib/SettingsAppearance.svelte": ["panel"],
  "src/lib/SettingsPacks.svelte": ["panel"],
  "src/lib/SettingsPlans.svelte": ["panel"],
  "src/lib/CompanionDock.svelte": ["ghostdock"],
};

test("glass targets are data-glass only", () => {
  const frames = src("src/lib/glass/cardFrames.ts");
  assert.equal(frames.includes("GLASS_CARD_SELECTORS"), false);
  assert.equal(frames.includes("absorbGlassRect"), false);
  assert.equal(frames.includes("rectInside"), false);
  const calls = [...frames.matchAll(/querySelectorAll\(([^)]*)\)/g)];
  assert.equal(calls.length, 1);
  assert.equal(calls[0][1], '"[data-glass]"');
  assert.equal(/export const \w+ = \[[^\]]*['"]\./.test(frames), false);

  const page = src("src/routes/+page.svelte");
  const flushAt = page.indexOf("const flush = () => {");
  assert.ok(flushAt >= 0);
  const flush = page.slice(flushAt, page.indexOf("flush();", flushAt));
  assert.match(flush, /collectGlassCardRects\(/);
  assert.equal(flush.includes("querySelectorAll"), false);
  assert.equal(flush.includes(".row, .panel, .card"), false);

  const seen = new Map<string, string[]>();
  for (const rel of svelteRels()) {
    const tags = openTagsWithDataGlass(src(rel));
    if (tags.length === 0) continue;
    seen.set(rel, tags.map((tag) => classTokens(tag).join(" ")));
  }
  assert.deepEqual([...seen.keys()].sort(), Object.keys(TAGS).sort());
  for (const [rel, classes] of Object.entries(TAGS)) {
    const got = seen.get(rel) ?? [];
    assert.deepEqual(got, classes, rel);
  }
});

test("glass targets do not nest", () => {
  const skipClass = new Set([
    "row",
    "l2-hit",
    "b-col",
    "l2-board",
    "layer2-unmapped",
    "kanban",
    "k-main",
    "settings",
    "stars",
    "companion",
    "rail",
    "mail",
    "mail-todo",
    "mail-needed",
    "mail-done",
    "b-zone",
    "k-body",
    "l2-preview",
    "l2-expanded",
  ]);
  for (const rel of svelteRels()) {
    const text = src(rel);
    for (const tag of openTagsWithDataGlass(text)) {
      for (const token of classTokens(tag)) {
        assert.equal(skipClass.has(token), false, `${rel} ${token}`);
      }
    }
    for (const inner of dataGlassInteriors(text)) {
      assert.equal(inner.includes("data-glass"), false, rel);
    }
  }
  const starsCol = openTagsWithDataGlass(src("src/lib/stars/StarsPlanetColumn.svelte"));
  assert.equal(starsCol.length, 0);
  const yard = openTagsWithDataGlass(src("src/lib/YardWarnPage.svelte"));
  assert.equal(yard.length, 0);
  const settings = openTagsWithDataGlass(src("src/lib/SettingsPage.svelte"));
  assert.equal(settings.length, 0);
  const add = openTagsWithDataGlass(src("src/lib/stars/StarsTrackers.svelte"));
  assert.equal(add.length, 0);
  const access = src("src/lib/rail/List.svelte").match(/<section\b[^>]*class="card"[^>]*>/);
  assert.ok(access);
  assert.equal(access[0].includes("data-glass"), false);
});
