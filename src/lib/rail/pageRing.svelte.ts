// @ts-nocheck
// Moved function bodies. scope fields are the lets each function already closed over.
export function archDo(scope: any, next: ArchState) {
  const pendingSame =
    (next.pending == null && scope.arch.pending == null) ||
    (!!next.pending &&
      !!scope.arch.pending &&
      next.pending.zone === scope.arch.pending.zone &&
      next.pending.since === scope.arch.pending.since);
  if (
    next.front === scope.arch.front &&
    next.pid === scope.arch.pid &&
    next.center2 === scope.arch.center2 &&
    pendingSame
  ) {
    return;
  }
  const prevPid = scope.arch.pid;
  scope.arch = next;
  if (next.pid !== prevPid) {
    scope.applyBeside(next.pid === scope.UNMAPPED_KEY ? "unmapped" : "planet");
  }
}

export function planetCards(scope: any, pid: string): KanbanCard[] {
  return scope.kanbanCards.filter((c) => c.projectId === pid);
}

export function layer2For(scope: any, pid: string | null): Layer2Item[] {
  if (!pid || pid === scope.UNMAPPED_KEY) return [];
  return scope.mirrorItems({
    pid,
    cards: scope.planetCards(pid),
    rows: scope.rowsFor(pid).map((w) => ({
      id: w.id,
      title: scope.titleFor(w),
      sleeping: !!w.sleeping,
      isMinimized: !!w.isMinimized,
      iconBase64: w.iconBase64,
      appName: w.appName,
      isFrontmost: !!w.isFrontmost,
    })),
  });
}

export function focusMirrorBoard(scope: any) {
  requestAnimationFrame(() => {
    const panel = document.querySelector(".l2-expanded .l2-board");
    if (!(panel instanceof HTMLElement)) return;
    const el = panel.querySelector<HTMLElement>(
      "input, textarea, select, button, a[href], [tabindex]:not([tabindex='-1'])",
    );
    (el ?? panel).focus();
  });
}

export function confirmLayer2Stop(scope: any, id: string) {
  const item = scope.layer2For(scope.arch.pid).find((it) => it.stop === id);
  if (!item || !scope.arch.pid) return;
  if (item.kind === "needed") {
    scope.armNeededMouth(item);
    return;
  }
  if (item.kind === "board") {
    const land = scope.enterBoard(scope.arch, scope.planetCards(scope.arch.pid));
    if (land) {
      scope.archDo(land.state);
      scope.walkInto("board");
      scope.focusBoardLanding(land.focus);
      return;
    }
    scope.focusMirrorBoard();
    return;
  }
  if (item.kind !== "window") return;
  const action = scope.archConfirm(item);
  const w = scope.rowsFor(scope.arch.pid).find((row) => row.id === item.id);
  if (!w) return;
  if (action === "wake") void scope.wakeSleeper(w).then(() => scope.parkAfterRaise());
  else if (action === "restore") void scope.restore(w).then(() => scope.focusRow(w)).then(() => scope.parkAfterRaise());
  else void scope.focusRow(w).then(() => scope.parkAfterRaise());
}

export function noteRailAct(scope: any) {
  if (embedded) return;
  const now = Date.now();
  if (now - scope.lastNoteAct < 1000) return;
  scope.lastNoteAct = now;
  scope.invoke("starter_note_act").catch(() => {});
}

export function confirmHabitatStop(scope: any, id: string) {
  const spin = scope.spinConfirm(id);
  if (spin === "stars") {
    scope.openStageDest("stars");
    return;
  }
  if (spin === "settings") {
    scope.openStageDest("settings");
    return;
  }
  if (spin === "newProject") {
    scope.creatingProject = true;
    return;
  }
  if (spin === "coachClaim") {
    void scope.coachClaim();
    return;
  }
  if (spin === "coachSkip") {
    void scope.coachSkip();
    return;
  }
  if (spin === "centre") {
    scope.applyBeside("unmapped");
    const item = scope.spinItems(scope.projects.map((p) => p.id), scope.UNMAPPED_KEY).find((it) => it.stop === id);
    if (item) scope.archDo(scope.archEnter(scope.arch, item, scope.layer2For(item.pid ?? null)));
    return;
  }
  if (spin === "raise") {
    scope.applyBeside("planet");
    void scope.raisePlanetWindows(id.slice("rhead-".length));
    return;
  }
  const action = scope.habitatConfirm(id);
  if (action === "toggleCol") {
    scope.toggleBoardCol(id.slice("rcol-".length));
    return;
  }
  if (action === "stars") {
    scope.openStageDest("stars");
    return;
  }
  if (action === "settings") {
    scope.openStageDest("settings");
    return;
  }
  if (action === "expand") {
    scope.ensureExpand(id.slice("rhead-".length));
    return;
  }
  if (action === "raise") {
    void scope.raisePlanetWindows(id.slice("rraise-".length));
    return;
  }
  if (action === "focusTile") {
    const w = scope.rowById(id.slice("rtile-".length));
    if (!w) return;
    if (w.sleeping) void scope.wakeSleeper(w).then(() => scope.parkAfterRaise());
    else void scope.focusRow(w).then(() => scope.parkAfterRaise());
    return;
  }
  if (action === "openSnap") {
    void scope.openKanbanFocus(id.slice("rsnap-".length));
    return;
  }
  if (action === "todos") void scope.openPlanetTodos(id.slice("rtodos-".length));
}

export function takeReselect(scope: any) {
  if (!scope.reselectStop) return;
  const id = scope.reselectStop;
  scope.reselectStop = null;
  scope.menuRing.select(id);
  scope.focusMkey(id);
}

export function syncInspectorRing(scope: any) {
  if (scope.lane !== "projects") {
    scope.menuRing.setStops([]);
    scope.menuRing.onEdge = null;
    return;
  }
  if (
    !embedded &&
    scope.keyOwner === "unmapped" &&
    scope.arch.front !== "arch" &&
    scope.arch.pid === scope.UNMAPPED_KEY
  ) {
    const rows = scope.visibleRows(scope.unmapped);
    const outward = scope.outwardDir(scope.railEdge);
    scope.menuRing.onEdge = (dir) => {
      if (dir !== outward) return false;
      scope.walkOut();
      return true;
    };
    const stops: RingStop[] = rows.map((w) => ({
      id: `umap-${w.id}`,
      group: "unmapped",
      confirm: () => {
        if (w.sleeping) void scope.wakeSleeper(w);
        else void scope.locateRow(w);
      },
    }));
    scope.menuRing.setStops(stops);
    const active = scope.menuRing.activeId;
    if (stops.length && (!active || !stops.some((s) => s.id === active))) {
      scope.menuRing.select(stops[0].id);
      scope.focusMkey(stops[0].id);
    }
    return;
  }
  scope.menuRing.onEdge = null;
  const items = scope.spinItems(scope.projects.map((p) => p.id), scope.UNMAPPED_KEY);
  const due = scope.coachIsDue();
  const cIdx = Math.min(Math.max(scope.spinCenter, 0), Math.max(items.length - 1, 0));
  const l2 = scope.layer2For(scope.archFocusPid(scope.arch, items[cIdx]));
  const nested = (scope.arch.front === "layer2" || scope.arch.front === "board") && l2.length > 0;
  const inwardKey = (dir: string) => {
    if (scope.railEdge === "right") return dir === "left";
    if (scope.railEdge === "top") return dir === "down";
    if (scope.railEdge === "bottom") return dir === "up";
    return dir === "right";
  };
  const applyArrow = (activeId: string, dir: string) => {
    const before = scope.arch;
    const result = scope.archArrow(
      before,
      items,
      scope.spinCenter,
      dir,
      scope.railEdge,
      l2,
      activeId,
      due,
      before.pid ? scope.planetCards(before.pid) : [],
    );
    if (!result) {
      const cur = l2[before.center2];
      if (before.front === "layer2" && cur?.kind === "needed" && inwardKey(dir)) scope.armNeededMouth(cur);
      return;
    }
    if (result.walkIn) {
      scope.archDo(result.state);
      scope.walkInto(result.walkIn);
      if (result.walkIn === "board" && result.focus) scope.focusBoardLanding(result.focus);
      return;
    }
    scope.archDo(result.state);
    if (result.stop) {
      scope.menuRing.select(result.stop);
      const landed = items.find((it) => it.stop === result.stop);
      scope.applyBeside(landed ? scope.pickFromKind(landed.kind) : "planet");
    }
  };
  const stops: RingStop[] = nested
    ? l2.map((item) => ({
        id: item.stop,
        group: "layer2",
        confirm: () => scope.confirmLayer2Stop(item.stop),
        arrow: (dir) => {
          applyArrow(item.stop, dir);
          return true;
        },
      }))
    : scope.spinStops(items, due).map((stop) => ({
        id: stop.id,
        group: stop.group,
        confirm: () => scope.confirmHabitatStop(stop.id),
        arrow: (dir) => {
          applyArrow(stop.id, dir);
          return true;
        },
      }));
  scope.menuRing.setStops(stops);
  if (nested) {
    const active = scope.menuRing.activeId;
    const onL2 = !!active && l2.some((it) => it.stop === active);
    if (!onL2) {
      const want = l2[Math.min(Math.max(scope.arch.center2, 0), l2.length - 1)];
      if (want) scope.menuRing.select(want.stop);
    }
  } else if (scope.arch.front === "arch") {
    const active = scope.menuRing.activeId;
    const onWheel =
      !!active &&
      (items.some((it) => it.stop === active) ||
        active === "rcoach-claim" ||
        active === "rcoach-skip");
    if (active && !onWheel) {
      const back =
        items.find((it) => it.pid != null && it.pid === scope.arch.pid) ??
        items[Math.min(Math.max(scope.spinCenter, 0), Math.max(items.length - 1, 0))];
      if (back) scope.menuRing.select(back.stop);
    }
  }
  scope.takeReselect();
}

export function confirmRingTab(scope: any, id: string) {
  if (id === "ptab-add") {
    scope.creatingProject = true;
    return;
  }
  if (id === "ptab-unclaimed") {
    scope.sheetSlot = "unclaimed";
    return;
  }
  scope.onSheetPlanetClick(id.slice("ptab-".length));
}

export function onRailEscape(scope: any): boolean {
  if (scope.claimPickerFor) {
    scope.claimPickerFor = null;
    return true;
  }
  if (scope.creatingProject) {
    scope.creatingProject = false;
    return true;
  }
  if (scope.keyOwner === "unmapped") {
    scope.walkOut();
    return true;
  }
  if (scope.stageEmbed) {
    scope.closeStageEmbed();
    return true;
  }
  if (!embedded && scope.arch.front === "layer2") {
    const active = document.activeElement;
    if (active instanceof Element && active.closest(".l2-expanded")) {
      const stop = scope.layer2For(scope.arch.pid)[scope.arch.center2]?.stop;
      const icon = stop
        ? document.querySelector(`[data-mkey="${CSS.escape(stop)}"]`)
        : null;
      if (icon instanceof HTMLElement) icon.focus();
      return true;
    }
  }
  if (!embedded && scope.arch.front !== "arch") {
    if (scope.arch.front === "layer2") {
      const backStop = scope.depthOutStop(scope.arch.pid);
      if (backStop) scope.reselectStop = backStop;
      scope.centreCause = "depthOut";
    }
    const step = scope.depthOut(scope.arch.front);
    scope.archDo({ ...scope.arch, front: step.front, pending: null });
    return true;
  }
  if (
    scope.menuRing.activeId === "rcoach-claim" ||
    scope.menuRing.activeId === "rcoach-skip"
  ) {
    void scope.coachSkip();
    return true;
  }
  if (!embedded && scope.previewStop(scope.menuRing.activeId)) {
    const pid = scope.openSectionId();
    if (pid) {
      scope.menuRing.select(`rhead-${pid}`);
      return true;
    }
  }
  if (!embedded && scope.stageRevealed && scope.depthOut(scope.arch.front).hide) {
    void scope.parkStage();
    return true;
  }
  return false;
}

export function reclaimRailRing(scope: any) {
  scope.detachMenuKeys?.();
  scope.detachMenuKeys = scope.menuRing.attach(scope.onRailEscape);
}

export function toggleTag(scope: any, windowId: string, tagId: string) {
  const cur =
    scope.liveWindowTags.find((wt) => wt.windowId === windowId)?.tagIds ?? [];
  const next = cur.includes(tagId)
    ? cur.filter((id) => id !== tagId)
    : [...cur, tagId];
  scope.windowTags = [
    ...scope.liveWindowTags.filter((wt) => wt.windowId !== windowId),
    ...(next.length ? [{ windowId, tagIds: next }] : []),
  ];
  scope.persist();
}
