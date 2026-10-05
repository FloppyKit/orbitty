// Pure planet-column decisions. No Svelte, so node:test can call these.
// Collapse is session state in the page; this module only decides.

/** Open column: chip + name + count. The preview clamp subtracts this. */
export const COLUMN_OPEN_PX = 168;

/** Chips only: marble (or pin mark) + count. Narrower than the open column. */
export const COLUMN_ICONS_PX = 72;

/**
 * Board width in px under which `pref: "auto"` collapses to chips.
 * An explicit "open" or "icons" pref wins over this number.
 * "Under" means strictly below: 640 stays open.
 */
export const COLUMN_AUTO_MIN_PX = 640;

export type ColumnArrowDir = "up" | "down" | "left" | "right";
export type ColumnPref = "auto" | "open" | "icons";
export type ColumnMode = "open" | "icons";

export function columnArrow(
  ids: string[],
  activeId: string,
  dir: ColumnArrowDir,
  ctx: { firstCardId: string | null; firstNeededId: string | null },
): string | null {
  if (dir === "left") return null;
  if (dir === "right") return ctx.firstCardId ?? ctx.firstNeededId;
  const n = ids.length;
  if (n === 0) return null;
  const i = ids.indexOf(activeId);
  if (i < 0) return null;
  const step = dir === "down" ? 1 : -1;
  return ids[(i + step + n) % n];
}

/** First row whose midpoint is below the pointer. Null means drop at the end. */
export function beforeIdAt(
  rows: { id: string; top: number; height: number }[],
  clientY: number,
): string | null {
  for (const row of rows) {
    if (clientY < row.top + row.height / 2) return row.id;
  }
  return null;
}

export function columnMode(
  availPx: number,
  pref: ColumnPref,
  minOpenPx: number = COLUMN_AUTO_MIN_PX,
): ColumnMode {
  if (pref === "open" || pref === "icons") return pref;
  return availPx < minOpenPx ? "icons" : "open";
}

export function columnPx(mode: ColumnMode): number {
  return mode === "icons" ? COLUMN_ICONS_PX : COLUMN_OPEN_PX;
}
