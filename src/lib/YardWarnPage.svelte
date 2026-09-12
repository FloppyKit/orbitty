<script lang="ts">
  // GO-ORBITTY-YARD-WARN + GO-ORBITTY-HERDING-CATS: the Herding Cats
  // confirm. ⌘⇧Y or Settings → Appearance → Herding Cats… opens this own
  // small window (so the shortcut works with Settings closed). Esc / ⌘⇧Y /
  // Cancel / × all scoop = stop the yard if it is on, then hide. Drop the
  // cats (phrase #2) starts the
  // yard: unclaimed windows rain onto the desktop as bitty cats until
  // claimed, closed, or scooped — session only, planets stay empty of
  // unmapped sprites. window.confirm is dead on the companion overlay;
  // this is a real window, never a JS dialog.
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";
  import { startPaneDrag } from "./dragPane";
  import {
    applyHostTokens,
    applyWindowOpacity,
  } from "../themes/apply";
  import type { PersistedState, WindowInfo } from "./persist";

  const THEME_IDS = new Set(["systemLight", "systemDark", "terminal"]);

  function osThemeId(): string {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "systemDark"
      : "systemLight";
  }

  // GO-ORBITTY-PANE-PAINT: same first-paint discipline as Settings — seed
  // the OS theme synchronously so the chrome never flashes white.
  let themeId = $state(osThemeId());
  let windowOpacity = $state(1.0);

  // N = unclaimed windows, the SAME set as the inspector's Unclaimed
  // section (RailPage): list_windows already drops Orbitty/LiveSprite
  // windows native-side, and a claim counts only while its project
  // exists. null = still counting.
  let unclaimed = $state<number | null>(null);
  let listErr = $state(false);
  // Two independent checkboxes persist as yardWindows / yardTabs; tabs
  // stay disabled until Tabbity. The old yardSource radio key is healed
  // away on load. No yard-on flag persists — the yard is session only.
  let yardWindows = $state(true);
  let yardTabs = $state(false);
  // Drop arms when a source is on AND there is something to herd.
  const noneOn = $derived(!yardWindows && !yardTabs);
  const dropReady = $derived(!noneOn && !listErr && (unclaimed ?? 0) > 0);
  let dropping = $state(false);

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

  async function countUnclaimed() {
    try {
      const s = await invoke<PersistedState>("load_state");
      if (THEME_IDS.has(s.themeId)) themeId = s.themeId;
      windowOpacity = s.windowOpacity;
      // Heal pre-checkbox saves: missing bools (from the yardSource
      // radio era) mean windows on / tabs off; write the heal back once.
      if (s.yardWindows === undefined || s.yardTabs === undefined) {
        s.yardWindows = s.yardWindows ?? true;
        s.yardTabs = s.yardTabs ?? false;
        invoke("save_state", { state: s }).catch(() => {});
      }
      yardWindows = s.yardWindows ?? true;
      yardTabs = s.yardTabs ?? false;
      const windows = await invoke<WindowInfo[]>("list_windows");
      const liveClaims = (s.claims ?? []).filter((c) =>
        (s.projects ?? []).some((p) => p.id === c.projectId),
      );
      unclaimed = windows.filter(
        (w) => !liveClaims.some((c) => c.windowId === w.id),
      ).length;
      listErr = false;
    } catch {
      // Accessibility off (or state unreadable) — say so, never fake N.
      listErr = true;
      unclaimed = null;
    }
  }

  function persistSources() {
    // Checkboxes are independent; write both bools so the settings fields
    // are the one honest record. Tabs can't flip yet (disabled), but its
    // persisted false is still the truth.
    invoke<PersistedState>("load_state")
      .then((s) => {
        s.yardWindows = yardWindows;
        s.yardTabs = yardTabs;
        return invoke("save_state", { state: s });
      })
      .catch(() => {});
  }

  function cancel() {
    // Scoop = Cancel: if the clowder is out, scoop it first — yard_stop
    // is an idempotent no-op when the yard is off. Post-Drop the pane is
    // already hidden, but a still-up pane must never leave cats running.
    invoke("yard_stop").catch(() => {});
    invoke("hide_window", { label: "yard-warn" }).catch(() => {});
  }

  // GO-ORBITTY-HERDING-CATS: Drop starts the yard native-side (the
  // current unclaimed set rains in, staggered), then the pane gets out
  // of the way so the clowder is visible. Drop on an already-on yard is
  // a native no-op — never a double spawn.
  async function dropCats() {
    if (!dropReady || dropping) return;
    dropping = true;
    try {
      await invoke("yard_start");
      invoke("hide_window", { label: "yard-warn" }).catch(() => {});
    } catch {
      dropping = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      cancel();
    }
  }

  // Re-count on every re-show — the window is hide-not-destroy, so a
  // stale N from the last open would lie. While open, N updates live
  // (2s beat; the same list_windows the rail already polls at 800ms).
  const onVisibility = () => {
    if (!document.hidden) countUnclaimed();
  };

  let countTimer: ReturnType<typeof setInterval> | undefined;

  onMount(() => {
    invoke("ensure_glass").catch(() => {});
    countUnclaimed();
    countTimer = setInterval(() => {
      if (!document.hidden) countUnclaimed();
    }, 2000);
    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("keydown", onKeydown, true);
  });
  onDestroy(() => {
    if (countTimer) clearInterval(countTimer);
    document.removeEventListener("visibilitychange", onVisibility);
    window.removeEventListener("keydown", onKeydown, true);
  });

  // One drag source: JS startDragging on the banner (borderless pane).
  function onBannerMouseDown(e: MouseEvent) {
    startPaneDrag(e);
  }
</script>

<main class="yardwarn">
  <header class="s-banner" onmousedown={onBannerMouseDown}>
    <button
      class="winx"
      onmousedown={(e) => e.stopPropagation()}
      onclick={cancel}
      title="Scoop — hide this pane (Esc or ⌘⇧Y works too)"
      aria-label="Close Herding Cats">×</button
    >
    <h1>Herding Cats?</h1>
  </header>

  <div class="s-body">
    <section class="panel">
      {#if listErr}
        <p class="copy">
          Orbitty can't see windows without Accessibility access — nothing
          to herd.
        </p>
      {:else if unclaimed === null}
        <p class="copy">Counting unclaimed windows…</p>
      {:else if unclaimed === 0}
        <p class="copy">
          Nothing to herd — every window has a planet. Esc or ⌘⇧Y closes
          this pane.
        </p>
      {:else}
        <p class="copy">
          Rain a cat onto the desktop for every <strong>unclaimed</strong>
          window ({unclaimed}). They are not claimed. Planets stay empty.
          Close a window or assign it to a planet to retire its cat. Esc or
          ⌘⇧Y scoops them without closing anything.
        </p>
      {/if}

      <fieldset class="source">
        <legend class="block-label">Herd from</legend>
        <label class="check">
          <input
            type="checkbox"
            bind:checked={yardWindows}
            onchange={persistSources}
          />
          Unclaimed windows
        </label>
        <label class="check off" title="needs Tabbity">
          <input type="checkbox" disabled bind:checked={yardTabs} />
          Unclaimed browser tabs
        </label>
      </fieldset>

      <div class="buttons">
        <button
          class="mini"
          disabled={!dropReady || dropping}
          onclick={dropCats}
          title={dropReady
            ? "rain a cat per unclaimed window — Esc or ⌘⇧Y scoops"
            : noneOn
              ? "pick windows or tabs"
              : "nothing to herd"}>Drop the cats</button
        >
        <button class="mini" onclick={cancel}>Cancel</button>
      </div>
      <p class="help"
        >{noneOn
          ? "pick windows or tabs"
          : dropReady
            ? "cats meow until claimed, closed, or scooped"
            : "nothing to herd"}</p
      >
    </section>
  </div>
</main>

<style>
  /* Same chrome recipe as SettingsPage: theme fill at the slider alpha,
     text/icons never fade. */
  .yardwarn {
    height: 100vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border-radius: 12px;
    font-family:
      "SF Pro Text", "SF Pro Display", system-ui, -apple-system,
      BlinkMacSystemFont, "Helvetica Neue", sans-serif;
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
  .s-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 12px 16px;
  }
  .panel {
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 10px;
    padding: 10px 12px 12px;
  }
  .copy {
    margin: 2px 0 4px;
    font-size: 0.74rem;
    line-height: 1.5;
  }
  .block-label {
    margin: 8px 0 4px;
    font-size: 0.64rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--dim);
  }
  .source {
    margin: 8px 0 0;
    padding: 0;
    border: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .source legend {
    padding: 0;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.72rem;
  }
  .check.off {
    color: var(--dim);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    margin-top: 12px;
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
  .mini:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .mini:disabled:hover {
    border-color: var(--edge);
  }
  .help {
    margin: 6px 0 0;
    font-size: 0.64rem;
    line-height: 1.5;
    color: var(--dim);
    text-align: right;
  }
</style>
