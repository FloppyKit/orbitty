<script lang="ts">
  // GO-ORBITTY-WORD-SKY: an opt-in sky of the user's OWN words. The bag
  // law, mute (state.sky.muted), click/right-click, and the field itself
  // live in SkyField now — this page is only the window chrome (banner +
  // hide ×) plus theme/opacity, so STARS Review can mount the same field
  // without the Cloud banner. The field's four-layer background is the
  // shared .orbit-field recipe (src/lib/orbit-field.css), never forked.
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount, onDestroy } from "svelte";
  import SkyField from "./SkyField.svelte";
  import {
    applyHostTokens,
    applyWindowOpacity,
    clampWindowOpacity,
  } from "../themes/apply";
  import type { PersistedState } from "./persist";



  // GO-ORBITTY-COMPANION-BRAND: the seven palette skins are retired; a
  // dead saved id falls back to the OS pair. Terminal stays valid.
  const KNOWN_THEMES = new Set(["systemLight", "systemDark", "terminal"]);

  function osThemeId(): string {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "systemDark"
      : "systemLight";
  }

  let themeId = $state("");
  let windowOpacity = $state(1.0);

  async function loadState() {
    try {
      const s = await invoke<PersistedState>("load_state");
      themeId = KNOWN_THEMES.has(s.themeId) ? s.themeId : osThemeId();
      windowOpacity = clampWindowOpacity(s.windowOpacity ?? 1);
    } catch {
      // next state:changed / visibility refresh retries
    }
  }

  $effect(() => {
    if (!themeId) return;
    document.documentElement.dataset.theme = themeId;
    applyHostTokens(themeId);
    invoke("ensure_glass").catch(() => {});
  });

  $effect(() => {
    if (!themeId) return;
    applyWindowOpacity(windowOpacity);
  });

  // The window is created hidden at launch and hidden again on close — a
  // re-show reloads from disk. SkyField carries its own visibility reload
  // for the bag; this one is theme/opacity only.
  const onVisibility = () => {
    if (!document.hidden) loadState();
  };

  let unlisten: (() => void) | undefined;
  onMount(async () => {
    invoke("ensure_glass").catch(() => {});
    await loadState();
    unlisten = await listen("state:changed", loadState);
    document.addEventListener("visibilitychange", onVisibility);
  });
  onDestroy(() => {
    unlisten?.();
    document.removeEventListener("visibilitychange", onVisibility);
  });

  function onBannerMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    const t = e.target as HTMLElement | null;
    if (t?.closest("button, input, a, select, textarea")) return;
    e.preventDefault();
    getCurrentWindow()
      .startDragging()
      .catch(() => {});
  }
</script>

<main class="sky">
  <!-- One drag source: JS startDragging on this banner. Cloud is
       borderless — no Overlay strip competing with the click. -->
  <header class="s-banner" onmousedown={onBannerMouseDown}>
    <button
      class="winx"
      onmousedown={(e) => e.stopPropagation()}
      onclick={() => invoke("hide_window", { label: "sky" }).catch(() => {})}
      title="Hide Cloud"
      aria-label="Hide Cloud">×</button
    >
    <h1><span class="mark"><span class="o">O</span><span class="rest">rb</span><span class="idot">i</span><span class="pop">ty</span></span> Cloud</h1>
  </header>

  <div class="s-body">
    <SkyField />
  </div>
</main>

<style>
  .sky {
    height: 100vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    font-family:
      "SF Pro Text", "SF Pro Display", system-ui, -apple-system,
      BlinkMacSystemFont, "Helvetica Neue", sans-serif;
  }

  :global(.low-fill) .sky {
    text-shadow: 0 1px 2px var(--bg-solid, transparent);
  }

  .s-banner {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 14px 0 10px;
    border-bottom: 1px solid var(--edge);
    user-select: none;
    -webkit-user-select: none;
    cursor: grab;
  }
  .s-banner:active {
    cursor: grabbing;
  }

  /* The only close affordance: hides Sky, never quits (tray → Quit). */
  .winx {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    font: inherit;
    font-size: 0.82rem;
    line-height: 1;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 999px;
    cursor: pointer;
  }

  .winx:hover {
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
  }

  .s-banner h1 {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  /* GO-ORBITTY-WORDMARK: ochre i-dot / ink O / accent ty. */
  .mark {
    text-shadow:
      1px 1px 0 rgba(0, 0, 0, 0.28),
      -1px -1px 0 rgba(255, 255, 255, 0.09);
  }

  .o {
    color: var(--ink);
    text-shadow:
      1px 1px 0 rgba(0, 0, 0, 0.42),
      -1px -1px 0 rgba(255, 255, 255, 0.14);
  }

  .idot {
    color: #e59e3d;
    text-shadow:
      1px 1px 0 rgba(0, 0, 0, 0.42),
      -1px -1px 0 rgba(255, 255, 255, 0.14);
  }

  .rest {
    color: var(--ink);
  }

  .pop {
    color: var(--accent);
  }

  .s-body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    position: relative;
    padding: 12px 14px 14px;
    display: flex;
    flex-direction: column;
  }
</style>
