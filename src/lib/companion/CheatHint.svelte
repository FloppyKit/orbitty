<script lang="ts">
  import type { CompanionApi } from "./api";
  let { dock }: { dock: CompanionApi } = $props();
</script>

  {#if dock.cheatSheetHint}
    <div class="actionrow">
      <button
        type="button"
        class="actionhint"
        onclick={() => dock.onCheatSheet?.()}
        title="Cheat sheet — ⌘⇧/"
        aria-label="Open cheat sheet"
      >Action: ⌘?(⌘⇧/)</button>
    </div>
  {/if}

