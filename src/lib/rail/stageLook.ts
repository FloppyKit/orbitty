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

/** Pixel deltas (trackpad) emit one step per this many pixels. */
export const SPIN_STEP_PX = 56;
/** Quiet this long and the scroll residual drops so rest is a whole item. */
export const SPIN_SETTLE_MS = 150;

export type SpinKind = "stars" | "settings" | "planet" | "unmapped" | "add";

export interface SpinItem {
  stop: string;
  kind: SpinKind;
  pid?: string;
}

export type SpinDir = "up" | "down" | "left" | "right";

export interface SpinStop {
  id: string;
  group: "wheel";
}

export interface SpinAccumState {
  residual: number;
  lastAt: number;
}

export type SpinConfirmAction = "stars" | "settings" | "newProject" | "centre" | "raise";

export type SpinPane =
  | { kind: "board"; selected: string }
  | { kind: "unclaimed" };

/** Stars, Settings, planets in input order, Unmapped, + Project. Never sorted. */
export function spinItems(planetIds: readonly string[], unmappedKey: string): SpinItem[] {
  return [
    { stop: "rstars", kind: "stars" },
    { stop: "rsettings", kind: "settings" },
    ...planetIds.map((pid) => ({ stop: `rhead-${pid}`, kind: "planet" as const, pid })),
    { stop: `rhead-${unmappedKey}`, kind: "unmapped", pid: unmappedKey },
    { stop: "rnewproj", kind: "add" },
  ];
}

/** Wrap both ways. n = 0 or a zero-width wheel returns 0. */
export function spinStep(center: number, delta: number, n: number): number {
  if (n <= 0) return 0;
  return ((center + delta) % n + n) % n;
}

/** Shortest signed slot in (−n/2, n/2]. Integer when i, center, and n are. */
export function spinOffset(i: number, center: number, n: number): number {
  if (n <= 1) return 0;
  let d = i - center;
  d = ((d % n) + n) % n;
  if (d > n / 2) d -= n;
  return d;
}

/** True when a tile's slot jumps more than one step (wrap across the centre). */
export function spinJump(prevK: number, nextK: number): boolean {
  return Math.abs(nextK - prevK) > 1;
}

/**
 * Dominant-axis wheel. Line/page (a mouse notch) is one step.
 * Pixel mode accumulates and emits at most ±2 steps per event.
 * A sign flip drops the residual. A gap longer than SPIN_SETTLE_MS
 * drops it too, so the wheel always rests on a whole item.
 */
export function spinAccumulate(
  state: SpinAccumState,
  ev: { deltaX: number; deltaY: number; deltaMode: number },
  now: number,
): { state: SpinAccumState; steps: number } {
  let residual = state.residual;
  if (state.lastAt !== 0 && now - state.lastAt > SPIN_SETTLE_MS) residual = 0;
  const dominant = Math.abs(ev.deltaY) >= Math.abs(ev.deltaX) ? ev.deltaY : ev.deltaX;
  if (dominant === 0) return { state: { residual, lastAt: now }, steps: 0 };
  const sign = Math.sign(dominant);
  if (ev.deltaMode === 1 || ev.deltaMode === 2) {
    return { state: { residual: 0, lastAt: now }, steps: sign };
  }
  if (residual !== 0 && Math.sign(residual) !== sign) residual = 0;
  residual += dominant;
  let steps = 0;
  while (Math.abs(residual) >= SPIN_STEP_PX && Math.abs(steps) < 2) {
    const stepSign = Math.sign(residual);
    steps += stepSign;
    residual -= stepSign * SPIN_STEP_PX;
  }
  if (Math.abs(steps) >= 2) residual = 0;
  return { state: { residual, lastAt: now }, steps };
}

/** Open planet if it is on the wheel; else Unmapped when there are no planets; else the first planet. */
export function spinInitial(items: readonly SpinItem[], openId: string | null): number {
  if (items.length === 0) return 0;
  if (openId) {
    const openAt = items.findIndex((it) => it.kind === "planet" && it.pid === openId);
    if (openAt >= 0) return openAt;
  }
  if (!items.some((it) => it.kind === "planet")) {
    const unmapped = items.findIndex((it) => it.kind === "unmapped");
    return unmapped >= 0 ? unmapped : 0;
  }
  const first = items.findIndex((it) => it.kind === "planet");
  return first >= 0 ? first : 0;
}

/** A highlighted wheel stop wins. Anything else keeps the previous centre, clamped. */
export function spinCenterFor(
  items: readonly SpinItem[],
  activeStopId: string | null,
  prev: number,
): number {
  const n = items.length;
  if (n === 0) return 0;
  if (activeStopId) {
    const at = items.findIndex((it) => it.stop === activeStopId);
    if (at >= 0) return at;
  }
  const c = Number.isFinite(prev) ? Math.round(prev) : 0;
  if (c < 0) return 0;
  if (c >= n) return n - 1;
  return c;
}

export function spinStops(items: readonly SpinItem[]): SpinStop[] {
  return items.map((it) => ({ id: it.stop, group: "wheel" }));
}

function spinAxis(edge: string): {
  back: SpinDir;
  forward: SpinDir;
  inward: SpinDir;
  outward: SpinDir;
} {
  switch (normalizeStageEdge(edge)) {
    case "right":
      return { back: "up", forward: "down", inward: "left", outward: "right" };
    case "top":
      return { back: "left", forward: "right", inward: "down", outward: "up" };
    case "bottom":
      return { back: "left", forward: "right", inward: "up", outward: "down" };
    default:
      return { back: "up", forward: "down", inward: "right", outward: "left" };
  }
}

/** One step along the wheel with wrap. Inward and outward stay on the wheel (null). */
export function spinArrow(
  items: readonly SpinItem[],
  activeId: string | null,
  dir: SpinDir | string,
  edge: string = "left",
): string | null {
  if (!activeId || items.length === 0) return null;
  const i = items.findIndex((it) => it.stop === activeId);
  if (i < 0) return null;
  const ax = spinAxis(edge);
  if (dir === ax.inward || dir === ax.outward) return null;
  if (dir !== ax.back && dir !== ax.forward) return null;
  const delta = dir === ax.forward ? 1 : -1;
  return items[spinStep(i, delta, items.length)].stop;
}

export function spinConfirm(stop: string): SpinConfirmAction | null {
  if (stop === "rstars") return "stars";
  if (stop === "rsettings") return "settings";
  if (stop === "rnewproj") return "newProject";
  if (stop === "rhead-__unmapped__") return "centre";
  if (stop.startsWith("rhead-") && stop.length > "rhead-".length) return "raise";
  return null;
}

/** Planet → its board. Stars, Settings, and + Project → Inbox. Unmapped → unclaimed rows. */
export function spinPane(item: SpinItem): SpinPane {
  if (item.kind === "planet" && item.pid) return { kind: "board", selected: item.pid };
  if (item.kind === "unmapped") return { kind: "unclaimed" };
  return { kind: "board", selected: "inbox" };
}
