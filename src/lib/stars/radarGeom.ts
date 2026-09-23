// Five axes from the top, 72° apart. Value 1 sits at the center; 5 at the rim.
// An unset draft 0 draws at the center too.

export const RADAR_R = 52;
export const SPOKE_COUNT = 5;

export function spokeAngle(i: number): number {
  return -Math.PI / 2 + (i * 2 * Math.PI) / SPOKE_COUNT;
}

export function radarVertex(i: number, v: number): [number, number] {
  const a = spokeAngle(i);
  const clamped = Math.max(1, Math.min(5, v || 1));
  const r = ((clamped - 1) / 4) * RADAR_R;
  return [Math.cos(a) * r, Math.sin(a) * r];
}

export function radarRing(v: number): string {
  return Array.from({ length: SPOKE_COUNT }, (_, i) =>
    radarVertex(i, v).join(","),
  ).join(" ");
}

// Project a viewBox point onto spoke i and snap to 1–5.
export function valueFromPointer(i: number, x: number, y: number): number {
  const a = spokeAngle(i);
  const dot = x * Math.cos(a) + y * Math.sin(a);
  const r = Math.max(0, Math.min(RADAR_R, dot));
  return Math.max(1, Math.min(5, Math.round(1 + (r / RADAR_R) * 4)));
}
