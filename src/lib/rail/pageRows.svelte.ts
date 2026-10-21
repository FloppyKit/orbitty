// Moved function bodies. scope fields are the lets each function already closed over.

import type { AgentRun, CardTombstone, Claim, KanbanCard, PointsEntry, Project, Tag, WindowInfo, WindowTags } from "../persist";
import { isTabClaim, tabRow } from "./tabbityRows";

type Scope_nowEpoch = {};

type Scope_todayKey = {};

type Scope_ageClass = {
  claims: Claim[];
  nowEpoch: (...args: any[]) => number;
  GONE_AFTER_S: number;
  PARK_AFTER_S: number;
  FADE_AFTER_S: number;
};

type Scope_pointsToday = {
  todayKey: (...args: any[]) => string;
  pointsLedger: PointsEntry[];
};

type Scope_touchClaim = {
  nowEpoch: (...args: any[]) => number;
  claims: Claim[];
  lastTouchPersist: number;
  persist: (...args: any[]) => Promise<any>;
};

type Scope_windowDragHot = {
  dragWindowId: string | null;
  dragStartedAt: number;
};

type Scope_absorbPendingCards = {
  pendingDisk: { kanbanCards: KanbanCard[]; pointsLedger: PointsEntry[]; agentRuns: AgentRun[]; cardTombstones: CardTombstone[] } | null;
  kanbanCards: KanbanCard[];
  pointsLedger: PointsEntry[];
  agentRuns: AgentRun[];
  cardTombstones: CardTombstone[];
};

type Scope_todosFor = {
  kanbanCards: KanbanCard[];
};

type Scope_rowsFor = {
  windows: WindowInfo[];
  liveClaims: Claim[];
  claimStableKey: (...args: any[]) => string | null;
};

type Scope_visibleRows = {
  tagFilter: string | null;
  hasTag: (...args: any[]) => boolean;
};

type Scope_hasTag = {
  liveWindowTags: WindowTags[];
};

type Scope_tagsFor = {
  liveWindowTags: WindowTags[];
  tags: Tag[];
};

type Scope_isCollapsed = {
  collapsedSectionIds: string[];
};

type Scope_toggleCollapse = {
  collapsedSectionIds: string[];
  isCollapsed: (...args: any[]) => boolean;
  persist: (...args: any[]) => Promise<any>;
};

type Scope_sectionIds = {
  UNMAPPED_KEY: string;
  projects: Project[];
};

type Scope_openSectionId = {
  sectionIds: (...args: any[]) => string[];
  isCollapsed: (...args: any[]) => boolean;
};

export function nowEpoch(scope: Scope_nowEpoch): number {
  return Date.now() / 1000;
}

export function todayKey(scope: Scope_todayKey): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function ageClass(scope: Scope_ageClass, windowId: string): "fresh" | "fade" | "park" | "gone" {
  const t = scope.claims.find((c) => c.windowId === windowId)?.lastTouchedAt ?? 0;
  if (t <= 0) return "fresh";
  const age = scope.nowEpoch() - t;
  if (age >= scope.GONE_AFTER_S) return "gone";
  if (age >= scope.PARK_AFTER_S) return "park";
  if (age >= scope.FADE_AFTER_S) return "fade";
  return "fresh";
}

export function pointsToday(scope: Scope_pointsToday): number {
  const key = scope.todayKey();
  return scope.pointsLedger
    .filter((e) => e.day === key)
    .reduce((a, e) => a + e.amount, 0);
}

export function touchClaim(scope: Scope_touchClaim, windowId: string, force = false) {
  const now = scope.nowEpoch();
  scope.claims = scope.claims.map((c) =>
    c.windowId === windowId ? { ...c, lastTouchedAt: now } : c,
  );
  if (force || now - scope.lastTouchPersist > 30) {
    scope.lastTouchPersist = now;
    scope.persist();
  }
}

export function windowDragHot(scope: Scope_windowDragHot): boolean {
  return !!scope.dragWindowId || Date.now() - scope.dragStartedAt < 400;
}

export function absorbPendingCards(scope: Scope_absorbPendingCards) {
  const disk = scope.pendingDisk;
  if (!disk) return;
  scope.pendingDisk = null;
  const local = scope.kanbanCards;
  const merged = new Map<string, KanbanCard>();
  for (const c of disk.kanbanCards) merged.set(c.id, c);
  for (const c of local) {
    const have = merged.get(c.id);
    if (!have || c.lastTouchedAt > have.lastTouchedAt) merged.set(c.id, c);
  }
  for (const t of disk.cardTombstones) {
    const c = merged.get(t.id);
    const localNewer = local.some((x) => x.id === t.id && x.lastTouchedAt > t.lastTouchedAt);
    if (c && t.lastTouchedAt >= c.lastTouchedAt && !localNewer) merged.delete(t.id);
  }
  scope.kanbanCards = [...merged.values()];
  scope.pointsLedger = disk.pointsLedger;
  scope.agentRuns = disk.agentRuns;
  scope.cardTombstones = disk.cardTombstones;
}

export function todosFor(scope: Scope_todosFor, projectId: string): KanbanCard[] {
  return scope.kanbanCards
    .filter(
      // Needed cards never count as rail todos.
      (c) =>
        c.kind !== "needed" &&
        c.status !== "done" &&
        !c.windowId &&
        c.projectId === projectId,
    )
    .sort((a, b) => a.createdAt - b.createdAt);
}

export function rowsFor(scope: Scope_rowsFor, projectId: string): WindowInfo[] {
  const live = scope.windows.filter((w) =>
    scope.liveClaims.some((c) => c.windowId === w.id && c.projectId === projectId && !isTabClaim(c)),
  );
  // A claimed window that closed stays listed as an
  // asleep row, synthesized from the claim fingerprint (icon may be gone —
  // that's fine). Done/Unclaim is the only way it leaves.
  // Tab claims are not windows; they get their own rows below.
  const sleepers: WindowInfo[] = scope.liveClaims
    .filter(
      (c) =>
        c.projectId === projectId &&
        !isTabClaim(c) &&
        !scope.windows.some((w) => w.id === c.windowId),
    )
    .map((c) => ({
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
    }));
  const tabs: WindowInfo[] = scope.liveClaims
    .filter((c) => c.projectId === projectId && isTabClaim(c))
    .map((c) => tabRow(c, scope.windows));
  return [...live, ...sleepers, ...tabs];
}

export function visibleRows(scope: Scope_visibleRows, rows: WindowInfo[]): WindowInfo[] {
  if (!scope.tagFilter) return rows;
  return rows.filter((w) => scope.hasTag(w.id, scope.tagFilter!));
}

export function hasTag(scope: Scope_hasTag, windowId: string, tagId: string): boolean {
  return scope.liveWindowTags.some(
    (wt) => wt.windowId === windowId && wt.tagIds.includes(tagId),
  );
}

export function tagsFor(scope: Scope_tagsFor, windowId: string): Tag[] {
  const ids =
    scope.liveWindowTags.find((wt) => wt.windowId === windowId)?.tagIds ?? [];
  return scope.tags.filter((t) => ids.includes(t.id));
}

export function isCollapsed(scope: Scope_isCollapsed, sectionId: string): boolean {
  return scope.collapsedSectionIds.includes(sectionId);
}

export function toggleCollapse(scope: Scope_toggleCollapse, sectionId: string) {
  scope.collapsedSectionIds = scope.isCollapsed(sectionId)
    ? scope.collapsedSectionIds.filter((id) => id !== sectionId)
    : [...scope.collapsedSectionIds, sectionId];
  scope.persist();
}

export function sectionIds(scope: Scope_sectionIds): string[] {
  return [...scope.projects.map((p) => p.id), scope.UNMAPPED_KEY];
}

export function openSectionId(scope: Scope_openSectionId): string | null {
  const open = scope.sectionIds().filter((id) => !scope.isCollapsed(id));
  return open.length === 1 ? open[0] : null;
}
