<script lang="ts">
  import type { Project } from "../persist";
  import { planetCss } from "../planetColor";

  let {
    projects,
    scope,
    onScope,
    onReorder,
    onReorderCommit,
  }: {
    projects: Project[];
    scope: string;
    onScope: (id: string) => void;
    onReorder: (fromId: string, beforeId: string | null) => void;
    onReorderCommit: () => void;
  } = $props();

  let pillsEl: HTMLDivElement | null = $state(null);
  let dragId = $state<string | null>(null);
  let dropId = $state<string | null>(null);
  let fadeLeft = $state(false);
  let fadeRight = $state(false);
  let cue = $state({ on: false, left: 0, width: 100 });
  let didDrag = false;

  function syncFade() {
    const el = pillsEl;
    if (!el) {
      fadeLeft = false;
      fadeRight = false;
      cue = { on: false, left: 0, width: 100 };
      return;
    }
    const overflow = el.scrollWidth > el.clientWidth + 1;
    fadeLeft = overflow && el.scrollLeft > 2;
    fadeRight = overflow && el.scrollLeft + el.clientWidth < el.scrollWidth - 2;
    if (!overflow) {
      cue = { on: false, left: 0, width: 100 };
      return;
    }
    const width = Math.max(18, (el.clientWidth / el.scrollWidth) * 100);
    const room = Math.max(0, 100 - width);
    const maxScroll = el.scrollWidth - el.clientWidth;
    const left = maxScroll > 0 ? (el.scrollLeft / maxScroll) * room : 0;
    cue = { on: true, left, width };
  }

  $effect(() => {
    const el = pillsEl;
    void projects.length;
    if (!el) return;
    const measure = () => syncFade();
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    el.addEventListener("scroll", measure, { passive: true });
    return () => {
      ro.disconnect();
      el.removeEventListener("scroll", measure);
    };
  });

  function onChipClick(id: string) {
    if (didDrag) {
      didDrag = false;
      return;
    }
    onScope(id);
  }

  // Insertion point among the pills that are not the one being dragged.
  // Skipping the dragged pill keeps a live reorder from chasing its own box.
  function beforeAt(clientX: number, movingId: string): string | null {
    const root = pillsEl;
    if (!root) return null;
    const els = root.querySelectorAll(".chip[data-pid]");
    for (const el of els) {
      const pid = el.getAttribute("data-pid");
      if (!pid || pid === movingId) continue;
      const r = el.getBoundingClientRect();
      if (clientX < r.left + r.width / 2) return pid;
    }
    return null;
  }

  function edgeScroll(clientX: number) {
    const root = pillsEl;
    if (!root) return;
    const r = root.getBoundingClientRect();
    const edge = 28;
    if (clientX < r.left + edge) root.scrollLeft -= 14;
    else if (clientX > r.right - edge) root.scrollLeft += 14;
  }

  function onPlanetPointerDown(e: PointerEvent, id: string) {
    if (e.button !== 0) return;
    e.stopPropagation();
    const x0 = e.clientX;
    const y0 = e.clientY;
    let dragging = false;
    let done = false;
    // pointerup is followed by click, often on a different pill after reorder.
    const swallowClick = (ev: MouseEvent) => {
      if (!didDrag) return;
      ev.preventDefault();
      ev.stopPropagation();
      didDrag = false;
    };
    const cleanup = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
    const finish = () => {
      if (done) return;
      done = true;
      if (dragging) onReorderCommit();
      dragId = null;
      dropId = null;
      cleanup();
      setTimeout(() => {
        window.removeEventListener("click", swallowClick, true);
        didDrag = false;
      }, 0);
    };
    const onMove = (ev: PointerEvent) => {
      const dx = ev.clientX - x0;
      const dy = ev.clientY - y0;
      if (!dragging) {
        if (Math.hypot(dx, dy) < 8) return;
        if (Math.abs(dy) > Math.abs(dx)) {
          finish();
          return;
        }
        dragging = true;
        didDrag = true;
        dragId = id;
      }
      ev.preventDefault();
      edgeScroll(ev.clientX);
      const before = beforeAt(ev.clientX, id);
      dropId = before;
      onReorder(id, before);
    };
    const onUp = () => finish();
    window.addEventListener("click", swallowClick, true);
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }
</script>

<div class="nudges" role="group" aria-label="Nudge scope">
  <div class="shelf">
    <button
      type="button"
      class="chip pin"
      class:on={scope === "overall"}
      aria-pressed={scope === "overall"}
      onclick={() => onScope("overall")}
    >All</button>
  </div>
  <div class="scroller">
    <div
      class="pills"
      class:fade-left={fadeLeft}
      class:fade-right={fadeRight}
      bind:this={pillsEl}
    >
      {#each projects as p (p.id)}
        <button
          type="button"
          class="chip"
          class:on={scope === p.id}
          class:dragging={dragId === p.id}
          class:drop-ok={dropId === p.id && dragId !== p.id}
          data-pid={p.id}
          draggable="false"
          aria-pressed={scope === p.id}
          title="Drag to reorder"
          onpointerdown={(e) => onPlanetPointerDown(e, p.id)}
          onclick={() => onChipClick(p.id)}
        ><span class="dot" style="background: {planetCss(p.id)}"></span>{p.name}</button>
      {/each}
    </div>
    {#if cue.on}
      <div class="cue" aria-hidden="true">
        <span style="left:{cue.left}%;width:{cue.width}%"></span>
      </div>
    {/if}
  </div>
</div>

<style>
  .nudges {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    overflow: hidden;
    padding: 6px 10px 5px;
    border-bottom: 1px solid var(--edge);
    user-select: none;
    -webkit-user-select: none;
  }

  .shelf {
    flex: none;
    display: flex;
    align-items: center;
    align-self: center;
    padding: 4px;
    border-radius: 12px;
    background: rgba(232, 236, 248, 0.08);
    border: 1px solid rgba(232, 236, 248, 0.22);
    box-shadow:
      0 1px 0 rgba(232, 236, 248, 0.12),
      0 8px 16px rgba(0, 0, 0, 0.35);
  }

  .scroller {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  .pills {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    overflow-x: auto;
    overscroll-behavior-x: contain;
    scrollbar-width: none;
  }

  .pills::-webkit-scrollbar {
    display: none;
    width: 0;
    height: 0;
  }

  .cue {
    position: relative;
    height: 3px;
    margin-top: 3px;
    border-radius: 2px;
    background: rgba(232, 236, 248, 0.24);
  }

  .cue span {
    position: absolute;
    top: 0;
    height: 3px;
    border-radius: 2px;
    background: #e8ecf8;
  }

  .pills.fade-left {
    -webkit-mask-image: linear-gradient(90deg, transparent, #000 14px);
    mask-image: linear-gradient(90deg, transparent, #000 14px);
  }

  .pills.fade-right {
    -webkit-mask-image: linear-gradient(90deg, #000 calc(100% - 14px), transparent);
    mask-image: linear-gradient(90deg, #000 calc(100% - 14px), transparent);
  }

  .pills.fade-left.fade-right {
    -webkit-mask-image: linear-gradient(
      90deg,
      transparent,
      #000 14px,
      #000 calc(100% - 14px),
      transparent
    );
    mask-image: linear-gradient(
      90deg,
      transparent,
      #000 14px,
      #000 calc(100% - 14px),
      transparent
    );
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
    border: 1px solid transparent;
    border-radius: 999px;
    padding: 1px 8px;
    min-height: 24px;
    cursor: grab;
    touch-action: none;
    white-space: nowrap;
  }

  .chip:active {
    cursor: grabbing;
  }

  .chip.pin {
    cursor: pointer;
    touch-action: auto;
    font-weight: 750;
    letter-spacing: 0.04em;
    padding: 2px 12px;
    border-color: transparent;
    background: transparent;
  }

  .chip.dragging {
    opacity: 0.45;
  }

  .chip.drop-ok {
    box-shadow: inset 2px 0 0 var(--accent);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 99px;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.28);
    flex: none;
  }

  .chip.on {
    background: var(--accent);
    border-color: transparent;
    color: var(--bg);
    font-weight: 700;
  }
</style>
