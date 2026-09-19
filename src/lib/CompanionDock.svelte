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
    togglePlanetsHidden();
  }

  function onPlanetsRight(e: MouseEvent) {
    e.preventDefault();
    toggleDest("inspector");
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
  function onSpeakRight(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    volOpen = false;
    toggleSpeak();
  }
  function onVolInput(e: Event) {
    const v = Number((e.currentTarget as HTMLInputElement).value);
    setSpeakVolume(v / 100);
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
    dockDragging = false;
    hoverArmed = false;
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
    class:dragging={dockDragging}
    class:hover-armed={hoverArmed}
    onmousedown={onChromeDown}
  >
  {#if cheatSheetHint}
    <div class="actionrow">
      <button
        type="button"
        class="actionhint"
        onclick={() => onCheatSheet?.()}
        title="Cheat sheet — ⌘⇧/"
        aria-label="Open cheat sheet"
      >Action: ⌘?(⌘⇧/)</button>
    </div>
  {/if}
  <!-- Agent row: left cluster / dests (star at window center) / right. -->
  <div class="agentrow">
    <div class="agentleft">
    <!-- Always-on purple well left of
         the agent pick. Glow/pulse only while sending or hackLive.
         Not a button: it drags with the dock chrome like any gap. -->
    <span
      class="hackled"
      class:on={hackOn}
      role="img"
      aria-label={hackOn ? "agent working" : "agent idle"}
      title={hackTitle}
    ></span>
    <span class="agentwrap">
      <button
        id="companion-agent"
        type="button"
        class="agentpick"
        onclick={() => (agentOpen = !agentOpen)}
        disabled={sending}
        aria-haspopup="listbox"
        aria-expanded={agentOpen}
        aria-label="Which agent soul is the companion"
      >{agentLabel(agent)}</button>
      {#if agentOpen}
        <div
          class="agentbubble"
          role="listbox"
          aria-label="Agents"
          onpointerdown={(e) => e.stopPropagation()}
        >
          {#each agents as a}
            <button
              type="button"
              class="agentopt"
              class:on={a === agent}
              role="option"
              aria-selected={a === agent}
              onclick={() => pickAgent(a)}
            >{agentLabel(a)}</button>
          {/each}
        </div>
      {/if}
    </span>
    <button
      class="expandbtn iconbtn promptsbtn"
      class:on={prompts}
      onclick={togglePrompts}
      aria-pressed={prompts}
      aria-label="STARS prompts"
      title="STARS prompts — one quiet strip when today's check-in has a thin spoke. Off by default."
    >
      <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M9 3l1.4 4.6L15 9l-4.6 1.4L9 15l-1.4-4.6L3 9l4.6-1.4z" />
        <path d="M17 11l.9 2.9 2.9.9-2.9.9-.9 2.9-.9-2.9-2.9-.9 2.9-.9z" />
        <path d="M16.5 3l.7 2.3 2.3.7-2.3.7-.7 2.3-.7-2.3-2.3-.7 2.3-.7z" />
      </svg>
    </button>
    {#if nextCount > 0}
      <button
        class="nextpip iconbtn"
        onclick={toggleNextCard}
        aria-label="Next card — {nextCount} Needed"
        data-chip="Needed"
        title="Next card (⌘⇧N)"
      >
        <span class="nextpip-n">{nextCount > 9 ? "9+" : nextCount}</span>
      </button>
    {/if}
    </div>
{#if showDests}
    <span class="dests" role="group" aria-label="Destinations">
      <button
        class="dest iconbtn"
        class:on={destOpen.launchpad}
        onclick={() => toggleDest("launchpad")}
        aria-label="Launchpad"
        data-chip="Launchpad"
        title={destOpen.launchpad
          ? "Launchpad is open — click to hide"
          : "Launchpad — click to open"}
      >
        <!-- DEST-CHROME: the same ship flying ↗ — the blessed rocket
             rotated 45° so the nose rides upper-right, never the
             upright capsule. -->
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <g transform="rotate(45 12 12)">
            <path
              fill-rule="evenodd"
              d="M12 2.5 C14.6 4.6 16 7.9 16 11.2 C16 13.1 15.7 14.9 15.2 16.2 L8.8 16.2 C8.3 14.9 8 13.1 8 11.2 C8 7.9 9.4 4.6 12 2.5 Z M10.5 9.3 a1.5 1.5 0 1 0 3 0 a1.5 1.5 0 1 0 -3 0 Z"
            />
            <path d="M8.7 12.3 C7.2 13.4 6.2 15.4 5.8 17.8 L8.9 16.6 C8.6 15.2 8.6 13.7 8.7 12.3 Z" />
            <path d="M15.3 12.3 C16.8 13.4 17.8 15.4 18.2 17.8 L15.1 16.6 C15.4 15.2 15.4 13.7 15.3 12.3 Z" />
            <path d="M12 17.6 C12.8 18.7 13.2 19.8 13.2 21 C12.5 20.6 11.5 20.6 10.8 21 C10.8 19.8 11.2 18.7 12 17.6 Z" />
          </g>
        </svg>
      </button>
      <button
        class="dest iconbtn"
        class:on={destOpen.stars}
        onclick={() => toggleDest("stars")}
        aria-label="STARS"
        data-chip="STARS"
        title={destOpen.stars
          ? "STARS is open — click to hide"
          : "STARS — click to open"}
      >
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M12 3 L14.1 9.1 L20.6 9.2 L15.4 13.1 L17.3 19.3 L12 15.6 L6.7 19.3 L8.6 13.1 L3.4 9.2 L9.9 9.1 Z" />
        </svg>
      </button>
      <button
        class="dest iconbtn planetsdest"
        class:on={!planetsHidden}
        onclick={onPlanetsClick}
        oncontextmenu={onPlanetsRight}
        aria-label="Planets"
        data-chip="Planets"
        title={!planetsHidden
          ? "Planets on — click to hide discs. Right-click toggles Inspector"
          : "Planets off — click to show discs. Right-click toggles Inspector"}
      >
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <circle cx="12" cy="12" r="5" />
          <ellipse
            cx="12"
            cy="12"
            rx="9.4"
            ry="3.3"
            transform="rotate(-20 12 12)"
            fill="none"
            stroke="currentColor"
            stroke-width="1.7"
          />
        </svg>
      </button>
    </span>
{/if}
    <div class="agentright">
    <span class="volwrap">
      <button
        class="expandbtn iconbtn speakbtn"
        class:on={speak && speakVolume > 0}
        class:muted={!speak || speakVolume <= 0}
        onclick={onSpeakClick}
        oncontextmenu={onSpeakRight}
        aria-pressed={speak && speakVolume > 0}
        aria-label="Companion volume"
        title="Click: volume · Right-click: mute"
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <polygon
            points="11 5 6 9 2.5 9 2.5 15 6 15 11 19 11 5"
            fill="currentColor"
            stroke="none"
          />
          <path d="M15 9.2a4.2 4.2 0 0 1 0 5.6" />
          {#if !speak || speakVolume <= 0}
            <line x1="5.6" y1="18.4" x2="18.4" y2="5.6" />
          {/if}
        </svg>
      </button>
      {#if volOpen}
        <div
          class="volpop"
          role="dialog"
          aria-label="Companion volume"
          onpointerdown={(e) => e.stopPropagation()}
        >
          <input
            type="range"
            min="0"
            max="100"
            step="1"
            value={Math.round(speakVolume * 100)}
            oninput={onVolInput}
            aria-label="Volume"
          />
        </div>
      {/if}
    </span>
    <button
      class="expandbtn iconbtn viewbtn"
      class:on={headShowing}
      class:muted={!sprite}
      onclick={toggleView}
      oncontextmenu={(e) => {
        e.preventDefault();
        toggleSprite(e);
      }}
      disabled={viewBusy}
      aria-pressed={headShowing}
      aria-label="Head or full body, or right-click to hide sprite"
      title={viewTitle}
    >
      {#if headShowing}
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <circle cx="12" cy="8" r="4.4" />
          <path d="M4.6 21c.6-4 3.6-6.4 7.4-6.4s6.8 2.4 7.4 6.4z" />
          {#if !sprite}
            <line
              x1="5.6"
              y1="18.4"
              x2="18.4"
              y2="5.6"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
            />
          {/if}
        </svg>
      {:else}
        <!-- Full body: restroom-man (head + shouldered torso + two legs),
             never the circle + rounded-rect that read as a fat i. -->
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <circle cx="12" cy="4.6" r="2.6" />
          <path
            d="M9.2 8.4h5.6c1.3 0 2.2 1 2.2 2.2v4.4h-1.6v-3.6h-.6V21h-1.9v-6h-1.8v6H9.4v-9.6h-.6V15H7.2v-4.4c0-1.2.9-2.2 2-2.2z"
          />
          {#if !sprite}
            <line
              x1="5.6"
              y1="18.4"
              x2="18.4"
              y2="5.6"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
            />
          {/if}
        </svg>
      {/if}
    </button>
    <button
      class="expandbtn iconbtn"
      onclick={toggleExpanded}
      aria-label={expanded ? "Collapse transcript" : "Expand transcript"}
      title={expanded ? "Collapse transcript" : "Expand transcript"}
    >
      {#if expanded}
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <line x1="18" y1="6" x2="7.5" y2="16.5" />
          <polyline points="15.5 16.5 7.5 16.5 7.5 8.5" />
        </svg>
      {:else}
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <line x1="6" y1="18" x2="16.5" y2="7.5" />
          <polyline points="8.5 7.5 16.5 7.5 16.5 15.5" />
        </svg>
      {/if}
    </button>
    <button
      type="button"
      class="gear"
      onclick={() => invoke("toggle_labeled", { label: "settings" }).catch(() => {})}
      title="Settings — click again to hide"
      aria-label="Toggle Settings"
    >
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <circle cx="12" cy="12" r="3" />
        <path
          d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 0 1-4 0v-.2a1.7 1.7 0 0 0-1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 0 1 0-4h.2a1.7 1.7 0 0 0 1.5-1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 0 1 4 0v.2a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9c.3.6.9 1 1.5 1H21a2 2 0 0 1 0 4h-.2a1.7 1.7 0 0 0-1.5 1z"
        />
      </svg>
    </button>
    </div>
  </div>

  <form
    class="composer"
    onsubmit={(e) => {
      e.preventDefault();
      sendLine();
    }}
  >
    {#if attach}
      <!-- The one chip — name + size, × to drop.
           Vanishes on send (CompanionPage clears it); an honest send
           error brings it back. -->
      <div class="chiprow">
        <span class="attachchip" title={attach.path}>
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.9"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path
              d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48"
            />
          </svg>
          <span class="attachname">{attach.name}</span>
          <span class="attachsize">{fmtSize(attach.size)}</span>
          <button
            type="button"
            class="attachx"
            onclick={onDropAttach}
            aria-label="Drop file"
            title="Drop file — it was never copied"
          >×</button>
        </span>
      </div>
    {/if}
    <button
      type="button"
      class="thinbtn"
      onmousedown={(e) => e.stopPropagation()}
      onpointerdown={(e) => e.stopPropagation()}
      onclick={() => onThin?.()}
      disabled={sending}
      aria-label="Thin — label this chat"
      title="Label this chat thin (⌘⌥T). Mouth keeps going."
    >thin</button>
    <div
      class="fieldwrap"
      class:talking
      onmousedown={(e) => e.stopPropagation()}
      onpointerdown={(e) => e.stopPropagation()}
    >
      <button
        type="button"
        class="fieldplus"
        onmousedown={(e) => e.stopPropagation()}
        onpointerdown={(e) => e.stopPropagation()}
        onclick={onPickAttach}
        disabled={sending}
        aria-label="Attach one file"
        title="Attach one file (25 MB cap)"
      >+</button>
      <textarea
        class="field"
        rows="1"
        bind:this={fieldEl}
        bind:value={draft}
        onmousedown={(e) => e.stopPropagation()}
        onpointerdown={(e) => e.stopPropagation()}
        onkeydown={onFieldKeydown}
        placeholder={neededHint
          ? `answer: ${neededHint}  ·  Rule: … to remember`
          : "type…  ·  todo: buy milk -fkt"}
        aria-label="Message your companion"
        autocomplete="off"
        spellcheck="false"
      ></textarea>
      <span class="talkslot">
        {#if talking}
          <button
            type="button"
            class="talkic"
            onclick={talkStopOnly}
            aria-label="Stop talking — text stays for review"
            title="Stop talking — the text stays in the field; you hit Send"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.6"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <polyline points="4.5 12.5 9.5 17.5 19.5 6.5" />
            </svg>
          </button>
          <button
            type="button"
            class="talkic sendnow"
            onclick={talkStopSend}
            aria-label="Stop and send now"
            title="Stop and send now"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.6"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <line x1="12" y1="19" x2="12" y2="5" />
              <polyline points="5.5 11.5 12 5 18.5 11.5" />
            </svg>
          </button>
        {:else}
          <button
            type="button"
            class="talkic"
            onclick={talkStart}
            aria-label="Talk"
            title="Click to talk. Source is Settings → Talk. Text lands in the field; you hit Send"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.9"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <rect x="9" y="2.5" width="6" height="11.5" rx="3" />
              <path d="M5.5 10.5a6.5 6.5 0 0 0 13 0" />
              <line x1="12" y1="17.5" x2="12" y2="21" />
            </svg>
          </button>
        {/if}
      </span>
    </div>
    <button
      class="send"
      type="submit"
      disabled={sending || (!draft.trim() && !attach)}
      aria-label="Send"
      title="Send"
    >
      <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M12 4l8 11h-4.5L12 10.4 8.5 15H4z" />
      </svg>
    </button>
  </form>
</div>

