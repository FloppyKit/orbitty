<script lang="ts">
  import { columnLabel, type ColumnId, type FoldedProject } from "./fold";

  let { projects }: { projects: FoldedProject[] } = $props();

  const columns: ColumnId[] = ["todo", "doing", "done"];
</script>

{#if projects.length === 0}
  <p class="empty">No board proofs.</p>
{:else}
  {#each projects as project (project.projectId ?? "")}
    <section class="project">
      <h3>{project.projectId ?? "No project tag"}</h3>
      {#each columns as column (column)}
        <div class="column" data-column={column}>
          <header>
            <span>{columnLabel(column)}</span>
            <span class="count">{project[column].length}</span>
          </header>
          {#if project[column].length === 0}
            <p class="none">Nothing here</p>
          {:else}
            <ul>
              {#each project[column] as card (card.cardId)}
                <li>
                  <span class="title">{card.title}</span>
                  {#if card.title !== card.cardId}
                    <span class="id">{card.cardId}</span>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/each}
    </section>
  {/each}
{/if}

<style>
  .empty,
  .none {
    margin: 0;
    color: #6d675e;
    font-size: 13px;
  }

  .project + .project {
    margin-top: 18px;
  }

  h3 {
    margin: 0 0 8px;
    font-size: 12px;
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: #5c564c;
  }

  .column {
    margin: 0 0 10px;
    border-radius: 14px;
    background: #fff;
    overflow: hidden;
  }

  .column[data-column="todo"] header {
    background: #f3efe6;
  }

  .column[data-column="doing"] header {
    background: #e4eef6;
  }

  .column[data-column="done"] header {
    background: #e5f2e6;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    padding: 8px 12px;
    font-size: 13px;
    font-weight: 650;
  }

  .count {
    font-variant-numeric: tabular-nums;
    color: #6d675e;
    font-weight: 550;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 4px 0;
  }

  li {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 12px;
  }

  li + li {
    border-top: 1px solid #f0ebe3;
  }

  .title {
    font-size: 15px;
    line-height: 1.3;
  }

  .id {
    font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
    font-size: 11px;
    color: #8a8378;
  }

  .none {
    padding: 8px 12px 12px;
  }
</style>
