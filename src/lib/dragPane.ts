// Borderless panes drag from chrome. Tauri startDragging() on macOS
// treats the click as the window origin — the pane jumps ("runs away").
// Drag by outerPosition + click offset instead. Click is not a drag:
// wait SLOP. Pointer capture so a small dock keeps the move off-window.
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";

const SLOP = 8;
const IGNORE =
  "button, input, a, select, textarea, option, .field, .volpop, .agentbubble, .agentopt";

export function startPaneDrag(e: MouseEvent | PointerEvent): void {
  if (e.button !== 0) return;
  const t = e.target as HTMLElement | null;
  if (t?.closest(IGNORE)) return;
  e.preventDefault();
  const win = getCurrentWindow();
  const x0 = e.screenX;
  const y0 = e.screenY;
  let grab: { gx: number; gy: number; scale: number } | null = null;
  void win.scaleFactor().then((scale) =>
    win.outerPosition().then((pos) => {
      grab = {
        scale,
        gx: x0 * scale - pos.x,
        gy: y0 * scale - pos.y,
      };
    }),
  );
  let dragging = false;
  const capEl = (e.currentTarget as HTMLElement | null) ?? t;
  const pid = "pointerId" in e ? e.pointerId : null;
  if (pid != null && capEl?.setPointerCapture) {
    try {
      capEl.setPointerCapture(pid);
    } catch {
      /* already released */
    }
  }
  const onMove = (ev: MouseEvent | PointerEvent) => {
    if (!grab) return;
    if (!dragging) {
      if (Math.hypot(ev.screenX - x0, ev.screenY - y0) < SLOP) return;
      dragging = true;
    }
    void win.setPosition(
      new PhysicalPosition(
        Math.round(ev.screenX * grab.scale - grab.gx),
        Math.round(ev.screenY * grab.scale - grab.gy),
      ),
    );
  };
  const onUp = () => {
    if (pid != null && capEl?.hasPointerCapture?.(pid)) {
      try {
        capEl.releasePointerCapture(pid);
      } catch {
        /* ignore */
      }
    }
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", onUp);
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
  window.addEventListener("pointercancel", onUp);
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}
