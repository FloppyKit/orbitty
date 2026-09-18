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
  } = $props();
</script>

<div
          class="mail-needed"
          class:empty={neededRows.length === 0 && !addingNeeded}
          style="flex-basis: {neededRows.length === 0 && !addingNeeded
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
        {#if neededRows.length > 0 || addingNeeded}
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
              });
            }}
          ></div>
        {/if}
        
