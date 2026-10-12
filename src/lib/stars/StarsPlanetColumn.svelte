<script lang="ts">
  import type { Project } from "../persist";
  import { beforeIdAt, columnArrow } from "../board/planetColumn";
  import PlanetChip from "./PlanetChip.svelte";

  let {
    projects,
    scope,
    dots = false,
    onScope,
    onReorder,
    onReorderCommit,
    active = false,
  }: {
    projects: Project[];
    scope: string;
    dots?: boolean;
    onScope: (id: string) => void;
    onReorder: (fromId: string, beforeId: string | null) => void;
    onReorderCommit: () => void;
    active?: boolean;
  } = $props();

  let listEl: HTMLDivElement | null = $state(null);
  let focusId = $state("overall");
  let dragId = $state<string | null>(null);

  const ids = $derived(["overall", ...projects.map((p) => p.id)]);

  $effect(() => {
    if (!ids.includes(focusId)) focusId = scope === "overall" || ids.includes(scope) ? scope : "overall";
  });

  function move(key: string) {
    if (key === "Home") {
      focusId = ids[0] ?? "overall";
      return;
    }
    if (key === "End") {
      focusId = ids[ids.length - 1] ?? "overall";
      return;
    }
    if (key !== "ArrowUp" && key !== "ArrowDown") return;
    const next = columnArrow(ids, focusId, key === "ArrowDown" ? "down" : "up", {
      firstCardId: null,
      firstNeededId: null,
    });
    if (next) focusId = next;
  }

  function onKey(e: KeyboardEvent) {
    if (e.target !== e.currentTarget) return;
    if (e.key === "ArrowUp" || e.key === "ArrowDown" || e.key === "Home" || e.key === "End") {
      e.preventDefault();
      move(e.key);
      return;
    }
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onScope(focusId);
    }
  }

  function beforeAt(clientY: number, movingId: string): string | null {
    const root = listEl;
    if (!root) return null;
    const rows: { id: string; top: number; height: number }[] = [];
    for (const el of root.querySelectorAll("[data-pid]")) {
      const pid = el.getAttribute("data-pid");
      if (!pid || pid === movingId) continue;
      const r = el.getBoundingClientRect();
      rows.push({ id: pid, top: r.top, height: r.height });
    }
    return beforeIdAt(rows, clientY);
  }

  function onPlanetPointerDown(e: PointerEvent, id: string) {
    if (e.button !== 0) return;
    e.stopPropagation();
    const y0 = e.clientY;
    const x0 = e.clientX;
    let dragging = false;
    let done = false;
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
      cleanup();
    };
    const onMove = (ev: PointerEvent) => {
      const dx = ev.clientX - x0;
      const dy = ev.clientY - y0;
      if (!dragging) {
        if (Math.hypot(dx, dy) < 8) return;
        if (Math.abs(dx) > Math.abs(dy)) {
          finish();
          return;
        }
        dragging = true;
        dragId = id;
      }
      ev.preventDefault();
      onReorder(id, beforeAt(ev.clientY, id));
    };
    const onUp = () => finish();
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }
</script>

<div
  class="planet-col"
  class:dots
  class:col-on={active}
  role="listbox"
  aria-label="Planets"
  aria-activedescendant={focusId === "overall" ? "stars-planet-all" : `stars-planet-${focusId}`}
  tabindex="0"
  data-stars-col="planets"
  onkeydown={onKey}
>
  <div class="rows" bind:this={listEl}>
    <button
      type="button"
      id="stars-planet-all"
      class="row"
      class:on={scope === "overall"}
      role="option"
      aria-selected={scope === "overall"}
      data-stars-row="0"
      data-focus-zone="Z1"
      data-focus-item
      tabindex="-1"
      title="All planets"
      onclick={() => {
        focusId = "overall";
        onScope("overall");
      }}
    >
      <span class="glyph" aria-hidden="true">✦</span>
      {#if !dots}<span class="name">All planets</span>{/if}
    </button>
    {#each projects as p, i (p.id)}
      <button
        type="button"
        id="stars-planet-{p.id}"
        class="row"
        class:on={scope === p.id}
        class:dragging={dragId === p.id}
        role="option"
        aria-selected={scope === p.id}
        data-pid={p.id}
        data-stars-row={i + 1}
        data-focus-zone="Z1"
        data-focus-item
        tabindex="-1"
        title={p.name}
        aria-label={p.name}
        onpointerdown={(e) => onPlanetPointerDown(e, p.id)}
        onclick={() => {
          focusId = p.id;
          onScope(p.id);
        }}
      >
        <PlanetChip id={p.id} size={10} label={p.name} />
        {#if !dots}<span class="name">{p.name}</span>{/if}
      </button>
    {/each}
  </div>
</div>

<style>
  .planet-col {
    --stars-ring: var(--focus-ring);
    min-height: 0;
    overflow: auto;
    padding: 4px;
    border-right: 1px solid var(--glass-edge, rgba(0, 0, 0, 0.40));
  }

  .planet-col:focus-visible {
    outline: none;
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 28px;
    padding: 4px 8px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--ink);
    font: inherit;
    font-size: 0.78rem;
    text-align: left;
    cursor: pointer;
  }

  .row.on {
    background: var(--panel-fill);
  }

  .row.dragging {
    opacity: 0.55;
  }

  .glyph {
    width: 10px;
    text-align: center;
    color: var(--dim);
    flex: none;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dots .row {
    justify-content: center;
    padding: 6px 0;
  }
</style>
