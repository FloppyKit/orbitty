<script lang="ts">
  // Needed pile. Launchpad strip is an accordion: one header with a
  // count, open to a standard card (question / why / Answer·Later·Skip).
  // Why is mdLite so **bold**, `paths`, and lists read as prose, not
  // raw markdown. Parent owns mutate/logEvent so persist stays one door.
  import { mdLite } from "./mdLite";
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
  let open = $state(false);

  $effect(() => {
    if (addingNeeded) open = true;
  });

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

{#if variant === "strip" && (neededNow.length > 0 || addingNeeded)}
  <section class="k-needed" aria-label="Needed from you">
    <div class="kn-head">
      <button
        type="button"
        class="kn-toggle"
        aria-expanded={open}
        onclick={() => (open = !open)}
      >
        <span class="kn-chev" class:open aria-hidden="true">▸</span>
        <span class="kn-title">Needed</span>
        {#if neededNow.length > 0}
          <span class="kn-count">{neededNow.length}</span>
        {/if}
      </button>
      <button
        class="ka-plus"
        onclick={() => {
          addingNeeded = true;
          open = true;
        }}
        title="Add a Needed question"
        aria-label="Add a Needed question">+</button
      >
    </div>
    {#if open}
      {#if addingNeeded}
        <div class="ka-form">
          <input
            bind:value={newNeeded}
            placeholder="Needed from you…"
            aria-label="Needed question"
            onkeydown={(e) => {
              if (e.key === "Enter") addNeeded();
              if (e.key === "Escape") addingNeeded = false;
            }}
          />
          <button class="mini" onclick={addNeeded}>Save</button>
          <button class="mini ghost" onclick={() => (addingNeeded = false)}>Cancel</button>
        </div>
      {/if}
      <ul class="kn-rows">
        {#each neededNow as card (card.id)}
          <li class="kn-row">
            <p class="kn-q">{card.title}</p>
            {#if card.neededWhy}
              <div class="kn-why">{@html mdLite(card.neededWhy)}</div>
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
            <div class="kn-acts">
              <button
                class="mini"
                class:active={answerFor === card.id}
                onclick={() => {
                  answerFor = answerFor === card.id ? null : card.id;
                  answerDraft = "";
                }}>Answer</button
              >
              <button
                class="mini ghost"
                onclick={() => laterNeeded(card)}
                title="Ask again tomorrow">Later</button
              >
              <button class="mini ghost" onclick={() => skipNeeded(card)}>Skip</button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
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
    padding: 4px 12px 8px;
    border-bottom: 1px solid var(--edge);
  }

  .kn-head {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .kn-toggle {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 6px 4px;
    font: inherit;
    color: inherit;
    background: transparent;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    text-align: left;
  }

  .kn-toggle:hover {
    background: var(--panel-fill, var(--panel));
  }

  .kn-chev {
    flex-shrink: 0;
    font-size: 0.62rem;
    color: var(--dim);
    transition: transform 0.12s;
  }

  .kn-chev.open {
    transform: rotate(90deg);
  }

  .kn-title {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--ink);
  }

  .kn-count {
    font-size: 0.62rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    color: var(--bg);
    background: #e59e3d;
    border-radius: 999px;
    padding: 0 0.45rem;
    line-height: 1.5;
  }

  .kn-rows {
    list-style: none;
    margin: 0;
    padding: 4px 0 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-height: 42vh;
    overflow-y: auto;
  }

  .kn-row {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px 10px;
    border: 1px solid var(--edge);
    border-radius: 10px;
    background: var(--panel-fill, var(--panel));
  }

  .kn-q {
    margin: 0;
    font-size: 0.92rem;
    font-weight: 600;
    line-height: 1.4;
    color: var(--ink);
    overflow-wrap: anywhere;
  }

  .kn-why {
    font-size: 0.82rem;
    line-height: 1.55;
    color: var(--ink);
    overflow-wrap: anywhere;
  }

  .kn-why :global(strong) {
    font-weight: 700;
  }

  .kn-why :global(em) {
    font-style: italic;
  }

  .kn-why :global(code) {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 0.78em;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 5px;
    padding: 0.05em 0.35em;
  }

  .kn-why :global(p),
  .kn-why :global(ul),
  .kn-why :global(ol) {
    margin: 0 0 0.55em;
  }

  .kn-why :global(p:last-child),
  .kn-why :global(ul:last-child),
  .kn-why :global(ol:last-child) {
    margin-bottom: 0;
  }

  .kn-why :global(li) {
    margin: 0 0 0.2em 1.1em;
  }

  .kn-why :global(br) {
    display: block;
    content: "";
    margin-top: 0.45em;
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
    display: flex;
    align-items: center;
    gap: 6px;
    padding-top: 2px;
  }

  .ka-form {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 4px 8px;
  }

  .ka-form input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 0.82rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 8px;
    padding: 6px 10px;
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
