// Layout of the Stars surface from its own width. No window measure.
// Three and dots: planets | radar column (radar, DayPositives, trackers) | journal.
// The journal never shares a column with the radar or the trackers.

export type StarsLayout = "three" | "dots" | "single";

export interface StarsColumnSpec {
  planet: number;
  /** Radar column: radar, DayPositives, then the trackers grid. */
  middle: string;
  middleMin: number;
  /** Journal / sky folio. Its own column. */
  journal: string;
  journalMin: number;
}

/** ≥ 900 three columns, 600–899 dots, under 600 one column. */
export function starsLayout(widthPx: number): StarsLayout {
  if (widthPx >= 900) return "three";
  if (widthPx >= 600) return "dots";
  return "single";
}

export function starsColumns(layout: StarsLayout): StarsColumnSpec {
  if (layout === "three") {
    return {
      planet: 168,
      middle: "minmax(360px, 1fr)",
      middleMin: 360,
      journal: "minmax(320px, 0.9fr)",
      journalMin: 320,
    };
  }
  if (layout === "dots") {
    return {
      planet: 72,
      middle: "minmax(0, 1fr)",
      middleMin: 0,
      journal: "minmax(260px, 1fr)",
      journalMin: 260,
    };
  }
  return {
    planet: 0,
    middle: "minmax(0, 1fr)",
    middleMin: 0,
    journal: "minmax(0, 1fr)",
    journalMin: 0,
  };
}

/** CSS grid template. Single is one column; the segments pick the body. */
export function starsGridTemplate(layout: StarsLayout, cols: StarsColumnSpec = starsColumns(layout)): string {
  if (layout === "single") return "minmax(0, 1fr)";
  return `${cols.planet}px ${cols.middle} ${cols.journal}`;
}

export interface StarsScopeTargets {
  trackers: string;
  radar: "overall";
  journal: "overall";
  sky: "overall";
}

/** Planet selection filters trackers only. The other three stay overall. */
export function starsScopeTargets(scope: string): StarsScopeTargets {
  return {
    trackers: scope,
    radar: "overall",
    journal: "overall",
    sky: "overall",
  };
}
