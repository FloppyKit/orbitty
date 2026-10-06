/** View-only macOS arch: layer math for the stage rail.
 *  Imports geometry helpers only. Nothing here is written to disk. */

// @ts-expect-error Node's test runner needs the .ts suffix; the bundler flag for it is off.
import { normalizeStageEdge, spinArrow, wheelLayout, type SpinDir, type SpinItem, type StageEdge, type ZoneRect } from "./stageLook.ts";

export const ARCH_DWELL_MS = 160;
export const ARCH_SPAN_DEG = 56;
/** Ellipse centre sits past the screen edge. Ring-1 centre lands ~170px in. */
export const ARCH_X0 = 100;
export const ARCH_RX = 158.79962131089377;
export const ARCH_RY_MAX = 330;
export const ARCH_RING_GAP = 220;
export const ARCH_ITEM_BOX = 88;
export const EDGE_SLOP = 24;
export const EDGE_HIDE_MS = 600;
export const EDGE_GRACE_MS = 900;
export const CLAIM_FLASH_MS = 2500;
export const STAGE_PLATTER_RGBA: readonly [number, number, number, number] = [28, 28, 32, 0.78];
export const STAGE_INK = "#f5f5f7";
export const STAGE_INK_DIM = "#c7c7cc";

export interface ArchFrame {
  w: number;
  h: number;
}

export const ARCH_FRAME: ArchFrame = { w: 1040, h: 760 };
const STEP_RAD = (ARCH_SPAN_DEG / 4) * (Math.PI / 180);
const THETA_MAX = 4 * STEP_RAD;
export const ARCH_DEPTH_SCALE = 0.86;
export const ARCH_DEPTH_OPACITY = 0.5;
export const ARCH_DEPTH_BLUR = 3;
export const ARCH_DEPTH_SHIFT = 24;
export const ARCH_DEPTH_MS = 360;
export const ARCH_EASING = "cubic-bezier(0.34, 1.56, 0.64, 1)";

export type ArchFront = "arch" | "layer2" | "board";
export type ArchZone = "arch" | "layer2" | "board" | "none";

export interface ArchPending {
  zone: ArchZone;
  since: number;
}

export interface ArchState {
  front: ArchFront;
  pid: string | null;
  center2: number;
  pending: ArchPending | null;
}

export const ARCH_IDLE: ArchState = {
  front: "arch",
  pid: null,
  center2: 0,
  pending: null,
};

export interface ArchCard {
  id?: string;
  title?: string;
  projectId?: string;
  windowId?: string | null;
  kind?: string;
  status?: string;
  createdAt?: number;
  neededState?: string | null;
}

export interface ArchRow {
  id: string;
  title?: string;
  sleeping?: boolean;
  isMinimized?: boolean;
  minimized?: boolean;
  iconBase64?: string | null;
  appName?: string;
  isFrontmost?: boolean;
}

export type Layer2Kind = "board" | "needed" | "window" | "doing" | "todo" | "more";

export interface Layer2Item {
  kind: Layer2Kind;
  stop: string;
  id?: string;
  title?: string;
  sleeping?: boolean;
  minimized?: boolean;
  count?: number;
  iconBase64?: string | null;
  appName?: string;
  frontmost?: boolean;
  /** Stage Manager board tile. Cards live in the embed, not as doing/todo rows. */
  preview?: boolean;
}

export type ArchConfirmAction = "board" | "wake" | "restore" | "focus";

export interface ArchSlot {
  k: number;
  x: number;
  y: number;
  rotate: number;
  scale: number;
  opacity: number;
  z: number;
  hidden: boolean;
}

export interface ArchDepth {
  scale: number;
  opacity: number;
  blur: number;
  shift: number;
  durationMs: number;
  easing: string;
}

export interface ArchLayout {
  arch: ZoneRect;
  layer2: ZoneRect;
  board: ZoneRect;
  pinned: ZoneRect;
}

export interface ArchArrowResult {
  state: ArchState;
  stop: string | null;
  step: number | null;
}

function copy(s: ArchState): ArchState {
  return {
    front: s.front,
    pid: s.pid,
    center2: s.center2,
    pending: s.pending ? { zone: s.pending.zone, since: s.pending.since } : null,
  };
}

function axis(edge: string): {
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

function falloff(k: number): { scale: number; opacity: number; z: number; hidden: boolean } {
  const a = Math.abs(k);
  if (a > 4) return { scale: 0.68, opacity: 0, z: 0, hidden: true };
  const scale = a === 0 ? 1.18 : 1 - (a - 1) * 0.08;
  const opacity = 1 - a * 0.15;
  return { scale, opacity, z: 50 - a * 8, hidden: false };
}

function ryFor(edge: StageEdge, frame: ArchFrame): number {
  const span = edge === "top" || edge === "bottom" ? frame.w : frame.h;
  const sinMax = Math.sin(THETA_MAX);
  if (sinMax <= 0) return 0;
  const fit = (span / 2 - 56) / sinMax;
  return Math.min(ARCH_RY_MAX, Math.max(0, fit));
}

function bulge(theta: number, ring: 1 | 2): number {
  const base = ARCH_X0 + ARCH_RX * (Math.cos(theta) - Math.cos(THETA_MAX));
  return ring === 2 ? base + ARCH_RING_GAP : base;
}

/** Point on the edge ellipse. theta is radians; k·STEP is `k * STEP_RAD`. */
export function archPoint(
  theta: number,
  edge: StageEdge | string = "left",
  ring: 1 | 2 = 1,
  frame: ArchFrame = ARCH_FRAME,
): { x: number; y: number } {
  const e = normalizeStageEdge(edge);
  const along = ryFor(e, frame) * Math.sin(theta);
  const from = bulge(theta, ring);
  if (e === "right") return { x: -from, y: along };
  if (e === "top") return { x: along, y: from };
  if (e === "bottom") return { x: along, y: -from };
  return { x: from, y: along };
}

/** Upright arch slot. k = 0 is the centre. Ring 2 is the same ellipse, further in. */
export function archSlot(
  k: number,
  edge: StageEdge | string = "left",
  ring: 1 | 2 = 1,
  frame?: ArchFrame,
): ArchSlot {
  const pt = archPoint(k * STEP_RAD, edge, ring, frame ?? ARCH_FRAME);
  const vis = falloff(k);
  return {
    k,
    x: pt.x,
    y: pt.y,
    rotate: 0,
    scale: vis.scale,
    opacity: vis.opacity,
    z: vis.z + (ring === 2 ? 20 : 0),
    hidden: vis.hidden,
  };
}

/** SVG `d` for the orbit the slots sit on. Samples past |k| = 4 to the frame edge. */
export function archOrbitPath(
  edge: StageEdge | string = "left",
  ring: 1 | 2 = 1,
  frame: ArchFrame = ARCH_FRAME,
): string {
  const e = normalizeStageEdge(edge);
  const limit = (e === "top" || e === "bottom" ? frame.w : frame.h) / 2;
  const ry = ryFor(e, frame);
  let thetaEdge = Math.PI / 2;
  if (ry > 1e-6) thetaEdge = Math.asin(Math.min(1, limit / ry));
  if (thetaEdge < THETA_MAX + 1e-4) {
    thetaEdge = Math.min(Math.PI / 2, THETA_MAX + STEP_RAD * 0.5);
  }
  const step = 0.35 / Math.max(ry, ARCH_RX, 1);
  const thetas: number[] = [];
  for (let th = -thetaEdge; th < thetaEdge; th += step) thetas.push(th);
  thetas.push(thetaEdge);
  for (let k = -4; k <= 4; k++) thetas.push(k * STEP_RAD);
  thetas.sort((a, b) => a - b);
  let d = "";
  let prev = Number.NaN;
  for (const th of thetas) {
    if (th < -thetaEdge - 1e-9 || th > thetaEdge + 1e-9) continue;
    if (Number.isFinite(prev) && Math.abs(th - prev) < 1e-8) continue;
    prev = th;
    const p = archPoint(th, e, ring, frame);
    d += `${d ? "L" : "M"}${p.x.toFixed(3)} ${p.y.toFixed(3)}`;
  }
  return d;
}

export function archEnter(s: ArchState, item: SpinItem, layer2: readonly Layer2Item[] = []): ArchState {
  if (item.kind === "planet" && item.pid) {
    return { front: "layer2", pid: item.pid, center2: archStart2(layer2), pending: null };
  }
  if (item.kind === "unmapped") {
    return { front: "layer2", pid: item.pid ?? null, center2: 0, pending: null };
  }
  return copy(s);
}

export function archBack(s: ArchState): ArchState {
  if (s.front === "board") return { ...copy(s), front: "layer2", pending: null };
  if (s.front === "layer2") return { ...copy(s), front: "arch", pending: null };
  return copy(s);
}

export function archForward(s: ArchState): ArchState {
  if (s.front === "arch" && s.pid) return { ...copy(s), front: "layer2", pending: null };
  return copy(s);
}

export function archCentreChanged(s: ArchState, item: SpinItem | null | undefined): ArchState {
  if (!s.pid) return copy(s);
  const same =
    !!item && (item.kind === "planet" || item.kind === "unmapped") && item.pid === s.pid;
  if (same) return copy(s);
  return { front: "arch", pid: null, center2: 0, pending: null };
}

export function archOpenBoard(s: ArchState, unmappedKey = "__unmapped__"): ArchState {
  if (s.front !== "layer2" || !s.pid || s.pid === unmappedKey) return copy(s);
  return { ...copy(s), front: "board", pending: null };
}

export function archScroll2(s: ArchState, steps: number, n2: number): ArchState {
  if (s.front !== "layer2" || steps === 0) return copy(s);
  const next = s.center2 + steps;
  if (next < 0) return archBack(s);
  const max = Math.max(0, n2 - 1);
  return { ...copy(s), center2: Math.min(next, max), pending: null };
}

function zoneBrings(s: ArchState, zone: ArchZone): ArchFront | null {
  if (zone === "arch" && s.front !== "arch") return "arch";
  if (zone === "layer2" && s.front !== "layer2" && s.pid) return "layer2";
  return null;
}

export function archPointer(s: ArchState, zone: ArchZone, now: number): ArchState {
  const dest = zoneBrings(s, zone);
  if (zone === "none" || !dest) {
    if (s.pending) return { ...copy(s), pending: null };
    return copy(s);
  }
  if (!s.pending || s.pending.zone !== zone) {
    return { ...copy(s), pending: { zone, since: now } };
  }
  if (now - s.pending.since < ARCH_DWELL_MS) return copy(s);
  return { ...copy(s), front: dest, pending: null };
}

export function archArrow(
  s: ArchState,
  items: readonly SpinItem[],
  centre: number,
  dir: string,
  edge: string = "left",
  layer2: readonly Layer2Item[] = [],
  activeId: string | null = null,
  due = false,
): ArchArrowResult | null {
  const ax = axis(edge);
  const centred = items[centre];

  if (s.front === "board") {
    if (dir === ax.outward) return { state: archBack(s), stop: null, step: null };
    return null;
  }

  if (s.front === "arch") {
    if (due && (activeId === "rcoach-claim" || activeId === "rcoach-skip")) {
      const next = spinArrow(items, activeId, dir, edge, true);
      if (!next) return { state: copy(s), stop: null, step: null };
      return { state: copy(s), stop: next, step: null };
    }
    if (dir === ax.inward) {
      const activeIsCentre = !activeId || activeId === centred?.stop;
      if (
        activeIsCentre &&
        centred &&
        (centred.kind === "planet" || centred.kind === "unmapped")
      ) {
        return { state: archEnter(s, centred, layer2), stop: null, step: null };
      }
      return null;
    }
    if (dir === ax.outward) return { state: archBack(s), stop: null, step: null };
    if (dir === ax.back || dir === ax.forward) {
      const active = activeId || centred?.stop || null;
      const next = spinArrow(items, active, dir, edge, due);
      if (!next) return null;
      const delta = dir === ax.forward ? 1 : -1;
      return { state: copy(s), stop: next, step: delta };
    }
    return null;
  }

  if (dir === ax.outward) return { state: archBack(s), stop: null, step: null };
  if (dir === ax.inward) {
    const cur = layer2[s.center2];
    if (cur?.kind === "board") return { state: archOpenBoard(s), stop: null, step: null };
    return null;
  }
  if (dir !== ax.back && dir !== ax.forward) return null;
  const n = layer2.length;
  const delta = dir === ax.forward ? 1 : -1;
  const nextIndex = s.center2 + delta;
  if (nextIndex < 0) return { state: archBack(s), stop: null, step: null };
  if (n <= 0) return { state: copy(s), stop: null, step: null };
  const clamped = Math.min(nextIndex, n - 1);
  return {
    state: { ...copy(s), center2: clamped, pending: null },
    stop: layer2[clamped]?.stop ?? null,
    step: null,
  };
}

function byCreated(a: ArchCard, b: ArchCard): number {
  return (a.createdAt ?? 0) - (b.createdAt ?? 0);
}

export function archLayer2Items(opts: {
  pid: string;
  cards: readonly ArchCard[];
  rows: readonly ArchRow[];
  maxWindows?: number;
  maxTodos?: number;
}): Layer2Item[] {
  const maxWindows = opts.maxWindows ?? 8;
  const maxTodos = opts.maxTodos ?? 10;
  const pid = opts.pid;
  const cards = opts.cards;
  const needed = cards
    .filter(
      (c) =>
        c.kind === "needed" &&
        c.projectId === pid &&
        (c.neededState === "open" || c.neededState === "later"),
    )
    .slice()
    .sort(byCreated);
  const windowRows = opts.rows.slice(0, Math.max(0, maxWindows));
  const windowOverflow = Math.max(0, opts.rows.length - windowRows.length);
  const tasks = cards.filter(
    (c) =>
      c.projectId === pid &&
      c.kind !== "needed" &&
      c.kind !== "window" &&
      !c.windowId &&
      (c.status === "doing" || c.status === "todo"),
  );
  const doing = tasks.filter((c) => c.status === "doing").slice().sort(byCreated);
  const todo = tasks.filter((c) => c.status === "todo").slice().sort(byCreated);
  const doingTake = doing.slice(0, Math.max(0, maxTodos));
  const todoTake = todo.slice(0, Math.max(0, maxTodos - doingTake.length));
  const todoOverflow = doing.length - doingTake.length + (todo.length - todoTake.length);
  const items: Layer2Item[] = [{ kind: "board", stop: "rl2-board", title: "Open board" }];
  for (const c of needed) {
    items.push({
      kind: "needed",
      stop: `rl2-n-${c.id ?? items.length}`,
      id: c.id,
      title: c.title,
    });
  }
  for (const w of windowRows) {
    items.push({
      kind: "window",
      stop: `rl2-w-${w.id}`,
      id: w.id,
      title: w.title,
      sleeping: !!w.sleeping,
      minimized: !!(w.minimized || w.isMinimized),
      iconBase64: w.iconBase64,
      appName: w.appName,
      frontmost: !!w.isFrontmost,
    });
  }
  for (const c of doingTake) {
    items.push({
      kind: "doing",
      stop: `rl2-d-${c.id ?? items.length}`,
      id: c.id,
      title: c.title,
    });
  }
  for (const c of todoTake) {
    items.push({
      kind: "todo",
      stop: `rl2-t-${c.id ?? items.length}`,
      id: c.id,
      title: c.title,
    });
  }
  const hidden = windowOverflow + todoOverflow;
  if (hidden > 0) items.push({ kind: "more", stop: "rl2-more", count: hidden, title: `+${hidden} more` });
  return items;
}

/** Windows shown as preview tiles. The rest are a count, never invented rows. */
export const PREVIEW_WINDOW_CAP = 8;
/** Gap used by beside-rail placement. Matches drawers::DRAWER_GAP. */
export const BESIDE_GAP = 8;
/** A placed pane is adjacent when the gap toward the rail is at most this, or they overlap. */
export const BESIDE_ADJACENT = 48;

export interface PlaceRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface StagePreviewModel {
  board: Layer2Item;
  windows: Layer2Item[];
  needed: Layer2Item[];
  /** Claimed windows past PREVIEW_WINDOW_CAP. Not tiles and not fake window ids. */
  overflow: number;
  popOut: { stop: string; label: string };
  /** Stage Manager order: one board tile, Needed, window tiles. No doing/todo. */
  slots: Layer2Item[];
}

function windowItem(w: ArchRow): Layer2Item {
  return {
    kind: "window",
    stop: `rl2-w-${w.id}`,
    id: w.id,
    title: w.title,
    sleeping: !!w.sleeping,
    minimized: !!(w.minimized || w.isMinimized),
    iconBase64: w.iconBase64,
    appName: w.appName,
    frontmost: !!w.isFrontmost,
  };
}

/** Stage Manager layer 2. Doing/todo cards stay inside the board tile.
 *  Window art is the card fallback (icon, title, app, state) — no capture API. */
export function stagePreviewModel(opts: {
  pid: string;
  cards: readonly ArchCard[];
  rows: readonly ArchRow[];
  maxWindows?: number;
}): StagePreviewModel {
  const maxWindows = opts.maxWindows ?? PREVIEW_WINDOW_CAP;
  const pid = opts.pid;
  const needed = opts.cards
    .filter(
      (c) =>
        c.kind === "needed" &&
        c.projectId === pid &&
        (c.neededState === "open" || c.neededState === "later"),
    )
    .slice()
    .sort(byCreated)
    .map((c, i): Layer2Item => ({
      kind: "needed",
      stop: `rl2-n-${c.id ?? i}`,
      id: c.id,
      title: c.title,
    }));
  const cap = Math.max(0, maxWindows);
  const windowRows = opts.rows.slice(0, cap);
  const overflow = Math.max(0, opts.rows.length - windowRows.length);
  const windows = windowRows.map(windowItem);
  const board: Layer2Item = {
    kind: "board",
    preview: true,
    stop: "rl2-board",
    title: "Board",
  };
  return {
    board,
    windows,
    needed,
    overflow,
    popOut: { stop: "rl2-pop", label: "Pop out" },
    slots: [board, ...needed, ...windows],
  };
}

function overlapAxes(a: PlaceRect, b: PlaceRect): { ox: number; oy: number } {
  return {
    ox: Math.max(0, Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x)),
    oy: Math.max(0, Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y)),
  };
}

function mostlyOn(rect: PlaceRect, frame: PlaceRect): boolean {
  if (rect.w <= 0 || rect.h <= 0) return false;
  const { ox, oy } = overlapAxes(rect, frame);
  return ox >= rect.w * 0.5 && oy >= rect.h * 0.5;
}

/** Place `size` on the rail's display, inboard of the rail. `stale` is ignored:
 *  a stage-originated open must not reopen a last-place on another screen. */
export function placeBesideRail(
  rail: PlaceRect,
  size: { w: number; h: number },
  frames: readonly PlaceRect[],
  stale?: PlaceRect | null,
): PlaceRect | null {
  void stale;
  if (frames.length === 0) return null;
  const vf = frames.find((frame) => mostlyOn(rail, frame)) ?? frames[0];
  const w = Math.max(1, Math.min(size.w, vf.w));
  const h = Math.max(1, Math.min(size.h, vf.h));
  const inboardRight = rail.x + rail.w / 2 < vf.x + vf.w * (2 / 3);
  let x = inboardRight ? rail.x + rail.w + BESIDE_GAP : rail.x - BESIDE_GAP - w;
  let y = rail.y;
  if (x < vf.x) x = vf.x;
  if (y < vf.y) y = vf.y;
  if (x + w > vf.x + vf.w) x = Math.max(vf.x, vf.x + vf.w - w);
  if (y + h > vf.y + vf.h) y = Math.max(vf.y, vf.y + vf.h - h);
  return { x, y, w, h };
}

/** True when the pane shares the rail's display and sits against that rail. */
export function besideRailAdjacent(placed: PlaceRect, rail: PlaceRect, frames: readonly PlaceRect[]): boolean {
  const home = frames.find((frame) => mostlyOn(rail, frame));
  if (!home || !mostlyOn(placed, home)) return false;
  const gapRight = placed.x - (rail.x + rail.w);
  const gapLeft = rail.x - (placed.x + placed.w);
  const toward =
    (gapRight >= -placed.w && gapRight <= BESIDE_ADJACENT) ||
    (gapLeft >= -placed.w && gapLeft <= BESIDE_ADJACENT);
  const yOverlap = Math.min(placed.y + placed.h, rail.y + rail.h) - Math.max(placed.y, rail.y);
  return toward && yOverlap > 0;
}

export interface StackLayout {
  needed: ZoneRect;
  board: ZoneRect;
  windows: ZoneRect[];
  settings: ZoneRect;
}

/** Preview tiles inside the wheel's preview rect (beside the arch, same screen). */
export function stageStackLayout(
  edge: string,
  frameW: number,
  frameH: number,
  windowCount: number,
  neededCount: number,
): StackLayout {
  const area = wheelLayout(edge, frameW, frameH).preview;
  const neededH = neededCount > 0 ? Math.min(44, Math.max(28, area.h * 0.08)) : 0;
  const needed: ZoneRect = { x: area.x, y: area.y, w: area.w, h: neededH };
  const bodyY = area.y + neededH + (neededH > 0 ? 8 : 0);
  const bodyH = Math.max(0, area.y + area.h - bodyY);
  const winW = windowCount > 0 ? Math.min(180, Math.max(120, area.w * 0.26)) : 0;
  const gap = windowCount > 0 && winW > 0 ? 10 : 0;
  const boardW = Math.max(0, area.w - (windowCount > 0 ? winW + gap : 0));
  const board: ZoneRect = { x: area.x, y: bodyY, w: boardW, h: bodyH };
  const windows: ZoneRect[] = [];
  if (windowCount > 0 && winW > 0 && bodyH > 0) {
    const colX = area.x + boardW + gap;
    const tileH = bodyH / windowCount;
    for (let i = 0; i < windowCount; i++) {
      windows.push({ x: colX, y: bodyY + i * tileH, w: winW, h: tileH });
    }
  }
  return {
    needed,
    board,
    windows,
    settings: { x: area.x, y: area.y, w: area.w, h: area.h },
  };
}

export function archStart2(items: readonly Layer2Item[]): number {
  const i = items.findIndex((it) => it.kind === "board");
  if (i >= 0 && i + 1 < items.length) return i + 1;
  return 0;
}

/** On the arch, the centred item's own pid. Layer 2 and the board keep `s.pid`. */
export function archFocusPid(s: ArchState, centred: SpinItem | null | undefined): string | null {
  if (s.front !== "arch") return s.pid;
  if (!centred) return null;
  if (centred.kind === "planet" || centred.kind === "unmapped") return centred.pid ?? null;
  return null;
}

export function archConfirm(item: Layer2Item): ArchConfirmAction {
  if (item.kind === "window") {
    if (item.sleeping) return "wake";
    if (item.minimized) return "restore";
    return "focus";
  }
  return "board";
}

export function archDepth(front: ArchFront, layer: 1 | 2, reduce: boolean): ArchDepth {
  const isFront = (layer === 1 && front === "arch") || (layer === 2 && front === "layer2");
  return {
    scale: isFront ? 1 : ARCH_DEPTH_SCALE,
    opacity: isFront ? 1 : ARCH_DEPTH_OPACITY,
    blur: !isFront && !reduce ? ARCH_DEPTH_BLUR : 0,
    shift: isFront ? 0 : ARCH_DEPTH_SHIFT,
    durationMs: reduce ? 0 : ARCH_DEPTH_MS,
    easing: ARCH_EASING,
  };
}

function place(rect: ZoneRect): ZoneRect {
  return { x: rect.x, y: rect.y, w: rect.w, h: rect.h };
}

function reach(ring: 1 | 2): number {
  const half = ring === 1 ? ARCH_ITEM_BOX / 2 : 148 / 2;
  return bulge(0, ring) + half + EDGE_SLOP;
}

export function archLayout(edge: string, frameW: number, frameH: number): ArchLayout {
  const base = wheelLayout(edge, frameW, frameH);
  const e = normalizeStageEdge(edge);
  const archReach = Math.max(0, reach(1));
  const l2Reach = Math.max(archReach, reach(2));
  let arch: ZoneRect;
  let layer2: ZoneRect;
  if (e === "right") {
    const aw = Math.min(archReach, frameW);
    const lw = Math.min(l2Reach, frameW);
    arch = { x: frameW - aw, y: 0, w: aw, h: frameH };
    layer2 = { x: frameW - lw, y: 0, w: Math.max(0, lw - aw), h: frameH };
  } else if (e === "top") {
    const ah = Math.min(archReach, frameH);
    const lh = Math.min(l2Reach, frameH);
    arch = { x: 0, y: 0, w: frameW, h: ah };
    layer2 = { x: 0, y: ah, w: frameW, h: Math.max(0, lh - ah) };
  } else if (e === "bottom") {
    const ah = Math.min(archReach, frameH);
    const lh = Math.min(l2Reach, frameH);
    arch = { x: 0, y: frameH - ah, w: frameW, h: ah };
    layer2 = { x: 0, y: frameH - lh, w: frameW, h: Math.max(0, lh - ah) };
  } else {
    const aw = Math.min(archReach, frameW);
    const lw = Math.min(l2Reach, frameW);
    arch = { x: 0, y: 0, w: aw, h: frameH };
    layer2 = { x: aw, y: 0, w: Math.max(0, lw - aw), h: frameH };
  }
  return {
    arch,
    layer2,
    board: place(base.preview),
    pinned: place(base.pinned),
  };
}

function inside(point: { x: number; y: number }, zone: ZoneRect): boolean {
  return (
    point.x >= zone.x &&
    point.x < zone.x + zone.w &&
    point.y >= zone.y &&
    point.y < zone.y + zone.h
  );
}

/** Front layer wins where rects meet. `"none"` is the gap. */
export function archZone(layout: ArchLayout, point: { x: number; y: number }, front: ArchFront): ArchZone {
  const inArch = inside(point, layout.arch);
  const inL2 = inside(point, layout.layer2);
  const inBoard = inside(point, layout.board);
  if (front === "board") {
    if (inBoard) return "board";
    if (inL2) return "layer2";
    if (inArch) return "arch";
    return "none";
  }
  if (front === "layer2") {
    if (inL2) return "layer2";
    if (inArch) return "arch";
    if (inBoard) return "board";
    return "none";
  }
  if (inArch) return "arch";
  if (inL2) return "layer2";
  if (inBoard) return "board";
  return "none";
}

export function archInitial(
  items: readonly SpinItem[],
  centre: number,
  planetCount: number,
): ArchState {
  const item = items[centre];
  if (planetCount <= 0 && item?.kind === "unmapped") {
    return { front: "layer2", pid: item.pid ?? null, center2: 0, pending: null };
  }
  return copy(ARCH_IDLE);
}

function grow(zone: ZoneRect, slop: number): ZoneRect {
  return {
    x: zone.x - slop,
    y: zone.y - slop,
    w: zone.w + slop * 2,
    h: zone.h + slop * 2,
  };
}

/** Painted if the point is inside the arch, the open layer 2, the board, or the pinned strip.
 *  Layer-2 Stage Manager tiles and the settings embed use the preview rect
 *  (`layout.board`) so a pointer on a painted tile never counts as outside. */
export function archPainted(
  layout: ArchLayout,
  point: { x: number; y: number },
  opts: {
    front: ArchFront;
    pid: string | null;
    revealed: boolean;
    slop?: number;
    embed?: "settings" | null;
  },
): boolean {
  const slop = opts.slop ?? EDGE_SLOP;
  const zones: ZoneRect[] = [layout.arch, layout.pinned];
  if (opts.pid) zones.push(layout.layer2);
  if (opts.front === "board") zones.push(layout.board);
  if (opts.front === "layer2" && opts.pid) zones.push(layout.board);
  if (opts.embed === "settings") zones.push(layout.board);
  // Revealed is the stage flag the caller already has. Hysteresis, not this
  // test, decides whether a miss parks the drawer.
  void opts.revealed;
  return zones.some((zone) => inside(point, grow(zone, slop)));
}

export interface EdgeHoverState {
  revealed: boolean;
  revealedAt: number;
  outsideSince: number | null;
}

export type EdgeHoverEvent =
  | { kind: "move"; painted: boolean }
  | { kind: "leave"; side: "edge" | "away" }
  | { kind: "tick" }
  | { kind: "revealed" }
  | { kind: "parked" };

function copyEdge(s: EdgeHoverState): EdgeHoverState {
  return { revealed: s.revealed, revealedAt: s.revealedAt, outsideSince: s.outsideSince };
}

/** Park only after a real exit, past the grace window. An edge-side leave is still "at the edge". */
export function edgeHover(
  s: EdgeHoverState,
  ev: EdgeHoverEvent,
  now: number,
): { state: EdgeHoverState; act: "reveal" | "park" | null } {
  if (ev.kind === "revealed") {
    if (s.revealed) return { state: copyEdge(s), act: null };
    return { state: { revealed: true, revealedAt: now, outsideSince: null }, act: null };
  }
  if (ev.kind === "parked") {
    return { state: { revealed: false, revealedAt: s.revealedAt, outsideSince: null }, act: null };
  }
  if (!s.revealed) {
    if (ev.kind === "move") {
      return { state: { revealed: true, revealedAt: now, outsideSince: null }, act: "reveal" };
    }
    return { state: copyEdge(s), act: null };
  }
  if (ev.kind === "move") {
    if (ev.painted) return { state: { ...copyEdge(s), outsideSince: null }, act: null };
    if (s.outsideSince == null) return { state: { ...copyEdge(s), outsideSince: now }, act: null };
    return { state: copyEdge(s), act: null };
  }
  if (ev.kind === "leave") {
    if (ev.side === "edge") return { state: copyEdge(s), act: null };
    if (s.outsideSince == null) return { state: { ...copyEdge(s), outsideSince: now }, act: null };
    return { state: copyEdge(s), act: null };
  }
  if (s.outsideSince == null) return { state: copyEdge(s), act: null };
  const hiddenLong = now - s.outsideSince >= EDGE_HIDE_MS;
  const graced = now - s.revealedAt >= EDGE_GRACE_MS;
  if (hiddenLong && graced) {
    return { state: { revealed: false, revealedAt: s.revealedAt, outsideSince: null }, act: "park" };
  }
  return { state: copyEdge(s), act: null };
}

/** Which border the pointer crossed. The screen-edge side is not an exit. */
export function edgeLeaveSide(
  edge: string,
  clientX: number,
  clientY: number,
  w: number,
  h: number,
): "edge" | "away" {
  const e = normalizeStageEdge(edge);
  if (e === "right") return clientX >= w ? "edge" : "away";
  if (e === "top") return clientY <= 0 ? "edge" : "away";
  if (e === "bottom") return clientY >= h ? "edge" : "away";
  return clientX <= 0 ? "edge" : "away";
}

export interface ClaimSnap {
  windowId: string;
  projectId: string;
  lastTouchedAt?: number;
}

/** The claim that appeared or moved. First load and unclaims are quiet. */
export function claimLanded(
  prev: readonly ClaimSnap[] | null,
  next: readonly ClaimSnap[],
): { windowId: string; projectId: string } | null {
  if (prev === null) return null;
  const before = new Map(prev.map((c) => [c.windowId, c.projectId]));
  let best: ClaimSnap | null = null;
  for (const c of next) {
    const old = before.get(c.windowId);
    if (old === c.projectId) continue;
    if (!best || (c.lastTouchedAt ?? 0) >= (best.lastTouchedAt ?? 0)) best = c;
  }
  if (!best) return null;
  return { windowId: best.windowId, projectId: best.projectId };
}

function hexRgb(hex: string): [number, number, number] {
  const h = hex.trim().replace("#", "");
  const n = h.length === 3
    ? h.split("").map((c) => parseInt(c + c, 16))
    : [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
  return [n[0] || 0, n[1] || 0, n[2] || 0];
}

function wallRgb(wallpaper: string): [number, number, number] {
  const name = wallpaper.trim().toLowerCase();
  if (name === "white" || name === "#fff" || name === "#ffffff") return [255, 255, 255];
  if (name === "black" || name === "#000" || name === "#000000") return [0, 0, 0];
  return hexRgb(wallpaper);
}

function channel(c: number): number {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
}

function luminance(rgb: [number, number, number]): number {
  return 0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2]);
}

/** WCAG contrast of `fg` over a platter composited onto `wallpaper`. */
export function stageContrast(
  fg: string,
  rgba: readonly [number, number, number, number],
  wallpaper: string,
): number {
  const wall = wallRgb(wallpaper);
  const a = rgba[3];
  const bg: [number, number, number] = [
    rgba[0] * a + wall[0] * (1 - a),
    rgba[1] * a + wall[1] * (1 - a),
    rgba[2] * a + wall[2] * (1 - a),
  ];
  const lighter = Math.max(luminance(hexRgb(fg)), luminance(bg));
  const darker = Math.min(luminance(hexRgb(fg)), luminance(bg));
  return (lighter + 0.05) / (darker + 0.05);
}
