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
    GLASS_CARD_SELECTORS,
    GLASS_RECT_MIN_MS,
    nextWatchDelay,
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

  // Gated FR4 dump. Silent unless the dev launch set VITE_GLASS_FR4_DIAG=1.
  // Selector counts, computed card backgrounds, and opaque ancestors.
  // No titles, paths, or document text.
  $effect(() => {
    if (import.meta.env.VITE_GLASS_FR4_DIAG !== "1") return;
    if (!labelKnown || !windowLabel) return;
    const label = windowLabel;
    const safeBg = (bg: string) => {
      if (!bg) return "missing";
      if (bg.includes("url(") || bg.includes("http") || bg.includes("Users")) return "skipped";
      return bg.slice(0, 80);
    };
    const alphaOf = (bg: string): number | null => {
      if (!bg || bg === "missing" || bg === "skipped") return null;
      if (bg === "transparent") return 0;
      const slash = bg.match(/\/\s*([0-9.]+)\s*\)\s*$/);
      if (slash) return Number(slash[1]);
      if (bg.startsWith("rgba")) {
        const m = bg.match(
          /rgba\(\s*[0-9.]+\s*,\s*[0-9.]+\s*,\s*[0-9.]+\s*,\s*([0-9.]+)\s*\)/,
        );
        if (m) return Number(m[1]);
      }
      if (bg.startsWith("rgb(")) return 1;
      return null;
    };
    const classToken = (el: Element) => {
      const raw = (el as HTMLElement).className;
      const text = typeof raw === "string" ? raw : "";
      return text
        .replace(/[^A-Za-z0-9_-]+/g, ".")
        .replace(/^\.+|\.+$/g, "")
        .slice(0, 40);
    };
    const ancestorsOf = (el: Element) => {
      const bits: string[] = [];
      let p = el.parentElement;
      while (p && p !== document.documentElement && bits.length < 6) {
        const bg = safeBg(getComputedStyle(p).backgroundColor);
        const a = alphaOf(bg);
        const img = getComputedStyle(p).backgroundImage;
        const imgOn = !!img && img !== "none";
        if ((a != null && a > 0.02) || imgOn) {
          const tag = p.tagName.toLowerCase().replace(/[^a-z0-9]/g, "") || "el";
          const cls = classToken(p);
          const aText = a == null ? "na" : a.toFixed(3);
          bits.push(`${tag}${cls ? "." + cls : ""} a=${aText} bg=${bg} img=${imgOn ? 1 : 0}`);
        }
        p = p.parentElement;
      }
      return bits.length ? bits.join(";") : "none";
    };
    const collect = () => {
      const html = document.documentElement;
      const htmlBg = safeBg(getComputedStyle(html).backgroundColor);
      const bodyBg = safeBg(getComputedStyle(document.body).backgroundColor);
      const htmlA = alphaOf(htmlBg);
      const bodyA = alphaOf(bodyBg);
      const theme = (html.getAttribute("data-theme") || "")
        .replace(/[^A-Za-z0-9_-]/g, "")
        .slice(0, 32);
      const panelFill = safeBg(getComputedStyle(html).getPropertyValue("--panel-fill").trim());
      const rects = collectGlassCardRects(document, window.innerWidth, window.innerHeight);
      const sample = rects[0]
        ? `${rects[0].x.toFixed(1)},${rects[0].y.toFixed(1)},${rects[0].w.toFixed(1)}x${rects[0].h.toFixed(1)}`
        : "none";
      const rt = html.classList.contains("glass-rt");
      const lines = [
        `GLASS_FR4_DIAG css label=${label} rt=${rt} mediaReduce=${window.matchMedia("(prefers-reduced-transparency: reduce)").matches} solidClass=${html.classList.contains("glass-solid")} theme=${theme} collected=${rects.length} sample=${sample} vw=${Math.round(window.innerWidth)} vh=${Math.round(window.innerHeight)} html=${htmlBg} htmlA=${htmlA == null ? "na" : htmlA.toFixed(3)} body=${bodyBg} bodyA=${bodyA == null ? "na" : bodyA.toFixed(3)} panelFill=${panelFill || "missing"}`,
      ];
      for (const sel of GLASS_CARD_SELECTORS) {
        const nodes = Array.from(document.querySelectorAll(sel)).slice(0, 48);
        let firstBg = "missing";
        let firstA: number | null = null;
        let maxA = 0;
        let solidN = 0;
        let ancestors = "none";
        nodes.forEach((el, i) => {
          const bg = safeBg(getComputedStyle(el).backgroundColor);
          const a = alphaOf(bg);
          if (i === 0) {
            firstBg = bg;
            firstA = a;
            ancestors = ancestorsOf(el);
          }
          if (a != null && a > maxA) maxA = a;
          if (a != null && a >= 0.85) solidN += 1;
        });
        const alpha = nodes.length === 0 || firstA == null ? "na" : firstA.toFixed(3);
        const maxAlpha = nodes.length === 0 ? "na" : maxA.toFixed(3);
        lines.push(
          `GLASS_FR4_DIAG css-card label=${label} rt=${rt} sel=${sel} n=${document.querySelectorAll(sel).length} bg=${firstBg} alpha=${alpha} maxAlpha=${maxAlpha} solidN=${solidN} ancestors=${ancestors}`,
        );
      }
      return { lines };
    };
    (window as unknown as { __glassFr4Collect?: () => { lines: string[] } }).__glassFr4Collect =
      collect;
    const timers = [2000, 8000].map((ms) =>
      window.setTimeout(() => {
        for (const line of collect().lines) console.log(line);
      }, ms),
    );
    return () => {
      for (const t of timers) window.clearTimeout(t);
      delete (window as unknown as { __glassFr4Collect?: () => { lines: string[] } })
        .__glassFr4Collect;
    };
  });

  // Card rects for the native blur. Coalesced: unchanged lists are not sent,
  // and a moving list (scroll or spin) goes out at most every GLASS_RECT_MIN_MS.
  $effect(() => {
    if (!labelKnown) return;
    let prev = "";
    let last = 0;
    let streak = 0;
    let timer = 0;
    const flush = () => {
      const rects = collectGlassCardRects(document, window.innerWidth, window.innerHeight);
      const sig = rectSignature(rects);
      const now = performance.now();
      if (shouldSendGlassRects(prev, sig, now, last, GLASS_RECT_MIN_MS)) {
        prev = sig;
        last = now;
        streak = 0;
        invoke("set_glass_card_rects", { rects }).catch(() => {});
      } else {
        streak += 1;
      }
      timer = window.setTimeout(flush, nextWatchDelay(streak));
    };
    flush();
    return () => window.clearTimeout(timer);
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
    --glass-card-tint: 0.45;
    --glass-edge: rgba(0, 0, 0, 0.40);
  }
  :global(:root[data-theme="systemLight"]) {
    --glass-edge: rgba(0, 0, 0, 0.16);
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
  /* Reduce transparency does not paint the page root. Roots stay clear.
     Rust sets html.glass-rt from NSWorkspace (the webview media query does
     not match on this Mac). Tiles that are otherwise clear get the veil.
     Hover and the front row keep their own fills. */
  :global(html.glass-rt .row:not(:hover):not(.front)),
  :global(html.glass-rt .l2-window-card),
  :global(html.glass-rt .l2-needed-card),
  :global(html.glass-rt .l2-preview) {
    background: var(--panel-fill, rgba(44, 44, 46, 0.72));
  }

  /* Non-RT tint over the per-card blur. 45% of the theme panel:
     systemDark #2C2C2E, terminal #12201a, systemLight #FFFFFF.
     Terminal tracker cards stay transparent (their own rule wins).
     The stage unmapped well is clear so the row gaps are not a slab. */
  :global(html:not(.glass-rt) .row:not(:hover):not(.front):not(.on)),
  :global(html:not(.glass-rt) .b-card),
  :global(html:not(.glass-rt) .planet-col),
  :global(html:not(.glass-rt) .card),
  :global(html:not(.glass-rt) .ghostdock),
  :global(html:not(.glass-rt) .panel) {
    background: color-mix(in srgb, var(--panel) 45%, transparent);
  }
  :global(html:not(.glass-rt) .row:not(:hover):not(.front):not(.on)) {
    outline: 1px solid var(--glass-edge, rgba(0, 0, 0, 0.40));
    outline-offset: -1px;
  }
  :global(html:not(.glass-rt) .rail.stage-shell .layer2-unmapped) {
    background: transparent;
    box-shadow: none;
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
