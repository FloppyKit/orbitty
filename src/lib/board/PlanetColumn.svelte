<script lang="ts">
  import PlanetHead from "../PlanetHead.svelte";
  import { columnPx, type ColumnMode } from "./planetColumn";
  import type { Project } from "../persist";

  type Status = "todo" | "doing" | "done";

  let {
    dropCol,
    grabCol,
    menuRing,
    mode,
    onColDragLeave,
    onColDragOver,
    onColDrop,
    onPlanetPointerDown,
    onPlanetTabClick,
    onPlanetTabDblClick,
    onToggle,
    planetCss,
    planetDragId,
    planetDropId,
    planetIsLive,
    projects,
    selected = $bindable(),
    sideCount,
    toggleDoneChip,
  }: {
    dropCol: Status | null;
    grabCol: Status | null;
    menuRing: { activeId: string | null };
    mode: ColumnMode;
    onColDragLeave: (e: DragEvent, col: Status) => void;
    onColDragOver: (e: DragEvent, col: Status) => void;
    onColDrop: (e: DragEvent, col: Status) => void;
    onPlanetPointerDown: (e: PointerEvent, id: string) => void;
    onPlanetTabClick: (id: string) => void;
    onPlanetTabDblClick: (id: string) => void;
    onToggle: () => void;
    planetCss: (id: string) => string;
    planetDragId: string | null;
    planetDropId: string | null;
    planetIsLive: (id: string) => boolean;
    projects: Project[];
    selected: string;
    sideCount: (list: string) => number;
    toggleDoneChip: () => void;
  } = $props();

  const compact = $derived(mode === "icons");
</script>

<div
  data-glass
  class="planet-col"
  class:icons={compact}
  style="width: {columnPx(mode)}px"
>
  <button
    type="button"
    class="planet-col-toggle"
    aria-pressed={compact}
    aria-label={compact ? "Show planet names" : "Collapse planet names"}
    title={compact ? "Show names" : "Chips only"}
    onclick={onToggle}
  >
    {compact ? "›" : "‹"}
  </button>
  <div class="planet-col-list" role="tablist" aria-label="Lists and planets">
  <div class="planet-col-pins">
    <PlanetHead
      name="Inbox"
      mark="I"
      count={sideCount("inbox")}
      {compact}
      sel={selected === "inbox"}
      mkeySel={menuRing.activeId === "listtab-inbox"}
      mkey="listtab-inbox"
      onclick={() => (selected = "inbox")}
    />
    <PlanetHead
      name="Today"
      mark="T"
      count={sideCount("today")}
      {compact}
      sel={selected === "today"}
      mkeySel={menuRing.activeId === "listtab-today"}
      mkey="listtab-today"
      onclick={() => (selected = "today")}
    />
    <PlanetHead
      name="Done"
      mark="D"
      hint="Done — click to show completed, click again to return"
      count={sideCount("done")}
      {compact}
      sel={selected === "done"}
      mkeySel={menuRing.activeId === "listtab-done"}
      mkey="listtab-done"
      col="done"
      dropOk={dropCol === "done" || grabCol === "done"}
      onclick={() => toggleDoneChip()}
      ondragover={(e) => onColDragOver(e, "done")}
      ondragleave={(e) => onColDragLeave(e, "done")}
      ondrop={(e) => onColDrop(e, "done")}
    />
  </div>
  <div class="planet-col-rule" role="separator"></div>
  <div class="planet-col-planets">
    {#each projects as p (p.id)}
      <PlanetHead
        name={p.name}
        chip={planetCss(p.id)}
        count={sideCount(p.id)}
        {compact}
        live={planetIsLive(p.id)}
        sel={selected === p.id}
        mkeySel={menuRing.activeId === `listtab-${p.id}`}
        mkey={`listtab-${p.id}`}
        pid={p.id}
        dragging={planetDragId === p.id}
        dropOk={planetDropId === p.id}
        onpointerdown={(e) => onPlanetPointerDown(e, p.id)}
        onclick={() => onPlanetTabClick(p.id)}
        ondblclick={() => onPlanetTabDblClick(p.id)}
      />
    {/each}
  </div>
  </div>
</div>
