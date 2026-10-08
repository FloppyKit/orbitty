// @ts-nocheck
// Moved function bodies. scope fields are the lets each function already closed over.
export function titleFor(scope: any, w: WindowInfo): string {
  return (
    scope.titleOverrides.find((o) => o.windowId === w.id)?.title ||
    w.title ||
    w.appName
  );
}

export function startRename(scope: any, w: WindowInfo) {
  scope.renamingId = w.id;
  scope.renameValue =
    scope.titleOverrides.find((o) => o.windowId === w.id)?.title ?? w.title;
}

export function openCtxMenu(scope: any, e: MouseEvent, w: WindowInfo) {
  e.preventDefault();
  e.stopPropagation();
  // Clamp inside the rail so the menu never opens under the cursor's edge.
  const x = Math.min(e.clientX, window.innerWidth - 170);
  const y = Math.min(e.clientY, window.innerHeight - 220);
  scope.ctxMenu = { x, y, windowId: w.id };
  scope.ctxAssignOpen = false;
  scope.newProjectName = "";
}

export function ctxWindow(scope: any): WindowInfo | undefined {
  return scope.windows.find((w) => w.id === scope.ctxMenu?.windowId);
}

export function ctxProject(scope: any): Project | undefined {
  const id = scope.ctxMenu?.windowId;
  if (!id) return undefined;
  const projectId = scope.liveClaims.find((c) => c.windowId === id)?.projectId;
  return scope.projects.find((p) => p.id === projectId);
}

export function ctxAssign(scope: any, projectId: string) {
  const w = scope.ctxWindow();
  scope.ctxMenu = null;
  if (w) scope.claim(w.id, projectId);
}

export function ctxCreateAndClaim(scope: any) {
  const w = scope.ctxWindow();
  if (!w || !scope.newProjectName.trim()) return;
  const p = scope.createProject(scope.newProjectName);
  scope.ctxMenu = null;
  scope.claim(w.id, p.id);
  scope.newProjectName = "";
}

export function ctxRename(scope: any) {
  const w = scope.ctxWindow();
  scope.ctxMenu = null;
  if (w) scope.startRename(w);
}

export function ctxMinimize(scope: any) {
  const w = scope.ctxWindow();
  scope.ctxMenu = null;
  // Claimed → the walker walks its planet; unmapped → Dock only (the
  // habitat snapshot never sees unclaimed windows).
  if (w) scope.stow(w);
}

export function ctxRestore(scope: any) {
  const w = scope.ctxWindow();
  scope.ctxMenu = null;
  if (w) scope.restore(w);
}

export function ctxCloseWindow(scope: any) {
  const w = scope.ctxWindow();
  scope.ctxMenu = null;
  if (!w) return;
  scope.invoke("close_window", { id: w.id })
    .catch(() => {})
    .then(() => scope.tick());
}

export function ctxUnclaim(scope: any) {
  const w = scope.ctxWindow();
  scope.ctxMenu = null;
  if (w) scope.unclaim(w.id);
}

export function commitRename(scope: any, w: WindowInfo) {
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

export async function setWorkspaceRoot(scope: any, projectId: string, path: string) {
  try {
    await scope.invoke("set_workspace_root", { projectId, path });
    scope.projects = scope.projects.map((p) =>
      p.id === projectId ? { ...p, workspaceRoot: path.trim() || undefined } : p,
    );
  } catch {
    /* void refuses a root */
  }
}

export async function persist(scope: any) {
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

export function sleeperRowFor(scope: any, c: Claim): WindowInfo {
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

export function boardCardsFor(scope: any, p: Project) {
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

export function habitatEntryFor(scope: any, w: WindowInfo, p: Project) {
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

export function runIsSeated(scope: any, r: AgentRun): boolean {
  return (
    !!(r.agentShortcutId || "").trim() || !!(r.herdr?.agentName || "").trim()
  );
}

export function goToken(scope: any, title: string): string | null {
  const m = title.match(/GO-[A-Z0-9]+(?:-[A-Z0-9]+)*/i);
  return m ? m[0].toUpperCase() : null;
}

export function goNeededOnPlanet(scope: any, card: KanbanCard | undefined, planetId: string): boolean {
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

export function agentRowsFor(scope: any, projectId: string) {
  return scope.agentRuns.filter(
    (r) =>
      r.planetId === projectId &&
      r.state !== "done" &&
      scope.runIsSeated(r) &&
      scope.kanbanCards.some((k) => k.id === r.cardId),
  );
}

export async function locateAgent(scope: any, r: AgentRun) {
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

export function agentEntriesFor(scope: any, p: Project) {
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

export function habitatPlanetFor(scope: any, p: Project, rows: WindowInfo[]) {
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

export function habitatSnapshotPayload(scope: any, 
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

export async function syncHabitat(scope: any) {
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

export function lastKnownRowFor(scope: any, c: Claim): WindowInfo {
  return { ...scope.sleeperRowFor(c), sleeping: false };
}

export async function syncHabitatLastKnown(scope: any) {
  if (!scope.stateLoaded || embedded) return;
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

export function flashRow(scope: any, windowId: string) {
  if (scope.locateFlashIds.includes(windowId)) return;
  scope.locateFlashIds = [...scope.locateFlashIds, windowId];
  setTimeout(() => {
    scope.locateFlashIds = scope.locateFlashIds.filter((x) => x !== windowId);
  }, 550);
}

export async function locateRow(scope: any, w: WindowInfo) {
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

export async function stow(scope: any, w: WindowInfo) {
  try {
    await scope.invoke("minimize_window", { id: w.id });
  } catch {
    // app may refuse or exit; next tick reflects reality
  }
  await scope.tick();
}

export async function restore(scope: any, w: WindowInfo) {
  try {
    await scope.invoke("restore_window", { id: w.id });
  } catch {}
  scope.touchClaim(w.id, true);
  await scope.tick();
}

export async function wakeSleeper(scope: any, w: WindowInfo) {
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

export function nudgeClaim(scope: any, projectId: string) {
  if (!scope.nudgeFor) return;
  scope.claim(scope.nudgeFor, projectId);
  scope.nudgeFor = null;
}

export function nudgeCreate(scope: any) {
  if (!scope.nudgeFor || !scope.nudgeNewName.trim()) return;
  const p = scope.createProject(scope.nudgeNewName);
  scope.claim(scope.nudgeFor, p.id);
  scope.nudgeFor = null;
  scope.nudgeNewName = "";
}

export function clampSpriteScale(scope: any, n: number) {
  if (!Number.isFinite(n)) return 1;
  return Math.min(2.5, Math.max(0.4, n));
}

export async function openKanban(scope: any) {
  try {
    await scope.invoke("open_kanban");
  } catch {
    // window may already exist mid-creation; next click retries
  }
}

export async function bringAllFront(scope: any) {
  try {
    await scope.invoke("bring_all_front");
  } catch {
    // no planets yet — harmless
  }
}

export function onBannerMouseDown(scope: any, e: MouseEvent) {
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

export function onWordmarkClick(scope: any) {
  if (scope.wordmarkDidDrag) return;
  scope.bringAllFront();
}

export async function openKanbanFocus(scope: any, cardId: string) {
  await scope.openKanban();
  scope.emitTo("kanban", "orbitty:focus-card", cardId).catch(() => {});
  setTimeout(
    () => scope.emitTo("kanban", "orbitty:focus-card", cardId).catch(() => {}),
    400,
  );
}

export async function launchHermes(scope: any, p: Project) {
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

export async function toggleSoul(scope: any, p: Project) {
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

export async function saveSoul(scope: any) {
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

export async function openSoulEditor(scope: any) {
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

export function createProject(scope: any, name: string): Project {
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

export function revealPlanet(scope: any, projectId: string) {
  const ids = scope.sectionIds();
  if (!ids.includes(projectId)) return;
  scope.collapsedSectionIds = [
    ...scope.collapsedSectionIds.filter((id) => !ids.includes(id)),
    ...ids.filter((id) => id !== projectId),
  ];
}

export function newCardId(scope: any): string {
  return `c-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export function isClinicalOrMaster(scope: any, windowId: string): boolean {
  return scope.tagsFor(windowId).some((t) => {
    const n = t.name.trim().toLowerCase();
    return n === "clinical" || n === "master";
  });
}

export function titleIndexFor(scope: any, w: WindowInfo): number {
  const bundle = w.bundleId || w.appName;
  return (
    scope.windows
      .filter(
        (x) => (x.bundleId || x.appName) === bundle && x.title === w.title,
      )
      .findIndex((x) => x.id === w.id) + 1
  );
}

export function claimStableKey(scope: any, c: Claim): string | null {
  const bundle = c.bundleId || c.appName;
  if (!bundle || c.titleAtClaim == null || !c.titleIndex) return null;
  return `${bundle}|${c.titleAtClaim}|${c.titleIndex}`;
}

export function isHermesClass(scope: any, w: WindowInfo): boolean {
  return /hermes/i.test(w.bundleId ?? "") || /hermes/i.test(w.appName);
}

export function sleeperStamps(scope: any, w: WindowInfo, prev?: Claim): Partial<Claim> {
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

export function rollCharSeed(scope: any): number {
  return Math.floor(Math.random() * 0xffffffff) >>> 0;
}
