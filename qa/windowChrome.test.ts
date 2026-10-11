import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { planetCss } from "../src/lib/planetColor.ts";
import { dragTargetIgnored } from "../src/lib/dragPane.ts";
import {
  boardTitleDot,
  boardWindowTitle,
  chromeMode,
} from "../src/lib/windowChrome.ts";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

test("W1 chromeMode and boardWindowTitle", () => {
  assert.equal(chromeMode("Mozilla/5.0 (Macintosh; Intel Mac OS X)", "MacIntel"), "mac");
  assert.equal(chromeMode("Mozilla/5.0", "MacIntel"), "mac");
  assert.equal(chromeMode("Mozilla/5.0 (Macintosh)", "Win32"), "mac");
  assert.equal(chromeMode("Mozilla/5.0 (X11; Linux x86_64)", "Linux x86_64"), "custom");
  assert.equal(chromeMode("Mozilla/5.0 (Windows NT 10.0)", "Win32"), "custom");
  const projects = [
    { id: "p1", name: "Habitat" },
    { id: "p2", name: "Orbit" },
  ];
  assert.equal(boardWindowTitle("today", projects), "Today");
  assert.equal(boardWindowTitle("done", projects), "Done");
  assert.equal(boardWindowTitle("inbox", projects), "Inbox");
  assert.equal(boardWindowTitle("p1", projects), "Habitat");
  assert.equal(boardWindowTitle("missing", projects), "Inbox");
});

test("W9 planet dot only before a planet name", () => {
  const projects = [{ id: "p1" }, { id: "p2" }];
  assert.equal(boardTitleDot("today", projects), null);
  assert.equal(boardTitleDot("done", projects), null);
  assert.equal(boardTitleDot("inbox", projects), null);
  assert.equal(boardTitleDot("missing", projects), null);
  assert.equal(boardTitleDot("p1", projects), planetCss("p1"));
  const row = src("src/lib/WindowTitleRow.svelte");
  const dotAt = row.indexOf('class="wtr-dot"');
  const titleAt = row.indexOf('class="wtr-title"');
  assert.ok(dotAt > 0 && titleAt > dotAt, "dot before title");
  assert.match(row, /class="wtr-dot" aria-hidden="true"/);
  assert.match(row, /\{#if dot\}/);
  const chrome = src("src/lib/board/Chrome.svelte");
  assert.match(chrome, /dot=\{titleDot\}/);
  assert.equal(chrome.includes("planet-chip"), false);
});

test("W2 board title row and embedded banner", () => {
  const chrome = src("src/lib/board/Chrome.svelte");
  assert.match(chrome, /<WindowTitleRow[\s\S]*title=\{listTitle\}/);
  assert.match(chrome, /chips=\{selected === "done" \? doneActorChips : undefined\}/);
  assert.match(chrome, /\{#if embedded\}[\s\S]*class="k-banner"/);
  assert.match(chrome, /<h1>Launchpad<\/h1>/);
  assert.match(chrome, /class="popout"/);
  assert.match(chrome, /class="winx"/);
  const embedded = chrome.slice(chrome.indexOf("{#if embedded}"), chrome.indexOf("{:else}"));
  const windowed = chrome.slice(chrome.indexOf("{:else}"));
  assert.match(embedded, /class="k-banner"/);
  assert.match(embedded, /class="winx"/);
  assert.equal(windowed.includes('class="winx"'), false);
  assert.match(windowed, /title=\{listTitle\}/);
});

test("W3 stars title row keeps day nav; embedded banner keeps close", () => {
  const stars = src("src/lib/StarsPage.svelte");
  assert.match(stars, /title="Stars"/);
  assert.match(stars, /chips=\{dayNav\}/);
  const snippet = stars.slice(stars.indexOf("{#snippet dayNav()}"), stars.indexOf("</main>"));
  assert.match(snippet, /aria-label="Previous day"/);
  assert.match(snippet, /aria-label="Next day"/);
  assert.match(snippet, /class="nav-view"/);
  assert.match(snippet, /data-mkey="stars-nav"/);
  assert.match(snippet, /Prompts/);
  assert.match(snippet, /class="nav-date"/);
  assert.match(snippet, /type="date"/);
  const embedded = stars.slice(stars.indexOf("{#if embedded}"), stars.indexOf("{:else}"));
  assert.match(embedded, /class="s-banner"/);
  assert.match(embedded, /class="winx"/);
  assert.match(embedded, /onClose\?\.\(\)/);
  assert.match(embedded, /\{@render dayNav\(\)\}/);
  const elseAt = stars.indexOf("{:else}");
  const windowed = stars.slice(elseAt, stars.indexOf("{/if}", elseAt));
  assert.equal(windowed.includes('class="s-banner"'), false);
  assert.match(windowed, /<WindowTitleRow/);
});

test("W4 companion title only while expanded, no chips", () => {
  const page = src("src/lib/CompanionPage.svelte");
  const css = src("src/lib/companion/page.css");
  assert.match(page, /\{#if expanded\}[\s\S]*title="Orbitty"/);
  const block = page.slice(page.indexOf("{#if expanded}"), page.indexOf("{#if expanded && sprite}"));
  assert.match(block, /<WindowTitleRow/);
  assert.equal(block.includes("chips="), false);
  assert.match(css, /--wtr-h/);
  assert.match(css, /\.companion\.chrome \.exphead/);
});

test("W5 custom close only, mode from chromeMode", () => {
  const row = src("src/lib/WindowTitleRow.svelte");
  const custom = row.slice(row.indexOf('{#if mode === "custom"}'), row.indexOf("{/if}", row.indexOf('{#if mode === "custom"}')));
  assert.match(custom, /class="winx"/);
  const before = row.slice(0, row.indexOf('{#if mode === "custom"}'));
  assert.equal(before.includes('class="winx"'), false);
  for (const rel of [
    "src/lib/board/Chrome.svelte",
    "src/lib/StarsPage.svelte",
    "src/lib/CompanionPage.svelte",
  ]) {
    const file = src(rel);
    assert.match(file, /chromeMode\(navUa\(\), navPlatform\(\)\)/);
  }
});

test("W6 drag stays on startPaneDrag; chips do not start it", () => {
  const row = src("src/lib/WindowTitleRow.svelte");
  const drag = src("src/lib/dragPane.ts");
  assert.match(row, /data-title-row/);
  assert.match(row, /onmousedown=\{onRowMouseDown\}/);
  assert.match(row, /startPaneDrag/);
  assert.match(row, /data-no-drag/);
  assert.match(drag, /\[data-no-drag\]/);
  assert.match(drag, /\.wtr-chips \*/);
  const chip = {
    closest(sel: string) {
      return sel.includes("[data-no-drag]") ? this : null;
    },
  };
  assert.equal(dragTargetIgnored(chip), true);
  const plain = { closest: () => null };
  assert.equal(dragTargetIgnored(plain), false);
  for (const rel of [
    "src/lib/WindowTitleRow.svelte",
    "src/lib/board/Chrome.svelte",
    "src/lib/StarsPage.svelte",
    "src/lib/CompanionPage.svelte",
  ]) {
    assert.equal(src(rel).includes("data-tauri-drag-region"), false, rel);
  }
});

test("W7 done chips once", () => {
  const cols = src("src/lib/board/Columns.svelte");
  const kanban = src("src/lib/KanbanPage.svelte");
  assert.match(cols, /\{#if !chipsInTitle\}[\s\S]*doneActorChips/);
  assert.match(kanban, /chipsInTitle=\{railPlanet == null && planetColMode !== "hidden"\}/);
});

test("W8 theme vars and terminal color-scheme", () => {
  const row = src("src/lib/WindowTitleRow.svelte");
  const style = row.slice(row.indexOf("<style>"), row.indexOf("</style>"));
  assert.equal(/#[0-9a-fA-F]{3,8}/.test(style), false, style);
  assert.equal(/rgb\(/.test(style), false, style);
  assert.match(style, /var\(--ink\)/);
  assert.match(style, /background:\s*transparent/);
  const page = src("src/routes/+page.svelte");
  const terminal = page.slice(page.indexOf('data-theme="terminal"'), page.indexOf("S4.4"));
  assert.match(terminal, /color-scheme:\s*dark/);
  assert.match(row, /window_appearance/);
});

const SETUP_LABELS =
  'for label in ["main", "kanban", "companion", "stars", "settings", "sky", "cheatsheet"]';

/** Brace-balanced slice starting at `{`, skipping comments and strings. */
function balanced(text: string, open: number): { text: string; end: number } {
  if (text[open] !== "{") throw new Error(`expected { at ${open}`);
  let i = open;
  let depth = 0;
  while (i < text.length) {
    const c = text[i];
    const n = text[i + 1];
    if (c === "/" && n === "/") {
      const nl = text.indexOf("\n", i);
      i = nl < 0 ? text.length : nl + 1;
      continue;
    }
    if (c === "/" && n === "*") {
      const end = text.indexOf("*/", i + 2);
      i = end < 0 ? text.length : end + 2;
      continue;
    }
    if (c === '"') {
      i++;
      while (i < text.length && text[i] !== '"') {
        if (text[i] === "\\") i += 2;
        else i++;
      }
      i++;
      continue;
    }
    if (c === "'" && text[i + 1] === "\\" && text[i + 3] === "'") {
      i += 4;
      continue;
    }
    if (c === "'" && text[i + 1] !== "'" && text[i + 2] === "'") {
      i += 3;
      continue;
    }
    if (c === "{") depth++;
    else if (c === "}") {
      depth--;
      if (depth === 0) return { text: text.slice(open, i + 1), end: i + 1 };
    }
    i++;
  }
  throw new Error("unbalanced brace");
}

function macosCfgBlockContaining(text: string, needle: string): string | null {
  const attr = '#[cfg(target_os = "macos")]';
  let from = 0;
  while (from < text.length) {
    const at = text.indexOf(attr, from);
    if (at < 0) return null;
    let i = at + attr.length;
    while (i < text.length && /\s/.test(text[i])) i++;
    if (text[i] === "{") {
      const block = balanced(text, i);
      if (block.text.includes(needle)) return block.text;
      from = block.end;
      continue;
    }
    from = at + attr.length;
  }
  return null;
}

test("W10 setup loop and ensure_glass stay macos-gated", () => {
  const lib = src("src-tauri/src/lib.rs");
  const bootAt = lib.indexOf("let boot_ctx");
  assert.ok(bootAt > 0, "let boot_ctx");
  let i = bootAt;
  while (i > 0 && /\s/.test(lib[i - 1])) i--;
  assert.equal(lib[i - 1], "{", "boot_ctx sits in a block");
  const open = i - 1;
  const attr = '#[cfg(target_os = "macos")]';
  let j = open;
  while (j > 0 && /\s/.test(lib[j - 1])) j--;
  assert.equal(lib.slice(j - attr.length, j), attr, "block directly preceded by macos cfg");
  const boot = balanced(lib, open);
  assert.ok(boot.text.includes("let boot_ctx"));
  assert.ok(boot.text.includes(SETUP_LABELS), "setup loop inside the cfg block");
  assert.ok(boot.text.includes("apply_lights(&w, label, boot_ctx)"));
  const forAt = boot.text.indexOf(SETUP_LABELS);
  const forOpen = boot.text.indexOf("{", forAt);
  const loop = balanced(boot.text, forOpen);
  assert.ok(loop.end < boot.text.length, "whole setup loop inside the cfg block");
  assert.ok(boot.text.indexOf("let boot_ctx") < forAt);

  const g0 = lib.indexOf("fn ensure_glass(");
  const g1 = lib.indexOf("\nfn ", g0 + 10);
  assert.ok(g0 > 0 && g1 > g0, "ensure_glass fn");
  const glass = lib.slice(g0, g1);
  const glassBlock = macosCfgBlockContaining(glass, SETUP_LABELS);
  assert.ok(glassBlock, "ensure_glass keeps its macos cfg around the loop");
  assert.match(glass, /#\[cfg\(target_os = "macos"\)\]/);
});

test("W11 window_chrome eprintln stays inside chrome_log", () => {
  const rs = src("src-tauri/src/windows/window_chrome.rs");
  const uses: number[] = [];
  const needle = "eprintln!";
  let from = 0;
  while (from < rs.length) {
    const at = rs.indexOf(needle, from);
    if (at < 0) break;
    uses.push(at);
    from = at + needle.length;
  }
  if (uses.length === 0) return;
  const macros = [...rs.matchAll(/macro_rules!\s+chrome_log\b/g)];
  assert.equal(macros.length, 1, "single chrome_log macro");
  const macroAt = macros[0].index ?? -1;
  const bodyOpen = rs.indexOf("{", macroAt);
  const body = balanced(rs, bodyOpen);
  assert.match(body.text, /cfg!\(debug_assertions\)/);
  assert.match(body.text, /eprintln!/);
  for (const at of uses) {
    assert.ok(
      at >= bodyOpen && at < body.end,
      `eprintln outside chrome_log at ${at}`,
    );
  }
});

test("W12 title text is bound inside the h1", () => {
  const row = src("src/lib/WindowTitleRow.svelte");
  const h1 = row.match(/<h1\s+class="wtr-title"\s*>([\s\S]*?)<\/h1>/);
  assert.ok(h1, "h1.wtr-title");
  const inner = h1[1].trim();
  assert.equal(inner, "{title}", `title text inside h1, got ${JSON.stringify(inner)}`);
  assert.equal(inner.length > 0, true);
  assert.equal(/^\{["']/.test(inner), false, "not a literal");

  const kanban = src("src/lib/KanbanPage.svelte");
  assert.match(
    kanban,
    /listTitle\s*=\s*\$derived\(\s*boardWindowTitle\(\s*selected\s*,\s*projects\s*\)\s*\)/,
  );
  assert.match(kanban, /<BoardChrome[\s\S]*?\{listTitle\}/);
  const chrome = src("src/lib/board/Chrome.svelte");
  const windowed = chrome.slice(chrome.indexOf("{:else}"));
  assert.match(windowed, /title=\{listTitle\}/);

  const stars = src("src/lib/StarsPage.svelte");
  const banner = stars.slice(stars.indexOf("{#if embedded}"));
  const starWindowed = banner.slice(banner.indexOf("{:else}"), banner.indexOf("{/if}"));
  assert.match(starWindowed, /<WindowTitleRow[\s\S]*title="Stars"/);
});
