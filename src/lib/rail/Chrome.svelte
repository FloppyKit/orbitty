<script lang="ts">
  import type { RailApi } from "./api";

  let { rail }: { rail: RailApi } = $props();
</script>

{#if rail.embedded}
  <div class="chrome">
    <header
      class="banner"
      onmousedown={rail.onBannerMouseDown}
    >
      <h1>Planets</h1>
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
  </div>
{/if}
