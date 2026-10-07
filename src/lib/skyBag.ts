// Word-cloud bag. Tokens are never stored; callers recompute from state.
// Bag law: done titles, notes, tag names, weak planet names, your own
// companion lines. Bot replies never enter. A range keeps companion lines
// only when they carry a real timestamp (none do today).

export interface SkyEntry {
  token: string;
  weight: number;
}

export interface SkyDoc {
  entries: SkyEntry[];
  at: number;
  /** False for companion lines that have no timestamp of their own. */
  stamped: boolean;
}

export interface SkyRange {
  from: string;
  to: string;
}

export interface SkyBagCard {
  status: string;
  title?: string;
  notes?: string;
  tagIds?: string[];
  projectId?: string;
  completedAt?: number | null;
  lastTouchedAt?: number;
}

export interface SkyBagTurn {
  role: string;
  text: string;
  at?: number;
}

export interface SkyBagState {
  kanbanCards?: SkyBagCard[];
  tags?: { id: string; name: string }[];
  projects?: { id: string; name: string }[];
  companion?: {
    turns?: SkyBagTurn[];
    lastUserLine?: string;
  };
}

const STOPWORDS = new Set([
  "the", "and", "for", "with", "this", "that", "you", "your", "yours",
  "are", "was", "were", "will", "would", "can", "could", "should", "have",
  "has", "had", "not", "but", "all", "any", "out", "off", "from", "into",
  "over", "under", "about", "after", "before", "when", "while", "what",
  "how", "why", "who", "which", "there", "here", "then", "than", "too",
  "very", "just", "like", "get", "got", "its", "our", "ours", "their",
  "them", "they", "she", "him", "his", "her", "hers", "mine", "let",
  "lets", "now", "today", "tomorrow", "yesterday", "also", "still",
  "back", "again", "maybe", "really", "need", "needs", "want", "wants",
  "going", "gonna", "yeah", "yes", "okay", "say", "said", "says", "tell",
  "told", "make", "made", "take", "took", "thing", "things", "stuff",
  "way", "lot", "lots", "bit", "one", "two", "new", "old", "see", "seen",
  "know", "think", "feel", "much", "many", "some", "more", "most",
  "other", "another", "same", "own", "only", "even", "well", "good",
  "right", "been", "being", "because", "until", "though", "where",
  "whom", "whose", "each", "every", "both", "few", "per", "via", "etc",
  "else", "must", "might", "shall", "nor", "yet", "either", "neither",
  "through", "during", "without", "within", "don", "didn", "doesn",
  "isn", "aren", "won", "cant", "couldn", "shouldn", "wouldn", "hasn",
  "haven", "hadn", "ain",
]);

export function tokenize(text: string): string[] {
  return text
    .toLowerCase()
    .split(/[^\p{L}]+/u)
    .filter((w) => w.length >= 3 && !STOPWORDS.has(w));
}

export function epochSecToDayKey(epochSec: number): string {
  const d = new Date(epochSec * 1000);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** Docs whose `at` falls in [from 00:00, to 24:00) local time. */
export function docsInRange<T extends { at: number }>(
  docs: readonly T[],
  from: string,
  to: string,
): T[] {
  let start = from;
  let end = to;
  if (start > end) [start, end] = [end, start];
  return docs.filter((doc) => {
    const day = epochSecToDayKey(doc.at);
    return day >= start && day <= end;
  });
}

function pushAll(entries: SkyEntry[], text: string, weight: number) {
  for (const t of tokenize(text)) entries.push({ token: t, weight });
}

function cardDocs(state: SkyBagState): SkyDoc[] {
  const docs: SkyDoc[] = [];
  const tagById = new Map((state.tags ?? []).map((t) => [t.id, t.name]));
  const projById = new Map((state.projects ?? []).map((p) => [p.id, p.name]));
  for (const c of state.kanbanCards ?? []) {
    if (c.status !== "done") continue;
    const entries: SkyEntry[] = [];
    pushAll(entries, c.title ?? "", 1);
    pushAll(entries, c.notes ?? "", 1);
    for (const tid of c.tagIds ?? []) {
      const name = tagById.get(tid);
      if (name) pushAll(entries, name, 1);
    }
    const planet = c.projectId ? projById.get(c.projectId) : undefined;
    if (planet) pushAll(entries, planet, 0.5);
    if (entries.length > 0) {
      docs.push({
        entries,
        at: c.completedAt ?? c.lastTouchedAt ?? 0,
        stamped: true,
      });
    }
  }
  return docs;
}

function companionDocs(state: SkyBagState, nowSec: number, ranged: boolean): SkyDoc[] {
  const docs: SkyDoc[] = [];
  const turns = state.companion?.turns ?? [];
  const userLines: { text: string; at: number | null }[] = [];
  for (const t of turns) {
    if (t.role !== "you") continue;
    const at = typeof t.at === "number" && Number.isFinite(t.at) ? t.at : null;
    userLines.push({ text: t.text, at });
  }
  const lastUser = (state.companion?.lastUserLine ?? "").trim();
  if (lastUser && (userLines[userLines.length - 1]?.text ?? "").trim() !== lastUser) {
    userLines.push({ text: lastUser, at: null });
  }
  for (const line of userLines) {
    const toks = tokenize(line.text);
    if (!toks.length) continue;
    if (ranged && line.at == null) continue;
    docs.push({
      entries: toks.map((t) => ({ token: t, weight: 1 })),
      at: line.at ?? nowSec,
      stamped: line.at != null,
    });
  }
  return docs;
}

/**
 * Build the bag. No range = today's full bag (companion lines included,
 * stamped false, at = now). A range drops unstamped companion lines and
 * keeps docs inside the local days. `filter` is the microtag seam.
 */
export function buildDocs(
  state: SkyBagState,
  range?: SkyRange | null,
  filter?: (doc: SkyDoc) => boolean,
): SkyDoc[] {
  const nowSec = Date.now() / 1000;
  const ranged = !!(range && (range.from || range.to));
  let docs = [...cardDocs(state), ...companionDocs(state, nowSec, ranged)];
  if (ranged && range) {
    const from = range.from || range.to;
    const to = range.to || range.from;
    docs = docsInRange(docs, from, to);
  }
  if (filter) docs = docs.filter(filter);
  return docs;
}

/** The page's own body, as one document, so a journal cloud includes it. */
export function pageCloudDocs(page: {
  body?: string;
  dayKey?: string;
  createdAt?: number;
}): SkyDoc[] {
  const entries = tokenize(page.body ?? "").map((t) => ({ token: t, weight: 1 }));
  if (!entries.length) return [];
  let at = Date.now() / 1000;
  if (page.dayKey && /^\d{4}-\d{2}-\d{2}$/.test(page.dayKey)) {
    const [y, m, d] = page.dayKey.split("-").map(Number);
    at = new Date(y, m - 1, d, 12, 0, 0).getTime() / 1000;
  } else if (typeof page.createdAt === "number") {
    at = page.createdAt > 1e12 ? page.createdAt / 1000 : page.createdAt;
  }
  return [{ entries, at, stamped: true }];
}
