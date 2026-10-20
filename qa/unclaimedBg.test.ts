import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";
import { cssRuleBlocks } from "./cssChain.ts";

function ruleBody(src: string, selector: string): string {
  const at = src.indexOf(selector);
  assert.ok(at >= 0, selector);
  const open = src.indexOf("{", at);
  const close = src.indexOf("}", open);
  return src.slice(open + 1, close);
}

/** Same light-slab scan as qa/railInput.test.ts N2. Copied, not imported. */
function lightBackgrounds(cssSrc: string): string[] {
  const src = cssSrc.replace(/\/\*[\s\S]*?\*\//g, "");
  const hits: string[] = [];
  const re = /(^|[\s;}])(background(?:-color)?)\s*:\s*([^;]*);/g;
  for (const m of src.matchAll(re)) {
    const value = m[3].replace(/var\(--planet,\s*#fff(?:fff)?\)/gi, "var(--planet)");
    const where = `${m[2]}: ${value.trim()}`;
    const bare = value.replace(/color-mix\((?:[^)(]+|\([^)]*\))*\)/gi, "");
    const lightRgb = (body: string): boolean => {
      const rgbRe = /rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)(?:[\s,/]+(\d*\.?\d+))?\s*\)/gi;
      for (const c of body.matchAll(rgbRe)) {
        const ch = [Number(c[1]), Number(c[2]), Number(c[3])];
        const alpha = c[4] === undefined ? 1 : Number(c[4]);
        if (ch.every((n) => n >= 230) && alpha >= 0.5) return true;
      }
      if (/(^|[^#\w])#fff\b/i.test(body) || /#ffffff\b/i.test(body)) return true;
      if (/(^|[^\w-])white\b/i.test(body)) return true;
      return false;
    };
    if (/gradient\s*\(/i.test(value)) {
      if (!/transparent/i.test(value)) {
        if (lightRgb(bare)) hits.push(where);
        continue;
      }
      const rgbRe = /rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)(?:[\s,/]+(\d*\.?\d+))?\s*\)/gi;
      for (const c of value.matchAll(rgbRe)) {
        const ch = [Number(c[1]), Number(c[2]), Number(c[3])];
        const alpha = c[4] === undefined ? 1 : Number(c[4]);
        if (ch.every((n) => n >= 230) && alpha > 0.55) hits.push(where);
      }
      const stripped = bare.replace(/rgba?\([^)]*\)/gi, "");
      if (/(^|[^#\w])#fff\b/i.test(stripped) || /#ffffff\b/i.test(stripped) || /(^|[^\w-])white\b/i.test(stripped)) {
        hits.push(where);
      }
      continue;
    }
    if (lightRgb(bare)) hits.push(where);
  }
  return hits;
}

function railCss(): string {
  const sheet = readFileSync(new URL("../src/lib/rail/rail.css", import.meta.url), "utf8");
  const names = [...sheet.matchAll(/@import\s+"\.\/([^"]+)"/g)].map((m) => m[1]);
  return names.map((name) => readFileSync(new URL(`../src/lib/rail/${name}`, import.meta.url), "utf8")).join("\n");
}

const arch = readFileSync(new URL("../src/lib/rail/rail-arch.css", import.meta.url), "utf8");
const css = railCss();

test("unclaimed well has the pre-glass dark background", () => {
  const body = ruleBody(arch, ".rail.stage-shell .layer2-unmapped {");
  assert.match(body, /background:\s*rgb\(28 28 32 \/ 0\.94\)/);
  assert.match(body, /box-shadow:\s*var\(--stage-shadow\)/);
  assert.doesNotMatch(body, /transparent|backdrop-filter|light-dark\(/);
});

test("unclaimed well fix leaves the l2-expanded wells alone", () => {
  const dark = ruleBody(arch, ".rail.stage-shell .l2-expanded {");
  const light = ruleBody(arch, ':root[data-theme="systemLight"] .rail.stage-shell .l2-expanded {');
  assert.match(dark, /background:\s*transparent/);
  assert.match(dark, /box-shadow:\s*none/);
  assert.match(light, /background:\s*transparent/);
  assert.match(light, /box-shadow:\s*none/);
});

test("unclaimed well is dark in every theme", () => {
  const dir = new URL("../src/lib/rail/", import.meta.url);
  const themed: string[] = [];
  for (const name of readdirSync(dir)) {
    if (!name.endsWith(".css")) continue;
    const src = readFileSync(new URL(name, dir), "utf8");
    for (const block of cssRuleBlocks(src)) {
      if (!/:root\[data-theme=/.test(block.selector)) continue;
      if (!block.selector.includes(".layer2-unmapped")) continue;
      if (/background(?:-color)?\s*:/.test(block.body)) themed.push(`${name} ${block.selector}`);
    }
  }
  assert.deepEqual(themed, []);
  assert.deepEqual(lightBackgrounds(css), []);
});
