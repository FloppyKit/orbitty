import assert from "node:assert/strict";
import { test } from "node:test";
import { parseFlagReply } from "../src/lib/flags.ts";

test("flags_get reply parses on and source", () => {
  assert.deepEqual(parseFlagReply({ on: true, source: "default" }), {
    on: true,
    source: "default",
  });
  assert.deepEqual(parseFlagReply({ on: false, source: "env" }), {
    on: false,
    source: "env",
  });
  assert.deepEqual(parseFlagReply({ on: true, source: "auto-verify" }), {
    on: true,
    source: "auto-verify",
  });
  assert.throws(() => parseFlagReply({ on: "yes", source: "file" }));
  assert.throws(() => parseFlagReply({ on: true }));
  assert.throws(() => parseFlagReply({ source: "default" }));
  assert.throws(() => parseFlagReply(null));
});
