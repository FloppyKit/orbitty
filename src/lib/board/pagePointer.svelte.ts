// @ts-nocheck
// Moved function bodies. scope fields are the lets each function already closed over.
export function onCardDragStart(scope: any, e: DragEvent, _card: KanbanCard) {
  // Pointer path owns the mouse. Prevent native WK drag so it cannot
  // pointercancel the stream at ~4px. DnD handlers stay as a belt.
  e.preventDefault();
}

export function onCardDragEnd(scope: any) {
  scope.dragCardId = null;
  scope.dropCol = null;
}

export function isCardDrag(scope: any, e: DragEvent): boolean {
  return !!scope.dragCardId || !!e.dataTransfer?.types?.includes("text/plain");
}

export function onColDragOver(scope: any, e: DragEvent, col: Status) {
  if (!scope.isCardDrag(e)) return;
  e.preventDefault();
  if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
  if (scope.dropCol !== col) scope.dropCol = col;
}

export function onColDragLeave(scope: any, e: DragEvent, col: Status) {
  const next = e.relatedTarget as Node | null;
  if (
    next &&
    e.currentTarget instanceof HTMLElement &&
    e.currentTarget.contains(next)
  )
    return;
  if (scope.dropCol === col) scope.dropCol = null;
}

export function onColDrop(scope: any, e: DragEvent, col: Status) {
  e.preventDefault();
  const id = e.dataTransfer?.getData("text/plain") || scope.dragCardId;
  const card = scope.kanbanCards.find((c) => c.id === id);
  if (card && col !== card.status) scope.moveCard(card, col);
  scope.dragCardId = null;
  scope.dropCol = null;
}

export function onCardPointerDown(scope: any, e: PointerEvent, card: KanbanCard) {
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

export function onPtrMove(scope: any, e: PointerEvent) {
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

export function onPtrUp(scope: any, e: PointerEvent) {
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

export function onPtrCancel(scope: any) {
  if (scope.ptr?.held && scope.grabbedId) scope.ungrab();
  scope.ptrEnd();
}

export function ptrEnd(scope: any) {
  scope.ptr = null;
  window.removeEventListener("pointermove", scope.onPtrMove);
  window.removeEventListener("pointerup", scope.onPtrUp);
  window.removeEventListener("pointercancel", scope.onPtrCancel);
  window.removeEventListener("keydown", scope.onPtrKey, true);
  window.removeEventListener("blur", scope.onPtrCancel);
}

export function onPtrKey(scope: any, e: KeyboardEvent) {
  if (e.key !== "Escape" || !scope.ptr?.held || !scope.grabbedId) return;
  scope.ungrab();
  e.preventDefault();
  e.stopPropagation();
}

export function overStartStrip(scope: any, x: number, y: number): boolean {
  const strip = document.querySelector(".start-drop");
  if (!strip) return false;
  const r = strip.getBoundingClientRect();
  return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
}

export function colAtPoint(scope: any, x: number, y: number): Status | null {
  const id = document
    .elementFromPoint(x, y)
    ?.closest("[data-col]")
    ?.getAttribute("data-col");
  return scope.COLUMNS.some((c) => c.id === id) ? (id as Status) : null;
}

export function rowAtPoint(scope: any, col: Status, y: number): number {
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

export function onCardClick(scope: any, card: KanbanCard) {
  if (scope.suppressClick) {
    scope.suppressClick = false;
    return;
  }
  scope.openDetail(card);
}

export async function refreshWindows(scope: any) {
  try {
    scope.windows = await scope.invoke<WindowInfo[]>("list_windows");
  } catch {
    // AX not granted here yet; window icons just stay generic
  }
}

export async function focusCard(scope: any, card: KanbanCard) {
  const w = scope.winFor(card);
  if (!w) return;
  try {
    await scope.invoke("focus_window", { id: w.id });
  } catch {
    // app may have exited between polls; next refresh reflects reality
  }
}

export async function launchHermes(scope: any) {
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
