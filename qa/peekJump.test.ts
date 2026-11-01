// Jump from the peek bubble: one button, only when the status is known.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const file = readFileSync(new URL("../src/lib/habitat/PeekCapsule.svelte", import.meta.url), "utf8");

function template(src: string): string {
  const at = src.lastIndexOf("</script>");
  assert.ok(at >= 0);
  return src.slice(at + "</script>".length);
}

test("open terminal button only when status is known", () => {
  const tpl = template(file);
  const rowAt = tpl.indexOf('class="row"');
  assert.ok(rowAt >= 0);
  const rowEnd = tpl.indexOf("</div>", rowAt);
  assert.ok(rowEnd > rowAt);
  const row = tpl.slice(rowAt, rowEnd);
  assert.match(row, /Open terminal/);
  assert.match(row, /status !== "unknown"/);
  const btn = row.indexOf("Open terminal");
  const guard = row.lastIndexOf('status !== "unknown"', btn);
  assert.ok(guard >= 0 && guard < btn);
  assert.equal(tpl.slice(rowEnd).includes("Open terminal"), false);
  assert.match(row, />peek</);
  assert.match(row, />hide</);
});

test("open terminal click invokes herdr_jump with the capsule card", () => {
  const scriptEnd = file.lastIndexOf("</script>");
  const script = file.slice(0, scriptEnd);
  assert.match(script, /async function openTerminal/);
  assert.match(script, /invoke<\{ ok\?: boolean; reason\?: string \}>\("herdr_jump", \{ card \}\)/);
  assert.match(script, /status = v\.reason/);
  assert.equal(script.includes("observe"), false);
  const tpl = template(file);
  assert.match(tpl, /onclick=\{openTerminal\}/);
});
