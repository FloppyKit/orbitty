// Borderless panes drag from chrome. Tauri startDragging() jumps the
// frame to the click. setPosition is the wrong space. After slop, native
// pane_drag_begin follows NSEvent mouseLocation (same path as the dock).
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const SLOP = 8;
const IGNORE =
  "button, input, a, select, textarea, option, .field, .volpop, .agentbubble, .agentopt, .b-card, .kn-item, .k-doing";

/** Clicking a button's text node is a Text target — closest() is not on it. */
export function eventElement(e: Event): Element | null {
  const t = e.target;
  if (t instanceof Element) return t;
  if (t instanceof CharacterData) return t.parentElement;
  return null;
}

export function startPaneDrag(e: MouseEvent | PointerEvent): void {
  if (e.button !== 0) return;
  const t = eventElement(e);
  if (t?.closest(IGNORE)) return;
  const x0 = e.screenX;
  const y0 = e.screenY;
  const label = getCurrentWindow().label;
  const onMove = (ev: MouseEvent | PointerEvent) => {
    if (Math.hypot(ev.screenX - x0, ev.screenY - y0) < SLOP) return;
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("mouseup", onUp);
    invoke("pane_drag_begin", { label }).catch(() => {});
  };
  const onUp = () => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("mouseup", onUp);
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("mousemove", onMove);
  window.addEventListener("pointerup", onUp);
  window.addEventListener("mouseup", onUp);
}
