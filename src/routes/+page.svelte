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
  import {
    collectGlassCardRects,
    GLASS_RECT_MIN_MS,
    rectSignature,
    shouldSendGlassRects,
  } from "$lib/glass/cardFrames";

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

  // Card rects for the native blur. Coalesced: unchanged lists are not sent,
  // and a moving list (scroll or spin) goes out at most every GLASS_RECT_MIN_MS.
  $effect(() => {
    if (!labelKnown) return;
    let prev = "";
    let last = 0;
    const flush = () => {
      const rects = collectGlassCardRects(document, window.innerWidth, window.innerHeight);
      const sig = rectSignature(rects);
      const now = performance.now();
      if (!shouldSendGlassRects(prev, sig, now, last, GLASS_RECT_MIN_MS)) return;
      prev = sig;
      last = now;
      invoke("set_glass_card_rects", { rects }).catch(() => {});
    };
    flush();
    const id = window.setInterval(flush, GLASS_RECT_MIN_MS);
    return () => window.clearInterval(id);
  });

  // Gated FR3 card-rect dump. Silent unless the dev launch set
  // VITE_GLASS_FR3_DIAG=1. No titles, paths, or document text.
  $effect(() => {
    const env = import.meta.env as { VITE_GLASS_FR3_DIAG?: string };
    if (env.VITE_GLASS_FR3_DIAG !== "1") return;
    if (!labelKnown || !windowLabel) return;
    const label = windowLabel;
    const r1 = (n: number) => Math.round(n * 10) / 10;
    const bgOf = (el: Element | null) =>
      el ? getComputedStyle(el).backgroundColor : "missing";
    const bfOf = (el: Element | null) => {
      if (!el) return "missing";
      const s = getComputedStyle(el);
      const a = s.getPropertyValue("backdrop-filter").trim();
      const b = s.getPropertyValue("-webkit-backdrop-filter").trim();
      if (a && a !== "none") return a;
      if (b && b !== "none") return b;
      return "none";
    };
    const rectsOf = (list: Element[]) =>
      list
        .map((el) => {
          const b = el.getBoundingClientRect();
          return { x: r1(b.x), y: r1(b.y), w: r1(b.width), h: r1(b.height) };
        })
        .filter((r) => r.w > 1 && r.h > 1)
        .slice(0, 8);
    const pack = (sels: string[]) =>
      sels.map((sel) => {
        const all = Array.from(document.querySelectorAll(sel));
        const first = all[0] ?? null;
        return { sel, n: all.length, bg: bgOf(first), bf: bfOf(first), rects: rectsOf(all) };
      });
    const collect = () => {
      const html = document.documentElement;
      const hs = getComputedStyle(html);
      const rootSels = [
        ".rail",
        ".kanban",
        ".stars",
        ".companion",
        ".settings",
        ".sky",
        ".yardwarn",
        "main.sheet",
        ".nextwin",
        ".spa-slab",
      ];
      const tileSels = [
        ".l2-window-card",
        ".l2-needed-card",
        ".l2-hit",
        ".l2-preview",
        ".tile",
        ".b-card",
        ".card",
        ".ghostdock",
        ".panel",
        ".planet-col",
        ".k-side",
        ".k-main",
        ".row",
        ".snap-card",
      ];
      const placeSels = [
        ".l2-window-card",
        ".l2-needed-card",
        ".b-card",
        ".card",
        ".ghostdock",
        ".panel",
        ".tile",
        ".snap-card",
        ".k-main",
        ".planet-col",
      ];
      const vw = window.innerWidth;
      const vh = window.innerHeight;
      const areaCap = vw * vh * 0.9;
      const place: { x: number; y: number; w: number; h: number }[] = [];
      for (const sel of placeSels) {
        for (const el of Array.from(document.querySelectorAll(sel))) {
          if (place.length >= 24) break;
          const b = el.getBoundingClientRect();
          const w = r1(b.width);
          const h = r1(b.height);
          if (w < 40 || h < 24) continue;
          if (w * h >= areaCap) continue;
          place.push({ x: r1(b.x), y: r1(b.y), w, h });
        }
      }
      let backdropSupports = false;
      try {
        backdropSupports =
          typeof CSS !== "undefined" &&
          typeof CSS.supports === "function" &&
          (CSS.supports("backdrop-filter: blur(1px)") ||
            CSS.supports("-webkit-backdrop-filter: blur(1px)"));
      } catch {
        backdropSupports = false;
      }
      return {
        label,
        html: bgOf(html),
        body: bgOf(document.body),
        roots: pack(rootSels),
        tiles: pack(tileSels),
        place,
        vw,
        vh,
        backdropSupports,
        reduce: window.matchMedia("(prefers-reduced-transparency: reduce)").matches,
        glassSolid: html.classList.contains("glass-solid"),
        theme: html.getAttribute("data-theme") || "",
        scheme: hs.getPropertyValue("color-scheme").trim(),
        bg: hs.getPropertyValue("--bg").trim(),
        panel: hs.getPropertyValue("--panel").trim(),
        panelFill: hs.getPropertyValue("--panel-fill").trim(),
        bgFill: hs.getPropertyValue("--bg-fill").trim(),
        veil: hs.getPropertyValue("--glass-veil").trim(),
      };
    };
    (window as unknown as { __glassFr3Collect?: () => unknown }).__glassFr3Collect = collect;
    const timers = [1200, 8000].map((ms) =>
      window.setTimeout(() => {
        console.log("GLASS_FR3_DIAG page-console", JSON.stringify(collect()));
      }, ms),
    );
    return () => {
      for (const t of timers) window.clearTimeout(t);
      delete (window as unknown as { __glassFr3Collect?: () => unknown }).__glassFr3Collect;
    };
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
  /* Vibrancy off: the page root is the theme fill, not a hole.
     The class is set from the same media query. */
  @media (prefers-reduced-transparency: reduce) {
    :global(html.glass-solid .rail),
    :global(html.glass-solid .rail.stage-shell),
    :global(html.glass-solid .kanban),
    :global(html.glass-solid .stars),
    :global(html.glass-solid .companion),
    :global(html.glass-solid .settings),
    :global(html.glass-solid .sky),
    :global(html.glass-solid main.sheet),
    :global(html.glass-solid .yardwarn),
    :global(html.glass-solid .nextwin) {
      background: var(--bg-fill, var(--bg));
    }
  }

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
