<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Project } from "../persist";
  // doneActorChips: Snippet from parent (also used in Columns).
  let {
    embedded,
    hermesNote = $bindable(),
    invoke,
    listTitle,
    onBannerMouseDown,
    onClose,
    onPopout,
    planetCss,
    planetIsLive,
    selected,
    selectedProject,
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

