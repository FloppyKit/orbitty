// Moved function bodies. scope fields are the lets each function already closed over.

import type { CardStatus as Status, KanbanCard, PersistedState } from "../persist";
import type { ColumnMode } from "./planetColumn";

type Invoke = <T = unknown>(cmd: string, args?: any) => Promise<T>;

type Scope_logEvent = {
  invoke: Invoke;
};

type Scope_mutate = {
  invoke: Invoke;
  loadState: (...args: any[]) => Promise<any>;
  errText: (...args: any[]) => string;
  showLinkToast: (...args: any[]) => any;
};

type Scope_errText = {};

type Scope_showLinkToast = {
  linkToast: string;
  linkToastTimer: ReturnType<typeof setTimeout> | undefined;
};

type Scope_openCardLink = {
  showLinkToast: (...args: any[]) => any;
  requestLink: (...args: any[]) => Promise<void>;
  linkErrText: (...args: any[]) => string;
};

type Scope_openUrl = {
  requestLink: (...args: any[]) => Promise<void>;
  showLinkToast: (...args: any[]) => any;
  linkErrText: (...args: any[]) => string;
};

type Scope_addCardLink = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

type Scope_removeCardLink = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

type Scope_completeCard = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
  todayKey: (...args: any[]) => string;
  DAILY_CAP: number;
  logEvent: (...args: any[]) => any;
};

type Scope_toggleCard = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
  logEvent: (...args: any[]) => any;
  completeCard: (...args: any[]) => any;
};

type Scope_moveCard = {
  completeCard: (...args: any[]) => any;
  kanbanCards: KanbanCard[];
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
  logEvent: (...args: any[]) => any;
};

type Scope_openDetail = {
  neededSelectedId: string | null;
  armNeededMouth: (...args: any[]) => any;
  detailId: string | null;
  newTagName: string;
  previewEdit: boolean;
  planetDdOpen: boolean;
};

type Scope_toggleDoing = {
  moveCard: (...args: any[]) => any;
  invoke: Invoke;
  loadState: (...args: any[]) => Promise<any>;
};

type Scope_skipNeededCard = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
  logEvent: (...args: any[]) => any;
};

type Scope_laterNeededCard = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
  logEvent: (...args: any[]) => any;
};

type Scope_onSplitDown = {
  mailEl: HTMLDivElement | null;
  neededSplit: number;
  clampSharedSplit: (...args: any[]) => any;
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

type Scope_columnWidthNow = {
  columnPx: (...args: any[]) => number;
  columnMode: (...args: any[]) => ColumnMode;
  boardPx: number;
  planetColPref: "auto" | "open" | "icons" | "hidden";
};

type Scope_mailMinPx = {
  columnWidthNow: (...args: any[]) => number;
};

type Scope_togglePlanetCol = {
  planetColPref: "auto" | "open" | "icons" | "hidden";
  planetColMode: any;
  previewWidth: number;
  clampPreview: (...args: any[]) => number;
};

type Scope_previewMinPx = {};

type Scope_clampPreview = {
  bodyEl: HTMLDivElement | null;
  previewMinPx: (...args: any[]) => number;
  mailMinPx: (...args: any[]) => number;
};

type Scope_onEastSplitDown = {
  bodyEl: HTMLDivElement | null;
  previewWidth: number;
  clampPreview: (...args: any[]) => number;
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

type Scope_resetPreviewWidth = {
  previewWidth: number;
  clampPreview: (...args: any[]) => number;
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

export function logEvent(scope: Scope_logEvent, eType: string, content: Record<string, unknown>, projectId?: string) {
  scope.invoke("log_event", {
    eType,
    content,
    projectId: projectId || null,
    actor: "user",
  }).catch(() => {});
}

export async function mutate(
  scope: Scope_mutate,
  fn: (s: PersistedState) => void,
  origin?: string,
) {
  try {
    const s = await scope.invoke<PersistedState>("load_state");
    s.projects ??= [];
    s.claims ??= [];
    s.tags ??= [];
    s.windowTags ??= [];
    s.collapsedSectionIds ??= [];
    s.nudgeSuppressedWindowIds ??= [];
    s.kanbanCards ??= [];
    s.pointsLedger ??= [];
    s.titleOverrides ??= [];
    fn(s);
    // Origin is an IPC argument only. Omit the key unless a caller passed one,
    // so every existing board write stays a user save.
    if (origin !== undefined) {
      await scope.invoke("save_state", { state: s, origin });
    } else {
      await scope.invoke("save_state", { state: s });
    }
    await scope.loadState();
  } catch (e) {
    // Optimistic column paint already moved the card; reload disk
    // so a failed save does not leave a lying column.
    await scope.loadState();
    const msg = scope.errText(e);
    if (/link/i.test(msg)) scope.showLinkToast(msg);
  }
}

export function errText(scope: Scope_errText, e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "";
}

export function showLinkToast(scope: Scope_showLinkToast, msg: string) {
  scope.linkToast = /file not found/i.test(msg) ? "file not found" : msg;
  clearTimeout(scope.linkToastTimer);
  scope.linkToastTimer = setTimeout(() => {
    scope.linkToast = "";
  }, 4000);
}

export async function openCardLink(scope: Scope_openCardLink, card: KanbanCard, index = 0) {
  const url = card.links?.[index]?.url;
  if (!url) {
    scope.showLinkToast("no link");
    return;
  }
  try {
    await scope.requestLink(url);
  } catch (e) {
    scope.showLinkToast(scope.linkErrText(e));
  }
}

export async function openUrl(scope: Scope_openUrl, url: string) {
  try {
    await scope.requestLink(url);
  } catch (e) {
    scope.showLinkToast(scope.linkErrText(e));
  }
}

export function addCardLink(scope: Scope_addCardLink, card: KanbanCard, url: string, label: string) {
  const nextUrl = url.trim();
  if (!nextUrl) return;
  const nextLabel = label.trim();
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) => {
      if (c.id !== card.id) return c;
      const cur = c.links ?? [];
      if (cur.some((l) => l.url === nextUrl)) return c;
      return {
        ...c,
        links: [...cur, { url: nextUrl, label: nextLabel || undefined }],
        lastTouchedAt: Date.now() / 1000,
      };
    });
  });
}

export function removeCardLink(scope: Scope_removeCardLink, card: KanbanCard, url: string) {
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) => {
      if (c.id !== card.id) return c;
      const links = (c.links ?? []).filter((l) => l.url !== url);
      return {
        ...c,
        links: links.length ? links : undefined,
        lastTouchedAt: Date.now() / 1000,
      };
    });
  });
}

export function completeCard(scope: Scope_completeCard, card: KanbanCard) {
  const now = Date.now() / 1000;
  scope.mutate((s) => {
    const key = scope.todayKey();
    const todayPts = s.pointsLedger
      .filter((e) => e.day === key)
      .reduce((a, e) => a + e.amount, 0);
    const alreadyAwarded = s.pointsLedger.some((e) => e.cardId === card.id);
    const base = card.clinicalOrMaster ? 2 : 1;
    const award = !alreadyAwarded && todayPts + base <= scope.DAILY_CAP ? base : 0;
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === card.id
        ? {
            ...c,
            status: "done",
            lastTouchedAt: now,
            completedAt: now,
            pointsAwarded: c.pointsAwarded + award,
            actor: "user" as const,
          }
        : c,
    );
    s.agentRuns = (s.agentRuns ?? []).map((r) =>
      r.cardId === card.id && r.state !== "done" ? { ...r, state: "done" as const } : r,
    );
    if (award > 0) {
      s.pointsLedger = [
        ...s.pointsLedger,
        {
          id: `pt-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
          cardId: card.id,
          amount: award,
          day: key,
          at: now,
          actor: "user" as const,
        },
      ];
    }
    // No unclaim here: the walk-off narrates first, then the rail's
    // habitat:walked-off handler unclaims (the existing Done eviction).
  });
  scope.logEvent("board-move", { from: card.status, to: "done", cardId: card.id }, card.projectId);
}

export function toggleCard(scope: Scope_toggleCard, card: KanbanCard) {
  const now = Date.now() / 1000;
  if (card.status === "done") {
    scope.mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? {
              ...c,
              status: "todo",
              windowId: null,
              completedAt: null,
              lastTouchedAt: now,
            }
          : c,
      );
    });
    scope.logEvent("board-move", { from: "done", to: "todo", cardId: card.id }, card.projectId);
    return;
  }
  scope.completeCard(card);
}

export function moveCard(scope: Scope_moveCard, card: KanbanCard, target: Status) {
  if (card.status === target) return;
  if (target === "done") {
    scope.completeCard(card);
    return;
  }
  const now = Date.now() / 1000;
  // Paint the column this frame. Doing-fire / walker attach run after
  // the save returns — awaiting them here is the todo→doing hitch.
  scope.kanbanCards = scope.kanbanCards.map((c) =>
    c.id === card.id
      ? {
          ...c,
          status: target,
          completedAt: null,
          lastTouchedAt: now,
        }
      : c,
  );
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === card.id
        ? {
            ...c,
            status: target,
            completedAt: null,
            lastTouchedAt: now,
          }
        : c,
    );
  });
  scope.logEvent("board-move", { from: card.status, to: target, cardId: card.id }, card.projectId);
}

export function openDetail(scope: Scope_openDetail, card: KanbanCard) {
  if (card.kind !== "needed") {
    if (scope.neededSelectedId) scope.armNeededMouth(null, null);
    scope.neededSelectedId = null;
  }
  scope.detailId = card.id;
  scope.newTagName = "";
  scope.previewEdit = false;
  scope.planetDdOpen = false;
}

export async function toggleDoing(scope: Scope_toggleDoing, card: KanbanCard, e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  if (card.status === "doing") {
    scope.moveCard(card, "todo");
    return;
  }
  const agentId = (card.assignedAgentId || "").trim();
  if (agentId) {
    try {
      await scope.invoke("agent_pickup", { id: agentId, cardId: card.id });
      await scope.loadState();
      return;
    } catch {
      // Harness miss still parks the card so the walker can mint.
    }
  }
  scope.moveCard(card, "doing");
}

export function skipNeededCard(scope: Scope_skipNeededCard, card: KanbanCard) {
  const now = Date.now() / 1000;
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === card.id
        ? { ...c, neededState: "skipped" as const, lastTouchedAt: now }
        : c,
    );
  });
  scope.logEvent("needed-skip", { cardId: card.id }, card.projectId);
}

export function laterNeededCard(scope: Scope_laterNeededCard, card: KanbanCard) {
  const now = Date.now() / 1000;
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === card.id
        ? { ...c, neededState: "later" as const, lastTouchedAt: now }
        : c,
    );
  });
  scope.logEvent("needed-later", { cardId: card.id }, card.projectId);
}

export function onSplitDown(scope: Scope_onSplitDown, e: PointerEvent) {
  const el = scope.mailEl;
  if (!el) return;
  e.preventDefault();
  const startY = e.clientY;
  const start = scope.neededSplit;
  const h = Math.max(el.clientHeight, 1);
  const move = (ev: PointerEvent) => {
    scope.neededSplit = scope.clampSharedSplit(start + (ev.clientY - startY) / h);
  };
  const up = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    const v = scope.neededSplit;
    scope.mutate((s) => {
      s.neededSplit = v;
    });
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}

export function columnWidthNow(scope: Scope_columnWidthNow): number {
  return scope.columnPx(scope.columnMode(scope.boardPx, scope.planetColPref));
}

export function mailMinPx(scope: Scope_mailMinPx): number {
  return 120 + scope.columnWidthNow();
}

export function togglePlanetCol(scope: Scope_togglePlanetCol) {
  scope.planetColPref = scope.planetColMode === "open" ? "icons" : "open";
  scope.previewWidth = scope.clampPreview(scope.previewWidth);
}

export function previewMinPx(scope: Scope_previewMinPx): number {
  const probe = document.createElement("span");
  probe.style.cssText =
    "position:absolute;left:-9999px;font:400 0.8rem 'SF Pro Text',system-ui,sans-serif;white-space:nowrap";
  probe.textContent = "LED watch dogfood";
  document.body.appendChild(probe);
  const text = probe.getBoundingClientRect().width;
  probe.remove();
  return Math.max(160, Math.ceil(text + 11 + 7 + 16));
}

export function clampPreview(scope: Scope_clampPreview, px: number): number {
  const body = scope.bodyEl;
  const prevMin = scope.previewMinPx();
  if (!body) return Math.max(prevMin, px);
  const cs = getComputedStyle(body);
  const pad =
    (parseFloat(cs.paddingLeft) || 0) + (parseFloat(cs.paddingRight) || 0);
  // 6px is the east split. mailMinPx already includes the planet column.
  const avail = Math.max(body.clientWidth - pad - 6, 1);
  const maxPreview = Math.max(prevMin, avail - scope.mailMinPx());
  return Math.min(maxPreview, Math.max(prevMin, px));
}

export function onEastSplitDown(scope: Scope_onEastSplitDown, e: PointerEvent) {
  if (!scope.bodyEl) return;
  e.preventDefault();
  (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
  const startX = e.clientX;
  const start = scope.previewWidth;
  const move = (ev: PointerEvent) => {
    scope.previewWidth = scope.clampPreview(start - (ev.clientX - startX));
  };
  const up = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    const v = scope.previewWidth;
    scope.mutate((s) => {
      s.previewWidth = v;
    });
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}

export function resetPreviewWidth(scope: Scope_resetPreviewWidth) {
  scope.previewWidth = scope.clampPreview(280);
  scope.mutate((s) => {
    s.previewWidth = scope.previewWidth;
  });
}
