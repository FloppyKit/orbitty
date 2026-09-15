<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import KanbanPage from "$lib/KanbanPage.svelte";
  import CompanionPage from "$lib/CompanionPage.svelte";
  import StarsPage from "$lib/StarsPage.svelte";
  import SettingsPage from "$lib/SettingsPage.svelte";
  import SkyPage from "$lib/SkyPage.svelte";
  import YardWarnPage from "$lib/YardWarnPage.svelte";
  import NextCard from "$lib/NextCard.svelte";
  import RailPage from "$lib/RailPage.svelte";
  import type { PersistedState, SharedPaneMode } from "$lib/persist";

  // The same SPA bundle serves the separate Kanban/Companion/STARS/Sky
  // windows; each mounts only its own page and never runs the rail
  // poll/persist loop.
  // The label read can NEVER throw — in a
  // runtime-created drawer webview __TAURI_INTERNALS__ can lag module init,
  // and a throw at module top kills the whole page module before Svelte
  // mounts (white slab, empty AXWebArea). Read the internals metadata
  // directly (the exact field getCurrentWindow() itself reads). If it is
  // not there yet the label stays unknown — rendering a dark slab, never
  // white, never rail chrome in a drawer — until onMount retries once via
  // a try/catch getCurrentWindow(), falling back to "main".
  const labelFromInternals = (): string | null => {
    const l = (window as any).__TAURI_INTERNALS__?.metadata?.currentWindow
      ?.label;
    return typeof l === "string" && l ? l : null;
  };
  const initialLabel = labelFromInternals();
  let windowLabel = $state<string | null>(initialLabel);
  onMount(() => {
    if (windowLabel != null) return;
    try {
      windowLabel = getCurrentWindow().label;
    } catch {
      windowLabel = "main";
    }
  });
  const isKanbanWindow = $derived(windowLabel === "kanban");
  const isCompanionWindow = $derived(windowLabel === "companion");
  const isStarsWindow = $derived(windowLabel === "stars");
  const isSettingsWindow = $derived(windowLabel === "settings");
  const isSkyWindow = $derived(windowLabel === "sky");
  // The Herding Cats confirm — own small window.
  const isYardWarnWindow = $derived(windowLabel === "yard-warn");
  const isNextCardWindow = $derived(windowLabel === "next-card");
  const isRailWindow = $derived(windowLabel === "main");
  // An unrecognized (or not-yet-resolved) label is never treated as the
  // rail — it renders the dark slab at the bottom of the markup.
  const labelKnown = $derived(
    isRailWindow ||
      isKanbanWindow ||
      isCompanionWindow ||
      isStarsWindow ||
      isSettingsWindow ||
      isSkyWindow ||
      isYardWarnWindow ||
      isNextCardWindow,
  );

  // Chrome drawers are opaque windows, not rail
  // glass — tag <html> synchronously (before first paint) so the drawer
  // rule in the style block paints the theme slab on html/body. The rail
  // and sky keep transparent html/body. When the
  // label resolves late (IPC lagged module init) the effect applies the
  // tag on resolution.
  const isDrawerWindow = $derived(
    isKanbanWindow ||
      isCompanionWindow ||
      isSettingsWindow ||
      isYardWarnWindow ||
      isNextCardWindow,
  );
  if (
    initialLabel === "kanban" ||
    initialLabel === "companion" ||
    initialLabel === "settings" ||
    initialLabel === "yard-warn" ||
    initialLabel === "next-card"
  ) {
    document.documentElement.dataset.drawer = "";
  }
  $effect(() => {
    if (isDrawerWindow) document.documentElement.dataset.drawer = "";
  });

  // The `kanban` webview is the ONE shared 780×520
  // overlay — Board mounts KanbanPage, STARS mounts StarsPage inside it.
  // The URL never carries the mode (a query 404s the
  // custom protocol) — native persists sharedPaneMode BEFORE show; warm
  // swaps arrive as `orbitty:shared-pane-mode`, and the load_state
  // reconcile on mount covers a cold webview. Last mode persists
  // native-side, so a bare re-show (tray Lists) restores it.
  // Seed "board" before first paint — a null mode
  // renders nothing, and a transparent window rendering nothing still
  // hit-tests (the invisible click-eater over the rail). Belt-and-braces:
  // while the mode is somehow null the overlay ignores the mouse entirely.
  let paneMode = $state<SharedPaneMode | null>("board");
  $effect(() => {
    if (!isKanbanWindow) return;
    getCurrentWindow()
      .setIgnoreCursorEvents(paneMode == null)
      .catch(() => {});
  });



  // Shared-pane mode channel (kanban window only): warm swaps land as the
  // event; the reconcile covers a create/load race where the event fired
  // before this webview finished loading.
  let unlistenPaneMode: (() => void) | undefined;
  onMount(() => {
    if (!isKanbanWindow) return;
    let dead = false;
    let liveFromNative = false;
    listen<SharedPaneMode>("orbitty:shared-pane-mode", (e) => {
      if (e.payload === "board" || e.payload === "stars") {
        liveFromNative = true;
        paneMode = e.payload;
      }
    }).then((un) => {
      if (dead) un();
      else unlistenPaneMode = un;
    });
    invoke<PersistedState>("load_state")
      .then((s) => {
        // A live emit already owns the mode — do not let a late load_state
        // snap Board back to last-saved Stars (or the reverse).
        if (liveFromNative) return;
        const m: SharedPaneMode = s.sharedPaneMode === "stars" ? "stars" : "board";
        if (m !== paneMode) paneMode = m;
      })
      .catch(() => {
        paneMode ??= "board";
      });
    return () => {
      dead = true;
    };
  });
</script>

{#if !labelKnown}
  <!-- Label unknown or unrecognized — dark slab,
       never white, never rail chrome in a drawer. -->
  <div class="spa-slab" aria-hidden="true"></div>
{:else if isKanbanWindow}
  {#if paneMode === "stars"}
    <StarsPage hideLabel="kanban" />
  {:else if paneMode === "board"}
    <KanbanPage />
  {/if}
{:else if isCompanionWindow}
  <CompanionPage />
{:else if isStarsWindow}
  <StarsPage />
{:else if isSettingsWindow}
  <SettingsPage />
{:else if isSkyWindow}
  <SkyPage />
{:else if isYardWarnWindow}
  <YardWarnPage />
{:else if isNextCardWindow}
  <NextCard />
{:else}
  <RailPage />
{/if}

<style>
  /* System pair (systemLight / systemDark): tokens are bound from the Tahoe
     semantic table in src/themes/host.ts and applied inline at runtime —
     brand accent/mark/focus. The one Custom
     skin below (Terminal) stays pure CSS. */
  :global(:root[data-theme="terminal"]) {
    --bg: #0a120e;
    --panel: #12201a;
    --edge: #1a2a20;
    --ink: #c8f0d0;
    --dim: #5f8a6d;
    --accent: #3ddc84;
    --on-accent: #ffffff;
  }
  /* S4.4: html paints nothing either — the only fills anywhere are the
     JS-computed --bg-fill/--panel-fill alpha vars. */
  :global(html),
  :global(body) {
    height: 100%;
    overflow: hidden;
    background: transparent;
  }

  /* Window label not yet resolved (IPC lagged module
     init in a runtime-created drawer) or unrecognized — paint the dark
     slab, never white, never rail chrome. */
  .spa-slab {
    position: fixed;
    inset: 0;
    background: #1e1e1e;
  }

  /* The webview window is transparent; the rail paints the theme --bg at the
     slider's alpha (S4.2). Text/icons/panels never fade. */
  :global(body) {
    margin: 0;
    background: transparent;
    color: var(--ink);
    font-family:
      "SF Pro Text", "SF Pro Display", system-ui, -apple-system,
      "Helvetica Neue", sans-serif;
    -webkit-font-smoothing: antialiased;
  }
</style>
