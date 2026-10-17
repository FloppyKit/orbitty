<script lang="ts">
  // The dock — agent pick, Prompts,
  // dests Launchpad/STARS/Planets (hover chips, diagonal rocket), Speak,
  // Full/Head sprite toggle, Expand, ⚙ last after Expand (GEAR-DOCK),
  // composer. Paint + presses only: CompanionPage owns all state, the
  // sends (the field still calls companion_send), and the well. Planets
  // dest L-click = toggleDest("inspector").
  // No dest right-click menu — planet tabs live in the well / Launchpad.
  // × stays a CompanionPage corner.
  // The live LED sits immediately left of the agent
  // pick (sending || hackLive); the pick hugs its labels (46% cap dead).
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { eventElement } from "./dragPane";
  import "./companion/dock.css";
  import DockCheatHint from "./companion/CheatHint.svelte";
  import DockAgentLeft from "./companion/AgentLeft.svelte";
  import DockDests from "./companion/Dests.svelte";
  import DockAgentRight from "./companion/AgentRight.svelte";
  import DockMouth from "./companion/Mouth.svelte";

  // Chrome labels only. The option value stays
  // the stem id so set_companion_agent and the on-disk allowlist are
  // untouched; an unknown stem fails open to the raw id.
  const AGENT_LABELS: Record<string, string> = {
    spark: "Spark",
    grok: "Work",
    "chaos-gremlin": "Chat",
    "fkt-homie": "Hold",
  };
  const agentLabel = (id: string) => AGENT_LABELS[id] ?? id;

  // One 7px LED immediately left of the agent pick
  // — well always there (idle dim); glow/pulse while ANY agent is
  // working: Spark mouth (sending) or ctl hackLive. Not a button.
  const HACK_WHO_TITLES: Record<string, string> = {
    cursor: "Cursor leftover",
    goose: "Goose",
    grok: "Grok",
    hermes: "Hermes",
    pi: "Pi",
    spark: "Spark mouth",
  };

  type HostMode = "off" | "launchpad" | "stars" | "inspector";

  // One chip = name + size. Original path rides
  // companion_send; never a copy.
  type AttachChip = { path: string; name: string; size: number };
  const fmtSize = (n: number) =>
    n >= 1024 * 1024
      ? `${(n / (1024 * 1024)).toFixed(1)} MB`
      : n >= 1024
        ? `${(n / 1024).toFixed(1)} KB`
        : `${n} B`;

  let {
    agents,
    agent = $bindable(),
    sending,
    hackLive,
    hackWho,
    prompts,
    hosted,
    destOpen = { launchpad: false, stars: false, inspector: false },
    speak,
    speakVolume = 1,
    headShowing,
    viewBusy,
    sprite,
    bustOk,
    viewTitle,
    expanded,
    planetsHidden = false,
    showDests = false,
    talking,
    neededHint = null,
    draft = $bindable(),
    fieldEl = $bindable(),
    composerExtra = $bindable(0),
    attach = null,
    onPickAttach,
    onDropAttach,
    onAgentChange,
    togglePrompts,
    toggleDest,
    nextCount = 0,
    toggleNextCard,
    toggleSpeak,
    toggleCaptions,
    setSpeakVolume,
    toggleSprite,
    toggleView,
    toggleExpanded,
    togglePlanetsHidden,
    sendLine,
    talkStart,
    talkStopOnly,
    talkStopSend,
    onDockMouseDown,
    cheatSheetHint = true,
    onCheatSheet,
    onThin,
  }: {
    agents: string[];
    agent: string;
    sending: boolean;
    hackLive: boolean;
    hackWho: string;
    prompts: boolean;
    hosted: HostMode;
    destOpen?: { launchpad: boolean; stars: boolean; inspector: boolean };
    speak: boolean;
    speakVolume?: number;
    headShowing: boolean;
    viewBusy: boolean;
    sprite: boolean;
    bustOk: boolean;
    viewTitle: string;
    expanded: boolean;
    planetsHidden?: boolean;
    /** PEER-CHROME: Launchpad/STARS/Planets dest strip. */
    showDests?: boolean;
    talking: boolean;
    neededHint?: string | null;
    draft: string;
    fieldEl: HTMLTextAreaElement | null;
    // The one-file chip (name + size, × to drop).
    // CompanionPage owns it; the dock only paints and reports presses.
    attach?: AttachChip | null;
    onPickAttach: () => void;
    onDropAttach: () => void;
    // COLLAPSE-CC: px the composer grew past its one-line idle (collapsed
    // only — expanded/hosted keep the 6-line cap and report 0).
    // CompanionPage turns it into the native hug / window grow.
    composerExtra?: number;
    onAgentChange: () => void;
    togglePrompts: () => void;
    toggleDest: (mode: "launchpad" | "stars" | "inspector") => void;
    nextCount?: number;
    toggleNextCard: () => void;
    toggleSpeak: () => void;
    toggleCaptions: () => void;
    setSpeakVolume: (v: number) => void;
    toggleSprite: (e: MouseEvent) => void;
    toggleView: () => void;
    toggleExpanded: () => void;
    togglePlanetsHidden: () => void;
    sendLine: () => void;
    talkStart: (e: Event) => void;
    talkStopOnly: () => void;
    talkStopSend: () => void;
    onDockMouseDown: (e: MouseEvent | PointerEvent) => void;
    cheatSheetHint?: boolean;
    onCheatSheet?: () => void;
    onThin?: () => void;
  } = $props();

  // DOCK-HACK: the LED is live when either source is — the Spark mouth
  // turn (sending) or the ctl flag (hackLive). Title names the live
  // source when known: the explicit who wins, else the mouth.
  const hackOn = $derived(sending || hackLive);
  const hackTitle = $derived.by(() => {
    if (!hackOn) return "idle";
    if (hackLive) {
      const w = hackWho.trim();
      if (w) return HACK_WHO_TITLES[w.toLowerCase()] ?? w;
    }
    if (sending) return "Spark mouth";
    return "Agent working";
  });

  function onPlanetsClick() {
    toggleDest("inspector");
  }

  function onPlanetsRight(e: MouseEvent) {
    e.preventDefault();
    togglePlanetsHidden();
    rehitPointer();
  }

  let agentOpen = $state(false);
  function pickAgent(id: string) {
    agent = id;
    agentOpen = false;
    onAgentChange();
  }

  let volOpen = $state(false);
  function onSpeakClick(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    volOpen = !volOpen;
  }
  async function onSpeakRight(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    volOpen = false;
    // UI is muted when !speak OR volume==0. R-click must toggle that
    // effective mute — flipping speak alone while volume is 0 stays muted
    // until the slider is opened (the sticky unmute Matt saw).
    const muted = !speak || speakVolume <= 0;
    if (muted) {
      if (speakVolume <= 0) await setSpeakVolume(1);
      else await toggleSpeak();
    } else {
      await toggleSpeak();
    }
    rehitPointer();
  }
  function onVolInput(e: Event) {
    const v = Number((e.currentTarget as HTMLInputElement).value);
    setSpeakVolume(v / 100);
  }

  // After a dock R-click toggles UI, WKWebView often drops hover until the
  // cursor moves. Re-hit the element under the last pointer so rapid R-clicks
  // register without a nudge.
  let lastPtr = { x: 0, y: 0 };
  function trackPtr(e: PointerEvent) {
    lastPtr = { x: e.clientX, y: e.clientY };
  }
  function rehitPointer() {
    hoverArmed = true;
    requestAnimationFrame(() => {
      const el = document.elementFromPoint(lastPtr.x, lastPtr.y) as HTMLElement | null;
      if (!el) return;
      el.dispatchEvent(
        new PointerEvent("pointermove", {
          clientX: lastPtr.x,
          clientY: lastPtr.y,
          bubbles: true,
        }),
      );
    });
  }
  function onToggleSprite(e: MouseEvent) {
    toggleSprite(e);
    rehitPointer();
  }
  function onToggleCaptions() {
    toggleCaptions();
    rehitPointer();
  }

  // COMPOSER-WRAP: the one-line field is a wrapping textarea now (Grok
  // web composer). Enter sends, Shift+Enter newline, ⌘↩/Ctrl+↩ send.
  // Auto-grows from scrollHeight (WKWebView-safe — no field-sizing).
  // Collapsed drops the 6-line cap — never an
  // inner scroll; the WINDOW grows instead (composerExtra rides up to
  // CompanionPage's hug / grow-down report). Expanded / hosted keep the
  // 6-line cap so the field never eats the transcript / well. Empty
  // clears the inline height entirely, so the idle box is the exact
  // 0224c1f one-line height (rows=1, same padding/font/border).
  // No chevron, no extra icon.
  const MAX_EXTRA_LINES = 5; // 1 idle line + 5 = 6 visible lines

  let oneLineH = 0; // border-box px of the empty field
  let lineH = 0; // content px per wrapped line

  function measureField() {
    const el = fieldEl;
    if (!el) return;
    const cs = getComputedStyle(el);
    const pad = parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom);
    const border =
      parseFloat(cs.borderTopWidth) + parseFloat(cs.borderBottomWidth);
    lineH = el.scrollHeight - pad; // empty → one line of content
    oneLineH = el.scrollHeight + border; // empty → border-box target
  }

  let growing = false;
  function growField() {
    const el = fieldEl;
    if (!el || growing) return;
    growing = true;
    try {
    // Park at rows=1 so scrollHeight reports the honest content height.
    el.style.height = "auto";
    if (draft === "") {
      // Idle / just sent: no inline height at all → CSS rows=1 → the
      // pixel-exact 0224c1f empty height. Re-measure so a late font or
      // layout change never drifts the cap math.
      measureField();
      el.style.height = "";
      el.style.overflowY = "hidden";
      composerExtra = 0;
      return;
    }
    const border =
      parseFloat(getComputedStyle(el).borderTopWidth) * 2;
    const needed = el.scrollHeight + border; // border-box target
    const collapsed = !expanded && hosted === "off";
    const max =
      oneLineH > 0 ? oneLineH + MAX_EXTRA_LINES * lineH : needed;
    const capped = !collapsed && needed > max + 0.5;
    el.style.height = `${capped ? max : needed}px`;
    el.style.overflowY = capped ? "auto" : "hidden";
    composerExtra =
      collapsed && oneLineH > 0 ? Math.max(0, Math.round(needed - oneLineH)) : 0;
    } finally {
      growing = false;
    }
  }

  function onFieldKeydown(e: KeyboardEvent) {
    if (e.key !== "Enter" || e.isComposing) return;
    if (e.shiftKey) return; // Shift+Enter = newline, never sends
    e.preventDefault(); // Enter / ⌘↩ / Ctrl+↩ = send, no newline
    sendLine();
  }

  $effect(() => {
    // Grow on every keystroke / talk fill / send-clear; a surface flip
    // re-runs so the 6-line cap re-applies (or releases) in place.
    draft;
    expanded;
    hosted;
    growField();
  });

  let dockDragging = $state(false);
  let hoverArmed = $state(true);

  function freezeDragHover() {
    dockDragging = true;
    hoverArmed = false;
    const active = document.activeElement;
    // Chrome grab only — never the composer. pointerup always unfreezes,
    // and blurring there killed the caret on a normal field click.
    if (
      active instanceof HTMLElement &&
      !active.closest("textarea, input, .field")
    ) {
      active.blur();
    }
  }

  function unfreezeDragHover() {
    // Only disarm hover after a real chrome drag. A normal button click
    // also gets pointerup — if we set hoverArmed=false there, dock.css
    // applies pointer-events:none until the next pointermove (sticky click).
    if (dockDragging) {
      dockDragging = false;
      hoverArmed = false;
    } else {
      hoverArmed = true;
    }
  }

  // Window-drag: no control hover. Freeze on chrome grab, not after slop —
  // a grab next to the field still drifts onto Thin/+.
  function onChromeDown(e: MouseEvent | PointerEvent) {
    if (e.button !== 0) {
      onDockMouseDown(e);
      return;
    }
    const t = eventElement(e);
    const onControl = t?.closest(
      "button, input, a, select, textarea, option, .field, .fieldwrap, .volpop, .agentbubble, .agentopt",
    );
    if (!onControl) freezeDragHover();
    onDockMouseDown(e);
  }

  onMount(async () => {
    const onDoc = (e: MouseEvent) => {
      const t = e.target as HTMLElement | null;
      if (t?.closest(".volwrap")) return;
      volOpen = false;
    };
    window.addEventListener("mousedown", onDoc);
    const onDocAgent = (e: MouseEvent) => {
      const t = e.target as HTMLElement | null;
      if (t?.closest(".agentwrap")) return;
      agentOpen = false;
    };
    window.addEventListener("mousedown", onDocAgent);
    const armHover = (ev: PointerEvent) => {
      if (dockDragging || ev.buttons & 1) return;
      hoverArmed = true;
    };
    window.addEventListener("pointermove", armHover);
    const onUp = () => unfreezeDragHover();
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    const unBegin = await listen("pane-drag-begin", () => freezeDragHover());
    const unEnd = await listen("pane-drag-end", () => unfreezeDragHover());
    measureField();
    // Tile open/close widens the window → rewrap → re-grow without
    // waiting for the next keystroke. RO fires only on real size
    // deltas, so the grow writes above do not loop it.
    window.addEventListener("resize", growField);
    return () => {
      window.removeEventListener("mousedown", onDoc);
      window.removeEventListener("mousedown", onDocAgent);
      window.removeEventListener("pointermove", armHover);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      window.removeEventListener("resize", growField);
      unBegin();
      unEnd();
    };
  });
</script>
<!-- Agentrow + composer ride in one wrap — the
     ghost's outlined bottom slab. × stays a floating corner; ⚙ is on
     the dock after Expand (GEAR-DOCK). -->
<div
    class="ghostdock"
    onpointermove={trackPtr}
    class:dragging={dockDragging}
    class:hover-armed={hoverArmed}
    onmousedown={onChromeDown}
  >

  <DockCheatHint {cheatSheetHint} {onCheatSheet} />

  <div class="agentrow">
    <DockAgentLeft
      {hackOn}
      {hackTitle}
      bind:agentOpen
      {agent}
      {agents}
      {sending}
      {agentLabel}
      {pickAgent}
      {prompts}
      {togglePrompts}
      {nextCount}
      {toggleNextCard}
    />
    <DockDests
      {showDests}
      {destOpen}
      {toggleDest}
      {planetsHidden}
      {onPlanetsClick}
      {onPlanetsRight}
    />
    <DockAgentRight
      {speak}
      {speakVolume}
      bind:volOpen
      {onSpeakClick}
      {onSpeakRight}
      {onVolInput}
      {headShowing}
      {viewBusy}
      {toggleView}
      {viewTitle}
      {sprite}
      {bustOk}
      {expanded}
      {toggleExpanded}
      toggleSprite={onToggleSprite}
      toggleCaptions={onToggleCaptions}
    />
  </div>

  <DockMouth
    {attach}
    {onDropAttach}
    {onPickAttach}
    {onThin}
    {sending}
    bind:draft
    bind:fieldEl
    {onFieldKeydown}
    {neededHint}
    {talking}
    {talkStopOnly}
    {talkStopSend}
    {talkStart}
    {sendLine}
    {fmtSize}
  />
</div>
