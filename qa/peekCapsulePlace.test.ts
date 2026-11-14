// Peek capsule is its own habitat pane, beside the walker, not a rail card.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

function sliceFn(src: string, name: string, nextName: string): string {
  const at = src.indexOf(name);
  assert.ok(at >= 0, name);
  const end = src.indexOf(nextName, at + name.length);
  assert.ok(end > at, nextName);
  return src.slice(at, end);
}

test("peekCapsulePlace", () => {
  const page = src("src/routes/+page.svelte");
  assert.match(page, /windowLabel === "peek-capsule"/);
  assert.equal(page.split("<PeekCapsule />").length - 1, 1);
  const peekBranch = page.slice(page.indexOf('windowLabel === "peek-capsule"'));
  assert.match(peekBranch, /<PeekCapsule \/>/);
  const rail = page.slice(page.lastIndexOf("{:else}"));
  assert.match(rail, /<RailPage \/>/);
  assert.equal(rail.includes("PeekCapsule"), false);

  const panes = src("src-tauri/src/windows/panes.rs");
  assert.match(panes, /"peek-capsule" => Some\(\("", 420\.0, 320\.0/);
  assert.match(panes, /fn show_peek_capsule/);
  assert.match(panes, /fn hide_peek_capsule/);
  const show = sliceFn(panes, "fn show_peek_capsule", "fn hide_peek_capsule");
  assert.match(show, /appkit_place_points/);
  assert.equal(show.includes("place_logical_rect"), false);

  assert.match(src("src-tauri/capabilities/default.json"), /"peek-capsule"/);

  const present = sliceFn(
    src("src-tauri/src/herdr/mod.rs"),
    "pub fn present",
    "pub fn status_for",
  );
  assert.match(present, /show_peek_capsule/);
  assert.match(present, /emit_to\(\s*"peek-capsule",\s*"herdr-peek"/);
  assert.equal(/app\.emit\(\s*"herdr-peek"/.test(present), false);
});

test("peekCapsulePlace seat end, hide, and timer", () => {
  const file = src("src/lib/habitat/PeekCapsule.svelte");
  const scriptEnd = file.lastIndexOf("</script>");
  const script = file.slice(0, scriptEnd);
  const tpl = file.slice(scriptEnd);

  const hide = sliceFn(script, "function hideCapsule", "function noteSeat");
  assert.match(hide, /invoke\("hide_peek_capsule"\)/);
  assert.match(tpl, /onclick=\{hideCapsule\}/);
  assert.equal(tpl.includes("shown = false"), false);

  const arm = sliceFn(script, "function arm", "function openPeek");
  assert.match(arm, /setInterval\(\(\) => void refresh\(\), 2000\)/);
  assert.match(arm, /if \(shown\)/);
  assert.equal(arm.includes("preview"), false);

  const close = sliceFn(script, "function closePeek", "async function openTerminal");
  assert.equal(close.includes("stop()"), false);

  const note = sliceFn(script, "function noteSeat", "async function openTerminal");
  assert.match(note, /status === "none"/);
  const noneAt = note.indexOf('status === "none"');
  const downAt = note.indexOf("up === false");
  const gateAt = note.indexOf("downStreak >= 2");
  assert.ok(noneAt >= 0 && downAt > noneAt && gateAt > downAt);
  assert.ok(note.indexOf("hideCapsule()") < gateAt);
  assert.equal(note.slice(downAt, gateAt).includes("hideCapsule"), false);
  assert.ok(note.slice(gateAt).includes("hideCapsule"));
  assert.match(note, /status === "unknown"/);
  assert.match(note, /status === "blocked"/);
});
