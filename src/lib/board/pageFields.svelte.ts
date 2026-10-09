// Moved function bodies. scope fields are the lets each function already closed over.

import type { KanbanCard, PersistedState, Project, Tag, WindowInfo } from "../persist";
import type { RingStop } from "../menuKeys.svelte";

type Invoke = <T = unknown>(cmd: string, args?: any) => Promise<T>;

type Scope_drawerStops = {
  setDue: (...args: any[]) => any;
  toggleSubtask: (...args: any[]) => any;
  deleteSubtask: (...args: any[]) => any;
  tags: Tag[];
  toggleCardTag: (...args: any[]) => any;
  winFor: (...args: any[]) => WindowInfo | undefined;
  focusCard: (...args: any[]) => Promise<any>;
  setRating: (...args: any[]) => any;
  deleteTodo: (...args: any[]) => any;
};

type Scope_saveTitle = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

type Scope_undoThisTask = {
  invoke: Invoke;
  loadState: (...args: any[]) => Promise<any>;
};

type Scope_saveNotes = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

type Scope_setPlanet = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
  logEvent: (...args: any[]) => any;
};

type Scope_setAgent = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
  logEvent: (...args: any[]) => any;
};

type Scope_newCardId = {};

type Scope_childrenOf = {
  kanbanCards: KanbanCard[];
};

type Scope_remainingKids = {
  childrenOf: (...args: any[]) => KanbanCard[];
};

type Scope_toggleOpenSubtasks = {
  openSubtaskIds: string[];
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

type Scope_wouldCycle = {
  kanbanCards: KanbanCard[];
};

type Scope_linkableTodos = {
  kanbanCards: KanbanCard[];
  wouldCycle: (...args: any[]) => boolean;
};

type Scope_mintTodo = {
  projects: Project[];
  selected: string;
  newCardId: (...args: any[]) => string;
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
  logEvent: (...args: any[]) => any;
  collapsedSectionIds: string[];
  emitTo: (...args: any[]) => any;
  addingTodo: boolean;
  newTodoTitle: string;
  detailId: string | null;
};

type Scope_attachChild = {
  wouldCycle: (...args: any[]) => boolean;
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

type Scope_detachChild = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

type Scope_setDue = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

type Scope_dateInputValue = {};

type Scope_cardHasTag = {};

type Scope_addTagToCard = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
  newTagName: string;
};

type Scope_toggleCardTag = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

type Scope_toggleSubtask = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

type Scope_deleteTodo = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
  logEvent: (...args: any[]) => any;
  previewEdit: boolean;
  detailId: string | null;
  neededSelectedId: string | null;
};

type Scope_setRating = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

type Scope_deleteSubtask = {
  mutate: (fn: (s: PersistedState) => void) => Promise<void>;
};

export function drawerStops(scope: Scope_drawerStops, card: KanbanCard): RingStop[] {
  const focusStop = (id: string) => () => {
    document
      .querySelector<HTMLElement>(`[data-mkey="${CSS.escape(id)}"]`)
      ?.focus();
  };
  const stops: RingStop[] = [
    { id: "d-title", confirm: focusStop("d-title") },
    { id: "d-planet", confirm: focusStop("d-planet") },
    { id: "d-agent", confirm: focusStop("d-agent") },
    { id: "d-notes", confirm: focusStop("d-notes") },
  ];
  stops.push({ id: "d-due", confirm: focusStop("d-due") });
  if (card.dueAt) {
    stops.push({ id: "d-due-clear", confirm: () => scope.setDue(card, "") });
  }
  for (const sub of card.subtasks ?? []) {
    stops.push({
      id: `d-sub-check-${sub.id}`,
      confirm: () => scope.toggleSubtask(card, sub.id),
    });
    stops.push({
      id: `d-sub-del-${sub.id}`,
      confirm: () => scope.deleteSubtask(card, sub.id),
    });
  }
  stops.push({ id: "d-tag-new", confirm: focusStop("d-tag-new") });
  for (const t of scope.tags) {
    stops.push({
      id: `d-tag-${t.id}`,
      confirm: () => scope.toggleCardTag(card, t.id),
    });
  }
  if ((card.kind === "window" || card.windowId) && scope.winFor(card)) {
    stops.push({ id: "d-win-focus", confirm: () => scope.focusCard(card) });
  }
  for (let n = 1; n <= 7; n++) {
    stops.push({
      id: `d-rate-${n}`,
      confirm: () => scope.setRating(card, card.rating === n ? null : n),
    });
  }
  if (card.kind !== "window" && card.kind !== "needed") {
    stops.push({ id: "d-trash", confirm: () => scope.deleteTodo(card) });
  }
  return stops;
}

export function saveTitle(scope: Scope_saveTitle, card: KanbanCard, raw: string) {
  const title = raw.trim();
  if (!title || title === card.title) return;
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === card.id ? { ...c, title, lastTouchedAt: Date.now() / 1000 } : c,
    );
  });
}

export async function undoThisTask(scope: Scope_undoThisTask, card: KanbanCard) {
  try {
    await scope.invoke("undo_task", { cardId: card.id });
    await scope.loadState();
  } catch {
    /* no before snapshot */
  }
}

export function saveNotes(scope: Scope_saveNotes, card: KanbanCard, raw: string) {
  const notes = raw.trim();
  if (notes === (card.notes ?? "")) return;
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === card.id
        ? { ...c, notes: notes || undefined, lastTouchedAt: Date.now() / 1000 }
        : c,
    );
  });
}

export function setPlanet(scope: Scope_setPlanet, card: KanbanCard, projectId: string) {
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === card.id
        ? { ...c, projectId, lastTouchedAt: Date.now() / 1000 }
        : c,
    );
  });
  scope.logEvent("board-planet", { cardId: card.id, projectId }, projectId || undefined);
}

export function setAgent(scope: Scope_setAgent, card: KanbanCard, agentId: string) {
  const assignedAgentId = agentId || null;
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === card.id
        ? {
            ...c,
            assignedAgentId,
            actor: assignedAgentId ? "agent" : c.actor,
            lastTouchedAt: Date.now() / 1000,
          }
        : c,
    );
  });
  scope.logEvent(
    "board-assign",
    { cardId: card.id, agentId: assignedAgentId },
    card.projectId,
  );
}

export function newCardId(scope: Scope_newCardId): string {
  return `c-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export function childrenOf(scope: Scope_childrenOf, parentId: string): KanbanCard[] {
  return scope.kanbanCards.filter(
    (c) => c.parentId === parentId && c.kind !== "needed" && c.kind !== "window",
  );
}

export function remainingKids(scope: Scope_remainingKids, parentId: string): number {
  return scope.childrenOf(parentId).filter((c) => c.status !== "done").length;
}

export function toggleOpenSubtasks(scope: Scope_toggleOpenSubtasks, parentId: string, e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  const on = scope.openSubtaskIds.includes(parentId);
  const next = on
    ? scope.openSubtaskIds.filter((id) => id !== parentId)
    : [...scope.openSubtaskIds, parentId];
  scope.openSubtaskIds = next;
  scope.mutate((s) => {
    s.openSubtaskIds = next;
  });
}

export function wouldCycle(scope: Scope_wouldCycle, parentId: string, childId: string): boolean {
  let id: string | null | undefined = parentId;
  const seen = new Set<string>();
  while (id) {
    if (id === childId) return true;
    if (seen.has(id)) return true;
    seen.add(id);
    id = scope.kanbanCards.find((c) => c.id === id)?.parentId;
  }
  return false;
}

export function linkableTodos(scope: Scope_linkableTodos, parent: KanbanCard): KanbanCard[] {
  return scope.kanbanCards.filter(
    (c) =>
      c.id !== parent.id &&
      c.kind !== "needed" &&
      c.kind !== "window" &&
      c.parentId !== parent.id &&
      !scope.wouldCycle(parent.id, c.id),
  );
}

export function mintTodo(scope: Scope_mintTodo, title: string, parentId?: string) {
  const t = title.trim();
  if (!t) return;
  const now = Date.now() / 1000;
  const projectId =
    scope.projects.some((p) => p.id === scope.selected) ? scope.selected : "";
  const card: KanbanCard = {
    id: scope.newCardId(),
    title: t,
    projectId,
    kind: "todo",
    status: "todo",
    createdAt: now,
    lastTouchedAt: now,
    goalAligned: true,
    clinicalOrMaster: false,
    pointsAwarded: 0,
    parentId: parentId || undefined,
  };
  scope.mutate((s) => {
    s.kanbanCards = [...s.kanbanCards, card];
  });
  scope.logEvent("board-add", { cardId: card.id, title: t, parentId }, projectId || undefined);
  // A marble opens when a todo lands on it so the new card is visible.
  if (projectId && scope.collapsedSectionIds.includes(projectId)) {
    scope.emitTo("main", "habitat:expand", projectId).catch(() => {});
  }
  if (!parentId) {
    scope.addingTodo = false;
    scope.newTodoTitle = "";
    scope.detailId = card.id;
  }
}

export function attachChild(scope: Scope_attachChild, parent: KanbanCard, childId: string) {
  if (!childId || scope.wouldCycle(parent.id, childId)) return;
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === childId
        ? { ...c, parentId: parent.id, lastTouchedAt: Date.now() / 1000 }
        : c,
    );
  });
}

export function detachChild(scope: Scope_detachChild, childId: string) {
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === childId
        ? { ...c, parentId: undefined, lastTouchedAt: Date.now() / 1000 }
        : c,
    );
  });
}

export function setDue(scope: Scope_setDue, card: KanbanCard, dateValue: string) {
  // Local noon sidesteps DST/timezone edge flips on the stored epoch.
  const dueAt = dateValue
    ? new Date(`${dateValue}T12:00:00`).getTime() / 1000
    : null;
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === card.id ? { ...c, dueAt } : c,
    );
  });
}

export function dateInputValue(scope: Scope_dateInputValue, card: KanbanCard): string {
  if (!card.dueAt) return "";
  const d = new Date(card.dueAt * 1000);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function cardHasTag(scope: Scope_cardHasTag, card: KanbanCard, tagId: string): boolean {
  return (card.tagIds ?? []).includes(tagId);
}

export function addTagToCard(scope: Scope_addTagToCard, card: KanbanCard, raw: string) {
  const name = raw.trim();
  if (!name) return;
  scope.mutate((s) => {
    s.tags ??= [];
    let tag = s.tags.find((t) => t.name.toLowerCase() === name.toLowerCase());
    if (!tag) {
      tag = {
        id: `t-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
        name,
      };
      s.tags = [...s.tags, tag];
    }
    const tagId = tag.id;
    s.kanbanCards = s.kanbanCards.map((c) => {
      if (c.id !== card.id) return c;
      const cur = c.tagIds ?? [];
      if (cur.includes(tagId)) return c;
      return { ...c, tagIds: [...cur, tagId] };
    });
  });
  scope.newTagName = "";
}

export function toggleCardTag(scope: Scope_toggleCardTag, card: KanbanCard, tagId: string) {
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) => {
      if (c.id !== card.id) return c;
      const cur = c.tagIds ?? [];
      const next = cur.includes(tagId)
        ? cur.filter((id) => id !== tagId)
        : [...cur, tagId];
      return { ...c, tagIds: next.length ? next : undefined };
    });
  });
}

export function toggleSubtask(scope: Scope_toggleSubtask, card: KanbanCard, subId: string) {
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === card.id
        ? {
            ...c,
            subtasks: (c.subtasks ?? []).map((sub) =>
              sub.id === subId ? { ...sub, done: !sub.done } : sub,
            ),
          }
        : c,
    );
  });
}

export function deleteTodo(scope: Scope_deleteTodo, card: KanbanCard) {
  if (card.kind === "window" || card.kind === "needed") return;
  const id = card.id;
  const now = Date.now() / 1000;
  scope.mutate((s) => {
    s.cardTombstones = [
      ...(s.cardTombstones ?? []).filter((t) => t.id !== id),
      { id, lastTouchedAt: now },
    ];
    s.kanbanCards = s.kanbanCards
      .filter((c) => c.id !== id)
      .map((c) =>
        c.parentId === id
          ? { ...c, parentId: undefined, lastTouchedAt: Date.now() / 1000 }
          : c,
      );
    s.agentRuns = (s.agentRuns ?? []).filter((r) => r.cardId !== id);
  });
  scope.logEvent("board-delete", { cardId: id, title: card.title }, card.projectId);
  scope.previewEdit = false;
  if (scope.detailId === id) scope.detailId = null;
  if (scope.neededSelectedId === id) scope.neededSelectedId = null;
}

export function setRating(scope: Scope_setRating, card: KanbanCard, n: number | null) {
  const rating = n != null && n >= 1 && n <= 7 ? n : null;
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === card.id
        ? { ...c, rating, lastTouchedAt: Date.now() / 1000 }
        : c,
    );
  });
}

export function deleteSubtask(scope: Scope_deleteSubtask, card: KanbanCard, subId: string) {
  scope.mutate((s) => {
    s.kanbanCards = s.kanbanCards.map((c) =>
      c.id === card.id
        ? { ...c, subtasks: (c.subtasks ?? []).filter((sub) => sub.id !== subId) }
        : c,
    );
  });
}
