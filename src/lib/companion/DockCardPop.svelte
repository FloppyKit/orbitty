<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import type { KanbanCard } from "../persist";

  let { card, onClose }: { card: KanbanCard; onClose: () => void } = $props();

  let excerpt = $state("");
  let linkName = $state("");

  $effect(() => {
    const url = card.links?.find((link) => link.url)?.url ?? "";
    excerpt = "";
    linkName = "";
    if (!url) return;
    let dead = false;
    invoke<{ preview?: { excerpt?: string | null; name?: string | null } }>(
      "click_link",
      { url },
    )
      .then((res) => {
        if (dead) return;
        excerpt = res.preview?.excerpt ?? "";
        linkName = res.preview?.name ?? "";
      })
      .catch(() => {});
    return () => {
      dead = true;
    };
  });

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopImmediatePropagation();
      onClose();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  });

  const body = $derived(
    [card.notes, card.neededWhy, card.neededAnswer].filter(Boolean).join("\n\n"),
  );
</script>

<div class="dcp-root">
  <button class="dcp-scrim" type="button" aria-label="Close preview" onclick={onClose}></button>
  <div class="dcp-pop" role="dialog" aria-label="Card preview">
    <p class="dcp-kind">{card.kind === "needed" ? "Needed" : card.status}</p>
    <p class="dcp-name">{card.title}</p>
    {#if body}
      <pre class="dcp-body">{body}</pre>
    {:else if !excerpt}
      <p class="dcp-empty">No notes on this card.</p>
    {/if}
    {#if linkName}
      <p class="dcp-link">{linkName}</p>
    {/if}
    {#if excerpt}
      <pre class="dcp-body">{excerpt}</pre>
    {/if}
    <button type="button" class="dcp-btn" onclick={onClose}>Close</button>
  </div>
</div>

<style>
  .dcp-root {
    position: fixed;
    inset: 0;
    z-index: 80;
  }
  .dcp-scrim {
    position: absolute;
    inset: 0;
    border: 0;
    padding: 0;
    background: color-mix(in srgb, var(--bg) 35%, transparent);
    cursor: default;
  }
  .dcp-pop {
    position: absolute;
    left: 50%;
    top: 12%;
    transform: translateX(-50%);
    width: min(420px, calc(100% - 24px));
    max-height: min(70vh, 480px);
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    background: var(--panel-fill, var(--panel));
    color: var(--ink);
    border: 1px solid var(--edge);
    border-radius: 10px;
  }
  .dcp-kind {
    margin: 0;
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--dim);
  }
  .dcp-name {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 650;
    overflow-wrap: anywhere;
  }
  .dcp-link,
  .dcp-empty {
    margin: 0;
    font-size: 0.75rem;
    color: var(--dim);
    overflow-wrap: anywhere;
  }
  .dcp-body {
    margin: 0;
    max-height: 16em;
    overflow: auto;
    padding: 8px;
    font: inherit;
    font-size: 0.78rem;
    line-height: 1.45;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 8px;
  }
  .dcp-btn {
    align-self: flex-end;
    padding: 4px 10px;
    font: inherit;
    font-size: 0.78rem;
    color: var(--ink);
    background: transparent;
    border: 1px solid var(--edge);
    border-radius: 8px;
    cursor: pointer;
  }
</style>
