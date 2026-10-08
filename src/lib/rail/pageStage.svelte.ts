// @ts-nocheck
// Moved function bodies. scope fields are the lets each function already closed over.
export function stageRole(scope: any, pid: string): StageRole {
  return scope.stageRoles(scope.sectionIds(), scope.openSectionId()).find((r) => r.id === pid)?.role ?? "icon";
}

export function ensureExpand(scope: any, pid: string) {
  const ids = scope.sectionIds();
  if (!ids.includes(pid)) return;
  const nextOpen = scope.applyPlanetOpen(scope.openSectionId(), ids, pid);
  scope.collapsedSectionIds = nextOpen ? ids.filter((id) => id !== nextOpen) : [...ids];
  scope.persist();
  void scope.showStage();
}

export function collapsePlanet(scope: any, pid: string) {
  if (!scope.sectionIds().includes(pid) || scope.isCollapsed(pid)) return;
  scope.collapsedSectionIds = [...scope.collapsedSectionIds, pid];
  scope.persist();
}

export function currentBeside(scope: any): Beside {
  if (scope.stageEmbed === "stars" || scope.stageEmbed === "settings") return scope.stageEmbed;
  if (scope.arch.front === "board") return "board";
  if (scope.arch.pid === scope.UNMAPPED_KEY && scope.arch.front !== "arch") return "unmapped";
  return null;
}

export function hideBesidePopouts(scope: any) {
  void scope.invoke("hide_window", { label: "stars-beside" }).catch(() => {});
  void scope.invoke("hide_window", { label: "settings-beside" }).catch(() => {});
}

export function clearArchToFront(scope: any) {
  if (scope.arch.front === "arch") return;
  let s = scope.arch;
  if (s.front === "board") s = scope.archBack(s);
  if (s.front === "layer2") s = scope.archBack(s);
  scope.arch = s;
}

export function ownerStop(scope: any, owner: KeyOwner): string | null {
  if (owner === "stars") return "rstars";
  if (owner === "settings") return "rsettings";
  if (owner === "unmapped") return `rhead-${scope.UNMAPPED_KEY}`;
  if (owner === "board") return "rl2-board";
  return null;
}

export function focusMkey(scope: any, id: string) {
  queueMicrotask(() => {
    const icon = document.querySelector(`[data-mkey="${CSS.escape(id)}"]`);
    if (icon instanceof HTMLElement) icon.focus();
  });
}

export function releaseKeysToRail(scope: any) {
  if (scope.keyOwner === "rail") return;
  scope.keyOwner = scope.keyOwnerNext(scope.keyOwner, { type: "select", other: "rail" });
  scope.reclaimRailRing();
}

export function openStageDest(scope: any, label: "stars" | "settings") {
  scope.applyBeside(label);
}

export function popOutStars(scope: any) {
  if (scope.keyOwner === "stars") scope.walkOut();
  scope.stageEmbed = null;
  void scope.invoke("rail_stage_set", { revealed: false });
  void scope.invoke("dest_show", { dest: "stars-beside" });
}

export function applyBeside(scope: any, pick: BesidePick) {
  const open = scope.currentBeside();
  const next = scope.besideNext(open, pick);
  if (next === open) return;
  scope.hideBesidePopouts();
  scope.releaseKeysToRail();
  if (scope.besideFront(pick) === "arch") scope.clearArchToFront();
  if (next === "settings") scope.stageEmbed = "settings";
  else if (next === "stars") scope.stageEmbed = "stars";
  else scope.stageEmbed = null;
}

export function walkInto(scope: any, target: WalkTarget) {
  scope.applyBeside(target);
  scope.reselectStop = null;
  scope.keyOwner = scope.keyOwnerNext(scope.keyOwner, { type: "walkIn", target });
}

export function walkOut(scope: any) {
  const fromBoard = scope.keyOwner === "board" && scope.arch.front === "board";
  scope.reselectStop = scope.ownerStop(scope.keyOwner);
  scope.keyOwner = scope.keyOwnerNext(scope.keyOwner, { type: "walkOut" });
  if (fromBoard) scope.archDo({ ...scope.arch, front: "layer2", pending: null });
  scope.reclaimRailRing();
}

export function closeStageEmbed(scope: any) {
  const which = scope.stageEmbed;
  scope.stageEmbed = null;
  if (which && scope.keyOwner === which) {
    scope.keyOwner = scope.keyOwnerNext(scope.keyOwner, { type: "panelClosed" });
    scope.reselectStop = which === "stars" ? "rstars" : "rsettings";
    scope.reclaimRailRing();
    return;
  }
  if (which) {
    const id = which === "stars" ? "rstars" : "rsettings";
    queueMicrotask(() => {
      scope.menuRing.select(id);
      scope.focusMkey(id);
    });
  }
}

export function popOutSettings(scope: any) {
  if (scope.keyOwner === "settings") scope.walkOut();
  scope.stageEmbed = null;
  void scope.invoke("rail_stage_set", { revealed: false });
  void scope.invoke("dest_show", { dest: "settings-beside" });
}

export function popOutBoard(scope: any) {
  void scope.invoke("dest_show", { dest: "launchpad" });
}

export function coachIsDue(scope: any): boolean {
  return scope.coachDue({
    trusted: !!scope.status?.trusted,
    stateLoaded: scope.stateLoaded,
    claimsCount: scope.claims.length,
    story: scope.starterStory,
    embedded,
  });
}

export function maybeRevealCoach(scope: any) {
  if (!scope.bootParkDone || scope.coachRevealed || embedded) return;
  if (!scope.coachIsDue()) return;
  scope.coachRevealed = true;
  void scope.showStage();
}

export function noteAx(scope: any, s: AxStatus) {
  scope.status = s;
  scope.maybeRevealCoach();
}

export async function coachClaim(scope: any) {
  scope.coachError = "";
  try {
    await scope.invoke("claim_front_window");
  } catch (e) {
    const text = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
    if (text.includes("cancelled")) return;
    scope.coachError = text;
    if (scope.coachErrorTimer) clearTimeout(scope.coachErrorTimer);
    scope.coachErrorTimer = setTimeout(() => {
      scope.coachError = "";
    }, 6000);
  }
}

export async function coachSkip(scope: any) {
  try {
    await scope.invoke("set_starter_coach_done");
  } catch {
    /* the flag lands on the next state:changed */
  }
}

export async function showStage(scope: any) {
  if (embedded) return;
  scope.stageRevealed = true;
  try {
    await scope.invoke("rail_stage_set", { revealed: true });
  } catch {
    /* the strip still paints if the native place is unavailable */
  }
}

export async function parkStage(scope: any) {
  if (embedded) return;
  scope.stageRevealed = false;
  try {
    await scope.invoke("rail_stage_set", { revealed: false });
  } catch {
    /* css parks the strip */
  }
}

export function scheduleGraceRecheck(scope: any) {
  if (scope.graceRecheck) return;
  const wait = Math.max(16, scope.revealGraceUntil + 16 - Date.now());
  scope.graceRecheck = setTimeout(() => {
    scope.graceRecheck = null;
    scope.maybeAutoHide();
  }, wait);
}

export function maybeAutoHide(scope: any) {
  if (embedded || !scope.stageRevealed) return;
  if (Date.now() < scope.revealGraceUntil) {
    scope.scheduleGraceRecheck();
    return;
  }
  if (!scope.autoHideWanted({ railAutoHideOnBlur: scope.railAutoHideOnBlur, focusInside: scope.focusInside, pointerInside: scope.pointerInside })) return;
  void scope.parkStage();
}

export function parkAfterRaise(scope: any) {
  void scope.invoke("rail_stage_set", { revealed: false, reason: "raised" });
}

export function armNeededMouth(scope: any, item: { kind?: string; id?: string; title?: string }) {
  const act = scope.neededAct(item);
  if (!act) return;
  void scope.invoke("arm_companion_mouth", { id: act.id, title: act.title });
}

export function stageFocus(scope: any, inside: boolean) {
  if (embedded) return;
  scope.focusInside = inside;
  if (!inside) scope.maybeAutoHide();
}

export function previewStop(scope: any, id: string | null): boolean {
  if (!id) return false;
  return (
    id.startsWith("rraise-") ||
    id.startsWith("rtile-") ||
    id.startsWith("rsnap-") ||
    id.startsWith("rtodos-")
  );
}
