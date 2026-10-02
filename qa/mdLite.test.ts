import assert from "node:assert/strict";
import { test } from "node:test";
import { mdLite } from "../src/lib/mdLite.ts";

test("link-free markdown is unchanged", () => {
  const src = "**bold** and a list\n\n- item\n\nHello\n\nthere";
  assert.equal(
    mdLite(src),
    "<strong>bold</strong> and a list<ul><li>item</li></ul>Hello<br><br>there",
  );
  assert.equal(mdLite("2 * 3 * 4"), "2 * 3 * 4");
  assert.equal(mdLite("`code`"), "<code>code</code>");
});

test("hostile strings are not links", () => {
  for (const src of [
    "javascript:alert(1)",
    "JAVASCRIPT:alert(1)",
    "data:text/html,<script>alert(1)</script>",
    "http://example.com",
    "vbscript:msgbox(1)",
    "blob:https://example.com/x",
    "<script>alert(1)</script>",
    '<img src=x onerror="alert(1)">',
  ]) {
    const html = mdLite(src);
    assert.equal(html.includes("data-link"), false, src);
    assert.equal(html.includes("<script"), false, src);
    assert.equal(html.includes("<img"), false, src);
    assert.equal(html.includes("href="), false, src);
  }
  assert.equal(mdLite("javascript:alert(1)"), "javascript:alert(1)");
  assert.equal(
    mdLite("<script>alert(1)</script>"),
    "&lt;script&gt;alert(1)&lt;/script&gt;",
  );
});

test("safe urls become data-link spans without href", () => {
  const https = mdLite("see https://example.com/a.");
  assert.equal(
    https,
    'see <a data-link="https://example.com/a">https://example.com/a</a>.',
  );
  const file = mdLite("file:///Users/dude/Orbitty%20GOs/x.md");
  assert.match(file, /data-link="file:\/\/\/Users\/dude\/Orbitty%20GOs\/x.md"/);
  assert.equal(file.includes("href="), false);
  const bare = mdLite("/Users/dude/notes.md");
  assert.match(bare, /data-link="\/Users\/dude\/notes.md"/);
});

test("urls inside code stay literal", () => {
  const html = mdLite("`https://example.com`");
  assert.equal(html, "<code>https://example.com</code>");
  assert.equal(html.includes("data-link"), false);
});

test("quote breakout does not become an attribute", () => {
  const html = mdLite('https://evil.com" onclick="alert(1)');
  assert.equal(/<a[^>]*onclick/.test(html), false);
  assert.match(html, /data-link="https:\/\/evil.com"/);
});

test("attrEscape escapes gt and apostrophe", () => {
  const gt = mdLite("https://example.com/x>y");
  assert.match(gt, /data-link="https:\/\/example.com\/x&gt;y"/);
  assert.equal(gt.includes("'"), false);
  const apos = mdLite("https://example.com/x'y");
  assert.match(apos, /data-link="https:\/\/example.com\/x&#39;y"/);
  assert.equal(apos.includes("data-link=\"https://example.com/x'y\""), false);
});

test("ampersands in urls are escaped once", () => {
  const html = mdLite("https://example.com/q?a=1&b=2");
  assert.equal(
    html,
    '<a data-link="https://example.com/q?a=1&amp;b=2">https://example.com/q?a=1&amp;b=2</a>',
  );
  assert.equal(html.includes("&amp;amp;"), false);
  const m = html.match(/data-link="([^"]*)"/);
  assert.equal(m?.[1].replace(/&amp;/g, "&"), "https://example.com/q?a=1&b=2");
});
