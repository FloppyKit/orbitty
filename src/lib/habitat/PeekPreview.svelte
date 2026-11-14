<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
  import { startPaneDrag } from "../dragPane";

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
  let text = $state("");
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
    void invoke("reclamp_peek_preview").catch(() => {});
    try {
      const v = await invoke<Capsule>("herdr_peek", { card, lines: 40 });
      const next = v.status || "unknown";
      text = next === "unknown" ? "" : v.text || "";
      noteSeat(next, v.up);
    } catch {
      text = "";
      noteSeat("unknown", false);
    }
    await stickToBottom();
  }

  function arm() {
    stop();
    if (shown) timer = setInterval(() => void refresh(), 2000);
  }

  function openPeek() {
    followBottom = true;
    shown = true;
    arm();
    void refresh();
  }

  function closePeek() {
    shown = false;
    text = "";
    followBottom = true;
    watchedBody = null;
    stop();
    void invoke("hide_peek_preview").catch(() => {});
  }

  function hideBoth() {
    shown = false;
    text = "";
    followBottom = true;
    watchedBody = null;
    downStreak = 0;
    stop();
    void invoke("hide_peek_preview", { restore: false }).catch(() => {});
    void invoke("hide_peek_capsule").catch(() => {});
  }

  function noteSeat(status: string, up: boolean) {
    if (status === "none") {
      void hideBoth();
      return;
    }
    if (status === "blocked") {
      downStreak = 0;
      return;
    }
    if (up === false || status === "unknown") {
      downStreak += 1;
      if (downStreak >= 2) void hideBoth();
      return;
    }
    downStreak = 0;
  }

  function applyCapsule(v: Capsule) {
    card = v.card || "";
    text = "";
    downStreak = 0;
    openPeek();
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
      if (ev.payload.label !== "peek-preview" || ev.payload.visible !== false) return;
      shown = false;
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
  <div class="peek">
    <div class="strip" onmousedown={startPaneDrag}>
      <button
        type="button"
        class="peek-btn"
        onmousedown={(e) => e.stopPropagation()}
        onclick={closePeek}>close</button>
    </div>
    <pre class="body" tabindex="-1">{text}</pre>
  </div>
{/if}

<style>
  .peek {
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--panel, #12201a);
    color: var(--ink, #c8f0d0);
    border: 1px solid var(--edge, #1a2a20);
    border-radius: 8px;
    padding: 6px 10px 8px;
    font: 12px/1.4 ui-monospace, "SF Mono", Menlo, monospace;
  }
  .strip {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    height: 22px;
    cursor: grab;
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
    flex: 1;
    min-height: 0;
    max-height: 240px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
    scrollbar-width: auto;
    scrollbar-color: #5f8a6d #1b2e24;
  }
  .body::-webkit-scrollbar {
    width: 10px;
  }
  .body::-webkit-scrollbar-track {
    background: #1b2e24;
  }
  .body::-webkit-scrollbar-thumb {
    background: #5f8a6d;
    border-radius: 5px;
  }
</style>
