import { untrack } from "svelte";
import "./menuKeys.css";
import { classifyKey, surfaceGroups, tabMove, type RingDir } from "./ringKeys";

export type { RingDir };

// One focus ring for the hosted well and the overlay windows. Arrows
// move, Enter opens, Esc backs, Shift+Enter grabs a Todo. Tab / Shift+Tab
// walk surfaces when the current surface has two or more groups. One
// live ring per webview; last attach() wins.

export interface RingStop {
  /** Stable id — also the element's data-mkey, for scroll-into-view. */
  id: string;
  /** Surface id for Tab. Stops without one are not a Tab stop. */
  group?: string;
  /** A button: open / select. */
  confirm?: () => void;
  /**
   * X (Shift+Enter): grab this stop — KANBAN-GRAB's held Todo. A stop
   * with a grab handler eats the chord; a field keeps its newline.
   */
  grab?: () => void;
  /**
   * Arrow override: return true when the stop ate the key (value bump,
   * column hop, parked). Omit / return false for the default ring walk.
   */
  arrow?: (dir: RingDir) => boolean | void;
}

let live: MenuRing | null = null;

export class MenuRing {
  /** The highlighted stop — pages paint class:mkey-sel off this. */
  activeId = $state<string | null>(null);
  /**
   * Optional. When an arrow is not eaten by the stop and there is no
   * further stop in that direction, call this instead of wrapping.
   * Return true to eat the key. Absent: the ring wraps, as before.
   */
  onEdge: ((dir: RingDir) => boolean) | null = null;
  private stops: RingStop[] = [];
  private onEscape: (() => boolean) | null = null;
  // The surface stack: each entry suspends the surface below (its stops,
  // highlight, and Esc) while a nested surface owns the top.
  private stack: {
    stops: RingStop[];
    activeId: string | null;
    onEscape: (() => boolean) | null;
  }[] = [];

  /** Pages rebuild stops off state. The highlight survives by id; when
   *  its row left the menu (an answered Needed, a claimed window) it
   *  lands on whatever took its index. Never auto-highlights a fresh
   *  menu — the first arrow picks an end, console-style. */
  setStops(stops: RingStop[]) {
    const prevId = untrack(() => this.activeId);
    const prevIdx = prevId
      ? untrack(() => this.stops.findIndex((s) => s.id === prevId))
      : -1;
    this.stops = stops;
    if (!prevId) return;
    if (stops.some((s) => s.id === prevId)) return;
    const next = stops[Math.min(Math.max(prevIdx, 0), stops.length - 1)];
    this.setActive(next?.id ?? null);
  }

  /** Jump the highlight — column / tab hops from arrow overrides. */
  select(id: string) {
    if (this.stops.some((s) => s.id === id)) this.setActive(id);
  }

  /** Descend one surface (pad A into a nested menu). Suspends the
   *  current stops/highlight/Esc; the page setStops() the fresh top,
   *  whose onEscape is its B (back one level). The fresh top starts
   *  with no highlight — the first arrow picks an end, console-style. */
  pushSurface(onEscape: () => boolean) {
    this.stack.push({
      stops: this.stops,
      activeId: untrack(() => this.activeId),
      onEscape: this.onEscape,
    });
    this.stops = [];
    this.onEscape = onEscape;
    this.setActive(null);
  }

  /** Back one surface (pad B): restore the suspended stops, highlight,
   *  and Esc. The root never pops — the well's own Esc lives below the
   *  ring (CompanionPage's bubble handler). */
  popSurface() {
    const prev = this.stack.pop();
    if (!prev) return;
    this.stops = prev.stops;
    this.onEscape = prev.onEscape;
    this.setActive(prev.activeId);
  }

  /** Capture-phase keydown on the window; returns the detach. onEscape
   *  closes the page's innermost open menu: true eats B, false lets the
   *  bubble handlers close the cheatsheet / overlay / well. */
  attach(onEscape?: () => boolean): () => void {
    this.onEscape = onEscape ?? null;
    const h = (e: KeyboardEvent) => {
      if (live !== this) return;
      if (this.handleKey(e)) {
        e.preventDefault();
        e.stopImmediatePropagation();
      }
    };
    window.addEventListener("keydown", h, true);
    live = this;
    return () => {
      window.removeEventListener("keydown", h, true);
      if (live === this) live = null;
    };
  }

  private current(): RingStop | undefined {
    return this.stops.find((s) => s.id === this.activeId);
  }

  private setActive(id: string | null) {
    this.activeId = id;
    if (!id) return;
    queueMicrotask(() => {
      document
        .querySelector(`[data-mkey="${CSS.escape(id)}"]`)
        ?.scrollIntoView({ block: "nearest" });
    });
  }

  private move(delta: number) {
    const n = this.stops.length;
    if (n === 0) return;
    const i = this.stops.findIndex((s) => s.id === this.activeId);
    const next = i < 0 ? (delta > 0 ? 0 : n - 1) : (i + delta + n) % n;
    this.setActive(this.stops[next].id);
  }

  private onTab(dir: 1 | -1): boolean {
    const move = tabMove(surfaceGroups(this.stops), this.activeId, dir);
    if (!move.eat) return false;
    if (move.id) this.select(move.id);
    return true;
  }

  private handleKey(e: KeyboardEvent): boolean {
    // A field owns its caret — arrows type, Enter is its own, Esc blurs
    // (the blur half is CompanionPage's bubble Esc; never eaten here).
    const t = e.target as HTMLElement | null;
    const inField = !!t?.closest?.("input, textarea, select");
    const verb = classifyKey({
      key: e.key,
      shiftKey: e.shiftKey,
      metaKey: e.metaKey,
      ctrlKey: e.ctrlKey,
      altKey: e.altKey,
      inField,
    });
    if (verb === null) return false;
    // KANBAN-GRAB: Shift+Enter is the X verb. A stop with no grab
    // handler does not eat the key. Tab never takes this branch.
    if (verb === "grab") {
      const g = this.current()?.grab;
      if (!g) return false;
      g();
      return true;
    }
    if (verb === "tab-next" || verb === "tab-prev") {
      return this.onTab(verb === "tab-next" ? 1 : -1);
    }
    if (verb === "back") return this.onEscape?.() ?? false;
    if (typeof verb === "object") {
      const dir = verb.arrow;
      if (this.stops.length === 0) return false;
      if (this.current()?.arrow?.(dir)) return true;
      const i = this.stops.findIndex((s) => s.id === this.activeId);
      const delta = dir === "left" || dir === "up" ? -1 : 1;
      const atEnd =
        i >= 0 &&
        ((delta < 0 && i === 0) || (delta > 0 && i === this.stops.length - 1));
      if (atEnd && this.onEdge?.(dir)) return true;
      this.move(delta);
      return true;
    }
    if (verb === "confirm") {
      const cur = this.current();
      if (!cur?.confirm) return false;
      cur.confirm();
      return true;
    }
    return false;
  }
}
