<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open as shellOpen } from "@tauri-apps/plugin-shell";
  import { onMount } from "svelte";

  let { cue = "", alwaysOffer = false }: { cue?: string; alwaysOffer?: boolean } = $props();

  const OLLAMA_DOWNLOAD = "https://ollama.com/download";
  const NO_MODEL = "I don't have a model yet. Click Set up a model to connect one.";

  type Hit = { source: string; id: string; base: string };

  let alpha = $state(false);
  let open = $state(false);
  let hits = $state<Hit[]>([]);
  let clis = $state<string[]>([]);
  let probed = $state(false);
  let note = $state("");
  let orKey = $state("");
  let compatKey = $state("");
  let compatBase = $state("https://openrouter.ai/api/v1");
  let compatModel = $state("");

  const show = $derived(alpha && (alwaysOffer || open || (cue ?? "").includes("Set up a model")));

  async function probe() {
    note = "";
    try {
      hits = await invoke<Hit[]>("model_setup_probe");
      clis = await invoke<string[]>("model_setup_clis");
      probed = true;
    } catch (e) {
      note = `${e}`;
    }
  }

  onMount(async () => {
    try {
      alpha = await invoke<boolean>("alpha_build");
    } catch {
      alpha = false;
    }
    if (alpha && ((cue ?? "").includes(NO_MODEL) || alwaysOffer)) open = true;
  });

  $effect(() => {
    if (alpha && (cue ?? "").includes("Set up a model")) open = true;
  });

  async function useLocal(hit: Hit) {
    await invoke("set_companion_reply_model", { model: hit.id });
    await invoke("set_companion_reply_lane", { lane: "local" });
    note = `Using ${hit.id}`;
  }

  async function useCli(lane: string) {
    if (lane === "cursor") {
      note = "Cursor is a signed-in harness for a planet row, not this chat.";
      return;
    }
    await invoke("set_companion_backend", { id: lane });
    await invoke("set_companion_reply_lane", { lane });
    note = `Using ${lane}`;
  }

  async function useNone() {
    await invoke("set_companion_reply_lane", { lane: "none" });
    note = "No model";
  }

  async function saveOr() {
    await invoke("set_openrouter_key", { key: orKey.trim() });
    await invoke("set_companion_reply_lane", { lane: "openrouter" });
    orKey = "";
    note = "Key saved in Keychain";
  }

  async function saveCompat() {
    await invoke("set_direct_endpoint", { base: compatBase.trim(), model: compatModel.trim() });
    await invoke("set_direct_key", { key: compatKey.trim() });
    await invoke("set_companion_backend", { id: "direct" });
    await invoke("set_companion_reply_lane", { lane: "compat" });
    compatKey = "";
    note = "Key saved in Keychain";
  }

  async function download() {
    await shellOpen(OLLAMA_DOWNLOAD);
  }
</script>

{#if show}
  <section class="model-setup">
    <button type="button" class="mini" onclick={() => { open = true; probe(); }}>Set up a model</button>
    {#if open}
      <p class="help">No model until you attach one. Keys stay in Keychain.</p>
      <button type="button" class="mini ghost" onclick={useNone}>No model</button>
      <button type="button" class="mini" onclick={probe}>Look again</button>
      {#if probed && hits.length === 0}
        <p class="help">Neither Ollama nor LM Studio answered.</p>
        <button type="button" class="mini" onclick={download}>Download Ollama</button>
      {/if}
      {#each hits as hit (hit.source + hit.id)}
        <div class="row">
          <span>{hit.source}: {hit.id}</span>
          <button type="button" class="mini" onclick={() => useLocal(hit)}>Use</button>
        </div>
      {/each}
      {#if clis.length}
        <p class="help">Signed-in app on this Mac</p>
        {#each clis as lane (lane)}
          <button type="button" class="mini" onclick={() => useCli(lane)}>Use {lane}</button>
        {/each}
      {/if}
      <label>Paste an API key
        <input type="password" bind:value={orKey} placeholder="OpenRouter key" />
        <button type="button" class="mini" disabled={!orKey.trim()} onclick={saveOr}>Save</button>
      </label>
      <label>Compat key
        <input type="password" bind:value={compatKey} placeholder="API key" />
        <input bind:value={compatBase} placeholder="Base URL" />
        <input bind:value={compatModel} placeholder="Model id" />
        <button type="button" class="mini" disabled={!compatKey.trim()} onclick={saveCompat}>Save</button>
      </label>
      {#if note}<p class="help">{note}</p>{/if}
    {/if}
  </section>
{/if}

<style>
  .model-setup { display: flex; flex-direction: column; gap: 6px; margin: 8px 0; }
  .row { display: flex; gap: 8px; align-items: center; }
  .help { margin: 0; color: #8a8f98; font-size: 12px; }
  input { background: transparent; color: inherit; border: 1px solid #3a3f48; border-radius: 6px; padding: 4px 6px; }
</style>
