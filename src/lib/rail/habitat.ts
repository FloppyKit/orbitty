/** Pure inspector-habitat decisions. No Svelte. node:test imports this file. */

export type BoardCol = "todo" | "doing" | "done";

export const BOARD_COLS: readonly BoardCol[] = ["todo", "doing", "done"];

/** Tile grid width. WindowTiles repeats this many columns. */
export const TILE_COLS = 3;

const COL_SET = new Set<string>(BOARD_COLS);

export interface HabitatCard {
  id: string;
  projectId: string;
  title?: string;
  kind?: string | null;
  status?: string | null;
  neededState?: string | null;
  createdAt?: number;
  lastTouchedAt?: number;
  completedAt?: number | null;
  windowId?: string | null;
}

export interface HabitatRow {
  id: string;
  sleeping?: boolean;
  isMinimized?: boolean;
  isFrontmost?: boolean;
}

export type TileState = "open" | "stowed" | "asleep";

export interface HabitatSection {
  pid: string;
  open: boolean;
  tileIds: string[];
  snapCols: string[][];
  hasTodos: boolean;
}

export interface HabitatStop {
  id: string;
  group: string;
}

export type HabitatDir = "left" | "right" | "up" | "down";

export interface SnapColumn<T> {
  id: "needed" | BoardCol;
  items: T[];
  more: number;
}

function isBoardCol(v: unknown): v is BoardCol {
  return v === "todo" || v === "doing" || v === "done";
}

/** Non-array → ["todo"]. Known ids only, deduped, canonical order.
 *  "needed" is dropped (always on, never stored). [] is valid. */
export function normalizeCols(raw: unknown): BoardCol[] {
  if (!Array.isArray(raw)) return ["todo"];
  const keep = new Set<BoardCol>();
  for (const item of raw) {
    if (isBoardCol(item)) keep.add(item);
  }
  return BOARD_COLS.filter((c) => keep.has(c));
}

/** Flip one stored column. "needed" is not stored, so the list is unchanged. */
export function toggleCol(cols: readonly string[], col: string): BoardCol[] {
  if (col === "needed") return cols.filter(isBoardCol);
  if (!COL_SET.has(col)) return normalizeCols(cols);
  const cur = normalizeCols(cols);
  const next = cur.includes(col as BoardCol)
    ? cur.filter((c) => c !== col)
    : [...cur, col as BoardCol];
  return normalizeCols(next);
}

function byCreated<T extends HabitatCard>(a: T, b: T): number {
  return (a.createdAt ?? 0) - (b.createdAt ?? 0);
}

/** Open Neededs, then later Neededs. Same rule as NeededInbox.neededSorted,
 *  plus the planet filter. */
function neededFor<T extends HabitatCard>(cards: readonly T[], pid: string): T[] {
  return cards
    .filter(
      (c) =>
        c.projectId === pid &&
        c.kind === "needed" &&
        (c.neededState === "open" || c.neededState === "later"),
    )
    .sort((a, b) => {
      const al = a.neededState === "later" ? 1 : 0;
      const bl = b.neededState === "later" ? 1 : 0;
      if (al !== bl) return al - bl;
      if (al === 1) return (a.lastTouchedAt ?? 0) - (b.lastTouchedAt ?? 0);
      return byCreated(a, b);
    });
}

function statusFor<T extends HabitatCard>(
  cards: readonly T[],
  pid: string,
  status: BoardCol,
): T[] {
  const rows = cards.filter(
    (c) => c.projectId === pid && c.kind !== "needed" && c.status === status,
  );
  if (status === "done") {
    rows.sort((a, b) => (b.completedAt ?? 0) - (a.completedAt ?? 0));
  } else {
    rows.sort(byCreated);
  }
  return rows;
}

function cut<T>(id: "needed" | BoardCol, rows: T[], cap: number): SnapColumn<T> {
  const n = Math.max(0, cap);
  return { id, items: rows.slice(0, n), more: Math.max(0, rows.length - n) };
}

/** Needed first (omitted only when empty), then the chosen columns in
 *  canonical order. Window-linked cards count. */
export function boardSnapshot<T extends HabitatCard>(
  cards: readonly T[],
  pid: string,
  cols: readonly string[],
  cap = 4,
): SnapColumn<T>[] {
  const out: SnapColumn<T>[] = [];
  const needed = neededFor(cards, pid);
  if (needed.length > 0) out.push(cut("needed", needed, cap));
  for (const col of normalizeCols(cols)) {
    out.push(cut(col, statusFor(cards, pid, col), cap));
  }
  return out;
}

/** Open todos for the Todos control. Same filter as RailPage.todosFor. */
export function planetCount(cards: readonly HabitatCard[], pid: string): number {
  return cards.filter(
    (c) =>
      c.kind !== "needed" &&
      c.status !== "done" &&
      !c.windowId &&
      c.projectId === pid,
  ).length;
}

function tileState(row: HabitatRow): TileState {
  if (row.sleeping) return "asleep";
  if (row.isMinimized) return "stowed";
  return "open";
}

export function tiles(
  rows: readonly HabitatRow[],
  max = 6,
): { shown: { id: string; state: TileState }[]; more: number } {
  const n = Math.max(0, max);
  const all = rows.map((r) => ({ id: r.id, state: tileState(r) }));
  return { shown: all.slice(0, n), more: Math.max(0, all.length - n) };
}

/** Asleep rows are skipped. Minimized live ids are restored, then every
 *  live id is focused with the frontmost last. Both lists cap together. */
export function raisePlan(
  rows: readonly HabitatRow[],
  cap = 12,
): { restore: string[]; focus: string[] } {
  const live = rows.filter((r) => !r.sleeping);
  if (live.length === 0 || cap <= 0) return { restore: [], focus: [] };
  let frontAt = -1;
  live.forEach((r, i) => {
    if (r.isFrontmost) frontAt = i;
  });
  let chosen: HabitatRow[];
  if (frontAt < 0) {
    chosen = live.slice(0, cap);
  } else {
    const front = live[frontAt];
    const others = live.filter((_, i) => i !== frontAt);
    chosen = [...others.slice(0, Math.max(0, cap - 1)), front];
  }
  return {
    restore: chosen.filter((r) => r.isMinimized).map((r) => r.id),
    focus: chosen.map((r) => r.id),
  };
}

export function habitatStops(sections: readonly HabitatSection[]): HabitatStop[] {
  const stops: HabitatStop[] = [
    { id: "rstars", group: "front" },
    { id: "rsettings", group: "front" },
    ...BOARD_COLS.map((c) => ({
      id: `rcol-${c}`,
      group: "cols",
    })),
  ];
  for (const s of sections) {
    stops.push({ id: `rhead-${s.pid}`, group: s.pid });
    if (!s.open) continue;
    stops.push({ id: `rraise-${s.pid}`, group: s.pid });
    for (const id of s.tileIds) stops.push({ id: `rtile-${id}`, group: s.pid });
    for (const col of s.snapCols) {
      for (const id of col) stops.push({ id: `rsnap-${id}`, group: s.pid });
    }
    if (s.hasTodos) stops.push({ id: `rtodos-${s.pid}`, group: s.pid });
  }
  return stops;
}

function step(ids: string[], id: string, delta: number): string | null {
  const i = ids.indexOf(id);
  if (i < 0) return null;
  return ids[i + delta] ?? null;
}

function tileArrow(
  section: HabitatSection,
  wid: string,
  dir: HabitatDir,
): string | null {
  const ti = section.tileIds.indexOf(wid);
  if (ti < 0) return null;
  const col = ti % TILE_COLS;
  if (dir === "right") {
    if (col + 1 >= TILE_COLS) return null;
    const next = section.tileIds[ti + 1];
    return next ? `rtile-${next}` : null;
  }
  if (dir === "left") {
    if (col === 0) return null;
    const prev = section.tileIds[ti - 1];
    return prev ? `rtile-${prev}` : null;
  }
  return null;
}

function snapArrow(
  section: HabitatSection,
  cardId: string,
  dir: HabitatDir,
): string | null {
  let ci = -1;
  let ri = -1;
  section.snapCols.forEach((col, i) => {
    const r = col.indexOf(cardId);
    if (r >= 0) {
      ci = i;
      ri = r;
    }
  });
  if (ci < 0) return null;
  if (dir !== "left" && dir !== "right") return null;
  const dest = section.snapCols[ci + (dir === "right" ? 1 : -1)];
  if (!dest || dest.length === 0) return null;
  return `rsnap-${dest[Math.min(ri, dest.length - 1)]}`;
}

export type HabitatConfirm =
  | "expand"
  | "raise"
  | "toggleCol"
  | "focusTile"
  | "openSnap"
  | "todos"
  | "stars"
  | "settings";

/** Ring Enter for a habitat stop. Head expands; raise is its own stop.
 *  Stars and Settings are the stage-strip front, never a planet raise. */
export function habitatConfirm(id: string): HabitatConfirm | null {
  if (id === "rstars") return "stars";
  if (id === "rsettings") return "settings";
  if (id.startsWith("rhead-") && id.length > "rhead-".length) return "expand";
  if (id.startsWith("rraise-") && id.length > "rraise-".length) return "raise";
  if (id.startsWith("rcol-") && id.length > "rcol-".length) return "toggleCol";
  if (id.startsWith("rtile-") && id.length > "rtile-".length) return "focusTile";
  if (id.startsWith("rsnap-") && id.length > "rsnap-".length) return "openSnap";
  if (id.startsWith("rtodos-") && id.length > "rtodos-".length) return "todos";
  return null;
}

/** ↑/↓ walk the painted stop list. ←/→ stay inside a tile row or hop
 *  snapshot columns. A move that would leave that row returns null. */
export function habitatArrow(
  sections: readonly HabitatSection[],
  activeId: string | null,
  dir: HabitatDir,
): string | null {
  if (!activeId) return null;
  const ids = habitatStops(sections).map((s) => s.id);
  if (!ids.includes(activeId)) return null;

  if (activeId.startsWith("rtile-")) {
    const wid = activeId.slice("rtile-".length);
    const section = sections.find((s) => s.open && s.tileIds.includes(wid));
    if (!section) return null;
    if (dir === "left" || dir === "right") return tileArrow(section, wid, dir);
  } else if (activeId.startsWith("rsnap-")) {
    const cardId = activeId.slice("rsnap-".length);
    const section = sections.find(
      (s) => s.open && s.snapCols.some((col) => col.includes(cardId)),
    );
    if (!section) return null;
    if (dir === "left" || dir === "right") return snapArrow(section, cardId, dir);
  } else if (
    (dir === "left" || dir === "right") &&
    activeId.startsWith("rcol-")
  ) {
    const cols = BOARD_COLS.map((c) => `rcol-${c}`);
    const i = cols.indexOf(activeId);
    if (i < 0) return null;
    return cols[i + (dir === "right" ? 1 : -1)] ?? null;
  } else if (dir === "left" || dir === "right") {
    return null;
  }

  if (dir === "up" || dir === "down") return step(ids, activeId, dir === "down" ? 1 : -1);
  return null;
}

/** Screen edge the stage strip parks against. Missing or unknown → left. */
export type RailEdge = "left" | "right" | "top" | "bottom";

export const RAIL_EDGES: readonly RailEdge[] = ["left", "right", "top", "bottom"];

/** Distinct from `autohideRowChrome` (row-button chrome). */
export const RAIL_AUTO_HIDE_PREF = "railAutoHideOnBlur";

export const ROW_CHROME_PREF = "autohideRowChrome";

/** ⌘⇧O. Rail owns this chord. Companion stays on ⌥⌘O / tray / dock. */
export const RAIL_REVEAL_ACTION = "rail-reveal";

const RAIL_EDGE_SET = new Set<string>(RAIL_EDGES);

export function normalizeRailEdge(raw: unknown): RailEdge {
  return typeof raw === "string" && RAIL_EDGE_SET.has(raw) ? (raw as RailEdge) : "left";
}

export interface StagePrefs {
  railEdge: RailEdge;
  railAutoHideOnBlur: boolean;
  autohideRowChrome: boolean;
}

/** Per-machine stage prefs. Auto-hide defaults on and is not the row-chrome flag. */
export function stagePrefs(raw: {
  railEdge?: unknown;
  railAutoHideOnBlur?: unknown;
  autohideRowChrome?: unknown;
} | null | undefined): StagePrefs {
  const src = raw ?? {};
  return {
    railEdge: normalizeRailEdge(src.railEdge),
    railAutoHideOnBlur:
      typeof src.railAutoHideOnBlur === "boolean" ? src.railAutoHideOnBlur : true,
    autohideRowChrome:
      typeof src.autohideRowChrome === "boolean" ? src.autohideRowChrome : true,
  };
}

/** Hover near the preferred edge OR the rail hotkey. Either one reveals. */
export function revealWanted(hoverEdge: boolean, hotkey: boolean): boolean {
  return hoverEdge || hotkey;
}

/** Hide only when the auto-hide pref is on, focus has left, and the pointer
 *  is outside the strip and the edge hit-zone. */
export function autoHideWanted(opts: {
  railAutoHideOnBlur: boolean;
  focusInside: boolean;
  pointerInside: boolean;
}): boolean {
  if (!opts.railAutoHideOnBlur) return false;
  if (opts.focusInside || opts.pointerInside) return false;
  return true;
}

/** Collapsed icon order: Stars, Settings, then planets. */
export function stageStripIds(planetIds: readonly string[]): string[] {
  return ["stars", "settings", ...planetIds];
}

export type StageRole = "icon" | "open" | "receded";

/** One open id. Everyone else recedes. None open → every id is an icon. */
export function stageRoles(
  ids: readonly string[],
  openId: string | null,
): { id: string; role: StageRole }[] {
  const open = openId && ids.includes(openId) ? openId : null;
  return ids.map((id) => ({
    id,
    role: !open ? "icon" : id === open ? "open" : "receded",
  }));
}

/** Click a planet: that one opens and the previous one closes.
 *  Click the open planet again (back) and the pack returns to icons. */
export function applyPlanetOpen(
  openId: string | null,
  planetIds: readonly string[],
  clicked: string,
): string | null {
  if (!planetIds.includes(clicked)) return openId && planetIds.includes(openId) ? openId : null;
  if (openId === clicked) return null;
  return clicked;
}

export interface StageFront {
  id: "stars" | "settings";
  z: number;
}

/** Stars and Settings stay in front of an expanded planet. */
export function stageFront(openId: string | null): { front: StageFront[]; openZ: number } {
  void openId;
  return {
    front: [
      { id: "stars", z: 40 },
      { id: "settings", z: 39 },
    ],
    openZ: 10,
  };
}

/** Global ⌘⇧O maps to rail reveal, never companion summon. */
export function chordAction(chord: string): string | null {
  const c = chord.trim().toLowerCase().replace(/\s+/g, "");
  if (c === "cmd+shift+o" || c === "⌘⇧o") return RAIL_REVEAL_ACTION;
  if (c === "cmd+alt+o" || c === "⌥⌘o") return "companion-open";
  return null;
}

/** At most one section open. A save where every known id was open
 *  (nothing stored collapsed) becomes the icon strip. */
export function stageCollapsedIds(
  saved: readonly string[],
  sectionIds: readonly string[],
): string[] {
  if (sectionIds.length === 0) return [...saved];
  const known = new Set(sectionIds);
  const extra = saved.filter((id) => !known.has(id));
  const collapsed = saved.filter((id) => known.has(id));
  const open = sectionIds.filter((id) => !collapsed.includes(id));
  if (collapsed.length === 0 && open.length === sectionIds.length) {
    return [...extra, ...sectionIds];
  }
  if (open.length <= 1) return [...extra, ...collapsed];
  const keep = open[0];
  return [...extra, ...sectionIds.filter((id) => id !== keep)];
}
