import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { GLASS_VEIL_ALPHA, isLightSlab, veilAlpha } from "../src/themes/apply.ts";
import { MAC_SYSTEM_TOKENS } from "../src/themes/host.ts";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

function parseCssColor(raw: string): [number, number, number, number] {
  const v = raw.trim();
  if (v.startsWith("#")) {
    const h = v.slice(1);
    const n =
      h.length === 3 || h.length === 4
        ? h
            .split("")
            .map((c) => c + c)
            .join("")
        : h;
    const r = parseInt(n.slice(0, 2), 16);
    const g = parseInt(n.slice(2, 4), 16);
    const b = parseInt(n.slice(4, 6), 16);
    const a = n.length === 8 ? parseInt(n.slice(6, 8), 16) / 255 : 1;
    return [r, g, b, a];
  }
  const m = v.match(/rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*(?:,\s*([\d.]+)\s*)?\)/);
  assert.ok(m, v);
  return [Number(m[1]), Number(m[2]), Number(m[3]), m[4] == null ? 1 : Number(m[4])];
}

test("glass dark surfaces are not light slabs", () => {
  for (const key of ["bg", "panel"] as const) {
    const dark = parseCssColor(MAC_SYSTEM_TOKENS.systemDark[key]);
    assert.equal(isLightSlab(...dark), false, `systemDark ${key}`);
    const term = key === "bg" ? "#0a120e" : "#12201a";
    assert.equal(isLightSlab(...parseCssColor(term)), false, `terminal ${key}`);
  }
  const page = src("src/routes/+page.svelte");
  const terminal = page.slice(page.indexOf('data-theme="terminal"'), page.indexOf("color-scheme: dark"));
  assert.match(terminal, /--bg:\s*#0a120e/);
  assert.match(terminal, /--panel:\s*#12201a/);
  assert.equal(isLightSlab(...parseCssColor(MAC_SYSTEM_TOKENS.systemLight.panel)), true);
});

test("glass veil alpha is 0.72 and the slider scales it", () => {
  assert.equal(GLASS_VEIL_ALPHA, 0.72);
  assert.equal(veilAlpha(1), 0.72);
  assert.equal(veilAlpha(0.5), 0.36);
  const apply = src("src/themes/apply.ts");
  assert.match(apply, /setFill\("--panel", "--panel-fill", veilAlpha\(o\)\)/);
  assert.match(src("src/routes/+page.svelte"), /--glass-veil:\s*0\.72/);
});

test("glass page roots are clear and tiles keep the veil glow", () => {
  assert.match(src("src/lib/rail/rail-chrome.css"), /\.rail \{[\s\S]*?background:\s*transparent/);
  assert.match(src("src/lib/board/board.css"), /\.kanban \{[\s\S]*?background:\s*transparent/);
  assert.match(src("src/lib/board/board.css"), /\.kanban \{[\s\S]*?border-radius:\s*0/);
  assert.match(src("src/lib/StarsPage.svelte"), /\.stars \{[\s\S]*?background:\s*transparent/);
  assert.match(src("src/lib/companion/page.css"), /\.companion \{[\s\S]*?background:\s*transparent/);
  assert.match(src("src/lib/SettingsPage.svelte"), /\.settings \{[\s\S]*?background:\s*transparent/);
  const glow = "color-mix(in srgb, var(--accent) 45%, transparent)";
  assert.ok(src("src/lib/board/board.css").includes(glow));
  assert.ok(src("src/lib/companion/dock.css").includes("--panel-fill"));
  assert.ok(src("src/routes/+page.svelte").includes("--glass-glow:"));
});
