<script lang="ts">
  import type { BoardApi } from "./api";
  import type { Tracker } from "../persist";
  let { board }: { board: BoardApi } = $props();
</script>

{#snippet trackerWidget(projectId: string, tr: Tracker, showChip: boolean)}
  <div class="k-tracktile" class:armed={board.trackerArmed} role="group" aria-label={tr.label}>
    {#if board.trackerArmed}
      <button
        type="button"
        class="k-track-x"
        onclick={() => board.removeTracker(projectId, tr.id)}
        aria-label="Remove {tr.label}"
        title="Remove tracker"
      >×</button>
    {/if}
    <div class="k-track-name">
      {#if showChip}
        <span
          class="planet-chip"
          style="background: {board.planetCss(projectId)}"
          title={board.projectName(projectId)}
          aria-hidden="true"
        ></span>
      {/if}
      <span class="k-track-lbl">{board.clipTrackLabel(tr.label)}</span>
    </div>
    <div class="k-track-row">
      <button
        type="button"
        class="k-track-nudge"
        tabindex={board.trackerArmed ? -1 : 0}
        onclick={() => board.bumpTracker(projectId, tr.id, -1)}
        aria-label="Decrease {tr.label}"
      >‹</button>
      <input
        class="k-track-val"
        value={tr.value}
        inputmode="numeric"
        aria-label={tr.label}
        readonly={board.trackerArmed}
        onmousedown={(e) => e.stopPropagation()}
        onblur={(e) => board.setTrackerValue(projectId, tr.id, e.currentTarget.value)}
        onkeydown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
          e.stopPropagation();
        }}
      />
      <button
        type="button"
        class="k-track-nudge"
        tabindex={board.trackerArmed ? -1 : 0}
        onclick={() => board.bumpTracker(projectId, tr.id, 1)}
        aria-label="Increase {tr.label}"
      >›</button>
    </div>
  </div>

{/snippet}

    <div
      class="mail-east-split"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize preview"
      title="Drag to resize preview · double-click resets"
      onpointerdown={board.onEastSplitDown}
      ondblclick={board.resetPreviewWidth}
    ></div>
    <div class="k-east" bind:this={board.eastEl} style="width: {board.previewWidth}px">
    {#if board.showTrackers}
      <section
        class="k-trackpane"
        class:armed={board.trackerArmed}
        class:empty={board.visibleTrackers.length === 0 && !board.addingTracker}
        bind:this={board.trackPaneEl}
        style="flex-basis: {board.visibleTrackers.length === 0 && !board.addingTracker
          ? 'auto'
          : `${Math.round(board.trackerSplit * 100)}%`}"
        aria-label="Trackers"
      >
        <header class="k-trackpane-head">
          <span class="k-trackpane-title">Trackers</span>
          <button
            type="button"
            class="ka-plus"
            onclick={() => {
              board.addingTracker = !board.addingTracker;
              board.newTrackerLabel = "";
              board.newTrackerPlanetId = board.selectedProject?.id ?? board.projects[0]?.id ?? "";
              board.trackerArmed = false;
            }}
            title={board.addingTracker ? "Cancel" : "Add a tracker"}
            aria-label={board.addingTracker ? "Cancel add tracker" : "Add a tracker"}
          >{board.addingTracker ? "×" : "+"}</button>
        </header>
        {#if board.addingTracker}
          <div class="k-track-addrow">
            <select
              class="k-track-planet"
              bind:value={board.newTrackerPlanetId}
              aria-label="Planet for this tracker"
            >
              {#each board.projects as p (p.id)}
                <option value={p.id}>{p.name}</option>
              {/each}
            </select>
            <input
              class="k-track-add-field"
              bind:value={board.newTrackerLabel}
              maxlength="7"
              placeholder="Name (7)…"
              aria-label="New tracker name"
              autofocus
              onkeydown={(e) => {
                if (e.key === "Enter") board.addTracker(board.newTrackerPlanetId, board.newTrackerLabel);
                if (e.key === "Escape") {
                  board.addingTracker = false;
                  board.newTrackerLabel = "";
                }
              }}
            />
          </div>
        {/if}
        {#if board.visibleTrackers.length}
          <div class="k-trackgrid">
            {#each board.visibleTrackers as row (row.tr.id)}
              {@render trackerWidget(row.projectId, row.tr, row.chip)}
            {/each}
          </div>
        {:else if !board.addingTracker}
          <p class="d-hint">{board.selected === "inbox" ? "No planet trackers yet." : "No trackers on this planet."}</p>
        {/if}
        <button
          type="button"
          class="k-track-trash"
          class:on={board.trackerArmed}
          onclick={() => (board.trackerArmed = !board.trackerArmed)}
          aria-label={board.trackerArmed ? "Done removing trackers" : "Remove trackers"}
          title={board.trackerArmed ? "Done" : "Remove trackers"}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
            <path d="M4 7h16" stroke-linecap="round"/>
            <path d="M9 7V5h6v2"/>
            <path d="M6 7l1 13h10l1-13"/>
            <path d="M10 11v6M14 11v6" stroke-linecap="round"/>
          </svg>
        </button>
      </section>
      {#if board.visibleTrackers.length > 0 || board.addingTracker}
        <div
          class="k-east-split"
          role="separator"
          aria-orientation="horizontal"
          aria-label="Resize trackers"
          title="Drag to resize Trackers · double-click resets"
          onpointerdown={board.onTrackSplitDown}
          ondblclick={board.resetTrackerSplit}
        ></div>
      {/if}
    {/if}
    <aside class="k-detail" aria-label="Item details">
      <div class="k-preview-head">Preview:</div>
      {#if board.detailCard}
      {@const card = board.detailCard}
        {#if card.kind === "needed"}
          {@const npid = board.livePlanetId(card)}
          <p class="d-need-title">{card.title}</p>
          <label class="d-label">Planet</label>
          <div class="planet-dd">
            <button
              type="button"
              class="d-input planet-dd-btn"
              onclick={() => (board.planetDdOpen = !board.planetDdOpen)}
            >
              {#if npid}
                <span class="planet-chip" style="background: {board.planetCss(npid)}"></span>
                {board.projectName(npid)}
              {:else}
                Inbox
              {/if}
            </button>
            {#if board.planetDdOpen}
              <div class="planet-dd-list" role="listbox">
                <button
                  type="button"
                  class="planet-dd-opt"
                  onclick={() => {
                    board.setPlanet(card, "");
                    board.planetDdOpen = false;
                  }}
                >Inbox</button>
                {#each board.projects as p (p.id)}
                  <button
                    type="button"
                    class="planet-dd-opt"
                    onclick={() => {
                      board.setPlanet(card, p.id);
                      board.planetDdOpen = false;
                    }}
                  >
                    <span class="planet-chip" style="background: {board.planetCss(p.id)}"></span>
                    {p.name}
                  </button>
                {/each}
              </div>
            {/if}
          </div>
          {#if card.neededWhy}
            <div class="kn-why md-lite">{@html board.mdLite(card.neededWhy)}</div>
          {/if}
        {:else}
        {#if !board.previewEdit}
          {@const pid = board.livePlanetId(card)}
          {@const due = board.dueInfo(card)}
          <div class="d-read-top">
            <h3 class="d-need-title">{board.displayTitle(card)}</h3>
            <button type="button" class="mini" onclick={() => (board.previewEdit = true)}>Edit</button>
          </div>
          {#if pid}
            <p class="d-read-line">
              <span class="planet-chip" style="background: {board.planetCss(pid)}"></span>
              {board.projectName(pid)}
            </p>
          {:else}
            <p class="d-read-line dim">Inbox</p>
          {/if}
          {#if board.agentName(card)}
            <p class="d-read-line">{board.agentName(card)}</p>
          {/if}
          {#if card.rating}
            <p class="d-read-line">Rating {card.rating}</p>
          {/if}
          {#if card.notes}
            <div class="d-read-notes md-lite">{@html board.mdLite(card.notes)}</div>
          {/if}
          {#if due}
            <p class="d-read-line" class:overdue={due.overdue}>{due.label}</p>
          {/if}
          {#if board.childrenOf(card.id).length}
            <ul class="d-read-subs">
              {#each board.childrenOf(card.id) as kid (kid.id)}
                <li class:strike={kid.status === "done"}>{kid.title}</li>
              {/each}
            </ul>
          {/if}
          {#if (card.tagIds ?? []).length}
            <div class="d-tags">
              {#each board.tags.filter((t) => board.cardHasTag(card, t.id)) as t (t.id)}
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
                class:mkey-sel={board.menuRing.activeId === "d-trash"}
                data-mkey="d-trash"
                onclick={() => board.deleteTodo(card)}
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
            <button type="button" class="mini ghost" onclick={() => (board.previewEdit = false)}>Done</button>
          </div>
        </div>
        <label class="d-label" for="d-name">Title</label>
        <input
          id="d-name"
          class="d-input"
          class:mkey-sel={board.menuRing.activeId === "d-title"}
          data-mkey="d-title"
          value={card.title}
          onchange={(e) => board.saveTitle(card, e.currentTarget.value)}
          required
        />

        <label class="d-label" for="d-planet">Planet</label>
        <select
          id="d-planet"
          class="d-input"
          class:mkey-sel={board.menuRing.activeId === "d-planet"}
          data-mkey="d-planet"
          value={board.livePlanetId(card) ?? ""}
          onchange={(e) => board.setPlanet(card, e.currentTarget.value)}
        >
          <option value="">Inbox</option>
          {#each board.projects as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>

        <label class="d-label" for="d-agent">Agent</label>
        <select
          id="d-agent"
          class="d-input"
          class:mkey-sel={board.menuRing.activeId === "d-agent"}
          data-mkey="d-agent"
          value={card.assignedAgentId ?? ""}
          onchange={(e) => board.setAgent(card, e.currentTarget.value)}
        >
          <option value="">None</option>
          {#each board.agentShortcuts as a (a.id)}
            <option value={a.id}>{a.name}</option>
          {/each}
        </select>

        {#if card.taskId}
          <button
            type="button"
            class="mini"
            onclick={() => board.undoThisTask(card)}
            title="Restore this planet's workspace root to the snapshot taken when the task started"
          >Undo this task</button>
        {/if}

        <label class="d-label" for="d-notes">Notes</label>
        <textarea
          id="d-notes"
          class="d-input d-textarea d-notes-lg"
          class:mkey-sel={board.menuRing.activeId === "d-notes"}
          data-mkey="d-notes"
          rows="10"
          placeholder="Add notes…"
          value={card.notes ?? ""}
          onchange={(e) => board.saveNotes(card, e.currentTarget.value)}
        ></textarea>

        <span class="d-label">Rating</span>
        <div class="d-diff" role="group" aria-label="Rating 1 to 7">
          {#each [1, 2, 3, 4, 5, 6, 7] as n}
            <button
              type="button"
              class="mini"
              class:active={card.rating === n}
              class:mkey-sel={board.menuRing.activeId === `d-rate-${n}`}
              data-mkey={`d-rate-${n}`}
              aria-pressed={card.rating === n}
              onclick={() => board.setRating(card, card.rating === n ? null : n)}
            >{n}</button>
          {/each}
        </div>

        <label class="d-label" for="d-due">Due date</label>
        <div class="d-duerow">
          <input
            id="d-due"
            class="d-input"
            class:mkey-sel={board.menuRing.activeId === "d-due"}
            data-mkey="d-due"
            type="date"
            value={board.dateInputValue(card)}
            onchange={(e) => board.setDue(card, e.currentTarget.value)}
          />
          {#if card.dueAt}
            <button
              class="mini ghost"
              class:mkey-sel={board.menuRing.activeId === "d-due-clear"}
              data-mkey="d-due-clear"
              onclick={() => board.setDue(card, "")}>Clear</button
            >
          {/if}
        </div>

        <span class="d-label">Subtasks</span>
        <ul class="d-subs">
          {#each board.childrenOf(card.id) as kid (kid.id)}
            <li class="d-sub">
              <button
                class="k-check"
                class:checked={kid.status === "done"}
                onclick={() => board.moveCard(kid, kid.status === "done" ? "todo" : "done")}
                aria-label={kid.status === "done" ? "Reopen subtask" : "Complete subtask"}
              >{kid.status === "done" ? "✓" : ""}</button>
              <button
                type="button"
                class="d-subtitle"
                class:strike={kid.status === "done"}
                onclick={() => board.openDetail(kid)}
              >{kid.title}</button>
              <button
                class="d-del"
                onclick={() => board.detachChild(kid.id)}
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
                class:mkey-sel={board.menuRing.activeId === `d-sub-check-${sub.id}`}
                data-mkey={`d-sub-check-${sub.id}`}
                onclick={() => board.toggleSubtask(card, sub.id)}
                aria-label={sub.done ? "Reopen subtask" : "Complete subtask"}
                title={sub.done ? "Reopen" : "Complete"}>{sub.done ? "✓" : ""}</button
              >
              <span class="d-subtitle" class:strike={sub.done}>{sub.title}</span>
              <button
                class="d-del"
                class:mkey-sel={board.menuRing.activeId === `d-sub-del-${sub.id}`}
                data-mkey={`d-sub-del-${sub.id}`}
                onclick={() => board.deleteSubtask(card, sub.id)}
                aria-label="Delete subtask"
                title="Delete">×</button
              >
            </li>
          {/each}
          {#if board.childrenOf(card.id).length === 0 && !(card.subtasks ?? []).length}
            <li class="d-hint">No subtasks yet — tag a to-do below.</li>
          {/if}
        </ul>
        {#if board.linkableTodos(card).length > 0}
          <select
            class="d-input"
            aria-label="Tag an existing to-do as a subtask"
            onchange={(e) => {
              board.attachChild(card, e.currentTarget.value);
              e.currentTarget.value = "";
            }}
          >
            <option value="">Tag a to-do as subtask…</option>
            {#each board.linkableTodos(card) as t (t.id)}
              <option value={t.id}>{t.title}</option>
            {/each}
          </select>
        {/if}

        <span class="d-label">Tags</span>
        {#if board.tags.length > 0}
          <div class="d-tags">
            {#each board.tags as t (t.id)}
              <button
                class="mini"
                class:active={board.cardHasTag(card, t.id)}
                class:mkey-sel={board.menuRing.activeId === `d-tag-${t.id}`}
                data-mkey={`d-tag-${t.id}`}
                aria-pressed={board.cardHasTag(card, t.id)}
                onclick={() => board.toggleCardTag(card, t.id)}>{t.name}</button
              >
            {/each}
          </div>
        {/if}
        <div class="d-subadd">
          <input
            class="d-input"
            class:mkey-sel={board.menuRing.activeId === "d-tag-new"}
            data-mkey="d-tag-new"
            bind:value={board.newTagName}
            placeholder="New tag"
            onkeydown={(e) => e.key === "Enter" && board.addTagToCard(card, board.newTagName)}
          />
          <button
            class="mini"
            onclick={() => board.addTagToCard(card, board.newTagName)}>Add</button
          >
        </div>

        {#if card.kind === "window" || card.windowId}
          {@const w = board.winFor(card)}
          <span class="d-label">Linked window</span>
          <div class="d-win">
            {#if w?.iconBase64}
              <img class="k-icon" src="data:image/png;base64,{w.iconBase64}" alt="" />
            {/if}
            <span class="d-winname" title={board.displayTitle(card)}>
              {card.windowId ? board.displayTitle(card) : "Not linked"}
            </span>
            {#if w}
              <button
                class="mini"
                class:mkey-sel={board.menuRing.activeId === "d-win-focus"}
                data-mkey="d-win-focus"
                onclick={() => board.focusCard(card)}>Focus</button
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
