import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

function fnBody(file: string, name: string): string {
  const start = file.indexOf(`export async function ${name}`);
  assert.notEqual(start, -1, name);
  const next = file.indexOf("\nexport ", start + 10);
  return file.slice(start, next === -1 ? undefined : next);
}

test("requestLink goes through click_link", () => {
  const file = src("src/lib/linkOpen.svelte.ts");
  const body = fnBody(file, "requestLink");
  assert.match(body, /invoke<LinkClick>\("click_link"/);
  assert.equal(body.includes("open_link"), false);
  assert.equal(body.includes("open_card_link"), false);
});

test("confirmOpen sends the preview fingerprint to open_link", () => {
  const file = src("src/lib/linkOpen.svelte.ts");
  const body = fnBody(file, "confirmOpen");
  assert.match(body, /invoke<LinkClick>\("open_link"/);
  for (const key of ["kind:", "size:", "modifiedSecs:", "contentSha256:", "plistSha256:"]) {
    assert.equal(body.includes(key), true, key);
  }
  assert.equal(body.includes("open_card_link"), false);
  assert.equal(body.includes("click_link"), false);
});

test("card surfaces call requestLink and do not mount a second popover", () => {
  const callers = [
    "src/lib/KanbanPage.svelte",
    "src/lib/CaptionPane.svelte",
    "src/lib/NextCard.svelte",
  ];
  for (const rel of callers) {
    const file = src(rel);
    assert.equal(file.includes("requestLink"), true, rel);
    assert.equal(file.includes("LinkPreviewPopover"), false, rel);
    assert.equal(file.includes('invoke("click_link"'), false, rel);
    assert.equal(file.includes('invoke("open_link"'), false, rel);
    assert.equal(file.includes("open_card_link"), false, rel);
  }
  const page = src("src/routes/+page.svelte");
  assert.equal(page.includes("LinkPreviewPopover"), true);
  const pop = src("src/lib/LinkPreviewPopover.svelte");
  assert.equal(pop.includes("claimPreviewPopover"), true);
  assert.equal(pop.includes("mine && linkUi.preview"), true);
});
