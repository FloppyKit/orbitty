// Walker click shows a one-row pills pane. Peek folds that row away and
// opens a separate preview. One Esc or click-away restores the pills.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

function sliceFn(text: string, name: string, nextName: string): string {
  const at = text.indexOf(name);
  assert.ok(at >= 0, name);
  const end = text.indexOf(nextName, at + name.length);
  assert.ok(end > at, nextName);
  return text.slice(at, end);
}

function scriptOf(text: string): string {
  const at = text.lastIndexOf("</script>");
  assert.ok(at >= 0);
  return text.slice(0, at);
}

test("peekCapsulePills mounts pills and preview in their own windows", () => {
  const page = src("src/routes/+page.svelte");
  assert.match(page, /windowLabel === "peek-capsule"/);
  assert.match(page, /windowLabel === "peek-preview"/);
  assert.equal(page.split("<PeekCapsule />").length - 1, 1);
  assert.equal(page.split("<PeekPreview />").length - 1, 1);

  const pillsAt = page.indexOf("isPeekCapsuleWindow}\n  <PeekCapsule />");
  const previewAt = page.indexOf("isPeekPreviewWindow}\n  <PeekPreview />");
  assert.ok(pillsAt >= 0 && previewAt > pillsAt);
  const pillsBranch = page.slice(pillsAt, previewAt);
  assert.match(pillsBranch, /<PeekCapsule \/>/);
  assert.equal(pillsBranch.includes("PeekPreview"), false);
  const previewBranch = page.slice(previewAt, page.indexOf("{:else}", previewAt));
  assert.match(previewBranch, /<PeekPreview \/>/);
  assert.equal(previewBranch.includes("PeekCapsule"), false);

  const rail = page.slice(page.lastIndexOf("{:else}"));
  assert.match(rail, /<RailPage \/>/);
  assert.equal(rail.includes("PeekCapsule"), false);
  assert.equal(rail.includes("PeekPreview"), false);

  const known = sliceFn(page, "const labelKnown", "const isDrawerWindow");
  assert.match(known, /isPeekCapsuleWindow/);
  assert.match(known, /isPeekPreviewWindow/);
});

test("peekCapsulePills pane specs, place, and allowlist", () => {
  const panes = src("src-tauri/src/windows/panes.rs");
  const pillsSpec = panes.match(/"peek-capsule" => Some\(\("",\s*([0-9.]+),\s*([0-9.]+)/);
  assert.ok(pillsSpec);
  assert.ok(Number(pillsSpec[2]) <= 48);
  assert.ok(Number(pillsSpec[2]) >= 24);
  const previewSpec = panes.match(
    /"peek-preview" => Some\(\("",\s*([0-9.]+),\s*([0-9.]+),\s*Some\(\(([0-9.]+),\s*([0-9.]+)\)\)/,
  );
  assert.ok(previewSpec);

  assert.match(panes, /fn show_peek_capsule/);
  assert.match(panes, /fn hide_peek_capsule/);
  assert.match(panes, /fn show_peek_preview/);
  assert.match(panes, /fn hide_peek_preview/);

  const showPills = sliceFn(panes, "fn show_peek_capsule", "fn hide_peek_capsule");
  assert.match(showPills, /appkit_place_points/);
  assert.equal(showPills.includes("place_logical_rect"), false);
  const showPreview = sliceFn(panes, "fn show_peek_preview", "fn hide_peek_preview");
  assert.match(showPreview, /appkit_place_points/);
  assert.equal(showPreview.includes("place_logical_rect"), false);
  assert.match(showPreview, /hide_labeled\(&app, "peek-capsule"\)/);
  assert.match(showPreview, /set_resizable\(true\)/);

  const resizableFalse = panes.indexOf("config.resizable = false");
  assert.ok(resizableFalse >= 0);
  const around = panes.slice(Math.max(0, resizableFalse - 160), resizableFalse);
  assert.match(around, /peek-capsule/);
  assert.equal(around.includes("peek-preview"), false);

  const caps = src("src-tauri/capabilities/default.json");
  assert.match(caps, /"peek-capsule"/);
  assert.match(caps, /"peek-preview"/);

  const present = sliceFn(src("src-tauri/src/herdr/mod.rs"), "pub fn present", "pub fn status_for");
  assert.match(present, /show_peek_capsule/);
  assert.match(present, /emit_to\(\s*"peek-capsule",\s*"herdr-peek"/);
  assert.equal(/app\.emit\(\s*"herdr-peek"/.test(present), false);
  assert.equal(present.includes("show_peek_preview"), false);
  assert.equal(present.includes("peek-preview"), false);
});

test("peekCapsulePills fold, scroll bar, and one-step dismiss", () => {
  const pills = src("src/lib/habitat/PeekCapsule.svelte");
  const pillsScript = scriptOf(pills);
  assert.equal(pills.includes('<pre class="body">'), false);
  assert.equal(pillsScript.includes("stickToBottom"), false);

  const open = sliceFn(pillsScript, "function openPeek", "function hideCapsule");
  assert.match(open, /invoke\("show_peek_preview"\)/);

  const hide = sliceFn(pillsScript, "function hideCapsule", "function noteSeat");
  assert.match(hide, /invoke\("hide_peek_capsule"/);
  assert.match(hide, /invoke\("hide_peek_preview", \{ restore: false \}\)/);

  const arm = sliceFn(pillsScript, "function arm", "function openPeek");
  assert.match(arm, /setInterval\(\(\) => void refresh\(\), 2000\)/);
  assert.match(arm, /if \(shown\)/);
  assert.equal(arm.includes("preview"), false);

  const note = sliceFn(pillsScript, "function noteSeat", "async function openTerminal");
  assert.match(note, /status === "none"/);
  const noneAt = note.indexOf('status === "none"');
  const downAt = note.indexOf("up === false");
  const gateAt = note.indexOf("downStreak >= 2");
  assert.ok(noneAt >= 0 && downAt > noneAt && gateAt > downAt);
  assert.equal(note.slice(downAt, gateAt).includes("hideCapsule"), false);
  assert.ok(note.slice(gateAt).includes("hideCapsule"));
  assert.match(note, /status === "blocked"/);

  const preview = src("src/lib/habitat/PeekPreview.svelte");
  const previewScript = scriptOf(preview);
  assert.match(preview, /<pre class="body"/);
  assert.match(preview, /::-webkit-scrollbar-thumb/);
  assert.match(preview, /overflow:\s*auto/);
  assert.match(preview, /startPaneDrag/);
  assert.match(preview, /class="strip"/);
  assert.match(src("src/lib/dragPane.ts"), /pane_drag_begin/);

  const close = sliceFn(previewScript, "function closePeek", "function hideBoth");
  assert.match(close, /invoke\("hide_peek_preview"\)/);
  assert.equal(close.includes("restore: false"), false);
  assert.equal(close.includes("hide_peek_capsule"), false);

  const previewNote = sliceFn(previewScript, "function noteSeat", "function applyCapsule");
  assert.match(previewNote, /status === "none"/);
  assert.match(previewNote, /hideBoth\(\)/);
  const pDown = previewNote.indexOf("up === false");
  const pGate = previewNote.indexOf("downStreak >= 2");
  assert.ok(pDown >= 0 && pGate > pDown);
  assert.equal(previewNote.slice(pDown, pGate).includes("hideBoth"), false);
  const both = sliceFn(previewScript, "function hideBoth", "function noteSeat");
  assert.match(both, /restore: false/);
  assert.match(both, /hide_peek_capsule/);

  const panes = src("src-tauri/src/windows/panes.rs");
  const step = sliceFn(panes, "fn peek_dismiss_one_step", "fn show_labeled");
  assert.match(step, /peek_layer_after_dismiss/);
  assert.ok(step.indexOf("hide_peek_preview") < step.indexOf("hide_peek_capsule"));
  const law = sliceFn(panes, "fn peek_layer_after_dismiss", "fn peek_click_is_away");
  assert.match(law, /PeekLayer::Preview => PeekLayer::Pills/);
  assert.match(law, /PeekLayer::Pills => PeekLayer::None/);

  const restore = sliceFn(panes, "fn hide_peek_preview", "fn reclamp_peek_preview");
  assert.match(restore, /show_pills_from_arm/);
  const back = sliceFn(panes, "fn show_pills_from_arm", "fn clear_peek_arm");
  assert.match(back, /"peek-capsule"/);
  assert.match(back, /appkit_place_points/);
  assert.equal(back.includes("place_logical_rect"), false);

  const reclamp = sliceFn(panes, "fn reclamp_peek_preview", "fn prewarm_peek_preview");
  assert.match(reclamp, /rect_fully_off_all/);
  assert.ok(reclamp.indexOf("rect_fully_off_all") < reclamp.indexOf("appkit_place_points"));
  assert.match(reclamp, /remembered_preview_size\(\)/);
  assert.equal(reclamp.includes("peek_preview_default_size"), false);

  const dismiss = src("src-tauri/src/habitat/habitat_mac/peek_dismiss.rs");
  assert.match(dismiss, /hit_being/);
  assert.match(dismiss, /Some\(\(_, true\)\)/);
  assert.match(dismiss, /peek_click_is_away/);
  assert.match(dismiss, /peek_dismiss_one_step/);
  assert.match(dismiss, /walker hit is not click-away/);
});
