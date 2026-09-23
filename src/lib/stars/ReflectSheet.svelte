<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { MetricFlags } from "../persist";

  let {
    onClose,
    onWrite,
  }: {
    onClose: () => void;
    onWrite: (flags: MetricFlags) => void;
  } = $props();

  let spokes = $state(true);
  let wins = $state(true);
  let grateful = $state(true);
  let trackers = $state(true);
  let cloud = $state(true);
  let closeBtn: HTMLButtonElement | null = $state(null);

  function flags(): MetricFlags {
    return { spokes, wins, grateful, trackers, cloud };
  }

  function onKey(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.preventDefault();
    e.stopImmediatePropagation();
    onClose();
  }

  onMount(() => {
    closeBtn?.focus();
    window.addEventListener("keydown", onKey, true);
  });
  onDestroy(() => window.removeEventListener("keydown", onKey, true));
</script>

<div
  class="shade"
  role="presentation"
  onclick={onClose}
  onkeydown={(e) => {
    if (e.key === "Escape") onClose();
  }}
>
  <div
    class="sheet"
    role="dialog"
    aria-label="Reflect"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onmousedown={(e) => e.stopPropagation()}
  >
    <header>
      <h2>Reflect</h2>
      <button type="button" class="x" bind:this={closeBtn} onclick={onClose} aria-label="Close">×</button>
    </header>
    <p class="hint">Choose what a later page may include. This does not write a reflection.</p>
    <label><input type="checkbox" bind:checked={spokes} /> Spokes</label>
    <label><input type="checkbox" bind:checked={wins} /> Wins</label>
    <label><input type="checkbox" bind:checked={grateful} /> Grateful</label>
    <label><input type="checkbox" bind:checked={trackers} /> Trackers</label>
    <label><input type="checkbox" bind:checked={cloud} /> Cloud</label>
    <div class="row">
      <button type="button" class="btn" onclick={onClose}>Close</button>
      <button type="button" class="btn strong" onclick={() => onWrite(flags())}>Write empty page</button>
    </div>
  </div>
</div>

<style>
  .shade {
    position: absolute;
    inset: 0;
    z-index: 5;
    background: rgba(0, 0, 0, 0.28);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding: 36px 12px 12px;
  }

  .sheet {
    width: min(280px, 100%);
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px 12px;
    border-radius: 10px;
    border: 1px solid var(--edge);
    background: var(--panel-fill, var(--panel));
    color: var(--ink);
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  h2 {
    flex: 1;
    margin: 0;
    font-size: 0.85rem;
  }

  .x,
  .btn {
    font: inherit;
    font-size: 0.72rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    cursor: pointer;
  }

  .x {
    width: 22px;
    height: 22px;
    padding: 0;
  }

  .hint {
    margin: 0;
    font-size: 0.68rem;
    color: var(--dim);
  }

  label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.75rem;
  }

  .row {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    margin-top: 4px;
  }

  .btn {
    padding: 3px 8px;
  }

  .strong {
    font-weight: 700;
  }
</style>
