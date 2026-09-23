<script lang="ts">
  import type { Project } from "../persist";

  let {
    projects,
    scope,
    onScope,
    onReflect,
  }: {
    projects: Project[];
    scope: string;
    onScope: (id: string) => void;
    onReflect: () => void;
  } = $props();
</script>

<div class="nudges" role="group" aria-label="Nudge scope">
  <button
    type="button"
    class="chip"
    class:on={scope === "overall"}
    aria-pressed={scope === "overall"}
    onclick={() => onScope("overall")}
  >Overall</button>
  {#each projects as p (p.id)}
    <button
      type="button"
      class="chip"
      class:on={scope === p.id}
      aria-pressed={scope === p.id}
      onclick={() => onScope(p.id)}
    >{p.name}</button>
  {/each}
  <button type="button" class="reflect" onclick={onReflect}>Reflect</button>
</div>

<style>
  .nudges {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    overflow-x: auto;
    padding: 4px 10px;
    border-bottom: 1px solid var(--edge);
  }

  .chip {
    flex: none;
    font: inherit;
    font-size: 0.64rem;
    color: var(--ink);
    background: transparent;
    border: 1px solid var(--edge);
    border-radius: 999px;
    padding: 1px 8px;
    cursor: pointer;
  }

  .chip.on {
    background: var(--accent);
    border-color: var(--focus, #6e4db0);
    color: var(--bg);
  }

  .reflect {
    margin-left: auto;
    flex: none;
    font: inherit;
    font-size: 0.68rem;
    font-weight: 700;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 2px 8px;
    cursor: pointer;
  }
</style>
