<script lang="ts">
  let {
    sendNote = $bindable(null),
    showPromptStrip,
    lowestSpoke,
    ackStrip,
    openSmallestTodo,
  }: {
    sendNote: string | null;
    showPromptStrip: boolean;
    lowestSpoke: { label: string } | null | undefined;
    ackStrip: () => void | Promise<void>;
    openSmallestTodo: () => void | Promise<void>;
  } = $props();
</script>

{#if sendNote}
    <div class="note err">
      <span class="note-text">{sendNote}</span>
      <button
        class="note-x"
        onclick={() => (sendNote = null)}
        aria-label="Dismiss">×</button
      >
    </div>
  {/if}

  {#if showPromptStrip && lowestSpoke}
    <div class="starstrip">
      <span class="starstrip-text">{lowestSpoke.label} is the thin spoke today.</span>
      <span class="starstrip-chips">
        <button class="chip" onclick={ackStrip}>Rest</button>
        <button class="chip" onclick={ackStrip}>Water</button>
        <button class="chip" onclick={openSmallestTodo}>Smallest todo</button>
      </span>
      <button class="starstrip-x" onclick={ackStrip} aria-label="Dismiss">×</button>
    </div>
  {/if}

