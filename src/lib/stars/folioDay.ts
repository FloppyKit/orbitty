// Date face of the journal / sky folio. Pure.

export interface FolioPage {
  id?: string;
  createdAt: number;
  dayKey?: string;
  dayKeyEnd?: string;
  scope?: string;
  body?: string;
}

export type FolioKind = "journal" | "cloud";

export interface JournalFace {
  kind: "journal";
  pages: FolioPage[];
}

export interface CloudFace {
  kind: "cloud";
}

export type FolioFace = JournalFace | CloudFace;

export type StripMark = "page" | "cloud" | "none";
export type SpanRole = "start" | "mid" | "end" | "solo" | null;

export interface StripCell {
  day: string;
  mark: StripMark;
  spanRole: SpanRole;
  selected: boolean;
}

export type StarsView = "day" | "week" | "month" | "year" | "custom";

export type ReflectPlan =
  | { ok: true; dayKey: string; dayKeyEnd?: string; scope: "overall" }
  | { ok: false; hint: string };

export function shiftDay(key: string, delta: number): string {
  const [y, m, d] = key.split("-").map(Number);
  const dt = new Date(y, (m || 1) - 1, d || 1);
  dt.setDate(dt.getDate() + delta);
  return dayKeyFromDate(dt);
}

export function dayKeyFromDate(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function dayKeyFromMs(ms: number): string {
  return dayKeyFromDate(new Date(ms));
}

export function formatDayLabel(day: string): string {
  const [y, m, d] = day.split("-").map(Number);
  if (!y || !m || !d) return day;
  return new Date(y, m - 1, d).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

export function formatSpanLabel(from: string, to: string): string {
  if (!to || from === to) return formatDayLabel(from);
  return `${formatDayLabel(from)} – ${formatDayLabel(to)}`;
}

export function pageStart(page: FolioPage): string {
  if (page.dayKey) return page.dayKey;
  return dayKeyFromMs(page.createdAt);
}

export function pageEnd(page: FolioPage): string {
  if (page.dayKeyEnd) return page.dayKeyEnd;
  if (page.dayKey) return page.dayKey;
  return dayKeyFromMs(page.createdAt);
}

/** Inclusive. A page without dayKey uses createdAt's local day. */
export function pageCovers(page: FolioPage, day: string): boolean {
  const a = pageStart(page);
  const b = pageEnd(page);
  const start = a <= b ? a : b;
  const end = a <= b ? b : a;
  return day >= start && day <= end;
}

export function folioFace(
  day: string,
  pages: readonly FolioPage[],
  pageScope: (page: FolioPage) => boolean = () => true,
): FolioFace {
  const covering = pages
    .filter((p) => pageScope(p) && pageCovers(p, day))
    .slice()
    .sort((a, b) => b.createdAt - a.createdAt);
  if (covering.length) return { kind: "journal", pages: covering };
  return { kind: "cloud" };
}

function inclusiveDays(from: string, to: string): string[] {
  if (!from || !to || from > to) return [];
  const out: string[] = [];
  let cur = from;
  for (let n = 0; n < 400 && cur <= to; n++) {
    out.push(cur);
    cur = shiftDay(cur, 1);
  }
  return out;
}

function dayViewDays(center: string, today: string): string[] {
  const anchor = center > today ? today : center;
  let end = shiftDay(anchor, 7);
  if (end > today) end = today;
  let start = shiftDay(anchor, -6);
  let days = inclusiveDays(start, end);
  if (days.length < 14) {
    start = shiftDay(end, -13);
    days = inclusiveDays(start, end);
  }
  if (days.length > 14) days = days.slice(days.length - 14);
  return days.filter((d) => d <= today);
}

function rangeDays(from: string, to: string, today: string, max: number): string[] {
  const end = to > today ? today : to;
  const start = from > end ? end : from;
  let days = inclusiveDays(start, end);
  if (days.length > max) days = days.slice(days.length - max);
  return days;
}

function yearWeeks(from: string, to: string, today: string): string[] {
  const days = rangeDays(from, to, today, 400);
  const weeks: string[] = [];
  for (let i = 0; i < days.length; i += 7) weeks.push(days[i]);
  return weeks;
}

function spanRoleFor(day: string, pages: readonly FolioPage[]): SpanRole {
  const covering = pages.filter((p) => pageCovers(p, day));
  if (!covering.length) return null;
  const span = covering.find((p) => pageStart(p) !== pageEnd(p));
  if (!span) return "solo";
  const s = pageStart(span);
  const e = pageEnd(span);
  if (s === e) return "solo";
  if (day === s) return "start";
  if (day === e) return "end";
  return "mid";
}

function cellFor(
  day: string,
  selected: string,
  pages: readonly FolioPage[],
  hasWords: (day: string) => boolean,
): StripCell {
  const covered = pages.some((p) => pageCovers(p, day));
  return {
    day,
    mark: covered ? "page" : hasWords(day) ? "cloud" : "none",
    spanRole: spanRoleFor(day, pages),
    selected: day === selected,
  };
}

export function stripCells(
  center: string,
  view: StarsView,
  from: string,
  to: string,
  pages: readonly FolioPage[],
  hasWords: (day: string) => boolean,
  today: string,
): StripCell[] {
  let days: string[];
  if (view === "day") days = dayViewDays(center, today);
  else if (view === "year") days = yearWeeks(from || center, to || center, today);
  else if (view === "month") days = rangeDays(from || center, to || center, today, 31);
  else days = rangeDays(from || center, to || center, today, 31);
  return days
    .filter((d) => d <= today)
    .map((d) => cellFor(d, center, pages, hasWords));
}

export function reflectTarget(
  view: string,
  cursor: string,
  from: string,
  to: string,
  today: string,
): ReflectPlan {
  if (view === "year") {
    return { ok: false, hint: "Pick a week, month or custom span" };
  }
  if (view === "day") {
    if (cursor > today) return { ok: false, hint: "Future days are not open yet" };
    return { ok: true, dayKey: cursor, scope: "overall" };
  }
  if (view === "week" || view === "month" || view === "custom") {
    let start = from;
    let end = to;
    if (!start || !end) return { ok: false, hint: "Pick a week, month or custom span" };
    if (start > end) [start, end] = [end, start];
    if (start > today) return { ok: false, hint: "Future days are not open yet" };
    if (end > today) end = today;
    return { ok: true, dayKey: start, dayKeyEnd: end, scope: "overall" };
  }
  return { ok: false, hint: "Pick a week, month or custom span" };
}
