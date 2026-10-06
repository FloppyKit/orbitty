<script lang="ts">
  type SnapItem = { id: string; title?: string };
  type SnapCol = { id: string; items: SnapItem[]; more: number };

  let {
    columns,
    completingIds,
    activeId,
    onOpen,
    onComplete,
  }: {
    columns: SnapCol[];
    completingIds: string[];
    activeId: string | null;
    onOpen: (id: string) => void;
    onComplete: (id: string) => void;
  } = $props();

  const label: Record<string, string> = {
    needed: "Needed",
    todo: "Todo",
    doing: "Doing",
    done: "Done",
  };
</script>

{#if columns.length > 0}
  <div class="snap preview-snap" aria-label="Board snapshot">
    {#each columns as col (col.id)}
      <div class="snap-col">
        <span class="snap-h">{label[col.id] ?? col.id}</span>
        {#each col.items as item (item.id)}
          <div class="snap-item" class:todo-row={col.id === "todo"}>
            {#if col.id === "todo"}
              <button
                type="button"
                class="circle-btn"
                class:filling={completingIds.includes(item.id)}
                onclick={() => onComplete(item.id)}
                title="Complete — marks the to-do done"
                aria-label="Complete to-do"
                >{#if completingIds.includes(item.id)}✓{/if}</button
              >
            {/if}
            <button
              type="button"
              class="snap-card"
              class:mkey-sel={activeId === `rsnap-${item.id}`}
              data-mkey={`rsnap-${item.id}`}
              onclick={() => onOpen(item.id)}
              title="{item.title ?? ""} — open in Lists">{item.title ?? ""}</button
            >
          </div>
        {/each}
        {#if col.more > 0}
          <span class="snap-more">+{col.more}</span>
        {/if}
      </div>
    {/each}
  </div>
{/if}
