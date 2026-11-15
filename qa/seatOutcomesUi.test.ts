// Door words on agent walkers. Flag off stays the base entry and the base sig.
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { registerHooks } from "node:module";
import { dirname, extname, join, resolve } from "node:path";
import { mock, test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (
      (specifier.startsWith("./") || specifier.startsWith("../")) &&
      extname(specifier) === "" &&
      context.parentURL
    ) {
      const base = resolve(dirname(fileURLToPath(context.parentURL)), specifier);
      if (existsSync(`${base}.ts`)) return nextResolve(`${specifier}.ts`, context);
      if (existsSync(`${base}.svelte.ts`)) return nextResolve(`${specifier}.svelte.ts`, context);
    }
    return nextResolve(specifier, context);
  },
});

function src(rel) {
  return readFileSync(join(root, rel), "utf8");
}

function walk(rel) {
  const out = [];
  for (const name of readdirSync(join(root, rel))) {
    const child = join(rel, name);
    if (statSync(join(root, child)).isDirectory()) out.push(...walk(child));
    else out.push(child);
  }
  return out;
}

const NAME_RE = /herdr|goose|hermes|grok|berd|pi-grok|cook-pi/gi;

function nameHits(text) {
  return [...text.matchAll(new RegExp(NAME_RE.source, "gi"))].length;
}

function door() {
  return import("../src/lib/rail/seatOutcomes.ts");
}

function claim() {
  return import("../src/lib/rail/pageClaim.svelte.ts");
}

const project = { id: "p1", name: "Kansas" };

function card(id, title, planetId) {
  return {
    id,
    title: title ?? "Seat",
    projectId: planetId ?? "p1",
    status: "doing",
    createdAt: 1,
    lastTouchedAt: 1,
    goalAligned: false,
    clinicalOrMaster: false,
    pointsAwarded: 0,
  };
}

const andy = { id: "a1", name: "Andy", harness: "cursor", profile: "p", mdPaths: [] };

const BASE_ENTRIES =
  '[{"id":"agent:c1","appName":"Andy","title":"Seat","bundleId":null,"minimized":false,"iconBase64":null,"age":"fresh","asleep":false,"spritePackId":"","charSeed":0,"charLook":null,"spriteScale":0,"cardStatus":"doing","cardKind":"todo","neededState":"blocked","agentSeed":"p1:c1"}]';

const BASE_SIG =
  '[[["p1",false,[],[],[["agent:c1",false,false,"fresh",false,"",0,null,0,"doing","todo","blocked","p1:c1"]]]],["pk"],1,false,false,false]';

const BASE_SNAPSHOT =
  '{"planets":[{"projectId":"p1","name":"Kansas","collapsed":false,"boardCards":[],"todoTrees":[],"entries":[{"id":"agent:c1","appName":"Andy","title":"Seat","bundleId":null,"minimized":false,"iconBase64":null,"age":"fresh","asleep":false,"spritePackId":"","charSeed":0,"charLook":null,"spriteScale":0,"cardStatus":"doing","cardKind":"todo","neededState":"blocked","agentSeed":"p1:c1"}]}],"spriteScale":1,"marbleDrift":false,"allPlanets":[{"projectId":"p1","name":"Kansas"}],"treeGrowDays":14}';

function seatedRun() {
  return { cardId: "c1", planetId: "p1", agentShortcutId: "a1", state: "blocked" };
}

function entriesScope(mod, opts) {
  return {
    agentRuns: opts.runs,
    kanbanCards: opts.cards,
    runIsSeated: (r) => mod.runIsSeated({}, r),
    agentShortcuts: opts.agents,
    goNeededOnPlanet: (c, planetId) =>
      mod.goNeededOnPlanet(
        {
          goToken: (title) => mod.goToken({}, title),
          kanbanCards: opts.cards,
        },
        c,
        planetId,
      ),
    seatStates: opts.seatStates,
  };
}

async function syncOnce(mod, seatStates) {
  let snapshot = "";
  const cards = [card("c1")];
  const runs = [seatedRun()];
  const scope = {
    stateLoaded: true,
    status: { trusted: true, bundlePath: "", signingLine: "", inBundle: false, isAdhoc: false },
    projects: [project],
    liveClaims: [],
    occupancyOf: () => "p1",
    ageClass: () => "fresh",
    windows: [],
    sleeperRowFor: () => {
      throw new Error("sleeper");
    },
    habitatPlanetFor: (p, rows) =>
      mod.habitatPlanetFor(
        {
          isCollapsed: () => false,
          boardCardsFor: () => [],
          todoTrees: [],
          kanbanCards: cards,
          habitatEntryFor: () => {
            throw new Error("window");
          },
          agentEntriesFor: (proj) =>
            mod.agentEntriesFor(
              entriesScope(mod, { runs, cards, agents: [andy], seatStates }),
              proj,
            ),
        },
        p,
        rows,
      ),
    packs: [{ id: "pk", name: "", license: null, warnLicense: false, frameCount: 1, path: "", role: "" }],
    spriteScale: 1,
    marbleDrift: false,
    starterStory: { v: 0, skipped: false, coachDone: false, commsBuilt: false, starsBuilt: false },
    lastHabitatSig: "",
    invoke: async (_cmd, args) => {
      snapshot = JSON.stringify(args.snapshot);
    },
    habitatSnapshotPayload: (planets) =>
      mod.habitatSnapshotPayload(
        {
          spriteScale: 1,
          marbleDrift: false,
          projects: [project],
          treeGrowDays: 14,
          habitatPlanetFor: scope.habitatPlanetFor,
        },
        planets,
      ),
  };
  await mod.syncHabitat(scope);
  return { sig: scope.lastHabitatSig, snapshot };
}

async function flush() {
  for (let i = 0; i < 8; i++) await Promise.resolve();
}

test("door_parse_off_is_empty", async () => {
  const { parseDoor, stateFor } = await door();
  for (const v of [null, "nope", { enabled: false }, { enabled: true }]) {
    assert.equal(parseDoor(v).size, 0);
  }
  const parsed = parseDoor({
    enabled: true,
    source: "herdr",
    seats: [{ seat: "agent:c1", state: "waiting_on_you", line: "hi", since: 1 }],
  });
  assert.equal(parsed.get("agent:c1"), "waiting_on_you");
  assert.equal(stateFor(parsed, "c1"), "waiting_on_you");
  assert.equal(stateFor(undefined, "c1"), "");
  assert.equal(stateFor(new Map(), "c1"), "");
});

test("agent_entries_flag_off_byte_equal", async () => {
  const mod = await claim();
  const runs = [seatedRun()];
  const cards = [card("c1")];
  const none = mod.agentEntriesFor(
    entriesScope(mod, { runs, cards, agents: [andy] }),
    project,
  );
  const empty = mod.agentEntriesFor(
    entriesScope(mod, { runs, cards, agents: [andy], seatStates: new Map() }),
    project,
  );
  assert.equal(JSON.stringify(none), BASE_ENTRIES);
  assert.equal(JSON.stringify(empty), BASE_ENTRIES);
});

test("agent_entries_carry_door_state", async () => {
  const mod = await claim();
  const runs = [seatedRun()];
  const cards = [card("c1")];
  const off = mod.agentEntriesFor(
    entriesScope(mod, { runs, cards, agents: [andy] }),
    project,
  )[0];
  const on = mod.agentEntriesFor(
    entriesScope(mod, {
      runs,
      cards,
      agents: [andy],
      seatStates: new Map([["agent:c1", "waiting_on_you"]]),
    }),
    project,
  )[0];
  assert.equal(on.outcome, "waiting_on_you");
  const rest = { ...on };
  delete rest.outcome;
  assert.deepEqual(rest, off);
});

test("agent_entries_membership_unchanged", async () => {
  const mod = await claim();
  const runs = [
    seatedRun(),
    { cardId: "c2", planetId: "p1", state: "working" },
    { cardId: "c3", planetId: "p1", state: "done" },
    { cardId: "c4", planetId: "p2", agentShortcutId: "a1", state: "working" },
  ];
  const cards = [card("c1"), card("c2", "Other"), card("c3", "Finished"), card("c4", "Away", "p2")];
  const ids = (seatStates) =>
    mod
      .agentEntriesFor(entriesScope(mod, { runs, cards, agents: [andy], seatStates }), project)
      .map((e) => e.id);
  const off = ids(undefined);
  const on = ids(
    new Map([
      ["agent:c2", "failed"],
      ["agent:c9", "working"],
    ]),
  );
  assert.deepEqual(off, ["agent:c1", "agent:c3"]);
  assert.deepEqual(on, off);
});

test("sig_flag_off_byte_equal", async () => {
  const file = src("src/lib/rail/pageClaim.svelte.ts");
  const at = file.indexOf("const sig = JSON.stringify");
  assert.ok(at >= 0);
  const end = file.indexOf("if (sig === scope.lastHabitatSig)", at);
  assert.ok(end > at);
  const sigSrc = file.slice(at, end);
  assert.match(sigSrc, /\.\.\.\(e\.outcome \? \[e\.outcome\] : \[\]\)/);
  assert.equal(sigSrc.split("e.outcome").length - 1, 2);
  const mod = await claim();
  const bare = await syncOnce(mod, undefined);
  const empty = await syncOnce(mod, new Map());
  assert.equal(bare.sig, BASE_SIG);
  assert.equal(empty.sig, BASE_SIG);
  assert.equal(bare.snapshot, BASE_SNAPSHOT);
  assert.equal(empty.snapshot, BASE_SNAPSHOT);
});

test("poll_one_in_flight_and_stops", async () => {
  const { startSeatPoll, sameStates } = await door();
  assert.equal(
    sameStates(new Map([["agent:c1", "working"]]), new Map([["agent:c1", "working"]])),
    true,
  );
  assert.equal(sameStates(new Map(), new Map([["agent:c1", "idle"]])), false);
  mock.timers.enable({ apis: ["setInterval"] });
  try {
    let has = true;
    let pending = null;
    const calls = [];
    const seen = [];
    const stop = startSeatPoll({
      invoke: () => {
        calls.push(1);
        return new Promise((resolve, reject) => {
          pending = { resolve, reject };
        });
      },
      hasSeats: () => has,
      onChange: (s) => seen.push(s.get("agent:c1") ?? ""),
      everyMs: 1000,
    });
    mock.timers.tick(4999);
    assert.equal(calls.length, 0);
    mock.timers.tick(1);
    assert.equal(calls.length, 1);
    mock.timers.tick(5000);
    assert.equal(calls.length, 1);
    pending.resolve({
      enabled: true,
      seats: [{ seat: "agent:c1", state: "working" }],
    });
    await flush();
    assert.deepEqual(seen, ["working"]);
    mock.timers.tick(5000);
    assert.equal(calls.length, 2);
    pending.resolve({
      enabled: true,
      seats: [{ seat: "agent:c1", state: "working" }],
    });
    await flush();
    assert.deepEqual(seen, ["working"]);
    has = false;
    mock.timers.tick(5000);
    assert.equal(calls.length, 2);
    has = true;
    mock.timers.tick(5000);
    assert.equal(calls.length, 3);
    pending.reject(new Error("down"));
    await flush();
    mock.timers.tick(5000);
    assert.equal(calls.length, 4);
    pending.resolve({ enabled: false });
    await flush();
    stop();
    const n = calls.length;
    mock.timers.tick(20000);
    assert.equal(calls.length, n);
  } finally {
    mock.timers.reset();
  }
});

test("rail_reads_door_only_through_seatOutcomes_ts", () => {
  const files = [...walk("src/lib/rail"), "src/lib/RailPage.svelte"];
  const namers = [];
  for (const rel of files) {
    const text = src(rel);
    assert.equal(text.includes("herdr_peek"), false, rel);
    assert.equal(text.includes("herdr_status"), false, rel);
    assert.equal(text.includes("herdr-peek"), false, rel);
    if (text.includes("seat_outcomes")) namers.push(rel);
  }
  assert.deepEqual(namers, ["src/lib/rail/seatOutcomes.ts"]);
});

test("peek_listeners_keep_targets", () => {
  const capsule = src("src/lib/habitat/PeekCapsule.svelte");
  const preview = src("src/lib/habitat/PeekPreview.svelte");
  const capAt = capsule.indexOf('listen<Capsule>("herdr-peek"');
  assert.ok(capAt >= 0);
  const capListen = capsule.slice(capAt, capsule.indexOf(".then", capAt));
  assert.match(capListen, /target:\s*"peek-capsule"/);
  const prevAt = preview.indexOf('listen<Capsule>("herdr-peek"');
  assert.ok(prevAt >= 0);
  const prevListen = preview.slice(prevAt, preview.indexOf(".then", prevAt));
  assert.match(prevListen, /target:\s*"peek-preview"/);
});

test("persist_never_writes_outcome", () => {
  const page = src("src/lib/RailPage.svelte");
  const at = page.indexOf("return persistBody({");
  assert.ok(at >= 0);
  const end = page.indexOf("});", at);
  assert.ok(end > at);
  const scope = page.slice(at, end);
  assert.equal(scope.includes("outcome"), false);
  assert.equal(scope.includes("seatStates"), false);
  const rows = src("src/lib/rail/pageRows.svelte.ts");
  assert.equal(rows.includes("outcome"), false);
  assert.equal(rows.includes("seatStates"), false);
});

test("guard_scan_ui_names", () => {
  const text = src("src-tauri/src/agents/outcome/ALLOWLIST.txt");
  const allowed = new Map();
  const lines = text.split("\n").filter((line) => line.length > 0);
  assert.ok(lines.length > 0);
  for (const line of lines) {
    const parts = line.split("\t");
    assert.equal(parts.length, 3, line);
    const [path, countRaw, reason] = parts;
    assert.match(countRaw, /^\d+$/, path);
    assert.ok(reason.trim().length > 0, path);
    assert.equal(allowed.has(path), false, path);
    allowed.set(path, Number(countRaw));
  }
  const files = [...walk("src-tauri/src/habitat"), ...walk("src/lib/rail"), "src/lib/RailPage.svelte"];
  const seen = new Set(files);
  for (const path of allowed.keys()) assert.equal(seen.has(path), true, path);
  for (const rel of files) {
    const n = nameHits(src(rel));
    if (!allowed.has(rel)) {
      assert.equal(n, 0, rel);
      continue;
    }
    assert.ok(n <= allowed.get(rel), `${rel} ${n} > ${allowed.get(rel)}`);
  }
});
