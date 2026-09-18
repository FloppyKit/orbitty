<script lang="ts">
  import type { BoardApi } from "./api";
  let { board }: { board: BoardApi } = $props();
</script>

<!-- One drag source: JS startDragging on this banner. Drawers are
       borderless — no native Overlay strip competing with the click.
       HOST-SPLIT: embedded, this drags the host companion window. -->
  <header class="k-banner" onmousedown={board.onBannerMouseDown}>
    {#if board.embedded}
      <h1>Launchpad</h1>
      <!-- HOST-SPLIT: pop-out lives on the embedded Launchpad banner only
           (never STARS, never companion chrome) — the real kanban window
           becomes the popped Launchpad. -->
      <button
        class="popout"
        onmousedown={(e) => e.stopPropagation()}
        onclick={() => board.onPopout?.()}
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
        board.embedded
          ? board.onClose?.()
          : board.invoke("hide_window", { label: "kanban" }).catch(() => {})}
      title={board.embedded
        ? "Dismiss the Launchpad well — the companion stays"
        : "Hide Lists — reopen from the rail or tray"}
      aria-label={board.embedded ? "Dismiss Launchpad" : "Hide Lists"}>×</button
    >
  </header>

  <div class="planet-strip" role="tablist" aria-label="Lists and planets">
    <div class="strip-pins">
      <button
        type="button"
        class="planet-tab"
        class:sel={board.selected === "inbox"}
        class:mkey-sel={board.menuRing.activeId === "listtab-inbox"}
        data-mkey="listtab-inbox"
        role="tab"
        aria-selected={board.selected === "inbox"}
        onclick={() => (board.selected = "inbox")}
      >
        <span class="planet-tab-name">Inbox</span>
        <span class="side-count">{board.sideCount("inbox")}</span>
      </button>
      <button
        type="button"
        class="planet-tab"
        class:sel={board.selected === "today"}
        class:mkey-sel={board.menuRing.activeId === "listtab-today"}
        data-mkey="listtab-today"
        role="tab"
        aria-selected={board.selected === "today"}
        onclick={() => (board.selected = "today")}
      >
        <span class="planet-tab-name">Today</span>
        <span class="side-count">{board.sideCount("today")}</span>
      </button>
      <button
        type="button"
        class="planet-tab"
        class:sel={board.selected === "done"}
        class:mkey-sel={board.menuRing.activeId === "listtab-done"}
        data-mkey="listtab-done"
        role="tab"
        aria-selected={board.selected === "done"}
        data-col="done"
        class:drop-ok={board.dropCol === "done" || board.grabCol === "done"}
        title="Done — click to show completed, click again to return"
        onclick={() => board.toggleDoneChip()}
        ondragover={(e) => board.onColDragOver(e, "done")}
        ondragleave={(e) => board.onColDragLeave(e, "done")}
        ondrop={(e) => board.onColDrop(e, "done")}
      >
        <span class="planet-tab-name">Done</span>
        <span class="side-count">{board.sideCount("done")}</span>
      </button>
    </div>
    <span class="strip-gap" aria-hidden="true"></span>
    <div class="strip-planets">
    {#each board.projects as p (p.id)}
      <button
        type="button"
        class="planet-tab"
        class:sel={board.selected === p.id}
        class:drop-ok={board.planetDropId === p.id}
        class:dragging={board.planetDragId === p.id}
        class:mkey-sel={board.menuRing.activeId === `listtab-${p.id}`}
        data-mkey={`listtab-${p.id}`}
        data-pid={p.id}
        role="tab"
        aria-selected={board.selected === p.id}
        title={p.name}
        onpointerdown={(e) => board.onPlanetPointerDown(e, p.id)}
        onclick={() => board.onPlanetTabClick(p.id)}
        ondblclick={() => board.onPlanetTabDblClick(p.id)}
      >
        <span
          class="planet-chip"
          class:live={board.planetIsLive(p.id)}
          style="background: {board.planetCss(p.id)}; --pglow: {board.planetCss(p.id)}"
          aria-hidden="true"
        ></span>
        <span class="planet-tab-name">{p.name}</span>
        <span class="side-count">{board.sideCount(p.id)}</span>
      </button>
    {/each}
    </div>
  </div>

  <div class="k-heads" onmousedown={board.onBannerMouseDown}>
    <div class="k-titlerow">
      {#if board.selectedProject}
        <span
          class="planet-chip"
          class:live={board.planetIsLive(board.selectedProject.id)}
          style="background: {board.planetCss(board.selectedProject.id)}; --pglow: {board.planetCss(board.selectedProject.id)}"
          title={board.selectedProject.name}
          aria-hidden="true"
        ></span>
      {/if}
      <h2 class="k-title">{board.listTitle}</h2>
      {#if board.selected === "done"}
        {@render doneActorChips()}
      {/if}
    </div>
  </div>
  {#if board.hermesNote}
    <div class="k-hermesnote" class:err={board.hermesNote.kind === "err"}>
      <span class="k-hermestext">{board.hermesNote.text}</span>
      <button class="mini ghost" onclick={() => (board.hermesNote = null)} aria-label="Dismiss">×</button>
    </div>
  {/if}

