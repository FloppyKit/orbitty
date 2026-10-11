<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount, tick } from "svelte";
  import KanbanPage from "$lib/KanbanPage.svelte";
  import CompanionPage from "$lib/CompanionPage.svelte";
  import StarsPage from "$lib/StarsPage.svelte";
  import SettingsPage from "$lib/SettingsPage.svelte";
  import SkyPage from "$lib/SkyPage.svelte";
  import YardWarnPage from "$lib/YardWarnPage.svelte";
  import NextCard from "$lib/NextCard.svelte";
  import CheatSheet from "$lib/CheatSheet.svelte";
  import RailPage from "$lib/RailPage.svelte";
  import LinkPreviewPopover from "$lib/LinkPreviewPopover.svelte";

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
  const isCheatSheetWindow = $derived(windowLabel === "cheatsheet");
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
      isNextCardWindow ||
      isCheatSheetWindow,
  );

  // Chrome drawers are opaque windows, not rail
  // glass — tag <html> synchronously (before first paint) so the drawer
  // rule in the style block paints the theme slab on html/body. The rail
  // and sky keep transparent html/body. When the
  // label resolves late (IPC lagged module init) the effect applies the
  // tag on resolution.
  const isDrawerWindow = $derived(
    isKanbanWindow ||
      isStarsWindow ||
      isCompanionWindow ||
      isSettingsWindow ||
      isYardWarnWindow ||
      isNextCardWindow ||
      isCheatSheetWindow,
  );
  if (
    initialLabel === "kanban" ||
    initialLabel === "stars" ||
    initialLabel === "companion" ||
    initialLabel === "settings" ||
    initialLabel === "yard-warn" ||
    initialLabel === "next-card" ||
    initialLabel === "cheatsheet"
  ) {
    document.documentElement.dataset.drawer = "";
  }
  $effect(() => {
    if (isDrawerWindow) document.documentElement.dataset.drawer = "";
  });

  // Tell native this page really mounted, once per document. The drawer
  // mount check trusts this instead of an AX read on the main thread
  // (the first AX read of a warm pane built its whole AX tree while
  // AppKit waited — the first-open Settings beachball). A reload/navigate
  // clears it natively; the fresh document reports again.
  let reportedMount = false;
  $effect(() => {
    if (!labelKnown || reportedMount) return;
    reportedMount = true;
    void tick().then(() => invoke("pane_mounted").catch(() => {}));
  });

  // Phase-0 inventory. Unset is silent. Label and pixel size only.
  let glassDiagLogged = false;
  $effect(() => {
    if (import.meta.env.VITE_GLASS_CARDS_DIAG !== "1" || !labelKnown || !windowLabel || glassDiagLogged) {
      return;
    }
    glassDiagLogged = true;
    console.info(
      `glass-cards-diag label=${windowLabel} material=page vibrancy=0 w=${window.innerWidth} h=${window.innerHeight}`,
    );
  });

</script>

{#if !labelKnown}
  <!-- Label unknown or unrecognized — dark slab,
       never white, never rail chrome in a drawer. -->
  <div class="spa-slab" aria-hidden="true"></div>
{:else if isKanbanWindow}
  <KanbanPage />
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
{:else if isCheatSheetWindow}
  <CheatSheet />
{:else}
  <RailPage />
{/if}
<LinkPreviewPopover />

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
    color-scheme: dark;
  }
  /* S4.4: html paints nothing either — the only fills anywhere are the
     JS-computed --bg-fill/--panel-fill alpha vars. */
  :global(:root) {
    --glass-veil: 0.72;
    --glass-glow: color-mix(in srgb, var(--accent) 45%, transparent);
  }
  :global(html),
  :global(body) {
    height: 100%;
    overflow: hidden;
    background: transparent;
    --focus-ring: 0 0 0 2px var(--bg), 0 0 0 4px var(--accent);
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
