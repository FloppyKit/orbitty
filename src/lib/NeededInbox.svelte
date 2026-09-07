<script lang="ts">
  // GO-ORBITTY-CLEAN-KANBAN-WELLS: Needed pile (not a column). Strip under
  // the banner + sidebar mint. Parent owns mutate/logEvent so persist
  // round-trip stays one door.
  import type { KanbanCard, PersistedState } from "./persist";

  let {
    variant,
    cards,
    projectId = "",
    mutate,
    logEvent,
    addingNeeded = $bindable(false),
  }: {
    variant: "strip" | "side";
    cards: KanbanCard[];
    projectId?: string;
    mutate: (fn: (s: PersistedState) => void) => void;
    logEvent: (
      eType: string,
      content: Record<string, unknown>,
      projectId?: string,
    ) => void;
    addingNeeded?: boolean;
  } = $props();

  let newNeeded = $state("");
  let answerFor = $state<string | null>(null);
  let answerDraft = $state("");

  let neededNow = $derived(
    cards
      .filter(
        (c) =>
          c.kind === "needed" &&
          (c.neededState === "open" || c.neededState === "later"),
      )
      .sort((a, b) => a.createdAt - b.createdAt),
  );

  function addNeeded() {
    const title = newNeeded.trim();
    if (!title) return;
    const now = Date.now() / 1000;
    const id = `n-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    mutate((s) => {
      s.kanbanCards = [
        ...s.kanbanCards,
        {
          id,
          title,
          projectId,
          windowId: null,
          kind: "needed" as const,
          neededState: "open" as const,
          status: "todo" as const,
          createdAt: now,
          lastTouchedAt: now,
          completedAt: null,
          goalAligned: false,
          clinicalOrMaster: false,
          pointsAwarded: 0,
          starred: false,
        },
      ];
    });
    logEvent("needed-open", { cardId: id, title }, projectId);
    newNeeded = "";
    addingNeeded = false;
  }

  function answerNeeded(card: KanbanCard) {
    const answer = answerDraft.trim();
    if (!answer) return;
    const now = Date.now() / 1000;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? {
              ...c,
              neededAnswer: answer,
              neededState: "answered" as const,
              lastTouchedAt: now,
            }
          : c,
      );
    });
    logEvent("needed-answer", { cardId: card.id }, card.projectId);
    answerFor = null;
    answerDraft = "";
  }

  function skipNeeded(card: KanbanCard) {
    const now = Date.now() / 1000;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? { ...c, neededState: "skipped" as const, lastTouchedAt: now }
          : c,
      );
    });
    logEvent("needed-skip", { cardId: card.id }, card.projectId);
    if (answerFor === card.id) {
      answerFor = null;
      answerDraft = "";
    }
  }

  function laterNeeded(card: KanbanCard) {
    const now = Date.now() / 1000;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? {
              ...c,
              neededState: "later" as const,
              dueAt: now + 86400,
              lastTouchedAt: now,
            }
          : c,
      );
    });
    logEvent("needed-later", { cardId: card.id }, card.projectId);
    if (answerFor === card.id) {
      answerFor = null;
      answerDraft = "";
    }
  }
</script>

{#if variant === "strip" && neededNow.length > 0}
  <section class="k-needed" aria-label="Needed from you">
    <div class="kn-head">
      <span class="kn-title">Needed from you</span>
      <button
        class="ka-plus"
        onclick={() => (addingNeeded = true)}
        title="Add a Needed question"
        aria-label="Add a Needed question">+</button
      >
    </div>
    <ul class="kn-rows">
      {#each neededNow as card (card.id)}
        <li class="kn-row">
          <div class="kn-main">
            <span class="kn-q">{card.title}</span>
            {#if card.neededWhy}
              <span class="kn-why">{card.neededWhy}</span>
            {/if}
            {#if answerFor === card.id}
              <div class="kn-answer">
                <input
                  bind:value={answerDraft}
                  placeholder="Your answer…"
                  aria-label="Your answer"
                  onkeydown={(e) => {
                    if (e.key === "Enter") answerNeeded(card);
                    if (e.key === "Escape") {
                      answerFor = null;
                      answerDraft = "";
                    }
                  }}
                />
                <button class="mini" onclick={() => answerNeeded(card)}>Save</button>
              </div>
            {/if}
          </div>
          <div class="kn-acts">
            <button
              class="mini"
              class:active={answerFor === card.id}
              onclick={() => {
                answerFor = answerFor === card.id ? null : card.id;
                answerDraft = "";
              }}>Answer</button
            >
            <button class="mini ghost" onclick={() => skipNeeded(card)}>Skip</button>
            <button
              class="mini ghost"
              onclick={() => laterNeeded(card)}
              title="Ask again tomorrow">Later</button
            >
          </div>
        </li>
      {/each}
    </ul>
  </section>
{/if}

{#if variant === "side"}
  <section class="k-needed-side" aria-label="Needed">
    <header class="ka-head">
      <span class="ka-title">Needed</span>
      <button
        class="ka-plus"
        onclick={() => (addingNeeded = !addingNeeded)}
        title={addingNeeded ? "Cancel" : "Add a Needed question"}
        aria-label={addingNeeded ? "Cancel add needed" : "Add a Needed question"}
        aria-expanded={addingNeeded}>{addingNeeded ? "×" : "+"}</button
      >
    </header>
    {#if addingNeeded}
      <div class="ka-form">
        <input
          bind:value={newNeeded}
          placeholder="Needed from you…"
          aria-label="Needed question"
          onkeydown={(e) => e.key === "Enter" && addNeeded()}
        />
        <button class="mini" onclick={addNeeded}>Save</button>
      </div>
    {/if}
  </section>
{/if}

<style>
  .k-needed {
    flex-shrink: 0;
    padding: 8px 14px 10px;
    border-bottom: 1px solid var(--edge);
    background: var(--panel-fill, var(--panel));
  }

  .kn-head {
    display: flex;
    align-items: center;
    padding: 0 0 4px;
  }

  .kn-title {
    flex: 1;
    font-size: 0.66rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: #e59e3d;
  }

  .kn-rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .kn-row {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 5px 8px;
    border: 1px solid var(--edge);
    border-radius: 8px;
    background: var(--bg-fill, var(--bg));
  }

  .kn-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .kn-q {
    font-size: 0.8rem;
    line-height: 1.35;
    color: var(--ink);
    overflow-wrap: anywhere;
  }

  .kn-why {
    font-size: 0.68rem;
    line-height: 1.35;
    color: var(--dim);
    overflow-wrap: anywhere;
  }

  .kn-answer {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
  }

  .kn-answer input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 0.76rem;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 4px 8px;
  }

  .kn-answer input:focus {
    outline: none;
    border-color: var(--focus, #6e4db0);
  }

  .kn-acts {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .ka-head {
    display: flex;
    align-items: center;
    padding: 0 4px 0 10px;
  }

  .ka-title {
    flex: 1;
    font-size: 0.66rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--dim);
  }

  .ka-plus {
    font: inherit;
    font-size: 0.82rem;
    line-height: 1;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 5px;
    padding: 1px 6px;
    cursor: pointer;
  }

  .ka-plus:hover {
    color: var(--accent);
    background: var(--bg-fill, var(--bg));
  }

  .ka-form {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 2px 0;
  }

  .ka-form input {
    width: 100%;
    box-sizing: border-box;
    font: inherit;
    font-size: 0.76rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 5px 8px;
  }

  .ka-form input:focus {
    outline: none;
    border-color: var(--focus, #6e4db0);
  }

  .mini {
    font: inherit;
    font-size: 0.64rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    padding: 3px 8px;
    cursor: pointer;
  }

  .mini:hover {
    border-color: var(--focus, #6e4db0);
  }

  .mini.active {
    border-color: var(--focus, #6e4db0);
    color: var(--accent);
    font-weight: 700;
  }

  .mini.ghost {
    color: var(--dim);
  }
</style>
