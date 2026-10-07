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
  assert.match(style, /var\(--edge\)/);
  assert.match(style, /var\(--bg-fill, var\(--bg\)\)/);
  const page = src("src/routes/+page.svelte");
  const terminal = page.slice(page.indexOf('data-theme="terminal"'), page.indexOf("S4.4"));
  assert.match(terminal, /color-scheme:\s*dark/);
  assert.match(row, /window_appearance/);
});
