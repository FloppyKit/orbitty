<script lang="ts">
  import type { RailApi } from "./api";
  import { railCrumbParts, type ArchFrontName } from "./railInput";

  let { rail }: { rail: RailApi } = $props();

  const parts = $derived.by(() => {
    const names: Record<string, string> = {};
    const projects = (rail.projects ?? []) as { id: string; name?: string }[];
    for (const p of projects) names[p.id] = p.name || "Planet";
    const front = (rail.arch?.front ?? "arch") as ArchFrontName;
    return railCrumbParts(front, rail.arch?.pid ?? null, names);
  });
</script>

{#snippet crumb()}
  <span class="rail-crumb">
    {#each parts as part, i (part.text + i)}
      {#if i > 0}<span class="sep" aria-hidden="true"> › </span>{/if}
      <span class:now={part.current} class:was={!part.current}>{part.text}</span>
    {/each}
  </span>
{/snippet}

<div class="chrome">
  {#if !rail.embedded}
    <p class="rail-crumb-row">{@render crumb()}</p>
  {/if}
  {#if rail.embedded}
  <header
    class="banner"
    onmousedown={rail.onBannerMouseDown}
  >
    <h1>{@render crumb()}</h1>
    <button
      class="popout"
      onmousedown={(e) => e.stopPropagation()}
      onclick={() => rail.onPopout?.()}
      title="Pop out to the rail window — the well closes, the companion stays"
      aria-label="Pop out Planets"
    >
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M15 3h6v6" />
        <path d="M10 14 21 3" />
        <path d="M21 14v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5" />
      </svg>
    </button>
    {#if rail.status?.trusted && rail.refreshedAt}
      <span class="clock" title="last refresh">{rail.refreshedAt}</span>
    {/if}
    <button
      class="winx"
      onmousedown={(e) => e.stopPropagation()}
      onclick={() => rail.onClose?.()}
      title="Dismiss the Planets well — the companion stays"
      aria-label="Dismiss Planets"
      >×</button
    >
  </header>
  {/if}
</div>
