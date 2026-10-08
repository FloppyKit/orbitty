import assert from "node:assert/strict";
import { test } from "node:test";
import { bech32 } from "@scure/base";
import { hexToBytes } from "@noble/hashes/utils.js";
import { KIND_AUTH, nip01Id, secretFromNsec, signAuth, verifySigned } from "../src/lib/lifesprite/auth.ts";

// Compact NIP-01 serialization. Same array serde_json writes for the relay:
// no whitespace, tags left in order. The id is sha256 of that UTF-8 string.
const NIP01 = {
  id: "8990a59a3d7482031e6a0daa921a1b9df580a8aa6ec24d6cb75532d2d4cf58bd",
  pubkey: "6e468422dfb74a5738702a8823b9b28168abab8655faacb6853cd0ee15deee93",
  created_at: 1673347337,
  kind: 1,
  tags: [
    ["e", "5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36", "wss://nostr.example.com"],
    ["p", "f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca"],
  ],
  content: "I am Satoshi Nakamoto",
};

test("NIP-01 id is the sha256 of the compact event array", () => {
  const id = nip01Id(NIP01.pubkey, NIP01.created_at, NIP01.kind, NIP01.tags, NIP01.content);
  assert.equal(id, NIP01.id);
  assert.equal(
    JSON.stringify([0, NIP01.pubkey, NIP01.created_at, NIP01.kind, NIP01.tags, NIP01.content]),
    '[0,"6e468422dfb74a5738702a8823b9b28168abab8655faacb6853cd0ee15deee93",1673347337,1,[["e","5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36","wss://nostr.example.com"],["p","f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca"]],"I am Satoshi Nakamoto"]',
  );
});

test("AUTH is kind 22242 with relay and challenge, and the secret is not on the event", () => {
  const hex = "1".padStart(64, "0"); // scalar 1, not a device key from this repo
  const words = bech32.toWords(hexToBytes(hex));
  const nsec = bech32.encode("nsec", words, 200);
  const fromHex = signAuth(hex, "ws://127.0.0.1:13120", "abc123", 1_700_000_000);
  const fromNsec = signAuth(nsec, "ws://127.0.0.1:13120", "abc123", 1_700_000_000);
  assert.equal(fromHex.kind, KIND_AUTH);
  assert.equal(fromHex.content, "");
  assert.deepEqual(fromHex.tags, [
    ["relay", "ws://127.0.0.1:13120"],
    ["challenge", "abc123"],
  ]);
  assert.equal(fromHex.pubkey, fromNsec.pubkey);
  assert.equal(verifySigned(fromHex), true);
  const dumped = JSON.stringify(fromHex);
  assert.equal(dumped.includes(hex), false);
  assert.equal(dumped.includes(nsec), false);
  assert.equal(secretFromNsec(nsec).length, 32);
});

test("a bad nsec fails closed without echoing the text", () => {
  const junk = "nsec1thisisnotakey";
  assert.throws(() => signAuth(junk, "ws://127.0.0.1:13120", "c", 1), (err: unknown) => {
    assert.ok(err instanceof Error);
    assert.equal(err.message, "unreadable nsec");
    assert.equal(err.message.includes(junk), false);
    return true;
  });
});
