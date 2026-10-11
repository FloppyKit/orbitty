/** DOM rects for the per-card blur. CSS pixels, top-left origin, y down. */

export const GLASS_CARD_SELECTORS = [
  ".l2-window-card",
  ".l2-needed-card",
  ".l2-preview",
  ".b-card",
  ".planet-col",
  ".card",
  ".ghostdock",
  ".panel",
  ".row",
] as const;

export const GLASS_RECT_MIN_MS = 80;
export const GLASS_RECT_IDLE_MS = 240;
export const GLASS_MOTION_FRAMES = 8;
export const GLASS_CARD_CAP = 48;

export type GlassDomRect = { x: number; y: number; w: number; h: number };

/** Intersect a rect with the viewport. Drop slivers under 8px. */
export function clipDomRect(
  x: number,
  y: number,
  w: number,
  h: number,
  vw: number,
  vh: number,
): GlassDomRect | null {
  if (![x, y, w, h, vw, vh].every((n) => Number.isFinite(n))) return null;
  if (vw <= 0 || vh <= 0) return null;
  const x1 = Math.max(x, 0);
  const y1 = Math.max(y, 0);
  const x2 = Math.min(x + w, vw);
  const y2 = Math.min(y + h, vh);
  const cw = x2 - x1;
  const ch = y2 - y1;
  if (cw < 8 || ch < 8) return null;
  return { x: x1, y: y1, w: cw, h: ch };
}

export function rectSignature(rects: readonly GlassDomRect[]): string {
  return rects
    .map((r) => `${Math.round(r.x)}:${Math.round(r.y)}:${Math.round(r.w)}:${Math.round(r.h)}`)
    .join("|");
}

/** Skip an IPC send when the rects did not move, or the last send was too recent. */
/** Fast while rects move (spin or scroll). Idle backs off after a short streak. */
export function nextWatchDelay(unchangedStreak: number): number {
  if (unchangedStreak < GLASS_MOTION_FRAMES) return GLASS_RECT_MIN_MS;
  return GLASS_RECT_IDLE_MS;
}

export function shouldSendGlassRects(
  prevSig: string,
  nextSig: string,
  now: number,
  lastSent: number,
  minMs: number,
): boolean {
  if (nextSig === prevSig) return false;
  if (now - lastSent < minMs) return false;
  return true;
}

export function collectGlassCardRects(doc: Document, vw: number, vh: number): GlassDomRect[] {
  const out: GlassDomRect[] = [];
  for (const sel of GLASS_CARD_SELECTORS) {
    for (const el of doc.querySelectorAll(sel)) {
      if (out.length >= GLASS_CARD_CAP) return out;
      const b = el.getBoundingClientRect();
      const clipped = clipDomRect(b.x, b.y, b.width, b.height, vw, vh);
      if (clipped) out.push(clipped);
    }
  }
  return out;
}
