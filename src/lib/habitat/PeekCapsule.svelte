<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";

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
  let followBottom = true;
  let watchedBody: HTMLPreElement | null = null;
  let pinning = false;
  let downStreak = 0;

  function stop() {
    if (timer) clearInterval(timer);
    timer = undefined;
  }

  function nearBottom(el: HTMLElement): boolean {
    return el.scrollHeight - el.scrollTop - el.clientHeight <= 2;
  }

  function watchBody(el: HTMLPreElement) {
    if (watchedBody === el) return;
    watchedBody = el;
    el.addEventListener("scroll", () => {
      if (pinning) return;
      followBottom = nearBottom(el);
    });
  }

  async function stickToBottom() {
    const follow = followBottom;
    await tick();
    const found = document.querySelector("pre.body");
    if (!(found instanceof HTMLPreElement)) return;
    watchBody(found);
    if (!follow) return;
    pinning = true;
    found.scrollTop = found.scrollHeight;
    pinning = false;
    followBottom = true;
  }

  async function refresh() {
    if (!shown || !card) return;
    void invoke("reclamp_peek_capsule").catch(() => {});
    try {
      const v = await invoke<Capsule>("herdr_peek", { card, lines: 40 });
      status = v.status || "unknown";
      termState = v.state || "";
      if (preview) text = status === "unknown" ? "" : v.text || "";
      noteSeat(status, v.up);
    } catch {
      status = "unknown";
      if (preview) text = "";
      noteSeat("unknown", false);
    }
    if (preview) await stickToBottom();
  }

  function arm() {
    stop();
    if (shown) timer = setInterval(() => void refresh(), 2000);
  }

  function openPeek() {
    followBottom = true;
    preview = true;
    arm();
    void refresh();
  }

  function hideCapsule() {
    shown = false;
    preview = false;
    text = "";
    followBottom = true;
    watchedBody = null;
    downStreak = 0;
    stop();
    void invoke("hide_peek_capsule").catch(() => {});
  }

  function noteSeat(status: string, up: boolean) {
    if (status === "none") {
      void hideCapsule();
      return;
    }
    if (status === "blocked") {
      downStreak = 0;
      return;
    }
    if (up === false || status === "unknown") {
      downStreak += 1;
      if (downStreak >= 2) void hideCapsule();
      return;
    }
    downStreak = 0;
  }

  function closePeek() {
    preview = false;
    text = "";
    followBottom = true;
    watchedBody = null;
  }

  async function openTerminal() {
    if (!card) {
      status = "Nothing to open for this card.";
      return;
    }
    try {
      const v = await invoke<{ ok?: boolean; reason?: string }>("herdr_jump", { card });
      if (v && v.ok === false) {
        status = v.reason || "Couldn't open the terminal.";
      }
    } catch (e) {
      status = typeof e === "string" && e.trim() ? e : "Couldn't open the terminal.";
    }
  }

  function applyCapsule(v: Capsule) {
    card = v.card || "";
    status = v.status || "unknown";
    termState = v.state || "";
    text = "";
    preview = false;
    followBottom = true;
    watchedBody = null;
    downStreak = 0;
    shown = true;
    arm();
  }

  onMount(() => {
    let unlistenPeek: (() => void) | undefined;
    let unlistenVis: (() => void) | undefined;
    void listen<Capsule>("herdr-peek", (ev) => {
      applyCapsule(ev.payload);
    }).then((fn) => {
      unlistenPeek = fn;
    });
    void listen<{ label?: string; visible?: boolean }>("orbitty:pane-vis", (ev) => {
      if (ev.payload.label !== "peek-capsule" || ev.payload.visible !== false) return;
      shown = false;
      preview = false;
      stop();
      downStreak = 0;
    }).then((fn) => {
      unlistenVis = fn;
    });
    void invoke<Capsule | null>("peek_capsule_arm")
      .then((arm) => {
        if (arm && arm.card) applyCapsule(arm);
      })
      .catch(() => {});
    return () => {
      stop();
      unlistenPeek?.();
      unlistenVis?.();
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
      {#if status !== "unknown"}
        <button type="button" class="peek-btn" onclick={openTerminal}>Open terminal</button>
      {/if}
      <button type="button" class="peek-btn" onclick={hideCapsule}>hide</button>
    </div>
    {#if preview}
      <pre class="body" tabindex="-1">{text}</pre>
    {/if}
  </div>
{/if}

<style>
  .peek {
    box-sizing: border-box;
    width: 100%;
    height: 100%;
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
