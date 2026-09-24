<script lang="ts">
  import { onDestroy } from "svelte";
  import SkyField from "../SkyField.svelte";
  import LedgerStack from "./LedgerStack.svelte";
  import type { LedgerPage, Project } from "../persist";

  let {
    pages,
    projects,
    loaded = false,
    dayKey,
    onReflect,
  }: {
    pages: LedgerPage[];
    projects: Project[];
    loaded?: boolean;
    dayKey: string;
    onReflect: () => void;
  } = $props();

  let face = $state<"cloud" | "journal">("cloud");
  let userPicked = $state(false);
  let live = $state(false);
  let ready = false;
  let prev = 0;
  let alive = true;
  onDestroy(() => {
    alive = false;
  });

  $effect(() => {
    const n = pages.length;
    if (!loaded) return;
    if (!ready) {
      ready = true;
      prev = n;
      if (!userPicked && n === 0) face = "journal";
      queueMicrotask(() => {
        if (alive) live = true;
      });
      return;
    }
    if (n > prev) face = "journal";
    prev = n;
  });
</script>

<section
  class="folio"
  class:journal={face === "journal"}
  class:live
  aria-label="Almanac"
>
  <div class="tabs">
    <div class="tablist" role="tablist">
      <button
        type="button"
        role="tab"
        aria-selected={face === "cloud"}
        class:on={face === "cloud"}
        onclick={() => {
          userPicked = true;
          face = "cloud";
        }}
      >Cloud</button>
      <button
        type="button"
        role="tab"
        aria-selected={face === "journal"}
        class:on={face === "journal"}
        onclick={() => {
          userPicked = true;
          face = "journal";
        }}
      >Journal</button>
    </div>
    {#if face === "journal" && pages.length > 0}
      <button type="button" class="reflect" aria-label="Reflect" onclick={onReflect}>Reflect</button>
    {/if}
    {#if pages.length}
      <span class="count" class:beside={face === "journal"}>{pages.length}</span>
    {/if}
  </div>
  <div class="face">
    <div
      class="cover orbit-field"
      class:off={face === "journal"}
      aria-hidden={face === "journal"}
      inert={face === "journal"}
    >
      <SkyField />
    </div>
    <div
      class="pages"
      class:on={face === "journal"}
      role="tabpanel"
      aria-hidden={face !== "journal"}
      inert={face !== "journal"}
    >
      <LedgerStack {pages} {projects} {dayKey} {onReflect} />
    </div>
  </div>
</section>

<style>
  .folio {
    flex: 1 1 0;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--depth-1-line, var(--edge));
    border-radius: 4px 14px 14px 4px;
    background: transparent;
    box-shadow: var(--depth-2-shadow, 0 12px 28px rgba(0, 0, 0, 0.45));
    overflow: hidden;
    transition: transform 180ms ease, box-shadow 180ms ease;
  }

  .folio:hover {
    transform: translateY(-2px);
    box-shadow: var(--depth-2-shadow-hover, 0 18px 36px rgba(0, 0, 0, 0.55));
  }

  .folio.journal {
    --ink: #1b1730;
    --dim: #5c5674;
    --edge: rgba(27, 23, 48, 0.16);
    --bg: var(--paper, #e7e3f4);
    --bg-fill: rgba(255, 255, 255, 0.45);
    --panel-fill: transparent;
    background: var(--paper, #e7e3f4);
    border-color: rgba(27, 23, 48, 0.18);
    color: var(--ink);
    color-scheme: light;
  }

  .tabs {
    position: relative;
    z-index: 3;
    flex: none;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--depth-1-line, var(--edge));
    background: #0b1020;
  }

  .folio.journal .tabs {
    background: transparent;
    border-bottom-color: rgba(27, 23, 48, 0.12);
  }

  .tablist {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .tabs button {
    font: inherit;
    font-size: 0.68rem;
    font-weight: 650;
    color: var(--dim);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 999px;
    padding: 2px 8px;
    min-height: 24px;
    cursor: pointer;
  }

  .tabs button.on {
    color: var(--ink);
    border-color: var(--depth-1-line, var(--edge));
    background: var(--depth-1-fill, rgba(232, 236, 248, 0.1));
    box-shadow: var(--depth-1-shadow, none);
  }

  .folio.journal .tabs button.on {
    background: rgba(27, 23, 48, 0.08);
    border-color: rgba(27, 23, 48, 0.2);
    box-shadow: none;
  }

  .tabs button.reflect {
    margin-left: auto;
    font-size: 0.72rem;
    font-weight: 700;
    color: #f4f1fb;
    background: #1b1730;
    border-color: transparent;
    border-radius: 8px;
    padding: 4px 12px;
  }

  .count {
    margin-left: auto;
    font-size: 0.64rem;
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }

  .count.beside {
    margin-left: 8px;
  }

  .face {
    position: relative;
    flex: 1;
    min-height: 0;
  }

  .cover,
  .pages {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
    transition: opacity 280ms ease, transform 280ms ease;
  }

  .cover {
    --ink: #e8ecf8;
    --dim: #b7c0d8;
    --edge: rgba(232, 236, 248, 0.2);
    color: #e8ecf8;
    opacity: 1;
    transform: none;
  }

  .cover::before,
  .cover::after {
    content: "";
    position: absolute;
    pointer-events: none;
    z-index: 2;
  }

  .cover::before {
    left: 0;
    top: 0;
    bottom: 0;
    width: 16px;
    background: linear-gradient(90deg, rgba(0, 0, 0, 0.55), rgba(0, 0, 0, 0.05) 72%, transparent);
  }

  .cover::after {
    inset: 0;
    background:
      linear-gradient(90deg, transparent 76%, rgba(0, 0, 0, 0.32)),
      linear-gradient(180deg, rgba(255, 255, 255, 0.07), transparent 16%, transparent 84%, rgba(0, 0, 0, 0.28));
  }

  .cover.off {
    opacity: 0;
    transform: translateX(-8px);
    pointer-events: none;
  }

  .cover :global(.sky-field) {
    flex: 1;
    min-height: 0;
  }

  .cover :global(.field) {
    border-radius: 0;
  }

  .pages {
    opacity: 0;
    transform: translateX(10px);
    pointer-events: none;
    padding: 8px 12px 12px 22px;
    overflow: auto;
    background: transparent;
    box-shadow: inset 16px 0 18px rgba(40, 28, 72, 0.14);
  }

  .pages.on {
    opacity: 1;
    transform: none;
    pointer-events: auto;
  }

  .pages :global(.ledger) {
    position: relative;
    z-index: 0;
    flex: 1;
    min-height: 100%;
  }

  .folio:not(.live) .cover,
  .folio:not(.live) .pages {
    transition: none;
  }

  @media (prefers-reduced-motion: reduce) {
    .folio,
    .cover,
    .cover.off,
    .pages,
    .pages.on {
      transition: opacity 120ms linear;
      transform: none;
    }

    .folio:hover {
      transform: none;
    }
  }
</style>
