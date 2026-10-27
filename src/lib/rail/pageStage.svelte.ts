// Moved function bodies. scope fields are the lets each function already closed over.

import type { Claim } from "../persist";
import type { MenuRing } from "../menuKeys.svelte";
import type { ArchState } from "./stageArch";
import type { Beside, BesideFront, BesidePick, KeyOwner, WalkTarget } from "./stageFocus";
import type { StageRole } from "./habitat";
import type { CoachStory } from "./coach";

interface AxStatus {
  trusted: boolean;
  bundlePath: string;
  signingLine: string;
  inBundle: boolean;
  isAdhoc: boolean;
}

type Invoke = <T = unknown>(cmd: string, args?: any) => Promise<T>;

type Scope_stageRole = {
  stageRoles: (...args: any[]) => { id: string; role: StageRole }[];
  sectionIds: (...args: any[]) => string[];
  openSectionId: (...args: any[]) => string | null;
};

type Scope_ensureExpand = {
  sectionIds: (...args: any[]) => string[];
  applyPlanetOpen: (...args: any[]) => string | null;
  openSectionId: (...args: any[]) => string | null;
  collapsedSectionIds: string[];
  persist: (...args: any[]) => Promise<any>;
  showStage: (...args: any[]) => Promise<any>;
};

type Scope_collapsePlanet = {
  sectionIds: (...args: any[]) => string[];
  isCollapsed: (...args: any[]) => boolean;
  collapsedSectionIds: string[];
  persist: (...args: any[]) => Promise<any>;
};

type Scope_currentBeside = {
  stageEmbed: null | "settings" | "stars";
  arch: ArchState;
  UNMAPPED_KEY: string;
};

type Scope_hideBesidePopouts = {
  invoke: Invoke;
};

type Scope_clearArchToFront = {
  arch: ArchState;
  archBack: (...args: any[]) => ArchState;
};

type Scope_ownerStop = {
  UNMAPPED_KEY: string;
};

type Scope_focusMkey = {};

type Scope_releaseKeysToRail = {
  keyOwner: KeyOwner;
  keyOwnerNext: (...args: any[]) => KeyOwner;
  reclaimRailRing: (...args: any[]) => any;
};

type Osd = {
  applyBeside: (...args: any[]) => any;
};

type Pos = {
  keyOwner: KeyOwner;
  walkOut: (...args: any[]) => any;
  stageEmbed: null | "settings" | "stars";
  invoke: Invoke;
};

type Abs = {
  currentBeside: (...args: any[]) => Beside;
  besideNext: (...args: any[]) => Beside;
  hideBesidePopouts: (...args: any[]) => any;
  releaseKeysToRail: (...args: any[]) => any;
  besideFront: (...args: any[]) => BesideFront;
  clearArchToFront: (...args: any[]) => any;
  stageEmbed: null | "settings" | "stars";
};

type Scope_walkInto = {
  applyBeside: (...args: any[]) => any;
  reselectStop: string | null;
  keyOwner: KeyOwner;
  keyOwnerNext: (...args: any[]) => KeyOwner;
};

type Scope_walkOut = {
  keyOwner: KeyOwner;
  arch: ArchState;
  reselectStop: string | null;
  ownerStop: (...args: any[]) => string | null;
  keyOwnerNext: (...args: any[]) => KeyOwner;
  archDo: (...args: any[]) => any;
  reclaimRailRing: (...args: any[]) => any;
};

type Scope_closeStageEmbed = {
  stageEmbed: null | "settings" | "stars";
  keyOwner: KeyOwner;
  keyOwnerNext: (...args: any[]) => KeyOwner;
  reselectStop: string | null;
  reclaimRailRing: (...args: any[]) => any;
  menuRing: MenuRing;
  focusMkey: (...args: any[]) => any;
};

type Scope_popOutSettings = {
  keyOwner: KeyOwner;
  walkOut: (...args: any[]) => any;
  stageEmbed: null | "settings" | "stars";
  invoke: Invoke;
};

type Scope_popOutBoard = {
  invoke: Invoke;
};

type Scope_coachIsDue = {
  coachDue: (...args: any[]) => boolean;
  status: AxStatus | null;
  stateLoaded: boolean;
  claims: Claim[];
  starterStory: CoachStory;
  embedded: boolean;
};

type Scope_maybeRevealCoach = {
  bootParkDone: boolean;
  coachRevealed: boolean;
  embedded: boolean;
  coachIsDue: (...args: any[]) => boolean;
  showStage: (...args: any[]) => Promise<any>;
};

type Scope_noteAx = {
  status: AxStatus | null;
  maybeRevealCoach: (...args: any[]) => any;
};

type Scope_coachClaim = {
  coachError: string;
  invoke: Invoke;
  coachErrorTimer: ReturnType<typeof setTimeout> | undefined;
};

type Scope_coachSkip = {
  invoke: Invoke;
};

type Scope_showStage = {
  embedded: boolean;
  stageRevealed: boolean;
  invoke: Invoke;
};

type Scope_parkStage = {
  embedded: boolean;
  stageRevealed: boolean;
  invoke: Invoke;
  coachIsDue?: () => boolean;
};

type Scope_scheduleGraceRecheck = {
  graceRecheck: ReturnType<typeof setTimeout> | null;
  revealGraceUntil: number;
  maybeAutoHide: (...args: any[]) => any;
};

type Scope_maybeAutoHide = {
  embedded: boolean;
  stageRevealed: boolean;
  revealGraceUntil: number;
  scheduleGraceRecheck: (...args: any[]) => any;
  autoHideWanted: (...args: any[]) => boolean;
  railAutoHideOnBlur: boolean;
  focusInside: boolean;
  pointerInside: boolean;
  parkStage: (...args: any[]) => Promise<any>;
  coachIsDue?: () => boolean;
};

type Scope_parkAfterRaise = {
  invoke: Invoke;
};

type Scope_armNeededMouth = {
  neededAct: (...args: any[]) => { kind: "mouth"; id: string; title: string } | null;
  invoke: Invoke;
};

type Scope_stageFocus = {
  embedded: boolean;
  focusInside: boolean;
  maybeAutoHide: (...args: any[]) => any;
};

type Scope_previewStop = {};

export function stageRole(scope: Scope_stageRole, pid: string): StageRole {
  return scope.stageRoles(scope.sectionIds(), scope.openSectionId()).find((r) => r.id === pid)?.role ?? "icon";
}

export function ensureExpand(scope: Scope_ensureExpand, pid: string) {
  const ids = scope.sectionIds();
  if (!ids.includes(pid)) return;
  const nextOpen = scope.applyPlanetOpen(scope.openSectionId(), ids, pid);
  scope.collapsedSectionIds = nextOpen ? ids.filter((id) => id !== nextOpen) : [...ids];
  scope.persist();
  void scope.showStage();
}

export function collapsePlanet(scope: Scope_collapsePlanet, pid: string) {
  if (!scope.sectionIds().includes(pid) || scope.isCollapsed(pid)) return;
  scope.collapsedSectionIds = [...scope.collapsedSectionIds, pid];
  scope.persist();
}

export function currentBeside(scope: Scope_currentBeside): Beside {
  if (scope.stageEmbed === "stars" || scope.stageEmbed === "settings") return scope.stageEmbed;
  if (scope.arch.front === "board") return "board";
  if (scope.arch.pid === scope.UNMAPPED_KEY && scope.arch.front !== "arch") return "unmapped";
  return null;
}

export function hideBesidePopouts(scope: Scope_hideBesidePopouts) {
  void scope.invoke("hide_window", { label: "stars-beside" }).catch(() => {});
  void scope.invoke("hide_window", { label: "settings-beside" }).catch(() => {});
}

export function clearArchToFront(scope: Scope_clearArchToFront) {
  if (scope.arch.front === "arch") return;
  let s = scope.arch;
  if (s.front === "board") s = scope.archBack(s);
  if (s.front === "layer2") s = scope.archBack(s);
  scope.arch = s;
}

export function ownerStop(scope: Scope_ownerStop, owner: KeyOwner): string | null {
  if (owner === "stars") return "rstars";
  if (owner === "settings") return "rsettings";
  if (owner === "unmapped") return `rhead-${scope.UNMAPPED_KEY}`;
  if (owner === "board") return "rl2-board";
  return null;
}

export function focusMkey(scope: Scope_focusMkey, id: string) {
  queueMicrotask(() => {
    const icon = document.querySelector(`[data-mkey="${CSS.escape(id)}"]`);
    if (icon instanceof HTMLElement) icon.focus();
  });
}

export function releaseKeysToRail(scope: Scope_releaseKeysToRail) {
  if (scope.keyOwner === "rail") return;
  scope.keyOwner = scope.keyOwnerNext(scope.keyOwner, { type: "select", other: "rail" });
  scope.reclaimRailRing();
}

export function openStageDest(scope: Osd, label: "stars" | "settings") {
  scope.applyBeside(label);
}

export function popOutStars(scope: Pos) {
  if (scope.keyOwner === "stars") scope.walkOut();
  scope.stageEmbed = null;
  void scope.invoke("rail_stage_set", { revealed: false });
  void scope.invoke("dest_show", { dest: "stars-beside" });
}

export function applyBeside(scope: Abs, pick: BesidePick) {
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

export function walkInto(scope: Scope_walkInto, target: WalkTarget) {
  scope.applyBeside(target);
  scope.reselectStop = null;
  scope.keyOwner = scope.keyOwnerNext(scope.keyOwner, { type: "walkIn", target });
}

export function walkOut(scope: Scope_walkOut) {
  const fromBoard = scope.keyOwner === "board" && scope.arch.front === "board";
  scope.reselectStop = scope.ownerStop(scope.keyOwner);
  scope.keyOwner = scope.keyOwnerNext(scope.keyOwner, { type: "walkOut" });
  if (fromBoard) scope.archDo({ ...scope.arch, front: "layer2", pending: null });
  scope.reclaimRailRing();
}

export function closeStageEmbed(scope: Scope_closeStageEmbed) {
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

export function popOutSettings(scope: Scope_popOutSettings) {
  if (scope.keyOwner === "settings") scope.walkOut();
  scope.stageEmbed = null;
  void scope.invoke("rail_stage_set", { revealed: false });
  void scope.invoke("dest_show", { dest: "settings-beside" });
}

export function popOutBoard(scope: Scope_popOutBoard) {
  void scope.invoke("dest_show", { dest: "launchpad" });
}

export function coachIsDue(scope: Scope_coachIsDue): boolean {
  return scope.coachDue({
    trusted: !!scope.status?.trusted,
    stateLoaded: scope.stateLoaded,
    claimsCount: scope.claims.length,
    story: scope.starterStory,
    embedded: scope.embedded,
  });
}

export function maybeRevealCoach(scope: Scope_maybeRevealCoach) {
  if (!scope.bootParkDone || scope.coachRevealed || scope.embedded) return;
  if (!scope.coachIsDue()) return;
  scope.coachRevealed = true;
  void scope.showStage();
}

export function noteAx(scope: Scope_noteAx, s: AxStatus) {
  scope.status = s;
  scope.maybeRevealCoach();
}

export async function coachClaim(scope: Scope_coachClaim) {
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

export async function coachSkip(scope: Scope_coachSkip) {
  try {
    await scope.invoke("set_starter_coach_done");
  } catch {
    /* the flag lands on the next state:changed */
  }
}

export async function showStage(scope: Scope_showStage) {
  if (scope.embedded) return;
  scope.stageRevealed = true;
  try {
    await scope.invoke("rail_stage_set", { revealed: true });
  } catch {
    /* the strip still paints if the native place is unavailable */
  }
}

export async function parkStage(scope: Scope_parkStage) {
  if (scope.embedded) return;
  if (scope.coachIsDue?.()) return;
  scope.stageRevealed = false;
  try {
    await scope.invoke("rail_stage_set", { revealed: false });
  } catch {
    /* css parks the strip */
  }
}

export function scheduleGraceRecheck(scope: Scope_scheduleGraceRecheck) {
  if (scope.graceRecheck) return;
  const wait = Math.max(16, scope.revealGraceUntil + 16 - Date.now());
  scope.graceRecheck = setTimeout(() => {
    scope.graceRecheck = null;
    scope.maybeAutoHide();
  }, wait);
}

export function maybeAutoHide(scope: Scope_maybeAutoHide) {
  if (scope.embedded || !scope.stageRevealed) return;
  if (scope.coachIsDue?.()) return;
  if (Date.now() < scope.revealGraceUntil) {
    scope.scheduleGraceRecheck();
    return;
  }
  if (!scope.autoHideWanted({ railAutoHideOnBlur: scope.railAutoHideOnBlur, focusInside: scope.focusInside, pointerInside: scope.pointerInside })) return;
  void scope.parkStage();
}

export function parkAfterRaise(scope: Scope_parkAfterRaise) {
  void scope.invoke("rail_stage_set", { revealed: false, reason: "raised" });
}

export function armNeededMouth(scope: Scope_armNeededMouth, item: { kind?: string; id?: string; title?: string }) {
  const act = scope.neededAct(item);
  if (!act) return;
  void scope.invoke("arm_companion_mouth", { id: act.id, title: act.title });
}

export function stageFocus(scope: Scope_stageFocus, inside: boolean) {
  if (scope.embedded) return;
  scope.focusInside = inside;
  if (!inside) scope.maybeAutoHide();
}

export function previewStop(scope: Scope_previewStop, id: string | null): boolean {
  if (!id) return false;
  return (
    id.startsWith("rraise-") ||
    id.startsWith("rtile-") ||
    id.startsWith("rsnap-") ||
    id.startsWith("rtodos-")
  );
}
