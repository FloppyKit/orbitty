<script lang="ts">
  import type { Project } from "../persist";
  import { planetCss } from "../planetColor";

  let {
    projects,
    scope,
    onScope,
    onReflect,
    onReorder,
    onReorderCommit,
  }: {
    projects: Project[];
    scope: string;
    onScope: (id: string) => void;
    onReflect: () => void;
    onReorder: (fromId: string, beforeId: string | null) => void;
    onReorderCommit: () => void;
  } = $props();

  let stripEl: HTMLDivElement | null = $state(null);
  let dragId = $state<string | null>(null);
  let dropId = $state<string | null>(null);
  let didDrag = false;

  function onChipClick(id: string) {
    if (didDrag) {
      didDrag = false;
      return;
    }
    onScope(id);
  }

  function beforeAt(clientX: number): string | null {
    const root = stripEl;
    if (!root) return null;
    const els = root.querySelectorAll(".chip[data-pid]");
    for (const el of els) {
      const r = el.getBoundingClientRect();
      if (clientX < r.left + r.width / 2) return el.getAttribute("data-pid");
    }
    return null;
  }

  function onPlanetPointerDown(e: PointerEvent, id: string) {
    if (e.button !== 0) return;
    if (!(e.currentTarget instanceof HTMLElement)) return;
    e.stopPropagation();
    const tab = e.currentTarget;
    const x0 = e.clientX;
    const y0 = e.clientY;
    const scroll0 = stripEl?.scrollLeft ?? 0;
    let dragging = false;
    let armed = true;
    const SLOP = 16;
    const onMove = (ev: PointerEvent) => {
      if (!armed) return;
      const dx = ev.clientX - x0;
      const dy = ev.clientY - y0;
      if (!dragging) {
        if (Math.hypot(dx, dy) < SLOP) return;
        if (Math.abs(dy) > Math.abs(dx)) {
          armed = false;
          cleanup();
          return;
        }
        if (stripEl && Math.abs(stripEl.scrollLeft - scroll0) > 2) {
          armed = false;
          cleanup();
          return;
        }
        dragging = true;
        didDrag = true;
        dragId = id;
      }
      const before = beforeAt(ev.clientX);
      dropId = before;
      onReorder(id, before);
    };
    const onUp = () => {
      armed = false;
      if (dragging) onReorderCommit();
      dragId = null;
      dropId = null;
      cleanup();
    };
    const cleanup = () => {
      try {
        tab.releasePointerCapture(e.pointerId);
      } catch {
        /* capture is best-effort */
      }
      tab.removeEventListener("pointermove", onMove);
      tab.removeEventListener("pointerup", onUp);
      tab.removeEventListener("pointercancel", onUp);
    };
    try {
      tab.setPointerCapture(e.pointerId);
    } catch {
      /* capture is best-effort */
    }
    tab.addEventListener("pointermove", onMove);
    tab.addEventListener("pointerup", onUp);
    tab.addEventListener("pointercancel", onUp);
  }
</script>

<div class="nudges" role="group" aria-label="Nudge scope" bind:this={stripEl}>
  <button
    type="button"
    class="chip pin"
    class:on={scope === "overall"}
    aria-pressed={scope === "overall"}
    onclick={() => onScope("overall")}
  >All</button>
  <span class="rule" aria-hidden="true"></span>
  {#each projects as p (p.id)}
    <button
      type="button"
      class="chip"
      class:on={scope === p.id}
      class:dragging={dragId === p.id}
      class:drop-ok={dropId === p.id}
      data-pid={p.id}
      aria-pressed={scope === p.id}
      onpointerdown={(e) => onPlanetPointerDown(e, p.id)}
      onclick={() => onChipClick(p.id)}
    ><span class="dot" style="background: {planetCss(p.id)}"></span>{p.name}</button>
  {/each}
  <button type="button" class="reflect" onclick={onReflect}>Reflect</button>
</div>

<style>
  .nudges {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    overflow-x: auto;
    padding: 4px 10px;
    border-bottom: 1px solid var(--edge);
  }

  .chip {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font: inherit;
    font-size: 0.64rem;
    color: var(--ink);
    background: transparent;
    border: 1px solid var(--edge);
    border-radius: 999px;
    padding: 1px 8px;
    cursor: grab;
    touch-action: none;
  }

  .chip.pin,
  .reflect {
    cursor: pointer;
    touch-action: auto;
  }

  .chip.dragging {
    opacity: 0.45;
  }

  .chip.drop-ok {
    box-shadow: inset 0 0 0 1px var(--accent);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 99px;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.28);
    flex: none;
  }

  .rule {
    flex: none;
    width: 1px;
    height: 14px;
    margin: 0 4px;
    background: var(--edge);
  }

  .chip.on {
    background: var(--accent);
    border-color: var(--focus, #6e4db0);
    color: var(--bg);
  }

  .reflect {
    margin-left: auto;
    flex: none;
    font: inherit;
    font-size: 0.68rem;
    font-weight: 700;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 2px 8px;
    cursor: pointer;
  }
</style>
