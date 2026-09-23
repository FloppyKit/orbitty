<script lang="ts">
  import type { Project } from "../persist";

  let {
    projects,
    scope,
  }: {
    projects: Project[];
    scope: string;
  } = $props();

  const planetName = $derived(
    scope === "overall"
      ? ""
      : (projects.find((p) => p.id === scope)?.name ?? ""),
  );
</script>

<div class="cards" aria-label="Nudge cards">
  <article class="card">
    <h3>Overall</h3>
    <p>No nudge yet</p>
  </article>
  <article class="card">
    <h3>{planetName || "By planet"}</h3>
    <p>No nudge yet</p>
  </article>
</div>

<style>
  .cards {
    flex-shrink: 0;
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding: 6px 10px 8px;
    border-bottom: 1px solid var(--edge);
  }

  .card {
    flex: 1 1 0;
    min-width: 7.5rem;
    max-width: 12rem;
    margin: 0;
    padding: 6px 8px 7px;
    border: 1px dashed var(--edge);
    border-radius: 8px;
    background: var(--panel-fill, var(--panel));
  }

  h3 {
    margin: 0;
    font-size: 0.68rem;
    font-weight: 700;
  }

  p {
    margin: 2px 0 0;
    font-size: 0.64rem;
    color: var(--dim);
  }
</style>
