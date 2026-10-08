// Fold signed board proofs into columns.
//
// The Mac log is not a replay of the machine. Kind 31200 never enters
// this function's result. Kinds 420, 421, and 422 are column proofs only:
// a mint's content is `{ cardId, title }`, a move's content is
// `{ from, to, cardId }` and carries no title. The mint title sticks.
// A move for a card we have not minted still shows that card id.

export const KIND_TODO = 420;
export const KIND_DOING = 421;
export const KIND_DONE = 422;

export type ColumnId = "todo" | "doing" | "done";

/** One jsonl / relay line. `content` is a JSON string, as the Rust log writes it. */
export type BoardProof = {
  id?: string;
  pubkey?: string;
  created_at?: number;
  kind: number;
  tags?: readonly (readonly string[])[];
  content?: string;
  sig?: string;
};

export type FoldedCard = {
  cardId: string;
  /** Mint title, or the card id when no mint has been seen. */
  title: string;
};

export type FoldedProject = {
  /** Project tag value. Null when no proof carried one. */
  projectId: string | null;
  todo: FoldedCard[];
  doing: FoldedCard[];
  done: FoldedCard[];
};

type Column = ColumnId;

type Acc = {
  cardId: string;
  title: string | null;
  column: Column;
  projectId: string | null;
  placedAt: number;
  seq: number;
};

const COLUMNS: readonly Column[] = ["todo", "doing", "done"];

function columnOf(kind: number): Column | null {
  if (kind === KIND_TODO) return "todo";
  if (kind === KIND_DOING) return "doing";
  if (kind === KIND_DONE) return "done";
  return null;
}

function tagValues(ev: BoardProof, name: string): string[] {
  const out: string[] = [];
  for (const tag of ev.tags ?? []) {
    if (tag[0] === name && typeof tag[1] === "string" && tag[1] !== "") {
      out.push(tag[1]);
    }
  }
  return out;
}

/** Relay allowlist: t=orbitty, and not the other chains that reuse these numbers. */
function isOrbittyProof(ev: BoardProof): boolean {
  const t = tagValues(ev, "t");
  if (t.some((v) => v === "nch" || v === "local-only" || v === "publishable")) {
    return false;
  }
  return t.includes("orbitty");
}

function parseContent(content: string | undefined): Record<string, unknown> {
  if (!content) return {};
  try {
    const value = JSON.parse(content) as unknown;
    if (value && typeof value === "object" && !Array.isArray(value)) {
      return value as Record<string, unknown>;
    }
  } catch {
    // A broken content string still has a column and a `d` tag.
  }
  return {};
}

function cardIdOf(ev: BoardProof, content: Record<string, unknown>): string {
  const tagged = tagValues(ev, "d")[0];
  if (tagged) return tagged;
  const fromContent = content.cardId;
  return typeof fromContent === "string" ? fromContent : "";
}

/** First mint title wins. A move has no title field; a later one would not replace this. */
function mintTitle(content: Record<string, unknown>): string | null {
  const title = content.title;
  if (typeof title !== "string") return null;
  const trimmed = title.trim();
  if (!trimmed) return null;
  // Move lines are `{ from, to, cardId }`. They are not a title edit.
  if (typeof content.to === "string" || typeof content.from === "string") return null;
  return trimmed;
}

function projectKey(projectId: string | null): string {
  return projectId ?? "";
}

export function foldBoard(events: readonly BoardProof[]): FoldedProject[] {
  const ordered = events
    .map((ev, index) => ({ ev, index }))
    .filter(({ ev }) => columnOf(ev.kind) !== null && isOrbittyProof(ev))
    .sort((a, b) => {
      const at = (a.ev.created_at ?? 0) - (b.ev.created_at ?? 0);
      if (at !== 0) return at;
      const aid = a.ev.id ?? "";
      const bid = b.ev.id ?? "";
      if (aid < bid) return -1;
      if (aid > bid) return 1;
      return a.index - b.index;
    });

  const cards = new Map<string, Acc>();
  const projectSeen = new Map<string, number>();
  let seq = 0;

  for (const { ev } of ordered) {
    const column = columnOf(ev.kind);
    if (!column) continue;
    const content = parseContent(ev.content);
    const cardId = cardIdOf(ev, content);
    if (!cardId) continue;
    const projectTag = tagValues(ev, "project")[0] ?? null;
    const title = mintTitle(content);
    const at = ev.created_at ?? 0;
    if (projectTag !== null && !projectSeen.has(projectKey(projectTag))) {
      projectSeen.set(projectKey(projectTag), at);
    }
    const prev = cards.get(cardId);
    if (!prev) {
      cards.set(cardId, {
        cardId,
        title,
        column,
        projectId: projectTag,
        placedAt: at,
        seq: seq++,
      });
      continue;
    }
    prev.column = column;
    prev.placedAt = at;
    prev.seq = seq++;
    if (prev.title === null && title) prev.title = title;
    if (projectTag) prev.projectId = projectTag;
  }

  for (const card of cards.values()) {
    const key = projectKey(card.projectId);
    if (!projectSeen.has(key)) projectSeen.set(key, card.placedAt);
  }

  const groups = new Map<string, Acc[]>();
  for (const card of cards.values()) {
    const key = projectKey(card.projectId);
    const list = groups.get(key);
    if (list) list.push(card);
    else groups.set(key, [card]);
  }

  const keys = [...groups.keys()].sort((a, b) => {
    const at = (projectSeen.get(a) ?? 0) - (projectSeen.get(b) ?? 0);
    if (at !== 0) return at;
    return a < b ? -1 : a > b ? 1 : 0;
  });

  return keys.map((key) => {
    const list = groups.get(key) ?? [];
    const projectId = key === "" ? null : key;
    const columns = {
      todo: [] as FoldedCard[],
      doing: [] as FoldedCard[],
      done: [] as FoldedCard[],
    };
    const sorted = [...list].sort((a, b) => a.placedAt - b.placedAt || a.seq - b.seq);
    for (const card of sorted) {
      columns[card.column].push({
        cardId: card.cardId,
        title: card.title ?? card.cardId,
      });
    }
    return {
      projectId,
      todo: columns.todo,
      doing: columns.doing,
      done: columns.done,
    };
  });
}

export function columnLabel(id: ColumnId): string {
  if (id === "todo") return "Todo";
  if (id === "doing") return "Doing";
  return "Done";
}

export { COLUMNS };
