<script lang="ts">
  import type { Project } from "../persist";
  import { planetCss } from "../planetColor";

  let {
    projects,
    scope,
  }: {
    projects: Project[];
    scope: string;
  } = $props();

  const planet = $derived(
    scope === "overall" ? null : (projects.find((p) => p.id === scope) ?? null),
  );
</script>

<div class="cards" aria-label="Nudge cards">
  <article class="card">
    <p class="kicker">Overall</p>
    <h3>Across planets</h3>
    <p class="blank">No nudge yet</p>
  </article>
  <article class="card">
    <p class="kicker">
      {#if planet}
        <span class="dot" style="background: {planetCss(planet.id)}"></span>
      {/if}
      By planet
    </p>
    <h3>{planet?.name ?? "Pick a planet"}</h3>
    <p class="blank">No nudge yet</p>
  </article>
</div>

<style>
  .cards {
    flex-shrink: 0;
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding: 8px 10px 10px;
    border-bottom: 1px solid var(--edge);
  }

  .card {
    flex: 1 1 0;
    min-width: 8.5rem;
    max-width: 14rem;
    margin: 0;
    padding: 8px 10px 9px;
    border: 1px solid var(--edge);
    border-radius: 12px;
    background: var(--panel-fill, var(--panel));
    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.12);
  }

  .kicker {
    display: flex;
    align-items: center;
    gap: 5px;
    margin: 0;
    font-size: 0.6rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--dim);
  }

  h3 {
    margin: 3px 0 0;
    font-size: 0.84rem;
    font-weight: 650;
    letter-spacing: -0.01em;
  }

  .blank {
    margin: 4px 0 0;
    font-size: 0.68rem;
    color: var(--dim);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 99px;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.28);
    flex: none;
  }
</style>
