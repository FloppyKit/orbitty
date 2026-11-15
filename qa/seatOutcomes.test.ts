// Seat outcome words are display-only. Open terminal, hide, and auto-hide
// keep reading the unchanged status field.
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

function src(rel: string): string {
  return readFileSync(join(root, rel), "utf8");
}

function sliceFn(text: string, name: string, nextName: string): string {
  const at = text.indexOf(name);
  assert.ok(at >= 0, name);
  const end = text.indexOf(nextName, at + name.length);
  assert.ok(end > at, nextName);
  return text.slice(at, end);
}

function walk(rel: string): string[] {
  const abs = join(root, rel);
  const out: string[] = [];
  for (const name of readdirSync(abs)) {
    const child = join(rel, name);
    if (statSync(join(root, child)).isDirectory()) out.push(...walk(child));
    else out.push(child);
  }
  return out;
}

test("peek_pill_reads_outcome_only_for_display", () => {
  const file = src("src/lib/habitat/PeekCapsule.svelte");
  const scriptEnd = file.lastIndexOf("</script>");
  assert.ok(scriptEnd >= 0);
  const script = file.slice(0, scriptEnd);
  const tpl = file.slice(scriptEnd);
  const apply = sliceFn(script, "function applyCapsule", "onMount");
  const refresh = sliceFn(script, "async function refresh", "function arm");
  assert.match(apply, /said = v\.outcome \|\| ""/);
  assert.match(refresh, /said = v\.outcome \|\| ""/);
  const spanAt = tpl.indexOf('class="status"');
  assert.ok(spanAt >= 0);
  const span = tpl.slice(spanAt, tpl.indexOf("</span>", spanAt));
  assert.match(span, /\{#if said\}/);
  assert.match(span, /\{said\}/);
  assert.match(span, /\{status\}\{termState \? ` · \$\{termState\}` : ""\}/);
  assert.match(script, /noteSeat\(status/);
  assert.match(tpl, /\{#if status !== "unknown"\}/);
  assert.equal(script.includes("status = v.outcome"), false);
  assert.equal(script.includes("status = outcome"), false);
});

test("open_terminal_error_still_shows", () => {
  const file = src("src/lib/habitat/PeekCapsule.svelte");
  const script = file.slice(0, file.lastIndexOf("</script>"));
  const fn = sliceFn(script, "async function openTerminal", "function applyCapsule");
  assert.match(fn, /status = "Nothing to open for this card."/);
  assert.match(fn, /status = v\.reason/);
  assert.match(fn, /status = typeof e === "string"/);
  const clears = fn.split('said = ""').length - 1;
  assert.ok(clears >= 3, String(clears));
});

test("none_with_said_keeps_pill_empty_said_hides", () => {
  const file = src("src/lib/habitat/PeekCapsule.svelte");
  const scriptEnd = file.lastIndexOf("</script>");
  const script = file.slice(0, scriptEnd);
  const note = sliceFn(script, "function noteSeat", "async function openTerminal");
  const noneAt = note.indexOf('if (status === "none")');
  assert.ok(noneAt >= 0);
  const blockedAt = note.indexOf('status === "blocked"', noneAt);
  assert.ok(blockedAt > noneAt);
  const noneBranch = note.slice(noneAt, blockedAt);
  const keepAt = noneBranch.indexOf("if (said) return;");
  const hideAt = noneBranch.indexOf("hideCapsule()");
  assert.ok(keepAt >= 0, "flag on + outcome (said) keeps the pill");
  assert.ok(hideAt > keepAt, "flag off / empty said still hides");
  assert.equal(noneBranch.slice(keepAt, hideAt).includes("hideCapsule"), false);
  const tpl = file.slice(scriptEnd);
  assert.match(tpl, /\{#if status !== "unknown"\}/);
  assert.match(tpl, /onclick=\{hideCapsule\}/);
});

test("preview_none_hide_does_not_see_the_capsule_event", () => {
  const preview = src("src/lib/habitat/PeekPreview.svelte");
  const capsule = src("src/lib/habitat/PeekCapsule.svelte");
  const prevAt = preview.indexOf('listen<Capsule>("herdr-peek"');
  assert.ok(prevAt >= 0);
  const prevListen = preview.slice(prevAt, preview.indexOf(".then", prevAt));
  assert.match(prevListen, /target:\s*"peek-preview"/);
  const capAt = capsule.indexOf('listen<Capsule>("herdr-peek"');
  assert.ok(capAt >= 0);
  const capListen = capsule.slice(capAt, capsule.indexOf(".then", capAt));
  assert.match(capListen, /target:\s*"peek-capsule"/);
  const script = capsule.slice(0, capsule.lastIndexOf("</script>"));
  const apply = sliceFn(script, "function applyCapsule", "onMount");
  assert.match(apply, /if \(!said\) noteSeat\(status, v\.up\)/);
  assert.match(apply, /if \(status === "none" && !said\) return/);
  const note = sliceFn(script, "function noteSeat", "async function openTerminal");
  assert.match(note, /hideReason = "none-status"/);
  assert.match(note, /hideCapsule\(\)/);
  assert.match(note, /hideReason = "down-streak"/);
  const panes = src("src-tauri/src/windows/panes.rs");
  assert.match(panes, /"peek-capsule-hide"/);
  const dismiss = src("src-tauri/src/habitat/habitat_mac/peek_dismiss.rs");
  assert.match(dismiss, /"esc"/);
  assert.match(dismiss, /"click-away"/);
});

test("rail_and_habitat_mac_untouched_names", () => {
  const files = [
    ...walk("src/lib/rail"),
    "src/lib/RailPage.svelte",
    ...walk("src-tauri/src/habitat/habitat_mac"),
  ];
  assert.ok(files.length > 3);
  for (const rel of files) {
    const text = src(rel);
    assert.equal(text.includes("seat.outcomes"), false, rel);
    assert.equal(text.includes("seatOutcomes"), false, rel);
  }
});
