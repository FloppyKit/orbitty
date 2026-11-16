// Moved function bodies. scope fields are the lets each function already closed over.

import type { Claim, KanbanCard, PersistedState, Project, WindowInfo } from "../persist";
import type { MenuRing } from "../menuKeys.svelte";
import type { ArchFrame, ArchFront, ArchLayout, ArchState, BoardFocus, Layer2Item } from "./stageArch";
import type { ClaimSnap, EdgeHoverEvent, EdgeHoverState } from "./stagePlace";
import type {
  BoardCol,
  HabitatRow,
  RailEdge,
  SceneSavedFrame,
  SceneSession,
  SceneWindowRow,
  StagePrefs,
} from "./habitat";
import { armSceneHold, sceneEnter, sceneLeave } from "./habitat";
import type { DismissInput, InputIntent, IntentInput } from "./railInput";

type Invoke = <T = unknown>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

type Scope_overPainted = {
  archPainted: (layout: ArchLayout, point: { x: number; y: number }, opts: { front: ArchFront; pid: string | null; revealed: boolean; slop?: number; embed?: "settings" | "stars" | null; needsPanel?: boolean; edge?: string; frame?: ArchFrame; }) => boolean;
  archLayout: (edge: string, frameW: number, frameH: number) => ArchLayout;
  railEdge: RailEdge;
  arch: ArchState;
  stageRevealed: boolean;
  stageEmbed: null | "settings" | "stars";
  level1NeedsOpen: boolean;
};

type Scope_armEdgeTimer = {
  edgeTimer: ReturnType<typeof setTimeout> | null;
  edgeHoverState: EdgeHoverState;
  EDGE_HIDE_MS: number;
  EDGE_GRACE_MS: number;
  applyEdge: (ev: EdgeHoverEvent, now?: number) => unknown;
};

type Scope_applyEdge = {
  edgeHover: (s: EdgeHoverState, ev: EdgeHoverEvent, now: number) => { state: EdgeHoverState; act: "reveal" | "park" | null };
  edgeHoverState: EdgeHoverState;
  showStage: () => Promise<unknown>;
  maybeAutoHide: () => unknown;
  armEdgeTimer: () => unknown;
};

type Scope_noteClaims = {
  lastClaimsSeen: ClaimSnap[] | null;
  claimLanded: (prev: readonly ClaimSnap[] | null, next: readonly ClaimSnap[]) => { windowId: string; projectId: string } | null;
  windows: WindowInfo[];
  titleFor: (w: WindowInfo) => string;
  claimFlash: { pid: string; title: string; until: number } | null;
  CLAIM_FLASH_MS: number;
  claimFlashTimer: ReturnType<typeof setTimeout> | null;
  invoke: Invoke;
};

type Scope_onStagePointerMove = {
  embedded: boolean;
  edgePointerSeed: boolean;
  overPainted: (e: PointerEvent) => boolean;
  pointerInside: boolean;
  stageRevealed: boolean;
  applyEdge: (ev: EdgeHoverEvent, now?: number) => unknown;
};

type Scope_noteWheelSpin = {
  wheelSpinUntil: number;
};

type Scope_wheelAnimating = {
  wheelSpinUntil: number;
};

type Scope_nearHit = {};

type Scope_focusBoardLanding = {
  menuRing: MenuRing;
};

type Scope_takeMouseButton = {
  mouseButtonAt: number;
  inputIntent: (ev: IntentInput) => InputIntent;
  onRailEscape: () => boolean;
  confirmActive: () => boolean;
  textFieldFocused: () => boolean;
};

type Scope_onStagePointerDown = {
  embedded: boolean;
  stageRevealed: boolean;
  pointerDownAt: { x: number; y: number } | null;
  pointerInside: boolean;
  nearHit: (e: { clientX: number; clientY: number; target: EventTarget | null }) => boolean;
  overPainted: (e: PointerEvent) => boolean;
};

type Scope_onStagePointerUp = {
  embedded: boolean;
  stageRevealed: boolean;
  takeMouseButton: (button: number) => boolean;
  pointerDownAt: { x: number; y: number } | null;
  nearHit: (e: { clientX: number; clientY: number; target: EventTarget | null }) => boolean;
  wheelAnimating: () => boolean;
  spinClickTarget: (animating: boolean, onHit: boolean) => "landing" | "item";
  menuRing: MenuRing;
  arch: ArchState;
  confirmLayer2Stop: (id: string) => unknown;
  confirmHabitatStop: (id: string) => unknown;
  emptyDismiss: (input: DismissInput) => boolean;
  onRailEscape: () => boolean;
};

type Scope_onStageAux = {
  embedded: boolean;
  stageRevealed: boolean;
  takeMouseButton: (button: number) => boolean;
};

type Scope_onStageMouseLeave = {
  embedded: boolean;
  edgePointerSeed: boolean;
  edgeLeaveSide: (edge: string, clientX: number, clientY: number, w: number, h: number) => "edge" | "away";
  railEdge: RailEdge;
  pointerInside: boolean;
  applyEdge: (ev: EdgeHoverEvent, now?: number) => unknown;
};

type Scope_onWinBlur = {
  stageFocus: (inside: boolean) => unknown;
};

type Scope_onExpandedFieldEscape = {
  embedded: boolean;
  arch: ArchState;
  layer2For: (pid: string | null) => Layer2Item[];
};

type Scope_applyStagePrefs = {
  stagePrefs: (raw: { railEdge?: unknown; railAutoHideOnBlur?: unknown; autohideRowChrome?: unknown; } | null | undefined) => StagePrefs;
  autohideRowChrome: boolean;
  railEdge: RailEdge;
  railAutoHideOnBlur: boolean;
};

type Scope_sheetRowsFor = {
  windows: WindowInfo[];
  liveClaims: Claim[];
};

type Scope_raisePlanet = {
  invoke: Invoke;
  emitTo: (target: string, event: string, payload?: unknown) => Promise<void>;
  planetsHidden: boolean;
  isCollapsed: (id: string) => boolean;
};

type Scope_onSheetPlanetClick = {
  sheetSlot: string;
  raisePlanet: (id: string) => unknown;
};

type Scope_onSheetPlanetDblClick = {
  isCollapsed: (sectionId: string) => boolean;
  emitTo: (target: string, event: string, payload?: unknown) => Promise<void>;
};

type Scope_sheetClaim = {
  claimPickerFor: string | null;
  claim: (windowId: string, projectId: string) => unknown;
};

type Scope_sheetFocus = {
  invoke: Invoke;
};

type Scope_toggleBoardCol = {
  railBoardCols: BoardCol[];
  toggleCol: (cols: readonly string[], col: string) => BoardCol[];
  persist: () => Promise<unknown>;
};

type Scope_focusRow = {
  invoke: Invoke;
  touchClaim: (windowId: string, force?: boolean) => unknown;
  tick: () => Promise<unknown>;
};

type Scope_raisePlanetWindows = {
  raisePlan: (rows: readonly HabitatRow[], cap?: number) => { restore: string[]; focus: string[] };
  rowsFor: (projectId: string) => WindowInfo[];
  invoke: Invoke;
  raisePlanet: (id: string) => unknown;
  tick: () => Promise<unknown>;
  parkAfterRaise: () => unknown;
};

type Scope_openPlanetTodos = {
  openKanban: () => Promise<unknown>;
  emitTo: (target: string, event: string, payload?: unknown) => Promise<void>;
};

type Scope_rowById = {
  projects: Project[];
  rowsFor: (projectId: string) => WindowInfo[];
  windows: WindowInfo[];
};

export function overPainted(scope: Scope_overPainted, e: PointerEvent): boolean {
  const host = e.currentTarget;
  if (!(host instanceof HTMLElement)) return false;
  const rect = host.getBoundingClientRect();
  return scope.archPainted(
    scope.archLayout(scope.railEdge, rect.width, rect.height),
    { x: e.clientX - rect.left, y: e.clientY - rect.top },
    {
      front: scope.arch.front,
      pid: scope.arch.pid,
      revealed: scope.stageRevealed,
      embed: scope.stageEmbed,
      needsPanel: scope.level1NeedsOpen,
      edge: scope.railEdge,
      frame: { w: rect.width, h: rect.height },
    },
  );
}

export function armEdgeTimer(scope: Scope_armEdgeTimer) {
  if (scope.edgeTimer) clearTimeout(scope.edgeTimer);
  scope.edgeTimer = null;
  if (!scope.edgeHoverState.revealed || scope.edgeHoverState.outsideSince == null) return;
  const due = Math.max(
    scope.edgeHoverState.outsideSince + scope.EDGE_HIDE_MS,
    scope.edgeHoverState.revealedAt + scope.EDGE_GRACE_MS,
  );
  const wait = Math.max(16, due - Date.now() + 8);
  scope.edgeTimer = setTimeout(() => {
    scope.edgeTimer = null;
    scope.applyEdge({ kind: "tick" });
  }, wait);
}

export function applyEdge(scope: Scope_applyEdge, ev: EdgeHoverEvent, now = Date.now()) {
  const out = scope.edgeHover(scope.edgeHoverState, ev, now);
  scope.edgeHoverState = out.state;
  if (out.act === "reveal") void scope.showStage();
  else if (out.act === "park") scope.maybeAutoHide();
  scope.armEdgeTimer();
}

export function noteClaims(scope: Scope_noteClaims, next: Claim[]) {
  const snap = next.map((c) => ({
    windowId: c.windowId,
    projectId: c.projectId,
    lastTouchedAt: c.lastTouchedAt,
  }));
  const prev = scope.lastClaimsSeen;
  scope.lastClaimsSeen = snap;
  if (prev === null) return;
  const hit = scope.claimLanded(prev, snap);
  if (!hit) return;
  const w = scope.windows.find((x) => x.id === hit.windowId);
  const stored = next.find((c) => c.windowId === hit.windowId);
  const title = w ? scope.titleFor(w) : stored?.titleAtClaim || "Window";
  scope.claimFlash = { pid: hit.projectId, title, until: Date.now() + scope.CLAIM_FLASH_MS };
  if (scope.claimFlashTimer) clearTimeout(scope.claimFlashTimer);
  scope.claimFlashTimer = setTimeout(() => {
    scope.claimFlash = null;
    scope.claimFlashTimer = null;
  }, scope.CLAIM_FLASH_MS);
  void scope.invoke("locate_planet", { id: hit.projectId }).catch(() => {});
}

export function onStagePointerMove(scope: Scope_onStagePointerMove, e: PointerEvent) {
  if (scope.embedded) return;
  if (scope.edgePointerSeed) scope.edgePointerSeed = false;
  const painted = scope.overPainted(e);
  scope.pointerInside = painted;
  if (!scope.stageRevealed) return;
  scope.applyEdge({ kind: "move", painted });
}

export function noteWheelSpin(scope: Scope_noteWheelSpin) {
  scope.wheelSpinUntil = Date.now() + 360;
}

export function wheelAnimating(scope: Scope_wheelAnimating): boolean {
  return Date.now() < scope.wheelSpinUntil;
}

export function nearHit(scope: Scope_nearHit, e: { clientX: number; clientY: number; target: EventTarget | null }): boolean {
  const t = e.target;
  if (t instanceof Element) {
    if (
      t.closest(
        ".arch-hit, .arch-name, .arch-dot, .l2-hit, .l2-expanded, .l2-window-card, .l2-board, .l2-needed-card, .stage-settings, .b-card, .l1-abbrev-preview, .chrome, .arch-back, button, a, input, textarea, select",
      )
    ) {
      return true;
    }
  }
  const pad = 8;
  const nodes = document.querySelectorAll(".arch-hit, .arch-name, .l2-hit");
  for (const n of nodes) {
    const r = n.getBoundingClientRect();
    if (
      e.clientX >= r.left - pad &&
      e.clientX <= r.right + pad &&
      e.clientY >= r.top - pad &&
      e.clientY <= r.bottom + pad
    ) {
      return true;
    }
  }
  return false;
}

export function focusBoardLanding(scope: Scope_focusBoardLanding, focus: BoardFocus) {
  const id = focus.kind === "card" && focus.id ? `card-${focus.id}` : null;
  requestAnimationFrame(() => {
    const panel = document.querySelector(".l2-expanded .l2-board");
    if (!(panel instanceof HTMLElement)) return;
    if (id) {
      scope.menuRing.select(id);
      const el = panel.querySelector<HTMLElement>(`[data-mkey="${CSS.escape(id)}"]`);
      if (el) {
        el.focus();
        return;
      }
    }
    const add = panel.querySelector<HTMLElement>("[aria-label='Add a to-do'], .ka-plus");
    if (add) {
      add.focus();
      return;
    }
    const el = panel.querySelector<HTMLElement>(
      "input, textarea, select, button, a[href], [tabindex]:not([tabindex='-1'])",
    );
    (el ?? panel).focus();
  });
}

export function takeMouseButton(scope: Scope_takeMouseButton, button: number): boolean {
  if (button !== 3 && button !== 4) return false;
  const now = Date.now();
  if (now - scope.mouseButtonAt < 50) return true;
  scope.mouseButtonAt = now;
  const intent = scope.inputIntent({ kind: "pointer", button });
  if (intent === "out") scope.onRailEscape();
  else if (intent === "in" && !scope.textFieldFocused()) scope.confirmActive();
  return true;
}

export function onStagePointerDown(scope: Scope_onStagePointerDown, e: PointerEvent) {
  if (scope.embedded || !scope.stageRevealed) return;
  scope.pointerDownAt = { x: e.clientX, y: e.clientY };
  scope.pointerInside = scope.nearHit(e) || scope.overPainted(e);
}

export function onStagePointerUp(scope: Scope_onStagePointerUp, e: PointerEvent) {
  if (scope.embedded || !scope.stageRevealed) return;
  if (scope.takeMouseButton(e.button)) {
    e.preventDefault();
    scope.pointerDownAt = null;
    return;
  }
  if (e.button !== 0) return;
  const drag = scope.pointerDownAt
    ? Math.hypot(e.clientX - scope.pointerDownAt.x, e.clientY - scope.pointerDownAt.y)
    : 0;
  scope.pointerDownAt = null;
  const hit = scope.nearHit(e);
  const animating = scope.wheelAnimating();
  if (scope.spinClickTarget(animating, hit) === "landing") {
    const id = scope.menuRing.activeId;
    if (scope.arch.front === "layer2" && id) scope.confirmLayer2Stop(id);
    else if (id) scope.confirmHabitatStop(id);
    return;
  }
  const panelTarget = e.target;
  const overPanel =
    panelTarget instanceof Element &&
    (!!panelTarget.closest(".stage-settings, .l2-expanded, .l2-board, .chrome") ||
      !!panelTarget.closest(".b-card, .l1-abbrev-preview"));
  const dismiss = scope.emptyDismiss({
    phase: "pointerup",
    dragPx: drag,
    overHit: hit,
    overPanel,
    animating,
  });
  if (dismiss) scope.onRailEscape();
}

export function onStageAux(scope: Scope_onStageAux, e: MouseEvent) {
  if (scope.embedded || !scope.stageRevealed) return;
  if (!scope.takeMouseButton(e.button)) return;
  e.preventDefault();
}

export function onStageMouseLeave(scope: Scope_onStageMouseLeave, e: MouseEvent) {
  if (scope.embedded) return;
  if (scope.edgePointerSeed) scope.edgePointerSeed = false;
  const host = e.currentTarget;
  if (!(host instanceof HTMLElement)) return;
  const rect = host.getBoundingClientRect();
  const side = scope.edgeLeaveSide(
    scope.railEdge,
    e.clientX - rect.left,
    e.clientY - rect.top,
    rect.width,
    rect.height,
  );
  if (side === "away") scope.pointerInside = false;
  scope.applyEdge({ kind: "leave", side });
  // The rail has been used and left: now a raise cannot steal its click.
  void sceneFlushFocus();
}

export function onWinBlur(scope: Scope_onWinBlur) {
  scope.stageFocus(false);
}

export function onExpandedFieldEscape(scope: Scope_onExpandedFieldEscape, e: KeyboardEvent) {
  if (e.key !== "Escape" || scope.embedded || scope.arch.front !== "layer2") return;
  const active = document.activeElement;
  if (!(active instanceof HTMLElement) || !active.closest(".l2-expanded")) return;
  const tag = active.tagName;
  const field = tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || active.isContentEditable;
  if (!field) return;
  const stop = scope.layer2For(scope.arch.pid)[scope.arch.center2]?.stop;
  const icon = stop ? document.querySelector(`[data-mkey="${CSS.escape(stop)}"]`) : null;
  if (icon instanceof HTMLElement) icon.focus();
  e.preventDefault();
  e.stopPropagation();
}

export function applyStagePrefs(scope: Scope_applyStagePrefs, raw: {
  railEdge?: unknown;
  railAutoHideOnBlur?: unknown;
  autohideRowChrome?: unknown;
}) {
  const prefs = scope.stagePrefs({
    railEdge: raw.railEdge,
    railAutoHideOnBlur: raw.railAutoHideOnBlur,
    autohideRowChrome:
      raw.autohideRowChrome === undefined ? scope.autohideRowChrome : raw.autohideRowChrome,
  });
  scope.railEdge = prefs.railEdge;
  scope.railAutoHideOnBlur = prefs.railAutoHideOnBlur;
}

export function sheetRowsFor(scope: Scope_sheetRowsFor, projectId: string): WindowInfo[] {
  return scope.windows.filter((w) =>
    scope.liveClaims.some((c) => c.windowId === w.id && c.projectId === projectId),
  );
}

export async function raisePlanet(scope: Scope_raisePlanet, id: string) {
  // bring_planet_front is a no-op while the set is hidden, and locate only
  // glows a marble. Unhide, then expand this disc, then raise and locate it.
  if (scope.planetsHidden) {
    scope.planetsHidden = false;
    try {
      await scope.invoke("set_planets_hidden", { hidden: false });
    } catch {
      scope.planetsHidden = true;
    }
  }
  if (scope.isCollapsed(id)) {
    try {
      await scope.emitTo("main", "habitat:expand", id);
    } catch {
      /* expand is what opens a marble; raise still runs */
    }
  }
  try {
    await scope.invoke("bring_planet_front", { id });
  } catch {
    /* one disc; do not raise the rest */
  }
  try {
    await scope.invoke("locate_planet", { id });
  } catch {
    /* glow is the locate */
  }
}

export function onSheetPlanetClick(scope: Scope_onSheetPlanetClick, id: string) {
  scope.sheetSlot = id;
  scope.raisePlanet(id);
}

export function onSheetPlanetDblClick(scope: Scope_onSheetPlanetDblClick, id: string) {
  const collapsed = scope.isCollapsed(id);
  scope.emitTo("main", collapsed ? "habitat:expand" : "habitat:collapse", id).catch(
    () => {},
  );
}

export function sheetClaim(scope: Scope_sheetClaim, windowId: string, projectId: string) {
  scope.claimPickerFor = null;
  scope.claim(windowId, projectId);
}

export function sheetFocus(scope: Scope_sheetFocus, w: WindowInfo) {
  scope.invoke("focus_window", { id: w.id }).catch(() => {});
}

export function toggleBoardCol(scope: Scope_toggleBoardCol, col: string) {
  scope.railBoardCols = scope.toggleCol(scope.railBoardCols, col);
  scope.persist();
}

export async function focusRow(scope: Scope_focusRow, w: WindowInfo) {
  try {
    await scope.invoke("focus_window", { id: w.id });
  } catch {
    /* app may refuse; the next tick shows where it landed */
  }
  scope.touchClaim(w.id, true);
  await scope.tick();
}

export async function raisePlanetWindows(scope: Scope_raisePlanetWindows, pid: string) {
  const plan = scope.raisePlan(scope.rowsFor(pid));
  for (const id of plan.restore) {
    try {
      await scope.invoke("restore_window", { id });
    } catch {
      /* a closed window drops out; focus still runs */
    }
  }
  for (let i = 0; i < plan.focus.length; i++) {
    try {
      await scope.invoke("focus_window", { id: plan.focus[i] });
    } catch {
      /* skip a window the OS will not raise */
    }
    if (i + 1 < plan.focus.length) {
      await new Promise((r) => setTimeout(r, 40));
    }
  }
  scope.raisePlanet(pid);
  await scope.tick();
  scope.parkAfterRaise();
}

export async function openPlanetTodos(scope: Scope_openPlanetTodos, pid: string) {
  await scope.openKanban();
  scope.emitTo("kanban", "orbitty:focus-project", pid).catch(() => {});
  setTimeout(
    () => scope.emitTo("kanban", "orbitty:focus-project", pid).catch(() => {}),
    400,
  );
}

export function rowById(scope: Scope_rowById, id: string): WindowInfo | undefined {
  for (const p of scope.projects) {
    const hit = scope.rowsFor(p.id).find((w) => w.id === id);
    if (hit) return hit;
  }
  return scope.windows.find((w) => w.id === id);
}

// ---- Planet scenes (GO 42b) -----------------------------------------
// The desk as it was before the first scene lives here in memory only: a
// quit forgets it and every window stays where it is (42b Needs Matt 3).
// The decision itself is pure — habitat.ts sceneEnter / sceneLeave.

let sceneSession: SceneSession | null = null;

// FIX2 (42b regression): a scene toggle used restore_window, which is
// focus_window — focusing a window activates its app, the rail blurred and
// the auto-hide parked it in under a second. The toggle now unminimizes with
// deminiaturize_window (no activation) and keeps the raise here until the
// pointer leaves the rail.
let sceneDeferredRaise: { invoke: Invoke; ids: string[] } | null = null;

/** Hold this raise until the pointer leaves the rail. One list: a second
 *  toggle replaces the first, and one pointer-leave applies it once. */
function sceneDeferFocus(invoke: Invoke, ids: readonly string[]): void {
  const keep = ids.filter((id) => !!id);
  sceneDeferredRaise = keep.length ? { invoke, ids: [...keep] } : null;
}

/** Pointer left the strip: a raise is safe now, the rail has had its click.
 *  Fire and forget; a window gone since the toggle is skipped. */
async function sceneFlushFocus(): Promise<void> {
  const pending = sceneDeferredRaise;
  sceneDeferredRaise = null;
  if (!pending) return;
  for (let i = 0; i < pending.ids.length; i++) {
    try {
      await pending.invoke("focus_window", { id: pending.ids[i] });
    } catch {
      /* the window closed while the pointer was still on the rail */
    }
    if (i + 1 < pending.ids.length) await new Promise((r) => setTimeout(r, 40));
  }
}

/** The planet whose scene is open, or null. */
export function scenePid(): string | null {
  return sceneSession?.pid ?? null;
}

type SceneFrameRow = {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  isMinimized: boolean;
  onScreen: boolean;
};

type Scope_sceneRows = {
  windows: WindowInfo[];
  rowsFor: (pid: string) => WindowInfo[];
  invoke: Invoke;
  tick?: () => Promise<unknown>;
};

/** Every live window the scene may touch: its frame, its minimized flag, and
 *  whether it is on this Space. A row the AX read lost keeps frame null and
 *  is skipped by the plan instead of aborting the scene. */
async function sceneRows(scope: Scope_sceneRows, pid: string): Promise<SceneWindowRow[]> {
  const live = scope.windows.filter((w) => !w.sleeping && !w.id.startsWith("tab:"));
  if (live.length === 0) return [];
  const claimed = new Set(scope.rowsFor(pid).map((r) => r.id));
  let frames: SceneFrameRow[] = [];
  try {
    frames = await scope.invoke<SceneFrameRow[]>("window_frames", { ids: live.map((w) => w.id) });
  } catch {
    // No frames, no scene: entering must never guess at a window's box.
    return [];
  }
  const byId = new Map(frames.map((f) => [f.id, f]));
  return live.map((w) => {
    const f = byId.get(w.id);
    return {
      id: w.id,
      stableKey: w.stableKey,
      frame: f ? { x: f.x, y: f.y, w: f.w, h: f.h } : null,
      minimized: f ? f.isMinimized : !!w.isMinimized,
      claimed: claimed.has(w.id),
      onScreen: f ? f.onScreen : false,
      frontmost: !!w.isFrontmost,
    };
  });
}

/** The planet keeps the frames it was left in. Best effort: the desk restore
 *  never depends on this write, so a failed save still restores cleanly. */
async function sceneSave(
  scope: Scope_sceneRows,
  pid: string,
  windows: SceneSavedFrame[],
): Promise<void> {
  if (windows.length === 0) return;
  try {
    const state = await scope.invoke<PersistedState>("load_state");
    const map = state.planetScenes ?? {};
    map[pid] = { savedAt: Date.now(), windows };
    state.planetScenes = map;
    // The read-modify-write keeps every other slice; merge_window_save
    // carries the Rust-owned ones from disk.
    await scope.invoke("save_state", { state, origin: "rail" });
  } catch {
    /* a scene is still a scene without a remembered layout */
  }
}

type Scope_enterScene = Scope_sceneRows;

export async function enterScene(scope: Scope_enterScene, pid: string): Promise<void> {
  const rows = await sceneRows(scope, pid);
  if (!rows.length) return;
  // Switching planets: the planet being left keeps the layout it was left in.
  const prev = sceneSession;
  if (prev && prev.pid !== pid) {
    const left = await sceneRows(scope, prev.pid);
    await sceneSave(scope, prev.pid, sceneLeave(prev, left).save);
  }
  let saved: SceneSavedFrame[] = [];
  try {
    const state = await scope.invoke<PersistedState>("load_state");
    saved = state.planetScenes?.[pid]?.windows ?? [];
  } catch {
    /* no saved layout: the claimed windows stay where they are */
  }
  const next = sceneEnter(sceneSession, pid, rows, saved);
  sceneSession = next.session;
  const plan = next.plan;
  for (const id of plan.minimize) {
    try {
      await scope.invoke("minimize_window", { id });
    } catch {
      /* a window that refuses to stow simply stays up */
    }
  }
  for (const id of plan.restore) {
    try {
      // No activation: the rail keeps focus across the toggle (FIX2).
      await scope.invoke("deminiaturize_window", { id });
    } catch {
      /* a closed window drops out; the rest still rise */
    }
  }
  for (const f of plan.frames) {
    try {
      await scope.invoke("set_window_frame", { id: f.id, x: f.x, y: f.y, w: f.w, h: f.h });
    } catch {
      /* a window that refuses to move stays where it is */
    }
  }
  // Raising a claimed window activates its app and blurs the rail, so the
  // raise waits for the pointer to leave; the hold keeps the rail clickable.
  sceneDeferFocus(scope.invoke, plan.focus);
  armSceneHold();
  await scope.tick?.();
}

type Scope_leaveScene = Scope_sceneRows;

export async function leaveScene(scope: Scope_leaveScene): Promise<void> {
  const session = sceneSession;
  if (!session) return;
  const rows = await sceneRows(scope, session.pid);
  const out = sceneLeave(session, rows);
  await sceneSave(scope, session.pid, out.save);
  sceneSession = null;
  for (const id of out.plan.restore) {
    try {
      // No activation: the rail keeps focus across the toggle (FIX2).
      await scope.invoke("deminiaturize_window", { id });
    } catch {
      /* gone since the scene began */
    }
  }
  for (const f of out.plan.frames) {
    try {
      await scope.invoke("set_window_frame", { id: f.id, x: f.x, y: f.y, w: f.w, h: f.h });
    } catch {
      /* a closed window is not a reason to abort the leave */
    }
  }
  // Frames first, then stow: a window that was in the Dock before the scene
  // goes back into the Dock at its pre-scene box.
  for (const id of out.plan.minimize) {
    try {
      await scope.invoke("minimize_window", { id });
    } catch {
      /* a window that refuses to stow stays where it is */
    }
  }
  // The raise waits for the pointer to leave, exactly as on enter.
  sceneDeferFocus(scope.invoke, out.plan.focus);
  armSceneHold();
  await scope.tick?.();
}

/** One press on the scene tile: enter this planet's scene, or leave it. */
export async function toggleScene(scope: Scope_enterScene, pid: string): Promise<string | null> {
  if (sceneSession?.pid === pid) {
    await leaveScene(scope);
    return null;
  }
  await enterScene(scope, pid);
  return scenePid();
}
