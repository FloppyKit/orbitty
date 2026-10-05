// Pure key decisions for the one MenuRing. No Svelte, so node:test can
// call these directly. The ring still owns stops, highlight, and Esc.

export type RingDir = "left" | "right" | "up" | "down";

export type KeyClass =
  | "next"
  | "prev"
  | "tab-next"
  | "tab-prev"
  | "confirm"
  | "grab"
  | "back"
  | { arrow: RingDir };

export interface KeyInput {
  key: string;
  shiftKey?: boolean;
  metaKey?: boolean;
  ctrlKey?: boolean;
  altKey?: boolean;
  inField?: boolean;
}

const DIRS: readonly RingDir[] = ["left", "right", "up", "down"];

function isDir(s: string): s is RingDir {
  return (DIRS as readonly string[]).includes(s);
}

/** Bare keys are the ring. Chords (⌘/⌃/⌥) are not. A field keeps Tab,
 *  arrows, and Enter. Shift+Enter is grab outside a field. Shift+Tab is
 *  the one shifted key that still reaches the ring. */
export function classifyKey(input: KeyInput): KeyClass | null {
  const shift = !!input.shiftKey;
  const meta = !!input.metaKey;
  const ctrl = !!input.ctrlKey;
  const alt = !!input.altKey;
  const inField = !!input.inField;
  const key = input.key;

  if (meta || ctrl || alt) return null;

  if (key === "Enter" && shift) return inField ? null : "grab";

  if (key === "Tab") {
    if (inField) return null;
    return shift ? "tab-prev" : "tab-next";
  }

  if (shift) return null;
  if (inField) return null;

  if (key === "Escape") return "back";

  if (key.startsWith("Arrow")) {
    const dir = key.slice(5).toLowerCase();
    if (isDir(dir)) return { arrow: dir };
  }

  if (key === "Enter") return "confirm";
  return null;
}

/** Jump to the next or previous surface's first stop. Empty surfaces
 *  are skipped. No highlight yet picks an end (first surface forward,
 *  last surface backward). Wraps. */
export function tabTarget(
  groups: { id: string; stops: string[] }[],
  activeId: string | null,
  dir: 1 | -1,
): string | null {
  const live = groups.filter((g) => g.stops.length > 0);
  if (live.length === 0) return null;
  const end = () => (dir > 0 ? live[0] : live[live.length - 1]);
  if (activeId == null) return end().stops[0] ?? null;
  const gi = live.findIndex((g) => g.stops.includes(activeId));
  if (gi < 0) return end().stops[0] ?? null;
  const next = (gi + dir + live.length) % live.length;
  return live[next].stops[0] ?? null;
}
