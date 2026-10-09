// Moved function bodies. scope fields are the lets each function already closed over.

type Invoke = <T = unknown>(cmd: string, args?: any) => Promise<T>;

type Scope_startCompanionDrag = {
  invoke: Invoke;
};

type Scope_onDragMouseDown = {
  startCompanionDrag: (...args: any[]) => any;
};

type Scope_onDockMouseDown = {
  eventElement: (...args: any[]) => Element | null;
  startCompanionDrag: (...args: any[]) => any;
};

export function startCompanionDrag(scope: Scope_startCompanionDrag, e: MouseEvent) {
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

export function onDragMouseDown(scope: Scope_onDragMouseDown, e: MouseEvent) {
  scope.startCompanionDrag(e);
}

export function onDockMouseDown(scope: Scope_onDockMouseDown, e: MouseEvent | PointerEvent) {
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
