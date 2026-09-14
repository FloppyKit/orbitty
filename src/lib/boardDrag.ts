// GO-ORBITTY-BOARD-DRAG leftover: mouse again.
// Pointer grab for board cards. WKWebView starts a native drag at ~4px
// and pointercancel's the stream; the mouse belt must survive that.
// HTML5 DnD is not the mouse door.

const SLOP = 4;

export type BoardDragCol = "todo" | "doing" | "done";

export type BoardDragHandlers = {
  blocked: boolean;
  startGrab: (cardId: string) => void;
  setCursor: (col: BoardDragCol, idx: number) => void;
  drop: () => void;
  cancel: () => void;
  colAtPoint: (x: number, y: number) => BoardDragCol | null;
  rowAtPoint: (col: BoardDragCol, y: number) => number;
};

type Press = {
  id: string;
  x: number;
  y: number;
  held: boolean;
  capEl: HTMLElement | null;
  pointerId: number | null;
  h: BoardDragHandlers;
};

let press: Press | null = null;
let wired = false;
let trailingClick = false;

export function consumeTrailingClick(): boolean {
  if (!trailingClick) return false;
  trailingClick = false;
  return true;
}

function wire() {
  if (wired) return;
  wired = true;
  window.addEventListener("pointermove", onMove);
  window.addEventListener("mousemove", onMove);
  window.addEventListener("pointerup", onUp);
  window.addEventListener("mouseup", onUp);
  window.addEventListener("keydown", onKey, true);
  window.addEventListener("blur", onBlur);
}

function unwire() {
  if (!wired) return;
  wired = false;
  window.removeEventListener("pointermove", onMove);
  window.removeEventListener("mousemove", onMove);
  window.removeEventListener("pointerup", onUp);
  window.removeEventListener("mouseup", onUp);
  window.removeEventListener("keydown", onKey, true);
  window.removeEventListener("blur", onBlur);
}

function releaseCapture(p: Press) {
  if (p.pointerId == null || !p.capEl?.hasPointerCapture?.(p.pointerId)) return;
  try {
    p.capEl.releasePointerCapture(p.pointerId);
  } catch {
    /* already released */
  }
}

function endPress() {
  const p = press;
  press = null;
  if (p) releaseCapture(p);
  unwire();
}

export function cardPointerDown(
  e: PointerEvent | MouseEvent,
  cardId: string,
  h: BoardDragHandlers,
): void {
  if (e.button !== 0 || h.blocked) return;
  e.preventDefault();
  // Compat mouse after pointerdown: keep the original press.
  if (press && press.id === cardId) return;
  const capEl = e.currentTarget instanceof HTMLElement ? e.currentTarget : null;
  const pointerId = "pointerId" in e ? e.pointerId : null;
  if (pointerId != null && capEl?.setPointerCapture) {
    try {
      capEl.setPointerCapture(pointerId);
    } catch {
      /* ignore */
    }
  }
  trailingClick = false;
  press = {
    id: cardId,
    x: e.clientX,
    y: e.clientY,
    held: false,
    capEl,
    pointerId,
    h,
  };
  wire();
}

function onMove(e: PointerEvent | MouseEvent) {
  const p = press;
  if (!p) return;
  if (!p.held) {
    if (Math.hypot(e.clientX - p.x, e.clientY - p.y) < SLOP) return;
    p.held = true;
    p.h.startGrab(p.id);
  }
  const col = p.h.colAtPoint(e.clientX, e.clientY);
  if (!col) return;
  p.h.setCursor(col, p.h.rowAtPoint(col, e.clientY));
}

function onUp() {
  const p = press;
  if (!p) return;
  const wasHeld = p.held;
  endPress();
  if (!wasHeld) return;
  trailingClick = true;
  p.h.drop();
}

function onBlur() {
  const p = press;
  if (!p) return;
  const wasHeld = p.held;
  endPress();
  if (wasHeld) p.h.cancel();
}

function onKey(e: KeyboardEvent) {
  if (e.key !== "Escape" || !press?.held) return;
  const p = press;
  endPress();
  p.h.cancel();
  e.preventDefault();
  e.stopPropagation();
}
