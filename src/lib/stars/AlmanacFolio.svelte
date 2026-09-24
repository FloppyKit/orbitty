<script lang="ts">
  import SkyField from "../SkyField.svelte";
  import LedgerStack from "./LedgerStack.svelte";
  import type { LedgerPage, Project } from "../persist";

  let {
    pages,
    projects,
    loaded = false,
    onReflect,
  }: {
    pages: LedgerPage[];
    projects: Project[];
    loaded?: boolean;
    onReflect: () => void;
  } = $props();

  let face = $state<"cloud" | "journal">("cloud");
  let userPicked = $state(false);
  let ready = false;
  let prev = 0;

  $effect(() => {
    const n = pages.length;
    if (!loaded) return;
    if (!ready) {
      ready = true;
      prev = n;
      if (!userPicked && n === 0) face = "journal";
      return;
    }
    if (n > prev) face = "journal";
    prev = n;
  });
</script>

<section class="folio" aria-label="Almanac">
  <div class="tabs">
    <div class="tablist" role="tablist">
      <button
        type="button"
        role="tab"
        aria-selected={face === "cloud"}
        class:on={face === "cloud"}
        onclick={() => {
          userPicked = true;
          face = "cloud";
        }}
      >Cloud</button>
      <button
        type="button"
        role="tab"
        aria-selected={face === "journal"}
        class:on={face === "journal"}
        onclick={() => {
          userPicked = true;
          face = "journal";
        }}
      >Journal</button>
    </div>
    {#if face === "journal"}
      <button type="button" class="reflect" onclick={onReflect}>Reflect</button>
    {/if}
    {#if pages.length}
      <span class="count" class:beside={face === "journal"}>{pages.length}</span>
    {/if}
  </div>
  <div class="face">
    <div class="cover" class:behind={face === "journal"}>
      <SkyField />
    </div>
    {#if face === "journal"}
      <div class="pages" role="tabpanel">
        <LedgerStack {pages} {projects} {onReflect} />
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

  .tablist {
    display: flex;
    align-items: center;
    gap: 4px;
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

  .tabs button.reflect {
    margin-left: auto;
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border-color: color-mix(in srgb, var(--ink) 42%, var(--edge));
    border-radius: 7px;
    padding: 3px 10px;
  }

  .count {
    margin-left: auto;
    font-size: 0.64rem;
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }

  .count.beside {
    margin-left: 8px;
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
