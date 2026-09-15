<script lang="ts">
  // Collapsed CC + wait-in-slot +
  // bot click-expand overlay + expanded transcript + Speak-on pager.
  // CompanionPage mounts this; Send / dests / well / SpiritStage stay.
  // Today's bubble: Speak off = ~6-line scroll; Speak on = caps then
  // scroll + click-expand. Hosted CC = idle collapsed CC.

  import { invoke } from "@tauri-apps/api/core";
  import { onMount, onDestroy, tick } from "svelte";
  import { mdLite, fitPages } from "./mdLite";
  import type { CompanionTurn } from "./persist";

  let {
    expanded = false,
    hosted = false,
    userLine = "",
    botLine = "",
    sending = false,
    ackLine = "",
    turns = [],
    speak = false,
    draft = "",
    ready = false,
    onSpeakError,
    onOverlayChange,
  }: {
    expanded?: boolean;
    hosted?: boolean;
    userLine?: string;
    botLine?: string;
    sending?: boolean;
    // The conductor's fast ack, splashed over the
    // wait mark while the mouth answers. Ephemeral — never a turn.
    ackLine?: string;
    turns?: CompanionTurn[];
    speak?: boolean;
    draft?: string;
    ready?: boolean;
    onSpeakError?: (msg: string) => void;
    // COLLAPSE-CC: parent re-hugs on open/close.
    // Popup hug leftover: the second arg is the overlay's wanted WINDOW
    // height (0 = closed or fits already) — the parent grows the
    // companion window (hug sprite-off / composer grow sprite-on) so
    // the reply fits whole instead of inner-scrolling.
    onOverlayChange?: (open: boolean, wantWin?: number) => void;
  } = $props();

  // ---------- Collapsed pager ------
  // Only the collapsed view pages, and only the reply arriving right now
  // — primed/lastTailSig separate a fresh tail from a state reload, so
  // persisted history never pages. The paged turn is bound by index
  // (pagedTurnIdx), never by text, so a state:changed rewrite can't dump
  // the pager. The paged turn renders one page at a time; pageRaw is the
  // speakable text per page.
  let primed = false;
  let lastTailSig = "";
  let pagedTurnIdx = $state(-1);
  let pages = $state<string[]>([]);
  let pageIdx = $state(0);
  let pageRaw: string[] = [];
  let ccOpen = $state(false);
  let pageGen = 0;
  let autoTimer: ReturnType<typeof setTimeout> | undefined;
  let rootEl: HTMLElement | null = null;
  let transcriptEl = $state<HTMLElement | null>(null);

  const pageViewHtml = $derived.by(() => {
    if (pagedTurnIdx < 0 || pages.length === 0) return "";
    const i = Math.min(pageIdx, pages.length - 1);
    let h = pages[i];
    if (i < pages.length - 1) h += `<span class="pgmore">…</span>`;
    return h;
  });

  function clearAuto() {
    if (autoTimer) {
      clearTimeout(autoTimer);
      autoTimer = undefined;
    }
  }

  function releasePager() {
    clearAuto();
    if (pagedTurnIdx < 0 && pages.length === 0) return;
    pagedTurnIdx = -1;
    pages = [];
    pageRaw = [];
    pageIdx = 0;
    ccOpen = false;
  }

  function paginateToBox(text: string): string[] {
    const src = rootEl?.querySelector(".cc.bot") as HTMLElement | null;
    if (!src || src.clientWidth < 8) return [text];
    const cs = getComputedStyle(src);
    const cap = parseFloat(cs.maxHeight);
    const maxH = Number.isFinite(cap) && cap > 8 ? cap : src.clientHeight;
    if (!(maxH > 8)) return [text];
    const probe = src.cloneNode(false) as HTMLElement;
    probe.removeAttribute("title");
    probe.style.position = "absolute";
    probe.style.left = "-9999px";
    probe.style.top = "0";
    probe.style.visibility = "hidden";
    probe.style.pointerEvents = "none";
    probe.style.height = "auto";
    probe.style.maxHeight = "none";
    probe.style.overflow = "hidden";
    probe.style.width = `${src.clientWidth}px`;
    src.parentElement?.appendChild(probe);
    const fits = (prefix: string, hasMore: boolean) => {
      probe.innerHTML =
        mdLite(prefix) + (hasMore ? `<span class="pgmore">…</span>` : "");
      return probe.scrollHeight <= maxH + 1;
    };
    let rawPages: string[];
    try {
      rawPages = fitPages(text, fits);
    } finally {
      probe.remove();
    }
    return rawPages.length > 0 ? rawPages : [text];
  }

  function speakReply(text: string) {
    if (!speak || !text.trim()) return;
    invoke("speak_reply", { text }).catch((e) => {
      onSpeakError?.(`${e}`);
    });
  }

  // A fresh agent reply just became the transcript tail. Collapsed only:
  // fill the live `.cc.bot` box. One-page replies render (and speak)
  // whole. Expanded never pages. Each page is mdLite'd on its own; the
  // raw page doubles as the speakable text.
  async function arriveReply(text: string, idx: number) {
    const gen = ++pageGen;
    releasePager();
    if (expanded) {
      speakReply(text);
      return;
    }
    await tick();
    if (gen !== pageGen) return;
    // CAPTION-BUBBLE: Speak off = small scroll, no tiles. Speak on = caps.
    if (speak) {
      const rawPages = paginateToBox(text);
      if (rawPages.length > 1) {
        pages = rawPages.map(mdLite);
        pageRaw = rawPages.map((p) => p.replace(/\s+/g, " ").trim());
        pageIdx = 0;
        pagedTurnIdx = idx;
        speakPage();
        return;
      }
    }
    speakReply(text);
  }

  // Sound on: read the page, then auto-advance after the clip — `say`
  // runs ≈175 wpm by default and the native side fire-and-forgets it, so
  // the estimate is the only clock; a page with nothing speakable gets
  // the ~1.2s beat. Sound off waits; Reduce Motion never auto-advances.
  function speakPage() {
    clearAuto();
    const raw = pageRaw[pageIdx] ?? "";
    if (speak && raw) {
      invoke("speak_reply", { text: raw }).catch((e) => {
        onSpeakError?.(`${e}`);
      });
    }
    if (!speak) return;
    if (pageIdx >= pages.length - 1) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const words = raw ? raw.split(" ").filter(Boolean).length : 0;
    const ms = words === 0 ? 1200 : Math.max(1200, (words / 175) * 60000 + 300);
    autoTimer = setTimeout(() => {
      autoTimer = undefined;
      advancePage();
    }, ms);
  }

  function advancePage() {
    if (pagedTurnIdx < 0 || pages.length === 0 || ccOpen) return;
    clearAuto();
    if (pageIdx < pages.length - 1) {
      pageIdx++;
      speakPage();
    }
    // Last remaining chunk stays; click-expand is the full reply.
  }

  // Enter / Space advance. Composing wins: a non-empty field keeps
  // Enter = send and Space = type; focused buttons/selects keep theirs.
  function onPaneKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (ccOpen) {
        ccOpen = false;
        e.preventDefault();
        e.stopImmediatePropagation();
      }
      return;
    }
    if (pagedTurnIdx < 0 || pages.length === 0 || ccOpen) return;
    if (e.key !== "Enter" && e.key !== " ") return;
    const tag = (e.target as HTMLElement | null)?.tagName ?? "";
    if (tag === "SELECT" || tag === "BUTTON" || tag === "TEXTAREA") return;
    if (tag === "INPUT" && draft.trim()) return;
    e.preventDefault();
    advancePage();
  }

  // CAPTION-BUBBLE: click the bot CC toggles the full overlay. Pane
  // click no longer pages (that stole expand). Enter/Space/`say` page.
  function onBotClick(e: MouseEvent) {
    if (e.button !== 0) return;
    if (window.getSelection()?.toString()) return;
    e.stopPropagation();
    ccOpen = !ccOpen;
  }

  function onPaneClick(e: MouseEvent) {
    const t = e.target as HTMLElement | null;
    if (ccOpen && !t?.closest(".cc.bot, .cc-full")) ccOpen = false;
  }

  // The expand overlay is FIXED — out of the
  // caption flow, over the companion window (it may cover the you-row),
  // never the old in-stack pop-down that clipped and sprouted a window
  // scrollbar. Anchored above the dock when there is room; the
  // sprite-off bar (no room) is covered whole.
  // COLLAPSE-CC leftover (popup hug): at open the overlay's CONTENT
  // height is measured and the wanted WINDOW height rides up via
  // onOverlayChange — the parent grows the companion window (the same
  // door as the typing composer) until the reply fits whole. No inner
  // scroll until the ~80%-of-screen cap or the native grow clamp says
  // absurd. The mode + want LATCH at open (and on a reply swap while
  // open); a window resize only re-anchors (latch=false), so the grow
  // the report caused never feeds back into the want.
  let fullBottom = $state(8);
  let fullMaxH = $state(160);
  let fullMode: "dock" | "full" = "full";
  let fullWantWin = 0;
  let overlayReported = false;

  const fullCap = () => Math.round(window.screen.availHeight * 0.8);

  function measureFull(latch: boolean) {
    const full = rootEl?.querySelector(".cc-full") as HTMLElement | null;
    const dock = rootEl?.closest("main")?.querySelector(".ghostdock");
    const dockTop = dock?.getBoundingClientRect().top ?? window.innerHeight;
    const avail = dockTop - 14; // 8px top margin + 6px gap over the dock
    if (latch) {
      // Outer px the overlay needs to show the whole reply (scrollHeight
      // ignores the max-height clip; +2 for the border).
      const want = full ? Math.min(full.scrollHeight + 2, fullCap()) : 0;
      fullMode = avail >= 120 ? "dock" : "full";
      fullWantWin =
        fullMode === "dock"
          ? want > avail
            ? Math.round(window.innerHeight + want - avail)
            : 0
          : Math.round(Math.max(window.innerHeight, want + 16));
    }
    if (fullMode === "dock" && avail >= 120) {
      fullBottom = Math.round(window.innerHeight - dockTop + 6);
      fullMaxH = Math.round(avail);
    } else {
      fullBottom = 8;
      fullMaxH = Math.max(96, Math.round(window.innerHeight - 16));
    }
  }

  // Open latches the want and reports it up; close zeroes it (the hug /
  // grow snaps back). A reply swap while open re-latches — the overlay
  // renders botLine live. overlayReported gates the close report so a
  // botLine change while closed never double-reports.
  $effect(() => {
    botLine;
    if (!ccOpen) {
      fullWantWin = 0;
      if (overlayReported) {
        overlayReported = false;
        onOverlayChange?.(false, 0);
      }
      return;
    }
    tick().then(() => {
      if (!ccOpen) return; // closed before the measure landed
      measureFull(true);
      overlayReported = true;
      onOverlayChange?.(true, fullWantWin);
    });
  });

  // The hug/grow resize (or a user resize) lands here: re-anchor the
  // overlay to the new layout. Never re-latches the want.
  function onWinResize() {
    if (ccOpen) measureFull(false);
  }

  // Click the overlay itself closes too (a text selection never does);
  // click-off and Esc are the pane-level handlers above.
  function onFullClick(e: MouseEvent) {
    if (e.button !== 0) return;
    if (window.getSelection()?.toString()) return;
    e.stopPropagation();
    ccOpen = false;
  }

  // First turns snapshot is history (prime, do not page). Wait until
  // parent loadState has landed — the empty mount snapshot is not a
  // tail, and treating it as one re-paged / spoke the last reply.
  $effect(() => {
    const n = turns.length;
    const tail = n > 0 ? turns[n - 1] : null;
    const sig = tail ? `${tail.role}\u0000${tail.text}` : "";
    if (!ready) {
      lastTailSig = sig;
      return;
    }
    if (!primed) {
      lastTailSig = sig;
      primed = true;
      return;
    }
    if (sig === lastTailSig) return;
    lastTailSig = sig;
    if (!tail || tail.role !== "them") {
      releasePager();
      return;
    }
    void arriveReply(tail.text, n - 1);
  });

  // The pager lives only on the collapsed view; expanding mid-page
  // unmounts this instance (parent) which also releases. This covers
  // an expanded instance that still held a page for one frame.
  $effect(() => {
    if (expanded && pagedTurnIdx >= 0) releasePager();
  });

  $effect(() => {
    if (!speak) clearAuto();
  });

  $effect(() => {
    const watching = [turns.length, sending, expanded, pageIdx, pagedTurnIdx];
    void watching;
    if (!expanded || !transcriptEl) return;
    const el = transcriptEl;
    tick().then(() => {
      el.scrollTop = el.scrollHeight;
    });
  });

  onMount(() => {
    window.addEventListener("keydown", onPaneKey);
    window.addEventListener("click", onPaneClick);
    window.addEventListener("resize", onWinResize);
  });

  onDestroy(() => {
    clearAuto();
    // Popup hug: unmounting with the overlay open (expand flip) is a
    // close — release the parent's grown window.
    if (overlayReported) {
      overlayReported = false;
      onOverlayChange?.(false, 0);
    }
    window.removeEventListener("keydown", onPaneKey);
    window.removeEventListener("click", onPaneClick);
    window.removeEventListener("resize", onWinResize);
  });
</script>

{#snippet wait()}
  <!-- The thinking splash is EPHEMERAL — one
       short live line next to the mark while companion_send is in
       flight (the spirit already plays the thinking loop). Never a
       turn: nothing here persists; the final them-row is the only
       row the send path saves.
       When the conductor is on, its fast ack line
       replaces the bare "thinking…" here — same ephemeral slot, same
       no-persist law; the mouth's reply still lands as the only
       them-row. -->
  <span class="wait" role="status" aria-busy="true">
    <span class="sr-only">{ackLine ? ackLine : "thinking"}</span>
    <span class="wmark" aria-hidden="true">
      <svg class="ring" viewBox="0 0 20 20" fill="none">
        <ellipse cx="10" cy="10" rx="8" ry="5.2" transform="rotate(18 10 10)" />
      </svg>
      <i class="sq"></i>
      <span class="tilt">
        <i class="orb"></i>
        <i class="speck"></i>
      </span>
    </span>
    <span class="wline" aria-hidden="true">{ackLine || "thinking…"}</span>
  </span>
{/snippet}

{#if expanded}
  <!-- The one expanded transcript: idle Expand fills the pane; HOST-CC
       hosted Expand is the SAME pane as an overlay on the well
       (.hostoverlay — absolute inset 0 over .wellwrap, dimmed scrim,
       opaque bubbles). CAPTION-WORDS: the pager itself is collapsed-only
       (expanding releases it); the lock classes stay so the frame
       between Expand and releasePager still shows no scrollbar. -->
  <div
    class="transcript"
    class:hostoverlay={hosted}
    class:pagedlock={pagedTurnIdx >= 0}
    bind:this={transcriptEl}
  >
    {#each turns as t, i}
      <div class="row" class:you={t.role === "you"} class:them={t.role !== "you"}>
        <!-- The transcript already shows the whole reply — no title
             tooltip / hover popup here; the .cc-full overlay is
             collapsed-only. -->
        <div
          class="bubble"
          class:paged={i === pagedTurnIdx}
        >
          {#if i === pagedTurnIdx && pageViewHtml}
            {@html pageViewHtml}
          {:else}
            {@html mdLite(t.text)}
          {/if}
        </div>
      </div>
    {/each}
    {#if sending}
      <div class="row them wait-row" aria-live="polite">{@render wait()}</div>
    {/if}
  </div>
{:else}
  <div class="caption" bind:this={rootEl}>
    {#if userLine}
      <div class="ccrow you">
        <div class="cc user" title={userLine}>{@html mdLite(userLine)}</div>
      </div>
    {/if}
    {#if sending}
      <div class="ccrow them wait-row" aria-live="polite">{@render wait()}</div>
    {/if}
    {#if botLine && !sending}
      <div class="ccrow them" class:open={ccOpen}>
        <div
          class="cc bot"
          class:paged={pagedTurnIdx >= 0 && !ccOpen}
          onclick={onBotClick}
        >
          {#if pagedTurnIdx >= 0 && pageViewHtml && !ccOpen}
            {@html pageViewHtml}
          {:else}
            {@html mdLite(botLine)}
          {/if}
        </div>
        {#if ccOpen}
          <div
            class="cc-full"
            style:bottom="{fullBottom}px"
            style:max-height="{fullMaxH}px"
            onclick={onFullClick}
          >
            {@html mdLite(botLine)}
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .caption {
    display: contents;
  }

  /* Expanded transcript: starts at the top of the pane and scrolls the
     saved turns; never a drag region. CORNER: padding-right is always
     106px (96 portrait + 10 gap) so you/them bubbles never run under
     the face — you bubbles right-align inside the padded column.
     S2.5: SMS bubbles — you right on a warm accent fill, them left on a
     muted fill with an edge; no you:/them: prefixes. */
  .transcript {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px 10px 2px 106px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  /* Hosted + expanded — the transcript overlays the
     well (absolute, inset 0 over .wellwrap). The 72% bg scrim dims the
     well so the board stays visible behind it; the bubbles keep their
     own opaque fills. Not a second window; no grow/shrink on Expand. */
  .transcript.hostoverlay {
    position: absolute;
    inset: 0;
    z-index: 1;
    background: color-mix(in srgb, var(--bg-fill, var(--bg)) 72%, transparent);
  }

  .row {
    display: flex;
  }

  .row.you {
    justify-content: flex-end;
  }

  .row.them {
    justify-content: flex-start;
  }

  /* GRAB-LAW: bubbles select + copy — never a drag surface. */
  .bubble {
    margin: 0;
    max-width: 78%;
    padding: 6px 10px;
    font-size: 0.78rem;
    line-height: 1.35;
    overflow-wrap: anywhere;
    border-radius: 14px;
    user-select: text;
    -webkit-user-select: text;
  }

  .row.you .bubble {
    color: var(--on-accent, #ffffff);
    background: var(--accent);
  }

  .row.them .bubble {
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
  }

  .bubble :global(.pgmore),
  .cc :global(.pgmore) {
    opacity: 0.7;
  }

  .transcript.pagedlock {
    overflow-y: hidden;
  }

  .bubble.paged,
  .cc.paged {
    overflow: hidden;
  }

  .bubble :global(ul),
  .bubble :global(ol),
  .cc :global(ul),
  .cc :global(ol) {
    margin: 3px 0;
    padding-left: 18px;
  }

  .bubble :global(li),
  .cc :global(li) {
    margin: 1px 0;
  }

  .bubble :global(table),
  .cc :global(table) {
    border-collapse: collapse;
    margin: 4px 0;
    max-width: 100%;
  }

  .bubble :global(th),
  .bubble :global(td),
  .cc :global(th),
  .cc :global(td) {
    border: 1px solid color-mix(in srgb, currentColor 35%, transparent);
    padding: 2px 7px;
    text-align: left;
    vertical-align: top;
    overflow-wrap: anywhere;
  }

  .bubble :global(th),
  .cc :global(th) {
    font-weight: 600;
  }

  .bubble :global(code),
  .cc :global(code) {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 0.92em;
    padding: 0 4px;
    border-radius: 4px;
    background: color-mix(in srgb, currentColor 12%, transparent);
  }

  .wait-row {
    flex: none;
    justify-content: flex-start;
    pointer-events: none;
  }

  .wait {
    display: inline-flex;
    align-items: center;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
    border: 0;
  }

  .wmark {
    position: relative;
    display: inline-block;
    width: 20px;
    height: 20px;
  }

  /* MOUTH-LIVE splash line: short, muted, gone with the reply. */
  .wline {
    margin-left: 7px;
    font-size: 0.66rem;
    font-style: italic;
    opacity: 0.55;
  }

  .wmark .ring {
    position: absolute;
    inset: 0;
    display: block;
    width: 100%;
    height: 100%;
  }

  .wmark .ring ellipse {
    stroke: #7e63ce;
    stroke-width: 1;
  }

  .wmark .sq {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 6px;
    height: 6px;
    margin: -3px 0 0 -3px;
    border-radius: 2px;
    background: linear-gradient(135deg, #e59e3d, #c8a45c);
    animation: wspin 1.8s linear infinite;
  }

  .wmark .tilt {
    position: absolute;
    inset: 0;
    transform: rotate(18deg);
  }

  .wmark .tilt i {
    position: absolute;
    top: 50%;
    left: 50%;
    display: block;
    border-radius: 999px;
  }

  .wmark .orb {
    width: 4px;
    height: 4px;
    margin: -2px 0 0 -2px;
    background: #3db8b0;
    animation: worbit 1.6s linear infinite;
  }

  .wmark .speck {
    width: 2px;
    height: 2px;
    margin: -1px 0 0 -1px;
    background: #f2e7ce;
    opacity: 0.3;
    animation: wspeck 3.4s linear infinite;
  }

  @keyframes wspin {
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes worbit {
    0% {
      transform: translate(6.93px, 2.6px);
    }
    8.33% {
      transform: translate(8px, 0);
    }
    16.67% {
      transform: translate(6.93px, -2.6px);
    }
    25% {
      transform: translate(4px, -4.5px);
    }
    33.33% {
      transform: translate(0, -5.2px);
    }
    41.67% {
      transform: translate(-4px, -4.5px);
    }
    50% {
      transform: translate(-6.93px, -2.6px);
    }
    58.33% {
      transform: translate(-8px, 0);
    }
    66.67% {
      transform: translate(-6.93px, 2.6px);
    }
    75% {
      transform: translate(-4px, 4.5px);
    }
    83.33% {
      transform: translate(0, 5.2px);
    }
    91.67% {
      transform: translate(4px, 4.5px);
    }
    100% {
      transform: translate(6.93px, 2.6px);
    }
  }

  @keyframes wspeck {
    0% {
      transform: translate(-6.93px, 2.6px);
    }
    8.33% {
      transform: translate(-4px, 4.5px);
    }
    16.67% {
      transform: translate(0, 5.2px);
    }
    25% {
      transform: translate(4px, 4.5px);
    }
    33.33% {
      transform: translate(6.93px, 2.6px);
    }
    41.67% {
      transform: translate(8px, 0);
    }
    50% {
      transform: translate(6.93px, -2.6px);
    }
    58.33% {
      transform: translate(4px, -4.5px);
    }
    66.67% {
      transform: translate(0, -5.2px);
    }
    75% {
      transform: translate(-4px, -4.5px);
    }
    83.33% {
      transform: translate(-6.93px, -2.6px);
    }
    91.67% {
      transform: translate(-8px, 0);
    }
    100% {
      transform: translate(-6.93px, 2.6px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .wmark .sq,
    .wmark .orb {
      animation: none;
    }
    .wmark .orb {
      transform: translate(6.93px, 2.6px);
    }
    .wmark .speck {
      display: none;
    }
  }

  .ccrow {
    display: flex;
    padding: 2px 10px;
  }

  .ccrow.you {
    justify-content: flex-end;
  }

  .ccrow.them {
    position: relative;
    justify-content: flex-start;
  }

  .cc {
    margin: 0;
    max-width: 82%;
    max-height: calc(1.3em * 6 + 10px);
    overflow-y: auto;
    padding: 5px 10px;
    font-size: 0.68rem;
    line-height: 1.3;
    overflow-wrap: anywhere;
    border-radius: 14px;
    user-select: text;
    -webkit-user-select: text;
  }

  .cc.bot {
    position: relative;
    cursor: pointer;
  }

  /* COLLAPSE-CC: fixed = out of the caption stack, over the companion
     window (bottom/max-height inline from measureFull). The old
     absolute pop-down — and its .ghost.spriteoff downward
     top:calc(100%+4px) clip — is dead: no clipping, no new window
     scrollbar. GRAB-LAW: selects + copies, never a drag surface. */
  .cc-full {
    position: fixed;
    left: 10px;
    right: 10px;
    z-index: 7;
    overflow-y: auto;
    padding: 8px 10px;
    font-size: 0.72rem;
    line-height: 1.35;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 12px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.28);
    overflow-wrap: anywhere;
    user-select: text;
    -webkit-user-select: text;
  }

  .cc :global(ul),
  .cc :global(ol) {
    margin: 1px 0;
    padding-left: 14px;
  }

  .cc :global(li) {
    margin: 0;
  }

  .cc :global(table) {
    margin: 2px 0;
    font-size: 0.62rem;
  }

  .cc :global(th),
  .cc :global(td) {
    padding: 1px 5px;
  }

  .cc.user {
    color: var(--on-accent, #ffffff);
    background: var(--accent);
  }

  .cc.bot {
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
  }
</style>
