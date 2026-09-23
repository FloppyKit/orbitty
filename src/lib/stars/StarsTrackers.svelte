<script lang="ts">
  import type { Project, Tracker } from "../persist";
  import { planetCss } from "../planetColor";
  import { elapsedSec, formatElapsed, isTimer } from "./timer";

  type Row = { projectId: string; projectName: string; tr: Tracker };

  let {
    projects,
    scope,
    onAdd,
    onBump,
    onSetValue,
    onRemove,
    onPlay,
    onStop,
  }: {
    projects: Project[];
    scope: string;
    onAdd: (projectId: string, label: string, kind: "number" | "timer") => void;
    onBump: (projectId: string, trackerId: string, delta: number) => void;
    onSetValue: (projectId: string, trackerId: string, raw: string) => void;
    onRemove: (projectId: string, trackerId: string) => void;
    onPlay: (projectId: string, trackerId: string) => void;
    onStop: (projectId: string, trackerId: string) => void;
  } = $props();

  let adding = $state(false);
  let label = $state("");
  let kind = $state<"number" | "timer">("number");
  let planetId = $state("");
  let now = $state(Date.now());

  const rows = $derived.by(() => {
    const list =
      scope === "overall" ? projects : projects.filter((p) => p.id === scope);
    const out: Row[] = [];
    for (const p of list) {
      for (const tr of p.trackers ?? []) {
        out.push({ projectId: p.id, projectName: p.name, tr });
      }
    }
    return out;
  });

  const running = $derived(
    rows.some((r) => isTimer(r.tr) && r.tr.runningSince != null),
  );

  $effect(() => {
    if (!running) return;
    const id = setInterval(() => {
      now = Date.now();
    }, 250);
    return () => clearInterval(id);
  });

  function openAdd() {
    adding = !adding;
    label = "";
    kind = "number";
    planetId =
      scope !== "overall" && projects.some((p) => p.id === scope)
        ? scope
        : (projects[0]?.id ?? "");
  }

  function commitAdd() {
    const name = label.trim().slice(0, 7);
    const id = planetId || projects[0]?.id || "";
    if (!name || !id) return;
    onAdd(id, name, kind);
    label = "";
    adding = false;
  }
</script>

<section class="trackers" aria-label="Trackers">
  <header>
    <span>Trackers</span>
    <button
      type="button"
      class="plus"
      onclick={openAdd}
      aria-label={adding ? "Cancel add tracker" : "Add a tracker"}
    >{adding ? "×" : "+"}</button>
  </header>
  {#if adding}
    <div class="add">
      {#if scope === "overall"}
        <select bind:value={planetId} aria-label="Planet for this tracker">
          {#each projects as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>
      {/if}
      <input
        bind:value={label}
        maxlength="7"
        placeholder="Name (7)"
        aria-label="New tracker name"
        onkeydown={(e) => {
          if (e.key === "Enter") commitAdd();
          if (e.key === "Escape") adding = false;
        }}
      />
      <select bind:value={kind} aria-label="Tracker kind">
        <option value="number">Number</option>
        <option value="timer">Timer</option>
      </select>
      <button type="button" class="go" onclick={commitAdd}>Add</button>
    </div>
  {/if}
  {#if rows.length}
    <div class="grid">
      {#each rows as row (row.tr.id)}
        <div class="tile" role="group" aria-label={row.tr.label}>
          <button
            type="button"
            class="x"
            aria-label="Remove {row.tr.label}"
            onclick={() => onRemove(row.projectId, row.tr.id)}>×</button>
          <div class="name">
            <span
              class="chip"
              style="background: {planetCss(row.projectId)}"
            ></span>
            <span class="planet">{row.projectName}</span>
            <span class="lbl">{row.tr.label}</span>
          </div>
          {#if isTimer(row.tr)}
            <div class="row">
              <button
                type="button"
                class="play"
                aria-label={row.tr.runningSince != null ? "Stop timer" : "Start timer"}
                onclick={() =>
                  row.tr.runningSince != null
                    ? onStop(row.projectId, row.tr.id)
                    : onPlay(row.projectId, row.tr.id)}
              >{row.tr.runningSince != null ? "Stop" : "Play"}</button>
              <span class="elapsed" title="{Math.round(elapsedSec(row.tr, now))} seconds"
                >{formatElapsed(elapsedSec(row.tr, now))}</span>
            </div>
          {:else}
            <div class="row">
              <button
                type="button"
                class="nudge"
                aria-label="Decrease {row.tr.label}"
                onclick={() => onBump(row.projectId, row.tr.id, -1)}>−</button>
              <input
                class="val"
                value={row.tr.value}
                inputmode="numeric"
                aria-label={row.tr.label}
                onmousedown={(e) => e.stopPropagation()}
                onblur={(e) => onSetValue(row.projectId, row.tr.id, e.currentTarget.value)}
                onkeydown={(e) => {
                  if (e.key === "Enter") e.currentTarget.blur();
                  e.stopPropagation();
                }}
              />
              <button
                type="button"
                class="nudge"
                aria-label="Increase {row.tr.label}"
                onclick={() => onBump(row.projectId, row.tr.id, 1)}>+</button>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {:else if !adding}
    <p class="empty">No trackers</p>
  {/if}
</section>

<style>
  .trackers {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  header {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.72rem;
    font-weight: 700;
  }

  .plus,
  .go,
  .play,
  .nudge,
  .x {
    font: inherit;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    cursor: pointer;
  }

  .plus {
    margin-left: auto;
    width: 1.2rem;
    height: 1.2rem;
    padding: 0;
  }

  .add {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .add input,
  .add select,
  .val {
    font: inherit;
    font-size: 0.7rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    padding: 3px 6px;
  }

  .add input {
    flex: 1;
    min-width: 5rem;
  }

  .go {
    font-size: 0.68rem;
    padding: 2px 8px;
  }

  .grid {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 7.2rem;
    padding: 6px 6px 4px;
    border: 1px solid var(--edge);
    border-radius: 8px;
    background: var(--bg-fill, var(--bg));
  }

  .x {
    position: absolute;
    top: -6px;
    right: -6px;
    width: 16px;
    height: 16px;
    padding: 0;
    font-size: 0.68rem;
    line-height: 1;
    color: #fff;
    background: #ff453a;
    border: none;
  }

  .name {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 3px;
    max-width: 9rem;
  }

  .chip {
    width: 8px;
    height: 8px;
    border-radius: 99px;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.28);
    flex: none;
  }

  .planet {
    font-size: 0.58rem;
    color: var(--dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 6.5rem;
  }

  .lbl {
    flex-basis: 100%;
    font-size: 0.68rem;
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }

  .nudge {
    width: 16px;
    height: 18px;
    padding: 0;
    font-size: 0.75rem;
  }

  .val {
    width: 2.4rem;
    text-align: center;
    font-weight: 700;
    padding: 1px 2px;
  }

  .play {
    font-size: 0.62rem;
    padding: 1px 6px;
  }

  .elapsed {
    font-size: 0.72rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .empty {
    margin: 0;
    font-size: 0.66rem;
    color: var(--dim);
  }
</style>
