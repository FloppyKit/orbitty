import type { Tracker } from "../persist";

export function isTimer(tr: Tracker): boolean {
  return tr.kind === "timer";
}

// Elapsed seconds live in value. A running timer adds time since runningSince.
export function elapsedSec(tr: Tracker, now: number): number {
  const base = tr.value ?? 0;
  if (!isTimer(tr) || tr.runningSince == null) return base;
  return base + Math.max(0, (now - tr.runningSince) / 1000);
}

export function formatElapsed(sec: number): string {
  const s = Math.max(0, Math.floor(sec));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  if (h > 0) {
    return `${h}:${String(m).padStart(2, "0")}:${String(r).padStart(2, "0")}`;
  }
  return `${m}:${String(r).padStart(2, "0")}`;
}
