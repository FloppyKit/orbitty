// Five axes from the top, 72° apart.
// Center is unset (draft 0). Ring 1 is score 1. The outer ring is 5.
// The hub is not a score.

export const RADAR_R = 52;
export const SPOKE_COUNT = 5;
// View box is wider than the pentagon so a name and its number fit beside each tip.
export const RADAR_VB = { x: -200, y: -92, w: 400, h: 184 };

export function labelPlacement(i: number): {
  x: number;
  y: number;
  anchor: "start" | "middle" | "end";
} {
  const a = spokeAngle(i);
  const c = Math.cos(a);
  const s = Math.sin(a);
  const x = c * (RADAR_R + 12);
  const y = s * (RADAR_R + 12);
  if (c > 0.35) return { x: x + 2, y, anchor: "start" };
  if (c < -0.35) return { x: x - 2, y, anchor: "end" };
  if (s < 0) return { x, y: y - 8, anchor: "middle" };
  return { x, y: y + 12, anchor: "middle" };
}

export function spokeAngle(i: number): number {
  return -Math.PI / 2 + (i * 2 * Math.PI) / SPOKE_COUNT;
}

export function ringRadius(v: number): number {
  const clamped = Math.max(1, Math.min(5, v));
  return (clamped / 5) * RADAR_R;
}

export function radarVertex(i: number, v: number): [number, number] {
  const a = spokeAngle(i);
  if (!v || v < 1) return [0, 0];
  const r = ringRadius(v);
  return [Math.cos(a) * r, Math.sin(a) * r];
}

export function radarRing(v: number): string {
  return Array.from({ length: SPOKE_COUNT }, (_, i) =>
    radarVertex(i, v).join(","),
  ).join(" ");
}

// Project onto spoke i.
// 0 means the hub (inside ring 1) — not a score. Never round the hub up to 1.
export function valueFromPointer(i: number, x: number, y: number): number {
  const a = spokeAngle(i);
  const dot = x * Math.cos(a) + y * Math.sin(a);
  const ring1 = ringRadius(1);
  if (dot < ring1) return 0;
  const r = Math.min(RADAR_R, dot);
  const t = (r - ring1) / (RADAR_R - ring1);
  return Math.max(1, Math.min(5, Math.round(1 + t * 4)));
}

// A click on an unset spoke lands on 3. A click on a set spoke holds.
// After the pointer moves, 1–5 follows the spoke. The hub (0) writes nothing.
export function scoreFromGrab(
  pointerValue: number,
  unset: boolean,
  moved: boolean,
): number | null {
  if (!moved) return unset ? 3 : null;
  if (pointerValue >= 1 && pointerValue <= 5) return pointerValue;
  return null;
}

// Unset: the first arrow lands on 3. After that, step inside 1–5.
export function stepSpoke(current: number, delta: number): number {
  if (!(current >= 1)) return 3;
  const next = Math.round(current) + Math.sign(delta || 0);
  if (!Number.isFinite(next)) return Math.round(current);
  return Math.max(1, Math.min(5, next));
}
