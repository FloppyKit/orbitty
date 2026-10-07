/** Roving focus across the Stars columns. Extends the planet-column
 *  pattern. Nothing here is written to disk.
 *  The right-hand column id stays "trackers" so RAIL-INPUT columnNext
 *  keeps its contract; that column now holds the journal (Z5–Z7).
 *  Trackers live in the middle column (Z4). */

export type StarsCol = "planets" | "middle" | "trackers";
export type StarsLayoutName = "three" | "dots" | "single";
export type StarsSegment = "checkin" | "trackers" | "journal";
export type HDir = "left" | "right";
export type VDir = "up" | "down";
export type StarsZone = "Z0" | "Z1" | "Z2" | "Z3" | "Z4" | "Z5" | "Z6" | "Z7";
export type FocusKey =
  | "ArrowLeft"
  | "ArrowRight"
  | "ArrowUp"
  | "ArrowDown"
  | "Enter"
  | "Escape"
  | "Home"
  | "End";

const COLS: readonly StarsCol[] = ["planets", "middle", "trackers"];
const SEGS: readonly StarsSegment[] = ["checkin", "trackers", "journal"];

export const ZONE_LIST: readonly StarsZone[] = ["Z0", "Z1", "Z2", "Z3", "Z4", "Z5", "Z6", "Z7"];

/** Debounce for the first real check-in edit. Mount never saves. */
export const AUTOSAVE_MS = 600;

/** True only after a user edit. The untouched default draft is not due. */
export function autosaveDue(draftDirty: boolean): boolean {
  return draftDirty;
}

export interface ColumnNextOpts {
  embedded?: boolean;
  /** FOCUS F4 outward edge. Left-edge rail → "left". */
  outward?: "left" | "right" | "up" | "down";
}

/** ← / → between planets, radar+trackers, and the journal column.
 *  Single layout does not move columns (the segmented control does).
 *  Embedded: the outward arrow past the outer column calls onRingExit. */
export function columnNext(
  layout: StarsLayoutName,
  col: StarsCol,
  dir: HDir,
  opts: ColumnNextOpts = {},
): { col: StarsCol; exit: boolean } {
  if (layout === "single") return { col, exit: false };
  const i = COLS.indexOf(col);
  const at = i < 0 ? 0 : i;
  const delta = dir === "right" ? 1 : -1;
  const n = at + delta;
  if (n >= 0 && n < COLS.length) return { col: COLS[n], exit: false };
  const outward = opts.outward ?? "left";
  const onOuter =
    (dir === "left" && col === "planets") || (dir === "right" && col === "trackers");
  if (opts.embedded && onOuter && dir === outward) return { col, exit: true };
  return { col, exit: false };
}

/** ↑ / ↓ inside a column. Wraps, same as the planet-column roving tabindex. */
export function withinNext(col: StarsCol, idx: number, dir: VDir, n: number): number {
  void col;
  if (n <= 0) return 0;
  const step = dir === "down" ? 1 : -1;
  const i = ((idx % n) + n) % n;
  return (i + step + n) % n;
}

/** Single layout: ← / → move the segmented control and stop at the ends. */
export function segmentNext(current: StarsSegment, dir: HDir): StarsSegment {
  const i = SEGS.indexOf(current);
  const at = i < 0 ? 0 : i;
  const n = dir === "right" ? Math.min(SEGS.length - 1, at + 1) : Math.max(0, at - 1);
  return SEGS[n];
}

export function zoneColumn(zone: StarsZone): StarsCol | "title" {
  if (zone === "Z0") return "title";
  if (zone === "Z1") return "planets";
  if (zone === "Z2" || zone === "Z3" || zone === "Z4") return "middle";
  return "trackers";
}

const SEGMENT_ZONES: Record<StarsSegment, readonly StarsZone[]> = {
  checkin: ["Z2", "Z3"],
  trackers: ["Z4"],
  journal: ["Z5", "Z6", "Z7"],
};

export interface StarsFocus {
  zone: StarsZone;
  index: number;
  depth: 0 | 1 | 2;
  ctl: number;
  memory: Partial<Record<StarsZone, number>>;
  titleFrom: StarsCol;
}

export interface FocusWorld {
  layout: StarsLayoutName;
  counts: Partial<Record<StarsZone, number>>;
  trackerCols: number;
  /** Control count for each Z4 item. 0 means Return activates the item. */
  cardControls: number[];
  /** Index of the typeable number among that card's controls, or -1. */
  cardNumberCtl: number[];
  /** How many leading Z2 items are spoke handles (Return edits). */
  spokeItems?: number;
  wordPoints?: { x: number; y: number }[];
  embedded?: boolean;
  outward?: "left" | "right" | "up" | "down";
  reflectOpen?: boolean;
  segment?: StarsSegment;
}

export interface FocusStep {
  pos: StarsFocus;
  exit: boolean;
  activate: boolean;
  rootEsc: "reflect" | "hide" | null;
  /** Horizontal move that should also call columnNext (F4). */
  column: boolean;
  segment?: StarsSegment;
}

export function focusOrigin(zone: StarsZone = "Z2"): StarsFocus {
  const col = zoneColumn(zone);
  return {
    zone,
    index: 0,
    depth: 0,
    ctl: 0,
    memory: {},
    titleFrom: col === "title" ? "middle" : col,
  };
}

function blank(pos: StarsFocus, extra: Partial<FocusStep> = {}): FocusStep {
  return { pos, exit: false, activate: false, rootEsc: null, column: false, ...extra };
}

function remember(pos: StarsFocus): Partial<Record<StarsZone, number>> {
  return { ...pos.memory, [pos.zone]: pos.index };
}

function land(pos: StarsFocus, zone: StarsZone, world: FocusWorld, titleFrom = pos.titleFrom): StarsFocus {
  const n = world.counts[zone] ?? 0;
  const max = Math.max(0, n - 1);
  const index = n === 0 ? 0 : Math.min(pos.memory[zone] ?? 0, max);
  const col = zoneColumn(zone);
  return {
    zone,
    index,
    depth: 0,
    ctl: 0,
    memory: remember(pos),
    titleFrom: zone === "Z0" ? titleFrom : col === "title" ? titleFrom : col,
  };
}

function bodyZones(col: StarsCol): StarsZone[] {
  if (col === "planets") return ["Z1"];
  if (col === "middle") return ["Z2", "Z3", "Z4"];
  return ["Z5", "Z6", "Z7"];
}

function visibleBody(col: StarsCol, world: FocusWorld): StarsZone[] {
  return bodyZones(col).filter((z) => (world.counts[z] ?? 0) > 0);
}

function stackOf(pos: StarsFocus, world: FocusWorld): StarsZone[] {
  const head: StarsZone[] = (world.counts.Z0 ?? 0) > 0 ? ["Z0"] : [];
  if (world.layout === "single") {
    const seg = world.segment ?? "checkin";
    const body = SEGMENT_ZONES[seg].filter((z) => (world.counts[z] ?? 0) > 0);
    return [...head, ...body];
  }
  const col = zoneColumn(pos.zone);
  const from = col === "title" ? pos.titleFrom : col;
  return [...head, ...visibleBody(from, world)];
}

function firstOf(col: StarsCol, world: FocusWorld): StarsZone {
  return visibleBody(col, world)[0] ?? (col === "planets" ? "Z1" : col === "middle" ? "Z2" : "Z5");
}

function gridStep(index: number, count: number, cols: number, key: FocusKey): { index: number; edge: boolean } {
  const width = Math.max(1, cols);
  const col = index % width;
  if (key === "ArrowRight") {
    if (col + 1 < width && index + 1 < count) return { index: index + 1, edge: false };
    return { index, edge: true };
  }
  if (key === "ArrowLeft") {
    if (col > 0) return { index: index - 1, edge: false };
    return { index, edge: true };
  }
  if (key === "ArrowDown") {
    const n = index + width;
    if (n < count) return { index: n, edge: false };
    return { index, edge: true };
  }
  const n = index - width;
  if (n >= 0) return { index: n, edge: false };
  return { index, edge: true };
}

function nearestWord(points: { x: number; y: number }[], index: number, key: FocusKey): number | null {
  const here = points[index];
  if (!here) return null;
  let best: number | null = null;
  let bestD = Infinity;
  for (let i = 0; i < points.length; i++) {
    if (i === index) continue;
    const dx = points[i].x - here.x;
    const dy = points[i].y - here.y;
    const ok =
      (key === "ArrowLeft" && dx < -0.5) ||
      (key === "ArrowRight" && dx > 0.5) ||
      (key === "ArrowUp" && dy < -0.5) ||
      (key === "ArrowDown" && dy > 0.5);
    if (!ok) continue;
    const primary = key === "ArrowLeft" || key === "ArrowRight" ? Math.abs(dx) : Math.abs(dy);
    const secondary = key === "ArrowLeft" || key === "ArrowRight" ? Math.abs(dy) : Math.abs(dx);
    const d = primary * 1000 + secondary;
    if (d < bestD) {
      bestD = d;
      best = i;
    }
  }
  return best;
}

function columnMove(pos: StarsFocus, key: "ArrowLeft" | "ArrowRight", world: FocusWorld): FocusStep {
  if (world.layout === "single") {
    const dir: HDir = key === "ArrowRight" ? "right" : "left";
    const segment = segmentNext(world.segment ?? "checkin", dir);
    const body = SEGMENT_ZONES[segment].filter((z) => (world.counts[z] ?? 0) > 0);
    const zone = body[0] ?? "Z2";
    return { ...blank(land(pos, zone, world)), column: false, segment };
  }
  const from = zoneColumn(pos.zone);
  if (from === "title") return blank(pos);
  const dir: HDir = key === "ArrowRight" ? "right" : "left";
  const next = columnNext(world.layout, from, dir, {
    embedded: world.embedded,
    outward: world.outward ?? "left",
  });
  if (next.exit) return { ...blank(pos), exit: true, column: true };
  if (next.col === from) return { ...blank(pos), column: true };
  return { ...blank(land(pos, firstOf(next.col, world), world)), column: true };
}

function moveVerticalZone(pos: StarsFocus, dir: VDir, world: FocusWorld): FocusStep {
  const stack = stackOf(pos, world);
  const at = stack.indexOf(pos.zone);
  if (dir === "up") {
    if (at > 0) {
      const prev = stack[at - 1] ?? "Z0";
      const from = zoneColumn(pos.zone);
      const titleFrom = from === "title" ? pos.titleFrom : from;
      return blank(land(pos, prev, world, titleFrom));
    }
    return blank(pos);
  }
  if (at >= 0 && at < stack.length - 1) {
    const next = stack[at + 1] ?? pos.zone;
    return blank(land(pos, next, world));
  }
  return blank(pos);
}

function textOwnsArrows(pos: StarsFocus): boolean {
  if (pos.zone === "Z2" && pos.depth > 0) return true;
  if (pos.zone === "Z3" && pos.depth > 0) return true;
  if (pos.zone === "Z4" && pos.depth === 2) return true;
  return false;
}

export function focusStep(pos: StarsFocus, key: FocusKey, world: FocusWorld): FocusStep {
  if (key === "Home" || key === "End") {
    if (pos.zone !== "Z1" || pos.depth > 0) return blank(pos);
    const n = world.counts.Z1 ?? 0;
    if (n <= 0) return blank(pos);
    const index = key === "Home" ? 0 : n - 1;
    return blank({ ...pos, index, memory: { ...pos.memory, Z1: index } });
  }

  if (key === "Escape") {
    if (pos.depth > 0) {
      const depth = (pos.depth - 1) as 0 | 1;
      return blank({ ...pos, depth, ctl: depth === 0 ? 0 : pos.ctl });
    }
    if (pos.zone === "Z0" || pos.zone === "Z1") {
      return blank(land(pos, "Z2", world, "middle"));
    }
    if (world.reflectOpen) return { ...blank(pos), rootEsc: "reflect" };
    return { ...blank(pos), rootEsc: "hide" };
  }

  if (key === "Enter") {
    if (pos.zone === "Z2" && pos.depth === 0 && pos.index < (world.spokeItems ?? 0)) {
      return blank({ ...pos, depth: 1 });
    }
    if (pos.zone === "Z2" && pos.depth > 0) return blank({ ...pos, depth: 0 });
    if (pos.zone === "Z3" && pos.depth === 0) return blank({ ...pos, depth: 1 });
    if (pos.zone === "Z3" && pos.depth > 0) return blank({ ...pos, depth: 0 });
    if (pos.zone === "Z4" && pos.depth === 0) {
      const n = world.cardControls[pos.index] ?? 0;
      if (n <= 0) return { ...blank(pos), activate: true };
      return blank({ ...pos, depth: 1, ctl: 0 });
    }
    if (pos.zone === "Z4" && pos.depth === 1) {
      const num = world.cardNumberCtl[pos.index] ?? -1;
      if (num >= 0 && pos.ctl === num) return blank({ ...pos, depth: 2 });
      return { ...blank(pos), activate: true };
    }
    if (pos.zone === "Z4" && pos.depth === 2) return blank({ ...pos, depth: 1 });
    return { ...blank(pos), activate: true };
  }

  if (textOwnsArrows(pos)) return blank(pos);

  const n = world.counts[pos.zone] ?? 0;

  if (pos.zone === "Z4" && pos.depth === 1) {
    const ctls = world.cardControls[pos.index] ?? 0;
    if (key === "ArrowRight" && pos.ctl + 1 < ctls) return blank({ ...pos, ctl: pos.ctl + 1 });
    if (key === "ArrowLeft" && pos.ctl > 0) return blank({ ...pos, ctl: pos.ctl - 1 });
    return blank(pos);
  }

  if ((key === "ArrowLeft" || key === "ArrowRight") && world.layout === "single" && pos.depth === 0) {
    return columnMove(pos, key, world);
  }

  if (pos.zone === "Z4" && pos.depth === 0 && n > 0) {
    const step = gridStep(pos.index, n, world.trackerCols, key);
    if (!step.edge) return blank({ ...pos, index: step.index });
    if (key === "ArrowLeft" || key === "ArrowRight") return columnMove(pos, key, world);
    return moveVerticalZone(pos, key === "ArrowDown" ? "down" : "up", world);
  }

  if (pos.zone === "Z7" && n > 0) {
    const points = world.wordPoints;
    if (!points || points.length !== n) {
      if (key === "ArrowRight" && pos.index + 1 < n) return blank({ ...pos, index: pos.index + 1 });
      if (key === "ArrowLeft" && pos.index > 0) return blank({ ...pos, index: pos.index - 1 });
      if (key === "ArrowRight" || key === "ArrowLeft") return columnMove(pos, key, world);
      if ((key === "ArrowDown" && pos.index + 1 < n) || (key === "ArrowUp" && pos.index > 0)) {
        const index = key === "ArrowDown" ? pos.index + 1 : pos.index - 1;
        return blank({ ...pos, index });
      }
      return moveVerticalZone(pos, key === "ArrowDown" ? "down" : "up", world);
    }
    const hit = nearestWord(points, pos.index, key);
    if (hit != null) return blank({ ...pos, index: hit });
    if (key === "ArrowLeft" || key === "ArrowRight") return columnMove(pos, key, world);
    return moveVerticalZone(pos, key === "ArrowDown" ? "down" : "up", world);
  }

  const horizontal = pos.zone === "Z0" || pos.zone === "Z2" || pos.zone === "Z5";
  const vertical = pos.zone === "Z1" || pos.zone === "Z2" || pos.zone === "Z3" || pos.zone === "Z6";

  if ((key === "ArrowLeft" || key === "ArrowRight") && horizontal && n > 0) {
    const dir = key === "ArrowRight" ? 1 : -1;
    const next = pos.index + dir;
    if (next >= 0 && next < n) return blank({ ...pos, index: next });
    if (pos.zone === "Z0") return blank(pos);
    return columnMove(pos, key, world);
  }

  if ((key === "ArrowUp" || key === "ArrowDown") && vertical && n > 0) {
    const dir = key === "ArrowDown" ? 1 : -1;
    const next = pos.index + dir;
    if (next >= 0 && next < n) return blank({ ...pos, index: next });
    return moveVerticalZone(pos, key === "ArrowDown" ? "down" : "up", world);
  }

  if (key === "ArrowLeft" || key === "ArrowRight") {
    if (pos.zone === "Z0") return blank(pos);
    return columnMove(pos, key, world);
  }
  if (key === "ArrowUp" || key === "ArrowDown") {
    return moveVerticalZone(pos, key === "ArrowDown" ? "down" : "up", world);
  }
  return blank(pos);
}
