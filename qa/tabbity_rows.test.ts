import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import type { Claim, WindowInfo } from "../src/lib/persist.ts";
import { isTabClaim, keptTab, rematchPick, setTabLive, tabRow } from "../src/lib/rail/tabbityRows.ts";

function win(partial: Partial<WindowInfo> & { id: string }): WindowInfo {
  return {
    pid: 1,
    appName: "Brave",
    title: "page",
    bundleId: "com.brave.Browser",
    stableKey: "com.brave.Browser|page|1",
    iconBase64: null,
    isFrontmost: false,
    isMinimized: false,
    ...partial,
  };
}

function tabClaim(id: string): Claim {
  return {
    windowId: id,
    projectId: "A",
    lastTouchedAt: 1,
    tab: {
      browser: "brave",
      url: "https://example.test/a",
      title: "Example",
      lastSeenAt: 1,
    },
    charSeed: 42,
    spritePackId: "pack-1",
    spriteScale: 1.2,
  };
}

test("walker Assign of a tab claim keeps tab", () => {
  const prev = tabClaim("tab:11111111-1111-4111-8111-111111111111");
  assert.equal(keptTab(prev)?.url, "https://example.test/a");
  assert.equal(keptTab(undefined), null);
  const being = readFileSync(new URL("../src/lib/rail/pageBeing.svelte.ts", import.meta.url), "utf8");
  const claimAt = being.indexOf("export function claim(");
  const body = being.slice(claimAt, being.indexOf("export function unclaim("));
  assert.match(body, /tab: prev\?\.tab/);
  const next = readFileSync(new URL("../src/lib/NextCard.svelte", import.meta.url), "utf8");
  assert.match(next, /tab: prev\?\.tab/);
});

test("orphan tab claim is not adopted onto the one Brave window", () => {
  const brave = win({ id: "pid:1" });
  const orphan = tabClaim("tab:11111111-1111-4111-8111-111111111111");
  orphan.bundleId = "com.brave.Browser";
  orphan.appName = "Brave";
  assert.equal(rematchPick(orphan, [brave], new Set(), null), undefined);
  assert.equal(isTabClaim(orphan), true);
});

test("orphan Brave window claim is not adopted onto a tab row", () => {
  const windowClaim: Claim = {
    windowId: "old-brave",
    projectId: "A",
    lastTouchedAt: 1,
    bundleId: "com.brave.Browser",
    appName: "Brave",
    titleAtClaim: "page",
  };
  const tabRowWin = win({
    id: "tab:22222222-2222-4222-8222-222222222222",
    title: "page",
  });
  assert.equal(rematchPick(windowClaim, [tabRowWin], new Set(), null), undefined);
  const real = win({ id: "pid:9", title: "other" });
  const picked = rematchPick(windowClaim, [real], new Set(), null);
  assert.equal(picked?.id, "pid:9");
});

test("tab row is live or asleep and keeps the claim id", () => {
  const c = tabClaim("tab:11111111-1111-4111-8111-111111111111");
  setTabLive([]);
  const asleep = tabRow(c, []);
  assert.equal(asleep.sleeping, true);
  assert.equal(asleep.id, c.windowId);
  assert.equal(asleep.title, "Example");
  assert.equal(asleep.appName, "Brave");
  setTabLive([{ id: c.windowId, live: true }]);
  const live = tabRow(c, []);
  assert.equal(live.sleeping, false);
  const look = readFileSync(new URL("../src/lib/rail/pageClaim.svelte.ts", import.meta.url), "utf8");
  assert.match(look, /charSeed: c\?\.charSeed/);
  assert.match(look, /spritePackId: c\?\.spritePackId/);
  const restore = look.slice(look.indexOf("export async function restore("));
  assert.match(restore, /restore_window/);
  assert.match(restore, /id: w\.id/);
});
