/** Roving focus across the Stars columns. Extends the planet-column
 *  pattern. Nothing here is written to disk. */

export type StarsCol = "planets" | "middle" | "trackers";
export type StarsLayoutName = "three" | "dots" | "single";
export type StarsSegment = "checkin" | "trackers" | "journal";
export type HDir = "left" | "right";
export type VDir = "up" | "down";

const COLS: readonly StarsCol[] = ["planets", "middle", "trackers"];
const SEGS: readonly StarsSegment[] = ["checkin", "trackers", "journal"];

export interface ColumnNextOpts {
  embedded?: boolean;
  /** FOCUS F4 outward edge. Left-edge rail → "left". */
  outward?: "left" | "right" | "up" | "down";
}

/** ← / → between planets, radar+folio, and trackers.
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
