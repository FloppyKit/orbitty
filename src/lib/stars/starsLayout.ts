// Layout of the Stars surface from its own width. No window measure.

export type StarsLayout = "three" | "dots" | "single";

export interface StarsColumnSpec {
  planet: number;
  right: number;
  rightMin: number;
  rightMax: number;
  middle: string;
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
      right: 300,
      rightMin: 260,
      rightMax: 340,
      middle: "minmax(420px, 1fr)",
    };
  }
  if (layout === "dots") {
    return {
      planet: 72,
      right: 300,
      rightMin: 260,
      rightMax: 340,
      middle: "minmax(360px, 1fr)",
    };
  }
  return {
    planet: 0,
    right: 0,
    rightMin: 260,
    rightMax: 340,
    middle: "minmax(0, 1fr)",
  };
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
