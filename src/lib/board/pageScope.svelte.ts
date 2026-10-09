// Moved function bodies. scope fields are the lets each function already closed over.

import type { KanbanCard, PointsEntry, Project } from "../persist";

type Scope_todayKey = {};

type Scope_startOfToday = {};

type Scope_isDueToday = {
  startOfToday: (...args: any[]) => number;
};

type Scope_pointsToday = {
  todayKey: (...args: any[]) => string;
  pointsLedger: PointsEntry[];
};

type Scope_listOf = {
  projects: Project[];
};

type Scope_actorSide = {};

type Scope_inDoneFilter = {
  doneFilter: "all" | "mine" | "behalf";
  actorSide: (...args: any[]) => "mine" | "behalf";
};

type Scope_livePlanetId = {
  projects: Project[];
};

type Scope_inScope = {
  selected: string;
  isDueToday: (...args: any[]) => boolean;
  listOf: (...args: any[]) => string;
};

export function todayKey(scope: Scope_todayKey): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function startOfToday(scope: Scope_startOfToday): number {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return d.getTime() / 1000;
}

export function isDueToday(scope: Scope_isDueToday, c: KanbanCard): boolean {
  if (!c.dueAt) return false;
  const t = scope.startOfToday();
  return c.dueAt >= t && c.dueAt < t + 86400;
}

export function pointsToday(scope: Scope_pointsToday): number {
  const key = scope.todayKey();
  return scope.pointsLedger
    .filter((e) => e.day === key)
    .reduce((a, e) => a + e.amount, 0);
}

export function listOf(scope: Scope_listOf, c: KanbanCard): string {
  return scope.projects.some((p) => p.id === c.projectId) ? c.projectId : "inbox";
}

export function actorSide(scope: Scope_actorSide, card: KanbanCard): "mine" | "behalf" {
  return card.actor && card.actor !== "user" ? "behalf" : "mine";
}

export function inDoneFilter(scope: Scope_inDoneFilter, c: KanbanCard): boolean {
  return scope.doneFilter === "all" || scope.actorSide(c) === scope.doneFilter;
}

export function livePlanetId(scope: Scope_livePlanetId, card: KanbanCard): string | null {
  return scope.projects.some((p) => p.id === card.projectId) ? card.projectId : null;
}

export function inScope(scope: Scope_inScope, c: KanbanCard): boolean {
  // LP-SPLIT: the kind filter is also what keeps ANSWERED Neededs out
  // of the consolidated Inbox — a Needed never becomes a board todo.
  if (c.kind === "needed" || c.kind === "window") return false;
  if (scope.selected === "done") return true;
  if (scope.selected === "today") return scope.isDueToday(c);
  // LP-SPLIT: Inbox is the consolidated pile — every planet's todos,
  // each card marked by its planet chip. Planet tabs still filter.
  if (scope.selected === "inbox") return true;
  return scope.listOf(c) === scope.selected;
}
