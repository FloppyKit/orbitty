// Plain words for board history (relay/rewind.rs rows, plans, drift).
// Nothing here prints a kind number or a line id.

export interface FieldChange {
  field: string;
  from: unknown;
  to: unknown;
}

export interface HistoryEvent {
  id: string;
  what: "card" | "deleted" | "planet" | "planetGone" | "checkpoint";
  op: string;
  target: string;
  title: string;
  changes: FieldChange[];
  reason?: string;
}

export interface HistoryRow {
  id: string;
  at: number;
  actor: string;
  events: HistoryEvent[];
}

export interface HistoryPage {
  rows: HistoryRow[];
  more: boolean;
  canUndo: boolean;
}

export interface Named {
  id: string;
  title: string;
}

export interface Skipped extends Named {
  why: string;
}

export interface Change extends Named {
  fields: FieldChange[];
}

export interface Plan {
  at: number;
  cards: Change[];
  revive: Named[];
  added: Named[];
  removeAdded: boolean;
  planets: Change[];
  planetsRevive: Named[];
  planetsAdded: Named[];
  removePlanets: boolean;
  skipped: Skipped[];
}

export interface FieldDrift {
  field: string;
  history: unknown;
  now: unknown;
}

export interface ItemDrift extends Named {
  fields: FieldDrift[];
}

export interface Drift {
  cards: ItemDrift[];
  missing: Named[];
  unknown: Named[];
  planets: ItemDrift[];
  planetsMissing: Named[];
  planetsUnknown: Named[];
  missedWrite: boolean;
}

const WHO: Record<string, string> = {
  user: "You",
  ctl: "orbittyctl",
  agent: "Agent",
  rule: "Rule",
  app: "Orbitty",
  shift: "Shift",
  rewind: "Rewind",
  repair: "Repair",
};

export function who(actor: string): string {
  return WHO[actor] ?? "Orbitty";
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

function clock(d: Date): string {
  const h = d.getHours();
  return `${h % 12 || 12}:${String(d.getMinutes()).padStart(2, "0")} ${h < 12 ? "AM" : "PM"}`;
}

/** `5:21 PM`, `Yesterday 9:04 AM`, `Oct 3 9:04 AM`, `Oct 3, 2025 9:04 AM`. */
export function when(at: number, now: Date = new Date()): string {
  const d = new Date(at * 1000);
  const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const ago = Math.round((day(now) - day(d)) / 86_400_000);
  if (ago === 0) return clock(d);
  if (ago === 1) return `Yesterday ${clock(d)}`;
  const date = `${MONTHS[d.getMonth()]} ${d.getDate()}`;
  return d.getFullYear() === now.getFullYear()
    ? `${date} ${clock(d)}`
    : `${date}, ${d.getFullYear()} ${clock(d)}`;
}

const LABEL: Record<string, string> = {
  title: "title",
  status: "status",
  projectId: "planet",
  kind: "kind",
  notes: "notes",
  difficulty: "difficulty",
  subtasks: "subtasks",
  dueAt: "due date",
  tagIds: "tags",
  neededState: "Needed state",
  neededWhy: "Needed question",
  neededAnswer: "answer",
  assignedAgentId: "agent",
  parentId: "parent",
  rating: "rating",
  links: "links",
  completedAt: "done time",
  starred: "star",
  goalAligned: "goal flag",
  clinicalOrMaster: "clinical flag",
  name: "name",
  workspaceRoot: "folder",
};

export function label(field: string): string {
  return LABEL[field] ?? field;
}

const STATUS: Record<string, string> = { todo: "To do", doing: "Doing", done: "Done" };

export function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

function count(v: unknown): number {
  return Array.isArray(v) ? v.length : 0;
}

function changeWords(t: string, ch: FieldChange): string {
  switch (ch.field) {
    case "title":
      return `Renamed "${String(ch.from ?? "")}" to "${String(ch.to ?? "")}"`;
    case "status":
      return `Moved "${t}" to ${STATUS[String(ch.to)] ?? String(ch.to)}`;
    case "notes":
      return `Changed notes on "${t}"`;
    case "tagIds": {
      const [a, b] = [count(ch.from), count(ch.to)];
      if (b > a) return `Added a tag to "${t}"`;
      if (b < a) return `Removed a tag from "${t}"`;
      return `Changed tags on "${t}"`;
    }
    case "links":
      return `Links changed on "${t}"`;
    case "projectId":
      return `Moved "${t}" to another planet`;
    case "starred":
      return ch.to ? `Starred "${t}"` : `Unstarred "${t}"`;
    default:
      return `Changed ${label(ch.field)} on "${t}"`;
  }
}

const REASON: Record<string, string> = {
  keep: "Checkpoint: kept the board as it is",
  "pre-rewind": "Checkpoint before a rewind",
  "pre-repair": "Checkpoint before a repair",
};

export function eventWords(e: HistoryEvent): string {
  const t = e.title;
  switch (e.what) {
    case "checkpoint":
      return REASON[e.reason ?? ""] ?? "Checkpoint";
    case "deleted":
      return `Deleted "${t}"`;
    case "planetGone":
      return `Removed planet "${t}"`;
    case "planet": {
      if (e.op === "add") return `Added planet "${t}"`;
      const name = e.changes.find((c) => c.field === "name");
      if (name) return `Renamed planet "${String(name.from ?? "")}" to "${String(name.to ?? "")}"`;
      return `Changed the folder of planet "${t}"`;
    }
    default: {
      if (e.op === "add") return `Added "${t}"`;
      const words = e.changes.map((c) => changeWords(t, c));
      if (words.length <= 1) return words[0] ?? `Changed "${t}"`;
      return `${words[0]}, and ${plural(words.length - 1, "more change", "more changes")}`;
    }
  }
}

const GROUP: Record<string, (n: number) => string> = {
  links: (n) => `Links changed on ${n} cards`,
  notes: (n) => `Changed notes on ${n} cards`,
  status: (n) => `Moved ${n} cards`,
  title: (n) => `Renamed ${n} cards`,
  tagIds: (n) => `Changed tags on ${n} cards`,
};

/** One line per row. A save that touched several cards reads as one. */
export function rowWords(row: HistoryRow): string {
  const ev = row.events;
  if (ev.length === 1) return eventWords(ev[0]);
  const n = ev.length;
  if (ev.every((e) => e.what === "deleted")) return `Deleted ${n} cards`;
  if (!ev.every((e) => e.what === "card")) return `${n} changes`;
  if (ev.every((e) => e.op === "add")) return `Added ${n} cards`;
  const f = ev[0].changes[0]?.field;
  if (f && ev.every((e) => e.op === "set" && e.changes.length === 1 && e.changes[0].field === f)) {
    return GROUP[f]?.(n) ?? `Changed ${label(f)} on ${n} cards`;
  }
  return `Changed ${n} cards`;
}

/** A value cut to one line for the drift review. */
export function shown(field: string, v: unknown): string {
  if (v == null || v === "" || (Array.isArray(v) && v.length === 0)) return "(empty)";
  if (field === "tagIds" && Array.isArray(v)) return plural(v.length, "tag", "tags");
  if (field === "dueAt" || field === "completedAt") {
    const n = Number(v);
    return Number.isFinite(n) ? when(n > 1e11 ? n / 1000 : n) : String(v);
  }
  if (typeof v === "string") return v.replace(/\s+/g, " ");
  if (Array.isArray(v)) {
    return v
      .map((x) => {
        if (x && typeof x === "object") {
          const o = x as Record<string, unknown>;
          return String(o.label || o.url || o.title || JSON.stringify(o));
        }
        return String(x);
      })
      .join(", ");
  }
  return typeof v === "object" ? JSON.stringify(v) : String(v);
}

/** `12 cards differ from the saved history`, the same as `Drift::headline`. */
export function driftHeadline(d: Drift): string {
  const c = d.cards.length + d.missing.length + d.unknown.length;
  const p = d.planets.length + d.planetsMissing.length + d.planetsUnknown.length;
  const parts: string[] = [];
  if (c > 0) parts.push(plural(c, "card", "cards"));
  if (p > 0) parts.push(plural(p, "planet", "planets"));
  return `${parts.join(" and ")} ${c + p === 1 ? "differs" : "differ"} from the saved history`;
}

export function driftEmpty(d: Drift): boolean {
  return (
    d.cards.length + d.missing.length + d.unknown.length +
      d.planets.length + d.planetsMissing.length + d.planetsUnknown.length ===
    0
  );
}

export const MISSED_WRITE =
  "History could not be written at least once since Orbitty started; repairing may undo a real change.";

/** `Rewind the board to 5:21 PM? 14 cards change. 2 deleted cards come back. 3 cards added since then stay.` */
export function rewindConfirm(at: string, plan: Plan): string {
  const s: string[] = [`Rewind the board to ${at}?`];
  const say = (n: number, one: string, many: string) => {
    if (n > 0) s.push(`${n} ${n === 1 ? one : many}`);
  };
  say(plan.cards.length, "card changes.", "cards change.");
  say(plan.revive.length, "deleted card comes back.", "deleted cards come back.");
  if (plan.removeAdded) {
    say(plan.added.length, "card added since then is removed.", "cards added since then are removed.");
  } else {
    say(plan.added.length, "card added since then stays.", "cards added since then stay.");
  }
  say(plan.planets.length, "planet changes.", "planets change.");
  say(plan.planetsRevive.length, "removed planet comes back.", "removed planets come back.");
  if (s.length === 1) s.push("Nothing changes.");
  return s.join(" ");
}

/** `Also remove those 3 cards`. */
export function alsoRemove(n: number): string {
  return n === 1 ? "Also remove that card" : `Also remove those ${n} cards`;
}

function cap(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** `Restore this card to how it was at 5:21 PM? Title, notes and 2 more fields change.` */
export function restoreConfirm(at: string, plan: Plan): string {
  const head = `Restore this card to how it was at ${at}?`;
  const f = (plan.cards[0]?.fields ?? []).map((x) => label(x.field));
  if (f.length === 0) return `${head} ${plan.revive.length ? "It comes back." : "Nothing changes."}`;
  if (f.length === 1) return `${head} ${cap(f[0])} changes.`;
  if (f.length === 2) return `${head} ${cap(f[0])} and ${f[1]} change.`;
  return `${head} ${cap(f[0])}, ${f[1]} and ${plural(f.length - 2, "more field", "more fields")} change.`;
}

export function skippedWords(s: Skipped): string {
  return `"${s.title}" is skipped: ${s.why}.`;
}

/** After an apply: what happened, in one line. */
export function doneWords(plan: Plan, verb: string): string {
  const n = plan.cards.length + plan.revive.length + (plan.removeAdded ? plan.added.length : 0);
  const p = plan.planets.length + plan.planetsRevive.length;
  if (n + p === 0) return `${verb}: nothing changed.`;
  const parts: string[] = [];
  if (n > 0) parts.push(plural(n, "card", "cards"));
  if (p > 0) parts.push(plural(p, "planet", "planets"));
  return `${verb}: ${parts.join(" and ")} changed.`;
}
