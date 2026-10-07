<script lang="ts">
  import type { Tracker } from "../persist";
  import PlanetChip from "./PlanetChip.svelte";
  import { elapsedSec, formatElapsed, isTimer } from "./timer";
  import { trackerSessions, trackerWeek } from "./trackerHistory";

  let {
    projectId,
    projectName,
    tracker,
    today,
    now,
    editing = false,
    draft = "",
    menuOpen = false,
    confirm = false,
    onBump,
    onToggle,
    onEdit,
    onDraft,
    onCommit,
    onCancel,
    onMenu,
    onAskRemove,
    onRemove,
    onKeep,
  }: {
    projectId: string;
    projectName: string;
    tracker: Tracker;
    today: string;
    now: number;
    editing?: boolean;
    draft?: string;
    menuOpen?: boolean;
    confirm?: boolean;
    onBump: (delta: number) => void;
    onToggle: () => void;
    onEdit: () => void;
    onDraft: (value: string) => void;
    onCommit: () => void;
    onCancel: () => void;
    onMenu: () => void;
    onAskRemove: () => void;
    onRemove: () => void;
    onKeep: () => void;
  } = $props();

  const timer = $derived(isTimer(tracker));
  const live = $derived(timer && tracker.runningSince != null);
  const week = $derived(trackerWeek(tracker, today));
  const sessions = $derived(trackerSessions(tracker, today));
  const peak = $derived(Math.max(1, ...week));

  function barH(v: number, empty: boolean): string {
    if (empty) return "2px";
    return `${Math.max(2, Math.round((v / peak) * 40))}px`;
  }
</script>

<article class="card" class:running={live} tabindex="-1" data-focus-zone="Z4" data-focus-item data-focus-card>
  <div class="top">
    <PlanetChip id={projectId} size={10} label={projectName} />
    <span class="lbl">{tracker.label}</span>
    <span class="pname">{projectName}</span>
  </div>

  {#if editing}
    <input
      class="val num-in"
      data-focus-zone="Z4"
      data-focus-ctl
      data-focus-num
      inputmode="numeric"
      aria-label={tracker.label}
      value={draft}
      oninput={(e) => onDraft(e.currentTarget.value)}
      onblur={onCommit}
      onkeydown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          onCommit();
        } else if (e.key === "Escape") {
          e.preventDefault();
          onCancel();
        }
        e.stopPropagation();
      }}
    />
  {:else if timer}
    <button type="button" class="val time" data-focus-zone="Z4" data-focus-ctl data-focus-num onclick={onEdit}>
      {formatElapsed(elapsedSec(tracker, now))}
    </button>
  {:else}
    <button type="button" class="val" data-focus-zone="Z4" data-focus-ctl data-focus-num aria-label="Edit {tracker.label}" onclick={onEdit}>
      {tracker.value}
    </button>
  {/if}

  <div class="bars" aria-hidden="true">
    {#each week as v, i (i)}
      <span class="bar" class:today={i === week.length - 1} class:empty={v <= 0} style="--h: {barH(v, v <= 0)}"></span>
    {/each}
  </div>

  {#if timer}
    <div class="sessions" aria-hidden="true">
      {#if sessions.length === 0}
        <span class="sess empty"></span>
      {:else}
        {#each sessions as span, i (i)}
          <span class="sess" style="--h: {Math.max(2, Math.min(10, Math.round((span[1] - span[0]) / 60000)))}px"></span>
        {/each}
      {/if}
    </div>
  {/if}

  <div class="foot">
    {#if confirm}
      <span class="ask">Remove?</span>
      <button type="button" class="round" data-focus-zone="Z4" data-focus-ctl aria-label="Remove {tracker.label}" onclick={onRemove}>Yes</button>
      <button type="button" class="round" data-focus-zone="Z4" data-focus-ctl aria-label="Keep {tracker.label}" onclick={onKeep}>No</button>
    {:else if timer}
      <button
        type="button"
        class="round"
        class:hum={live}
        data-focus-zone="Z4"
        data-focus-ctl
        aria-label={live ? "Pause timer" : "Start timer"}
        onclick={onToggle}
      >{live ? "Pause" : "Start"}</button>
      <button type="button" class="round" data-focus-zone="Z4" data-focus-ctl aria-label="Tracker actions" aria-expanded={menuOpen} onclick={onMenu}>…</button>
    {:else}
      <button type="button" class="round" data-focus-zone="Z4" data-focus-ctl aria-label="Decrease {tracker.label}" onclick={() => onBump(-1)}>−</button>
      <button type="button" class="round" data-focus-zone="Z4" data-focus-ctl aria-label="Increase {tracker.label}" onclick={() => onBump(1)}>+</button>
      <button type="button" class="round" data-focus-zone="Z4" data-focus-ctl aria-label="Tracker actions" aria-expanded={menuOpen} onclick={onMenu}>…</button>
    {/if}
    {#if menuOpen && !confirm}
      <button type="button" class="round menu" data-focus-zone="Z4" data-focus-ctl onclick={onAskRemove}>Remove</button>
    {/if}
  </div>
</article>

<style>
  .card {
    --stars-ring: var(--focus-ring);
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    padding: 10px;
    border-radius: 10px;
    background: var(--panel-fill);
    border: 1px solid var(--edge);
    color: var(--ink);
  }

  .card:hover {
    background: color-mix(in srgb, var(--ink) 4%, var(--panel-fill));
    transform: translateY(-1px);
  }

  .top {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
  }

  .lbl {
    font-size: 13px;
    font-weight: 600;
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pname {
    font-size: 13px;
    font-weight: 400;
    color: var(--dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .val,
  .num-in {
    font: inherit;
    font-size: 26px;
    font-weight: 600;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
    color: var(--ink);
    background: transparent;
    border: none;
    padding: 0;
    text-align: left;
    align-self: flex-start;
  }

  .val {
    cursor: text;
  }

  .val.time,
  .num-in {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  .bars {
    display: flex;
    align-items: flex-end;
    gap: 3px;
    height: 40px;
  }

  .bar {
    flex: 1;
    height: var(--h);
    min-height: 2px;
    border-radius: 2px;
    background: var(--dim);
    opacity: 0.55;
  }

  .bar.today {
    background: color-mix(in srgb, var(--ink) 70%, transparent);
    opacity: 1;
  }

  .bar.empty {
    height: 2px;
    background: var(--dim);
    opacity: 0.4;
  }

  .sessions {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 10px;
  }

  .sess {
    width: 4px;
    height: var(--h, 4px);
    border-radius: 1px;
    background: color-mix(in srgb, var(--ink) 70%, transparent);
  }

  .sess.empty {
    width: 10px;
    height: 2px;
    background: var(--dim);
    opacity: 0.4;
  }

  .foot {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 4px;
    min-height: 28px;
    opacity: 0;
  }

  .card:hover .foot,
  .card:focus-within .foot {
    opacity: 1;
  }

  .round {
    width: 28px;
    height: 28px;
    min-width: 28px;
    min-height: 28px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: var(--ink);
    font: inherit;
    cursor: pointer;
  }

  .round.hum {
    box-shadow: 0 0 0 1px var(--accent);
    animation: motion-hum 1.6s ease-in-out infinite;
  }

  .ask {
    font-size: 13px;
    color: var(--dim);
  }

  @keyframes motion-hum {
    50% {
      box-shadow: 0 0 0 2px var(--accent);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .round.hum {
      animation: none;
    }

    .card:hover {
      transform: none;
    }
  }

  :global(:root[data-theme="terminal"]) .card {
    background: transparent;
    border-color: transparent;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  :global(:root[data-theme="terminal"]) .card:hover {
    background: transparent;
    transform: none;
  }

  :global(:root[data-theme="terminal"]) .round {
    border-radius: 0;
    width: auto;
    min-width: 28px;
    padding: 0 4px;
  }
</style>
