// Moved function bodies. scope fields are the lets each function already closed over.

import type { CharLook, Claim, KanbanCard, PointsEntry, Project, TitleOverride, WindowInfo } from "../persist";

interface BittyKind {
  name: string;
  slug: string;
  palettes: string[];
}

type Invoke = <T = unknown>(cmd: string, args?: any) => Promise<T>;

type Scope_claim = {
  nowEpoch: (...args: any[]) => number;
  windows: WindowInfo[];
  liveClaims: Claim[];
  claims: Claim[];
  rollCharSeed: (...args: any[]) => number;
  titleIndexFor: (...args: any[]) => number;
  noteClaims: (...args: any[]) => any;
  revealPlanet: (...args: any[]) => any;
  titleOverrides: TitleOverride[];
  kanbanCards: KanbanCard[];
  isClinicalOrMaster: (...args: any[]) => boolean;
  persist: (...args: any[]) => Promise<any>;
  sleeperStamps: (...args: any[]) => Partial<Claim>;
};

type Scope_unclaim = {
  claims: Claim[];
  liveClaims: Claim[];
  nowEpoch: (...args: any[]) => number;
  kanbanCards: KanbanCard[];
  persist: (...args: any[]) => Promise<any>;
};

type Scope_completeWindow = {
  nowEpoch: (...args: any[]) => number;
  isClinicalOrMaster: (...args: any[]) => boolean;
  titleOverrides: TitleOverride[];
  kanbanCards: KanbanCard[];
  newCardId: (...args: any[]) => string;
  persist: (...args: any[]) => Promise<any>;
  invoke: Invoke;
  tick: (...args: any[]) => Promise<any>;
  pointsToday: (...args: any[]) => number;
  DAILY_CAP: number;
  pointsLedger: PointsEntry[];
  todayKey: (...args: any[]) => string;
};

type Scope_completeTodo = {
  nowEpoch: (...args: any[]) => number;
  pointsLedger: PointsEntry[];
  pointsToday: (...args: any[]) => number;
  DAILY_CAP: number;
  kanbanCards: KanbanCard[];
  todayKey: (...args: any[]) => string;
  persist: (...args: any[]) => Promise<any>;
};

type Scope_circleComplete = {
  completingIds: string[];
};

type Scope_logEvent = {
  invoke: Invoke;
};

type Scope_addTodoTo = {
  newTodoTitle: string;
  nowEpoch: (...args: any[]) => number;
  newCardId: (...args: any[]) => string;
  kanbanCards: KanbanCard[];
  addingTodoFor: string | null;
  revealPlanet: (...args: any[]) => any;
  persist: (...args: any[]) => Promise<any>;
  logEvent: (...args: any[]) => any;
  openKanbanFocus: (...args: any[]) => Promise<any>;
};

type Scope_occupancyOf = {};

type Scope_setSpritePack = {
  claims: Claim[];
  liveClaims: Claim[];
  persist: (...args: any[]) => Promise<any>;
};

type Scope_beingClaim = {
  beingEditorFor: string | null;
  liveClaims: Claim[];
};

type Scope_beingWindow = {
  beingEditorFor: string | null;
  windows: WindowInfo[];
};

type Scope_refreshBeingPreview = {
  editorLook: CharLook | null;
  editorPreview: string;
  invoke: Invoke;
};

type Scope_commitBeingLook = {
  beingEditorFor: string | null;
  editorLook: CharLook | null;
  claims: Claim[];
  liveClaims: Claim[];
  persist: (...args: any[]) => Promise<any>;
  refreshBeingPreview: (...args: any[]) => Promise<any>;
};

type Scope_openBeingEditor = {
  liveClaims: Claim[];
  bittyKinds: BittyKind[];
  invoke: Invoke;
  rollCharSeed: (...args: any[]) => number;
  claims: Claim[];
  beingEditorFor: string | null;
  editorLook: CharLook | null;
  commitBeingLook: (...args: any[]) => Promise<any>;
};

type Scope_closeBeingEditor = {
  beingEditorFor: string | null;
  editorLook: CharLook | null;
  editorPreview: string;
};

type Scope_beingKindIndex = {
  editorLook: CharLook | null;
};

type Scope_beingPaletteIndex = {
  editorLook: CharLook | null;
};

type Scope_cycleKind = {
  editorLook: CharLook | null;
  bittyKinds: BittyKind[];
  beingKindIndex: (...args: any[]) => number;
  beingPaletteIndex: (...args: any[]) => number;
  commitBeingLook: (...args: any[]) => Promise<any>;
};

type Scope_cyclePalette = {
  editorLook: CharLook | null;
  bittyKinds: BittyKind[];
  beingKindIndex: (...args: any[]) => number;
  beingPaletteIndex: (...args: any[]) => number;
  commitBeingLook: (...args: any[]) => Promise<any>;
};

type Scope_kindLabel = {
  bittyKinds: BittyKind[];
  beingKindIndex: (...args: any[]) => number;
};

type Scope_paletteLabel = {
  bittyKinds: BittyKind[];
  beingKindIndex: (...args: any[]) => number;
  beingPaletteIndex: (...args: any[]) => number;
};

type Scope_rerollBeing = {
  rollCharSeed: (...args: any[]) => number;
  editorLook: CharLook | null;
  invoke: Invoke;
  commitBeingLook: (...args: any[]) => Promise<any>;
};

type Scope_beingScale = {
  beingClaim: (...args: any[]) => Claim | undefined;
  spriteScale: number;
};

type Scope_onBeingScaleInput = {
  beingEditorFor: string | null;
  clampSpriteScale: (...args: any[]) => any;
  claims: Claim[];
  liveClaims: Claim[];
  lastHabitatSig: string;
  syncHabitat: (...args: any[]) => Promise<any>;
};

type Scope_onBeingScaleCommit = {
  persist: (...args: any[]) => Promise<any>;
};

type Scope_resetBeingScale = {
  beingEditorFor: string | null;
  claims: Claim[];
  liveClaims: Claim[];
  persist: (...args: any[]) => Promise<any>;
};

type Scope_beingMoveHome = {
  beingEditorFor: string | null;
  claim: (...args: any[]) => any;
};

type Scope_beingUnclaim = {
  beingEditorFor: string | null;
  unclaim: (...args: any[]) => any;
  closeBeingEditor: (...args: any[]) => any;
};

type Scope_createProjectOnly = {
  newProjectName: string;
  createProject: (...args: any[]) => Project;
  creatingProject: boolean;
  embedded: boolean;
  sheetSlot: string;
  persist: (...args: any[]) => Promise<any>;
  invoke: Invoke;
};

type Scope_armRemove = {
  rmArmedFor: string | null;
  rmErrorFor: { id: string; msg: string } | null;
};

type Scope_removeProject = {
  invoke: Invoke;
  rmArmedFor: string | null;
  rmErrorFor: { id: string; msg: string } | null;
  projects: Project[];
  sheetSlot: string;
  syncHabitat: (...args: any[]) => Promise<any>;
};

export function claim(scope: Scope_claim, windowId: string, projectId: string) {
  const now = scope.nowEpoch();
  const w = scope.windows.find((x) => x.id === windowId);
  // The sprite pick / generated being / per-walker size are per-window
  // preferences — re-claiming into another project keeps them; a
  // first-time claim rolls a fresh person.
  const prev = scope.liveClaims.find((c) => c.windowId === windowId);
  scope.claims = [
    ...scope.liveClaims.filter((c) => c.windowId !== windowId),
    {
      windowId,
      projectId,
      lastTouchedAt: now,
      spritePackId: prev?.spritePackId ?? null,
      charSeed: prev?.charSeed ?? scope.rollCharSeed(),
      charLook: prev?.charLook ?? null,
      spriteScale: prev?.spriteScale ?? null,
      // Walker Assign is this same door. A rebuilt row must keep `tab`
      // or the claim becomes a dead tab: id with no url.
      tab: prev?.tab ?? null,
      // Claiming (rail Assign or walker
      // Assign-to-planet — same door) clears any leftover teleport
      // home. Occupancy is projectId only.
      walkerHome: null,
      ...(w
        ? {
            bundleId: w.bundleId ?? null,
            appName: w.appName,
            titleAtClaim: w.title,
            titleIndex: scope.titleIndexFor(w),
            ...scope.sleeperStamps(w, prev),
          }
        : {}),
    },
  ];
  scope.noteClaims(scope.claims);
  scope.revealPlanet(projectId);
  // Bare claim = grass only. A board card is born only when a todo is
  // assigned to this window. If a non-done card already names it, keep
  // the link and retarget the project — never force status back to todo.
  const shown = w
    ? scope.titleOverrides.find((o) => o.windowId === w.id)?.title || w.title
    : "";
  const title = w ? (shown ? `${w.appName} — ${shown}` : w.appName) : "Window";
  const open = scope.kanbanCards.find(
    (c) => c.windowId === windowId && c.status !== "done",
  );
  if (open) {
    scope.kanbanCards = scope.kanbanCards.map((c) =>
      c.id === open.id
        ? {
            ...c,
            projectId,
            windowId,
            title: c.title || title,
            clinicalOrMaster: c.clinicalOrMaster || scope.isClinicalOrMaster(windowId),
            lastTouchedAt: now,
          }
        : c,
    );
  }
  scope.persist();
}

export function unclaim(scope: Scope_unclaim, windowId: string) {
  scope.claims = scope.liveClaims.filter((c) => c.windowId !== windowId);
  const now = scope.nowEpoch();
  scope.kanbanCards = scope.kanbanCards.map((c) =>
    c.windowId === windowId && c.status !== "done"
      ? { ...c, windowId: null, status: "todo", lastTouchedAt: now }
      : c,
  );
  scope.persist();
}

export async function completeWindow(scope: Scope_completeWindow, w: WindowInfo, project: Project) {
  const now = scope.nowEpoch();
  const clinicalOrMaster = scope.isClinicalOrMaster(w.id);
  const shown = scope.titleOverrides.find((o) => o.windowId === w.id)?.title || w.title;
  const title = shown ? `${w.appName} — ${shown}` : w.appName;
  const existing =
    scope.kanbanCards.find(
      (c) => c.windowId === w.id && c.projectId === project.id,
    ) ?? scope.kanbanCards.find((c) => c.windowId === w.id);
  // Board walk-off only when a real todo is bound (kind todo, or any
  // non-window kind with a windowId). Bare claim / legacy kind:window
  // tickets: close + unclaim, no Done parade to the sign.
  const realTodo =
    !!existing &&
    existing.kind !== "window" &&
    existing.kind !== "needed" &&
    existing.status !== "done";
  if (!realTodo) {
    // Bare claim exit: kind:window Done keeps the claim so the walker
    // can walk home/off (native skips the board lip for non-todo).
    // No instant vanish; no sign checkout.
    if (existing) {
      scope.kanbanCards = scope.kanbanCards.map((c) =>
        c.id === existing.id
          ? {
              ...c,
              kind: c.kind === "todo" ? c.kind : ("window" as const),
              status: "done",
              windowId: w.id,
              title: c.title || title,
              lastTouchedAt: now,
              completedAt: c.completedAt ?? now,
            }
          : c,
      );
    } else {
      scope.kanbanCards = [
        ...scope.kanbanCards,
        {
          id: scope.newCardId(),
          title,
          projectId: project.id,
          windowId: w.id,
          kind: "window" as const,
          status: "done" as const,
          createdAt: now,
          lastTouchedAt: now,
          completedAt: now,
          goalAligned: true,
          clinicalOrMaster,
          pointsAwarded: 0,
          starred: false,
        },
      ];
    }
    scope.persist();
    try {
      await scope.invoke("close_window", { id: w.id });
    } catch {}
    await scope.tick();
    return;
  }
  // Award once per card (first transition to done only); 1 pt default,
  // 2 if tagged clinical/master; 0 when the daily cap would be exceeded —
  // the card still completes, quietly.
  const firstComplete = !existing?.completedAt;
  const base = clinicalOrMaster ? 2 : 1;
  const award = firstComplete && scope.pointsToday() + base <= scope.DAILY_CAP ? base : 0;
  const cardId = existing!.id;
  scope.kanbanCards = scope.kanbanCards.map((c) =>
    c.id === cardId
      ? {
          ...c,
          status: "done",
          title: c.title || title,
          clinicalOrMaster: c.clinicalOrMaster || clinicalOrMaster,
          lastTouchedAt: now,
          completedAt: c.completedAt ?? now,
          pointsAwarded: c.pointsAwarded + award,
        }
      : c,
  );
  if (award > 0) {
    scope.pointsLedger = [
      ...scope.pointsLedger,
      {
        id: `pt-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
        cardId,
        amount: award,
        day: scope.todayKey(),
        at: now,
      },
    ];
  }
  // Real todo: claim SURVIVES so the walker can walk to the board then
  // off — habitat:walked-off unclaims once the walker is gone.
  scope.persist();
  try {
    await scope.invoke("close_window", { id: w.id });
  } catch {}
  await scope.tick();
}

export function completeTodo(scope: Scope_completeTodo, card: KanbanCard) {
  const now = scope.nowEpoch();
  const alreadyAwarded = scope.pointsLedger.some((e) => e.cardId === card.id);
  const base = card.clinicalOrMaster ? 2 : 1;
  const award =
    !alreadyAwarded && scope.pointsToday() + base <= scope.DAILY_CAP ? base : 0;
  scope.kanbanCards = scope.kanbanCards.map((c) =>
    c.id === card.id
      ? {
          ...c,
          status: "done",
          lastTouchedAt: now,
          completedAt: now,
          pointsAwarded: c.pointsAwarded + award,
        }
      : c,
  );
  if (award > 0) {
    scope.pointsLedger = [
      ...scope.pointsLedger,
      {
        id: `pt-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
        cardId: card.id,
        amount: award,
        day: scope.todayKey(),
        at: now,
      },
    ];
  }
  scope.persist();
}

export function circleComplete(scope: Scope_circleComplete, key: string, fn: () => void) {
  if (scope.completingIds.includes(key)) return;
  scope.completingIds = [...scope.completingIds, key];
  setTimeout(() => {
    scope.completingIds = scope.completingIds.filter((k) => k !== key);
    fn();
  }, 240);
}

export function logEvent(scope: Scope_logEvent, eType: string, content: Record<string, unknown>, projectId?: string) {
  scope.invoke("log_event", {
    eType,
    content,
    projectId: projectId || null,
    actor: "user",
  }).catch(() => {});
}

export function addTodoTo(scope: Scope_addTodoTo, projectId: string) {
  const title = scope.newTodoTitle.trim();
  if (!title) return;
  const now = scope.nowEpoch();
  const id = scope.newCardId();
  scope.kanbanCards = [
    ...scope.kanbanCards,
    {
      id,
      title,
      projectId,
      windowId: null,
      kind: "todo" as const,
      status: "todo" as const,
      createdAt: now,
      lastTouchedAt: now,
      completedAt: null,
      goalAligned: true,
      clinicalOrMaster: false,
      pointsAwarded: 0,
      starred: false,
    },
  ];
  scope.newTodoTitle = "";
  scope.addingTodoFor = null;
  scope.revealPlanet(projectId);
  scope.persist();
  scope.logEvent("board-add", { cardId: id, title }, projectId);
  scope.openKanbanFocus(id);
}

export function occupancyOf(scope: Scope_occupancyOf, c: Claim): string {
  return c.projectId;
}

export function setSpritePack(scope: Scope_setSpritePack, windowId: string, packId: string | null) {
  scope.claims = scope.liveClaims.map((c) =>
    c.windowId === windowId ? { ...c, spritePackId: packId || null } : c,
  );
  scope.persist();
}

export function beingClaim(scope: Scope_beingClaim): Claim | undefined {
  return scope.beingEditorFor
    ? scope.liveClaims.find((c) => c.windowId === scope.beingEditorFor)
    : undefined;
}

export function beingWindow(scope: Scope_beingWindow): WindowInfo | undefined {
  return scope.beingEditorFor
    ? scope.windows.find((w) => w.id === scope.beingEditorFor)
    : undefined;
}

export async function refreshBeingPreview(scope: Scope_refreshBeingPreview) {
  if (!scope.editorLook) {
    scope.editorPreview = "";
    return;
  }
  try {
    const b64 = await scope.invoke<string>("char_preview", {
      look: scope.editorLook,
      px: 8,
    });
    scope.editorPreview = `data:image/png;base64,${b64}`;
  } catch {
    scope.editorPreview = ""; // preview is decorative; the planet is the truth
  }
}

export async function commitBeingLook(scope: Scope_commitBeingLook, seed?: number) {
  const id = scope.beingEditorFor;
  if (!id || !scope.editorLook) return;
  scope.claims = scope.liveClaims.map((c) =>
    c.windowId === id
      ? { ...c, charLook: { ...scope.editorLook! }, ...(seed != null ? { charSeed: seed } : {}) }
      : c,
  );
  scope.persist();
  await scope.refreshBeingPreview();
}

export async function openBeingEditor(scope: Scope_openBeingEditor, windowId: string) {
  const c = scope.liveClaims.find((x) => x.windowId === windowId);
  if (!c) return; // only claimed windows have beings
  if (scope.bittyKinds.length === 0) {
    try {
      scope.bittyKinds = await scope.invoke<BittyKind[]>("bitty_catalog");
    } catch {
      scope.bittyKinds = [];
    }
  }
  // Materialize: a claim without explicit picks derives them from its
  // seed once, so the cyclers edit concrete indices from then on.
  let seed = c.charSeed ?? 0;
  if (!seed) {
    seed = scope.rollCharSeed();
    scope.claims = scope.liveClaims.map((x) =>
      x.windowId === windowId ? { ...x, charSeed: seed } : x,
    );
  }
  let look = c.charLook;
  if (!look) {
    try {
      look = await scope.invoke<CharLook>("roll_char_look", { seed });
    } catch {
      look = { species: 0, body: 0, clothes: 0, eyes: 0, mouth: 0, hat: 0, colors: 0, kind: 0, palette: 0 };
    }
  }
  // Pre-S1 looks carry no kind/palette — the native side maps the legacy
  // species onto the nearest Kind+Palette so the cyclers start concrete.
  try {
    look = await scope.invoke<CharLook>("normalize_char_look", { look });
  } catch {
    // keep the un-normalized look; the planet still resolves it
  }
  scope.beingEditorFor = windowId;
  scope.editorLook = { ...look };
  await scope.commitBeingLook();
}

export function closeBeingEditor(scope: Scope_closeBeingEditor) {
  scope.beingEditorFor = null;
  scope.editorLook = null;
  scope.editorPreview = "";
}

export function beingKindIndex(scope: Scope_beingKindIndex): number {
  return scope.editorLook?.kind ?? 0;
}

export function beingPaletteIndex(scope: Scope_beingPaletteIndex): number {
  return scope.editorLook?.palette ?? 0;
}

export function cycleKind(scope: Scope_cycleKind, dir: 1 | -1) {
  if (!scope.editorLook || scope.bittyKinds.length === 0) return;
  const n = scope.bittyKinds.length;
  const k = (((scope.beingKindIndex() + dir) % n) + n) % n;
  const p = scope.beingPaletteIndex() % (scope.bittyKinds[k]?.palettes.length || 1);
  scope.editorLook = { ...scope.editorLook, kind: k, palette: p };
  scope.commitBeingLook();
}

export function cyclePalette(scope: Scope_cyclePalette, dir: 1 | -1) {
  if (!scope.editorLook || scope.bittyKinds.length === 0) return;
  const n = scope.bittyKinds[scope.beingKindIndex()]?.palettes.length || 1;
  const p = (((scope.beingPaletteIndex() + dir) % n) + n) % n;
  scope.editorLook = { ...scope.editorLook, palette: p };
  scope.commitBeingLook();
}

export function kindLabel(scope: Scope_kindLabel): string {
  return scope.bittyKinds[scope.beingKindIndex()]?.name ?? "";
}

export function paletteLabel(scope: Scope_paletteLabel): string {
  return scope.bittyKinds[scope.beingKindIndex()]?.palettes[scope.beingPaletteIndex()] ?? "";
}

export async function rerollBeing(scope: Scope_rerollBeing) {
  const seed = scope.rollCharSeed();
  try {
    scope.editorLook = await scope.invoke<CharLook>("roll_char_look", { seed });
  } catch {
    return;
  }
  await scope.commitBeingLook(seed);
}

export function beingScale(scope: Scope_beingScale): number {
  return scope.beingClaim()?.spriteScale ?? scope.spriteScale;
}

export function onBeingScaleInput(scope: Scope_onBeingScaleInput, e: Event) {
  const id = scope.beingEditorFor;
  if (!id) return;
  const v = scope.clampSpriteScale(+(e.target as HTMLInputElement).value);
  scope.claims = scope.liveClaims.map((c) =>
    c.windowId === id ? { ...c, spriteScale: v } : c,
  );
  scope.lastHabitatSig = "";
  scope.syncHabitat();
}

export function onBeingScaleCommit(scope: Scope_onBeingScaleCommit) {
  scope.persist();
}

export function resetBeingScale(scope: Scope_resetBeingScale) {
  const id = scope.beingEditorFor;
  if (!id) return;
  scope.claims = scope.liveClaims.map((c) =>
    c.windowId === id ? { ...c, spriteScale: null } : c,
  );
  scope.persist();
}

export function beingMoveHome(scope: Scope_beingMoveHome, projectId: string) {
  const id = scope.beingEditorFor;
  if (!id) return;
  scope.claim(id, projectId);
}

export function beingUnclaim(scope: Scope_beingUnclaim) {
  const id = scope.beingEditorFor;
  if (!id) return;
  scope.unclaim(id);
  scope.closeBeingEditor();
}

export async function createProjectOnly(scope: Scope_createProjectOnly) {
  if (!scope.newProjectName.trim()) return;
  const p = scope.createProject(scope.newProjectName);
  scope.newProjectName = "";
  scope.creatingProject = false;
  if (scope.embedded) scope.sheetSlot = p.id;
  // Marble is already in the snapshot. Lift it so it is visible when
  // planets are not muted. No locate glow — that pops the name.
  await scope.persist();
  scope.invoke("bring_planet_front", { id: p.id }).catch(() => {});
  setTimeout(() => scope.invoke("bring_planet_front", { id: p.id }).catch(() => {}), 50);
}

export function armRemove(scope: Scope_armRemove, id: string) {
  scope.rmArmedFor = id;
  scope.rmErrorFor = null;
}

export async function removeProject(scope: Scope_removeProject, p: Project) {
  try {
    await scope.invoke("project_rm", { id: p.id });
  } catch (e) {
    scope.rmArmedFor = null;
    scope.rmErrorFor = { id: p.id, msg: `${e}` };
    return;
  }
  scope.rmArmedFor = null;
  scope.rmErrorFor = null;
  scope.projects = scope.projects.filter((x) => x.id !== p.id);
  if (scope.sheetSlot === p.id) scope.sheetSlot = "unclaimed";
  scope.syncHabitat();
}
