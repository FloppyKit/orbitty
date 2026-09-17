import {
  DEAD_HYST_PX,
  DEAD_PX,
  HALF_SECTOR_DEG,
  HYST_DEG,
  LOOK_DIRS,
  cueFromCards,
  lookEnabled,
  overlayMap,
  pickDir,
  recordPoke,
  sectorIndex,
  vectorDeg,
} from "../src/lib/lookAt.ts";

function assert(cond: boolean, msg: string) {
  if (!cond) throw new Error(msg);
}

assert(lookEnabled(true, false), "fine pointer, motion on");
assert(!lookEnabled(false, false), "no fine pointer → idle");
assert(!lookEnabled(true, true), "reduced motion off");

assert(sectorIndex(0) === 0, "0° east");
assert(sectorIndex(45) === 1, "45° NE");
assert(sectorIndex(90) === 2, "90° north");
assert(sectorIndex(180) === 4, "180° west");
assert(sectorIndex(270) === 6, "270° south");
assert(sectorIndex(359) === 0, "359° wraps east");
assert(vectorDeg(10, 0) === 0, "right is east");
assert(Math.abs(vectorDeg(0, -10) - 90) < 1e-6, "up is north (y-down screen)");

let cur = pickDir(200, 0, "idle", true);
assert(cur.dir === "e" && !cur.inDead, `far east → e, got ${cur.dir}`);

cur = pickDir(10, 0, "e", false);
assert(cur.dir === "idle" && cur.inDead, "inside dead zone → idle");

cur = pickDir(DEAD_PX + 4, 0, "idle", true);
assert(cur.dir === "idle" && cur.inDead, "still dead until leave radius");

cur = pickDir(DEAD_PX + DEAD_HYST_PX + 4, 0, "idle", true);
assert(cur.dir === "e" && !cur.inDead, "leave dead zone → e");

const line = HALF_SECTOR_DEG + 1;
const rad = 200;
const x = rad * Math.cos((line * Math.PI) / 180);
const y = -rad * Math.sin((line * Math.PI) / 180);
cur = pickDir(x, y, "e", false);
assert(cur.dir === "e", `hysteresis holds E at ${line}°, got ${cur.dir}`);

const past = HALF_SECTOR_DEG + HYST_DEG + 4;
const x2 = rad * Math.cos((past * Math.PI) / 180);
const y2 = -rad * Math.sin((past * Math.PI) / 180);
cur = pickDir(x2, y2, "e", false);
assert(cur.dir === "ne", `cross hysteresis → NE, got ${cur.dir}`);

const poke1 = recordPoke([], 1000);
assert(poke1.kind === "poke" && poke1.times.length === 1, "first poke");
const poke2 = recordPoke(poke1.times, 1100);
const poke3 = recordPoke(poke2.times, 1200);
const poke4 = recordPoke(poke3.times, 1300);
assert(poke4.kind === "dizzy" && poke4.times.length === 0, "fourth poke dizzy");

const first = cueFromCards(
  [{ id: "a", status: "todo", kind: "todo" }],
  { ready: false, ids: [], doneIds: [] },
  10,
);
assert(first.cue === null && first.snap.ready, "first board load is not a ping");
const ping = cueFromCards(
  [
    { id: "a", status: "todo", kind: "todo" },
    { id: "b", status: "todo", kind: "todo" },
  ],
  first.snap,
  11,
);
assert(ping.cue === "sparkle", "new todo is task ping");
const done = cueFromCards(
  [
    { id: "a", status: "done", kind: "todo" },
    { id: "b", status: "todo", kind: "todo" },
  ],
  ping.snap,
  12,
);
assert(done.cue === "delighted", "done ping");
const overdue = cueFromCards(
  [{ id: "c", status: "todo", kind: "todo", dueAt: 1 }],
  { ready: true, ids: ["c"], doneIds: [] },
  99,
);
assert(overdue.overdue && overdue.cue === null, "overdue is sleepy flag, not a one-shot");

const map = overlayMap(["a", "b", "c", "d", "e", "f"]);
assert(map.blink === "a" && map.dizzy === "f", "overlay order");
assert(Object.keys(overlayMap(["short"])).length === 0, "short overlay set is empty");
assert(LOOK_DIRS.length === 8, "8-dir");

console.log("lookAt ok");
