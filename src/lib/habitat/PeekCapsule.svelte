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
    outcome?: string;
  };

  let shown = $state(false);
  let card = $state("");
  let status = $state("unknown");
  let termState = $state("");
  let said = $state("");
  let timer: ReturnType<typeof setInterval> | undefined;
  let downStreak = 0;

  function stop() {
    if (timer) clearInterval(timer);
    timer = undefined;
  }

  async function refresh() {
    if (!shown || !card) return;
    void invoke("reclamp_peek_capsule").catch(() => {});
    try {
      const v = await invoke<Capsule>("herdr_peek", { card, lines: 40 });
      status = v.status || "unknown";
      termState = v.state || "";
      said = v.outcome || "";
      noteSeat(status, v.up);
    } catch {
      status = "unknown";
      said = "";
      noteSeat("unknown", false);
    }
  }

  function arm() {
    stop();
    if (shown) timer = setInterval(() => void refresh(), 2000);
  }

  function openPeek() {
    void invoke("show_peek_preview").catch(() => {});
  }

  function hideCapsule() {
    shown = false;
    downStreak = 0;
    stop();
    void invoke("hide_peek_preview", { restore: false }).catch(() => {});
    void invoke("hide_peek_capsule").catch(() => {});
  }

  function noteSeat(status: string, up: boolean) {
    if (status === "none") {
      if (said) return;
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

  async function openTerminal() {
    if (!card) {
      status = "Nothing to open for this card.";
      said = "";
      return;
    }
    try {
      const v = await invoke<{ ok?: boolean; reason?: string }>("herdr_jump", { card });
      if (v && v.ok === false) {
        status = v.reason || "Couldn't open the terminal.";
        said = "";
      }
    } catch (e) {
      status = typeof e === "string" && e.trim() ? e : "Couldn't open the terminal.";
      said = "";
    }
  }

  function applyCapsule(v: Capsule) {
    card = v.card || "";
    status = v.status || "unknown";
    termState = v.state || "";
    said = v.outcome || "";
    downStreak = 0;
    if (!said) noteSeat(status, v.up);
    if (status === "none" && !said) return;
    shown = true;
    arm();
  }

  onMount(() => {
    let unlistenPeek: (() => void) | undefined;
    let unlistenVis: (() => void) | undefined;
    void listen<Capsule>("herdr-peek", (ev) => {
      applyCapsule(ev.payload);
    }, { target: "peek-capsule" }).then((fn) => {
      unlistenPeek = fn;
    });
    void listen<{ label?: string; visible?: boolean }>("orbitty:pane-vis", (ev) => {
      if (ev.payload.label !== "peek-capsule" || ev.payload.visible !== false) return;
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
  <div class="peek" role="status">
    <div class="row">
      <span class="status">{#if said}{said}{:else}{status}{termState ? ` · ${termState}` : ""}{/if}</span>
      <button type="button" class="peek-btn" onclick={openPeek}>peek</button>
      {#if status !== "unknown"}
        <button type="button" class="peek-btn" onclick={openTerminal}>Open terminal</button>
      {/if}
      <button type="button" class="peek-btn" onclick={hideCapsule}>hide</button>
    </div>
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
    padding: 4px 8px;
    font: 12px/1.4 ui-monospace, "SF Mono", Menlo, monospace;
    overflow: hidden;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: nowrap;
    white-space: nowrap;
  }
  .status {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .peek-btn {
    flex: none;
    background: transparent;
    color: var(--ink, #c8f0d0);
    border: 1px solid var(--edge, #1a2a20);
    border-radius: 4px;
    padding: 2px 8px;
  }
</style>
