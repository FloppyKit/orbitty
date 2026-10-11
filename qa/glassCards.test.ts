import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { GLASS_VEIL_ALPHA, isLightSlab, veilAlpha } from "../src/themes/glass.ts";
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

test("glass vibrancy material is HudWindow in dark and UnderWindowBackground in light", () => {
  const rust = src("src-tauri/src/windows/vibrancy_mac.rs");
  assert.match(rust, /MATERIAL_HUD_WINDOW: u64 = 13/);
  assert.match(rust, /MATERIAL_UNDER_WINDOW_BACKGROUND: u64 = 21/);
  assert.match(rust, /STATE_ACTIVE: u64 = 1/);
  assert.match(
    rust,
    /if dark \{\s*MATERIAL_HUD_WINDOW\s*\} else \{\s*MATERIAL_UNDER_WINDOW_BACKGROUND\s*\}/,
  );
  assert.match(src("src/lib/RailPage.svelte"), /set_vibrancy", \{ enabled: true, light \}/);
  assert.equal(src("src/lib/RailPage.svelte").includes("enabled: false"), false);
});

test("glass vendored vibrancy keeps the window-vibrancy notice and drops the crate", () => {
  const rust = src("src-tauri/src/windows/vibrancy_mac.rs");
  assert.match(rust, /SPDX-License-Identifier: Apache-2\.0/);
  assert.match(rust, /SPDX-License-Identifier: MIT/);
  assert.match(rust, /Copyright 2019-2022 Tauri Programme within The Commons Conservancy/);
  assert.match(rust, /window-vibrancy 0\.6/);
  const notice = src("third_party/window-vibrancy/NOTICE");
  assert.match(notice, /window-vibrancy 0\.6/);
  assert.match(notice, /Tauri Programme within The Commons Conservancy/);
  assert.ok(src("third_party/window-vibrancy/LICENSE-MIT").includes("MIT License"));
  assert.ok(src("third_party/window-vibrancy/LICENSE-APACHE").includes("Apache License"));
  assert.equal(src("src-tauri/Cargo.toml").includes("window-vibrancy"), false);
  assert.equal(src("src-tauri/src/lib.rs").includes("window_vibrancy"), false);
});

test("glass mark chip is a circle and the glyph sits in ink", () => {
  const head = src("src/lib/PlanetHead.svelte");
  assert.match(head, /\.planet-chip\.mark \{[\s\S]*?border-radius:\s*50%/);
  assert.match(head, /\.planet-chip\.mark \{[\s\S]*?background:\s*var\(--accent\)/);
  assert.match(head, /\.planet-head-mark \{[\s\S]*?color:\s*var\(--ink\)/);
  assert.match(head, /\.planet-head-count \{[\s\S]*?color:\s*var\(--ink\)/);
  assert.match(head, /<span class="planet-head-mark">\{mark\}<\/span>/);
  assert.equal(head.includes("data-tauri-drag-region"), false);
});

test("glass Claim one nudge sits below the overlay strip", () => {
  const stage = src("src/lib/rail/rail-stage.css");
  assert.match(stage, /\.rail\.stage-shell \.nudge \{[\s\S]*?margin-top:\s*28px/);
  const list = src("src/lib/rail/List.svelte");
  assert.match(list, /Claim\s+one\?/);
  assert.equal(list.includes("data-tauri-drag-region"), false);
  assert.equal(stage.includes("data-tauri-drag-region"), false);
});

test("glass titlebar separator style is none", () => {
  const glass = src("src-tauri/src/windows/glass.rs");
  assert.match(glass, /NSTitlebarSeparatorStyleNone = 1/);
  assert.match(glass, /setTitlebarSeparatorStyle: 1usize/);
  assert.equal(glass.includes("@available"), false);
  assert.equal(src("src/lib/rail/rail-chrome.css").includes("data-tauri-drag-region"), false);
});
