// Moved function bodies. scope fields are the lets each function already closed over.

import type { ActionShortcut, AgentRun, AgentShortcut, CardTombstone, Claim, CompanionSlice, DrawerSlice, KanbanCard, PersistedState, PointsEntry, Project, SkySlice, StarsSlice, Tag, TitleOverride, TodoTree, WindowInfo, WindowTags } from "../persist";
import type { BoardCol } from "./habitat";
import type { CoachStory } from "./coach";

interface AxStatus {
  trusted: boolean;
  bundlePath: string;
  signingLine: string;
  inBundle: boolean;
  isAdhoc: boolean;
}

interface PackInfo {
  id: string;
  name: string;
  license: string | null;
  warnLicense: boolean;
  frameCount: number;
  path: string;
  role: string;
}

type Invoke = <T = unknown>(cmd: string, args?: any) => Promise<T>;

type Scope_titleFor = {
  titleOverrides: TitleOverride[];
};

type Scope_startRename = {
  renamingId: string | null;
  renameValue: string;
  titleOverrides: TitleOverride[];
};

type Scope_openCtxMenu = {
  ctxMenu: { x: number; y: number; windowId: string } | null;
  ctxAssignOpen: boolean;
  newProjectName: string;
};

type Scope_ctxWindow = {
  windows: WindowInfo[];
  ctxMenu: { x: number; y: number; windowId: string } | null;
};

type Scope_ctxProject = {
  ctxMenu: { x: number; y: number; windowId: string } | null;
  liveClaims: Claim[];
  projects: Project[];
};

type Scope_ctxAssign = {
  ctxWindow: (...args: any[]) => WindowInfo | undefined;
  ctxMenu: { x: number; y: number; windowId: string } | null;
  claim: (...args: any[]) => any;
};

type Scope_ctxCreateAndClaim = {
  ctxWindow: (...args: any[]) => WindowInfo | undefined;
  newProjectName: string;
  createProject: (...args: any[]) => Project;
  ctxMenu: { x: number; y: number; windowId: string } | null;
  claim: (...args: any[]) => any;
};

type Scope_ctxRename = {
  ctxWindow: (...args: any[]) => WindowInfo | undefined;
  ctxMenu: { x: number; y: number; windowId: string } | null;
  startRename: (...args: any[]) => any;
};

type Scope_ctxMinimize = {
  ctxWindow: (...args: any[]) => WindowInfo | undefined;
  ctxMenu: { x: number; y: number; windowId: string } | null;
  stow: (...args: any[]) => Promise<any>;
};

type Scope_ctxRestore = {
  ctxWindow: (...args: any[]) => WindowInfo | undefined;
  ctxMenu: { x: number; y: number; windowId: string } | null;
  restore: (...args: any[]) => Promise<any>;
};

type Scope_ctxCloseWindow = {
  ctxWindow: (...args: any[]) => WindowInfo | undefined;
  ctxMenu: { x: number; y: number; windowId: string } | null;
  invoke: Invoke;
  tick: (...args: any[]) => Promise<any>;
};

type Scope_ctxUnclaim = {
  ctxWindow: (...args: any[]) => WindowInfo | undefined;
  ctxMenu: { x: number; y: number; windowId: string } | null;
  unclaim: (...args: any[]) => any;
};

type Scope_commitRename = {
  renameValue: string;
  titleOverrides: TitleOverride[];
  persist: (...args: any[]) => Promise<any>;
  renamingId: string | null;
};

type Scope_setWorkspaceRoot = {
  invoke: Invoke;
  projects: Project[];
};

type Scope_persist = {
  stateLoaded: boolean;
  windowDragHot: (...args: any[]) => boolean;
  persistDeferred: boolean;
  pendingDisk: { kanbanCards: KanbanCard[]; pointsLedger: PointsEntry[]; agentRuns: AgentRun[]; cardTombstones: CardTombstone[] } | null;
  absorbPendingCards: (...args: any[]) => any;
  themeChosen: boolean;
  themeId: string;
  windowOpacity: number;
  projects: Project[];
  liveClaims: Claim[];
  tags: Tag[];
  liveWindowTags: WindowTags[];
  collapsedSectionIds: string[];
  nudgeSuppressed: string[];
  kanbanCards: KanbanCard[];
  cardTombstones: CardTombstone[];
  pointsLedger: PointsEntry[];
  titleOverrides: TitleOverride[];
  autohideRowChrome: boolean;
  hideTitles: boolean;
  hideAppIcons: boolean;
  railBoardCols: BoardCol[];
  spriteScale: number;
  marbleDrift: boolean;
  peerChrome: boolean;
  planetIdle: string;
  agentTerminal: string;
  actionShortcuts: ActionShortcut[];
  agentShortcuts: AgentShortcut[];
  agentRuns: AgentRun[];
  neededSplit: number;
  previewWidth: number;
  trackerSplit: number;
  openSubtaskIds: string[];
  todoTrees: TodoTree[];
  treeGrowDays: number;
  companionState: CompanionSlice;
  starsState: StarsSlice;
  skyState: SkySlice;
  drawersState: Record<string, DrawerSlice>;
  invoke: Invoke;
  syncHabitat: (...args: any[]) => Promise<any>;
};

type Scope_sleeperRowFor = {
  claimStableKey: (...args: any[]) => string | null;
};

type Scope_boardCardsFor = {
  kanbanCards: KanbanCard[];
  liveClaims: Claim[];
};

type Scope_habitatEntryFor = {
  liveClaims: Claim[];
  kanbanCards: KanbanCard[];
  ageClass: (...args: any[]) => "fresh" | "fade" | "park" | "gone";
};

type Scope_runIsSeated = {};

type Scope_goToken = {};

type Scope_goNeededOnPlanet = {
  goToken: (...args: any[]) => string | null;
  kanbanCards: KanbanCard[];
};

type Scope_agentRowsFor = {
  agentRuns: AgentRun[];
  runIsSeated: (...args: any[]) => boolean;
  kanbanCards: KanbanCard[];
};

type Scope_locateAgent = {
  invoke: Invoke;
};

type Scope_agentEntriesFor = {
  agentRuns: AgentRun[];
  kanbanCards: KanbanCard[];
  runIsSeated: (...args: any[]) => boolean;
  agentShortcuts: AgentShortcut[];
  goNeededOnPlanet: (...args: any[]) => boolean;
};

type Scope_habitatPlanetFor = {
  isCollapsed: (...args: any[]) => boolean;
  boardCardsFor: (...args: any[]) => any;
  todoTrees: TodoTree[];
  kanbanCards: KanbanCard[];
  habitatEntryFor: (...args: any[]) => any;
  agentEntriesFor: (...args: any[]) => any;
};

type Scope_habitatSnapshotPayload = {
  spriteScale: number;
  marbleDrift: boolean;
  projects: Project[];
  treeGrowDays: number;
  habitatPlanetFor: (...args: any[]) => { projectId: string; collapsed: boolean; boardCards: any; todoTrees: any; entries: { id: string; minimized: boolean; iconBase64: string | null; age: string; asleep: boolean; spritePackId: string; charSeed: number; charLook: unknown; spriteScale: number; cardStatus: string; cardKind: string; neededState: string; agentSeed: string }[] };
};

type Scope_syncHabitat = {
  stateLoaded: boolean;
  status: AxStatus | null;
  projects: Project[];
  liveClaims: Claim[];
  occupancyOf: (...args: any[]) => string;
  ageClass: (...args: any[]) => "fresh" | "fade" | "park" | "gone";
  windows: WindowInfo[];
  sleeperRowFor: (...args: any[]) => WindowInfo;
  habitatPlanetFor: (...args: any[]) => { projectId: string; collapsed: boolean; boardCards: any; todoTrees: any; entries: { id: string; minimized: boolean; iconBase64: string | null; age: string; asleep: boolean; spritePackId: string; charSeed: number; charLook: unknown; spriteScale: number; cardStatus: string; cardKind: string; neededState: string; agentSeed: string }[] };
  packs: PackInfo[];
  spriteScale: number;
  marbleDrift: boolean;
  starterStory: CoachStory;
  lastHabitatSig: string;
  invoke: Invoke;
  habitatSnapshotPayload: (...args: any[]) => any;
};

type Scope_lastKnownRowFor = {
  sleeperRowFor: (...args: any[]) => WindowInfo;
};

type Scope_syncHabitatLastKnown = {
  stateLoaded: boolean;
  embedded: boolean;
  projects: Project[];
  liveClaims: Claim[];
  occupancyOf: (...args: any[]) => string;
  lastKnownRowFor: (...args: any[]) => WindowInfo;
  habitatPlanetFor: (...args: any[]) => { projectId: string; collapsed: boolean; boardCards: any; todoTrees: any; entries: { id: string; minimized: boolean; iconBase64: string | null; age: string; asleep: boolean; spritePackId: string; charSeed: number; charLook: unknown; spriteScale: number; cardStatus: string; cardKind: string; neededState: string; agentSeed: string }[] };
  invoke: Invoke;
  habitatSnapshotPayload: (...args: any[]) => any;
};

type Scope_flashRow = {
  locateFlashIds: string[];
};

type Scope_locateRow = {
  liveClaims: Claim[];
  projects: Project[];
  invoke: Invoke;
  flashRow: (...args: any[]) => any;
};

type Scope_stow = {
  invoke: Invoke;
  tick: (...args: any[]) => Promise<any>;
};

type Scope_restore = {
  invoke: Invoke;
  touchClaim: (...args: any[]) => any;
  tick: (...args: any[]) => Promise<any>;
};

type Scope_wakeSleeper = {
  wakeNote: { kind: "ok" | "err"; text: string } | null;
  invoke: Invoke;
  touchClaim: (...args: any[]) => any;
  tick: (...args: any[]) => Promise<any>;
};

type Scope_nudgeClaim = {
  nudgeFor: string | null;
  claim: (...args: any[]) => any;
};

type Scope_nudgeCreate = {
  nudgeFor: string | null;
  nudgeNewName: string;
  createProject: (...args: any[]) => Project;
  claim: (...args: any[]) => any;
};

type Scope_clampSpriteScale = {};

type Scope_openKanban = {
  invoke: Invoke;
};

type Scope_bringAllFront = {
  invoke: Invoke;
};

type Scope_onBannerMouseDown = {
  wordmarkDidDrag: boolean;
  startPaneDrag: (...args: any[]) => void;
};

type Scope_onWordmarkClick = {
  wordmarkDidDrag: boolean;
  bringAllFront: (...args: any[]) => Promise<any>;
};

type Scope_openKanbanFocus = {
  openKanban: (...args: any[]) => Promise<any>;
  emitTo: (...args: any[]) => any;
};

type Scope_launchHermes = {
  invoke: Invoke;
  hermesNote: { kind: "ok" | "err"; text: string } | null;
  rowsFor: (...args: any[]) => WindowInfo[];
  todosFor: (...args: any[]) => KanbanCard[];
};

type Scope_toggleSoul = {
  soulFor: string | null;
  soulBody: string;
  soulPath: string;
  soulMsg: string;
  invoke: Invoke;
};

type Scope_saveSoul = {
  soulFor: string | null;
  invoke: Invoke;
  soulBody: string;
  soulMsg: string;
};

type Scope_openSoulEditor = {
  soulFor: string | null;
  projects: Project[];
  invoke: Invoke;
  soulMsg: string;
};

type Scope_createProject = {
  projects: Project[];
  collapsedSectionIds: string[];
};

type Scope_revealPlanet = {
  sectionIds: (...args: any[]) => string[];
  collapsedSectionIds: string[];
};

type Scope_newCardId = {};

type Scope_isClinicalOrMaster = {
  tagsFor: (...args: any[]) => Tag[];
};

type Scope_titleIndexFor = {
  windows: WindowInfo[];
};

type Scope_claimStableKey = {};

type Scope_isHermesClass = {};

type Scope_sleeperStamps = {
  isHermesClass: (...args: any[]) => boolean;
};

type Scope_rollCharSeed = {};

export function titleFor(scope: Scope_titleFor, w: WindowInfo): string {
  return (
    scope.titleOverrides.find((o) => o.windowId === w.id)?.title ||
    w.title ||
    w.appName
  );
}

export function startRename(scope: Scope_startRename, w: WindowInfo) {
  scope.renamingId = w.id;
  scope.renameValue =
    scope.titleOverrides.find((o) => o.windowId === w.id)?.title ?? w.title;
}

export function openCtxMenu(scope: Scope_openCtxMenu, e: MouseEvent, w: WindowInfo) {
  e.preventDefault();
  e.stopPropagation();
  // Clamp inside the rail so the menu never opens under the cursor's edge.
  const x = Math.min(e.clientX, window.innerWidth - 170);
  const y = Math.min(e.clientY, window.innerHeight - 220);
  scope.ctxMenu = { x, y, windowId: w.id };
  scope.ctxAssignOpen = false;
  scope.newProjectName = "";
}

export function ctxWindow(scope: Scope_ctxWindow): WindowInfo | undefined {
  return scope.windows.find((w) => w.id === scope.ctxMenu?.windowId);
}

export function ctxProject(scope: Scope_ctxProject): Project | undefined {
  const id = scope.ctxMenu?.windowId;
  if (!id) return undefined;
  const projectId = scope.liveClaims.find((c) => c.windowId === id)?.projectId;
  return scope.projects.find((p) => p.id === projectId);
}

export function ctxAssign(scope: Scope_ctxAssign, projectId: string) {
  const w = scope.ctxWindow();
  scope.ctxMenu = null;
  if (w) scope.claim(w.id, projectId);
}

export function ctxCreateAndClaim(scope: Scope_ctxCreateAndClaim) {
  const w = scope.ctxWindow();
  if (!w || !scope.newProjectName.trim()) return;
  const p = scope.createProject(scope.newProjectName);
  scope.ctxMenu = null;
  scope.claim(w.id, p.id);
  scope.newProjectName = "";
}

export function ctxRename(scope: Scope_ctxRename) {
  const w = scope.ctxWindow();
  scope.ctxMenu = null;
  if (w) scope.startRename(w);
}

export function ctxMinimize(scope: Scope_ctxMinimize) {
  const w = scope.ctxWindow();
  scope.ctxMenu = null;
  // Claimed → the walker walks its planet; unmapped → Dock only (the
  // habitat snapshot never sees unclaimed windows).
  if (w) scope.stow(w);
}

export function ctxRestore(scope: Scope_ctxRestore) {
  const w = scope.ctxWindow();
  scope.ctxMenu = null;
  if (w) scope.restore(w);
}

export function ctxCloseWindow(scope: Scope_ctxCloseWindow) {
  const w = scope.ctxWindow();
  scope.ctxMenu = null;
  if (!w) return;
  scope.invoke("close_window", { id: w.id })
    .catch(() => {})
    .then(() => scope.tick());
}

export function ctxUnclaim(scope: Scope_ctxUnclaim) {
  const w = scope.ctxWindow();
  scope.ctxMenu = null;
  if (w) scope.unclaim(w.id);
}

export function commitRename(scope: Scope_commitRename, w: WindowInfo) {
  const next = scope.renameValue.trim();
  const cur = scope.titleOverrides.find((o) => o.windowId === w.id)?.title ?? "";
  if (next !== cur) {
    scope.titleOverrides = next
      ? [
          ...scope.titleOverrides.filter((o) => o.windowId !== w.id),
          { windowId: w.id, title: next },
        ]
      : scope.titleOverrides.filter((o) => o.windowId !== w.id);
    scope.persist();
  }
  scope.renamingId = null;
}

export async function setWorkspaceRoot(scope: Scope_setWorkspaceRoot, projectId: string, path: string) {
  try {
    await scope.invoke("set_workspace_root", { projectId, path });
    scope.projects = scope.projects.map((p) =>
      p.id === projectId ? { ...p, workspaceRoot: path.trim() || undefined } : p,
    );
  } catch {
    /* void refuses a root */
  }
}

export async function persist(scope: Scope_persist) {
  if (!scope.stateLoaded) return;
  // A drag-hot store may have skipped state:changed. Don't write it.
  // Drop/claim still mutates memory; drag-end flushes after the hot window.
  if (scope.windowDragHot()) {
    scope.persistDeferred = true;
    return;
  }
  if (scope.pendingDisk) scope.absorbPendingCards();
  scope.persistDeferred = false;
  const state: PersistedState = {
    // Only an explicit choice is saved; an untouched theme stays "" so the
    // next launch keeps following the OS instead of freezing today's pick.
    themeId: scope.themeChosen ? scope.themeId : "",
    windowOpacity: scope.windowOpacity,
    projects: scope.projects,
    claims: scope.liveClaims,
    tags: scope.tags,
    windowTags: scope.liveWindowTags,
    collapsedSectionIds: scope.collapsedSectionIds,
    nudgeSuppressedWindowIds: scope.nudgeSuppressed,
    kanbanCards: scope.kanbanCards,
    cardTombstones: scope.cardTombstones,
    pointsLedger: scope.pointsLedger,
    titleOverrides: scope.titleOverrides,
    autohideRowChrome: scope.autohideRowChrome,
    hideTitles: scope.hideTitles,
    hideAppIcons: scope.hideAppIcons,
    railBoardCols: scope.railBoardCols,
    spriteScale: scope.spriteScale,
    marbleDrift: scope.marbleDrift,
    peerChrome: scope.peerChrome,
    planetIdle: scope.planetIdle,
    agentTerminal: scope.agentTerminal,
    actionShortcuts: scope.actionShortcuts,
    agentShortcuts: scope.agentShortcuts,
    agentRuns: scope.agentRuns,
    neededSplit: scope.neededSplit,
    previewWidth: scope.previewWidth,
    trackerSplit: scope.trackerSplit,
    openSubtaskIds: scope.openSubtaskIds,
    todoTrees: scope.todoTrees,
    treeGrowDays: scope.treeGrowDays,
    companion: scope.companionState,
    stars: scope.starsState,
    sky: scope.skyState,
    drawers: scope.drawersState,
  };
  try {
    await scope.invoke("save_state", { state, origin: "rail" });
  } catch {
    // next mutation retries; rail stays usable without persistence
  }
  await scope.syncHabitat();
}

export function sleeperRowFor(scope: Scope_sleeperRowFor, c: Claim): WindowInfo {
  return {
    id: c.windowId,
    pid: 0,
    appName: c.appName ?? "App",
    title: c.titleAtClaim || c.appName || "",
    bundleId: c.bundleId ?? null,
    stableKey: scope.claimStableKey(c) ?? "",
    iconBase64: null,
    isFrontmost: false,
    isMinimized: true,
    documentPath: c.filePath ?? null,
    sleeping: true,
  };
}

export function boardCardsFor(scope: Scope_boardCardsFor, p: Project) {
  // ORPHAN-CARDS: a kind:"window" card ticks only while a live claim
  // carries its windowId — an orphan can't be opened from anywhere, so
  // it must not paint the sign. Loose todos still tick.
  const mine = scope.kanbanCards.filter(
    (k) =>
      (k.projectId ?? "") === p.id &&
      k.kind !== "needed" &&
      (k.kind !== "window" ||
        (!!k.windowId && scope.liveClaims.some((c) => c.windowId === k.windowId))),
  );
  const open = mine
    .filter((k) => k.status !== "done")
    .sort((a, b) => a.createdAt - b.createdAt);
  const done = mine
    .filter((k) => k.status === "done")
    .sort((a, b) => b.lastTouchedAt - a.lastTouchedAt);
  return [...open, ...done].slice(0, 12).map((k) => ({
    id: k.id,
    status: k.status,
    windowId: k.windowId ?? null,
  }));
}

export function habitatEntryFor(scope: Scope_habitatEntryFor, w: WindowInfo, p: Project) {
  const c = scope.liveClaims.find((x) => x.windowId === w.id);
  // The window's board truth rides the snapshot
  // (one channel — sync_habitat). The window-linked card wins for
  // this window id: an open card first, else the latest done one
  // (so the walk-off still shows between Complete and the rail's
  // unclaim). Needed cards never pose as the linked card — their
  // status is a dummy — they only sit the walker via neededState.
  const linked = scope.kanbanCards.filter(
    (k) => k.windowId === w.id && k.kind !== "needed",
  );
  const openCard = linked.find((k) => k.status !== "done");
  const doneCard = openCard
    ? undefined
    : linked
        .filter((k) => k.status === "done")
        .sort((a, b) => b.lastTouchedAt - a.lastTouchedAt)[0];
  const card = openCard ?? doneCard;
  // A Needed (open/later) naming this window — or this project
  // with no window — sits the walker. Needed wins over Doing.
  const needed = scope.kanbanCards.find(
    (k) =>
      k.kind === "needed" &&
      (k.neededState === "open" || k.neededState === "later") &&
      (k.windowId === w.id ||
        (k.windowId == null && (k.projectId ?? "") === p.id)),
  );
  return {
    id: w.id,
    appName: w.appName,
    title: w.title,
    bundleId: w.bundleId,
    minimized: w.isMinimized,
    iconBase64: w.iconBase64,
    age: scope.ageClass(w.id),
    asleep: !!w.sleeping,
    spritePackId: c?.spritePackId ?? "",
    // The generated being + per-walker size ride
    // the snapshot; 0 scale = inherit the gear's global slider.
    charSeed: c?.charSeed ?? 0,
    charLook: c?.charLook ?? null,
    spriteScale: c?.spriteScale ?? 0,
    cardStatus: card?.status ?? "",
    cardKind: card?.kind ?? "",
    neededState: needed ? (needed.neededState ?? "open") : "",
    // Window walkers never bake the droid —
    // the empty seed keeps one entry shape for the sig.
    agentSeed: "",
  };
}

export function runIsSeated(scope: Scope_runIsSeated, r: AgentRun): boolean {
  return (
    !!(r.agentShortcutId || "").trim() || !!(r.herdr?.agentName || "").trim()
  );
}

export function goToken(scope: Scope_goToken, title: string): string | null {
  const m = title.match(/GO-[A-Z0-9]+(?:-[A-Z0-9]+)*/i);
  return m ? m[0].toUpperCase() : null;
}

export function goNeededOnPlanet(scope: Scope_goNeededOnPlanet, card: KanbanCard | undefined, planetId: string): boolean {
  if (!card) return false;
  const go = scope.goToken(card.title);
  if (!go) return false;
  return scope.kanbanCards.some(
    (c) =>
      c.kind === "needed" &&
      (c.neededState === "open" || c.neededState === "later") &&
      (c.projectId || "") === planetId &&
      ((c.title || "").toUpperCase().includes(go) ||
        (c.neededWhy || "").toUpperCase().includes(go)),
  );
}

export function agentRowsFor(scope: Scope_agentRowsFor, projectId: string) {
  return scope.agentRuns.filter(
    (r) =>
      r.planetId === projectId &&
      r.state !== "done" &&
      scope.runIsSeated(r) &&
      scope.kanbanCards.some((k) => k.id === r.cardId),
  );
}

export async function locateAgent(scope: Scope_locateAgent, r: AgentRun) {
  const entryId = `agent:${r.cardId}`;
  try {
    await scope.invoke("locate_walker", { entryId });
  } catch {
    /* habitat may be mid-sync */
  }
  try {
    await scope.invoke("inspect_droid", { entryId });
  } catch {
    /* no herdr pane yet */
  }
}

export function agentEntriesFor(scope: Scope_agentEntriesFor, p: Project) {
  return scope.agentRuns
    .filter(
      (r) =>
        r.planetId === p.id &&
        scope.kanbanCards.some((k) => k.id === r.cardId) &&
        (scope.runIsSeated(r) || r.state === "done"),
    )
    .map((r) => {
      const card = scope.kanbanCards.find((k) => k.id === r.cardId);
      const agent = scope.agentShortcuts.find((a) => a.id === r.agentShortcutId);
      const needsYou = scope.goNeededOnPlanet(card, p.id);
      return {
        id: `agent:${r.cardId}`,
        appName: agent?.name ?? "agent",
        title: card?.title ?? "",
        bundleId: null,
        minimized: false,
        iconBase64: null,
        age: "fresh",
        asleep: false,
        spritePackId: "",
        charSeed: 0,
        charLook: null,
        spriteScale: 0,
        cardStatus: r.state === "done" ? "done" : "doing",
        cardKind: "todo",
        // blocked vs needs-you vs working.
        // Window walkers still use open/later for planet Neededs.
        neededState:
          r.state === "blocked" ? "blocked" : needsYou ? "open" : "",
        agentSeed: `${r.planetId}:${r.cardId}`,
      };
    });
}

export function habitatPlanetFor(scope: Scope_habitatPlanetFor, p: Project, rows: WindowInfo[]) {
  return {
    projectId: p.id,
    name: p.name,
    collapsed: scope.isCollapsed(p.id),
    boardCards: scope.boardCardsFor(p),
    todoTrees: scope.todoTrees
      .filter((t) => t.projectId === p.id)
      .map((t) => {
        const card = scope.kanbanCards.find((k) => k.id === t.cardId);
        return {
          cardId: t.cardId,
          x: t.x,
          y: t.y,
          plantedAt: t.plantedAt,
          dueAt: card?.dueAt ?? null,
          status: card?.status ?? "todo",
        };
      }),
    entries: [
      ...rows.map((w) => scope.habitatEntryFor(w, p)),
      ...scope.agentEntriesFor(p),
    ],
  };
}

export function habitatSnapshotPayload(scope: Scope_habitatSnapshotPayload, 
  planets: ReturnType<typeof scope.habitatPlanetFor>[],
) {
  return {
    planets,
    spriteScale: scope.spriteScale,
    marbleDrift: scope.marbleDrift,
    allPlanets: scope.projects.map((p) => ({ projectId: p.id, name: p.name })),
    treeGrowDays: scope.treeGrowDays,
  };
}

export async function syncHabitat(scope: Scope_syncHabitat) {
  if (!scope.stateLoaded || !scope.status?.trusted) return;
  const planets = scope.projects
    .map((p) => ({
      p,
      rows: [
        // Live claimed windows — every claim is a walker now (open or
        // minimized); the native side parks only age-`park` zombies.
        ...scope.windows.filter((w) =>
          scope.liveClaims.some(
            (c) =>
              c.windowId === w.id &&
              scope.occupancyOf(c) === p.id &&
              scope.ageClass(w.id) !== "gone",
          ),
        ),
        // Orphan claims (window closed) ride the snapshot as sleeper
        // rows — see sleeperRowFor.
        ...scope.liveClaims
          .filter(
            (c) =>
              scope.occupancyOf(c) === p.id &&
              !scope.windows.some((w) => w.id === c.windowId),
          )
          .map(scope.sleeperRowFor),
      ],
    }))
    // NO empty-planet filter — a mint
    // with zero claims still rides the snapshot (entries: []) so the
    // native side gives it a disc (empty town / marble). Filtering here
    // tore the panel down and the name "vanished" on the dest click.
    .map(({ p, rows }) => scope.habitatPlanetFor(p, rows));
  // Pack ids join the signature: an import/remove changes the frame pool
  // the habitat draws from, so it must re-sync even when no window moved.
  const sig = JSON.stringify([
    planets.map((p) => [
      p.projectId,
      p.collapsed,
      p.boardCards,
      p.todoTrees,
      p.entries.map((e) => [
        e.id,
        e.minimized,
        !!e.iconBase64,
        e.age,
        e.asleep,
        e.spritePackId,
        e.charSeed,
        e.charLook,
        e.spriteScale,
        e.cardStatus,
        e.cardKind,
        e.neededState,
        e.agentSeed ?? "",
      ]),
    ]),
    scope.packs.map((pk) => pk.id),
    scope.spriteScale,
    scope.marbleDrift,
    scope.starterStory.commsBuilt,
    scope.starterStory.starsBuilt,
  ]);
  if (sig === scope.lastHabitatSig) return;
  scope.lastHabitatSig = sig;
  try {
    await scope.invoke("sync_habitat", {
      snapshot: scope.habitatSnapshotPayload(planets),
    });
  } catch {
    scope.lastHabitatSig = ""; // retry next tick
  }
}

export function lastKnownRowFor(scope: Scope_lastKnownRowFor, c: Claim): WindowInfo {
  return { ...scope.sleeperRowFor(c), sleeping: false };
}

export async function syncHabitatLastKnown(scope: Scope_syncHabitatLastKnown) {
  if (!scope.stateLoaded || scope.embedded) return;
  const planets = scope.projects
    .map((p) => ({
      p,
      rows: scope.liveClaims
        .filter((c) => scope.occupancyOf(c) === p.id)
        .map(scope.lastKnownRowFor),
    }))
    // PROJECT-ADD leftover: empty planets boot as discs too — same
    // no-filter rule as syncHabitat, so a fresh mint survives restart.
    .map(({ p, rows }) => scope.habitatPlanetFor(p, rows));
  if (planets.length === 0) return;
  try {
    await scope.invoke("sync_habitat", {
      snapshot: scope.habitatSnapshotPayload(planets),
    });
  } catch {
    /* the live tick retries */
  }
}

export function flashRow(scope: Scope_flashRow, windowId: string) {
  if (scope.locateFlashIds.includes(windowId)) return;
  scope.locateFlashIds = [...scope.locateFlashIds, windowId];
  setTimeout(() => {
    scope.locateFlashIds = scope.locateFlashIds.filter((x) => x !== windowId);
  }, 550);
}

export async function locateRow(scope: Scope_locateRow, w: WindowInfo) {
  const claim = scope.liveClaims.find((c) => c.windowId === w.id);
  const hasPlanet =
    !!claim && scope.projects.some((p) => p.id === claim.projectId);
  if (w.sleeping || w.isMinimized) {
    if (hasPlanet) {
      try {
        await scope.invoke("locate_walker", { entryId: w.id });
      } catch {
        // habitat may be mid-sync; the glow is best-effort
      }
    } else {
      scope.flashRow(w.id);
    }
    return;
  }
  try {
    await scope.invoke("locate_window", { id: w.id });
  } catch {
    scope.flashRow(w.id);
  }
}

export async function stow(scope: Scope_stow, w: WindowInfo) {
  try {
    await scope.invoke("minimize_window", { id: w.id });
  } catch {
    // app may refuse or exit; next tick reflects reality
  }
  await scope.tick();
}

export async function restore(scope: Scope_restore, w: WindowInfo) {
  try {
    await scope.invoke("restore_window", { id: w.id });
  } catch {}
  scope.touchClaim(w.id, true);
  await scope.tick();
}

export async function wakeSleeper(scope: Scope_wakeSleeper, w: WindowInfo) {
  scope.wakeNote = null;
  try {
    const msg = await scope.invoke<string>("wake_sleeper", { id: w.id });
    scope.wakeNote = { kind: "ok", text: msg };
  } catch (e) {
    scope.wakeNote = { kind: "err", text: `${e}` };
  }
  scope.touchClaim(w.id, true);
  await scope.tick();
}

export function nudgeClaim(scope: Scope_nudgeClaim, projectId: string) {
  if (!scope.nudgeFor) return;
  scope.claim(scope.nudgeFor, projectId);
  scope.nudgeFor = null;
}

export function nudgeCreate(scope: Scope_nudgeCreate) {
  if (!scope.nudgeFor || !scope.nudgeNewName.trim()) return;
  const p = scope.createProject(scope.nudgeNewName);
  scope.claim(scope.nudgeFor, p.id);
  scope.nudgeFor = null;
  scope.nudgeNewName = "";
}

export function clampSpriteScale(scope: Scope_clampSpriteScale, n: number) {
  if (!Number.isFinite(n)) return 1;
  return Math.min(2.5, Math.max(0.4, n));
}

export async function openKanban(scope: Scope_openKanban) {
  try {
    await scope.invoke("open_kanban");
  } catch {
    // window may already exist mid-creation; next click retries
  }
}

export async function bringAllFront(scope: Scope_bringAllFront) {
  try {
    await scope.invoke("bring_all_front");
  } catch {
    // no planets yet — harmless
  }
}

export function onBannerMouseDown(scope: Scope_onBannerMouseDown, e: MouseEvent) {
  if (e.button !== 0) return;
  const t = e.target as HTMLElement | null;
  if (t?.closest(".winx, .popout")) return;

  if (t?.closest(".wordmark")) {
    scope.wordmarkDidDrag = false;
    const sx = e.screenX;
    const sy = e.screenY;
    const onMove = (ev: MouseEvent) => {
      if (Math.hypot(ev.screenX - sx, ev.screenY - sy) < 5) return;
      scope.wordmarkDidDrag = true;
      window.removeEventListener("mousemove", onMove);
      scope.startPaneDrag(e);
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return;
  }

  scope.startPaneDrag(e);
}

export function onWordmarkClick(scope: Scope_onWordmarkClick) {
  if (scope.wordmarkDidDrag) return;
  scope.bringAllFront();
}

export async function openKanbanFocus(scope: Scope_openKanbanFocus, cardId: string) {
  await scope.openKanban();
  scope.emitTo("kanban", "orbitty:focus-card", cardId).catch(() => {});
  setTimeout(
    () => scope.emitTo("kanban", "orbitty:focus-card", cardId).catch(() => {}),
    400,
  );
}

export async function launchHermes(scope: Scope_launchHermes, p: Project) {
  const titles = [
    ...scope.rowsFor(p.id).map((w) =>
      w.title ? `${w.appName} — ${w.title}` : w.appName,
    ),
    ...scope.todosFor(p.id).map((c) => `To-do: ${c.title}`),
  ];
  try {
    const route = await scope.invoke<string>("launch_coordinator", {
      projectId: p.id,
      projectName: p.name,
      titles,
    });
    scope.hermesNote = { kind: "ok", text: `${p.name}: ${route}` };
  } catch (e) {
    scope.hermesNote = { kind: "err", text: `${e}` };
  }
}

export async function toggleSoul(scope: Scope_toggleSoul, p: Project) {
  if (scope.soulFor === p.id) {
    scope.soulFor = null;
    return;
  }
  scope.soulFor = p.id;
  scope.soulBody = "";
  scope.soulPath = "";
  scope.soulMsg = "Loading…";
  try {
    scope.soulBody = await scope.invoke<string>("load_soul", {
      projectId: p.id,
      projectName: p.name,
    });
    scope.soulPath = await scope.invoke<string>("soul_path", { projectId: p.id });
    scope.soulMsg = "";
  } catch (e) {
    scope.soulMsg = `${e}`;
  }
}

export async function saveSoul(scope: Scope_saveSoul) {
  if (!scope.soulFor) return;
  try {
    await scope.invoke("save_soul", { projectId: scope.soulFor, body: scope.soulBody });
    scope.soulMsg = "Saved.";
    setTimeout(() => {
      if (scope.soulMsg === "Saved.") scope.soulMsg = "";
    }, 1500);
  } catch (e) {
    scope.soulMsg = `${e}`;
  }
}

export async function openSoulEditor(scope: Scope_openSoulEditor) {
  if (!scope.soulFor) return;
  const p = scope.projects.find((x) => x.id === scope.soulFor);
  try {
    await scope.invoke("open_soul_in_editor", {
      projectId: scope.soulFor,
      projectName: p?.name ?? null,
    });
  } catch (e) {
    scope.soulMsg = `${e}`;
  }
}

export function createProject(scope: Scope_createProject, name: string): Project {
  const p: Project = {
    id: `p-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
    name: name.trim(),
  };
  scope.projects = [...scope.projects, p];
  // No walker yet: a marble, not an open planet. A claim or a todo opens it.
  if (!scope.collapsedSectionIds.includes(p.id)) {
    scope.collapsedSectionIds = [...scope.collapsedSectionIds, p.id];
  }
  return p;
}

export function revealPlanet(scope: Scope_revealPlanet, projectId: string) {
  const ids = scope.sectionIds();
  if (!ids.includes(projectId)) return;
  scope.collapsedSectionIds = [
    ...scope.collapsedSectionIds.filter((id) => !ids.includes(id)),
    ...ids.filter((id) => id !== projectId),
  ];
}

export function newCardId(scope: Scope_newCardId): string {
  return `c-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export function isClinicalOrMaster(scope: Scope_isClinicalOrMaster, windowId: string): boolean {
  return scope.tagsFor(windowId).some((t) => {
    const n = t.name.trim().toLowerCase();
    return n === "clinical" || n === "master";
  });
}

export function titleIndexFor(scope: Scope_titleIndexFor, w: WindowInfo): number {
  const bundle = w.bundleId || w.appName;
  return (
    scope.windows
      .filter(
        (x) => (x.bundleId || x.appName) === bundle && x.title === w.title,
      )
      .findIndex((x) => x.id === w.id) + 1
  );
}

export function claimStableKey(scope: Scope_claimStableKey, c: Claim): string | null {
  const bundle = c.bundleId || c.appName;
  if (!bundle || c.titleAtClaim == null || !c.titleIndex) return null;
  return `${bundle}|${c.titleAtClaim}|${c.titleIndex}`;
}

export function isHermesClass(scope: Scope_isHermesClass, w: WindowInfo): boolean {
  return /hermes/i.test(w.bundleId ?? "") || /hermes/i.test(w.appName);
}

export function sleeperStamps(scope: Scope_sleeperStamps, w: WindowInfo, prev?: Claim): Partial<Claim> {
  return {
    filePath: w.documentPath ?? prev?.filePath ?? null,
    hermesProfile: prev?.hermesProfile ?? null,
    hermesSession:
      scope.isHermesClass(w) && w.title
        ? w.title
        : (prev?.hermesSession ?? null),
    actionId: prev?.actionId ?? null,
    command: prev?.command ?? null,
    cwd: prev?.cwd ?? null,
  };
}

export function rollCharSeed(scope: Scope_rollCharSeed): number {
  return Math.floor(Math.random() * 0xffffffff) >>> 0;
}
