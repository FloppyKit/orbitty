import assert from "node:assert/strict";
import { test } from "node:test";
import {
  classifyNeeded,
  failHasText,
  fixTitle,
  showPassFail,
} from "../src/lib/neededFlavor.ts";

test("dogfood titles classify as dogfood", () => {
  const c = classifyNeeded("Dogfood GO-ORBITTY-KITCHEN-FIX a161833e?", "");
  assert.equal(c.flavor, "dogfood");
  assert.equal(c.go, "GO-ORBITTY-KITCHEN-FIX");
  assert.equal(showPassFail("needed", "open", "Dogfood GO-ORBITTY-KITCHEN-FIX a161833e?", ""), true);
  assert.equal(showPassFail("needed", "later", "Dogfood GO-ORBITTY-KITCHEN-FIX a161833e?", ""), false);
});

test("writer and fact neededs do not get Pass/Fail", () => {
  assert.equal(classifyNeeded("Writer for GO-ORBITTY-X?", "").flavor, "writer");
  assert.equal(showPassFail("needed", "open", "Writer for GO-ORBITTY-X?", ""), false);
  assert.equal(classifyNeeded("What time is dinner?", "").flavor, "fact");
  assert.equal(showPassFail("needed", "open", "What time is dinner?", ""), false);
  assert.equal(showPassFail("todo", "open", "Dogfood GO-ORBITTY-X?", ""), false);
});

test("fail needs text after the prefix", () => {
  assert.equal(failHasText("FAIL: "), false);
  assert.equal(failHasText("FAIL:"), false);
  assert.equal(failHasText("PASS"), false);
  assert.equal(failHasText("FAIL: the Go button did nothing"), true);
});

test("fix needed title keeps the go and tip", () => {
  assert.equal(fixTitle("GO-ORBITTY-KITCHEN-FIX", "a161833e"), "Fix GO-ORBITTY-KITCHEN-FIX a161833e?");
});
