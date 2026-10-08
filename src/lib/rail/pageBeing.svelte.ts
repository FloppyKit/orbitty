// @ts-nocheck
// Moved function bodies. scope fields are the lets each function already closed over.
export function claim(scope: any, windowId: string, projectId: string) {
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

export function unclaim(scope: any, windowId: string) {
  scope.claims = scope.liveClaims.filter((c) => c.windowId !== windowId);
  const now = scope.nowEpoch();
  scope.kanbanCards = scope.kanbanCards.map((c) =>
    c.windowId === windowId && c.status !== "done"
      ? { ...c, windowId: null, status: "todo", lastTouchedAt: now }
      : c,
  );
  scope.persist();
}

export async function completeWindow(scope: any, w: WindowInfo, project: Project) {
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

export function completeTodo(scope: any, card: KanbanCard) {
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

export function circleComplete(scope: any, key: string, fn: () => void) {
  if (scope.completingIds.includes(key)) return;
  scope.completingIds = [...scope.completingIds, key];
  setTimeout(() => {
    scope.completingIds = scope.completingIds.filter((k) => k !== key);
    fn();
  }, 240);
}

export function logEvent(scope: any, eType: string, content: Record<string, unknown>, projectId?: string) {
  scope.invoke("log_event", {
    eType,
    content,
    projectId: projectId || null,
    actor: "user",
  }).catch(() => {});
}

export function addTodoTo(scope: any, projectId: string) {
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

export function occupancyOf(scope: any, c: Claim): string {
  return c.projectId;
}

export function setSpritePack(scope: any, windowId: string, packId: string | null) {
  scope.claims = scope.liveClaims.map((c) =>
    c.windowId === windowId ? { ...c, spritePackId: packId || null } : c,
  );
  scope.persist();
}

export function beingClaim(scope: any): Claim | undefined {
  return scope.beingEditorFor
    ? scope.liveClaims.find((c) => c.windowId === scope.beingEditorFor)
    : undefined;
}

export function beingWindow(scope: any): WindowInfo | undefined {
  return scope.beingEditorFor
    ? scope.windows.find((w) => w.id === scope.beingEditorFor)
    : undefined;
}

export async function refreshBeingPreview(scope: any) {
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

export async function commitBeingLook(scope: any, seed?: number) {
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

export async function openBeingEditor(scope: any, windowId: string) {
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

export function closeBeingEditor(scope: any) {
  scope.beingEditorFor = null;
  scope.editorLook = null;
  scope.editorPreview = "";
}

export function beingKindIndex(scope: any): number {
  return scope.editorLook?.kind ?? 0;
}

export function beingPaletteIndex(scope: any): number {
  return scope.editorLook?.palette ?? 0;
}

export function cycleKind(scope: any, dir: 1 | -1) {
  if (!scope.editorLook || scope.bittyKinds.length === 0) return;
  const n = scope.bittyKinds.length;
  const k = (((scope.beingKindIndex() + dir) % n) + n) % n;
  const p = scope.beingPaletteIndex() % (scope.bittyKinds[k]?.palettes.length || 1);
  scope.editorLook = { ...scope.editorLook, kind: k, palette: p };
  scope.commitBeingLook();
}

export function cyclePalette(scope: any, dir: 1 | -1) {
  if (!scope.editorLook || scope.bittyKinds.length === 0) return;
  const n = scope.bittyKinds[scope.beingKindIndex()]?.palettes.length || 1;
  const p = (((scope.beingPaletteIndex() + dir) % n) + n) % n;
  scope.editorLook = { ...scope.editorLook, palette: p };
  scope.commitBeingLook();
}

export function kindLabel(scope: any): string {
  return scope.bittyKinds[scope.beingKindIndex()]?.name ?? "";
}

export function paletteLabel(scope: any): string {
  return scope.bittyKinds[scope.beingKindIndex()]?.palettes[scope.beingPaletteIndex()] ?? "";
}

export async function rerollBeing(scope: any) {
  const seed = scope.rollCharSeed();
  try {
    scope.editorLook = await scope.invoke<CharLook>("roll_char_look", { seed });
  } catch {
    return;
  }
  await scope.commitBeingLook(seed);
}

export function beingScale(scope: any): number {
  return scope.beingClaim()?.spriteScale ?? scope.spriteScale;
}

export function onBeingScaleInput(scope: any, e: Event) {
  const id = scope.beingEditorFor;
  if (!id) return;
  const v = scope.clampSpriteScale(+(e.target as HTMLInputElement).value);
  scope.claims = scope.liveClaims.map((c) =>
    c.windowId === id ? { ...c, spriteScale: v } : c,
  );
  scope.lastHabitatSig = "";
  scope.syncHabitat();
}

export function onBeingScaleCommit(scope: any) {
  scope.persist();
}

export function resetBeingScale(scope: any) {
  const id = scope.beingEditorFor;
  if (!id) return;
  scope.claims = scope.liveClaims.map((c) =>
    c.windowId === id ? { ...c, spriteScale: null } : c,
  );
  scope.persist();
}

export function beingMoveHome(scope: any, projectId: string) {
  const id = scope.beingEditorFor;
  if (!id) return;
  scope.claim(id, projectId);
}

export function beingUnclaim(scope: any) {
  const id = scope.beingEditorFor;
  if (!id) return;
  scope.unclaim(id);
  scope.closeBeingEditor();
}

export async function createProjectOnly(scope: any) {
  if (!scope.newProjectName.trim()) return;
  const p = scope.createProject(scope.newProjectName);
  scope.newProjectName = "";
  scope.creatingProject = false;
  if (embedded) scope.sheetSlot = p.id;
  // Marble is already in the snapshot. Lift it so it is visible when
  // planets are not muted. No locate glow — that pops the name.
  await scope.persist();
  scope.invoke("bring_planet_front", { id: p.id }).catch(() => {});
  setTimeout(() => scope.invoke("bring_planet_front", { id: p.id }).catch(() => {}), 50);
}

export function armRemove(scope: any, id: string) {
  scope.rmArmedFor = id;
  scope.rmErrorFor = null;
}

export async function removeProject(scope: any, p: Project) {
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
