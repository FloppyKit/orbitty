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
  // Needed pile. Launchpad kanban-row COLUMN in front of Todo: the
  // header toggles it; open paints the pile titles in a top box (cap
  // four rows, scrolls) and the selected tile (question / why /
  // answer input + "if this again" rule row / Later·Skip) in the lower
  // preview box (needed-split.css) — the Todo/Doing/Done columns stay
  // on screen. The split band above the board (a menu within the
  // Needed menu) was the miss; the answer section riding the composer
  // below the well was the next one — it lives IN the card now, so
  // reading and answering a Needed never leaves the kanban row. Why is
  // mdLite so **bold**, `paths`, and lists read as prose, not raw
  // markdown. Parent owns mutate/logEvent so persist stays one door.
  // open/selectedId are bindable so the Launchpad
  // ring can walk the pile; ringActiveId paints the one focus ring.
  import { mdLite } from "./mdLite";
  import { planetCss } from "./planetColor";
  import NeededRuleRow from "./NeededRuleRow.svelte";
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
    open = $bindable(false),
    selectedId = $bindable<string | null>(null),
    ringActiveId = null,
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
    open?: boolean;
    selectedId?: string | null;
    ringActiveId?: string | null;
  } = $props();

  let newNeeded = $state("");
  // The answer section lives in the preview card — one draft per
  // Needed; selecting another row starts fresh.
  let answerDraft = $state("");

  $effect(() => {
    if (addingNeeded) open = true;
  });

  let neededNow = $derived(neededFor(cards, projectId));
  // The card half of the split. The effect below keeps selectedId alive;
  // the neededNow[0] fallback covers the frame between a Skip and the
  // state:changed reload.
  let selCard = $derived(
    neededNow.find((c) => c.id === selectedId) ?? neededNow[0] ?? null,
  );

  // Planet chip + short name on every row/card — a screenshot of the
  // consolidated Inbox reads which planet each Needed belongs to.
  function projOf(card: KanbanCard): Project | undefined {
    return card.projectId
      ? projects.find((p) => p.id === card.projectId)
      : undefined;
  }

  // The one answer door — the same needed-answer event the NextCard
  // window and `orbittyctl needed answer` log, so the watch dispatches.
  // The answered card leaves the pile on the reload; the keep-alive
  // effect below advances the preview to the next Needed.
  function answerCard(card: KanbanCard) {
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
    answerDraft = "";
  }

  function setOpen(v: boolean) {
    open = v;
    if (v && neededNow.length > 0) {
      selectedId = neededNow[0].id;
    } else if (!v) {
      selectedId = null;
    }
  }

  // Keep the preview alive: an answered/skipped card leaves the pile,
  // so the selection falls forward to the next Needed.
  $effect(() => {
    if (!open) return;
    const alive = neededNow.some((c) => c.id === selectedId);
    if (!alive) selectedId = neededNow[0]?.id ?? null;
  });

  // One answer draft per card — selecting another row starts fresh.
  $effect(() => {
    void selCard?.id;
    answerDraft = "";
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
      {#if neededNow.length > 0}
        <!-- LP-SPLIT leftover: pile titles in the top box (cap four
             rows, scrolls), the selected tile's preview in the lower
             box — a column in the board row, so the Todo/Doing/Done
             columns never leave the screen while a Needed is
             read/answered. -->
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
            </li>
          {/each}
        </ul>
        {#if selCard}
          {@const sc = selCard}
          {@const sp = projOf(sc)}
          <div class="kn-card">
            {#if sp}
              <div class="kn-card-head">
                <span
                  class="kn-chip"
                  style="background: {planetCss(sp.id)}"
                  aria-hidden="true"
                ></span>
                <span class="kn-pname">{sp.name}</span>
              </div>
            {/if}
            <p class="kn-q">{sc.title}</p>
            {#if sc.neededWhy}
              <div class="kn-why kn-card-why">{@html mdLite(sc.neededWhy)}</div>
            {/if}
            <!-- The answer section rides the preview card — the same
                 row the kanban columns live in, at the front of it.
                 Enter answers; Esc blurs. The "if this again" rule row
                 moved with it, keyed per card (Save never answers). -->
            <div class="kn-answer">
              <input
                data-mkey="kn-answer"
                bind:value={answerDraft}
                placeholder="Your answer…"
                aria-label="Your answer"
                onkeydown={(e) => {
                  if (e.key === "Enter") answerCard(sc);
                  if (e.key === "Escape") e.currentTarget.blur();
                }}
              />
            </div>
            {#key sc.id}
              <NeededRuleRow card={sc} answer={answerDraft} />
            {/key}
            <div class="kn-acts">
              <button
                class="mini ghost"
                onclick={() => laterNeeded(sc)}
                title="Drop to the bottom of the stack">Later</button
              >
              <button
                class="mini ghost"
                onclick={() => skipNeeded(sc)}
                title="Delete this Needed">Skip</button
              >
            </div>
          </div>
        {/if}
      {/if}
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
    padding: 2px 6px 0;
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
    flex-shrink: 0;
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
