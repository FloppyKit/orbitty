// @ts-nocheck
// Moved function bodies. scope fields are the lets each function already closed over.
export function raisePlanet(scope: any, id: string) {
  scope.invoke("bring_planet_front", { id }).catch(() => {});
  scope.invoke("locate_planet", { id }).catch(() => {});
}

export function projectIdList(scope: any): string[] {
  return scope.projects.map((p) => p.id);
}

export function applyFocusProject(scope: any, raw: string | null | undefined) {
  const id = (raw ?? "").trim();
  if (!id) return;
  const next = scope.resolveFocus({
    pending: id,
    selected: scope.selected,
    projectIds: scope.projectIdList(),
  });
  scope.pendingFocusProject = next.pending;
  scope.selected = next.selected;
  if (scope.projects.some((p) => p.id === id)) scope.raisePlanet(id);
}

export function applyPendingFocus(scope: any) {
  const before = scope.pendingFocusProject;
  const next = scope.resolveFocus({
    pending: before,
    selected: scope.selected,
    projectIds: scope.projectIdList(),
    projectsLoaded: scope.projects.length > 0,
  });
  scope.pendingFocusProject = next.pending;
  scope.selected = next.selected;
  if (
    before &&
    next.pending === null &&
    scope.projects.some((p) => p.id === before)
  ) {
    scope.raisePlanet(before);
  }
}

export function onPlanetTabClick(scope: any, id: string) {
  if (scope.planetDidDrag) {
    scope.planetDidDrag = false;
    return;
  }
  // A hand pick wins over an earlier board click.
  scope.pendingFocusProject = null;
  scope.selected = id;
  scope.raisePlanet(id);
}

export function reorderPlanetsLive(scope: any, fromId: string, beforeId: string | null) {
  const next = scope.reorderProjects(scope.projects, fromId, beforeId);
  if (next !== scope.projects) scope.projects = next;
}

export function persistPlanetOrder(scope: any) {
  const order = scope.projects.map((p) => p.id);
  scope.mutate((s) => {
    const byId = new Map((s.projects ?? []).map((p) => [p.id, p]));
    s.projects = order.map((id) => byId.get(id)).filter((p): p is Project => !!p);
  });
}

export function planetBeforeAt(scope: any, clientY: number): string | null {
  const col = document.querySelector(".planet-col");
  if (!col) return null;
  const rows: { id: string; top: number; height: number }[] = [];
  for (const el of col.querySelectorAll("[data-pid]")) {
    if (!(el instanceof HTMLElement)) continue;
    const id = el.getAttribute("data-pid");
    if (!id) continue;
    const r = el.getBoundingClientRect();
    rows.push({ id, top: r.top, height: r.height });
  }
  return scope.beforeIdAt(rows, clientY);
}

export function onPlanetPointerDown(scope: any, e: PointerEvent, id: string) {
  if (e.button !== 0) return;
  if (!(e.currentTarget instanceof HTMLElement)) return;
  e.stopPropagation();
  const tab = e.currentTarget;
  const scroller = tab.closest(".planet-col-planets");
  const x0 = e.clientX;
  const y0 = e.clientY;
  const scroll0 = scroller instanceof HTMLElement ? scroller.scrollTop : 0;
  let dragging = false;
  let armed = true;
  const SLOP = 16;
  const onMove = (ev: PointerEvent) => {
    if (!armed) return;
    const dx = ev.clientX - x0;
    const dy = ev.clientY - y0;
    if (!dragging) {
      if (Math.hypot(dx, dy) < SLOP) return;
      if (Math.abs(dx) > Math.abs(dy)) {
        armed = false;
        cleanup();
        return;
      }
      if (
        scroller instanceof HTMLElement &&
        Math.abs(scroller.scrollTop - scroll0) > 2
      ) {
        armed = false;
        cleanup();
        return;
      }
      dragging = true;
      scope.planetDidDrag = true;
      scope.planetDragId = id;
    }
    const before = scope.planetBeforeAt(ev.clientY);
    scope.planetDropId = before;
    scope.reorderPlanetsLive(id, before);
  };
  const onUp = () => {
    armed = false;
    if (dragging) scope.persistPlanetOrder();
    scope.planetDragId = null;
    scope.planetDropId = null;
    cleanup();
  };
  const cleanup = () => {
    try {
      tab.releasePointerCapture(e.pointerId);
    } catch {
      /* capture is best-effort */
    }
    tab.removeEventListener("pointermove", onMove);
    tab.removeEventListener("pointerup", onUp);
    tab.removeEventListener("pointercancel", onUp);
  };
  try {
    tab.setPointerCapture(e.pointerId);
  } catch {
    /* capture is best-effort */
  }
  tab.addEventListener("pointermove", onMove);
  tab.addEventListener("pointerup", onUp);
  tab.addEventListener("pointercancel", onUp);
}

export function toggleDoneChip(scope: any) {
  if (scope.selected === "done") {
    scope.selected = scope.selectedBeforeDone || "inbox";
  } else {
    scope.selectedBeforeDone = scope.selected;
    scope.selected = "done";
  }
}

export function onPlanetTabDblClick(scope: any, id: string) {
  const collapsed = scope.collapsedSectionIds.includes(id);
  scope.emitTo("main", collapsed ? "habitat:expand" : "habitat:collapse", id).catch(
    () => {},
  );
}
