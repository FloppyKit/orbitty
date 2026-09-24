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
  let menuId = $state<string | null>(null);
  let confirmId = $state<string | null>(null);

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
    <ul class="rows">
      {#each rows as row (row.tr.id)}
        <li class="line" aria-label={row.tr.label}>
          <span class="chip" style="background: {planetCss(row.projectId)}"></span>
          <span class="who">{row.projectName}</span>
          <span class="lbl">{row.tr.label}</span>
          {#if isTimer(row.tr)}
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
          {:else}
            <span class="step">
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
            </span>
          {/if}
          {#if confirmId === row.tr.id}
            <span class="confirm">
              Remove?
              <button type="button" onclick={() => onRemove(row.projectId, row.tr.id)}>Yes</button>
              <button type="button" onclick={() => (confirmId = null)}>No</button>
            </span>
          {:else}
            <button
              type="button"
              class="more"
              aria-label="Tracker actions"
              aria-expanded={menuId === row.tr.id}
              onclick={() => (menuId = menuId === row.tr.id ? null : row.tr.id)}
            >···</button>
            {#if menuId === row.tr.id}
              <button
                type="button"
                class="menu"
                onclick={() => {
                  menuId = null;
                  confirmId = row.tr.id;
                }}
              >Remove</button>
            {/if}
          {/if}
        </li>
      {/each}
    </ul>
  {:else if !adding}
    <p class="empty">No trackers</p>
  {/if}
</section>

<style>
  .trackers {
    flex: 1;
    min-height: 0;
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
  .more,
  .menu,
  .confirm button {
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

  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 2px 6px;
    border-radius: 8px;
    background: var(--bg-fill, var(--bg));
  }

  .chip {
    width: 8px;
    height: 8px;
    border-radius: 99px;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.28);
    flex: none;
  }

  .who {
    flex: none;
    max-width: 5.5rem;
    font-size: 0.62rem;
    color: var(--dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lbl {
    flex: 1;
    min-width: 0;
    font-size: 0.72rem;
    font-weight: 650;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .step {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    flex: none;
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

  .more,
  .menu {
    flex: none;
    font-size: 0.72rem;
    padding: 0 4px;
    color: var(--dim);
    background: transparent;
    border-color: transparent;
  }

  .line:hover .more,
  .more[aria-expanded="true"] {
    color: var(--ink);
  }

  .confirm {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 0.62rem;
    color: var(--dim);
  }

  .confirm button {
    font-size: 0.62rem;
    padding: 0 4px;
  }

  .empty {
    margin: 0;
    font-size: 0.66rem;
    color: var(--dim);
  }
</style>
