// Moved function bodies. scope fields are the lets each function already closed over.

import type { AgentRun, AgentShortcut, KanbanCard, PersistedState, PointsEntry, Project, Tag, TitleOverride, WindowInfo } from "../persist";

type Invoke = <T = unknown>(cmd: string, args?: any) => Promise<T>;

type Scope_sideCount = {
  openCards: KanbanCard[];
  isDueToday: (...args: any[]) => boolean;
  doneCards: KanbanCard[];
  listOf: (...args: any[]) => string;
};

type Scope_projectName = {
  projects: Project[];
};

type Scope_winFor = {
  windows: WindowInfo[];
};

type Scope_doneChip = {
  startOfToday: (...args: any[]) => number;
};

type Scope_notesSnippet = {};

type Scope_subProgress = {};

type Scope_agentName = {
  agentShortcuts: AgentShortcut[];
};

type Scope_runState = {
  agentRuns: AgentRun[];
};

type Scope_runIsLive = {};

type Scope_isWriting = {
  agentRuns: AgentRun[];
  runIsLive: (...args: any[]) => boolean;
};

type Scope_planetIsLive = {
  hackProjectId: string | null;
  agentRuns: AgentRun[];
  runIsLive: (...args: any[]) => boolean;
};

type Scope_hasAgent = {
  agentRuns: AgentRun[];
  runIsLive: (...args: any[]) => boolean;
};

type Scope_goToken = {};

type Scope_neededForTodo = {
  goToken: (...args: any[]) => string | null;
  kanbanCards: KanbanCard[];
};

type Scope_isDoingRow = {};

type Scope_ageLabel = {};

type Scope_dueInfo = {
  startOfToday: (...args: any[]) => number;
};

type Scope_showProj = {
  livePlanetId: (...args: any[]) => string | null;
};

type Scope_osThemeId = {};

type Scope_loadState = {
  invoke: Invoke;
  themeId: string;
  KNOWN_THEMES: Set<string>;
  osThemeId: (...args: any[]) => string;
  windowOpacity: number;
  clampWindowOpacity: (...args: any[]) => any;
  projects: Project[];
  applyPendingFocus: (...args: any[]) => any;
  tags: Tag[];
  kanbanCards: KanbanCard[];
  pointsLedger: PointsEntry[];
  titleOverrides: TitleOverride[];
  collapsedSectionIds: string[];
  agentShortcuts: AgentShortcut[];
  agentRuns: AgentRun[];
  neededSplit: number;
  clampSharedSplit: (...args: any[]) => any;
  previewWidth: number;
  openSubtaskIds: string[];
  hackProjectId: string | null;
  migratingCardTrackers: boolean;
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

export function sideCount(scope: Scope_sideCount, list: string): number {
  if (list === "today") return scope.openCards.filter((c) => scope.isDueToday(c)).length;
  if (list === "done") return scope.doneCards.length;
  // LP-SPLIT: Inbox counts the whole consolidated all-planets pile.
  if (list === "inbox") return scope.openCards.length;
  return scope.openCards.filter((c) => scope.listOf(c) === list).length;
}

export function projectName(scope: Scope_projectName, id: string): string {
  return scope.projects.find((p) => p.id === id)?.name ?? "";
}

export function winFor(scope: Scope_winFor, card: KanbanCard): WindowInfo | undefined {
  return card.windowId
    ? scope.windows.find((w) => w.id === card.windowId)
    : undefined;
}

export function doneChip(scope: Scope_doneChip, card: KanbanCard): string {
  if ((card.completedAt ?? 0) < scope.startOfToday()) return "";
  return card.pointsAwarded > 0 ? `+${card.pointsAwarded}` : "cap";
}

export function notesSnippet(scope: Scope_notesSnippet, card: KanbanCard): string {
  const first = (card.notes ?? "").split("\n")[0].trim();
  return first.length > 60 ? `${first.slice(0, 60)}…` : first;
}

export function subProgress(scope: Scope_subProgress, card: KanbanCard): string {
  const subs = card.subtasks ?? [];
  if (subs.length === 0) return "";
  return `${subs.filter((s) => s.done).length}/${subs.length}`;
}

export function agentName(scope: Scope_agentName, card: KanbanCard): string {
  const id = card.assignedAgentId;
  if (!id) return "";
  return scope.agentShortcuts.find((a) => a.id === id)?.name ?? "";
}

export function runState(scope: Scope_runState, card: KanbanCard): string {
  const r = scope.agentRuns.find((x) => x.cardId === card.id);
  if (!r || r.state === "done") return "";
  return r.state;
}

export function runIsLive(scope: Scope_runIsLive, r: AgentRun): boolean {
  if (r.state !== "working" && r.state !== "starting" && r.state !== "blocked")
    return false;
  // GO→doing mints a working row with an empty seat. That is not a
  // live writer — companion LED (hackLive) is the other honest signal.
  return !!(r.agentShortcutId || "").trim() || !!r.herdr?.agentName;
}

export function isWriting(scope: Scope_isWriting, card: KanbanCard): boolean {
  const r = scope.agentRuns.find((x) => x.cardId === card.id);
  return !!r && scope.runIsLive(r);
}

export function planetIsLive(scope: Scope_planetIsLive, projectId: string | null | undefined): boolean {
  if (!projectId) return false;
  if (scope.hackProjectId === projectId) return true;
  return scope.agentRuns.some((r) => r.planetId === projectId && scope.runIsLive(r));
}

export function hasAgent(scope: Scope_hasAgent, card: KanbanCard): boolean {
  if (card.assignedAgentId) return true;
  const r = scope.agentRuns.find((x) => x.cardId === card.id);
  return !!r && scope.runIsLive(r);
}

export function goToken(scope: Scope_goToken, title: string): string | null {
  const m = title.match(/GO-[A-Z0-9]+(?:-[A-Z0-9]+)*/i);
  return m ? m[0].toUpperCase() : null;
}

export function neededForTodo(scope: Scope_neededForTodo, card: KanbanCard): KanbanCard | undefined {
  const go = scope.goToken(card.title);
  if (!go) return undefined;
  const planet = card.projectId || "";
  return scope.kanbanCards.find(
    (c) =>
      c.kind === "needed" &&
      (c.neededState === "open" || c.neededState === "later") &&
      (c.projectId || "") === planet &&
      ((c.title || "").toUpperCase().includes(go) ||
        (c.neededWhy || "").toUpperCase().includes(go)),
  );
}

export function isDoingRow(scope: Scope_isDoingRow, c: KanbanCard): boolean {
  return (
    c.status === "doing" &&
    c.kind !== "needed" &&
    c.kind !== "window" &&
    !c.parentId
  );
}

export function ageLabel(scope: Scope_ageLabel, card: KanbanCard): string {
  const t = card.lastTouchedAt || card.createdAt;
  if (!t) return "";
  const s = Date.now() / 1000 - t;
  if (s < 90) return "now";
  if (s < 3600) return `${Math.max(1, Math.round(s / 60))}m`;
  if (s < 86400) return `${Math.max(1, Math.round(s / 3600))}h`;
  return `${Math.max(1, Math.round(s / 86400))}d`;
}

export function dueInfo(scope: Scope_dueInfo, card: KanbanCard): { label: string; overdue: boolean } | null {
  if (!card.dueAt) return null;
  const d = new Date(card.dueAt * 1000);
  const day = new Date(d);
  day.setHours(0, 0, 0, 0);
  const diffDays = Math.round(day.getTime() / 1000 - scope.startOfToday()) / 86400;
  let label: string;
  if (diffDays === 0) label = "Today";
  else if (diffDays === 1) label = "Tomorrow";
  else if (diffDays === -1) label = "Yesterday";
  else label = d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  return {
    label,
    overdue: card.status !== "done" && day.getTime() / 1000 < scope.startOfToday(),
  };
}

export function showProj(scope: Scope_showProj, card: KanbanCard): boolean {
  return scope.livePlanetId(card) !== null;
}

export function osThemeId(scope: Scope_osThemeId): string {
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "systemDark"
    : "systemLight";
}

export async function loadState(scope: Scope_loadState) {
  try {
    const s = await scope.invoke<PersistedState>("load_state");
    // "" means the rail follows the OS; retired ids (clear/liquidGlass)
    // resolve the same way.
    scope.themeId = scope.KNOWN_THEMES.has(s.themeId) ? s.themeId : scope.osThemeId();
    scope.windowOpacity = scope.clampWindowOpacity(s.windowOpacity ?? 1);
    scope.projects = s.projects ?? [];
    scope.applyPendingFocus();
    scope.tags = s.tags ?? [];
    scope.kanbanCards = s.kanbanCards ?? [];
    scope.pointsLedger = s.pointsLedger ?? [];
    scope.titleOverrides = s.titleOverrides ?? [];
    scope.collapsedSectionIds = s.collapsedSectionIds ?? [];
    scope.agentShortcuts = s.agentShortcuts ?? [];
    scope.agentRuns = s.agentRuns ?? [];
    scope.neededSplit = scope.clampSharedSplit(s.neededSplit ?? 0.28);
    scope.previewWidth = Math.max(160, s.previewWidth ?? 280);
    scope.openSubtaskIds = s.openSubtaskIds ?? [];
    scope.hackProjectId = s.companion?.hackLive
      ? (s.companion?.hackProjectId ?? null)
      : null;
    if (
      !scope.migratingCardTrackers &&
      (s.kanbanCards ?? []).some((c) => (c.trackers ?? []).length > 0)
    ) {
      scope.migratingCardTrackers = true;
      await scope.mutate((st) => {
        for (const c of st.kanbanCards) {
          const dump = c.trackers ?? [];
          if (!dump.length || !c.projectId) continue;
          st.projects = st.projects.map((p) =>
            p.id === c.projectId
              ? { ...p, trackers: [...(p.trackers ?? []), ...dump] }
              : p,
          );
        }
        st.kanbanCards = st.kanbanCards.map((c) =>
          c.trackers?.length ? { ...c, trackers: undefined } : c,
        );
      });
      scope.migratingCardTrackers = false;
    }
  } catch {
    // keep last good state; next state:changed retries
  }
}
