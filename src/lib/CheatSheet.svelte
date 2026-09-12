<script lang="ts">
  // Glass chord book. ⌘? toggles. Click a named action to run it.
  // Dest chords are listed; they are already global. Not a Settings tab.
  import { invoke } from "@tauri-apps/api/core";
  import type { ActionShortcut } from "./persist";

  let {
    open = false,
    actions = [],
    onClose,
  }: {
    open?: boolean;
    actions?: ActionShortcut[];
    onClose: () => void;
  } = $props();

  function comboGlyphs(c: string): string {
    return c
      .split("+")
      .map((t) => {
        const k = t.trim().toLowerCase();
        if (["cmd", "command", "super", "meta"].includes(k)) return "⌘";
        if (k === "shift") return "⇧";
        if (["alt", "option", "opt"].includes(k)) return "⌥";
        if (["ctrl", "control"].includes(k)) return "⌃";
        if (k === "space") return "Space";
        if (["return", "enter"].includes(k)) return "↩";
        if (["up", "arrowup"].includes(k)) return "↑";
        if (["down", "arrowdown"].includes(k)) return "↓";
        if (["left", "arrowleft"].includes(k)) return "←";
        if (["right", "arrowright"].includes(k)) return "→";
        return k.length === 1 ? k.toUpperCase() : k;
      })
      .join("");
  }

  const FIXED: { chord: string; label: string }[] = [
    { chord: "⌘⇧O", label: "Companion" },
    { chord: "⌘⌥1", label: "Launchpad" },
    { chord: "⌘⌥2", label: "STARS" },
    { chord: "⌘⌥3", label: "Planets" },
    { chord: "⌘⌥← / →", label: "Yaw dests" },
    { chord: "⌘⌥↓", label: "Dismiss dest" },
    { chord: "⌘⌥↑", label: "Last dest" },
    { chord: "⌘?", label: "This sheet" },
  ];

  let named = $derived(actions.filter((a) => a.combo && a.combo.trim()));
  let runningId = $state<string | null>(null);

  async function runAction(a: ActionShortcut) {
    if (runningId) return;
    runningId = a.id;
    try {
      await invoke("run_action", { command: a.command, cwd: a.cwd ?? null });
    } catch {
      /* ctl / log owns the honest error */
    }
    runningId = null;
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  }
</script>

{#if open}
  <div
    class="sheet"
    role="dialog"
    aria-label="Cheat sheet"
    tabindex="-1"
    onkeydown={onKey}
    onclick={(e) => {
      if (e.target === e.currentTarget) onClose();
    }}
  >
    <div class="card">
      <header class="head">
        <span>Chords</span>
        <button class="x" onclick={onClose} aria-label="Close">×</button>
      </header>
      <ul>
        {#each FIXED as row}
          <li>
            <span class="chord">{row.chord}</span>
            <span class="lab">{row.label}</span>
          </li>
        {/each}
        {#each named as a (a.id)}
          <li>
            <button
              class="run"
              class:on={runningId === a.id}
              onclick={() => runAction(a)}
              title={a.command}
            >
              <span class="chord">{comboGlyphs(a.combo ?? "")}</span>
              <span class="lab">{a.name}</span>
            </button>
          </li>
        {/each}
      </ul>
    </div>
  </div>
{/if}

<style>
  .sheet {
    position: absolute;
    inset: 0;
    z-index: 20;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    padding: 12px 12px 72px;
    background: rgb(0 0 0 / 0.18);
  }
  .card {
    width: min(360px, 100%);
    max-height: 70%;
    overflow: auto;
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 12px;
    color: var(--ink);
    box-shadow: 0 12px 40px rgb(0 0 0 / 0.35);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px 6px;
    font-size: 0.78rem;
    font-weight: 700;
    letter-spacing: 0.04em;
  }
  .x {
    border: none;
    background: transparent;
    color: var(--dim);
    cursor: pointer;
    font: inherit;
    font-size: 1rem;
    line-height: 1;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 8px 10px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 6px;
    font-size: 0.8rem;
  }
  .chord {
    flex: none;
    min-width: 5.2rem;
    font-variant-numeric: tabular-nums;
    color: var(--accent);
  }
  .lab {
    color: var(--ink);
  }
  .run {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    margin: 0;
    padding: 5px 6px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .run:hover {
    background: var(--panel-fill, var(--panel));
  }
</style>
