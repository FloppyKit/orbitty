import type { Tracker, TrackerHistory } from "../persist";

export const HISTORY_DAY_CAP = 60;
export const HISTORY_SPAN_CAP = 24;

export function shiftDayKey(key: string, delta: number): string {
  const [y, m, d] = key.split("-").map(Number);
  const dt = new Date(y, (m || 1) - 1, d || 1);
  dt.setDate(dt.getDate() + delta);
  return `${dt.getFullYear()}-${String(dt.getMonth() + 1).padStart(2, "0")}-${String(dt.getDate()).padStart(2, "0")}`;
}

function capDays(days: Record<string, number>): Record<string, number> {
  const keys = Object.keys(days).sort();
  if (keys.length <= HISTORY_DAY_CAP) return days;
  const next: Record<string, number> = {};
  for (const k of keys.slice(keys.length - HISTORY_DAY_CAP)) next[k] = days[k] ?? 0;
  return next;
}

/** Seven end-of-day values, oldest first, ending on `today`. Missing days are 0. */
export function trackerWeek(tr: Tracker, today: string): number[] {
  const days = tr.history?.days ?? {};
  const out: number[] = [];
  for (let i = 6; i >= 0; i--) out.push(days[shiftDayKey(today, -i)] ?? 0);
  return out;
}

/** Today's session spans. Another day's sessions are not shown. */
export function trackerSessions(tr: Tracker, today: string): [number, number][] {
  const sessions = tr.history?.sessions;
  if (!sessions || sessions.day !== today) return [];
  return sessions.spans.map((span) => [span[0], span[1]]);
}

export function withCounterHistory(tr: Tracker, today: string, value: number): Tracker {
  const days = capDays({ ...(tr.history?.days ?? {}), [today]: value });
  const history: TrackerHistory = { ...tr.history, days };
  return { ...tr, value, history };
}

/** Stop folds this session into today's seconds and keeps at most 24 spans. */
export function withTimerStop(tr: Tracker, today: string, nowMs: number): Tracker {
  const sessionSec =
    tr.runningSince == null ? 0 : Math.max(0, Math.round((nowMs - tr.runningSince) / 1000));
  const base = tr.value ?? 0;
  const total =
    tr.kind === "timer" && tr.runningSince != null
      ? Math.round(base + Math.max(0, (nowMs - tr.runningSince) / 1000))
      : Math.round(base);
  const days = capDays({
    ...(tr.history?.days ?? {}),
    [today]: (tr.history?.days?.[today] ?? 0) + sessionSec,
  });
  const start = tr.runningSince ?? nowMs;
  const prev = tr.history?.sessions?.day === today ? tr.history.sessions.spans : [];
  const spans = [...prev, [start, nowMs] as [number, number]].slice(-HISTORY_SPAN_CAP);
  const history: TrackerHistory = { days, sessions: { day: today, spans } };
  const next: Tracker = { ...tr, kind: "timer", value: total, history };
  delete next.runningSince;
  return next;
}
