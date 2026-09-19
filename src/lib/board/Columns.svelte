<script lang="ts">
  import BoardNeeded from "./Needed.svelte";
  import type { KanbanCard } from "../persist";
  let {
    addingTodo = $bindable(),
    ageLabel,
    agentName,
    childrenOf,
    detailId = $bindable(),
    displayTitle,
    doneFilter = $bindable(),
    dragCardId,
    dropCol,
    dueInfo,
    grabCol,
    grabGhost,
    grabbedId,
    hasAgent,
    isWriting,
    listTitle,
    livePlanetId,
    mailEl = $bindable(),
    menuRing,
    mintTodo,
    moveCard,
    neededForTodo,
    newTodoTitle = $bindable(),
    onCardClick,
    onCardDragEnd,
    onCardDragStart,
    onCardPointerDown,
    onColDragLeave,
    onColDragOver,
    onColDrop,
    openDetail,
    openSubtaskIds = $bindable(),
    planetCss,
    projectName,
    remainingKids,
    selected = $bindable(),
    showProj,
    subProgress,
    toggleDoing,
    toggleOpenSubtasks,
    // Needed leaf (forwarded)
    addingNeeded = $bindable(),
    armNeededMouth,
    kanbanCards,
    logEvent,
    mutate,
    neededProjectId,
    neededRows,
    neededSelectedId = $bindable(),
    neededSplit = $bindable(),
    neededEl = $bindable(null),
    onNeededAnswer,
    onSplitDown,
    projects,
    viewCols,
  } = $props();
</script>

{#snippet metaChips(card: KanbanCard)}
  {@const due = dueInfo(card)}
  {@const sub = subProgress(card)}
  {@const who = agentName(card)}
  {@const age = ageLabel(card)}
  {@const need = neededForTodo(card)}
  {#if need}
    <span class="k-needchip" title={need.title}>needed</span>
  {/if}
  {#if hasAgent(card)}
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
  {#if showProj(card)}
    <span class="k-proj">{projectName(card.projectId)}</span>
  {/if}
  {#if age}
    <span class="k-age" title="Last touched">{age}</span>
  {/if}

{/snippet}

{#snippet stackCard(card: KanbanCard)}
  {@const pid = livePlanetId(card)}
  {@const floating = grabbedId === card.id && !!grabGhost}
  {#if floating}
    <div class="b-slot" style="height:{grabGhost.h}px"></div>
  {/if}
  <div
    class="b-row"
    class:child={!!card.parentId}
    class:lifted={floating}
    style={floating && grabGhost
      ? `position:fixed;left:${grabGhost.x}px;top:${grabGhost.y}px;width:${grabGhost.w}px;z-index:40;margin:0;`
      : undefined}
  >
  <button
    type="button"
    class="k-check"
    class:checked={card.status === "done"}
    onclick={(e) => {
      e.stopPropagation();
      moveCard(card, card.status === "done" ? "todo" : "done");
    }}
    onpointerdown={(e) => e.stopPropagation()}
    aria-label={card.status === "done" ? "Reopen" : "Mark done"}
    title={card.status === "done" ? "Reopen" : "Mark done"}
  >{card.status === "done" ? "✓" : ""}</button>
  <article
    class="b-card"
    class:dragging={dragCardId === card.id}
    class:held={grabbedId === card.id}
    class:done={card.status === "done"}
    class:doing={card.status === "doing"}
    class:sel={detailId === card.id}
    class:mkey-sel={menuRing.activeId === `card-${card.id}`}
    data-mkey={`card-${card.id}`}
    draggable="true"
    ondragstart={(e) => onCardDragStart(e, card)}
    ondragend={onCardDragEnd}
    onpointerdown={(e) => onCardPointerDown(e, card)}
    onclick={() => onCardClick(card)}
    onkeydown={(e) => e.key === "Enter" && !e.shiftKey && openDetail(card)}
    tabindex="0"
    title="{displayTitle(card)} — click for details, drag to move"
  >
    <div class="b-card-top">
      {#if pid}
        <span
          class="planet-chip"
          class:live={isWriting(card)}
          style="background: {planetCss(pid)}; --pglow: {planetCss(pid)}"
          title={projectName(pid)}
          aria-hidden="true"
        ></span>
      {/if}
      <span class="b-title" class:strike={card.status === "done"}>{displayTitle(card)}</span>
      {#if card.status !== "done" && !card.parentId}
        <button
          type="button"
          class="k-doing"
          class:on={card.status === "doing"}
          class:live={isWriting(card)}
          onclick={(e) => toggleDoing(card, e)}
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
      {#if !card.parentId && childrenOf(card.id).length > 0}
        <button
          type="button"
          class="k-subs"
          class:open={openSubtaskIds.includes(card.id)}
          onclick={(e) => toggleOpenSubtasks(card.id, e)}
          onpointerdown={(e) => e.stopPropagation()}
          title={openSubtaskIds.includes(card.id) ? "Hide subtasks" : "Show subtasks"}
        >
          <span class="chev" class:open={openSubtaskIds.includes(card.id)}>▸</span>
          {remainingKids(card.id)}
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
      class:active={doneFilter === "all"}
      aria-pressed={doneFilter === "all"}
      onclick={() => (doneFilter = "all")}>All</button
    >
    <button
      class:active={doneFilter === "mine"}
      aria-pressed={doneFilter === "mine"}
      onclick={() => (doneFilter = "mine")}>I did</button
    >
    <button
      class:active={doneFilter === "behalf"}
      aria-pressed={doneFilter === "behalf"}
      onclick={() => (doneFilter = "behalf")}>On my behalf</button
    >
  </div>

{/snippet}

<section class="k-main" aria-label={listTitle}>

      <div class="mail" bind:this={mailEl}>
        {#if selected === "done"}
          <section class="mail-todo mail-done" aria-label="Done">
            <div
              class="b-zone todo"
              class:drop-ok={dropCol === "done" || grabCol === "done"}
              data-col="done"
              ondragover={(e) => onColDragOver(e, "done")}
              ondragleave={(e) => onColDragLeave(e, "done")}
              ondrop={(e) => onColDrop(e, "done")}
            >
              <header class="b-head">
                <span class="b-name">Done</span>
                <span class="b-count">{viewCols.done.length}</span>
                {@render doneActorChips()}
              </header>
              {#each viewCols.done as card (card.id)}
                {@render stackCard(card)}
              {:else}
                <p class="b-empty">Nothing done yet.</p>
              {/each}
            </div>
          </section>
        {:else}
        <BoardNeeded
          bind:addingNeeded
          bind:neededSelectedId
          bind:neededSplit
          bind:neededEl
          {armNeededMouth}
          {kanbanCards}
          {logEvent}
          {mutate}
          {neededProjectId}
          {neededRows}
          {onNeededAnswer}
          {onSplitDown}
          {projects}
          {menuRing}
        />
        <section class="mail-todo" aria-label="Todos">
          <div
            class="b-zone todo"
            class:drop-ok={dropCol === "todo" || grabCol === "todo"}
            ondragover={(e) => onColDragOver(e, "todo")}
            ondragleave={(e) => onColDragLeave(e, "todo")}
            ondrop={(e) => onColDrop(e, "todo")}
          >
            <header class="b-head">
              <span class="mail-ico" aria-hidden="true">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
                  <rect x="4.5" y="4.5" width="15" height="15" rx="3.5"/>
                  <path d="M8 12.2l2.6 2.6L16.5 9" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              </span>
              <span class="b-name">Todo</span>
              <span class="b-count">{viewCols.doing.length + viewCols.todo.length}</span>
              <button
                type="button"
                class="ka-plus"
                onclick={() => (addingTodo = !addingTodo)}
                title={addingTodo ? "Cancel" : "Add a to-do"}
                aria-label={addingTodo ? "Cancel add to-do" : "Add a to-do"}
              >{addingTodo ? "×" : "+"}</button>
            </header>
            {#if addingTodo}
              <div class="todo-add">
                <input
                  class="d-input"
                  bind:value={newTodoTitle}
                  placeholder="New to-do…"
                  aria-label="New to-do title"
                  onkeydown={(e) => {
                    if (e.key === "Enter") mintTodo(newTodoTitle);
                    if (e.key === "Escape") addingTodo = false;
                  }}
                />
                <button class="mini" onclick={() => mintTodo(newTodoTitle)}>Add</button>
              </div>
            {/if}
            <div class="doing-lane" data-col="doing">
            {#each viewCols.doing as card (card.id)}
              {@render stackCard(card)}
              {#if openSubtaskIds.includes(card.id)}
                {#each childrenOf(card.id) as kid (kid.id)}
                  {@render stackCard(kid)}
                {/each}
              {/if}
            {/each}
            </div>
            <div class="doing-floor" aria-hidden="true"></div>
            <div class="todo-lane" data-col="todo">
            {#each viewCols.todo as card (card.id)}
              {@render stackCard(card)}
              {#if openSubtaskIds.includes(card.id)}
                {#each childrenOf(card.id) as kid (kid.id)}
                  {@render stackCard(kid)}
                {/each}
              {/if}
            {:else}
              {#if viewCols.doing.length === 0}
                <p class="b-empty">No to-dos — drag a card here.</p>
              {/if}
            {/each}
            </div>
          </div>
        </section>
        {/if}
      </div>
    </section>

