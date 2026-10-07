// Five axes from the top, 72° apart.
// Center is unset (draft 0). Ring 1 is score 1. The outer ring is 5.
// The hub is not a score.

export const RADAR_R = 72;
export const SPOKE_COUNT = 5;
/** Visible knob radius in SVG units. Diameter is 14 at scale 1. */
export const HANDLE_R = 7;

export const STARS_SPOKES = [
  { key: "sleep", label: "Sleep" },
  { key: "terroir", label: "Terroir" },
  { key: "athleticism", label: "Athleticism" },
  { key: "relationships", label: "Relationships" },
  { key: "self", label: "Self practice" },
] as const;

export type SpokeKey = (typeof STARS_SPOKES)[number]["key"];
// View box clears the larger cage plus a name and its score mark.
export const RADAR_VB = { x: -176, y: -120, w: 360, h: 224 };

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

/** SVG hit radius whose CSS radius stays ≥ 14px. `scale` is CSS px per SVG unit. */
export function handleHitRadius(scale: number): number {
  const s = Number.isFinite(scale) && scale > 0 ? scale : 1;
  return 14 / s;
}

/** SVG visible radius whose CSS radius stays ≥ 7px. */
export function handleVisibleRadius(scale: number): number {
  const s = Number.isFinite(scale) && scale > 0 ? scale : 1;
  return Math.max(HANDLE_R, HANDLE_R / s);
}

/**
 * Center radius for a handle. At value 1 the ring is already past the
 * chord needed for a 7px knob; a larger visible radius (small embeds)
 * pushes the center out so neighbors do not overlap.
 */
export function handleCenterRadius(value: number, visibleR: number): number {
  const base = ringRadius(value >= 1 ? value : 1);
  const r = visibleR > 0 ? visibleR : HANDLE_R;
  const minR = r / Math.sin(Math.PI / SPOKE_COUNT);
  return Math.max(base, minR);
}

/** Distance along the spoke, in SVG units, snapped to 1–5. */
export function snapOnSpoke(distance: number, radius = RADAR_R): number {
  if (!Number.isFinite(distance) || !(radius > 0)) return 1;
  const t = Math.max(0, Math.min(1, distance / radius));
  const v = Math.round(t * 5);
  return Math.max(1, Math.min(5, v === 0 ? 1 : v));
}

export function spokeDistance(i: number, x: number, y: number): number {
  const a = spokeAngle(i);
  return x * Math.cos(a) + y * Math.sin(a);
}

/** " 4 " → 4. Anything that is not an integer 1–5 is null. */
export function parseRating(text: string): number | null {
  const t = text.trim();
  if (!/^[1-5]$/.test(t)) return null;
  return Number(t);
}

/**
 * Slider keys. Returns the next 1–5, 0 to clear, or null when ignored.
 * ← ↓ −1 (floor 1), → ↑ +1 (cap 5), Home 1, End 5, digits 1–5 set.
 */
export function ratingFromKey(current: number, key: string): number | null {
  const cur = current >= 1 && current <= 5 ? Math.round(current) : 1;
  switch (key) {
    case "ArrowLeft":
    case "ArrowDown":
      return Math.max(1, cur - 1);
    case "ArrowRight":
    case "ArrowUp":
      return Math.min(5, cur + 1);
    case "Home":
      return 1;
    case "End":
      return 5;
    case "1":
    case "2":
    case "3":
    case "4":
    case "5":
      return Number(key);
    case "Backspace":
    case "Delete":
      return 0;
    default:
      return null;
  }
}

export interface RadarDraftValues {
  sleep: number;
  terroir: number;
  athleticism: number;
  relationships: number;
  self: number;
}

export interface RadarDraft {
  values: RadarDraftValues | null;
  draft: boolean;
}

const DRAFT_THREES: RadarDraftValues = {
  sleep: 3,
  terroir: 3,
  athleticism: 3,
  relationships: 3,
  self: 3,
};

/** Saved check-in wins. Today with none is a draft at 3. Any other empty day stays empty. */
export function radarDraft(
  dayKey: string,
  today: string,
  checkins: Record<string, Partial<RadarDraftValues> | undefined>,
): RadarDraft {
  const saved = checkins[dayKey];
  if (saved) {
    return {
      values: {
        sleep: saved.sleep ?? 0,
        terroir: saved.terroir ?? 0,
        athleticism: saved.athleticism ?? 0,
        relationships: saved.relationships ?? 0,
        self: saved.self ?? 0,
      },
      draft: false,
    };
  }
  if (dayKey === today) return { values: { ...DRAFT_THREES }, draft: true };
  return { values: null, draft: false };
}
