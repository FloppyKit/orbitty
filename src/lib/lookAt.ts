// GO-ORBITTY-LOOK-AT — Kamran look-at contract on the Spirit flipbook.
// Pointer angle picks a cell. Dead zone when the pointer is close.
// Hysteresis on the sector line. Reaction overlay is a separate ~0.5s play.
// Window-local only. Reduced motion / no fine pointer → front idle.
// Gaze math lives here — not on CompanionPage.

export const LOOK_DIRS = ["e", "ne", "n", "nw", "w", "sw", "s", "se"] as const;
export type LookDir = "idle" | (typeof LOOK_DIRS)[number];

export const OVERLAY_KINDS = [
  "blink",
  "poke",
  "sleepy",
  "sparkle",
  "delighted",
  "dizzy",
] as const;
export type OverlayKind = (typeof OVERLAY_KINDS)[number];

export const OVERLAY_MS = 500;
export const BLINK_EVERY_MS = 4200;
export const SLEEPY_EVERY_MS = 8000;
export const POKE_WINDOW_MS = 1400;
export const POKE_DIZZY = 4;
export const POKE_DRAG_PX = 12;
export const DEAD_PX = 56;
export const DEAD_HYST_PX = 20;
export const HALF_SECTOR_DEG = 22.5;
export const HYST_DEG = 10;

export type CueSnap = {
  ready: boolean;
  ids: string[];
  doneIds: string[];
};
export const EMPTY_CUE_SNAP: CueSnap = { ready: false, ids: [], doneIds: [] };

export type CueCard = {
  id: string;
  status: string;
  kind?: string | null;
  dueAt?: number | null;
};

/** fine pointer + motion allowed. */
export function lookEnabled(finePointer: boolean, reducedMotion: boolean): boolean {
  return finePointer && !reducedMotion;
}

/** Screen delta → degrees, 0 = east, 90 = north (y-down inverted). */
export function vectorDeg(dx: number, dy: number): number {
  let a = (Math.atan2(-dy, dx) * 180) / Math.PI;
  if (a < 0) a += 360;
  return a;
}

export function sectorIndex(deg: number): number {
  return Math.round(deg / 45) % 8;
}

export function dirIndex(dir: LookDir): number {
  if (dir === "idle") return -1;
  return LOOK_DIRS.indexOf(dir);
}

function angleDeltaDeg(a: number, b: number): number {
  let d = a - b;
  while (d > 180) d -= 360;
  while (d < -180) d += 360;
  return d;
}

export function pickDir(
  dx: number,
  dy: number,
  current: LookDir,
  inDead: boolean,
): { dir: LookDir; inDead: boolean } {
  const dist = Math.hypot(dx, dy);
  if (inDead) {
    if (dist < DEAD_PX + DEAD_HYST_PX) return { dir: "idle", inDead: true };
  } else if (dist < DEAD_PX) {
    return { dir: "idle", inDead: true };
  }
  const next = LOOK_DIRS[sectorIndex(vectorDeg(dx, dy))];
  if (current === "idle" || current === next) {
    return { dir: next, inDead: false };
  }
  const curDeg = dirIndex(current) * 45;
  if (Math.abs(angleDeltaDeg(vectorDeg(dx, dy), curDeg)) <= HALF_SECTOR_DEG + HYST_DEG) {
    return { dir: current, inDead: false };
  }
  return { dir: next, inDead: false };
}

export function recordPoke(
  times: number[],
  now: number,
): { times: number[]; kind: OverlayKind } {
  const kept = times.filter((t) => now - t <= POKE_WINDOW_MS);
  kept.push(now);
  if (kept.length >= POKE_DIZZY) {
    return { times: [], kind: "dizzy" };
  }
  return { times: kept, kind: "poke" };
}

export function overlayMap(
  frames: string[],
): Partial<Record<OverlayKind, string>> {
  if (frames.length !== OVERLAY_KINDS.length) return {};
  const out: Partial<Record<OverlayKind, string>> = {};
  OVERLAY_KINDS.forEach((k, i) => {
    out[k] = frames[i];
  });
  return out;
}

function isOverdue(c: CueCard, nowSec: number): boolean {
  if (c.status === "done" || c.dueAt == null) return false;
  return c.dueAt < nowSec;
}

export function cueFromCards(
  cards: CueCard[],
  snap: CueSnap,
  nowSec: number,
): { cue: OverlayKind | null; snap: CueSnap; overdue: boolean } {
  const ids = cards.map((c) => c.id);
  const doneIds = cards.filter((c) => c.status === "done").map((c) => c.id);
  const overdue = cards.some((c) => isOverdue(c, nowSec));
  const next: CueSnap = { ready: true, ids, doneIds };
  if (!snap.ready) {
    return { cue: null, snap: next, overdue };
  }
  for (const id of doneIds) {
    if (!snap.doneIds.includes(id)) {
      return { cue: "delighted", snap: next, overdue };
    }
  }
  for (const c of cards) {
    if (!snap.ids.includes(c.id) && c.status !== "done") {
      return { cue: "sparkle", snap: next, overdue };
    }
  }
  return { cue: null, snap: next, overdue };
}
