// 27p-b1: expanded chat is the title row, the transcript, and one input line.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expandKey, type ExpandKeyEvent } from "../src/lib/companion/expandKey.ts";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

function template(file: string): string {
  const at = file.lastIndexOf("</script>");
  assert.ok(at >= 0);
  return file.slice(at + "</script>".length);
}

/** Condition of the `{#if}` that contains `index`, or null when none does. */
function enclosingIf(tpl: string, index: number): string | null {
  const re = /\{#if\s+([^}]+)\}|\{:else if\s+([^}]+)\}|\{:else\}|\{\/if\}/g;
  const stack: { cond: string | null }[] = [];
  let m: RegExpExecArray | null;
  while ((m = re.exec(tpl))) {
    if (m.index > index) break;
    if (m[0].startsWith("{#if")) stack.push({ cond: m[1].trim() });
    else if (m[0].startsWith("{:else if")) {
      if (stack.length) stack[stack.length - 1].cond = m[2].trim();
    } else if (m[0].startsWith("{:else}")) {
      if (stack.length) stack[stack.length - 1].cond = null;
    } else stack.pop();
  }
  return stack.length ? stack[stack.length - 1].cond : null;
}

function hit(over: Partial<ExpandKeyEvent> = {}): ExpandKeyEvent {
  return { key: "e", ...over };
}

test("expand_key_table", () => {
  assert.equal(expandKey(hit({ metaKey: true, shiftKey: true })), "toggle");
  assert.equal(expandKey(hit({ key: "E", metaKey: true, shiftKey: true })), "toggle");
  assert.equal(expandKey(hit({ ctrlKey: true, shiftKey: true })), "toggle");
  assert.equal(expandKey(hit({ key: "e" })), null);
  assert.equal(expandKey(hit({ key: "E", shiftKey: true })), null);
  assert.equal(expandKey(hit({ metaKey: true })), null);
  assert.equal(expandKey(hit({ key: "E", metaKey: true })), null);
  assert.equal(expandKey(hit({ metaKey: true, shiftKey: true, isComposing: true })), null);
  assert.equal(expandKey(hit({ metaKey: true, shiftKey: true }), { composing: true }), null);
  assert.equal(expandKey(hit({ metaKey: true, shiftKey: true, altKey: true })), null);
  assert.equal(expandKey(hit({ key: "o", metaKey: true, shiftKey: true })), null);
  assert.equal(expandKey(hit({ key: "n", metaKey: true, shiftKey: true })), null);
  assert.equal(expandKey(hit({ key: "y", metaKey: true, shiftKey: true })), null);

  const lib = src("src-tauri/src/lib.rs");
  assert.equal(lib.includes("Code::KeyE"), false);
  const chords = src("src-tauri/src/windows/tilechords.rs");
  assert.equal(chords.includes("KeyE"), false);
});

test("expanded_markup_no_chips_no_launcher", () => {
  const page = src("src/lib/CompanionPage.svelte");
  const dockFile = src("src/lib/CompanionDock.svelte");
  const dock = template(dockFile);
  const css = src("src/lib/companion/page.css");

  const left = dock.indexOf("<DockAgentLeft");
  const right = dock.indexOf("<DockAgentRight");
  const mouth = dock.indexOf("<DockMouth");
  const launcher = dock.indexOf("<DockLauncher");
  const cheat = dock.indexOf("<DockCheatHint");
  assert.ok(left > 0 && right > left && mouth > right && launcher > mouth);
  assert.equal(enclosingIf(dock, left), "!expanded");
  assert.equal(enclosingIf(dock, right), "!expanded");
  assert.equal(enclosingIf(dock, cheat), "!expanded");
  assert.equal(enclosingIf(dock, mouth), null);
  assert.equal(enclosingIf(dock, launcher), "!expanded");
  assert.match(dock.slice(mouth, mouth + 500), /variant=\{expanded \? "line" : "dock"\}/);
  assert.equal(dock.includes("Dests"), false);
  assert.equal(dock.includes("class=\"dest"), false);

  const block = page.slice(page.indexOf("{#if expanded}"), page.indexOf("{#if expanded && sprite}"));
  assert.match(block, /<WindowTitleRow/);
  assert.match(block, /title="Orbitty"/);
  assert.match(block, /hide_window/);
  assert.equal(block.includes("DockAgentLeft"), false);
  assert.equal(block.includes("DockLauncher"), false);
  assert.equal(block.includes("<DockMouth"), false);
  assert.match(page, /\{#if expanded && sprite\}/);
  assert.match(css, /\.companion\.chrome \.hostwell\s*\{[^}]*display:\s*none/);
  assert.match(css, /\.companion\.chrome\.hosting\s*\{[^}]*background:\s*var\(--bg-fill, var\(--bg\)\)/);
});

test("mouth_line_variant_sends_via_sendLine", () => {
  const mouth = src("src/lib/companion/Mouth.svelte");
  assert.match(mouth, /variant\?: "line" \| "dock"/);
  assert.match(mouth, /class:line=\{variant === "line"\}/);
  const form = mouth.slice(mouth.indexOf("<form"), mouth.indexOf("</form>"));
  assert.match(form, /sendLine\(\)/);
  assert.equal(form.split("sendLine()").length - 1, 1);
  assert.match(form, /type="submit"/);
  assert.match(form, /aria-label="Attach one file"/);
  assert.match(form, /aria-label="Talk"/);
  assert.match(form, /aria-label="Send"/);
  assert.match(form, /\{#if variant !== "line"\}/);
  const lineCss = src("src/lib/companion/dock.css");
  const rule = lineCss.slice(lineCss.indexOf(".composer.line {"));
  assert.match(rule, /\.composer\.line \{[^}]*flex-wrap:\s*nowrap/);
  assert.doesNotMatch(rule.slice(0, rule.indexOf(".composer.line .chiprow")), /#[0-9a-fA-F]{3,8}|rgb\(/);
});

test("collapse_glyph_no_drag_aria", () => {
  const page = src("src/lib/CompanionPage.svelte");
  const snippetAt = page.indexOf("{#snippet collapseToDock()}");
  assert.ok(snippetAt > 0);
  const snippet = page.slice(snippetAt, page.indexOf("{/snippet}", snippetAt));
  assert.match(snippet, /data-no-drag/);
  assert.match(snippet, /aria-label="Collapse to dock"/);
  assert.match(snippet, /onclick=\{toggleExpanded\}/);
  assert.equal(snippet.includes("hide_window"), false);
  assert.doesNotMatch(snippet, /\b(rail|host|seat)\b/i);
  const block = page.slice(page.indexOf("{#if expanded}"), page.indexOf("{#if expanded && sprite}"));
  assert.match(block, /chips: collapseToDock/);
  assert.match(block, /hide_window/);
  const css = src("src/lib/companion/page.css");
  assert.match(css, /\.companion\.chrome \.wtr-chips\s*\{[^}]*justify-content:\s*flex-end/);
  const glyph = css.slice(css.indexOf(".collapse-dock {"), css.indexOf(".collapse-dock svg"));
  assert.doesNotMatch(glyph, /#[0-9a-fA-F]{3,8}|rgb\(/);
  assert.match(glyph, /var\(--ink\)/);
});

test("esc_ladder_order_unchanged", () => {
  const page = src("src/lib/CompanionPage.svelte");
  const start = page.indexOf("function onPaneKey");
  const end = page.indexOf("function toggleView");
  assert.ok(start > 0 && end > start);
  const fn = page.slice(start, end);
  const chord = fn.indexOf("expandKey(");
  const escape = fn.indexOf('if (e.key !== "Escape") return;');
  const field = fn.indexOf('closest?.("input, textarea, select")');
  const blur = fn.indexOf("t.blur()");
  const fieldReturn = fn.indexOf("return;", blur);
  const collapse = fn.indexOf('if (hosted !== "off" && expanded)');
  const clear = fn.indexOf("expanded = false;");
  const dismiss = fn.indexOf("dismissHost()");
  assert.ok(chord >= 0 && chord < escape, "chord is before the Escape ladder");
  assert.ok(escape < field && field < blur && blur < fieldReturn, "field owns Esc first");
  assert.ok(fieldReturn < collapse && collapse < clear && clear < dismiss, "collapse then dismiss");
  assert.match(fn, /set_companion_expanded/);
  assert.equal(fn.includes("turns = []"), false);
});
