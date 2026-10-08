import assert from "node:assert/strict";
import { test } from "node:test";
import { KIND_DOING, KIND_DONE, KIND_TODO } from "../src/lib/lifesprite/fold.ts";
import { boardReq, readRelayFrame } from "../src/lib/lifesprite/relay.ts";

test("REQ asks only for the three board kinds", () => {
  assert.deepEqual(boardReq(), ["REQ", "lifesprite", { kinds: [KIND_TODO, KIND_DOING, KIND_DONE] }]);
  assert.deepEqual(boardReq()[2].kinds.includes(31200), false);
});

test("relay frames: challenge, AUTH ok, events, EOSE, rejection", () => {
  assert.deepEqual(readRelayFrame(["AUTH", "deadbeef"]), {
    type: "challenge",
    challenge: "deadbeef",
  });
  assert.deepEqual(readRelayFrame(["OK", "id", true, ""]), { type: "auth-ok" });
  assert.deepEqual(readRelayFrame(["OK", "id", false, "auth-required: bad challenge"]), {
    type: "auth-rejected",
    detail: "auth-required: bad challenge",
  });
  const event = {
    id: "aa",
    pubkey: "bb",
    created_at: 1,
    kind: 420,
    tags: [
      ["t", "orbitty"],
      ["d", "c-1"],
    ],
    content: "{\"cardId\":\"c-1\",\"title\":\"Hi\"}",
    sig: "cc",
  };
  assert.deepEqual(readRelayFrame(["EVENT", "lifesprite", event]), { type: "event", event });
  assert.deepEqual(readRelayFrame(["EOSE", "lifesprite"]), { type: "eose" });
  assert.deepEqual(readRelayFrame(["CLOSED", "lifesprite", "auth-required: AUTH first"]), {
    type: "closed",
    detail: "auth-required: AUTH first",
  });
  assert.deepEqual(readRelayFrame(["NOTICE", "invalid: bad json"]), {
    type: "notice",
    detail: "invalid: bad json",
  });
  assert.deepEqual(readRelayFrame({ nope: true }), { type: "ignore" });
});
