/** PEER-CHROME — demo hides peer directory chrome; tray/Dock stay the sole map. */

import type { Claim, KanbanCard, PersistedState } from "./persist";

/** 3 claims or any Done card = ready to graduate (atlas). */
export function peerChromeAuto(s: {
  claims?: Claim[] | null;
  kanbanCards?: KanbanCard[] | null;
}): boolean {
  const claims = s.claims?.length ?? 0;
  if (claims >= 3) return true;
  return (s.kanbanCards ?? []).some(
    (c) => c.status === "done" || (c.completedAt != null && c.completedAt > 0),
  );
}

/** Stored flag only — Settings can force off after graduation. */
export function peerChromeOn(s: PersistedState | null | undefined): boolean {
  return s?.peerChrome === true;
}

/** Persist graduation once when demo threshold is met. */
export function maybeGraduatePeerChrome(s: PersistedState): boolean {
  if (s.peerChrome) return false;
  if (!peerChromeAuto(s)) return false;
  s.peerChrome = true;
  return true;
}
