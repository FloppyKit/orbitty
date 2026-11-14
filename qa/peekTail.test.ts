// The peek bubble opens on the last lines. A reader who scrolls up is left there.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const file = readFileSync(new URL("../src/lib/habitat/PeekPreview.svelte", import.meta.url), "utf8");

function scriptOf(src: string): string {
  const at = src.lastIndexOf("</script>");
  assert.ok(at >= 0);
  return src.slice(0, at);
}

function templateOf(src: string): string {
  const at = src.lastIndexOf("</script>");
  return src.slice(at + "</script>".length);
}

test("peek body scrolls to the bottom after refresh unless the reader scrolled up", () => {
  const script = scriptOf(file);
  const refreshAt = script.indexOf("async function refresh");
  assert.ok(refreshAt >= 0);
  const armAt = script.indexOf("function arm", refreshAt);
  assert.ok(armAt > refreshAt);
  const refresh = script.slice(refreshAt, armAt);
  assert.match(refresh, /await stickToBottom\(\)/);

  const nearAt = script.indexOf("function nearBottom");
  const stickAt = script.indexOf("async function stickToBottom");
  assert.ok(nearAt >= 0 && stickAt > nearAt);
  const near = script.slice(nearAt, stickAt);
  assert.match(near, /el\.scrollHeight - el\.scrollTop - el\.clientHeight <= 2/);

  const stickEnd = script.indexOf("async function refresh", stickAt);
  assert.ok(stickEnd > stickAt);
  const stick = script.slice(stickAt, stickEnd);
  assert.match(stick, /const follow = followBottom/);
  assert.match(stick, /if \(!follow\) return/);
  assert.match(stick, /found\.scrollTop = found\.scrollHeight/);
  assert.match(script, /addEventListener\("scroll"/);
  assert.match(script, /followBottom = nearBottom\(el\)/);

  const openAt = script.indexOf("function openPeek");
  const openEnd = script.indexOf("function closePeek", openAt);
  assert.ok(openAt >= 0 && openEnd > openAt);
  assert.match(script.slice(openAt, openEnd), /followBottom = true/);

  const tpl = templateOf(file);
  assert.match(tpl, /<pre class="body" tabindex="-1">\{text\}<\/pre>/);
  assert.match(tpl, /max-height:\s*240px/);
  assert.doesNotMatch(tpl, /onscroll|bind:this/);
});
