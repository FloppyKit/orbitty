// In-window expand chord. Not a global shortcut. Null means the key
// belongs to the field, the IME, or some other chord.

export type ExpandKeyEvent = {
  key: string;
  metaKey?: boolean;
  ctrlKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
  isComposing?: boolean;
};

export type ExpandKeyState = {
  composing?: boolean;
};

/** ⌘⇧E (Ctrl+Shift+E counts the same) toggles. Plain E, ⌘E, and composing do not. */
export function expandKey(e: ExpandKeyEvent, state: ExpandKeyState = {}): "toggle" | null {
  if (state.composing || e.isComposing) return null;
  if (e.altKey) return null;
  if (e.key !== "e" && e.key !== "E") return null;
  const command = !!(e.metaKey || e.ctrlKey);
  if (!command || !e.shiftKey) return null;
  return "toggle";
}
