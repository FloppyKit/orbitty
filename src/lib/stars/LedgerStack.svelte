<script lang="ts">
  import type { LedgerPage, Project } from "../persist";

  let {
    pages,
    projects,
  }: {
    pages: LedgerPage[];
    projects: Project[];
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

  function scopeLabel(scope: string | undefined): string {
    if (!scope || scope === "overall") return "Overall";
    return projects.find((p) => p.id === scope)?.name ?? "Planet";
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

<section class="ledger" aria-label="Ledger">
  <header>
    <span>Ledger</span>
    <span class="count">{pages.length ? `${safe + 1} / ${pages.length}` : "0"}</span>
    <button type="button" class="nav" onclick={() => flip(-1)} disabled={!pages.length} aria-label="Previous page">‹</button>
    <button type="button" class="nav" onclick={() => flip(1)} disabled={!pages.length} aria-label="Next page">›</button>
  </header>
  {#key page?.id ?? "empty"}
    <article class="card">
      {#if page}
        <p class="meta">{page.dayKey ?? "Undated"} · {scopeLabel(page.scope)}</p>
        {#if flagLine(page)}
          <p class="flags">{flagLine(page)}</p>
        {/if}
        <p class="body">{page.body?.trim() ? page.body : "Empty page"}</p>
      {:else}
        <p class="body">No ledger pages</p>
      {/if}
    </article>
  {/key}
</section>

<style>
  .ledger {
    flex: 0 0 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
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

  .card {
    min-height: 72px;
    padding: 8px 10px;
    border: 1px solid var(--edge);
    border-radius: 10px;
    background: var(--panel-fill, var(--panel));
    animation: flip 0.28s ease;
  }

  .meta,
  .flags,
  .body {
    margin: 0;
  }

  .meta,
  .flags {
    font-size: 0.64rem;
    color: var(--dim);
  }

  .body {
    margin-top: 4px;
    font-size: 0.75rem;
    color: var(--ink);
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
