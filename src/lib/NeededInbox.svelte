<script module lang="ts">
  import type { KanbanCard, Project } from "./persist";

  // One Needed ordering, two readers — the strip
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

  // LP-SPLIT: the pile AND the planet-tab filter, one door — the strip
  // paints exactly these rows and the Launchpad ring walks exactly these
  // rows. "" (Inbox / Today / Done) = the consolidated all-planets pile;
  // a planet id = that planet's Neededs only.
  export function neededFor(
    cards: KanbanCard[],
    projectId: string,
  ): KanbanCard[] {
    return neededSorted(cards).filter(
      (c) => projectId === "" || c.projectId === projectId,
    );
  }
</script>

<script lang="ts">
  // Needed pile. Launchpad kanban-row COLUMN in front of Todo, ALWAYS
  // OPEN — not an accordion (Matt 2026-09-15: "have the neededs open
  // and not an accordian file"). Pile titles in a top box (cap four
  // rows, scrolls) and the selected tile (question / why /
  // Answer·Later·Skip) in the lower preview box (needed-split.css) —
  // the Todo/Doing/Done columns stay on screen. The companion text box
  // is the mouth for the answer: onNeededFocus arms the composer,
  // Answer / ring Enter lands the caret there — the in-card answer
  // input was the miss. The split band above the board (a menu within
  // the Needed menu) was the earlier one. Why is
  // mdLite so **bold**, `paths`, and lists read as prose, not raw
  // markdown. Parent owns mutate/logEvent so persist stays one door.
  // selectedId is bindable so the Launchpad
  // ring can walk the pile; ringActiveId paints the one focus ring.
  import { planetCss } from "./planetColor";
  import type { PersistedState } from "./persist";
  import "./needed-split.css";

  let {
    variant,
    cards,
    projects = [],
    projectId = "",
    mutate,
    logEvent,
    addingNeeded = $bindable(false),
    selectedId = $bindable<string | null>(null),
    ringActiveId = null,
    onNeededFocus,
    onNeededAnswer,
  }: {
    variant: "strip" | "side";
    cards: KanbanCard[];
    projects?: Project[];
    projectId?: string;
    mutate: (fn: (s: PersistedState) => void) => void;
    logEvent: (
      eType: string,
      content: Record<string, unknown>,
      projectId?: string,
    ) => void;
    addingNeeded?: boolean;
    selectedId?: string | null;
    ringActiveId?: string | null;
    onNeededFocus?: (id: string | null, title: string | null) => void;
    // The Answer button / ring Enter — arms the composer hijack (the
    // companion text box is the one answer mouth). Embedded Launchpad
    // only; the overlay Lists window has no composer, so no Answer
    // button paints there.
    onNeededAnswer?: () => void;
  } = $props();

  let newNeeded = $state("");

  let neededNow = $derived(neededFor(cards, projectId));

  // Planet chip + short name on every row/card — a screenshot of the
  // consolidated Inbox reads which planet each Needed belongs to.
  function projOf(card: KanbanCard): Project | undefined {
    return card.projectId
      ? projects.find((p) => p.id === card.projectId)
      : undefined;
  }

  // The Answer button arms the composer hijack — the companion text
  // box is the one answer mouth (same door as ring Enter).
  $effect(() => {
    if (selectedId && !neededNow.some((c) => c.id === selectedId)) {
      selectedId = null;
    }
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

{#if variant === "strip"}
  <section class="k-needed" aria-label="Needed from you">
    <div class="kn-head">
      <span class="kn-label">
        <span class="mail-ico" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true">
            <circle cx="12" cy="12" r="4.2"/>
            <ellipse cx="12" cy="12" rx="9.2" ry="3.2" transform="rotate(-22 12 12)"/>
          </svg>
        </span>
        <span class="kn-title">Needed</span>
        {#if neededNow.length > 0}
          <span class="kn-count">{neededNow.length}</span>
        {/if}
      </span>
    </div>
    {#if neededNow.length > 0}
      <!-- LP-SPLIT leftover: pile titles in the top box (cap four
           rows, scrolls), the selected tile's preview in the lower
           box — an always-open column in the board row, so the
           Todo/Doing/Done columns never leave the screen while a
           Needed is read/answered. -->
      <ul class="kn-list" aria-label="Needed pile">
        {#each neededNow as card (card.id)}
          {@const p = projOf(card)}
          <li
            class="kn-item"
            class:sel={selectedId === card.id}
            class:mkey-sel={ringActiveId === `needed-${card.id}`}
            data-mkey={`needed-${card.id}`}
            onclick={() => (selectedId = card.id)}
          >
            {#if p}
              <span
                class="kn-chip"
                style="background: {planetCss(p.id)}"
                title={p.name}
                aria-hidden="true"
              ></span>
            {/if}
            <span class="kn-item-title">{card.title}</span>
            <button
              type="button"
              class="mini ghost kn-act"
              onclick={(e) => {
                e.stopPropagation();
                laterNeeded(card);
              }}
            >Later</button>
            <button
              type="button"
              class="mini ghost kn-act"
              onclick={(e) => {
                e.stopPropagation();
                skipNeeded(card);
              }}
            >Skip</button>
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
  /* The column root (.k-needed) and the open boxes (kn-list / kn-item /
     kn-card) live in needed-split.css — the split band above the board
     was the miss; the kanban-row column replaced it. */
  .kn-head {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 8px 10px 6px;
  }

  /* Always open — the header is a label, not a toggle. */
  .kn-label {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0;
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

  /* LP-SPLIT leftover: the open column boxes (kn-list / kn-item /
     kn-card) live in needed-split.css — the split band above the board
     was the miss and is deleted, not piled on. */
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

  .kn-act {
    flex-shrink: 0;
    font-size: 0.6rem;
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
    padding: 4px 8px 8px;
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
