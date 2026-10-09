// Moved function bodies. scope fields are the lets each function already closed over.

import type { KanbanCard, Project, WindowInfo, WindowTags } from "../persist";
import type { MenuRing, RingStop } from "../menuKeys.svelte";
import type { ArchArrowResult, ArchConfirmAction, ArchState, BoardFocus, Layer2Item } from "./stageArch";
import type { BesidePick, KeyOwner } from "./stageFocus";
import type { HabitatConfirm, RailEdge } from "./habitat";
import type { ArchFrontName, CentreCause } from "./railInput";
import type { SpinConfirmAction, SpinItem, SpinStop } from "./stageLook";

type Invoke = <T = unknown>(cmd: string, args?: any) => Promise<T>;

type Scope_archDo = {
  arch: ArchState;
  applyBeside: (...args: any[]) => any;
  UNMAPPED_KEY: string;
};

type Scope_planetCards = {
  kanbanCards: KanbanCard[];
};

type Scope_layer2For = {
  UNMAPPED_KEY: string;
  mirrorItems: (...args: any[]) => Layer2Item[];
  planetCards: (...args: any[]) => KanbanCard[];
  rowsFor: (...args: any[]) => WindowInfo[];
  titleFor: (...args: any[]) => string;
};

type Scope_focusMirrorBoard = {};

type Scope_confirmLayer2Stop = {
  layer2For: (...args: any[]) => Layer2Item[];
  arch: ArchState;
  armNeededMouth: (...args: any[]) => any;
  enterBoard: (...args: any[]) => { state: ArchState; focus: BoardFocus } | null;
  planetCards: (...args: any[]) => KanbanCard[];
  archDo: (...args: any[]) => any;
  walkInto: (...args: any[]) => any;
  focusBoardLanding: (...args: any[]) => any;
  focusMirrorBoard: (...args: any[]) => any;
  archConfirm: (...args: any[]) => ArchConfirmAction;
  rowsFor: (...args: any[]) => WindowInfo[];
  wakeSleeper: (...args: any[]) => Promise<any>;
  parkAfterRaise: (...args: any[]) => any;
  restore: (...args: any[]) => Promise<any>;
  focusRow: (...args: any[]) => Promise<any>;
};

type Scope_noteRailAct = {
  embedded: boolean;
  lastNoteAct: number;
  invoke: Invoke;
};

type Scope_confirmHabitatStop = {
  spinConfirm: (...args: any[]) => SpinConfirmAction | null;
  openStageDest: (...args: any[]) => any;
  creatingProject: boolean;
  coachClaim: (...args: any[]) => Promise<any>;
  coachSkip: (...args: any[]) => Promise<any>;
  applyBeside: (...args: any[]) => any;
  spinItems: (...args: any[]) => SpinItem[];
  projects: Project[];
  UNMAPPED_KEY: string;
  archDo: (...args: any[]) => any;
  archEnter: (...args: any[]) => ArchState;
  arch: ArchState;
  layer2For: (...args: any[]) => Layer2Item[];
  raisePlanetWindows: (...args: any[]) => Promise<any>;
  habitatConfirm: (...args: any[]) => HabitatConfirm | null;
  toggleBoardCol: (...args: any[]) => any;
  ensureExpand: (...args: any[]) => any;
  rowById: (...args: any[]) => WindowInfo | undefined;
  wakeSleeper: (...args: any[]) => Promise<any>;
  parkAfterRaise: (...args: any[]) => any;
  focusRow: (...args: any[]) => Promise<any>;
  openKanbanFocus: (...args: any[]) => Promise<any>;
  openPlanetTodos: (...args: any[]) => Promise<any>;
};

type Scope_takeReselect = {
  reselectStop: string | null;
  menuRing: MenuRing;
  focusMkey: (...args: any[]) => any;
};

type Scope_syncInspectorRing = {
  lane: "projects" | "agents";
  menuRing: MenuRing;
  embedded: boolean;
  keyOwner: KeyOwner;
  arch: ArchState;
  UNMAPPED_KEY: string;
  visibleRows: (...args: any[]) => WindowInfo[];
  unmapped: WindowInfo[];
  outwardDir: (...args: any[]) => "left" | "right" | "up" | "down";
  railEdge: RailEdge;
  walkOut: (...args: any[]) => any;
  wakeSleeper: (...args: any[]) => Promise<any>;
  locateRow: (...args: any[]) => Promise<any>;
  focusMkey: (...args: any[]) => any;
  spinItems: (...args: any[]) => SpinItem[];
  projects: Project[];
  coachIsDue: (...args: any[]) => boolean;
  spinCenter: number;
  layer2For: (...args: any[]) => Layer2Item[];
  archFocusPid: (...args: any[]) => string | null;
  archArrow: (...args: any[]) => ArchArrowResult | null;
  planetCards: (...args: any[]) => KanbanCard[];
  armNeededMouth: (...args: any[]) => any;
  archDo: (...args: any[]) => any;
  walkInto: (...args: any[]) => any;
  focusBoardLanding: (...args: any[]) => any;
  applyBeside: (...args: any[]) => any;
  pickFromKind: (...args: any[]) => BesidePick;
  confirmLayer2Stop: (...args: any[]) => any;
  spinStops: (...args: any[]) => SpinStop[];
  confirmHabitatStop: (...args: any[]) => any;
  takeReselect: (...args: any[]) => any;
};

type Scope_confirmRingTab = {
  creatingProject: boolean;
  sheetSlot: string;
  onSheetPlanetClick: (...args: any[]) => any;
};

type Scope_onRailEscape = {
  claimPickerFor: string | null;
  creatingProject: boolean;
  keyOwner: KeyOwner;
  walkOut: (...args: any[]) => any;
  stageEmbed: null | "settings" | "stars";
  closeStageEmbed: (...args: any[]) => any;
  embedded: boolean;
  arch: ArchState;
  layer2For: (...args: any[]) => Layer2Item[];
  depthOutStop: (...args: any[]) => string | null;
  reselectStop: string | null;
  centreCause: CentreCause;
  depthOut: (...args: any[]) => { front: ArchFrontName; hide: boolean };
  archDo: (...args: any[]) => any;
  menuRing: MenuRing;
  coachSkip: (...args: any[]) => Promise<any>;
  previewStop: (...args: any[]) => boolean;
  openSectionId: (...args: any[]) => string | null;
  stageRevealed: boolean;
  parkStage: (...args: any[]) => Promise<any>;
};

type Scope_reclaimRailRing = {
  detachMenuKeys: (() => void) | undefined;
  menuRing: MenuRing;
  onRailEscape: (...args: any[]) => boolean;
};

type Scope_toggleTag = {
  liveWindowTags: WindowTags[];
  windowTags: WindowTags[];
  persist: (...args: any[]) => Promise<any>;
};

export function archDo(scope: Scope_archDo, next: ArchState) {
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

export function planetCards(scope: Scope_planetCards, pid: string): KanbanCard[] {
  return scope.kanbanCards.filter((c) => c.projectId === pid);
}

export function layer2For(scope: Scope_layer2For, pid: string | null): Layer2Item[] {
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

export function focusMirrorBoard(scope: Scope_focusMirrorBoard) {
  requestAnimationFrame(() => {
    const panel = document.querySelector(".l2-expanded .l2-board");
    if (!(panel instanceof HTMLElement)) return;
    const el = panel.querySelector<HTMLElement>(
      "input, textarea, select, button, a[href], [tabindex]:not([tabindex='-1'])",
    );
    (el ?? panel).focus();
  });
}

export function confirmLayer2Stop(scope: Scope_confirmLayer2Stop, id: string) {
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

export function noteRailAct(scope: Scope_noteRailAct) {
  if (scope.embedded) return;
  const now = Date.now();
  if (now - scope.lastNoteAct < 1000) return;
  scope.lastNoteAct = now;
  scope.invoke("starter_note_act").catch(() => {});
}

export function confirmHabitatStop(scope: Scope_confirmHabitatStop, id: string) {
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

export function takeReselect(scope: Scope_takeReselect) {
  if (!scope.reselectStop) return;
  const id = scope.reselectStop;
  scope.reselectStop = null;
  scope.menuRing.select(id);
  scope.focusMkey(id);
}

export function syncInspectorRing(scope: Scope_syncInspectorRing) {
  if (scope.lane !== "projects") {
    scope.menuRing.setStops([]);
    scope.menuRing.onEdge = null;
    return;
  }
  if (
    !scope.embedded &&
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

export function confirmRingTab(scope: Scope_confirmRingTab, id: string) {
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

export function onRailEscape(scope: Scope_onRailEscape): boolean {
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
  if (!scope.embedded && scope.arch.front === "layer2") {
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
  if (!scope.embedded && scope.arch.front !== "arch") {
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
  if (!scope.embedded && scope.previewStop(scope.menuRing.activeId)) {
    const pid = scope.openSectionId();
    if (pid) {
      scope.menuRing.select(`rhead-${pid}`);
      return true;
    }
  }
  if (!scope.embedded && scope.stageRevealed && scope.depthOut(scope.arch.front).hide) {
    void scope.parkStage();
    return true;
  }
  return false;
}

export function reclaimRailRing(scope: Scope_reclaimRailRing) {
  scope.detachMenuKeys?.();
  scope.detachMenuKeys = scope.menuRing.attach(scope.onRailEscape);
}

export function toggleTag(scope: Scope_toggleTag, windowId: string, tagId: string) {
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
