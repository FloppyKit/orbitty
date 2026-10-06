/** View-only macOS arch: layer math for the stage rail.
 *  Imports geometry helpers only. Nothing here is written to disk. */

// @ts-expect-error Node's test runner needs the .ts suffix; the bundler flag for it is off.
import { normalizeStageEdge, spinArrow, wheelLayout, type SpinDir, type SpinItem, type StageEdge, type ZoneRect } from "./stageLook.ts";

export const ARCH_DWELL_MS = 160;
export const ARCH_RADIUS = 560;
export const ARCH_RADIUS_2 = 750;
export const ARCH_SPAN_DEG = 56;
export const ARCH_DEPTH_SCALE = 0.86;
export const ARCH_DEPTH_OPACITY = 0.5;
export const ARCH_DEPTH_BLUR = 3;
export const ARCH_DEPTH_SHIFT = 24;
export const ARCH_DEPTH_MS = 360;
export const ARCH_EASING = "cubic-bezier(0.34, 1.56, 0.64, 1)";

const STEP_DEG = ARCH_SPAN_DEG / 4;

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

/** Upright arch slot. k = 0 is the centre. Ring 2 is concentric, toward screen centre. */
export function archSlot(k: number, edge: StageEdge | string = "left", ring: 1 | 2 = 1): ArchSlot {
  const e = normalizeStageEdge(edge);
  const radius = ring === 2 ? ARCH_RADIUS_2 : ARCH_RADIUS;
  const theta = k * STEP_DEG * (Math.PI / 180);
  const bulge = radius * Math.cos(theta);
  const along = radius * Math.sin(theta);
  const vis = falloff(k);
  let x = bulge;
  let y = along;
  if (e === "right") x = -bulge;
  else if (e === "top") {
    x = along;
    y = bulge;
  } else if (e === "bottom") {
    x = along;
    y = -bulge;
  }
  return {
    k,
    x,
    y,
    rotate: 0,
    scale: vis.scale,
    opacity: vis.opacity,
    z: vis.z + (ring === 2 ? 20 : 0),
    hidden: vis.hidden,
  };
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

export function archLayout(edge: string, frameW: number, frameH: number): ArchLayout {
  const base = wheelLayout(edge, frameW, frameH);
  const e = normalizeStageEdge(edge);
  const arch = place(base.wheel);
  let layer2: ZoneRect;
  if (e === "right") {
    const w = Math.min(240, Math.max(0, arch.x));
    layer2 = { x: arch.x - w, y: arch.y, w, h: arch.h };
  } else if (e === "top") {
    const y = arch.y + arch.h;
    const h = Math.min(180, Math.max(0, frameH - y));
    layer2 = { x: 0, y, w: Math.max(0, frameW), h };
  } else if (e === "bottom") {
    const h = Math.min(180, Math.max(0, arch.y));
    layer2 = { x: 0, y: arch.y - h, w: Math.max(0, frameW), h };
  } else {
    const x = arch.x + arch.w;
    const w = Math.min(240, Math.max(0, frameW - x));
    layer2 = { x, y: arch.y, w, h: arch.h };
  }
  return { arch, layer2, board: place(base.preview) };
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
