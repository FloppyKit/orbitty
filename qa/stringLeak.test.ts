import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

test("the four string helpers no longer forget every call", () => {
  for (const rel of [
    "src-tauri/src/habitat/habitat_mac/mod.rs",
    "src-tauri/src/yard/yard_mac/mod.rs",
    "src-tauri/src/windows/windows_mac_ax.rs",
    "src-tauri/src/windows/glass.rs",
  ]) {
    assert.equal(src(rel).includes("mem::forget(cf)"), false, rel);
  }
  assert.match(src("src-tauri/src/windows/windows_mac.rs"), /mem::forget\(cf\)/);
  assert.match(src("src-tauri/src/ns_hold.rs"), /fn screen_number_key_is_stable/);
});
