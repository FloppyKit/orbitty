import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { nudgePick } from "../src/lib/rail/nudgeTrigger.ts";
import {
  GLASS_VEIL_ALPHA,
  isLightSlab,
  tileFillAlpha,
  veilAlpha,
  vibrancyWanted,
} from "../src/themes/glass.ts";
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
  assert.match(apply, /setFill\("--panel", "--panel-fill", tileFillAlpha\(o, solid\)\)/);
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
  assert.match(
    src("src/lib/RailPage.svelte"),
    /set_vibrancy", \{ enabled: vibrancyWanted\(reduce\), light \}/,
  );
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
  assert.match(head, /\.planet-head-mark \{[^}]*color:\s*var\(--ink\)/);
  assert.match(head, /\.planet-head-count \{[\s\S]*?color:\s*var\(--ink\)/);
  assert.match(head, /<span class="planet-head-mark">\{mark\}<\/span>/);
  assert.equal(head.includes("data-tauri-drag-region"), false);
});

test("glass Claim chip appears on a new minimize of an unclaimed window", () => {
  const win = { id: "w1", stableKey: "sk1", isMinimized: true };
  const none = new Set<string>();
  assert.equal(nudgePick([win], none, none, [])?.id, "w1");
  assert.equal(nudgePick([win], new Set(["w1"]), none, []), undefined);
  assert.equal(nudgePick([win], none, new Set(["w1"]), []), undefined);
  assert.equal(nudgePick([win], none, none, ["w1"]), undefined);
  assert.equal(nudgePick([win], none, none, ["sk1"]), undefined);
  assert.equal(nudgePick([{ ...win, isMinimized: false }], none, none, []), undefined);
  const drag = src("src/lib/rail/pageDrag.svelte.ts");
  const fn = drag.slice(
    drag.indexOf("export function checkNudge"),
    drag.indexOf("export async function openSettings"),
  );
  assert.match(fn, /if \(!scope\.nudgePrimed\)/);
  assert.match(fn, /nudgePick\(/);
  assert.equal(fn.includes("flatMap"), false);
});

test("glass Claim one nudge sits below the overlay strip", () => {
  const stage = src("src/lib/rail/rail-stage.css");
  assert.match(stage, /\.rail\.stage-shell \.nudge \{[\s\S]*?margin-top:\s*28px/);
  const list = src("src/lib/rail/List.svelte");
  assert.match(list, /Claim\s+one\?/);
  assert.equal(list.includes("data-tauri-drag-region"), false);
  assert.equal(stage.includes("data-tauri-drag-region"), false);
});

test("glass main window has no native title bar", () => {
  const conf = src("src-tauri/tauri.conf.json");
  assert.match(conf, /"decorations": false/);
  assert.equal(conf.includes("titleBarStyle"), false);
  const glass = src("src-tauri/src/windows/glass.rs");
  assert.equal(glass.includes("clear_titlebar_separator"), false);
  assert.equal(glass.includes("setTitlebarSeparatorStyle"), false);
  const chrome = src("src/lib/rail/rail-chrome.css").match(/\.chrome \{[^}]*\}/);
  assert.ok(chrome);
  assert.equal(chrome[0].includes("border-bottom"), false);
  const wtr = src("src/lib/WindowTitleRow.svelte").match(/\.wtr \{[^}]*\}/);
  assert.ok(wtr);
  assert.equal(wtr[0].includes("border-bottom"), false);
  assert.equal(src("src/lib/rail/rail-chrome.css").includes("data-tauri-drag-region"), false);
});

test("glass reduced transparency skips vibrancy and uses a solid tile", () => {
  assert.equal(vibrancyWanted(true), false);
  assert.equal(vibrancyWanted(false), true);
  assert.equal(tileFillAlpha(1, true), 1);
  assert.equal(tileFillAlpha(0.5, false), 0.36);
  const page = src("src/routes/+page.svelte");
  assert.match(page, /@media \(prefers-reduced-transparency: reduce\)/);
  assert.match(page, /html\.glass-solid \.rail/);
  assert.match(src("src/lib/RailPage.svelte"), /prefers-reduced-transparency: reduce/);
  assert.match(src("src/themes/glass.ts"), /prefers-reduced-transparency: reduce/);
  const hit = src("src/lib/rail/rail-arch.css");
  assert.match(hit, /\.l2-hit[\s\S]*?backdrop-filter:\s*blur\(18px\)/);
});

test("glass labels are the nine glass windows", () => {
  const rust = src("src-tauri/src/windows/vibrancy_mac.rs");
  assert.match(
    rust,
    /GLASS_LABELS: &\[&str\] = &\[\s*"main",\s*"kanban",\s*"companion",\s*"stars",\s*"settings",\s*"sky",\s*"cheatsheet",\s*"yard-warn",\s*"next-card",\s*\]/,
  );
});

test("glass set_vibrancy honors enabled", () => {
  assert.match(src("src-tauri/src/lib.rs"), /set_wanted\(enabled,/);
});

test("glass window_appearance syncs the material", () => {
  const chrome = src("src-tauri/src/windows/window_chrome.rs");
  assert.match(chrome, /set_light\(scheme == "light"\)/);
  assert.match(chrome, /sync_one\(&window\)/);
});

test("glass vibrancy apply clears first and releases the effect view", () => {
  const rust = src("src-tauri/src/windows/vibrancy_mac.rs");
  const apply = rust.slice(rust.indexOf("pub fn apply("), rust.indexOf("fn content_view("));
  assert.match(apply, /clear\(window\);/);
  assert.match(apply, /addSubview: view[\s\S]*?msg_send!\[view, release\]/);
  assert.equal(apply.split("msg_send![view, release]").length - 1, 1);
});

test("glass vibrancy blends behind the window", () => {
  assert.match(src("src-tauri/src/windows/vibrancy_mac.rs"), /setBlendingMode: 0isize/);
});

test("glass material_for maps light to UnderWindowBackground", () => {
  const rust = src("src-tauri/src/windows/vibrancy_mac.rs");
  assert.match(rust, /fn material_for\(light: bool\)[\s\S]*?material_for_dark\(!light\)/);
  assert.match(rust, /material_for\(light\),/);
});

test("glass RailPage passes the real light flag", () => {
  const rail = src("src/lib/RailPage.svelte");
  assert.match(rail, /enabled: vibrancyWanted\(reduce\), light \}/);
  assert.match(rail, /enabled: vibrancyWanted\(mq\.matches\), light \}/);
});

test("glass reduced transparency listens and paints solid roots", () => {
  const rail = src("src/lib/RailPage.svelte");
  assert.match(rail, /mq\.addEventListener\("change", onChange\)/);
  assert.match(src("src/themes/apply.ts"), /classList\.toggle\("glass-solid", solid\)/);
  assert.match(src("src/routes/+page.svelte"), /html\.glass-solid \.kanban/);
});

test("glass tile glow stays on settings, trackers, sky, and the dock", () => {
  const panel = src("src/lib/SettingsAppearance.svelte").match(/\.panel \{[^}]*\}/);
  assert.ok(panel);
  assert.match(panel[0], /--glass-glow/);
  assert.equal(panel[0].includes("--edge"), false);
  const card = src("src/lib/stars/TrackerCard.svelte").match(/\.card \{[^}]*\}/);
  assert.ok(card);
  assert.match(card[0], /--glass-glow/);
  assert.equal(card[0].includes("--edge"), false);
  assert.match(src("src/lib/SkyPage.svelte"), /\.sky \{[^}]*background:\s*transparent/);
  assert.match(
    src("src/lib/StarsPage.svelte"),
    /\.stars::before \{[^}]*color-mix\(in srgb, var\(--bg\) 36%, transparent\)/,
  );
  const dock = src("src/lib/companion/dock.css");
  assert.match(
    dock,
    /\.ghost \.ghostdock,\s*\.hosting \.ghostdock \{[^}]*background:\s*var\(--panel-fill/,
  );
});
