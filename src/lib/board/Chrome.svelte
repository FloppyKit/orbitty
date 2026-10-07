<script lang="ts">
  import WindowTitleRow from "../WindowTitleRow.svelte";
  import { chromeMode, navPlatform, navUa } from "../windowChrome";

  // doneActorChips: Snippet from parent (also used in Columns).
  let {
    embedded,
    hermesNote = $bindable(),
    invoke,
    onBannerMouseDown,
    onClose,
    onPopout,
    selected,
    doneActorChips,
    listTitle = "Inbox",
    titleDot = null,
  } = $props();
</script>

<!-- Embedded Launchpad well keeps today's banner and ×. The real board
     window uses the shared title row (native lights on macOS). -->
{#if embedded}
  <header class="k-banner" onmousedown={onBannerMouseDown}>
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
    {#if selected === "done"}
      {@render doneActorChips()}
    {/if}
    <button
      class="winx"
      onmousedown={(e) => e.stopPropagation()}
      onclick={() => onClose?.()}
      title="Dismiss the Launchpad well — the companion stays"
      aria-label="Dismiss Launchpad">×</button
    >
  </header>
{:else}
  <WindowTitleRow
    title={listTitle}
    dot={titleDot}
    mode={chromeMode(navUa(), navPlatform())}
    label="kanban"
    closeLabel="Hide Lists"
    onClose={() => invoke("hide_window", { label: "kanban" }).catch(() => {})}
    chips={selected === "done" ? doneActorChips : undefined}
  />
{/if}

{#if hermesNote}
  <div class="k-hermesnote" class:err={hermesNote.kind === "err"}>
    <span class="k-hermestext">{hermesNote.text}</span>
    <button class="mini ghost" onclick={() => (hermesNote = null)} aria-label="Dismiss">×</button>
  </div>
{/if}
