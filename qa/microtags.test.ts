import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  KIND_VALUES,
  NAMESPACES,
  RESERVED,
  applyBoardMicrotags,
  inbound,
  normalize,
  planetSlug,
  toTTags,
} from "../src/lib/microtags.ts";

type Row = { id: string; raw: string; tag: string | null };
type Fixture = {
  reserved: string[];
  namespaces: string[];
  kindValues: string[];
  rows: Row[];
};

const fixture = JSON.parse(readFileSync("qa/fixtures/microtags.json", "utf8")) as Fixture;

function baseTags(): [string, string][] {
  return [
    ["t", "orbitty"],
    ["d", "c-1"],
    ["project", "p-keep"],
    ["actor", "user"],
    ["trace", "tr"],
  ];
}

function tVals(tags: [string, string][]): string[] {
  return tags.filter((t) => t[0] === "t").map((t) => t[1]);
}

test("M-T1 normal forms", () => {
  assert.equal(normalize("#Bug Fix"), "bug-fix");
  assert.equal(normalize("Kind:Bug"), "kind:bug");
  assert.equal(normalize("  a__b  "), "a-b");
  assert.equal(normalize("area:Relay"), "area:relay");
  assert.equal(normalize("Nostr"), "nostr");
});

test("M-T2 refusals", () => {
  assert.equal(normalize("a:b:c"), null);
  assert.equal(normalize(""), null);
  assert.equal(normalize("a".repeat(65)), null);
  assert.equal(normalize("a".repeat(64)), "a".repeat(64));
  assert.equal(normalize("é"), null);
  assert.equal(normalize("kind:nope"), null);
  assert.equal(normalize("-bad"), null);
  assert.equal(normalize("bad-"), null);
});

test("M-T3 reserved", () => {
  for (const raw of [
    "orbitty",
    "local-only",
    "root",
    "root-revision",
    "#Orbitty",
    "ROOT",
    "Local-Only",
    "status:done",
    "Status:Open",
  ]) {
    assert.equal(normalize(raw), null, raw);
  }
  assert.equal(normalize("root-cause"), "root-cause");
  assert.equal(normalize("orbitty-extra"), "orbitty-extra");
  assert.equal(normalize("status"), "status");
});

test("M-T4 fixture matches normalize", () => {
  assert.ok(fixture.rows.length >= 30, `rows ${fixture.rows.length}`);
  assert.deepEqual(fixture.reserved, [...RESERVED]);
  assert.deepEqual(fixture.namespaces, [...NAMESPACES]);
  assert.deepEqual(fixture.kindValues, [...KIND_VALUES]);
  for (const row of fixture.rows) {
    assert.equal(normalize(row.raw), row.tag, row.id);
  }
});

test("M-T5 orbitty first, then microtags", () => {
  const tags = baseTags();
  const refused = applyBoardMicrotags(
    tags,
    true,
    ["#Bug Fix", "area:Relay", "root", "#Bug Fix", "planet:nope", "kind:bug", "local-only"],
    "p-keep",
    "Relay",
  );
  assert.deepEqual(tags[0], ["t", "orbitty"]);
  assert.deepEqual(tVals(tags), ["orbitty", "bug-fix", "area:relay", "kind:bug", "planet:p-keep"]);
  assert.deepEqual(refused, ["root", "local-only"]);
  assert.ok(tVals(tags).indexOf("orbitty") < tVals(tags).indexOf("bug-fix"));
});

test("M-T6 inbound keeps raw display", () => {
  assert.deepEqual(inbound("é"), { microtag: null, display: "é" });
  assert.deepEqual(inbound("root"), { microtag: null, display: "root" });
  assert.deepEqual(inbound("a:b:c"), { microtag: null, display: "a:b:c" });
  assert.deepEqual(inbound("#Bug Fix"), { microtag: "bug-fix", display: "#Bug Fix" });
});

test("M-T7 one planet tag, slug frozen across rename", () => {
  assert.equal(planetSlug("p-keep", "Relay"), "p-keep");
  assert.equal(planetSlug("p-keep", "Renamed Relay"), "p-keep");
  const card = ["#Bug Fix", "planet:other"];
  const before = toTTags(card, planetSlug("p-keep", "Relay") ?? undefined);
  const after = toTTags(card, planetSlug("p-keep", "Renamed Relay") ?? undefined);
  assert.deepEqual(before, after);
  const planets = before.filter((t) => t[1].startsWith("planet:")).map((t) => t[1]);
  assert.deepEqual(planets, ["planet:p-keep"]);
});

test("flag_off_board_event_byte_equal", () => {
  const tags = baseTags();
  const before = JSON.stringify({
    kind: 420,
    tags,
    content: '{"cardId":"c-1"}',
  });
  const refused = applyBoardMicrotags(
    tags,
    false,
    ["#Bug Fix", "root", "area:Relay"],
    "p-keep",
    "Renamed",
  );
  assert.deepEqual(refused, []);
  const after = JSON.stringify({
    kind: 420,
    tags,
    content: '{"cardId":"c-1"}',
  });
  assert.equal(after, before);
});
