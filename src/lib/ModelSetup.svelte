<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open as shellOpen } from "@tauri-apps/plugin-shell";
  import { onMount } from "svelte";
  import { HARNESS_BLURB, HARNESS_GROUP, HARNESSES, LMSTUDIO_OFF } from "./modelSetupCopy";

  let { cue = "", alwaysOffer = false }: { cue?: string; alwaysOffer?: boolean } = $props();

  const OLLAMA_DOWNLOAD = "https://ollama.com/download";
  const NO_MODEL = "I don't have a model yet. Click Set up a model to connect one.";
  const CHAT_LANES = new Set(["grok", "goose", "hermes"]);

  type Hit = { source: string; id: string; base: string };
  type Lm = { installed: boolean; serving: boolean; line: string };

  let alpha = $state(false);
  let open = $state(false);
  let hits = $state<Hit[]>([]);
  let clis = $state<string[]>([]);
  let lm = $state<Lm>({ installed: false, serving: false, line: "" });
  let probed = $state(false);
  let note = $state("");
  let orKey = $state("");
  let compatKey = $state("");
  let compatBase = $state("https://openrouter.ai/api/v1");
  let compatModel = $state("");

  const show = $derived(alpha && (alwaysOffer || open || (cue ?? "").includes("Set up a model")));
  const lmOff = $derived(lm.installed && !lm.serving && !hits.some((h) => h.source === "lmstudio"));

  async function probe() {
    note = "";
    try {
      hits = await invoke<Hit[]>("model_setup_probe");
      clis = await invoke<string[]>("model_setup_clis");
      lm = await invoke<Lm>("model_setup_lmstudio");
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
    if (alpha && (alwaysOffer || (cue ?? "").includes("Set up a model"))) void probe();
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
    if (lane === "cursor" || lane === "pi" || !CHAT_LANES.has(lane)) {
      note = `${lane} is a signed-in harness for a planet row, not this chat.`;
      return;
    }
    if (!clis.includes(lane)) {
      note = `${lane} is not on PATH.`;
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
  <section class="model-setup-card" data-panel="model-setup">
    <button type="button" class="mini" onclick={() => { open = true; probe(); }}>Set up a model</button>
    {#if open}
      <p class="help">No model until you attach one. Keys stay in Keychain.</p>
      <button type="button" class="mini ghost" onclick={useNone}>No model</button>
      <button type="button" class="mini" onclick={probe}>Look again</button>
      {#if lmOff}
        <p class="help">{lm.line || LMSTUDIO_OFF}</p>
      {/if}
      {#if probed && hits.length === 0 && !lmOff}
        <p class="help">Neither Ollama nor LM Studio answered.</p>
      {/if}
      {#if probed && hits.length === 0}
        <button type="button" class="mini" onclick={download}>Download Ollama</button>
      {/if}
      {#each hits as hit (hit.source + hit.id)}
        <div class="row">
          <span>{hit.source}: {hit.id}</span>
          <button type="button" class="mini" onclick={() => useLocal(hit)}>Use</button>
        </div>
      {/each}
      <p class="group">{HARNESS_GROUP}</p>
      <p class="help">{HARNESS_BLURB}</p>
      {#each HARNESSES as h (h.id)}
        <div class="row">
          <span class="harness-name">{h.id}</span>
          <span class="help">{h.line}</span>
          <button type="button" class="mini" onclick={() => useCli(h.id)}>Use {h.id}</button>
        </div>
      {/each}
      <label>Paste an API key
        <input type="password" bind:value={orKey} placeholder="OpenRouter key" />
        <button type="button" class="mini" disabled={!orKey.trim()} onclick={saveOr}>Save</button>
      </label>
      <label>Compat key
        <input type="password" bind:value={compatKey} placeholder="API key" />
        <span class="help">Base URL</span>
        <input bind:value={compatBase} aria-label="Base URL" placeholder="Base URL" />
        <input bind:value={compatModel} placeholder="Model id" />
        <button type="button" class="mini" disabled={!compatKey.trim()} onclick={saveCompat}>Save</button>
      </label>
      {#if note}<p class="help">{note}</p>{/if}
    {/if}
  </section>
{/if}

<style>
  .model-setup-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 8px 10px;
    padding: 12px;
    max-width: 28rem;
    border-radius: 12px;
    background: var(--bg-solid, #14161c);
    color: var(--ink, #f4f5f7);
    border: 1px solid var(--edge, #2a2e36);
  }
  .row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .group { margin: 4px 0 0; font-weight: 600; color: var(--ink, #f4f5f7); }
  .harness-name { font-weight: 600; text-transform: none; }
  .help { margin: 0; color: var(--dim, #8a8f98); font-size: 12px; }
  button, input {
    background: color-mix(in srgb, var(--bg-solid, #14161c) 72%, #000);
    color: var(--ink, #f4f5f7);
    border: 1px solid var(--edge, #3a3f48);
    border-radius: 6px;
    padding: 4px 8px;
  }
  button:disabled { opacity: 0.45; }
</style>
