<script lang="ts">
  import SkyField from "../SkyField.svelte";
  import LedgerStack from "./LedgerStack.svelte";
  import type { LedgerPage, Project } from "../persist";

  let {
    pages,
    projects,
  }: {
    pages: LedgerPage[];
    projects: Project[];
  } = $props();

  let face = $state<"cloud" | "journal">("cloud");
</script>

<section class="folio" aria-label="Almanac">
  <div class="tabs" role="tablist">
    <button
      type="button"
      role="tab"
      aria-selected={face === "cloud"}
      class:on={face === "cloud"}
      onclick={() => (face = "cloud")}
    >Cloud</button>
    <button
      type="button"
      role="tab"
      aria-selected={face === "journal"}
      class:on={face === "journal"}
      onclick={() => (face = "journal")}
    >Journal</button>
    {#if pages.length}
      <span class="count">{pages.length}</span>
    {/if}
  </div>
  <div class="face">
    <div class="cover" class:behind={face === "journal"}>
      <SkyField />
    </div>
    {#if face === "journal"}
      <div class="pages" role="tabpanel">
        <LedgerStack {pages} {projects} />
      </div>
    {/if}
  </div>
</section>

<style>
  .folio {
    flex: 1 1 0;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--edge);
    border-radius: 12px;
    background: var(--panel-fill, var(--panel));
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.08);
    overflow: hidden;
  }

  .tabs {
    flex: none;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--edge);
  }

  .tabs button {
    font: inherit;
    font-size: 0.68rem;
    font-weight: 650;
    color: var(--dim);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 999px;
    padding: 2px 8px;
    cursor: pointer;
  }

  .tabs button.on {
    color: var(--ink);
    border-color: var(--edge);
    background: var(--bg-fill, var(--bg));
  }

  .count {
    margin-left: auto;
    font-size: 0.64rem;
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }

  .face {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .cover {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .cover.behind {
    flex: none;
    height: 36px;
    overflow: hidden;
    opacity: 0.55;
  }

  .pages {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 8px 10px 10px;
    background: var(--bg-fill, var(--bg));
  }
</style>
