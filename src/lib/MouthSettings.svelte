<script lang="ts">
  // One Mouth card: Reply · Quick ack · Keys. Scan running
  // locals (Ollama / LM Studio); hop owns the chosen Reply id.
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import type { PersistedState } from "./persist";

  type ReplyLane = "grok" | "local" | "openrouter" | "free";
  type ScannedModel = {
    id: string;
    source: string;
    base: string;
    label: string;
  };
  type MouthSuggest = {
    job: string;
    name: string;
    why: string;
    ollamaTag: string | null;
    have: boolean;
  };
  type MouthScan = {
    models: ScannedModel[];
    ollamaUp: boolean;
    lmstudioUp: boolean;
    suggest: MouthSuggest[];
    ramGb: number;
    appleSilicon: boolean;
    emptyHint: string;
    nextAck: string | null;
    ollamaBin: boolean;
  };

  const QUICK_MOUTH = "google/gemma-4-e4b";

  let replyLane = $state<ReplyLane>("grok");
  let replyModel = $state("");
  let conductorOn = $state(false);
  let orKeyDraft = $state("");
  let orKey = $state<{ set: boolean; source: string; tail: string }>({
    set: false,
    source: "",
    tail: "",
  });
  let orBusy = $state(false);
  let orNote = $state<string | null>(null);
  let scan = $state<MouthScan | null>(null);
  let pullBusy = $state(false);
  let pullNote = $state<string | null>(null);
  let handoffBackend = $state("");
  let autoHandoff = $state(false);
  let handoffChoices = $state<{ id: string; label: string }[]>([]);

  function healLane(raw: string | undefined, freeMode: boolean): ReplyLane {
    const s = (raw ?? "").trim().toLowerCase();
    if (s === "local" || s === "lmstudio" || s === "ollama") return "local";
    if (s === "openrouter" || s === "or") return "openrouter";
    if (s === "free" || s === "freemode" || s === "free-mode" || s === "free_mode")
      return "free";
    if (s === "grok") return "grok";
    return freeMode ? "free" : "grok";
  }

  async function loadState() {
    try {
      const s = await invoke<PersistedState>("load_state");
      replyLane = healLane(s.companion?.replyLane, s.companion?.freeMode ?? false);
      replyModel = s.companion?.replyModel ?? "";
      conductorOn = s.companion?.conductorOn ?? false;
      handoffBackend = s.companion?.handoffBackend ?? "";
      autoHandoff = s.companion?.autoHandoff ?? false;
    } catch {
      /* next scan still paints */
    }
  }

  async function loadKeyStatus() {
    try {
      orKey = await invoke<{ set: boolean; source: string; tail: string }>(
        "openrouter_key_status",
      );
    } catch {
      orKey = { set: false, source: "", tail: "" };
    }
  }

  async function loadHandoffChoices() {
    try {
      handoffChoices = await invoke<{ id: string; label: string }[]>("handoff_backends");
    } catch {
      handoffChoices = [];
    }
  }

  async function saveHandoffBackend() {
    try {
      const saved = await invoke<string>("set_companion_handoff_backend", {
        backend: handoffBackend,
      });
      handoffBackend = saved;
    } catch {
      /* state:changed restores */
    }
  }

  async function saveAutoHandoff(on: boolean) {
    autoHandoff = on;
    try {
      await invoke("set_companion_auto_handoff", { enabled: on });
    } catch {
      /* state:changed restores */
    }
  }

  async function rescan() {
    try {
      scan = await invoke<MouthScan>("mouth_scan");
      const ids = scan.models.map((m) => m.id);
      if (ids.includes(QUICK_MOUTH) && !replyModel) {
        replyModel = QUICK_MOUTH;
        await pickLocalModel();
      } else if (replyModel && ids.length > 0 && !ids.includes(replyModel)) {
        replyModel = "";
      }
    } catch {
      scan = {
        models: [],
        ollamaUp: false,
        lmstudioUp: false,
        suggest: [],
        ramGb: 0,
        appleSilicon: false,
        emptyHint: "none found — open Ollama or LM Studio.",
        nextAck: null,
        ollamaBin: false,
      };
    }
  }

  async function pickReply(lane: ReplyLane) {
    replyLane = lane;
    try {
      const saved = await invoke<string>("set_companion_reply_lane", { lane });
      replyLane = healLane(saved, lane === "free");
    } catch {
      /* state:changed restores */
    }
  }

  async function pickLocalModel() {
    try {
      const saved = await invoke<string>("set_companion_reply_model", {
        model: replyModel,
      });
      replyModel = saved;
    } catch {
      /* state:changed restores */
    }
  }

  async function pickAck(on: boolean) {
    conductorOn = on;
    try {
      await invoke("set_companion_conductor", { enabled: on });
    } catch {
      /* state:changed restores */
    }
  }

  async function saveOrKey() {
    if (orBusy) return;
    orBusy = true;
    orNote = null;
    try {
      orKey = await invoke("set_openrouter_key", { key: orKeyDraft.trim() });
      orKeyDraft = "";
    } catch (e) {
      orNote = `${e}`;
    }
    orBusy = false;
  }

  async function clearOrKey() {
    if (orBusy) return;
    orBusy = true;
    orNote = null;
    try {
      orKey = await invoke("set_openrouter_key", { key: "" });
    } catch (e) {
      orNote = `${e}`;
    }
    orBusy = false;
  }

  async function pullSuggest(tag: string) {
    if (pullBusy || !tag) return;
    pullBusy = true;
    pullNote = null;
    try {
      pullNote = await invoke<string>("pull_mouth_model", { tag });
      await rescan();
    } catch (e) {
      pullNote = `${e}`;
    }
    pullBusy = false;
  }

  let unlisten: (() => void) | undefined;
  const onVisibility = () => {
    if (!document.hidden) rescan();
  };
  onMount(async () => {
    await loadState();
    await loadKeyStatus();
    await loadHandoffChoices();
    await rescan();
    unlisten = await listen("state:changed", () => {
      loadState();
    });
    document.addEventListener("visibilitychange", onVisibility);
  });
  onDestroy(() => {
    unlisten?.();
    document.removeEventListener("visibilitychange", onVisibility);
  });

  const showLocalPick = $derived(
    replyLane === "local" || replyLane === "free",
  );
  const models = $derived(scan?.models ?? []);
  const emptyHint = $derived(
    scan?.emptyHint || "none found — open Ollama or LM Studio.",
  );
  const suggest = $derived(scan?.suggest?.[0] ?? null);
  const canPull = $derived(
    !!(
      scan?.ollamaUp &&
      scan?.ollamaBin &&
      suggest?.ollamaTag &&
      !suggest.have
    ),
  );
</script>

<p class="block-label">Mouth</p>

<div class="mouth-row">
  <span class="mouth-k">Reply</span>
  <div class="mouth-v">
    <div class="setting-row" role="group" aria-label="Who writes the reply">
      <button
        class="mini"
        class:active={replyLane === "grok"}
        aria-pressed={replyLane === "grok"}
        onclick={() => pickReply("grok")}
        title="Paid Grok via your existing grok CLI login"
      >Grok (paid)</button>
      <button
        class="mini"
        class:active={replyLane === "local"}
        aria-pressed={replyLane === "local"}
        onclick={() => pickReply("local")}
        title="A model already running on this Mac"
      >Local</button>
      <button
        class="mini"
        class:active={replyLane === "openrouter"}
        aria-pressed={replyLane === "openrouter"}
        onclick={() => pickReply("openrouter")}
        title="OpenRouter free models — needs a key"
      >OpenRouter free</button>
      <button
        class="mini"
        class:active={replyLane === "free"}
        aria-pressed={replyLane === "free"}
        onclick={() => pickReply("free")}
        title="Local first, then OpenRouter free. Paid Grok never runs."
      >Free Mode</button>
    </div>
    {#if showLocalPick}
      {#if models.length > 0}
        <div class="setting-row">
          <select
            class="bodypick"
            bind:value={replyModel}
            onchange={pickLocalModel}
            aria-label="Local model"
          >
            <option value="">Auto — prefer Gemma 4 E4B</option>
            {#each models as m (m.id + m.base)}
              <option value={m.id}>
                {m.id === QUICK_MOUTH ? "Quick mouth — google/gemma-4-e4b" : m.label}
              </option>
            {/each}
          </select>
        </div>
        {#if replyModel === QUICK_MOUTH}
          <p class="help">Quick mouth pinned: google/gemma-4-e4b</p>
        {/if}
      {:else}
        <p class="help">{emptyHint}</p>
      {/if}
    {/if}
    {#if suggest}
      <p class="help">
        Fits this Mac: {suggest.name}. {suggest.why}
        {#if canPull && suggest.ollamaTag}
          <button
            class="mini"
            disabled={pullBusy}
            onclick={() => pullSuggest(suggest.ollamaTag ?? "")}
            title="ollama pull — only if you click"
          >Download</button>
        {/if}
      </p>
    {/if}
    {#if pullNote}
      <p class="help">{pullNote}</p>
    {/if}
    <p class="help">
      Who writes the real English. Free Mode never bills Grok.
    </p>
  </div>
</div>

<div class="mouth-row">
  <span class="mouth-k">Quick ack</span>
  <div class="mouth-v">
    <div class="setting-row" role="group" aria-label="Quick ack">
      <button
        class="mini"
        class:active={!conductorOn}
        aria-pressed={!conductorOn}
        onclick={() => pickAck(false)}
        title="No fast ack — the reply is the first word"
      >Off</button>
      <button
        class="mini"
        class:active={conductorOn}
        aria-pressed={conductorOn}
        onclick={() => pickAck(true)}
        title="1B sidecar if it is up, else a tiny classifier. Never Grok."
      >Built-in</button>
    </div>
    {#if scan?.nextAck}
      <p class="help">Next: {scan.nextAck} — not used as Quick ack yet.</p>
    {/if}
    <p class="help">
      A fast “heard you” before the real reply. Built-in is the 1B sidecar
      on this Mac when it is hot; otherwise the tiny classifier. Never Grok.
    </p>
  </div>
</div>

<div class="mouth-row">
  <span class="mouth-k">Handoff</span>
  <div class="mouth-v">
    <div class="setting-row">
      <select
        class="bodypick"
        bind:value={handoffBackend}
        onchange={saveHandoffBackend}
        aria-label="Online agent for handoff"
      >
        <option value="">Ask me each time</option>
        {#each handoffChoices as b (b.id)}
          <option value={b.id}>{b.label}</option>
        {/each}
        {#if handoffBackend && !handoffChoices.some((b) => b.id === handoffBackend)}
          <option value={handoffBackend}>{handoffBackend}</option>
        {/if}
      </select>
    </div>
    <div class="setting-row" role="group" aria-label="Auto-handoff when the local model can't">
      <button
        class="mini"
        class:active={!autoHandoff}
        aria-pressed={!autoHandoff}
        onclick={() => saveAutoHandoff(false)}
        title="Show Ask online agent and wait for a click"
      >Auto off</button>
      <button
        class="mini"
        class:active={autoHandoff}
        aria-pressed={autoHandoff}
        onclick={() => saveAutoHandoff(true)}
        title="Send to the online agent above when the local model can't"
      >Auto on</button>
    </div>
    <p class="help">
      Online agent for handoff. When a local model can't do the ask, the
      chat offers Ask online agent. Auto sends it straight there. Default off.
    </p>
  </div>
</div>

<div class="mouth-row">
  <span class="mouth-k">Keys</span>
  <div class="mouth-v">
    <div class="setting-row">
      <input
        class="keyin"
        type="password"
        bind:value={orKeyDraft}
        placeholder="OpenRouter key (optional)"
        aria-label="OpenRouter API key"
        onkeydown={(e) => e.key === "Enter" && saveOrKey()}
      />
      <button
        class="mini"
        disabled={orBusy || orKeyDraft.trim().length === 0}
        onclick={saveOrKey}
        title="Save the key locally — never in state.json"
      >Save</button>
      {#if orKey.set && orKey.source === "file"}
        <button
          class="mini ghost"
          disabled={orBusy}
          onclick={clearOrKey}
          title="Delete the saved key file"
        >Clear</button>
      {/if}
    </div>
    {#if orNote}
      <p class="help err">{orNote}</p>
    {/if}
    <p class="help">
      {#if orKey.set}
        OpenRouter key {orKey.source === "env" ? "from env" : "saved locally"}
        (…{orKey.tail}). Grok uses your existing CLI login.
      {:else}
        OpenRouter is optional. Grok uses your existing CLI login — no key here.
      {/if}
    </p>
  </div>
</div>

<style>
  .block-label {
    margin: 8px 0 4px;
    font-size: 0.64rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--dim);
  }
  .mouth-row {
    display: grid;
    grid-template-columns: 5.6rem 1fr;
    gap: 4px 10px;
    margin-top: 8px;
    align-items: start;
  }
  .mouth-k {
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--ink);
    padding-top: 4px;
  }
  .mouth-v {
    min-width: 0;
  }
  .setting-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }
  .mini {
    font: inherit;
    font-size: 0.68rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 3px 8px;
    cursor: pointer;
  }
  .mini:hover {
    border-color: var(--accent);
  }
  .mini.active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--bg);
    font-weight: 700;
  }
  .mini.ghost {
    background: transparent;
  }
  .mini:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .bodypick {
    margin-top: 6px;
    font: inherit;
    font-size: 0.72rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 3px 8px;
    cursor: pointer;
    outline: none;
    max-width: 100%;
  }
  .bodypick:focus {
    border-color: var(--accent);
  }
  .keyin {
    flex: 1;
    min-width: 140px;
    font: inherit;
    font-size: 0.72rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 3px 8px;
    outline: none;
  }
  .keyin:focus {
    border-color: var(--accent);
  }
  .help {
    margin: 6px 0 0;
    font-size: 0.64rem;
    line-height: 1.5;
    color: var(--dim);
  }
  .help.err {
    color: var(--accent);
  }
</style>
