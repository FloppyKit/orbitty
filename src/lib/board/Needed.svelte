<script lang="ts">
  import NeededInbox from "../NeededInbox.svelte";
  import type { KanbanCard, Project } from "../persist";
  let {
    addingNeeded = $bindable(),
    armNeededMouth,
    kanbanCards,
    logEvent,
    menuRing,
    mutate,
    neededProjectId,
    neededRows,
    neededSelectedId = $bindable(),
    neededSplit = $bindable(),
    onNeededAnswer,
    onSplitDown,
    projects,
    neededEl = $bindable(null),
    // Paired with Trackers: only both-empty collapses to one line.
    bandEmpty = false,
  } = $props();

  const neededEmpty = $derived(neededRows.length === 0 && !addingNeeded);
</script>

<div
          class="mail-needed"
          bind:this={neededEl}
          class:empty={neededEmpty}
          class:band-empty={bandEmpty}
          style="flex-basis: {bandEmpty
            ? 'auto'
            : `${Math.round(neededSplit * 100)}%`}"
        >
          <NeededInbox
            variant="strip"
            cards={kanbanCards}
            projects={projects}
            projectId={neededProjectId}
            mutate={mutate}
            logEvent={logEvent}
            bind:addingNeeded={addingNeeded}
            bind:selectedId={neededSelectedId}
            ringActiveId={menuRing.activeId}
            onNeededFocus={armNeededMouth}
            onNeededAnswer={onNeededAnswer}
          />
        </div>
        {#if !bandEmpty}
        <div
            class="mail-split"
            role="separator"
            aria-orientation="horizontal"
            aria-label="Resize Needed"
            title="Drag to resize Needed · double-click resets"
            onpointerdown={onSplitDown}
            ondblclick={() => {
              neededSplit = 0.28;
              mutate((s) => {
                s.neededSplit = 0.28;
                s.trackerSplit = 0.28;
              });
            }}
          ></div>
        {/if}
