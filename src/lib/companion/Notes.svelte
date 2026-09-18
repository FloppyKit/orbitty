<script lang="ts">
  import type { CompanionApi } from "./api";
  let { comp }: { comp: CompanionApi } = $props();
</script>

  {#if comp.sendNote}
    <div class="note err">
      <span class="note-text">sendNote={comp.sendNote}</span>
      <button
        class="note-x"
        onclick={() => (comp.sendNote = null)}
        aria-label="Dismiss">×</button
      >
    </div>
  {/if}

  {#if comp.showPromptStrip && comp.lowestSpoke}
    <div class="starstrip">
      <span class="starstrip-text">{comp.lowestSpoke.label} is the thin spoke today.</span>
      <span class="starstrip-chips">
        <button class="chip" onclick={comp.ackStrip}>Rest</button>
        <button class="chip" onclick={comp.ackStrip}>Water</button>
        <button class="chip" onclick={comp.openSmallestTodo}>Smallest todo</button>
      </span>
      <button class="starstrip-x" onclick={comp.ackStrip} aria-label="Dismiss">×</button>
    </div>
  {/if}

