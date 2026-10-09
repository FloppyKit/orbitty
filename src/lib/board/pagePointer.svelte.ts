// Moved function bodies. scope fields are the lets each function already closed over.

import type { CardStatus as Status, KanbanCard, Project, WindowInfo } from "../persist";

type Invoke = <T = unknown>(cmd: string, args?: any) => Promise<T>;

type Scope_onCardDragStart = {};

type Scope_onCardDragEnd = {
  dragCardId: string | null;
  dropCol: Status | null;
};

type Scope_isCardDrag = {
  dragCardId: string | null;
};

type Scope_onColDragOver = {
  isCardDrag: (...args: any[]) => boolean;
  dropCol: Status | null;
};

type Scope_onColDragLeave = {
  dropCol: Status | null;
};

type Scope_onColDrop = {
  dragCardId: string | null;
  kanbanCards: KanbanCard[];
  moveCard: (...args: any[]) => any;
  dropCol: Status | null;
};

type Scope_onCardPointerDown = {
  grabbedId: string | null;
  suppressClick: boolean;
  ptr: { id: string; x: number; y: number; held: boolean; ox: number; oy: number; w: number; h: number } | null;
  onPtrMove: (...args: any[]) => any;
  onPtrUp: (...args: any[]) => any;
  onPtrCancel: (...args: any[]) => any;
  onPtrKey: (...args: any[]) => any;
};

type Scope_onPtrMove = {
  ptr: { id: string; x: number; y: number; held: boolean; ox: number; oy: number; w: number; h: number } | null;
  kanbanCards: KanbanCard[];
  ptrEnd: (...args: any[]) => any;
  startGrab: (...args: any[]) => any;
  grabGhost: { x: number; y: number; w: number; h: number } | null;
  grabbedId: string | null;
  overStartStrip: (...args: any[]) => boolean;
  colAtPoint: (...args: any[]) => Status | null;
  grabCol: Status | null;
  grabIdx: number;
  rowAtPoint: (...args: any[]) => number;
};

type Scope_onPtrUp = {
  ptr: { id: string; x: number; y: number; held: boolean; ox: number; oy: number; w: number; h: number } | null;
  overStartStrip: (...args: any[]) => boolean;
  grabCol: Status | null;
  grabbedId: string | null;
  ptrEnd: (...args: any[]) => any;
  suppressClick: boolean;
  dropGrab: (...args: any[]) => any;
};

type Scope_onPtrCancel = {
  ptr: { id: string; x: number; y: number; held: boolean; ox: number; oy: number; w: number; h: number } | null;
  grabbedId: string | null;
  ungrab: (...args: any[]) => any;
  ptrEnd: (...args: any[]) => any;
};

type Scope_ptrEnd = {
  ptr: { id: string; x: number; y: number; held: boolean; ox: number; oy: number; w: number; h: number } | null;
  onPtrMove: (...args: any[]) => any;
  onPtrUp: (...args: any[]) => any;
  onPtrCancel: (...args: any[]) => any;
  onPtrKey: (...args: any[]) => any;
};

type Scope_onPtrKey = {
  ptr: { id: string; x: number; y: number; held: boolean; ox: number; oy: number; w: number; h: number } | null;
  grabbedId: string | null;
  ungrab: (...args: any[]) => any;
};

type Scope_overStartStrip = {};

type Scope_colAtPoint = {
  COLUMNS: { id: Status; name: string; hint: string }[];
};

type Scope_rowAtPoint = {};

type Scope_onCardClick = {
  suppressClick: boolean;
  openDetail: (...args: any[]) => any;
};

type Scope_refreshWindows = {
  windows: WindowInfo[];
  invoke: Invoke;
};

type Scope_focusCard = {
  winFor: (...args: any[]) => WindowInfo | undefined;
  invoke: Invoke;
};

type Scope_launchHermes = {
  selectedProject: Project | undefined;
  kanbanCards: KanbanCard[];
  listOf: (...args: any[]) => string;
  invoke: Invoke;
  hermesNote: { kind: "ok" | "err"; text: string } | null;
};

export function onCardDragStart(scope: Scope_onCardDragStart, e: DragEvent, _card: KanbanCard) {
  // Pointer path owns the mouse. Prevent native WK drag so it cannot
  // pointercancel the stream at ~4px. DnD handlers stay as a belt.
  e.preventDefault();
}

export function onCardDragEnd(scope: Scope_onCardDragEnd) {
  scope.dragCardId = null;
  scope.dropCol = null;
}

export function isCardDrag(scope: Scope_isCardDrag, e: DragEvent): boolean {
  return !!scope.dragCardId || !!e.dataTransfer?.types?.includes("text/plain");
}

export function onColDragOver(scope: Scope_onColDragOver, e: DragEvent, col: Status) {
  if (!scope.isCardDrag(e)) return;
  e.preventDefault();
  if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
  if (scope.dropCol !== col) scope.dropCol = col;
}

export function onColDragLeave(scope: Scope_onColDragLeave, e: DragEvent, col: Status) {
  const next = e.relatedTarget as Node | null;
  if (
    next &&
    e.currentTarget instanceof HTMLElement &&
    e.currentTarget.contains(next)
  )
    return;
  if (scope.dropCol === col) scope.dropCol = null;
}

export function onColDrop(scope: Scope_onColDrop, e: DragEvent, col: Status) {
  e.preventDefault();
  const id = e.dataTransfer?.getData("text/plain") || scope.dragCardId;
  const card = scope.kanbanCards.find((c) => c.id === id);
  if (card && col !== card.status) scope.moveCard(card, col);
  scope.dragCardId = null;
  scope.dropCol = null;
}

export function onCardPointerDown(scope: Scope_onCardPointerDown, e: PointerEvent, card: KanbanCard) {
  if (e.button !== 0 || scope.grabbedId) return;
  if (!(e.currentTarget instanceof HTMLElement)) return;
  scope.suppressClick = false;
  const row = e.currentTarget.closest(".b-row");
  const r = (row instanceof HTMLElement
    ? row
    : e.currentTarget
  ).getBoundingClientRect();
  try {
    e.currentTarget.setPointerCapture(e.pointerId);
  } catch {
    /* capture is best-effort */
  }
  scope.ptr = {
    id: card.id,
    x: e.clientX,
    y: e.clientY,
    held: false,
    ox: e.clientX - r.left,
    oy: e.clientY - r.top,
    w: r.width,
    h: r.height,
  };
  window.addEventListener("pointermove", scope.onPtrMove);
  window.addEventListener("pointerup", scope.onPtrUp);
  window.addEventListener("pointercancel", scope.onPtrCancel);
  window.addEventListener("keydown", scope.onPtrKey, true);
  window.addEventListener("blur", scope.onPtrCancel);
}

export function onPtrMove(scope: Scope_onPtrMove, e: PointerEvent) {
  const p = scope.ptr;
  if (!p) return;
  if (!p.held) {
    if (Math.hypot(e.clientX - p.x, e.clientY - p.y) < 2) return;
    const card = scope.kanbanCards.find((c) => c.id === p.id);
    if (!card) return scope.ptrEnd();
    scope.startGrab(card);
    p.held = true;
    scope.grabGhost = {
      x: e.clientX - p.ox,
      y: e.clientY - p.oy,
      w: p.w,
      h: p.h,
    };
  }
  if (!scope.grabbedId) return;
  const card = scope.kanbanCards.find((c) => c.id === scope.grabbedId);
  if (!card) return;
  const over = scope.overStartStrip(e.clientX, e.clientY)
    ? "doing"
    : scope.colAtPoint(e.clientX, e.clientY);
  scope.grabCol = over === "doing" || over === "done" ? over : card.status;
  scope.grabIdx = scope.rowAtPoint(scope.grabCol === card.status ? card.status : scope.grabCol, e.clientY);
  if (scope.grabGhost) {
    scope.grabGhost = {
      ...scope.grabGhost,
      x: e.clientX - p.ox,
      y: e.clientY - p.oy,
    };
  }
}

export function onPtrUp(scope: Scope_onPtrUp, e: PointerEvent) {
  const p = scope.ptr;
  if (!p) return;
  if (scope.overStartStrip(e.clientX, e.clientY)) scope.grabCol = "doing";
  const wasHeld = p.held;
  const stillHeld = !!scope.grabbedId;
  scope.ptrEnd();
  if (!wasHeld) return;
  scope.suppressClick = true;
  if (stillHeld) scope.dropGrab();
}

export function onPtrCancel(scope: Scope_onPtrCancel) {
  if (scope.ptr?.held && scope.grabbedId) scope.ungrab();
  scope.ptrEnd();
}

export function ptrEnd(scope: Scope_ptrEnd) {
  scope.ptr = null;
  window.removeEventListener("pointermove", scope.onPtrMove);
  window.removeEventListener("pointerup", scope.onPtrUp);
  window.removeEventListener("pointercancel", scope.onPtrCancel);
  window.removeEventListener("keydown", scope.onPtrKey, true);
  window.removeEventListener("blur", scope.onPtrCancel);
}

export function onPtrKey(scope: Scope_onPtrKey, e: KeyboardEvent) {
  if (e.key !== "Escape" || !scope.ptr?.held || !scope.grabbedId) return;
  scope.ungrab();
  e.preventDefault();
  e.stopPropagation();
}

export function overStartStrip(scope: Scope_overStartStrip, x: number, y: number): boolean {
  const strip = document.querySelector(".start-drop");
  if (!strip) return false;
  const r = strip.getBoundingClientRect();
  return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
}

export function colAtPoint(scope: Scope_colAtPoint, x: number, y: number): Status | null {
  const id = document
    .elementFromPoint(x, y)
    ?.closest("[data-col]")
    ?.getAttribute("data-col");
  return scope.COLUMNS.some((c) => c.id === id) ? (id as Status) : null;
}

export function rowAtPoint(scope: Scope_rowAtPoint, col: Status, y: number): number {
  const els = document.querySelectorAll(
    `.${col}-lane .b-card:not(.held)`,
  );
  let i = 0;
  for (const el of els) {
    const r = el.getBoundingClientRect();
    if (y > r.top + r.height / 2) i++;
  }
  return i;
}

export function onCardClick(scope: Scope_onCardClick, card: KanbanCard) {
  if (scope.suppressClick) {
    scope.suppressClick = false;
    return;
  }
  scope.openDetail(card);
}

export async function refreshWindows(scope: Scope_refreshWindows) {
  try {
    scope.windows = await scope.invoke<WindowInfo[]>("list_windows");
  } catch {
    // AX not granted here yet; window icons just stay generic
  }
}

export async function focusCard(scope: Scope_focusCard, card: KanbanCard) {
  const w = scope.winFor(card);
  if (!w) return;
  try {
    await scope.invoke("focus_window", { id: w.id });
  } catch {
    // app may have exited between polls; next refresh reflects reality
  }
}

export async function launchHermes(scope: Scope_launchHermes) {
  const p = scope.selectedProject;
  if (!p) return;
  const titles = scope.kanbanCards
    .filter(
      (c) =>
        c.kind !== "needed" &&
        c.kind !== "window" &&
        c.status !== "done" &&
        scope.listOf(c) === p.id,
    )
    .map((c) => c.title);
  try {
    const route = await scope.invoke<string>("launch_coordinator", {
      projectId: p.id,
      projectName: p.name,
      titles,
    });
    scope.hermesNote = { kind: "ok", text: route };
  } catch (e) {
    scope.hermesNote = { kind: "err", text: `${e}` };
  }
}
