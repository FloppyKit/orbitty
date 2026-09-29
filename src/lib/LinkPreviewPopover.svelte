<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    claimPreviewPopover,
    confirmOpen,
    dismissPreview,
    linkUi,
    releasePreviewPopover,
    revealCurrent,
  } from "./linkOpen.svelte";

  const mine = claimPreviewPopover();
  onDestroy(() => {
    if (mine) releasePreviewPopover();
  });

  const FILE_KINDS = new Set(["document", "folder", "image", "executable"]);

  function fmtSize(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(n < 10 * 1024 ? 1 : 0)} KB`;
    return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  }

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || !linkUi.preview) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      dismissPreview();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  });
</script>

{#if mine && linkUi.preview}
  {@const p = linkUi.preview}
  <div class="lp-root">
    <button class="lp-scrim" type="button" aria-label="Cancel" onclick={dismissPreview}></button>
    <div class="lp-pop" role="dialog" aria-label="Link preview">
      <p class="lp-kind">{p.kind}</p>
      <p class="lp-name">{p.name}</p>
      <p class="lp-target">{p.target}</p>
      {#if p.kind === "https" && p.host}
        <p class="lp-meta">{p.host}</p>
      {/if}
      {#if p.size != null || p.modified}
        <p class="lp-meta">
          {#if p.size != null}{fmtSize(p.size)}{/if}
          {#if p.size != null && p.modified}
            ·
          {/if}
          {#if p.modified}{p.modified}{/if}
        </p>
      {/if}
      {#if p.thumbnail}
        <img class="lp-thumb" src={p.thumbnail} alt="" />
      {/if}
      {#if p.excerpt}
        <pre class="lp-excerpt">{p.excerpt}</pre>
      {/if}
      {#if p.warning}
        <p class="lp-warn">{p.warning}</p>
      {/if}
      {#if linkUi.error}
        <p class="lp-err">{linkUi.error}</p>
      {/if}
      <div class="lp-acts">
        <button
          type="button"
          class="lp-btn"
          class:lp-run={p.kind === "executable"}
          disabled={linkUi.busy}
          onclick={confirmOpen}
        >{p.kind === "executable" ? "Run" : "Open"}</button>
        {#if FILE_KINDS.has(p.kind)}
          <button type="button" class="lp-btn" disabled={linkUi.busy} onclick={revealCurrent}
            >Reveal in Finder</button>
        {/if}
        <button type="button" class="lp-btn" onclick={dismissPreview}>Cancel</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .lp-root {
    position: fixed;
    inset: 0;
    z-index: 80;
  }
  .lp-scrim {
    position: absolute;
    inset: 0;
    border: 0;
    padding: 0;
    background: color-mix(in srgb, var(--bg, #111) 35%, transparent);
    cursor: default;
  }
  .lp-pop {
    position: absolute;
    left: 50%;
    top: 18%;
    transform: translateX(-50%);
    width: min(320px, calc(100% - 24px));
    max-height: min(70vh, 420px);
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    background: var(--panel-fill, var(--panel, #1c1c1c));
    color: var(--ink, #eee);
    border: 1px solid var(--edge, #444);
    border-radius: 10px;
    box-shadow: 0 8px 28px color-mix(in srgb, #000 35%, transparent);
  }
  .lp-kind {
    margin: 0;
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--dim, #999);
  }
  .lp-name {
    margin: 0;
    font-size: 0.92rem;
    font-weight: 650;
    overflow-wrap: anywhere;
  }
  .lp-target,
  .lp-meta {
    margin: 0;
    font-size: 0.72rem;
    line-height: 1.4;
    color: var(--dim, #999);
    overflow-wrap: anywhere;
  }
  .lp-thumb {
    display: block;
    max-width: 100%;
    max-height: 120px;
    object-fit: contain;
    border-radius: 6px;
    border: 1px solid var(--edge, #444);
  }
  .lp-excerpt {
    margin: 0;
    max-height: 9.5em;
    overflow: auto;
    padding: 8px;
    font: inherit;
    font-size: 0.72rem;
    line-height: 1.45;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    color: var(--dim, #bbb);
    background: var(--bg-fill, var(--bg, #111));
    border: 1px dashed var(--edge, #444);
    border-radius: 8px;
  }
  .lp-warn {
    margin: 0;
    font-size: 0.75rem;
    color: #f0d0d0;
  }
  .lp-err {
    margin: 0;
    font-size: 0.75rem;
    color: #e8b4b4;
  }
  .lp-acts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 2px;
  }
  .lp-btn {
    font: inherit;
    font-size: 0.75rem;
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--edge, #555);
    background: transparent;
    color: var(--ink, #eee);
    cursor: pointer;
  }
  .lp-btn:hover {
    border-color: var(--accent, #c8a15a);
  }
  .lp-btn:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .lp-run {
    color: #fff;
    background: #8d3a3a;
    border-color: #8d3a3a;
  }
  .lp-run:hover {
    background: #a34747;
    border-color: #a34747;
  }
</style>
