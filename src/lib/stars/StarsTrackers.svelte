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
    onScope,
    showPlanetMenu = false,
  }: {
    projects: Project[];
    scope: string;
    onAdd: (projectId: string, label: string, kind: "number" | "timer") => void;
    onBump: (projectId: string, trackerId: string, delta: number) => void;
    onSetValue: (projectId: string, trackerId: string, raw: string) => void;
    onRemove: (projectId: string, trackerId: string) => void;
    onPlay: (projectId: string, trackerId: string) => void;
    onStop: (projectId: string, trackerId: string) => void;
    onScope?: (id: string) => void;
    showPlanetMenu?: boolean;
  } = $props();

  let adding = $state(false);
  let label = $state("");
  let kind = $state<"number" | "timer">("number");
  let planetId = $state("");
  let now = $state(Date.now());
  let menuId = $state<string | null>(null);
  let confirmId = $state<string | null>(null);
  let editingId = $state<string | null>(null);
  let draft = $state("");
  let editEl = $state<HTMLInputElement | null>(null);
  let skipCommit = false;

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

  $effect(() => {
    if (!editingId) return;
    editEl?.focus();
    editEl?.select();
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

  function openEdit(id: string, value: number) {
    skipCommit = false;
    menuId = null;
    editingId = id;
    draft = String(value);
  }

  function commitEdit(projectId: string, trackerId: string) {
    if (skipCommit) {
      skipCommit = false;
      return;
    }
    if (editingId !== trackerId) return;
    const raw = draft.trim();
    editingId = null;
    if (!/^-?\d+$/.test(raw)) return;
    onSetValue(projectId, trackerId, raw);
  }

  function cancelEdit() {
    skipCommit = true;
    editingId = null;
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
  {#if showPlanetMenu}
    <select
      class="planet-menu"
      aria-label="Planet filter"
      value={scope}
      onchange={(e) => onScope?.((e.currentTarget as HTMLSelectElement).value)}
    >
      <option value="overall">All planets</option>
      {#each projects as p (p.id)}
        <option value={p.id}>{p.name}</option>
      {/each}
    </select>
  {/if}
  {#if adding}
    <div class="add">
      <select bind:value={planetId} aria-label="Planet for this tracker">
        {#each projects as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      </select>
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
    <ul class="faces">
      {#each rows as row, i (row.tr.id)}
        {@const timer = isTimer(row.tr)}
        {@const live = timer && row.tr.runningSince != null}
        {#if scope === "overall" && (i === 0 || rows[i - 1].projectId !== row.projectId)}
          <li class="group">{row.projectName}</li>
        {/if}
        <li
          class="face row"
          class:timer
          class:running={live}
          tabindex="0"
          aria-label="{row.projectName} {row.tr.label}"
          onkeydown={(e) => {
            if (e.key !== "Enter" || e.target !== e.currentTarget) return;
            e.preventDefault();
            if (timer) {
              if (live) onStop(row.projectId, row.tr.id);
              else onPlay(row.projectId, row.tr.id);
            } else onBump(row.projectId, row.tr.id, 1);
          }}
        >
          <span
            class="chip"
            style="background: {planetCss(row.projectId)}"
            title={row.projectName}
          ></span>
          <span class="lbl">{row.tr.label}</span>
          {#if !timer && editingId === row.tr.id}
            <input
              bind:this={editEl}
              class="num-in"
              inputmode="numeric"
              aria-label={row.tr.label}
              bind:value={draft}
              onblur={() => commitEdit(row.projectId, row.tr.id)}
              onkeydown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  e.currentTarget.blur();
                } else if (e.key === "Escape") {
                  e.preventDefault();
                  cancelEdit();
                }
                e.stopPropagation();
              }}
            />
          {:else if timer}
            <span class="num time" title="{Math.round(elapsedSec(row.tr, now))} seconds"
              >{formatElapsed(elapsedSec(row.tr, now))}</span>
          {:else}
            <button
              type="button"
              class="num"
              aria-label="Edit {row.tr.label}"
              onclick={() => openEdit(row.tr.id, row.tr.value)}
            >{row.tr.value}</button>
          {/if}
          {#if confirmId === row.tr.id}
            <div class="overlay hold">
              <span class="ask">Remove?</span>
              <button type="button" aria-label="Remove {row.tr.label}" onclick={() => onRemove(row.projectId, row.tr.id)}>Yes</button>
              <button type="button" aria-label="Keep {row.tr.label}" onclick={() => (confirmId = null)}>No</button>
            </div>
          {:else if timer}
            <div class="overlay">
              <button
                type="button"
                aria-label={live ? "Stop timer" : "Start timer"}
                onclick={() =>
                  live
                    ? onStop(row.projectId, row.tr.id)
                    : onPlay(row.projectId, row.tr.id)}
              >{live ? "Stop" : "Start"}</button>
            </div>
          {:else}
            <div class="overlay">
              <button
                type="button"
                aria-label="Decrease {row.tr.label}"
                onclick={() => onBump(row.projectId, row.tr.id, -1)}>−</button>
              <button
                type="button"
                aria-label="Increase {row.tr.label}"
                onclick={() => onBump(row.projectId, row.tr.id, 1)}>+</button>
            </div>
          {/if}
          <button
            type="button"
            class="more"
            aria-label="Tracker actions"
            aria-expanded={menuId === row.tr.id}
            onclick={() => (menuId = menuId === row.tr.id ? null : row.tr.id)}
          >…</button>
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
    flex: none;
    font-size: 0.66rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--dim);
  }

  .plus,
  .go,
  .overlay button,
  .menu {
    font: inherit;
    color: var(--ink);
    background: var(--panel-fill);
    border: 1px solid var(--edge);
    border-radius: 8px;
    cursor: pointer;
  }

  .plus {
    margin-left: auto;
    width: 24px;
    height: 24px;
    padding: 0;
    color: var(--ink);
    background: transparent;
  }

  .add {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    flex: none;
  }

  .add input,
  .add select {
    font: inherit;
    font-size: 0.7rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    padding: 3px 6px;
    min-height: 24px;
  }

  .add input {
    flex: 1;
    min-width: 5rem;
  }

  .go {
    font-size: 0.68rem;
    padding: 2px 8px;
    min-height: 24px;
  }

  .faces {
    list-style: none;
    margin: 0;
    padding: 0;
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-content: stretch;
  }

  .group {
    list-style: none;
    font-size: 0.62rem;
    color: var(--dim);
    padding: 6px 4px 0;
  }

  .face {
    position: relative;
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: flex-start;
    gap: 8px;
    min-height: 36px;
    padding: 6px 8px;
    border-radius: 18px;
    background: var(--panel-fill);
    border: 1px solid var(--edge);
    box-shadow: 0 12px 28px rgba(0, 0, 0, 0.45);
    transition: transform 180ms ease, box-shadow 180ms ease;
  }

  .face:hover,
  .face:focus-within {
    transform: translateY(-2px);
    box-shadow: 0 18px 36px rgba(0, 0, 0, 0.55);
  }

  .face.running {
    box-shadow:
      0 0 0 1.5px var(--accent),
      0 12px 28px rgba(0, 0, 0, 0.45);
  }

  .face.running:hover,
  .face.running:focus-within {
    box-shadow:
      0 0 0 1.5px var(--accent),
      0 18px 36px rgba(0, 0, 0, 0.55);
  }

  .chip {
    width: 10px;
    height: 10px;
    border-radius: 99px;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.28);
    flex: none;
  }

  .lbl {
    max-width: 100%;
    font-size: 0.66rem;
    font-weight: 650;
    letter-spacing: 0.01em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .num,
  .num-in {
    font: inherit;
    font-size: 1.35rem;
    font-weight: 700;
    line-height: 1;
    font-variant-numeric: tabular-nums;
    text-align: center;
    color: var(--ink);
    background: transparent;
    border: none;
    min-width: 24px;
    min-height: 24px;
    padding: 0;
  }

  .num {
    cursor: text;
  }

  .num.time {
    font-size: 1.05rem;
    cursor: default;
  }

  .num-in {
    width: 4.2rem;
    border-bottom: 1px solid var(--accent);
    border-radius: 0;
  }

  .overlay {
    position: absolute;
    right: 28px;
    top: 4px;
    bottom: 4px;
    left: auto;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    min-height: 24px;
    opacity: 0;
    pointer-events: none;
  }

  .face:hover .overlay,
  .face:focus-within .overlay,
  .overlay.hold {
    opacity: 1;
    pointer-events: auto;
  }

  .overlay button {
    min-width: 24px;
    min-height: 24px;
    padding: 0 6px;
    font-size: 0.75rem;
    line-height: 1;
  }

  .ask {
    font-size: 0.62rem;
    color: var(--dim);
  }

  .more,
  .menu {
    position: absolute;
    z-index: 2;
  }

  .more {
    top: 4px;
    right: 4px;
    width: 24px;
    height: 24px;
    padding: 0;
    font: inherit;
    font-size: 0.85rem;
    line-height: 1;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    opacity: 0;
    pointer-events: none;
  }

  .face:hover .more,
  .face:focus-within .more,
  .more[aria-expanded="true"] {
    opacity: 1;
    pointer-events: auto;
    color: var(--ink);
  }

  .menu {
    top: 28px;
    right: 4px;
    min-height: 24px;
    padding: 2px 8px;
    font-size: 0.64rem;
  }

  .empty {
    margin: 0;
    font-size: 0.66rem;
    color: var(--dim);
  }

  @media (prefers-reduced-motion: reduce) {
    .face {
      transition: none;
    }

    .face:hover,
    .face:focus-within,
    .face.running:hover,
    .face.running:focus-within {
      transform: none;
    }
  }
</style>
