/** View-only macOS arch: layer math for the stage rail.
 *  Imports geometry helpers only. Nothing here is written to disk. */

// @ts-expect-error Node's test runner needs the .ts suffix; the bundler flag for it is off.
import { normalizeStageEdge, spinArrow, spinStep, wheelLayout, type SpinDir, type SpinItem, type StageEdge, type ZoneRect } from "./stageLook.ts";
// @ts-expect-error Node's test runner needs the .ts suffix; the bundler flag for it is off.
import { mirrorStart, slotFramePoint } from "./stagePlace.ts";

export const ARCH_DWELL_MS = 160;
export const ARCH_SPAN_DEG = 56;
/** Ellipse centre sits past the screen edge. Ring-1 centre lands ~170px in. */
export const ARCH_X0 = 100;
export const ARCH_RX = 158.79962131089377;
export const ARCH_RY_MAX = 330;
export const ARCH_RING_GAP = 220;
/** Layer-2 slots and their labels share this ring. */
export const L2_RING = 1 as const;
export const ARCH_ITEM_BOX = 88;
export const EDGE_SLOP = 24;
export const EDGE_HIDE_MS = 600;
export const EDGE_GRACE_MS = 900;
/** Inboard gap between the arch and the expanded mirror panel. */
export const MIRROR_GAP = 12;
/** Layer 1 slides toward the screen edge by this many pixels when layer 2 is front. */
export const MIRROR_RECEDE = 110;
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
export const ARCH_DEPTH_OPACITY = 0.45;
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
  neededWhy?: string | null;
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
  why?: string;
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

export type ArchWalkIn = "stars" | "settings" | "unmapped" | "board";

export interface ArchArrowResult {
  state: ArchState;
  stop: string | null;
  step: number | null;
  /** Set when the inward arrow walks into a beside panel. Absent otherwise. */
  walkIn?: ArchWalkIn | null;
  /** Set in the same transition as walkIn "board". */
  focus?: BoardFocus | null;
}

export function copy(s: ArchState): ArchState {
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

export function bulge(theta: number, ring: 1 | 2): number {
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

function radialCentre(ring: 1 | 2): number {
  const shift = ring === 2 ? ARCH_RING_GAP : 0;
  return ARCH_X0 - ARCH_RX * Math.cos(THETA_MAX) + shift;
}

function fromAt(theta: number, ring: 1 | 2): number {
  return radialCentre(ring) + ARCH_RX * Math.cos(theta);
}

/** Theta past ±90° where the ellipse meets the screen edge, or 90° when it cannot. */
function tailTheta(ring: 1 | 2): number {
  if (fromAt(Math.PI / 2, ring) <= 0) return Math.PI / 2;
  if (fromAt(Math.PI, ring) > 0) return Math.PI / 2;
  let lo = Math.PI / 2;
  let hi = Math.PI;
  for (let i = 0; i < 48; i++) {
    const mid = (lo + hi) / 2;
    if (fromAt(mid, ring) > 0) lo = mid;
    else hi = mid;
  }
  return hi;
}

export interface ArchOrbitArc {
  d: string;
  cx: number;
  cy: number;
  rx: number;
  ry: number;
  t0: number;
  t1: number;
  edge: StageEdge;
  ring: 1 | 2;
  frame: ArchFrame;
}

/** Elliptical arc of the slot ellipse, continued until it leaves through the screen edge. */
export function archOrbitArc(
  edge: StageEdge | string = "left",
  ring: 1 | 2 = 1,
  frame: ArchFrame = ARCH_FRAME,
): ArchOrbitArc {
  const e = normalizeStageEdge(edge);
  const ry = ryFor(e, frame);
  const radial = radialCentre(ring);
  const tEdge = tailTheta(ring);
  let cx = radial;
  let cy = 0;
  if (e === "right") cx = -radial;
  else if (e === "top") {
    cx = 0;
    cy = radial;
  } else if (e === "bottom") {
    cx = 0;
    cy = -radial;
  }
  const a = archPoint(-tEdge, e, ring, frame);
  const b = archPoint(tEdge, e, ring, frame);
  const vertical = e === "top" || e === "bottom";
  const drawnRx = vertical ? ry : ARCH_RX;
  const drawnRy = vertical ? ARCH_RX : ry;
  const sweep = e === "right" || e === "top" ? 0 : 1;
  const span = tEdge - -tEdge;
  const large = span > Math.PI ? 1 : 0;
  const d = `M${a.x.toFixed(3)} ${a.y.toFixed(3)}A${drawnRx.toFixed(3)} ${drawnRy.toFixed(3)} 0 ${large} ${sweep} ${b.x.toFixed(3)} ${b.y.toFixed(3)}`;
  return { d, cx, cy, rx: drawnRx, ry: drawnRy, t0: -tEdge, t1: tEdge, edge: e, ring, frame };
}

export function archOrbitPoint(arc: ArchOrbitArc, theta: number): { x: number; y: number } {
  return archPoint(theta, arc.edge, arc.ring, arc.frame);
}

export function archOrbitSamples(arc: ArchOrbitArc, n = 64): { x: number; y: number }[] {
  const pts: { x: number; y: number }[] = [];
  const steps = Math.max(2, n);
  for (let i = 0; i <= steps; i++) {
    const t = arc.t0 + ((arc.t1 - arc.t0) * i) / steps;
    pts.push(archOrbitPoint(arc, t));
  }
  return pts;
}

/** Fade axis: screen edge → bulge, so both tails (the low end) dissolve. */
export function archOrbitFade(arc: ArchOrbitArc): { x1: number; y1: number; x2: number; y2: number } {
  const mid = archOrbitPoint(arc, 0);
  const tail = archOrbitPoint(arc, arc.t0);
  if (arc.edge === "top" || arc.edge === "bottom") {
    return { x1: 0, y1: tail.y, x2: 0, y2: mid.y };
  }
  return { x1: tail.x, y1: 0, x2: mid.x, y2: 0 };
}

/** SVG `d` for the orbit. Elliptical arc of the same ellipse, tails through the screen edge. */
export function archOrbitPath(
  edge: StageEdge | string = "left",
  ring: 1 | 2 = 1,
  frame: ArchFrame = ARCH_FRAME,
): string {
  return archOrbitArc(edge, ring, frame).d;
}

export interface ArchLabelBox {
  x: number;
  y: number;
  w: number;
  h: number;
  side: "below" | "inboard";
}

const LABEL_BELOW_GAP = 4;
const LABEL_INBOARD_GAP = 4;

export function archMarbleBox(
  k: number,
  edge: StageEdge | string,
  ring: 1 | 2,
  frame: ArchFrame,
  selected = k === 0,
): ZoneRect {
  const slot = archSlot(k, edge, ring, frame);
  const c = slotFramePoint(edge, slot, frame);
  const size = selected ? 84 : 66 * slot.scale;
  return { x: c.x - size / 2, y: c.y - size / 2, w: size, h: size };
}

function rectsHit(a: ZoneRect, b: ZoneRect): boolean {
  return a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;
}

function edgeGap(a: ZoneRect, b: ZoneRect): number {
  const dx = Math.max(a.x - (b.x + b.w), b.x - (a.x + a.w), 0);
  const dy = Math.max(a.y - (b.y + b.h), b.y - (a.y + a.h), 0);
  return Math.hypot(dx, dy);
}

function placeLabel(
  marble: ZoneRect,
  edge: string,
  w: number,
  h: number,
  side: "below" | "inboard" | "outboard",
): ZoneRect {
  const e = normalizeStageEdge(edge);
  const gap = side === "below" ? LABEL_BELOW_GAP : LABEL_INBOARD_GAP;
  const yMid = marble.y + marble.h / 2 - h / 2;
  const xMid = marble.x + marble.w / 2 - w / 2;
  if (side === "below") {
    if (e === "bottom") return { x: xMid, y: marble.y - gap - h, w, h };
    if (e === "top") return { x: xMid, y: marble.y + marble.h + gap, w, h };
    return { x: xMid, y: marble.y + marble.h + gap, w, h };
  }
  const inward = side === "inboard";
  if (e === "right") {
    return inward
      ? { x: marble.x - gap - w, y: yMid, w, h }
      : { x: marble.x + marble.w + gap, y: yMid, w, h };
  }
  if (e === "top") {
    return inward
      ? { x: xMid, y: marble.y + marble.h + gap, w, h }
      : { x: xMid, y: marble.y - gap - h, w, h };
  }
  if (e === "bottom") {
    return inward
      ? { x: xMid, y: marble.y - gap - h, w, h }
      : { x: xMid, y: marble.y + marble.h + gap, w, h };
  }
  return inward
    ? { x: marble.x + marble.w + gap, y: yMid, w, h }
    : { x: marble.x - gap - w, y: yMid, w, h };
}

function layoutLabels(
  edge: string,
  ring: 1 | 2,
  frame: ArchFrame,
  avoid: readonly ZoneRect[],
): Map<number, ArchLabelBox> {
  const ks: number[] = [];
  for (let k = -4; k <= 4; k++) {
    if (!archSlot(k, edge, ring, frame).hidden) ks.push(k);
  }
  const marbles = new Map(ks.map((k) => [k, archMarbleBox(k, edge, ring, frame, k === 0)]));
  const chosen = new Map<number, ArchLabelBox>();
  const order = [...ks].sort((a, b) => Math.abs(a) - Math.abs(b) || a - b);
  for (const k of order) {
    const h = k === 0 ? 23 : 22;
    const marble = marbles.get(k)!;
    let best: ArchLabelBox | null = null;
    let beside: ArchLabelBox | null = null;
    for (const width of [k === 0 ? 120 : 96, 72, 56, 40]) {
      for (const side of ["below", "inboard", "outboard"] as const) {
        const box = placeLabel(marble, edge, width, h, side);
        const hitM = [...marbles.entries()].some(([kk, m]) => kk !== k && rectsHit(box, m));
        const hitL = [...chosen.values()].some((o) => rectsHit(box, o));
        const hitA = avoid.some((a) => a.w > 0 && a.h > 0 && rectsHit(box, a));
        const gap = edgeGap(marble, box);
        const gapOk = side === "below" ? gap <= 4.01 : gap <= 6.01;
        if (hitM || hitL || !gapOk || gap <= 0.5) continue;
        const placed: ArchLabelBox = { ...box, side: side === "below" ? "below" : "inboard" };
        if (!hitA) {
          best = placed;
          break;
        }
        beside ??= placed;
      }
      if (best) break;
    }
    chosen.set(k, best ?? beside ?? { ...placeLabel(marble, edge, 40, h, "outboard"), side: "inboard" });
  }
  return chosen;
}

/** Label rect in frame coordinates. Below the marble when it clears; otherwise inboard. */
export function archLabelBox(
  k: number,
  edge: StageEdge | string = "left",
  ring: 1 | 2 = 1,
  frame: ArchFrame = ARCH_FRAME,
  opts?: { avoid?: readonly ZoneRect[] },
): ArchLabelBox {
  const all = layoutLabels(edge, ring, frame, opts?.avoid ?? []);
  return all.get(k) ?? { x: 0, y: 0, w: 96, h: 15, side: "below" };
}

export function neededAct(item: {
  kind?: string;
  id?: string;
  title?: string;
}): { kind: "mouth"; id: string; title: string } | null {
  if (item.kind !== "needed" || !item.id) return null;
  return { kind: "mouth", id: item.id, title: item.title ?? "" };
}

export function thumbShowsImage(state: string | null | undefined): boolean {
  return state === "ok";
}

export function archEnter(s: ArchState, item: SpinItem, layer2: readonly Layer2Item[] = []): ArchState {
  if (item.kind === "planet" && item.pid) {
    return { front: "layer2", pid: item.pid, center2: mirrorStart(layer2), pending: null };
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

export interface BoardFocus {
  kind: "card" | "add";
  id: string | null;
}

function statusRank(status: string | undefined): number {
  if (status === "todo") return 0;
  if (status === "doing") return 1;
  if (status === "done") return 2;
  return 3;
}

/** One inward step from layer 2: the board is front and a card (or the add
 *  field, when the planet has no cards) is the focus target. No limbo. */
export function enterBoard(
  s: ArchState,
  cards: readonly { id: string; status?: string }[] = [],
  unmappedKey = "__unmapped__",
): { state: ArchState; focus: BoardFocus } | null {
  if (s.front !== "layer2") return null;
  const state = archOpenBoard(s, unmappedKey);
  if (state.front !== "board") return null;
  const ranked = cards.filter((c) => !!c.id).slice().sort((a, b) => statusRank(a.status) - statusRank(b.status));
  const focus: BoardFocus = ranked.length
    ? { kind: "card", id: ranked[0].id }
    : { kind: "add", id: null };
  return { state, focus };
}

export function archScroll2(s: ArchState, steps: number, n2: number): ArchState {
  if (s.front !== "layer2" || steps === 0) return copy(s);
  if (n2 <= 0) return copy(s);
  return { ...copy(s), center2: spinStep(s.center2, steps, n2), pending: null };
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
  // Hover dwell may keep `pending` as the highlight. It never changes depth.
  void dest;
  void ARCH_DWELL_MS;
  return copy(s);
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
  cards: readonly { id: string; status?: string }[] = [],
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
      const activeItem = activeId ? items.find((it) => it.stop === activeId) : undefined;
      const subject = activeItem ?? centred;
      if (subject?.kind === "stars" || subject?.kind === "settings") {
        return { state: copy(s), stop: subject.stop, step: null, walkIn: subject.kind };
      }
      const activeIsCentre = !activeId || activeId === centred?.stop;
      if (
        activeIsCentre &&
        centred &&
        (centred.kind === "planet" || centred.kind === "unmapped")
      ) {
        return {
          state: archEnter(s, centred, layer2),
          stop: null,
          step: null,
          walkIn: centred.kind === "unmapped" ? "unmapped" : null,
        };
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
    const cur =
      (activeId && layer2.find((it) => it.stop === activeId)) || layer2[s.center2];
    if (cur?.kind === "board") {
      const land = enterBoard(s, cards);
      if (!land) return null;
      return { state: land.state, stop: cur.stop, step: null, walkIn: "board", focus: land.focus };
    }
    return null;
  }
  if (dir !== ax.back && dir !== ax.forward) return null;
  const n = layer2.length;
  if (n <= 0) return { state: copy(s), stop: null, step: null, walkIn: null };
  const delta = dir === ax.forward ? 1 : -1;
  const nextIndex = spinStep(s.center2, delta, n);
  return {
    state: { ...copy(s), center2: nextIndex, pending: null },
    stop: layer2[nextIndex]?.stop ?? null,
    step: null,
    walkIn: null,
  };
}

export function byCreated(a: ArchCard, b: ArchCard): number {
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

export function windowItem(w: ArchRow): Layer2Item {
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

// @ts-expect-error Node's test runner needs the .ts suffix; the bundler flag for it is off.
export { archConfirm, archDepth, archFocusPid, archInitial, archLayout, archPainted, archStart2, archZone, besideRailAdjacent, claimLanded, edgeHover, edgeLeaveSide, mirrorDepth, mirrorItems, mirrorLayout, mirrorSlot, mirrorSlotBox, mirrorStart, placeBesideRail, previewCardOuter, previewCardScale, stageContrast, stagePreviewModel, stageStackLayout, wheelZone, type ClaimSnap, type EdgeHoverEvent, type EdgeHoverState, type MirrorLayout, type PreviewCardBox, type StackLayout } from "./stagePlace.ts";
