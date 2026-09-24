<script lang="ts">
  import type { LedgerPage, Project } from "../persist";
  import { planetCss } from "../planetColor";

  let {
    pages,
    projects,
    onReflect,
  }: {
    pages: LedgerPage[];
    projects: Project[];
    onReflect: () => void;
  } = $props();

  let index = $state(0);
  let seen = 0;
  $effect(() => {
    if (pages.length > seen) index = Math.max(0, pages.length - 1);
    seen = pages.length;
  });
  const safe = $derived(
    pages.length === 0 ? 0 : Math.min(index, pages.length - 1),
  );
  const page = $derived(pages[safe] ?? null);
  const depth = $derived(Math.min(2, Math.max(0, pages.length - 1)));

  function planetFor(scope: string | undefined): Project | null {
    if (!scope || scope === "overall") return null;
    return projects.find((p) => p.id === scope) ?? null;
  }

  function flagLine(page: LedgerPage): string {
    const f = page.metricFlags;
    if (!f) return "";
    const on: string[] = [];
    if (f.spokes) on.push("spokes");
    if (f.wins) on.push("wins");
    if (f.grateful) on.push("grateful");
    if (f.trackers) on.push("trackers");
    if (f.cloud) on.push("cloud");
    return on.join(" · ");
  }

  function flip(delta: number) {
    if (!pages.length) return;
    index = (safe + delta + pages.length) % pages.length;
  }
</script>

<section class="ledger" aria-label="Journal">
  {#if pages.length}
    <header>
      <span class="count">{safe + 1} / {pages.length}</span>
      <button type="button" class="nav" onclick={() => flip(-1)} aria-label="Previous page">‹</button>
      <button type="button" class="nav" onclick={() => flip(1)} aria-label="Next page">›</button>
    </header>
  {/if}
  <div class="stack" class:deep={depth > 0}>
    <div class="sheet back2" aria-hidden="true"></div>
    <div class="sheet back1" aria-hidden="true"></div>
    {#key page?.id ?? "empty"}
      <article class="page">
        {#if page}
          {@const planet = planetFor(page.scope)}
          <h3>{page.dayKey ?? "Undated"}</h3>
          <p class="meta">
            {#if planet}
              <span class="chip" style="background: {planetCss(planet.id)}"></span>
              {planet.name}
            {:else}
              All
            {/if}
          </p>
          {#if flagLine(page)}
            <p class="flags">{flagLine(page)}</p>
          {/if}
          <p class="body">{page.body?.trim() ? page.body : ""}</p>
        {:else}
          <p class="lead">Reflect writes the next page.</p>
          <button type="button" class="cta" onclick={onReflect}>Open Reflect</button>
        {/if}
      </article>
    {/key}
  </div>
</section>

<style>
  .ledger {
    flex: 0 0 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  header {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.68rem;
    font-weight: 700;
  }

  .count {
    flex: 1;
    font-weight: 500;
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }

  .nav {
    font: inherit;
    width: 22px;
    height: 22px;
    padding: 0;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    cursor: pointer;
  }

  .nav:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .stack {
    position: relative;
    min-height: 108px;
    padding: 0 8px 8px 0;
  }

  .stack:not(.deep) .sheet {
    display: none;
  }

  .sheet {
    position: absolute;
    inset: 8px 0 0 8px;
    border: 1px solid var(--edge);
    border-radius: 10px;
    background: var(--bg-fill, var(--bg));
    box-shadow: 0 1px 0 rgba(0, 0, 0, 0.04);
  }

  .back1 {
    transform: translate(4px, 4px);
    opacity: 0.85;
  }

  .back2 {
    transform: translate(8px, 8px);
    opacity: 0.55;
  }

  .page {
    position: relative;
    min-height: 96px;
    margin: 0;
    padding: 10px 12px 12px;
    border: 1px solid var(--edge);
    border-radius: 10px 4px 10px 10px;
    background: var(--panel-fill, var(--panel));
    box-shadow:
      0 8px 18px rgba(0, 0, 0, 0.12),
      inset -10px 0 12px rgba(0, 0, 0, 0.03);
    animation: flip 0.28s ease;
  }

  h3 {
    margin: 0;
    font-size: 0.84rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .meta,
  .flags,
  .body {
    margin: 0;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-top: 3px;
    font-size: 0.66rem;
    color: var(--dim);
  }

  .chip {
    width: 8px;
    height: 8px;
    border-radius: 99px;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.28);
    flex: none;
  }

  .flags {
    margin-top: 6px;
    font-size: 0.62rem;
    color: var(--dim);
  }

  .body {
    margin-top: 8px;
    font-size: 0.75rem;
    line-height: 1.35;
    color: var(--ink);
    white-space: pre-wrap;
  }

  .lead {
    margin: 0;
    font-size: 0.78rem;
    line-height: 1.35;
    color: var(--ink);
  }

  .cta {
    margin-top: 10px;
    font: inherit;
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 4px 10px;
    cursor: pointer;
  }

  @keyframes flip {
    from {
      transform: rotateY(-70deg);
      opacity: 0.4;
    }
    to {
      transform: none;
      opacity: 1;
    }
  }
</style>
