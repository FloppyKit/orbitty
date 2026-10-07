<script lang="ts">
  import SkyField from "../SkyField.svelte";
  import LedgerStack from "./LedgerStack.svelte";
  import type { LedgerPage, Project } from "../persist";
  import {
    folioFace,
    formatDayLabel,
    formatSpanLabel,
    pageCovers,
    pageEnd,
    pageStart,
    stripCells,
    type FolioFace,
    type StarsView,
  } from "./folioDay";

  let {
    pages,
    projects,
    dayKey,
    rangeFrom,
    rangeTo,
    view,
    today,
    face,
    hint = "",
    hasWords,
    onReflect,
    onSelectDay,
  }: {
    pages: LedgerPage[];
    projects: Project[];
    dayKey: string;
    rangeFrom: string;
    rangeTo: string;
    view: StarsView;
    today: string;
    face: FolioFace;
    hint?: string;
    hasWords: (day: string) => boolean;
    onReflect: () => void;
    onSelectDay: (day: string) => void;
  } = $props();

  let showCloud = $state(false);

  const cells = $derived(
    stripCells(dayKey, view, rangeFrom, rangeTo, pages, hasWords, today),
  );

  const windowPages = $derived(
    pages
      .filter((p) => {
        const a = pageStart(p);
        const b = pageEnd(p);
        return a <= rangeTo && b >= rangeFrom;
      })
      .slice()
      .sort((a, b) => b.createdAt - a.createdAt),
  );

  const dayPages = $derived(face.kind === "journal" ? face.pages : []);
  const cover = $derived(
    view === "day"
      ? face
      : folioFace(dayKey, pages, () => true),
  );

  const journalPages = $derived(
    view === "day" ? dayPages : windowPages.length ? windowPages : dayPages,
  );

  const showingJournal = $derived(
    view === "day" ? cover.kind === "journal" && !showCloud : journalPages.length > 0 && !showCloud,
  );

  const cloudFrom = $derived.by(() => {
    if (view !== "day") return rangeFrom;
    const page = dayPages[0];
    if (showCloud && page) return pageStart(page);
    return dayKey;
  });
  const cloudTo = $derived.by(() => {
    if (view !== "day") return rangeTo;
    const page = dayPages[0];
    if (showCloud && page) return pageEnd(page);
    return dayKey;
  });
  const cloudBody = $derived(
    showCloud && view === "day" ? (dayPages[0]?.body ?? "") : "",
  );

  const heading = $derived.by(() => {
    if (view === "day") {
      const page = dayPages[0];
      if (page && pageStart(page) !== pageEnd(page)) {
        return formatSpanLabel(pageStart(page), pageEnd(page));
      }
      return formatDayLabel(dayKey);
    }
    return formatSpanLabel(rangeFrom, rangeTo);
  });

  function cellLabel(cell: (typeof cells)[number]): string {
    const name = formatDayLabel(cell.day);
    const kind =
      cell.mark === "page" ? "journal page" : cell.mark === "cloud" ? "word cloud" : "empty";
    const span = cell.spanRole ? `, span ${cell.spanRole}` : "";
    const sel = cell.selected ? ", selected" : "";
    return `${name}, ${kind}${span}${sel}`;
  }

  $effect(() => {
    dayKey;
    view;
    showCloud = false;
  });
</script>

<section class="folio" aria-label="Journal">
  <div class="strip" role="listbox" aria-label="Dates">
    {#each cells as cell (cell.day)}
      <button
        type="button"
        class="cell"
        class:selected={cell.selected}
        role="option"
        aria-selected={cell.selected}
        aria-label={cellLabel(cell)}
        data-strip-day={cell.day}
        data-mark={cell.mark}
        data-span={cell.spanRole ?? ""}
        onclick={() => onSelectDay(cell.day)}
      >
        <span class="dow">{cell.day.slice(8)}</span>
        {#if cell.mark === "page" && cell.spanRole && cell.spanRole !== "solo"}
          <span class="bar {cell.spanRole}"></span>
        {:else if cell.mark === "page"}
          <span class="mark page"></span>
        {:else if cell.mark === "cloud"}
          <span class="mark cloud"></span>
        {:else}
          <span class="mark none"></span>
        {/if}
      </button>
    {/each}
  </div>

  <header class="head">
    <h2>{heading}</h2>
    {#if (view === "day" && cover.kind === "journal") || (view !== "day" && journalPages.length)}
      <button
        type="button"
        class="cloud-chip"
        aria-pressed={showCloud}
        onclick={() => (showCloud = !showCloud)}
      >Cloud</button>
    {/if}
    <button type="button" class="reflect" onclick={onReflect}>Reflect</button>
  </header>
  {#if hint}<p class="hint">{hint}</p>{/if}

  <div class="face">
    {#if showingJournal}
      <div class="page-face">
        <LedgerStack
          pages={journalPages.filter((p): p is LedgerPage => pageCovers(p, dayKey) || view !== "day")}
          {projects}
          dayKey={dayKey}
          onReflect={onReflect}
        />
      </div>
    {:else if view === "day" && !hasWords(dayKey) && !cloudBody.trim()}
      <div class="quiet">
        <p>No words for {formatDayLabel(dayKey)} yet</p>
        <button type="button" onclick={onReflect}>Write a page</button>
      </div>
    {:else}
      <SkyField from={cloudFrom} to={cloudTo} compact pageBody={cloudBody || undefined} />
    {/if}
  </div>
</section>

<style>
  .folio {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow: hidden;
  }

  .strip {
    display: flex;
    gap: 0;
    overflow-x: auto;
    flex: none;
  }

  .cell {
    flex: 1 0 28px;
    min-width: 28px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    padding: 3px 0 4px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--dim);
    font: inherit;
    font-size: 0.62rem;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
  }

  .cell.selected {
    box-shadow: 0 0 0 1px var(--accent);
    color: var(--ink);
  }

  .mark {
    width: 6px;
    height: 6px;
    border-radius: 99px;
  }

  .mark.page {
    background: color-mix(in srgb, var(--accent) 60%, var(--bg));
    border-radius: 1px;
  }

  .mark.cloud {
    background: transparent;
    border: 1px solid var(--dim);
    box-sizing: border-box;
  }

  .mark.none {
    background: transparent;
  }

  .bar {
    display: block;
    height: 3px;
    width: 100%;
    background: color-mix(in srgb, var(--accent) 60%, var(--bg));
  }

  .bar.start {
    border-radius: 2px 0 0 2px;
  }

  .bar.end {
    border-radius: 0 2px 2px 0;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }

  h2 {
    margin: 0;
    flex: 1;
    font-size: 0.84rem;
    font-weight: 650;
    color: var(--ink);
  }

  .cloud-chip,
  .reflect,
  .quiet button {
    font: inherit;
    font-size: 0.68rem;
    color: var(--ink);
    background: var(--panel-fill);
    border: 1px solid var(--edge);
    border-radius: 999px;
    padding: 2px 8px;
    min-height: 24px;
    cursor: pointer;
  }

  .cloud-chip[aria-pressed="true"] {
    border-color: var(--accent);
  }

  .hint {
    margin: 0;
    font-size: 0.68rem;
    color: var(--dim);
  }

  .face {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
  }

  .page-face {
    background: var(--panel-fill);
    color: var(--ink);
    border-radius: 12px;
    padding: 10px 12px 12px 14px;
    border-left: 2px solid color-mix(in srgb, var(--ink) 22%, transparent);
    font-family: "SF Pro Text", "SF Pro Display", system-ui, sans-serif;
    line-height: 1.5;
  }

  .quiet {
    margin: auto;
    text-align: center;
    color: var(--dim);
    font-size: 0.78rem;
  }

  .quiet p {
    margin: 0 0 8px;
  }
</style>
