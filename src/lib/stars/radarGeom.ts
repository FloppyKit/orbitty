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

// Fill only spokes that have a score. Consecutive set spokes share one
// wedge from the hub. A lone spoke is a narrow petal so it has area
// without covering the unset neighbors. A full set is one pentagon.
export function radarFillPolys(values: readonly number[]): string[] {
  const n = SPOKE_COUNT;
  const set = Array.from({ length: n }, (_, i) => (values[i] ?? 0) >= 1);
  const count = set.filter(Boolean).length;
  if (count === 0) return [];
  if (count === n) {
    return [values.map((v, i) => radarVertex(i, v).join(",")).join(" ")];
  }

  let origin = 0;
  for (let i = 0; i < n; i++) {
    if (!set[i] && set[(i + 1) % n]) {
      origin = (i + 1) % n;
      break;
    }
  }

  const runs: number[][] = [];
  let run: number[] = [];
  for (let k = 0; k < n; k++) {
    const i = (origin + k) % n;
    if (set[i]) run.push(i);
    else if (run.length) {
      runs.push(run);
      run = [];
    }
  }
  if (run.length) runs.push(run);

  return runs.map((idxs) => {
    if (idxs.length === 1) return spokePetal(idxs[0], values[idxs[0]]);
    const pts: [number, number][] = [[0, 0]];
    for (const i of idxs) pts.push(radarVertex(i, values[i]));
    pts.push([0, 0]);
    return pts.map((p) => `${p[0]},${p[1]}`).join(" ");
  });
}

function spokePetal(i: number, v: number): string {
  const a = spokeAngle(i);
  const r = ringRadius(v);
  const spread = 0.11;
  const pts: [number, number][] = [
    [0, 0],
    [Math.cos(a - spread) * r * 0.92, Math.sin(a - spread) * r * 0.92],
    [Math.cos(a) * r, Math.sin(a) * r],
    [Math.cos(a + spread) * r * 0.92, Math.sin(a + spread) * r * 0.92],
  ];
  return pts.map((p) => `${p[0]},${p[1]}`).join(" ");
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
