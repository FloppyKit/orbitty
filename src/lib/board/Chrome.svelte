<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Project } from "../persist";
  // doneActorChips: Snippet from parent (also used in Columns).
  let {
    dropCol,
    embedded,
    grabCol,
    hermesNote = $bindable(),
    invoke,
    listTitle,
    menuRing,
    onBannerMouseDown,
    onClose,
    onColDragLeave,
    onColDragOver,
    onColDrop,
    onPlanetPointerDown,
    onPlanetTabClick,
    onPlanetTabDblClick,
    onPopout,
    planetCss,
    planetDragId,
    planetDropId,
    planetIsLive,
    projects,
    selected = $bindable(),
    selectedProject,
    sideCount,
    toggleDoneChip,
    doneActorChips,
  } = $props();
</script>

<!-- One drag source: JS startDragging on this banner. Drawers are
       borderless — no native Overlay strip competing with the click.
       HOST-SPLIT: embedded, this drags the host companion window. -->
  <header class="k-banner" onmousedown={onBannerMouseDown}>
    {#if embedded}
      <h1>Launchpad</h1>
      <!-- HOST-SPLIT: pop-out lives on the embedded Launchpad banner only
           (never STARS, never companion chrome) — the real kanban window
           becomes the popped Launchpad. -->
      <button
        class="popout"
        onmousedown={(e) => e.stopPropagation()}
        onclick={() => onPopout?.()}
        title="Pop out to the real Lists window — the well closes, the companion stays"
        aria-label="Pop out Launchpad"
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="M15 3h6v6" />
          <path d="M10 14 21 3" />
          <path d="M21 14v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5" />
        </svg>
      </button>
    {:else}
      <h1><span class="mark"><span class="o">O</span><span class="rest">rb</span><span class="idot">i</span><span class="pop">tty</span></span> Lists</h1>
    {/if}
    <button
      class="winx"
      onmousedown={(e) => e.stopPropagation()}
      onclick={() =>
        embedded
          ? onClose?.()
          : invoke("hide_window", { label: "kanban" }).catch(() => {})}
      title={embedded
        ? "Dismiss the Launchpad well — the companion stays"
        : "Hide Lists — reopen from the rail or tray"}
      aria-label={embedded ? "Dismiss Launchpad" : "Hide Lists"}>×</button
    >
  </header>

  <div class="planet-strip" role="tablist" aria-label="Lists and planets">
    <div class="strip-pins">
      <button
        type="button"
        class="planet-tab"
        class:sel={selected === "inbox"}
        class:mkey-sel={menuRing.activeId === "listtab-inbox"}
        data-mkey="listtab-inbox"
        role="tab"
        aria-selected={selected === "inbox"}
        onclick={() => (selected = "inbox")}
      >
        <span class="planet-tab-name">Inbox</span>
        <span class="side-count">{sideCount("inbox")}</span>
      </button>
      <button
        type="button"
        class="planet-tab"
        class:sel={selected === "today"}
        class:mkey-sel={menuRing.activeId === "listtab-today"}
        data-mkey="listtab-today"
        role="tab"
        aria-selected={selected === "today"}
        onclick={() => (selected = "today")}
      >
        <span class="planet-tab-name">Today</span>
        <span class="side-count">{sideCount("today")}</span>
      </button>
      <button
        type="button"
        class="planet-tab"
        class:sel={selected === "done"}
        class:mkey-sel={menuRing.activeId === "listtab-done"}
        data-mkey="listtab-done"
        role="tab"
        aria-selected={selected === "done"}
        data-col="done"
        class:drop-ok={dropCol === "done" || grabCol === "done"}
        title="Done — click to show completed, click again to return"
        onclick={() => toggleDoneChip()}
        ondragover={(e) => onColDragOver(e, "done")}
        ondragleave={(e) => onColDragLeave(e, "done")}
        ondrop={(e) => onColDrop(e, "done")}
      >
        <span class="planet-tab-name">Done</span>
        <span class="side-count">{sideCount("done")}</span>
      </button>
    </div>
    <span class="strip-gap" aria-hidden="true"></span>
    <div class="strip-planets">
    {#each projects as p (p.id)}
      <button
        type="button"
        class="planet-tab"
        class:sel={selected === p.id}
        class:drop-ok={planetDropId === p.id}
        class:dragging={planetDragId === p.id}
        class:mkey-sel={menuRing.activeId === `listtab-${p.id}`}
        data-mkey={`listtab-${p.id}`}
        data-pid={p.id}
        role="tab"
        aria-selected={selected === p.id}
        title={p.name}
        onpointerdown={(e) => onPlanetPointerDown(e, p.id)}
        onclick={() => onPlanetTabClick(p.id)}
        ondblclick={() => onPlanetTabDblClick(p.id)}
      >
        <span
          class="planet-chip"
          class:live={planetIsLive(p.id)}
          style="background: {planetCss(p.id)}; --pglow: {planetCss(p.id)}"
          aria-hidden="true"
        ></span>
        <span class="planet-tab-name">{p.name}</span>
        <span class="side-count">{sideCount(p.id)}</span>
      </button>
    {/each}
    </div>
  </div>

  <div class="k-heads" onmousedown={onBannerMouseDown}>
    <div class="k-titlerow">
      {#if selectedProject}
        <span
          class="planet-chip"
          class:live={planetIsLive(selectedProject.id)}
          style="background: {planetCss(selectedProject.id)}; --pglow: {planetCss(selectedProject.id)}"
          title={selectedProject.name}
          aria-hidden="true"
        ></span>
      {/if}
      <h2 class="k-title">{listTitle}</h2>
      {#if selected === "done"}
        {@render doneActorChips()}
      {/if}
    </div>
  </div>
  {#if hermesNote}
    <div class="k-hermesnote" class:err={hermesNote.kind === "err"}>
      <span class="k-hermestext">{hermesNote.text}</span>
      <button class="mini ghost" onclick={() => (hermesNote = null)} aria-label="Dismiss">×</button>
    </div>
  {/if}

