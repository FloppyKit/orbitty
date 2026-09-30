import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const dockCss = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "../src/lib/companion/dock.css"),
  "utf8",
);

/** Bodies of rules whose selector list ends at `selector {`. */
function ruleBodies(css: string, selector: string): string[] {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const bodies = [...css.matchAll(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`, "g"))].map(
    (match) => match[1],
  );
  assert.ok(bodies.length > 0, `missing rule ${selector}`);
  return bodies;
}

function pinned(css: string, selector: string, column: number): void {
  const pin = new RegExp(`grid-column:\\s*${column}\\s*;`);
  assert.ok(
    ruleBodies(css, selector).some((body) => pin.test(body)),
    `${selector} is not pinned to column ${column}`,
  );
}

test("agentrow pins left, dests, and right to columns 1, 2, and 3", () => {
  const row = ruleBodies(dockCss, ".agentrow").join("\n");
  assert.match(
    row,
    /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+auto\s+minmax\(0,\s*1fr\)\s*;/,
  );
  pinned(dockCss, ".agentleft", 1);
  pinned(dockCss, ".dests", 2);
  pinned(dockCss, ".agentright", 3);
});
