<script lang="ts">
  import type { KanbanCard, Project } from "../persist";
  let {
    addTagToCard,
    agentName,
    agentShortcuts,
    attachChild,
    cardHasTag,
    childrenOf,
    dateInputValue,
    deleteSubtask,
    deleteTodo,
    detachChild,
    detailCard,
    displayTitle,
    dueInfo,
    focusCard,
    linkableTodos,
    livePlanetId,
    mdLite,
    menuRing,
    moveCard,
    newTagName = $bindable(),
    onEastSplitDown,
    openDetail,
    planetCss,
    planetDdOpen = $bindable(),
    previewEdit = $bindable(),
    previewWidth = $bindable(),
    projectName,
    projects,
    resetPreviewWidth,
    saveNotes,
    saveTitle,
    selected = $bindable(),
    setAgent,
    setDue,
    setPlanet,
    setRating,
    tags,
    toggleCardTag,
    toggleSubtask,
    undoThisTask,
    winFor,
  } = $props();
</script>

    <div
      class="mail-east-split"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize preview"
      title="Drag to resize preview · double-click resets"
      onpointerdown={onEastSplitDown}
      ondblclick={resetPreviewWidth}
    ></div>
    <div class="k-east" style="width: {previewWidth}px">
    <aside class="k-detail" aria-label="Item details">
      <div class="k-preview-head">Preview:</div>
      {#if detailCard}
      {@const card = detailCard}
        {#if card.kind === "needed"}
          {@const npid = livePlanetId(card)}
          <p class="d-need-title">{card.title}</p>
          <label class="d-label">Planet</label>
          <div class="planet-dd">
            <button
              type="button"
              class="d-input planet-dd-btn"
              onclick={() => (planetDdOpen = !planetDdOpen)}
            >
              {#if npid}
                <span class="planet-chip" style="background: {planetCss(npid)}"></span>
                {projectName(npid)}
              {:else}
                Inbox
              {/if}
            </button>
            {#if planetDdOpen}
              <div class="planet-dd-list" role="listbox">
                <button
                  type="button"
                  class="planet-dd-opt"
                  onclick={() => {
                    setPlanet(card, "");
                    planetDdOpen = false;
                  }}
                >Inbox</button>
                {#each projects as p (p.id)}
                  <button
                    type="button"
                    class="planet-dd-opt"
                    onclick={() => {
                      setPlanet(card, p.id);
                      planetDdOpen = false;
                    }}
                  >
                    <span class="planet-chip" style="background: {planetCss(p.id)}"></span>
                    {p.name}
                  </button>
                {/each}
              </div>
            {/if}
          </div>
          {#if card.neededWhy}
            <div class="kn-why md-lite">{@html mdLite(card.neededWhy)}</div>
          {/if}
        {:else}
        {#if !previewEdit}
          {@const pid = livePlanetId(card)}
          {@const due = dueInfo(card)}
          <div class="d-read-top">
            <h3 class="d-need-title">{displayTitle(card)}</h3>
            <button type="button" class="mini" onclick={() => (previewEdit = true)}>Edit</button>
          </div>
          {#if pid}
            <p class="d-read-line">
              <span class="planet-chip" style="background: {planetCss(pid)}"></span>
              {projectName(pid)}
            </p>
          {:else}
            <p class="d-read-line dim">Inbox</p>
          {/if}
          {#if agentName(card)}
            <p class="d-read-line">{agentName(card)}</p>
          {/if}
          {#if card.rating}
            <p class="d-read-line">Rating {card.rating}</p>
          {/if}
          {#if card.notes}
            <div class="d-read-notes md-lite">{@html mdLite(card.notes)}</div>
          {/if}
          {#if due}
            <p class="d-read-line" class:overdue={due.overdue}>{due.label}</p>
          {/if}
          {#if childrenOf(card.id).length}
            <ul class="d-read-subs">
              {#each childrenOf(card.id) as kid (kid.id)}
                <li class:strike={kid.status === "done"}>{kid.title}</li>
              {/each}
            </ul>
          {/if}
          {#if (card.tagIds ?? []).length}
            <div class="d-tags">
              {#each tags.filter((t) => cardHasTag(card, t.id)) as t (t.id)}
                <span class="mini active">{t.name}</span>
              {/each}
            </div>
          {/if}
        {:else}
        <div class="d-read-top">
          <span class="d-label" style="margin:0">Edit</span>
          <div class="d-edit-acts">
            {#if card.kind !== "window" && card.kind !== "needed"}
              <button
                type="button"
                class="d-trash"
                class:mkey-sel={menuRing.activeId === "d-trash"}
                data-mkey="d-trash"
                onclick={() => deleteTodo(card)}
                aria-label="Delete to-do"
                title="Delete this to-do — does not mark it done"
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
                  <path d="M4 7h16" stroke-linecap="round"/>
                  <path d="M9 7V5h6v2"/>
                  <path d="M6 7l1 13h10l1-13"/>
                  <path d="M10 11v6M14 11v6" stroke-linecap="round"/>
                </svg>
              </button>
            {/if}
            <button type="button" class="mini ghost" onclick={() => (previewEdit = false)}>Done</button>
          </div>
        </div>
        <label class="d-label" for="d-name">Title</label>
        <input
          id="d-name"
          class="d-input"
          class:mkey-sel={menuRing.activeId === "d-title"}
          data-mkey="d-title"
          value={card.title}
          onchange={(e) => saveTitle(card, e.currentTarget.value)}
          required
        />

        <label class="d-label" for="d-planet">Planet</label>
        <select
          id="d-planet"
          class="d-input"
          class:mkey-sel={menuRing.activeId === "d-planet"}
          data-mkey="d-planet"
          value={livePlanetId(card) ?? ""}
          onchange={(e) => setPlanet(card, e.currentTarget.value)}
        >
          <option value="">Inbox</option>
          {#each projects as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>

        <label class="d-label" for="d-agent">Agent</label>
        <select
          id="d-agent"
          class="d-input"
          class:mkey-sel={menuRing.activeId === "d-agent"}
          data-mkey="d-agent"
          value={card.assignedAgentId ?? ""}
          onchange={(e) => setAgent(card, e.currentTarget.value)}
        >
          <option value="">None</option>
          {#each agentShortcuts as a (a.id)}
            <option value={a.id}>{a.name}</option>
          {/each}
        </select>

        {#if card.taskId}
          <button
            type="button"
            class="mini"
            onclick={() => undoThisTask(card)}
            title="Restore this planet's workspace root to the snapshot taken when the task started"
          >Undo this task</button>
        {/if}

        <label class="d-label" for="d-notes">Notes</label>
        <textarea
          id="d-notes"
          class="d-input d-textarea d-notes-lg"
          class:mkey-sel={menuRing.activeId === "d-notes"}
          data-mkey="d-notes"
          rows="10"
          placeholder="Add notes…"
          value={card.notes ?? ""}
          onchange={(e) => saveNotes(card, e.currentTarget.value)}
        ></textarea>

        <span class="d-label">Rating</span>
        <div class="d-diff" role="group" aria-label="Rating 1 to 7">
          {#each [1, 2, 3, 4, 5, 6, 7] as n}
            <button
              type="button"
              class="mini"
              class:active={card.rating === n}
              class:mkey-sel={menuRing.activeId === `d-rate-${n}`}
              data-mkey={`d-rate-${n}`}
              aria-pressed={card.rating === n}
              onclick={() => setRating(card, card.rating === n ? null : n)}
            >{n}</button>
          {/each}
        </div>

        <label class="d-label" for="d-due">Due date</label>
        <div class="d-duerow">
          <input
            id="d-due"
            class="d-input"
            class:mkey-sel={menuRing.activeId === "d-due"}
            data-mkey="d-due"
            type="date"
            value={dateInputValue(card)}
            onchange={(e) => setDue(card, e.currentTarget.value)}
          />
          {#if card.dueAt}
            <button
              class="mini ghost"
              class:mkey-sel={menuRing.activeId === "d-due-clear"}
              data-mkey="d-due-clear"
              onclick={() => setDue(card, "")}>Clear</button
            >
          {/if}
        </div>

        <span class="d-label">Subtasks</span>
        <ul class="d-subs">
          {#each childrenOf(card.id) as kid (kid.id)}
            <li class="d-sub">
              <button
                class="k-check"
                class:checked={kid.status === "done"}
                onclick={() => moveCard(kid, kid.status === "done" ? "todo" : "done")}
                aria-label={kid.status === "done" ? "Reopen subtask" : "Complete subtask"}
              >{kid.status === "done" ? "✓" : ""}</button>
              <button
                type="button"
                class="d-subtitle"
                class:strike={kid.status === "done"}
                onclick={() => openDetail(kid)}
              >{kid.title}</button>
              <button
                class="d-del"
                onclick={() => detachChild(kid.id)}
                aria-label="Unlink subtask"
                title="Unlink — stays a to-do">×</button
              >
            </li>
          {/each}
          {#each card.subtasks ?? [] as sub (sub.id)}
            <li class="d-sub">
              <button
                class="k-check"
                class:checked={sub.done}
                class:mkey-sel={menuRing.activeId === `d-sub-check-${sub.id}`}
                data-mkey={`d-sub-check-${sub.id}`}
                onclick={() => toggleSubtask(card, sub.id)}
                aria-label={sub.done ? "Reopen subtask" : "Complete subtask"}
                title={sub.done ? "Reopen" : "Complete"}>{sub.done ? "✓" : ""}</button
              >
              <span class="d-subtitle" class:strike={sub.done}>{sub.title}</span>
              <button
                class="d-del"
                class:mkey-sel={menuRing.activeId === `d-sub-del-${sub.id}`}
                data-mkey={`d-sub-del-${sub.id}`}
                onclick={() => deleteSubtask(card, sub.id)}
                aria-label="Delete subtask"
                title="Delete">×</button
              >
            </li>
          {/each}
          {#if childrenOf(card.id).length === 0 && !(card.subtasks ?? []).length}
            <li class="d-hint">No subtasks yet — tag a to-do below.</li>
          {/if}
        </ul>
        {#if linkableTodos(card).length > 0}
          <select
            class="d-input"
            aria-label="Tag an existing to-do as a subtask"
            onchange={(e) => {
              attachChild(card, e.currentTarget.value);
              e.currentTarget.value = "";
            }}
          >
            <option value="">Tag a to-do as subtask…</option>
            {#each linkableTodos(card) as t (t.id)}
              <option value={t.id}>{t.title}</option>
            {/each}
          </select>
        {/if}

        <span class="d-label">Tags</span>
        {#if tags.length > 0}
          <div class="d-tags">
            {#each tags as t (t.id)}
              <button
                class="mini"
                class:active={cardHasTag(card, t.id)}
                class:mkey-sel={menuRing.activeId === `d-tag-${t.id}`}
                data-mkey={`d-tag-${t.id}`}
                aria-pressed={cardHasTag(card, t.id)}
                onclick={() => toggleCardTag(card, t.id)}>{t.name}</button
              >
            {/each}
          </div>
        {/if}
        <div class="d-subadd">
          <input
            class="d-input"
            class:mkey-sel={menuRing.activeId === "d-tag-new"}
            data-mkey="d-tag-new"
            bind:value={newTagName}
            placeholder="New tag"
            onkeydown={(e) => e.key === "Enter" && addTagToCard(card, newTagName)}
          />
          <button
            class="mini"
            onclick={() => addTagToCard(card, newTagName)}>Add</button
          >
        </div>

        {#if card.kind === "window" || card.windowId}
          {@const w = winFor(card)}
          <span class="d-label">Linked window</span>
          <div class="d-win">
            {#if w?.iconBase64}
              <img class="k-icon" src="data:image/png;base64,{w.iconBase64}" alt="" />
            {/if}
            <span class="d-winname" title={displayTitle(card)}>
              {card.windowId ? displayTitle(card) : "Not linked"}
            </span>
            {#if w}
              <button
                class="mini"
                class:mkey-sel={menuRing.activeId === "d-win-focus"}
                data-mkey="d-win-focus"
                onclick={() => focusCard(card)}>Focus</button
              >
            {/if}
          </div>
          <p class="d-hint">Window links are made by claiming in the rail — read-only here.</p>
        {/if}
        {/if}
        {/if}
      {:else}
        <p class="d-hint">Highlight a row to preview it here.</p>
      {/if}
    </aside>
    </div>
