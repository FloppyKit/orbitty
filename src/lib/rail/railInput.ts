/** One input contract for the rail. Nothing here is written to disk. */

// @ts-expect-error Node's test runner needs the .ts suffix; the bundler flag for it is off.
import { SPIN_STEP_PX, spinAccumulate, type SpinAccumState } from "./stageLook.ts";

export type InputIntent = "in" | "out" | "move" | "none";
export type ArchFrontName = "arch" | "layer2" | "board";

export interface IntentInput {
  kind: "key" | "click" | "pointer" | "hover" | "wheel";
  key?: string;
  /** DOM `MouseEvent.button`: 3 = mouse Back (button 4), 4 = mouse Forward (button 5). */
  button?: number;
  onTarget?: boolean;
  empty?: boolean;
}

/** Return / click-on-target / mouse Forward → in.
 *  Esc / mouse Back / empty-click → out.
 *  Arrows, hover, and the wheel → move. */
export function inputIntent(ev: IntentInput): InputIntent {
  if (ev.kind === "wheel" || ev.kind === "hover") return "move";
  if (ev.kind === "key") {
    if (ev.key === "Enter") return "in";
    if (ev.key === "Escape") return "out";
    if (
      ev.key === "ArrowLeft" ||
      ev.key === "ArrowRight" ||
      ev.key === "ArrowUp" ||
      ev.key === "ArrowDown"
    ) {
      return "move";
    }
    return "none";
  }
  if (ev.kind === "click") {
    if (ev.onTarget) return "in";
    if (ev.empty) return "out";
    return "none";
  }
  if (ev.kind === "pointer") {
    if (ev.button === 4) return "in";
    if (ev.button === 3) return "out";
    return "none";
  }
  return "none";
}

/** One level out. The arch is the root: hide the rail, do not pop further. */
export function depthOut(front: ArchFrontName): { front: ArchFrontName; hide: boolean } {
  if (front === "board") return { front: "layer2", hide: false };
  if (front === "layer2") return { front: "arch", hide: false };
  return { front: "arch", hide: true };
}

export interface CrumbPart {
  text: string;
  current: boolean;
}

export function railCrumbParts(
  front: ArchFrontName,
  pid: string | null,
  names: Readonly<Record<string, string>>,
): CrumbPart[] {
  const name = (pid && names[pid]) || "Planet";
  if (front === "board") {
    return [
      { text: "Planets", current: false },
      { text: name, current: false },
      { text: "Board", current: true },
    ];
  }
  if (front === "layer2") {
    return [
      { text: "Planets", current: false },
      { text: name, current: true },
    ];
  }
  return [{ text: "Planets", current: true }];
}

export function railCrumb(
  front: ArchFrontName,
  pid: string | null,
  names: Readonly<Record<string, string>>,
): string {
  return railCrumbParts(front, pid, names)
    .map((p) => p.text)
    .join(" › ");
}

/** Gutter between previews is a quarter of the preview width. */
export function archGutter(previewW: number): number {
  if (!Number.isFinite(previewW) || previewW <= 0) return 0;
  return previewW * 0.25;
}

export interface HitRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Union of the drawn marble and its label, outset by at least 8px. */
export function hitBox(marble: HitRect, label: HitRect, pad = 8): HitRect {
  const x = Math.min(marble.x, label.x) - pad;
  const y = Math.min(marble.y, label.y) - pad;
  const right = Math.max(marble.x + marble.w, label.x + label.w) + pad;
  const bottom = Math.max(marble.y + marble.h, label.y + label.h) + pad;
  return { x, y, w: right - x, h: bottom - y };
}

export interface DismissInput {
  phase: "pointerdown" | "pointerup";
  dragPx: number;
  overHit: boolean;
  overPanel: boolean;
  animating: boolean;
}

/** True empty backdrop dismisses on pointer-up only, with no drag, off every
 *  hit and panel, and never while the wheel is still spinning. */
export function emptyDismiss(input: DismissInput): boolean {
  if (input.phase !== "pointerup") return false;
  if (input.dragPx >= 4) return false;
  if (input.overHit || input.overPanel) return false;
  if (input.animating) return false;
  return true;
}

/** A click during a spin lands on the item that is arriving, not the backdrop. */
export function spinClickTarget(
  animating: boolean,
  onHit: boolean,
): "landing" | "item" {
  if (animating && !onHit) return "landing";
  return "item";
}

export const SPIN_FLICK_MS = 120;
export const SPIN_FLICK_MAX = 3;

export interface SpinCapState {
  spin: SpinAccumState;
  windowStart: number;
  windowSteps: number;
}

export const SPIN_CAP_IDLE: SpinCapState = {
  spin: { residual: 0, lastAt: 0 },
  windowStart: 0,
  windowSteps: 0,
};

/** Line/page: one step per event. Pixel: at most SPIN_FLICK_MAX steps per
 *  120ms, so one 600px flick cannot walk the whole wheel. `spinAccumulate`
 *  still owns the residual. */
export function spinStepsCap(
  state: SpinCapState,
  ev: { deltaX: number; deltaY: number; deltaMode: number },
  now: number,
): { state: SpinCapState; steps: number } {
  const dominant = Math.abs(ev.deltaY) >= Math.abs(ev.deltaX) ? ev.deltaY : ev.deltaX;
  const sign = Math.sign(dominant);
  let windowStart = state.windowStart;
  let used = state.windowSteps;
  if (windowStart === 0 || now - windowStart > SPIN_FLICK_MS) {
    windowStart = now;
    used = 0;
  }
  const acc = spinAccumulate(state.spin, ev, now);
  const line = ev.deltaMode === 1 || ev.deltaMode === 2;
  let raw = 0;
  if (sign !== 0) {
    if (line) raw = 1;
    else {
      const quotient = SPIN_STEP_PX > 0 ? Math.trunc(Math.abs(dominant) / SPIN_STEP_PX) : 0;
      raw = quotient > 0 ? quotient : Math.abs(acc.steps);
    }
  }
  const room = Math.max(0, SPIN_FLICK_MAX - used);
  const mag = Math.min(raw, room);
  const steps = sign === 0 ? 0 : sign * mag;
  return {
    state: {
      spin: line ? { residual: 0, lastAt: now } : acc.state,
      windowStart,
      windowSteps: used + Math.abs(steps),
    },
    steps,
  };
}
