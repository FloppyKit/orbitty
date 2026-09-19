<script lang="ts">
  let {
    cheatSheetHint = true,
    onCheatSheet,
  }: {
    cheatSheetHint?: boolean;
    onCheatSheet?: () => void;
  } = $props();

</script>

  {#if cheatSheetHint}
    <div class="actionrow">
      <button
        type="button"
        class="actionhint"
        onclick={() => onCheatSheet?.()}
        title="Cheat sheet — ⌘⇧/"
        aria-label="Open cheat sheet"
      >Action: ⌘?(⌘⇧/)</button>
    </div>
  {/if}
