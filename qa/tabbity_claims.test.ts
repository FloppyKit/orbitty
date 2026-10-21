import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  EXTENSION_ID,
  bindReopen,
  favicon,
  handleCommand,
  mintId,
  onAttached,
  onDetached,
  onMoved,
  onRemoved,
  onReplaced,
  onUpdated,
  reconcile,
  rematch,
} from "../tabbity/claims.js";

const id = "tab:11111111-1111-4111-8111-111111111111";

function claim(partial = {}) {
  return {
    id,
    browser: "brave",
    url: "https://example.test/a",
    title: "A",
    favIconUrl: null,
    windowOrdinal: 0,
    index: 1,
    tabId: 3,
    windowId: 1,
    lastSeenAt: 1,
    ...partial,
  };
}

test("manifest is pinned", () => {
  const manifest = JSON.parse(readFileSync(new URL("../tabbity/manifest.json", import.meta.url), "utf8"));
  assert.equal(manifest.manifest_version, 3);
  assert.deepEqual(manifest.permissions, ["nativeMessaging", "storage", "tabs"]);
  assert.equal(manifest.incognito, "not_allowed");
  assert.ok(manifest.key);
  assert.equal(EXTENSION_ID, "pbdnjlgnnkgbmnikjnfakajdkndgdicn");
  const sw = readFileSync(new URL("../tabbity/sw.js", import.meta.url), "utf8");
  for (const name of ["onUpdated", "onRemoved", "onReplaced", "onAttached", "onDetached", "onMoved"]) {
    assert.match(sw, new RegExp(`tabs\\.${name}`));
  }
  assert.match(sw, /DEBOUNCE_MS = 2000/);
  assert.match(sw, /connectNative/);
});

test("events keep the claim and update the binding", () => {
  let claims = [claim()];
  const updated = onUpdated(claims, 3, { url: "https://example.test/b", title: "B" }, { favIconUrl: "https://i.example/x.png" });
  assert.equal(updated.dirty, true);
  assert.equal(updated.claims[0].url, "https://example.test/b");
  assert.equal(updated.claims[0].id, id);
  claims = onRemoved(updated.claims, 3);
  assert.equal(claims[0].tabId, null);
  assert.equal(claims[0].id, id);
  claims = [{ ...claims[0], tabId: 3 }];
  claims = onReplaced(claims, 9, 3);
  assert.equal(claims[0].tabId, 9);
  claims = onMoved(claims, 9, { toIndex: 4, windowId: 1 });
  assert.equal(claims[0].index, 4);
  claims = onDetached(claims, 9);
  assert.equal(claims[0].windowId, null);
  claims = onAttached(claims, 9, { newWindowId: 2, newPosition: 0 }, [{ id: 1 }, { id: 2 }]);
  assert.equal(claims[0].windowId, 2);
  assert.equal(claims[0].windowOrdinal, 1);
  assert.equal(favicon("chrome://favicon"), null);
});

test("rematch is exact url with order tie-break", () => {
  const unbound = [
    { ...claim({ id: "tab:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", tabId: null, url: "https://same.test", windowOrdinal: 0, index: 0 }) },
    { ...claim({ id: "tab:bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", tabId: null, url: "https://same.test", windowOrdinal: 1, index: 2 }) },
    { ...claim({ id: "tab:cccccccc-cccc-4ccc-8ccc-cccccccccccc", tabId: null, url: "https://missing.test", tabId: null }) },
  ];
  const tabs = [
    { id: 10, windowId: 2, index: 2, url: "https://same.test", title: "far" },
    { id: 11, windowId: 1, index: 0, url: "https://same.test", title: "near" },
  ];
  const windows = [{ id: 1 }, { id: 2 }];
  const out = rematch(unbound, tabs, windows);
  assert.equal(out[0].tabId, 11);
  assert.equal(out[1].tabId, 10);
  assert.equal(out[2].tabId, null);
  const dup = rematch(
    [unbound[0], { ...unbound[0], id: "tab:dddddddd-dddd-4ddd-8ddd-dddddddddddd" }],
    [tabs[1]],
    windows,
  );
  assert.equal(dup.filter((c) => c.tabId === 11).length, 1);
});

test("reconcile drops local claims Orbitty no longer has", () => {
  const local = [claim(), claim({ id: "tab:eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee", url: "https://gone.test" })];
  const orbitty = [{ id, browser: "brave", url: "https://example.test/a", title: "A" }];
  const out = reconcile(local, orbitty);
  assert.deepEqual(out.map((c) => c.id), [id]);
  const added = reconcile([], [{ id: "tab:ffffffff-ffff-4fff-8fff-ffffffffffff", browser: "brave", url: "https://new.test", title: "N" }]);
  assert.equal(added[0].tabId, null);
  assert.equal(added[0].url, "https://new.test");
});

test("commands focus reopen close and unclaimed", () => {
  const claims = [claim()];
  assert.equal(handleCommand(claims, { cmd: "focus", claimId: id }).action.type, "focus");
  assert.equal(handleCommand(claims, { cmd: "close", claimId: id }).action.type, "close");
  assert.equal(handleCommand([{ ...claims[0], tabId: null }], { cmd: "reopen", claimId: id, url: "https://example.test/a" }).action.type, "reopen");
  const dropped = handleCommand(claims, { cmd: "unclaimed", ids: [id] });
  assert.equal(dropped.claims.length, 0);
  const rebound = bindReopen([{ ...claims[0], tabId: null }], id, { id: 40, windowId: 3, index: 1, url: "https://example.test/a" });
  assert.equal(rebound[0].tabId, 40);
  assert.equal(mintId(new Uint8Array(16).fill(1)).startsWith("tab:"), true);
});
