<script lang="ts">
  import NeededInbox from "../NeededInbox.svelte";
  import type { BoardApi } from "./api";
  let { board }: { board: BoardApi } = $props();
</script>

<div
          class="mail-needed"
          class:empty={board.neededRows.length === 0 && !board.addingNeeded}
          style="flex-basis: {board.neededRows.length === 0 && !board.addingNeeded
            ? 'auto'
            : `${Math.round(board.neededSplit * 100)}%`}"
        >
          <NeededInbox
            variant="strip"
            cards={board.kanbanCards}
            projects={board.projects}
            projectId={board.neededProjectId}
            mutate={board.mutate}
            logEvent={board.logEvent}
            bind:addingNeeded={board.addingNeeded}
            bind:selectedId={board.neededSelectedId}
            ringActiveId={board.menuRing.activeId}
            onNeededFocus={board.armNeededMouth}
            onNeededAnswer={board.onNeededAnswer}
          />
        </div>
        {#if board.neededRows.length > 0 || board.addingNeeded}
          <div
            class="mail-split"
            role="separator"
            aria-orientation="horizontal"
            aria-label="Resize Needed"
            title="Drag to resize Needed · double-click resets"
            onpointerdown={board.onSplitDown}
            ondblclick={() => {
              board.neededSplit = 0.28;
              board.mutate((s) => {
                s.neededSplit = 0.28;
              });
            }}
          ></div>
        {/if}
        
