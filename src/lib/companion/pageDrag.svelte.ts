// @ts-nocheck
// Moved function bodies. scope fields are the lets each function already closed over.
export function startCompanionDrag(scope: any, e: MouseEvent) {
  if (e.button !== 0) return;
  const x0 = e.screenX;
  const y0 = e.screenY;
  const move = (ev: MouseEvent) => {
    if (Math.hypot(ev.screenX - x0, ev.screenY - y0) < 12) return;
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", onUp);
    scope.invoke("companion_drag_begin").catch(() => {});
  };
  const onUp = () => {
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", onUp);
  };
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", onUp);
}

export function onDragMouseDown(scope: any, e: MouseEvent) {
  scope.startCompanionDrag(e);
}

export function onDockMouseDown(scope: any, e: MouseEvent | PointerEvent) {
  if (e.button !== 0) return;
  const t = scope.eventElement(e);
  if (
    t?.closest(
      "button, input, a, select, textarea, option, .field, .fieldwrap, .volpop, .agentbubble, .agentopt",
    )
  )
    return;
  scope.startCompanionDrag(e);
}
