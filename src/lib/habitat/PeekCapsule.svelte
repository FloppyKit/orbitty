<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  type Capsule = {
    card: string;
    status: string;
    state: string;
    text: string;
    source: string;
    up: boolean;
  };

  let shown = $state(false);
  let card = $state("");
  let status = $state("unknown");
  let termState = $state("");
  let text = $state("");
  let preview = $state(false);
  let timer: ReturnType<typeof setInterval> | undefined;

  function stop() {
    if (timer) clearInterval(timer);
    timer = undefined;
  }

  async function refresh() {
    if (!card || !preview) return;
    try {
      const v = await invoke<Capsule>("herdr_peek", { card, lines: 40 });
      status = v.status || "unknown";
      termState = v.state || "";
      text = status === "unknown" ? "" : v.text || "";
    } catch {
      status = "unknown";
      text = "";
    }
  }

  function arm() {
    stop();
    if (preview) timer = setInterval(() => void refresh(), 2000);
  }

  function openPeek() {
    preview = true;
    arm();
    void refresh();
  }

  function closePeek() {
    preview = false;
    text = "";
    stop();
  }

  onMount(() => {
    let un: (() => void) | undefined;
    void listen<Capsule>("herdr-peek", (ev) => {
      card = ev.payload.card || "";
      status = ev.payload.status || "unknown";
      termState = ev.payload.state || "";
      text = "";
      preview = false;
      shown = true;
      stop();
    }).then((fn) => {
      un = fn;
    });
    return () => {
      stop();
      un?.();
    };
  });
</script>

{#if shown}
  <div class="peek" role="status">
    <div class="row">
      <span class="status">{status}{termState ? ` · ${termState}` : ""}</span>
      {#if preview}
        <button type="button" class="peek-btn" onclick={closePeek}>close</button>
      {:else}
        <button type="button" class="peek-btn" onclick={openPeek}>peek</button>
      {/if}
      <button type="button" class="peek-btn" onclick={() => (shown = false)}>hide</button>
    </div>
    {#if preview}
      <pre class="body" tabindex="-1">{text}</pre>
    {/if}
  </div>
{/if}

<style>
  .peek {
    position: fixed;
    right: 12px;
    bottom: 12px;
    z-index: 40;
    max-width: min(420px, calc(100vw - 24px));
    background: var(--panel, #12201a);
    color: var(--ink, #c8f0d0);
    border: 1px solid var(--edge, #1a2a20);
    border-radius: 8px;
    padding: 8px 10px;
    font: 12px/1.4 ui-monospace, "SF Mono", Menlo, monospace;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .status {
    flex: 1;
  }
  .peek-btn {
    background: transparent;
    color: var(--ink, #c8f0d0);
    border: 1px solid var(--edge, #1a2a20);
    border-radius: 4px;
    padding: 2px 8px;
  }
  .body {
    margin: 8px 0 0;
    max-height: 240px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
