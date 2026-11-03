<script lang="ts">
  // The dock — agent pick, Prompts,
  // dests Launchpad/STARS/Planets (hover chips, diagonal rocket), Speak,
  // Full/Head sprite toggle, Expand, ⚙ last after Expand (GEAR-DOCK),
  // composer. Paint + presses only: CompanionPage owns all state, the
  // sends (the field still calls companion_send), and the well. Planets
  // dest L-click used to toggle Planets. Those actions are launcher rows now.
  // No dest right-click menu — planet tabs live in the well / Launchpad.
  // × stays a CompanionPage corner.
  // The live LED sits immediately left of the agent
  // pick (sending || hackLive); the pick hugs its labels (46% cap dead).
  import { onMount, untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { eventElement } from "./dragPane";
  import "./companion/dock.css";
  import DockCheatHint from "./companion/CheatHint.svelte";
  import DockAgentLeft from "./companion/AgentLeft.svelte";
  import DockAgentRight from "./companion/AgentRight.svelte";
  import DockMouth from "./companion/Mouth.svelte";
  import DockLauncher from "./companion/Launcher.svelte";
  import {
    clearAfterRun,
    ghostSuffix,
    launcherItems,
    launcherKey,
    noteUse,
    pickIndex,
    refitComposer,
    type Action,
    type LaunchRow,
    type Usage,
  } from "./companion/launcher";

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
    speak,
    speakVolume = 1,
    headShowing,
    viewBusy,
    sprite,
    bustOk,
    viewTitle,
    expanded,
    planetsHidden = false,
    planets = [],
    neededId = null,
    hostBusy = false,
    smallest = null,
    captions = true,
    mode = "",
    runAction,
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
    nextCount = 0,
    toggleNextCard,
    toggleSpeak,
    toggleCaptions,
    setSpeakVolume,
    toggleSprite,
    toggleView,
    toggleExpanded,
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
    speak: boolean;
    speakVolume?: number;
    headShowing: boolean;
    viewBusy: boolean;
    sprite: boolean;
    bustOk: boolean;
    viewTitle: string;
    expanded: boolean;
    planetsHidden?: boolean;
    planets?: { id: string; name: string; dot?: string }[];
    neededId?: string | null;
    hostBusy?: boolean;
    smallest?: { cardId: string; text: string } | null;
    captions?: boolean;
    mode?: string;
    runAction: (action: Action) => Promise<boolean>;
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
    nextCount?: number;
    toggleNextCard: () => void;
    toggleSpeak: () => void;
    toggleCaptions: () => void;
    setSpeakVolume: (v: number) => void;
    toggleSprite: (e: MouseEvent) => void;
    toggleView: () => void;
    toggleExpanded: () => void;
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

  let selected = $state(0);
  let visibleStart = $state(0);
  let stash = $state<string | null>(null);
  let byArrow = $state(false);
  // Memory-only use counts (count + last used) rank the list; they never
  // reach state.json. Ghost needs the caret and the IME flag.
  let usage = $state<Usage>({});
  let caretAtEnd = $state(true);
  let composing = $state(false);
  let queryFromKey = false;

  const items = $derived.by((): LaunchRow[] => {
    if (expanded) return [];
    return launcherItems(
      {
        query: draft,
        armed: neededId ? { id: neededId, title: neededHint ?? "Needed" } : null,
        planets,
        hosted,
        hostBusy,
        agent,
        agents: agents.map((id) => ({ id, label: agentLabel(id) })),
        mode,
        speak,
        sprite,
        captions,
        head: headShowing,
        prompts,
        nextCount,
        smallestTodo: smallest,
        planetsHidden,
      },
      usage,
    );
  });

  // The rest of the top match's title, painted faintly after the caret.
  // Ask/Answer rows never ghost; Return never takes it.
  const ghostTop = $derived(
    !expanded && ghostSuffix(draft, items, caretAtEnd, composing)
      ? (items.find((row) => row.kind === "command")?.title ?? "")
      : "",
  );
  const ghost = $derived(
    ghostTop && ghostTop.toLowerCase().startsWith(draft.toLowerCase())
      ? ghostTop.slice(draft.length)
      : "",
  );

  function syncCaret() {
    const el = fieldEl;
    caretAtEnd =
      !el || (el.selectionStart === el.selectionEnd && el.selectionEnd === draft.length);
  }

  function onComposing(on: boolean) {
    composing = on;
  }

  $effect(() => {
    draft;
    if (queryFromKey) {
      queryFromKey = false;
      return;
    }
    const rows = untrack(() => items);
    selected = pickIndex(draft, rows);
    visibleStart = 0;
    byArrow = false;
    stash = null;
  });

  $effect(() => {
    if (selected >= items.length && selected !== 0) selected = 0;
  });

  const activeId = $derived(
    !expanded && items.length > 0 ? `companion-launcher-opt-${selected}` : "",
  );

  async function runIndex(index: number, viaArrow: boolean) {
    const row = items[index];
    if (!row) return;
    const shouldClear = clearAfterRun({
      kind: row.run.t,
      query: draft,
      title: row.title,
      byArrow: viaArrow && index > 0,
    });
    const ok = await runAction(row.run);
    if (!ok) return;
    usage = noteUse(usage, row.id);
    if (row.run.t !== "send" && row.run.t !== "answer" && shouldClear) {
      queryFromKey = true;
      draft = "";
    }
    byArrow = false;
    selected = 0;
    visibleStart = 0;
  }

  function onFieldKeydown(e: KeyboardEvent) {
    if (expanded) {
      if (e.key !== "Enter" || e.isComposing) return;
      if (e.shiftKey) return;
      e.preventDefault();
      sendLine();
      return;
    }
    const undo = (e.metaKey || e.ctrlKey) && !e.shiftKey && (e.key === "z" || e.key === "Z") && stash !== null;
    const field = e.currentTarget instanceof HTMLTextAreaElement ? e.currentTarget : fieldEl;
    composing = e.isComposing;
    syncCaret();
    const result = launcherKey(
      { query: draft, selected, count: items.length, visibleStart, stash },
      {
        key: e.key,
        shift: e.shiftKey,
        meta: e.metaKey,
        alt: e.altKey,
        ctrl: e.ctrlKey,
        composing: e.isComposing,
        hasSelection: !!field && field.selectionStart !== field.selectionEnd,
        ghost,
        ghostTitle: ghostTop,
      },
      items.map((row) => row.title),
    );
    selected = result.state.selected;
    visibleStart = result.state.visibleStart;
    stash = result.state.stash;
    if (result.state.query !== draft) {
      // Escape must skip the draft effect so the cleared text stays
      // stashed for ⌘Z. Tab complete owns its own selection. Undo
      // (and a native undo) falls through so a one-word query re-picks.
      if (e.key === "Escape" || result.effect === "complete") queryFromKey = true;
      draft = result.state.query;
    }
    if (result.effect === "collapse-selection") {
      e.preventDefault();
      e.stopPropagation();
      if (field) {
        const end = field.selectionEnd ?? field.selectionStart ?? 0;
        field.setSelectionRange(end, end);
      }
      return;
    }
    if (e.key === "ArrowUp" || e.key === "ArrowDown") byArrow = true;
    if (result.effect === "run") {
      e.preventDefault();
      e.stopPropagation();
      void runIndex(result.runIndex ?? 0, e.key === "Enter" && byArrow);
      return;
    }
    if (result.effect === "newline") return;
    if (result.effect === "blur") {
      e.preventDefault();
      e.stopPropagation();
      fieldEl?.blur();
      return;
    }
    if (result.effect === "complete") {
      e.preventDefault();
      return;
    }
    if (result.effect === "ignore") {
      if (!e.isComposing) e.preventDefault();
      return;
    }
    if (
      e.key === "Escape" ||
      e.key === "ArrowUp" ||
      e.key === "ArrowDown" ||
      e.key === "Tab" ||
      undo ||
      ((e.metaKey || e.ctrlKey) && /^[1-9]$/.test(e.key))
    ) {
      e.preventDefault();
      if (e.key === "Escape") e.stopPropagation();
    }
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
    // Sprite-on: height auto, same scrollHeight as before. Sprite-off:
    // a just-ungrown window clips the flex box, and WebKit then reports
    // that visible height. Measuring from 0px recovers the full draft
    // so the hug (CompanionPage already watches sprite) fits the window
    // instead of locking the lines inside a short mouth.
    let needed = el.scrollHeight + border;
    if (!sprite) {
      el.style.height = "0px";
      const full = el.scrollHeight + border;
      if (full > needed) needed = full;
    }
    const fit = refitComposer({
      needed,
      oneLineH,
      lineH,
      expanded,
      hostedOff: hosted === "off",
      sprite,
      maxExtraLines: MAX_EXTRA_LINES,
    });
    el.style.height = `${fit.height}px`;
    el.style.overflowY = fit.scroll ? "auto" : "hidden";
    composerExtra = fit.composerExtra;
    } finally {
      growing = false;
    }
  }

  $effect(() => {
    // Grow on every keystroke / talk fill / send-clear; a surface flip
    // re-runs so the 6-line cap re-applies (or releases) in place.
    // Sprite is one of those flips: grow → hug must remeasure the field
    // or the ungrow resize locks the draft at the clipped height.
    draft;
    expanded;
    hosted;
    sprite;
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
      "button, input, a, select, textarea, option, .field, .fieldwrap, .volpop, .agentbubble, .agentopt, .launcher-row",
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

  {#if !expanded}
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
  {/if}

  <DockMouth
    variant={expanded ? "line" : "dock"}
    {attach}
    {onDropAttach}
    {onPickAttach}
    {onThin}
    {sending}
    bind:draft
    bind:fieldEl
    {onFieldKeydown}
    {ghost}
    onCaret={syncCaret}
    {onComposing}
    {neededHint}
    listOpen={!expanded}
    activeDescendant={activeId}
    {talking}
    {talkStopOnly}
    {talkStopSend}
    {talkStart}
    {sendLine}
    onSubmit={() => void runIndex(selected, byArrow)}
    {fmtSize}
  />
  {#if !expanded}
    <DockLauncher
      rows={items}
      {selected}
      {visibleStart}
      onRun={(index) => void runIndex(index, false)}
      onHover={(index) => (selected = index)}
      onScrollStart={(start) => (visibleStart = start)}
    />
  {/if}
</div>
