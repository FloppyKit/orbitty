import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { mailMinPx } from "../src/lib/board/pageMutate.svelte.ts";
import { GLASS_CARD_TINT_ALPHA, isLightSlab } from "../src/themes/glass.ts";
import { MAC_SYSTEM_TOKENS } from "../src/themes/host.ts";

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

function markup(text: string): string {
  return text.replace(/<script\b[\s\S]*?<\/script>/g, "").replace(/<style\b[\s\S]*?<\/style>/g, "");
}

function openTagsWithDataGlass(text: string): string[] {
  text = markup(text);
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
  text = markup(text);
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

function styleText(rel: string): string {
  const text = src(rel);
  if (!rel.endsWith(".svelte")) return text;
  const at = text.indexOf("<style");
  if (at < 0) return "";
  return text.slice(at, text.indexOf("</style>", at));
}

function ruleBodies(css: string): { sel: string; body: string }[] {
  const out: { sel: string; body: string }[] = [];
  const re = /([^{}]+)\{([^{}]*)\}/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(css))) out.push({ sel: m[1], body: m[2] });
  return out;
}

function subjectBodies(css: string, className: string): string[] {
  const end = new RegExp(`\\.${className}(?![\\w-])(?:[:\\[][^\\s,]*)?\\s*$`);
  return ruleBodies(css)
    .filter((rule) => rule.sel.split(",").some((part) => end.test(part.trim())))
    .map((rule) => rule.body);
}

function backgroundAlpha(body: string): number | null {
  const m = body.match(/background(?:-color)?:\s*([^;]+)/);
  if (!m) return null;
  const value = m[1].trim();
  if (value === "transparent" || value === "none") return 0;
  const slash = value.match(/\/\s*([\d.]+)\s*\)\s*$/);
  if (slash) return Number(slash[1]);
  const rgba = value.match(/rgba\(\s*[\d.]+\s*,\s*[\d.]+\s*,\s*[\d.]+\s*,\s*([\d.]+)\s*\)/);
  if (rgba) return Number(rgba[1]);
  const mix = value.match(/([\d.]+)%\s*,\s*transparent\s*\)/);
  if (mix && value.includes("color-mix")) return Number(mix[1]) / 100;
  if (/^#[0-9a-fA-F]{8}$/.test(value)) return parseInt(value.slice(7), 16) / 255;
  if (/^#[0-9a-fA-F]{3,6}$/.test(value)) return 1;
  if (value.includes("--panel-fill") || value.includes("--glass-veil")) return 0.72;
  if (value.includes("var(--panel)")) return 1;
  assert.fail(`unparsed background: ${value}`);
}

test("glass ancestors are not opaque", () => {
  const containers: [string, string][] = [
    ["src/lib/rail/rail-arch.css", "layer2-unmapped"],
    ["src/lib/rail/rail-arch.css", "l2-expanded"],
    ["src/lib/board/board.css", "kanban"],
    ["src/lib/board/board.css", "k-main"],
    ["src/lib/boardStack.css", "k-main"],
    ["src/lib/SettingsPage.svelte", "settings"],
    ["src/lib/StarsPage.svelte", "stars"],
    ["src/lib/companion/page.css", "companion"],
    ["src/lib/rail/rail-chrome.css", "rail"],
    ["src/lib/rail/rail-stage.css", "stage-shell"],
    ["src/routes/+page.svelte", "layer2-unmapped"],
    ["src/routes/+page.svelte", "l2-expanded"],
  ];
  for (const [rel, className] of containers) {
    for (const body of subjectBodies(styleText(rel), className)) {
      const alpha = backgroundAlpha(body);
      if (alpha == null) continue;
      assert.ok(alpha < 0.9, `${rel} .${className} alpha ${alpha}`);
    }
  }
  for (const rel of ["src/lib/rail/rail-arch.css", "src/routes/+page.svelte"]) {
    assert.equal(src(rel).includes("rgb(28 28 32 / 0.94)"), false, rel);
  }
  const page = src("src/routes/+page.svelte");
  const tintRule = page.match(/:global\(html \[data-glass\]\) \{[^}]*\}/);
  assert.ok(tintRule, "card tint is not limited to html:not(.glass-rt)");
  assert.equal(tintRule[0].includes(":not(.glass-rt)"), false);
  const unmapped = subjectBodies(src("src/lib/rail/rail-arch.css"), "layer2-unmapped").join("\n");
  assert.match(unmapped, /background:\s*transparent/);
  assert.match(unmapped, /box-shadow:\s*none/);
  const expanded = subjectBodies(src("src/lib/rail/rail-arch.css"), "l2-expanded");
  assert.ok(expanded.length >= 2);
  for (const body of expanded) {
    assert.match(body, /background:\s*transparent/);
    assert.match(body, /box-shadow:\s*none/);
  }
  const gaps: [string, string][] = [
    ["src/lib/board/board.css", "b-col"],
    ["src/lib/boardStack.css", "mail-todo"],
    ["src/lib/boardStack.css", "mail-needed"],
  ];
  for (const [rel, className] of gaps) {
    const bodies = subjectBodies(styleText(rel), className);
    const painted = bodies.filter((body) => /background(?:-color)?:/.test(body));
    assert.ok(painted.length >= 1, `${rel} .${className}`);
    for (const body of painted) {
      assert.match(body, /background:\s*transparent/, `${rel} .${className}`);
      assert.equal(backgroundAlpha(body), 0);
    }
  }
});

function parseColor(raw: string): [number, number, number, number] {
  const v = raw.trim();
  if (v.startsWith("#")) {
    const h = v.slice(1);
    const n = h.length <= 4 ? h.split("").map((c) => c + c).join("") : h;
    const r = parseInt(n.slice(0, 2), 16);
    const g = parseInt(n.slice(2, 4), 16);
    const b = parseInt(n.slice(4, 6), 16);
    const a = n.length === 8 ? parseInt(n.slice(6, 8), 16) / 255 : 1;
    return [r, g, b, a];
  }
  const m = v.match(/rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)\s*(?:[,/]\s*([\d.]+)\s*)?\)/);
  assert.ok(m, v);
  return [Number(m[1]), Number(m[2]), Number(m[3]), m[4] == null ? 1 : Number(m[4])];
}

const TINT = "color-mix(in srgb, var(--panel) calc(var(--glass-card-tint) * 100%), transparent)";

test("glass card tint is the one source", () => {
  assert.equal(GLASS_CARD_TINT_ALPHA, 0.45);
  const page = src("src/routes/+page.svelte");
  assert.match(page, /--glass-card-tint:\s*0\.45/);
  const tint = page.match(/:global\(html \[data-glass\]\) \{[^}]*\}/);
  assert.ok(tint);
  assert.equal(tint[0].includes(":not(.glass-rt)"), false);
  assert.equal(tint[0].includes("html.glass-rt"), false);
  assert.match(tint[0], /var\(--glass-card-tint\)/);
  assert.equal(tint[0].includes("--panel-fill"), false);
  assert.equal(tint[0].includes("--glass-veil"), false);
  assert.equal(page.includes("color-mix(in srgb, var(--panel) 45%"), false);
  assert.equal(src("src/lib/rail/rail-arch.css").includes("rgb(28 28 32 / 0.94)"), false);
  const cardRules: [string, string][] = [
    ["src/lib/board/board.css", ".b-card {"],
    ["src/lib/board/board.css", ".planet-col {"],
    ["src/lib/stars/TrackerCard.svelte", "\n  .card {"],
    ["src/lib/SettingsAppearance.svelte", ".panel {"],
    ["src/lib/SettingsPacks.svelte", ".panel {"],
    ["src/lib/SettingsPlans.svelte", ".panel {"],
    ["src/lib/companion/dock.css", ".hosting .ghostdock {"],
    ["src/lib/rail/rail-arch.css", ".l2-window-card {"],
    ["src/lib/rail/rail-arch.css", ".l2-needed-card {"],
  ];
  for (const [rel, sel] of cardRules) {
    const text = src(rel);
    const at = text.indexOf(sel);
    assert.ok(at >= 0, `${rel} ${sel}`);
    const body = text.slice(at, text.indexOf("}", at));
    assert.ok(body.includes(TINT), `${rel} ${sel}`);
    assert.equal(body.includes("--panel-fill"), false, rel);
    assert.equal(body.includes("--glass-veil"), false, rel);
  }
});

test("glass dark themes have no light glass fills", () => {
  for (const key of ["bg", "panel"] as const) {
    assert.equal(isLightSlab(...parseColor(MAC_SYSTEM_TOKENS.systemDark[key])), false, `systemDark ${key}`);
  }
  assert.equal(isLightSlab(...parseColor(MAC_SYSTEM_TOKENS.systemLight.panel)), true);
  const page = src("src/routes/+page.svelte");
  const termStart = page.indexOf('data-theme="terminal"');
  const term = page.slice(termStart, page.indexOf("color-scheme: dark", termStart));
  for (const prop of ["--bg", "--panel"]) {
    const m = term.match(new RegExp(`${prop}:\\s*([^;]+)`));
    assert.ok(m, prop);
    assert.equal(isLightSlab(...parseColor(m[1])), false, `terminal ${prop}`);
  }
  for (const m of term.matchAll(/background:\s*([^;]+)/g)) {
    assert.equal(isLightSlab(...parseColor(m[1])), false, m[1]);
  }
  assert.match(term, /--ink:\s*#c8f0d0/);
});

test("planet column gap is 8px", () => {
  const css = src("src/lib/board/board.css");
  const at = css.indexOf(".planet-col {");
  const body = css.slice(at, css.indexOf("}", at));
  assert.match(body, /margin-right:\s*8px/);
  const col = src("src/lib/board/planetColumn.ts");
  assert.match(col, /COLUMN_OPEN_PX = 168/);
  assert.match(col, /COLUMN_ICONS_PX = 72/);
  const mutate = src("src/lib/board/pageMutate.svelte.ts");
  const fn = mutate.slice(mutate.indexOf("export function mailMinPx"), mutate.indexOf("export function togglePlanetCol"));
  assert.match(fn, /120 \+ column \+ \(column > 0 \? 8 : 0\)/);
  const clamp = mutate.slice(mutate.indexOf("export function clampPreview"), mutate.indexOf("export function onEastSplitDown"));
  assert.match(clamp, /mailMinPx\(\)/);
  assert.match(clamp, /8px/);
  assert.equal(mailMinPx({ columnWidthNow: () => 168 } as never), 120 + 168 + 8);
  assert.equal(mailMinPx({ columnWidthNow: () => 72 } as never), 120 + 72 + 8);
  assert.equal(mailMinPx({ columnWidthNow: () => 0 } as never), 120);
});
