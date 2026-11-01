<script lang="ts">
  import type { LaunchRow } from "./launcher";

  let {
    rows,
    selected = 0,
    visibleStart = 0,
    onRun,
    onHover,
    onScrollStart,
  }: {
    rows: LaunchRow[];
    selected?: number;
    visibleStart?: number;
    onRun: (index: number) => void;
    onHover: (index: number) => void;
    onScrollStart: (start: number) => void;
  } = $props();

  let scroller = $state<HTMLDivElement | null>(null);

  $effect(() => {
    const top = visibleStart * 30;
    const el = scroller;
    if (!el) return;
    if (Math.abs(el.scrollTop - top) > 1) el.scrollTop = top;
  });

  function onScroll() {
    const el = scroller;
    if (!el) return;
    const start = Math.max(0, Math.round(el.scrollTop / 30));
    if (start !== visibleStart) onScrollStart(start);
  }
</script>

<div
  class="launcher"
  id="companion-launcher"
  role="listbox"
  aria-label="Commands"
  bind:this={scroller}
  onscroll={onScroll}
>
  {#each rows as row, i (row.id)}
    <div
      id={"companion-launcher-opt-" + i}
      class="launcher-row"
      class:on={i === selected}
      role="option"
      tabindex="-1"
      aria-selected={i === selected}
      onmousedown={(e) => {
        e.preventDefault();
        e.stopPropagation();
      }}
      onclick={() => onRun(i)}
      onkeydown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onRun(i);
        }
      }}
      onmouseenter={() => onHover(i)}
    >
      <span class="launcher-glyph" aria-hidden="true">{row.glyph}</span>
      {#if row.dot}
        <span class="launcher-dot" aria-hidden="true" style:background={row.dot}></span>
      {/if}
      <span class="launcher-title">{row.title}</span>
      {#if row.subtitle}
        <span class="launcher-sub">{row.subtitle}</span>
      {/if}
    </div>
  {/each}
</div>

<style>
  .launcher {
    height: 150px;
    overflow-x: hidden;
    overflow-y: auto;
    flex: none;
    margin: 0 8px 8px;
    border-top: 1px solid var(--edge);
    box-sizing: border-box;
  }

  .launcher-row {
    height: 30px;
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 4px;
    padding: 0 8px;
    border-radius: 6px;
    box-sizing: border-box;
    color: var(--ink);
    font-size: 13px;
    font-family: "SF Pro Text", "SF Pro Display", system-ui, sans-serif;
    cursor: pointer;
  }

  .launcher-row.on,
  .launcher-row[aria-selected="true"] {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    color: var(--ink);
    box-shadow: var(--focus-ring);
  }

  .launcher-glyph {
    flex: none;
    width: 16px;
    text-align: center;
    color: var(--dim);
    font-size: 13px;
  }

  .launcher-dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 999px;
  }

  .launcher-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink);
  }

  .launcher-sub {
    flex: none;
    margin-left: auto;
    color: var(--dim);
    font-size: 12px;
  }
</style>
