<script lang="ts">
  import { tick } from "svelte";
  import type { Project, Tracker } from "../persist";
  import PlanetChip from "./PlanetChip.svelte";
  import TrackerCard from "./TrackerCard.svelte";
  import { isTimer } from "./timer";

  type Row = { projectId: string; projectName: string; tr: Tracker };

  let {
    projects,
    scope,
    today,
    onAdd,
    onBump,
    onSetValue,
    onRemove,
    onPlay,
    onStop,
    onScope,
    onTyping,
    onAddFocus,
    onKeepFocus,
    onRegister,
    showPlanetMenu = false,
  }: {
    projects: Project[];
    scope: string;
    today: string;
    onAdd: (projectId: string, label: string, kind: "number" | "timer") => void | Promise<void>;
    onBump: (projectId: string, trackerId: string, delta: number) => void;
    onSetValue: (projectId: string, trackerId: string, raw: string) => void;
    onRemove: (projectId: string, trackerId: string) => void;
    onPlay: (projectId: string, trackerId: string) => void;
    onStop: (projectId: string, trackerId: string) => void;
    onScope?: (id: string) => void;
    onTyping?: (open: boolean) => void;
    onAddFocus?: (where: "add" | "commit") => void | Promise<void>;
    onKeepFocus?: () => void | Promise<void>;
    onRegister?: (api: { close: () => void } | null) => void;
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
  let gridEl: HTMLElement | null = $state(null);

  const rows = $derived.by(() => {
    const list = scope === "overall" ? projects : projects.filter((p) => p.id === scope);
    const out: Row[] = [];
    for (const p of list) {
      for (const tr of p.trackers ?? []) out.push({ projectId: p.id, projectName: p.name, tr });
    }
    return out;
  });

  const running = $derived(rows.some((r) => isTimer(r.tr) && r.tr.runningSince != null));

  $effect(() => {
    if (!running) return;
    const id = setInterval(() => {
      now = Date.now();
    }, 250);
    return () => clearInterval(id);
  });

  $effect(() => {
    const grid = gridEl;
    rows.length;
    adding;
    if (!grid) return;
    const measure = () => {
      const list = [...grid.querySelectorAll<HTMLElement>(":scope > [data-focus-card], :scope > .add")];
      if (!list.length) {
        grid.setAttribute("data-tracker-cols", "1");
        return;
      }
      const top = list[0].getBoundingClientRect().top;
      let n = 0;
      for (const el of list) {
        if (Math.abs(el.getBoundingClientRect().top - top) < 8) n += 1;
        else break;
      }
      grid.setAttribute("data-tracker-cols", String(Math.max(1, n)));
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(grid);
    return () => ro.disconnect();
  });

  function openAdd() {
    adding = !adding;
    label = "";
    kind = "number";
    planetId = scope !== "overall" && projects.some((p) => p.id === scope) ? scope : (projects[0]?.id ?? "");
  }

  function closeAdd() {
    adding = false;
    label = "";
  }

  function commitAdd() {
    const name = label.trim().slice(0, 7);
    const id = planetId || projects[0]?.id || "";
    if (!name || !id) return false;
    const pending = onAdd(id, name, kind);
    label = "";
    adding = false;
    void landedAdd(pending);
    return true;
  }

  async function landedAdd(pending: Promise<void> | void) {
    await pending;
    await tick();
    onAddFocus?.("add");
  }

  function keepAddFocus() {
    return onKeepFocus?.();
  }

  function onAddFormKey(e: KeyboardEvent) {
    if (e.key !== "Enter") return;
    const target = e.target;
    if (!(target instanceof Element) || target.tagName !== "INPUT") return;
    e.preventDefault();
    e.stopPropagation();
    if (!commitAdd()) onAddFocus?.("commit");
  }

  function publishAddForm() {
    onRegister?.({ close: closeAdd });
  }

  $effect(() => {
    publishAddForm();
  });

  function openEdit(row: Row) {
    menuId = null;
    editingId = row.tr.id;
    draft = String(Math.round(row.tr.value ?? 0));
    onTyping?.(true);
  }

  function commitEdit(row: Row) {
    if (editingId !== row.tr.id) return;
    const raw = draft.trim();
    editingId = null;
    onTyping?.(false);
    if (!/^-?\d+$/.test(raw)) return;
    onSetValue(row.projectId, row.tr.id, raw);
  }

  function cancelEdit() {
    editingId = null;
    onTyping?.(false);
  }
</script>

<section class="trackers" aria-label="Trackers">
  {#if showPlanetMenu}
    <select
      class="planet-menu"
      aria-label="Planet filter"
      data-focus-zone="Z1"
      data-focus-item
      value={scope}
      onchange={(e) => onScope?.((e.currentTarget as HTMLSelectElement).value)}
    >
      <option value="overall">All planets</option>
      {#each projects as p (p.id)}
        <option value={p.id}>{p.name}</option>
      {/each}
    </select>
  {/if}
  <div class="grid" bind:this={gridEl} data-tracker-cols="1">
    {#each rows as row (row.tr.id)}
      <TrackerCard
        projectId={row.projectId}
        projectName={row.projectName}
        tracker={row.tr}
        {today}
        {now}
        editing={editingId === row.tr.id}
        {draft}
        menuOpen={menuId === row.tr.id}
        confirm={confirmId === row.tr.id}
        onBump={(delta) => onBump(row.projectId, row.tr.id, delta)}
        onToggle={() =>
          isTimer(row.tr) && row.tr.runningSince != null
            ? onStop(row.projectId, row.tr.id)
            : onPlay(row.projectId, row.tr.id)}
        onEdit={() => openEdit(row)}
        onDraft={(value) => (draft = value)}
        onCommit={() => commitEdit(row)}
        onCancel={cancelEdit}
        onMenu={() => (menuId = menuId === row.tr.id ? null : row.tr.id)}
        onAskRemove={() => {
          menuId = null;
          confirmId = row.tr.id;
        }}
        onRemove={() => {
          confirmId = null;
          onRemove(row.projectId, row.tr.id);
        }}
        onKeep={() => (confirmId = null)}
      />
    {/each}
    {#if adding}
      <div class="add card" data-add-form data-focus-zone="Z4" data-focus-item>
        <PlanetChip id={planetId || projects[0]?.id || "overall"} size={10} />
        <select bind:value={planetId} aria-label="Planet for this tracker" data-focus-zone="Z4" data-focus-ctl onchange={keepAddFocus}>
          {#each projects as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>
        <input
          bind:value={label}
          maxlength="7"
          placeholder="Name (7)"
          aria-label="New tracker name"
          data-focus-zone="Z4"
          data-focus-ctl
          onkeydown={onAddFormKey}
        />
        <select bind:value={kind} aria-label="Tracker kind" data-focus-zone="Z4" data-focus-ctl onchange={keepAddFocus}>
          <option value="number">Number</option>
          <option value="timer">Timer</option>
        </select>
        <button type="button" class="round" data-focus-zone="Z4" data-focus-ctl onclick={commitAdd}>Add</button>
      </div>
    {:else}
      <button type="button" class="add card" data-focus-zone="Z4" data-focus-item onclick={openAdd}>+ Add</button>
    {/if}
  </div>
</section>

<style>
  .trackers {
    --stars-ring: var(--focus-ring);
    flex: 1;
    min-height: 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .grid {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 10px;
    align-content: start;
  }

  .planet-menu,
  .add select,
  .add input {
    font: inherit;
    font-size: 13px;
    color: var(--ink);
    background: var(--panel-fill);
    border: 1px solid var(--edge);
    border-radius: 8px;
    min-height: 28px;
  }

  .add {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    min-height: 88px;
    padding: 10px;
    border-radius: 10px;
    border: 1px solid var(--edge);
    background: var(--panel-fill);
    color: var(--ink);
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    text-align: left;
    cursor: pointer;
  }

  button.add {
    justify-content: center;
  }

  .round {
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: var(--ink);
    cursor: pointer;
  }

  :global(:root[data-theme="terminal"]) .add {
    background: transparent;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
</style>
