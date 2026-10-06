/** Pure half-wheel geometry for the stage rail. No Svelte. */

export const TILE_W = 180;
export const TILE_H = 60;
export const SELECTED_SCALE = 1.5;

/** Wheel column thickness (LEFT/RIGHT) and row thickness (TOP/BOTTOM). */
export const WHEEL_COL = 320;
export const WHEEL_ROW = 250;
/** Pinned cluster block above the vertical wheel. */
export const PIN_H = 132;
export const PREVIEW_MARGIN = 32;

const STEP_DEG = 18;
const RADIUS = 280;

export type StageEdge = "left" | "right" | "top" | "bottom";

export interface WheelSlot {
  k: number;
  x: number;
  y: number;
  rotate: number;
  scale: number;
  opacity: number;
  z: number;
  hidden: boolean;
}

export interface ZoneRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface WheelLayout {
  wheel: ZoneRect;
  pinned: ZoneRect;
  preview: ZoneRect;
  selected: ZoneRect;
}

export interface PinnedLook {
  scale: number;
  opacity: number;
  rotate: number;
}

export interface WheelMotion {
  durationMs: number;
  swingDeg: number;
}

const FALLOFF: { scale: number; opacity: number; z: number }[] = [
  { scale: SELECTED_SCALE, opacity: 1, z: 40 },
  { scale: 0.92, opacity: 0.85, z: 30 },
  { scale: 0.8, opacity: 0.6, z: 20 },
  { scale: 0.7, opacity: 0.35, z: 10 },
];

export function normalizeStageEdge(raw: unknown): StageEdge {
  return raw === "right" || raw === "top" || raw === "bottom" ? raw : "left";
}

function falloff(k: number): { scale: number; opacity: number; z: number; hidden: boolean } {
  const a = Math.abs(k);
  if (a > 3) return { scale: 0.7, opacity: 0, z: 0, hidden: true };
  return { ...FALLOFF[a], hidden: false };
}

/** Arc slot. k = 0 is the centre. LEFT x is the bulge toward screen centre. */
export function wheelSlot(k: number, edge: StageEdge | string = "left"): WheelSlot {
  const e = normalizeStageEdge(edge);
  const theta = k * STEP_DEG * (Math.PI / 180);
  const bulge = RADIUS * Math.cos(theta);
  const along = RADIUS * Math.sin(theta);
  const rot = k * STEP_DEG * 0.7;
  const vis = falloff(k);
  let x = bulge;
  let y = along;
  let rotate = rot;
  if (e === "right") {
    x = -bulge;
    rotate = -rot;
  } else if (e === "top") {
    x = along;
    y = bulge;
  } else if (e === "bottom") {
    x = along;
    y = -bulge;
    rotate = -rot;
  }
  return {
    k,
    x,
    y,
    rotate,
    scale: vis.scale,
    opacity: vis.opacity,
    z: vis.z,
    hidden: vis.hidden,
  };
}

function pidFromStop(id: string | null): string | null {
  if (!id) return null;
  for (const prefix of ["rhead-", "rraise-", "rtodos-"]) {
    if (id.startsWith(prefix) && id.length > prefix.length) return id.slice(prefix.length);
  }
  return null;
}

/** Open planet wins; else the planet that owns the highlighted stop; else the previous centre. */
export function wheelCenter(
  ids: readonly string[],
  openId: string | null,
  activeStopId: string | null,
  prevCenter: number,
): number {
  if (ids.length === 0) return 0;
  if (openId && ids.includes(openId)) return ids.indexOf(openId);
  const pid = pidFromStop(activeStopId);
  if (pid && ids.includes(pid)) return ids.indexOf(pid);
  const c = Number.isFinite(prevCenter) ? Math.round(prevCenter) : 0;
  if (c < 0) return 0;
  if (c >= ids.length) return ids.length - 1;
  return c;
}

/** Input order, never sorted. Along-track y is monotone in that order. */
export function orderedSlots(
  ids: readonly string[],
  center: number,
  edge: StageEdge | string = "left",
): { id: string; y: number }[] {
  const c = wheelCenter(ids, null, null, center);
  return ids.map((id, i) => ({ id, y: wheelSlot(i - c, edge).y }));
}

/** Stars, Settings, chips, and the banner pill. Never a wheel slot. */
export function pinnedLook(_edge: StageEdge | string = "left"): PinnedLook {
  return { scale: 1, opacity: 1, rotate: 0 };
}

export function wheelMotion(reduce: boolean): WheelMotion {
  if (reduce) return { durationMs: 80, swingDeg: 0 };
  return { durationMs: 240, swingDeg: -24 };
}

function rect(x: number, y: number, w: number, h: number): ZoneRect {
  return { x, y, w: Math.max(0, w), h: Math.max(0, h) };
}

function selectedIn(wheel: ZoneRect): ZoneRect {
  const w = TILE_W * SELECTED_SCALE;
  const h = TILE_H * SELECTED_SCALE;
  return rect(wheel.x + (wheel.w - w) / 2, wheel.y + (wheel.h - h) / 2, w, h);
}

/** Wheel column, pinned cluster, and the preview (the rest of the frame). */
export function wheelLayout(edge: string, frameW: number, frameH: number): WheelLayout {
  const e = normalizeStageEdge(edge);
  const m = PREVIEW_MARGIN;
  if (e === "top") {
    const wheel = rect(0, 0, frameW, WHEEL_ROW);
    const pinned = rect(8, 8, 200, WHEEL_ROW - 16);
    const preview = rect(m, WHEEL_ROW + m, frameW - 2 * m, frameH - WHEEL_ROW - 2 * m);
    return { wheel, pinned, preview, selected: selectedIn(wheel) };
  }
  if (e === "bottom") {
    const wheel = rect(0, frameH - WHEEL_ROW, frameW, WHEEL_ROW);
    const pinned = rect(8, frameH - WHEEL_ROW + 8, 200, WHEEL_ROW - 16);
    const preview = rect(m, m, frameW - 2 * m, frameH - WHEEL_ROW - 2 * m);
    return { wheel, pinned, preview, selected: selectedIn(wheel) };
  }
  const onRight = e === "right";
  const wheel = rect(onRight ? frameW - WHEEL_COL : 0, PIN_H, WHEEL_COL, frameH - PIN_H);
  const pinned = rect(wheel.x + 8, 8, WHEEL_COL - 16, PIN_H - 16);
  const preview = onRight
    ? rect(m, m, frameW - WHEEL_COL - 2 * m, frameH - 2 * m)
    : rect(WHEEL_COL + m, m, frameW - WHEEL_COL - 2 * m, frameH - 2 * m);
  return { wheel, pinned, preview, selected: selectedIn(wheel) };
}

function inside(point: { x: number; y: number }, zone: ZoneRect): boolean {
  return (
    point.x >= zone.x &&
    point.x < zone.x + zone.w &&
    point.y >= zone.y &&
    point.y < zone.y + zone.h
  );
}

/** True only over the band/wheel, the pinned cluster, or the open preview. */
export function paintedZoneHit(
  point: { x: number; y: number },
  layout: WheelLayout,
  open: boolean,
): boolean {
  if (inside(point, layout.wheel) || inside(point, layout.pinned)) return true;
  if (open && inside(point, layout.preview)) return true;
  return false;
}
