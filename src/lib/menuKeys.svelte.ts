import { untrack } from "svelte";
import "./menuKeys.css";

// GO-ORBITTY-MENU-KEYS — the ONE focus ring for the hosted well.
// Pad grammar lock (ORBITTY_PAD_GRAMMAR_LOCK_v0) mimicked on the
// keyboard: arrows = D-pad (move the highlight), Enter = A (open /
// select), Esc = B (close the innermost menu, then the well — the well
// half lives in CompanionPage's bubble Esc, which this capture handler
// never eats unless the page claimed it first). Menus only: Launchpad
// Needed/todo pile, STARS spokes/switcher, Planets pop. Never habitat
// walkers. KANBAN-GRAB rides this same ring: Shift+Enter = X (grab the
// focused Todo) is the ONE chord the ring owns — the handleKey carve-out
// below. GO-ORBITTY-PAD points a real gamepad at these verbs.
//
// Hold-repeat is the OS key repeat: every keydown is handled, e.repeat
// included, so a held arrow walks at the native delay/rate.
//
// One live ring at a time: the TILE-YAW dest swap mounts the outgoing
// and incoming faces together for ~400ms — the last attach() wins and
// the outgoing instance's handler goes inert (its detach never clears
// a successor).

export type RingDir = "left" | "right" | "up" | "down";

export interface RingStop {
  /** Stable id — also the element's data-mkey, for scroll-into-view. */
  id: string;
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
  private stops: RingStop[] = [];
  private onEscape: (() => boolean) | null = null;

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

  private handleKey(e: KeyboardEvent): boolean {
    // A field owns its caret — arrows type, Enter is its own, Esc blurs
    // (the blur half is CompanionPage's bubble Esc; never eaten here).
    const t = e.target as HTMLElement | null;
    const inField = !!t?.closest?.("input, textarea, select");
    // KANBAN-GRAB: Shift+Enter is the X verb — the one chord the ring
    // owns. A field keeps its newline (the composer binds Shift+Enter).
    if (
      e.key === "Enter" &&
      e.shiftKey &&
      !e.metaKey &&
      !e.ctrlKey &&
      !e.altKey
    ) {
      if (inField) return false;
      const g = this.current()?.grab;
      if (!g) return false;
      g();
      return true;
    }
    // Chords are not the ring: ⌘⌥←/→ yaw dests, ⌘⌥1/2/3 toggle them.
    // Bare keys only.
    if (e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return false;
    if (inField) return false;

    if (e.key === "Escape") return this.onEscape?.() ?? false;

    const dir = e.key.startsWith("Arrow")
      ? (e.key.slice(5).toLowerCase() as RingDir)
      : null;
    if (dir) {
      if (this.stops.length === 0) return false;
      if (this.current()?.arrow?.(dir)) return true;
      this.move(dir === "left" || dir === "up" ? -1 : 1);
      return true;
    }
    if (e.key === "Enter") {
      const cur = this.current();
      if (!cur?.confirm) return false;
      cur.confirm();
      return true;
    }
    return false;
  }
}
