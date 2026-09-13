<script module lang="ts">
  import type { KanbanCard } from "./persist";

  // GO-ORBITTY-MENU-KEYS: one Needed ordering, two readers — the strip
  // paints it, the Launchpad ring (KanbanPage) walks it.
  export function neededSorted(cards: KanbanCard[]): KanbanCard[] {
    return cards
      .filter(
        (c) =>
          c.kind === "needed" &&
          (c.neededState === "open" || c.neededState === "later"),
      )
      .sort((a, b) => {
        const al = a.neededState === "later" ? 1 : 0;
        const bl = b.neededState === "later" ? 1 : 0;
        if (al !== bl) return al - bl;
        if (al === 1) return (a.lastTouchedAt ?? 0) - (b.lastTouchedAt ?? 0);
        return a.createdAt - b.createdAt;
      });
  }
</script>

<script lang="ts">
  // Needed pile. Launchpad strip is an accordion: one header with a
  // count, open to a standard card (question / why / Answer·Later·Skip).
  // Why is mdLite so **bold**, `paths`, and lists read as prose, not
  // raw markdown. Parent owns mutate/logEvent so persist stays one door.
  // GO-ORBITTY-MENU-KEYS: open/selectedId are bindable so the Launchpad
  // ring can walk the pile; ringActiveId paints the one focus ring.
  import { mdLite } from "./mdLite";
  import type { PersistedState } from "./persist";

  let {
    variant,
    cards,
    projectId = "",
    mutate,
    logEvent,
    addingNeeded = $bindable(false),
    open = $bindable(false),
    selectedId = $bindable<string | null>(null),
    ringActiveId = null,
    onNeededFocus,
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
    open?: boolean;
    selectedId?: string | null;
    ringActiveId?: string | null;
    onNeededFocus?: (id: string | null, title: string | null) => void;
  } = $props();

  let newNeeded = $state("");

  $effect(() => {
    if (addingNeeded) open = true;
  });

  let neededNow = $derived(neededSorted(cards));

  function setOpen(v: boolean) {
    open = v;
    if (v && neededNow.length > 0) {
      selectedId = neededNow[0].id;
    } else if (!v) {
      selectedId = null;
    }
  }

  $effect(() => {
    if (!open) {
      onNeededFocus?.(null, null);
      return;
    }
    const alive = neededNow.some((c) => c.id === selectedId);
    if (!alive) selectedId = neededNow[0]?.id ?? null;
    const card = neededNow.find((c) => c.id === selectedId) ?? null;
    onNeededFocus?.(card?.id ?? null, card?.title ?? null);
  });

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
    if (selectedId === card.id) selectedId = null;
  }

  function laterNeeded(card: KanbanCard) {
    const now = Date.now() / 1000;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? {
              ...c,
              neededState: "later" as const,
              lastTouchedAt: now,
            }
          : c,
      );
    });
    logEvent("needed-later", { cardId: card.id }, card.projectId);
  }
</script>

{#if variant === "strip" && (neededNow.length > 0 || addingNeeded)}
  <section class="k-needed" aria-label="Needed from you">
    <div class="kn-head">
      <button
        type="button"
        class="kn-toggle"
        class:mkey-sel={ringActiveId === "needed-toggle"}
        data-mkey="needed-toggle"
        aria-expanded={open}
        onclick={() => setOpen(!open)}
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
          setOpen(true);
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
          <li
            class="kn-row"
            class:sel={selectedId === card.id}
            class:mkey-sel={ringActiveId === `needed-${card.id}`}
            data-mkey={`needed-${card.id}`}
            onclick={() => (selectedId = card.id)}
          >
            <p class="kn-q">{card.title}</p>
            {#if card.neededWhy}
              <div class="kn-why">{@html mdLite(card.neededWhy)}</div>
            {/if}
            <div class="kn-acts">
              <button
                class="mini ghost"
                onclick={(e) => {
                  e.stopPropagation();
                  laterNeeded(card);
                }}
                title="Drop to the bottom of the stack">Later</button
              >
              <button
                class="mini ghost"
                onclick={(e) => {
                  e.stopPropagation();
                  skipNeeded(card);
                }}
                title="Delete this Needed">Skip</button
              >
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
    cursor: pointer;
  }

  .kn-row.sel {
    border-color: var(--focus, #6e4db0);
    box-shadow: inset 0 0 0 1px var(--focus, #6e4db0);
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
