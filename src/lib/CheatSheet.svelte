<script lang="ts">
  // Own window (⌘? / dock pip). Click-off hides native-side.
  // Dest chords are listed; they are already global. Not a Settings tab.
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import { applyHostTokens, applyWindowOpacity, clampWindowOpacity } from "../themes/apply";
  import { startPaneDrag } from "./dragPane";
  import type { ActionShortcut, PersistedState } from "./persist";

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
    { chord: "⌘⇧O", label: "Reveal rail" },
    { chord: "⌘⌥1", label: "Launchpad" },
    { chord: "⌘⌥2", label: "STARS" },
    { chord: "⌘⌥3", label: "Planets" },
    { chord: "⌘⌥← / →", label: "Yaw dests" },
    { chord: "⌘⌥↓", label: "Dismiss dest" },
    { chord: "⌘⌥↑", label: "Last dest" },
    { chord: "⌘?", label: "This sheet" },
  ];

  let actions = $state<ActionShortcut[]>([]);
  let named = $derived(actions.filter((a) => a.combo && a.combo.trim()));
  let runningId = $state<string | null>(null);

  async function loadState() {
    try {
      const s = await invoke<PersistedState>("load_state");
      applyHostTokens(s.themeId || "");
      applyWindowOpacity(clampWindowOpacity(s.windowOpacity ?? 1));
      actions = s.actionShortcuts ?? [];
    } catch {
      /* keep last */
    }
  }

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

  function close() {
    invoke("hide_window", { label: "cheatsheet" }).catch(() => {});
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }

  let unlisten: (() => void) | undefined;
  onMount(async () => {
    invoke("ensure_glass").catch(() => {});
    window.addEventListener("keydown", onKey);
    await loadState();
    unlisten = await listen("state:changed", loadState);
  });
  onDestroy(() => {
    window.removeEventListener("keydown", onKey);
    unlisten?.();
  });
</script>

<main class="sheet" aria-label="Cheat sheet">
  <header class="head" onmousedown={startPaneDrag}>
    <span>Chords</span>
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
</main>

<style>
  .sheet {
    height: 100vh;
    box-sizing: border-box;
    overflow: auto;
    background: transparent;
    color: var(--ink);
    border-radius: 12px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 14px 8px;
    font-size: 0.78rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    cursor: grab;
    user-select: none;
    -webkit-user-select: none;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 10px 12px;
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
