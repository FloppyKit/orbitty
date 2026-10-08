// @ts-expect-error Node's test runner needs the .ts suffix; the bundler flag for it is off.
import { normalizeStageEdge, wheelLayout, type SpinItem, type StageEdge, type ZoneRect } from "./stageLook.ts";
// @ts-expect-error Node's test runner needs the .ts suffix; the bundler flag for it is off.
import { ARCH_DEPTH_BLUR, ARCH_DEPTH_MS, ARCH_DEPTH_OPACITY, ARCH_DEPTH_SCALE, ARCH_DEPTH_SHIFT, ARCH_EASING, ARCH_IDLE, ARCH_ITEM_BOX, BESIDE_ADJACENT, BESIDE_GAP, EDGE_GRACE_MS, EDGE_HIDE_MS, EDGE_SLOP, MIRROR_GAP, MIRROR_RECEDE, PREVIEW_WINDOW_CAP, archSlot, bulge, byCreated, copy, windowItem, type ArchCard, type ArchConfirmAction, type ArchDepth, type ArchFrame, type ArchFront, type ArchLayout, type ArchRow, type ArchSlot, type ArchState, type ArchZone, type Layer2Item, type PlaceRect, type StagePreviewModel } from "./stageArch.ts";

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

export interface MirrorLayout {
  arch: ZoneRect;
  expanded: ZoneRect;
  back: ZoneRect;
}

/** Board, then Needed, then every claimed window (asleep included). No cap, no doing/todo rows. */
export function mirrorItems(opts: {
  pid: string;
  cards: readonly ArchCard[];
  rows: readonly ArchRow[];
}): Layer2Item[] {
  const model = stagePreviewModel({
    pid: opts.pid,
    cards: opts.cards,
    rows: opts.rows,
    maxWindows: opts.rows.length,
  });
  return model.slots.map((it) => {
    if (it.kind !== "needed" || !it.id) return it;
    const card = opts.cards.find((c) => c.id === it.id);
    const why = card?.neededWhy?.trim();
    return why ? { ...it, why } : it;
  });
}

/** Layer 2 opens centred on Board. */
export function mirrorStart(items: readonly Layer2Item[]): number {
  const i = items.findIndex((it) => it.kind === "board");
  return i >= 0 ? i : 0;
}

/** Layer-2 icon `index` while `center` is the centred stop. Same ellipse as ring 1. */
export function mirrorSlot(
  index: number,
  center: number,
  edge: StageEdge | string = "left",
  frame?: ArchFrame,
): ArchSlot {
  return archSlot(index - center, edge, 1, frame);
}

export function slotFramePoint(
  edge: string,
  slot: { x: number; y: number },
  frame: ArchFrame,
): { x: number; y: number } {
  const e = normalizeStageEdge(edge);
  if (e === "right") return { x: frame.w + slot.x, y: frame.h / 2 + slot.y };
  if (e === "top") return { x: frame.w / 2 + slot.x, y: slot.y };
  if (e === "bottom") return { x: frame.w / 2 + slot.x, y: frame.h + slot.y };
  return { x: slot.x, y: frame.h / 2 + slot.y };
}

/** Visible icon box. `hover` applies the 1.06 hover scale on top of the slot scale (1.18 at centre). */
export function mirrorSlotBox(
  k: number,
  edge: string,
  frame: ArchFrame,
  hover = 1.06,
): ZoneRect {
  const slot = archSlot(k, edge, 1, frame);
  const size = ARCH_ITEM_BOX * slot.scale * hover;
  const c = slotFramePoint(edge, slot, frame);
  return { x: c.x - size / 2, y: c.y - size / 2, w: size, h: size };
}

function visibleSlotBoxes(edge: string, frame: ArchFrame): ZoneRect[] {
  const boxes: ZoneRect[] = [];
  for (let k = -4; k <= 4; k++) {
    const slot = archSlot(k, edge, 1, frame);
    if (slot.hidden) continue;
    boxes.push(mirrorSlotBox(k, edge, frame, 1.06));
  }
  return boxes;
}

/** Inboard of the arch, clear of every visible slot box (selected ×1.18 and hover ×1.06). */
export function mirrorLayout(edge: string, frameW: number, frameH: number): MirrorLayout {
  const frame: ArchFrame = { w: frameW, h: frameH };
  const lay = archLayout(edge, frameW, frameH);
  const e = normalizeStageEdge(edge);
  const boxes = visibleSlotBoxes(e, frame);
  const gap = MIRROR_GAP;
  let expanded: ZoneRect;
  let back: ZoneRect;
  if (e === "right") {
    const slotLeft = boxes.reduce((m, b) => Math.min(m, b.x), lay.arch.x);
    const x2 = Math.min(lay.arch.x, slotLeft) - gap;
    expanded = { x: gap, y: gap, w: Math.max(0, x2 - gap), h: Math.max(0, frameH - gap * 2) };
    const inset = boxes.reduce((m, b) => Math.min(m, frameW - (b.x + b.w)), frameW);
    const bw = Math.max(0, inset - 8);
    back = { x: frameW - bw, y: 0, w: bw, h: frameH };
  } else if (e === "top") {
    const slotBottom = boxes.reduce((m, b) => Math.max(m, b.y + b.h), 0);
    const y = Math.max(lay.arch.y + lay.arch.h, slotBottom) + gap;
    expanded = { x: gap, y, w: Math.max(0, frameW - gap * 2), h: Math.max(0, frameH - y - gap) };
    const inset = boxes.reduce((m, b) => Math.min(m, b.y), frameH);
    const bh = Math.max(0, inset - 8);
    back = { x: 0, y: 0, w: frameW, h: bh };
  } else if (e === "bottom") {
    const slotTop = boxes.reduce((m, b) => Math.min(m, b.y), frameH);
    const y2 = Math.min(lay.arch.y, slotTop) - gap;
    expanded = { x: gap, y: gap, w: Math.max(0, frameW - gap * 2), h: Math.max(0, y2 - gap) };
    const inset = boxes.reduce((m, b) => Math.min(m, frameH - (b.y + b.h)), frameH);
    const bh = Math.max(0, inset - 8);
    back = { x: 0, y: frameH - bh, w: frameW, h: bh };
  } else {
    const slotRight = boxes.reduce((m, b) => Math.max(m, b.x + b.w), 0);
    const x = Math.max(lay.arch.x + lay.arch.w, slotRight) + gap;
    expanded = { x, y: gap, w: Math.max(0, frameW - x - gap), h: Math.max(0, frameH - gap * 2) };
    const inset = boxes.reduce((m, b) => Math.min(m, b.x), frameW);
    const bw = Math.max(0, inset - 8);
    back = { x: 0, y: 0, w: bw, h: frameH };
  }
  return { arch: lay.arch, expanded, back };
}

/** Layer 1 recedes toward the edge while layer 2 is front. `archDepth` stays the locked contract. */
export function mirrorDepth(front: ArchFront, layer: 1 | 2, reduce: boolean): ArchDepth {
  if (front === "layer2" && layer === 1) {
    return {
      scale: 0.72,
      opacity: 0.45,
      blur: reduce ? 0 : 3,
      shift: MIRROR_RECEDE,
      durationMs: reduce ? 0 : ARCH_DEPTH_MS,
      easing: ARCH_EASING,
    };
  }
  if (front === "layer2" && layer === 2) {
    return {
      scale: 1,
      opacity: 1,
      blur: 0,
      shift: 0,
      durationMs: reduce ? 0 : ARCH_DEPTH_MS,
      easing: ARCH_EASING,
    };
  }
  return archDepth(front, layer, reduce);
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
    embed?: "settings" | "stars" | null;
    edge?: string;
    frame?: ArchFrame;
  },
): boolean {
  const slop = opts.slop ?? EDGE_SLOP;
  const zones: ZoneRect[] = [layout.arch, layout.pinned];
  if (opts.pid) zones.push(layout.layer2);
  if (opts.front === "board") zones.push(layout.board);
  if (opts.front === "layer2" && opts.pid) zones.push(layout.board);
  if (opts.front === "layer2" && opts.pid && opts.edge && opts.frame) {
    const mirror = mirrorLayout(opts.edge, opts.frame.w, opts.frame.h);
    zones.push(mirror.expanded, mirror.back);
  }
  if (opts.embed === "settings" || opts.embed === "stars") zones.push(layout.board);
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
