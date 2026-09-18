<script lang="ts">
  import BoardNeeded from "./Needed.svelte";
  import type { KanbanCard } from "../persist";
  import type { BoardApi } from "./api";
  let { board }: { board: BoardApi } = $props();
</script>

{#snippet metaChips(card: KanbanCard)}
  {@const due = board.dueInfo(card)}
  {@const sub = board.subProgress(card)}
  {@const who = board.agentName(card)}
  {@const age = board.ageLabel(card)}
  {@const need = board.neededForTodo(card)}
  {#if need}
    <span class="k-needchip" title={need.title}>needed</span>
  {/if}
  {#if board.hasAgent(card)}
    <span class="k-bot" title={who || "agent"} aria-label={who || "agent"}>
      <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <rect x="7" y="9" width="10" height="8" rx="2"/>
        <circle cx="10" cy="13" r="1.2"/>
        <circle cx="14" cy="13" r="1.2"/>
        <rect x="11" y="5" width="2" height="4" rx="1"/>
        <circle cx="12" cy="4.5" r="1.3"/>
        <rect x="5" y="11" width="2" height="4" rx="1"/>
        <rect x="17" y="11" width="2" height="4" rx="1"/>
      </svg>
    </span>
  {/if}
  {#if card.neededState}
    <span class="k-needst" title="Needed state">{card.neededState}</span>
  {/if}
  {#if card.rating}
    <span class="k-rating" title="Rating {card.rating}">{card.rating}</span>
  {/if}
  {#if due}
    <span class="k-due" class:overdue={due.overdue} title="Due date">{due.label}</span>
  {/if}
  {#if sub}
    <span class="k-sub" title="Sub-checklist done/total">{sub}</span>
  {/if}
  {#if board.showProj(card)}
    <span class="k-proj">{board.projectName(card.projectId)}</span>
  {/if}
  {#if age}
    <span class="k-age" title="Last touched">{age}</span>
  {/if}

{/snippet}

{#snippet stackCard(card: KanbanCard)}
  {@const pid = board.livePlanetId(card)}
  {@const floating = board.grabbedId === card.id && !!board.grabGhost}
  {#if floating}
    <div class="b-slot" style="height:{board.grabGhost.h}px"></div>
  {/if}
  <div
    class="b-row"
    class:child={!!card.parentId}
    class:lifted={floating}
    style={floating && board.grabGhost
      ? `position:fixed;left:${grabGhost.x}px;top:${grabGhost.y}px;width:${grabGhost.w}px;z-index:40;margin:0;`
      : undefined}
  >
  <button
    type="button"
    class="k-check"
    class:checked={card.status === "done"}
    onclick={(e) => {
      e.stopPropagation();
      board.moveCard(card, card.status === "done" ? "todo" : "done");
    }}
    onpointerdown={(e) => e.stopPropagation()}
    aria-label={card.status === "done" ? "Reopen" : "Mark done"}
    title={card.status === "done" ? "Reopen" : "Mark done"}
  >{card.status === "done" ? "✓" : ""}</button>
  <article
    class="b-card"
    class:dragging={board.dragCardId === card.id}
    class:held={board.grabbedId === card.id}
    class:done={card.status === "done"}
    class:doing={card.status === "doing"}
    class:sel={board.detailId === card.id}
    class:mkey-sel={board.menuRing.activeId === `card-${card.id}`}
    data-mkey={`card-${card.id}`}
    draggable="true"
    ondragstart={(e) => board.onCardDragStart(e, card)}
    ondragend={board.onCardDragEnd}
    onpointerdown={(e) => board.onCardPointerDown(e, card)}
    onclick={() => board.onCardClick(card)}
    onkeydown={(e) => e.key === "Enter" && !e.shiftKey && board.openDetail(card)}
    tabindex="0"
    title="{board.displayTitle(card)} — click for details, drag to move"
  >
    <div class="b-card-top">
      {#if pid}
        <span
          class="planet-chip"
          class:live={board.isWriting(card)}
          style="background: {board.planetCss(pid)}; --pglow: {board.planetCss(pid)}"
          title={board.projectName(pid)}
          aria-hidden="true"
        ></span>
      {/if}
      <span class="b-title" class:strike={card.status === "done"}>{board.displayTitle(card)}</span>
      {#if card.status !== "done" && !card.parentId}
        <button
          type="button"
          class="k-doing"
          class:on={card.status === "doing"}
          class:live={board.isWriting(card)}
          onclick={(e) => board.toggleDoing(card, e)}
          onpointerdown={(e) => e.stopPropagation()}
          title={card.status === "doing" ? "Doing — click to park" : "Start — pin to the top"}
        >
          {#if card.status === "doing"}
            <span class="k-doing-ico"><svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="12" cy="12" r="4"/><circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" stroke-width="1.6"/></svg></span>
            <span class="k-doing-lbl">doing</span>
          {:else}
            <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"/></svg>
            start
          {/if}
        </button>
      {/if}
      {#if !card.parentId && board.childrenOf(card.id).length > 0}
        <button
          type="button"
          class="k-subs"
          class:open={board.openSubtaskIds.includes(card.id)}
          onclick={(e) => board.toggleOpenSubtasks(card.id, e)}
          onpointerdown={(e) => e.stopPropagation()}
          title={board.openSubtaskIds.includes(card.id) ? "Hide subtasks" : "Show subtasks"}
        >
          <span class="chev" class:open={board.openSubtaskIds.includes(card.id)}>▸</span>
          {board.remainingKids(card.id)}
        </button>
      {/if}
    </div>
    <div class="b-meta">
      {@render metaChips(card)}
    </div>
  </article>
  </div>

{/snippet}

{#snippet doneActorChips()}
  <!-- Happened split — All default, session-only. -->
  <div class="actor-chips" role="group" aria-label="Filter Done by who acted">
    <button
      class:active={board.doneFilter === "all"}
      aria-pressed={board.doneFilter === "all"}
      onclick={() => (board.doneFilter = "all")}>All</button
    >
    <button
      class:active={board.doneFilter === "mine"}
      aria-pressed={board.doneFilter === "mine"}
      onclick={() => (board.doneFilter = "mine")}>I did</button
    >
    <button
      class:active={board.doneFilter === "behalf"}
      aria-pressed={board.doneFilter === "behalf"}
      onclick={() => (board.doneFilter = "behalf")}>On my behalf</button
    >
  </div>

{/snippet}

<section class="k-main" aria-label={board.listTitle}>

      <div class="mail" bind:this={board.mailEl}>
        {#if board.selected === "done"}
          <section class="mail-todo mail-done" aria-label="Done">
            <div
              class="b-zone todo"
              class:drop-ok={board.dropCol === "done" || board.grabCol === "done"}
              data-col="done"
              ondragover={(e) => board.onColDragOver(e, "done")}
              ondragleave={(e) => board.onColDragLeave(e, "done")}
              ondrop={(e) => board.onColDrop(e, "done")}
            >
              <header class="b-head">
                <span class="b-name">Done</span>
                <span class="b-count">{board.viewCols.done.length}</span>
                {@render doneActorChips()}
              </header>
              {#each board.viewCols.done as card (card.id)}
                {@render stackCard(card)}
              {:else}
                <p class="b-empty">Nothing done yet.</p>
              {/each}
            </div>
          </section>
        {:else}
        <BoardNeeded {board} />
        <section class="mail-todo" aria-label="Todos">
          <div
            class="b-zone todo"
            class:drop-ok={board.dropCol === "todo" || board.grabCol === "todo"}
            ondragover={(e) => board.onColDragOver(e, "todo")}
            ondragleave={(e) => board.onColDragLeave(e, "todo")}
            ondrop={(e) => board.onColDrop(e, "todo")}
          >
            <header class="b-head">
              <span class="mail-ico" aria-hidden="true">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
                  <rect x="4.5" y="4.5" width="15" height="15" rx="3.5"/>
                  <path d="M8 12.2l2.6 2.6L16.5 9" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              </span>
              <span class="b-name">Todo</span>
              <span class="b-count">{board.viewCols.doing.length + board.viewCols.todo.length}</span>
              <button
                type="button"
                class="ka-plus"
                onclick={() => (board.addingTodo = !board.addingTodo)}
                title={board.addingTodo ? "Cancel" : "Add a to-do"}
                aria-label={board.addingTodo ? "Cancel add to-do" : "Add a to-do"}
              >{board.addingTodo ? "×" : "+"}</button>
            </header>
            {#if board.addingTodo}
              <div class="todo-add">
                <input
                  class="d-input"
                  bind:value={board.newTodoTitle}
                  placeholder="New to-do…"
                  aria-label="New to-do title"
                  onkeydown={(e) => {
                    if (e.key === "Enter") board.mintTodo(board.newTodoTitle);
                    if (e.key === "Escape") board.addingTodo = false;
                  }}
                />
                <button class="mini" onclick={() => board.mintTodo(board.newTodoTitle)}>Add</button>
              </div>
            {/if}
            <div class="doing-lane" data-col="doing">
            {#each board.viewCols.doing as card (card.id)}
              {@render stackCard(card)}
              {#if board.openSubtaskIds.includes(card.id)}
                {#each board.childrenOf(card.id) as kid (kid.id)}
                  {@render stackCard(kid)}
                {/each}
              {/if}
            {/each}
            </div>
            <div class="doing-floor" aria-hidden="true"></div>
            <div class="todo-lane" data-col="todo">
            {#each board.viewCols.todo as card (card.id)}
              {@render stackCard(card)}
              {#if board.openSubtaskIds.includes(card.id)}
                {#each board.childrenOf(card.id) as kid (kid.id)}
                  {@render stackCard(kid)}
                {/each}
              {/if}
            {:else}
              {#if board.viewCols.doing.length === 0}
                <p class="b-empty">No to-dos — drag a card here.</p>
              {/if}
            {/each}
            </div>
          </div>
        </section>
        {/if}
      </div>
    </section>

