<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import type { Snippet } from "svelte";
  import { eventElement, startPaneDrag } from "./dragPane";

  let {
    title,
    mode,
    label,
    dot = null,
    onClose,
    closeLabel = "Hide",
    chips,
  }: {
    title: string;
    mode: "mac" | "custom" | "title";
    label: string;
    dot?: string | null;
    onClose?: () => void;
    closeLabel?: string;
    chips?: Snippet;
  } = $props();

  let rowEl: HTMLElement | null = $state(null);

  function resolvedScheme(): "light" | "dark" {
    const el = rowEl ?? document.documentElement;
    const cs = getComputedStyle(el).colorScheme;
    if (cs.includes("dark") && !cs.includes("light")) return "dark";
    if (cs.includes("light") && !cs.includes("dark")) return "light";
    if (cs.includes("dark")) return "dark";
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  function syncAppearance() {
    const scheme = resolvedScheme();
    invoke("window_appearance", { scheme }).catch(() => {});
  }

  onMount(() => {
    // Title-only dock row must not ask for traffic lights.
    if (mode === "title") return;
    syncAppearance();
    const obs = new MutationObserver(() => syncAppearance());
    obs.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme", "style"],
    });
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onMq = () => syncAppearance();
    mq.addEventListener("change", onMq);
    return () => {
      obs.disconnect();
      mq.removeEventListener("change", onMq);
    };
  });

  function onRowMouseDown(e: MouseEvent) {
    // Docked title uses the companion's own drag, not a second pane drag.
    if (mode === "title") return;
    startPaneDrag(e);
  }

  function onRowDblClick(e: MouseEvent) {
    if (mode !== "mac" || label === "companion") return;
    const t = eventElement(e);
    if (t?.closest("[data-no-drag], .wtr-title, .wtr-dot, button, a, input, select, textarea")) {
      return;
    }
    invoke("window_title_dblclick", { label }).catch(() => {});
  }
</script>

<!-- The row is the window's drag strip. A role would be redundant on header
     and a grab handle is not an ARIA widget. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<header
  class="wtr"
  bind:this={rowEl}
  data-title-row
  data-chrome={mode}
  onmousedown={onRowMouseDown}
  ondblclick={onRowDblClick}
>
  {#if mode === "mac"}
    <span class="wtr-lights" aria-hidden="true"></span>
  {/if}
  {#if dot}
    <span class="wtr-dot" aria-hidden="true" style:background={dot}></span>
  {/if}
  <h1 class="wtr-title">{title}</h1>
  <div class="wtr-chips" data-no-drag>
    {#if chips}
      {@render chips()}
    {/if}
  </div>
  {#if mode === "custom"}
    <button
      class="winx"
      onmousedown={(e) => e.stopPropagation()}
      onclick={() => onClose?.()}
      aria-label={closeLabel}>×</button>
  {/if}
</header>

<style>
  .wtr {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    height: var(--wtr-h, 38px);
    padding: 0 10px 0 0;
    box-sizing: border-box;
    background: var(--bg-fill, var(--bg));
    user-select: none;
    -webkit-user-select: none;
    cursor: default;
    color: var(--ink);
  }

  .wtr[data-chrome="title"] {
    padding-left: 10px;
  }

  .wtr-lights {
    width: var(--wtr-lights-w, 84px);
    flex: none;
    height: 100%;
  }

  .wtr-dot {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    flex: none;
    background: transparent;
  }

  .wtr-title {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--ink);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 0 1 auto;
    max-width: 40%;
  }

  .wtr-chips {
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    display: flex;
    align-items: center;
    scrollbar-width: none;
  }

  .wtr-chips::-webkit-scrollbar {
    display: none;
    height: 0;
    background: transparent;
  }

  .winx {
    margin-left: auto;
  }
</style>
