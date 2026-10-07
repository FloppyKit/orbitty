/** Pure beside-panel and key-owner transitions for the stage rail.
 *  Nothing here is written to disk. */

export type Beside = null | "stars" | "settings" | "unmapped" | "board";

export type BesidePick =
  | "planet"
  | "stars"
  | "settings"
  | "unmapped"
  | "board"
  | "add"
  | "other";

export type KeyOwner = "rail" | "stars" | "settings" | "unmapped" | "board";

export type WalkTarget = Exclude<KeyOwner, "rail">;

export type KeyEvent =
  | { type: "walkIn"; target: WalkTarget }
  | { type: "walkOut" }
  | { type: "panelClosed" }
  | { type: "select"; other: string };

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type Halo = "ring" | "glow" | "none";

/** Arch front a pick should leave behind. Unmapped is the layer-2 list. */
export type BesideFront = "arch" | "unmapped" | "layer2" | "board" | "keep";

export function pickFromKind(kind: string | null | undefined): BesidePick {
  if (
    kind === "planet" ||
    kind === "stars" ||
    kind === "settings" ||
    kind === "unmapped" ||
    kind === "board" ||
    kind === "add"
  ) {
    return kind;
  }
  return "other";
}

/**
 * One beside panel. A different owner yields that owner's panel
 * (Stars / Settings) or null (planet, Unmapped list, board, add).
 * The same owner is a no-op, so an open Unmapped list or board stays put.
 */
export function besideNext(open: Beside, pick: BesidePick): Beside {
  if (pick === open) return open;
  if (pick === "stars" || pick === "settings") return pick;
  return null;
}

/** Where the arch should sit for this pick. Callers still run archEnter / archOpenBoard. */
export function besideFront(pick: BesidePick): BesideFront {
  if (pick === "stars" || pick === "settings") return "arch";
  if (pick === "unmapped") return "unmapped";
  if (pick === "board") return "board";
  if (pick === "planet") return "layer2";
  return "keep";
}

/** Exactly one owner. walkOut / panelClosed / select(other) all return the rail. */
export function keyOwnerNext(owner: KeyOwner, event: KeyEvent): KeyOwner {
  if (event.type === "walkIn") return event.target;
  if (event.type === "walkOut" || event.type === "panelClosed" || event.type === "select") {
    return "rail";
  }
  return owner;
}

export function archHalo(
  kind: string,
  opts: { center: boolean; active: boolean },
): Halo {
  void kind;
  if (!opts.center && !opts.active) return "none";
  return "ring";
}

/** At most one focus ring is on at a time. */
export function oneFocusOwner(marked: readonly boolean[]): boolean {
  return marked.reduce((n, on) => n + (on ? 1 : 0), 0) <= 1;
}

/** Union of the marble (or glyph) box and the label box, outset by pad. */
export function selWrapBox(marble: Rect, label: Rect, pad = 6): Rect {
  const x = Math.min(marble.x, label.x) - pad;
  const y = Math.min(marble.y, label.y) - pad;
  const right = Math.max(marble.x + marble.w, label.x + label.w) + pad;
  const bottom = Math.max(marble.y + marble.h, label.y + label.h) + pad;
  return { x, y, w: right - x, h: bottom - y };
}

export function outwardDir(edge: string): "left" | "right" | "up" | "down" {
  if (edge === "right") return "right";
  if (edge === "top") return "up";
  if (edge === "bottom") return "down";
  return "left";
}
