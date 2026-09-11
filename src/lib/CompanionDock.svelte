<script lang="ts">
  // GO-ORBITTY-CLEAN-COMPANION-DOCK: the dock — agent pick, Prompts,
  // dests Launchpad/STARS/Planets (hover chips, diagonal rocket), Speak,
  // Full/Head sprite toggle, Expand, ⚙ last after Expand (GEAR-DOCK),
  // composer. Paint + presses only: CompanionPage owns all state, the
  // sends (the field still calls companion_send), and the well. Planets
  // dest L-click = toggleDest("inspector"). R-click = planet menu
  // (Add + live names) — not a dest toggle. × stays a CompanionPage corner.
  // GO-ORBITTY-DOCK-HACK: the live LED sits immediately left of the agent
  // pick (sending || hackLive); the pick hugs its labels (46% cap dead).
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { planetCss } from "./planetColor";
  import type { Project } from "./persist";

  // GO-ORBITTY-FOURCAST-RECAST: chrome labels only. The option value stays
  // the stem id so set_companion_agent and the on-disk allowlist are
  // untouched; an unknown stem fails open to the raw id.
  const AGENT_LABELS: Record<string, string> = {
    spark: "Spark",
    grok: "Work",
    "chaos-gremlin": "Chat",
    "fkt-homie": "Hold",
  };
  const agentLabel = (id: string) => AGENT_LABELS[id] ?? id;

  // GO-ORBITTY-DOCK-HACK: one 7px LED immediately left of the agent pick
  // — lit while ANY agent is working: the Spark mouth (sending) or the
  // ctl hack flag (companion.hackLive; a harness opts in). Off = not in
  // the DOM, never a dim grey. Not a button, not a count.
  const HACK_WHO_TITLES: Record<string, string> = {
    cursor: "Cursor leftover",
    goose: "Goose",
    grok: "Grok",
    hermes: "Hermes",
    spark: "Spark mouth",
  };

  type HostMode = "off" | "launchpad" | "stars" | "inspector";

  let {
    agents,
    agent = $bindable(),
    sending,
    hackLive,
    hackWho,
    hackProjectId = null,
    prompts,
    hosted,
    speak,
    headShowing,
    viewBusy,
    sprite,
    bustOk,
    viewTitle,
    expanded,
    talking,
    draft = $bindable(),
    fieldEl = $bindable(),
    composerExtra = $bindable(0),
    onAgentChange,
    togglePrompts,
    toggleDest,
    nextCount = 0,
    toggleNextCard,
    onNewPlanet,
    planets = [],
    onPlanetClick,
    onPlanetDblClick,
    toggleSpeak,
    toggleSprite,
    toggleView,
    toggleExpanded,
    sendLine,
    talkStart,
    talkStopOnly,
    talkStopSend,
    onDockMouseDown,
  }: {
    agents: string[];
    agent: string;
    sending: boolean;
    hackLive: boolean;
    hackWho: string;
    // AGENT-PLANET leftover: the live agent's pinned planet — the planet
    // menu below glows that one planet while the run is live.
    hackProjectId?: string | null;
    prompts: boolean;
    hosted: HostMode;
    speak: boolean;
    headShowing: boolean;
    viewBusy: boolean;
    sprite: boolean;
    bustOk: boolean;
    viewTitle: string;
    expanded: boolean;
    talking: boolean;
    draft: string;
    fieldEl: HTMLTextAreaElement | null;
    // COLLAPSE-CC: px the composer grew past its one-line idle (collapsed
    // only — expanded/hosted keep the 6-line cap and report 0).
    // CompanionPage turns it into the native hug / window grow.
    composerExtra?: number;
    onAgentChange: () => void;
    togglePrompts: () => void;
    toggleDest: (mode: "launchpad" | "stars" | "inspector") => void;
    nextCount?: number;
    toggleNextCard: () => void;
    onNewPlanet: () => void;
    planets?: Project[];
    onPlanetClick: (id: string) => void;
    onPlanetDblClick: (id: string) => void;
    toggleSpeak: () => void;
    toggleSprite: (e: MouseEvent) => void;
    toggleView: () => void;
    toggleExpanded: () => void;
    sendLine: () => void;
    talkStart: (e: Event) => void;
    talkStopOnly: () => void;
    talkStopSend: () => void;
    onDockMouseDown: (e: MouseEvent) => void;
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

  let planetsMenuOpen = $state(false);
  let skipDestClick = false;
  let planetsWrapEl: HTMLSpanElement | null = $state(null);

  function onPlanetsClick() {
    if (skipDestClick) {
      skipDestClick = false;
      return;
    }
    planetsMenuOpen = false;
    toggleDest("inspector");
  }

  function onPlanetsContext(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    skipDestClick = true;
    planetsMenuOpen = !planetsMenuOpen;
    setTimeout(() => {
      skipDestClick = false;
    }, 300);
  }

  function addPlanetFromMenu() {
    planetsMenuOpen = false;
    onNewPlanet();
  }

  $effect(() => {
    if (!planetsMenuOpen) return;
    const onDown = (ev: MouseEvent) => {
      const n = ev.target as Node | null;
      if (n && planetsWrapEl?.contains(n)) return;
      planetsMenuOpen = false;
    };
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") planetsMenuOpen = false;
    };
    const t = setTimeout(() => {
      window.addEventListener("mousedown", onDown, true);
      window.addEventListener("keydown", onKey, true);
    }, 0);
    return () => {
      clearTimeout(t);
      window.removeEventListener("mousedown", onDown, true);
      window.removeEventListener("keydown", onKey, true);
    };
  });

  // COMPOSER-WRAP: the one-line field is a wrapping textarea now (Grok
  // web composer). Enter sends, Shift+Enter newline, ⌘↩/Ctrl+↩ send.
  // Auto-grows from scrollHeight (WKWebView-safe — no field-sizing).
  // GO-ORBITTY-COLLAPSE-CC: collapsed drops the 6-line cap — never an
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

  function growField() {
    const el = fieldEl;
    if (!el) return;
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

  onMount(() => {
    measureField();
    // Tile open/close widens the window → rewrap → re-grow without
    // waiting for the next keystroke. RO fires only on real size
    // deltas, so the grow writes above do not loop it.
    const ro = new ResizeObserver(() => growField());
    if (fieldEl) ro.observe(fieldEl);
    return () => ro.disconnect();
  });
</script>

<!-- GO-ORBITTY-GHOST-DOCK: agentrow + composer ride in one wrap — the
     ghost's outlined bottom slab. × stays a floating corner; ⚙ is on
     the dock after Expand (GEAR-DOCK). -->
<div class="ghostdock" onmousedown={onDockMouseDown}>
  <!-- Agent row: one strip — agent pick + icon toggles (Prompts, Speak,
       View, Expand/Collapse). The Body picker lives in Settings →
       Appearance. Icons are inline SVG, currentColor, 16px. -->
  <div class="agentrow">
    <!-- GO-ORBITTY-DOCK-HACK leftover: always-on purple well left of
         the agent pick. Glow/pulse only while sending or hackLive.
         Not a button: it drags with the dock chrome like any gap. -->
    <span
      class="hackled"
      class:on={hackOn}
      role="img"
      aria-label={hackOn ? "agent working" : "agent idle"}
      title={hackTitle}
    ></span>
    <select
      id="companion-agent"
      class="agentpick"
      bind:value={agent}
      onchange={onAgentChange}
      disabled={sending}
      aria-label="Which agent soul is the companion"
    >
      {#each agents as a}
        <option value={a}>{agentLabel(a)}</option>
      {/each}
    </select>
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
    <span class="dests" role="group" aria-label="Destinations">
      <button
        class="dest iconbtn"
        class:on={hosted === "launchpad"}
        onclick={() => toggleDest("launchpad")}
        aria-label="Launchpad"
        data-chip="Launchpad"
        title="Launchpad in the well — click again to dismiss"
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
        class:on={hosted === "stars"}
        onclick={() => toggleDest("stars")}
        aria-label="STARS"
        data-chip="STARS"
        title="STARS in the well — click again to dismiss"
      >
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M12 3 L14.1 9.1 L20.6 9.2 L15.4 13.1 L17.3 19.3 L12 15.6 L6.7 19.3 L8.6 13.1 L3.4 9.2 L9.9 9.1 Z" />
        </svg>
      </button>
      <span class="planetsdest-wrap" bind:this={planetsWrapEl}>
        <button
          class="dest iconbtn planetsdest"
          class:on={hosted === "inspector"}
          onclick={onPlanetsClick}
          oncontextmenu={onPlanetsContext}
          aria-label="Planets. Right-click for Add planet and the live list"
          data-chip="Planets"
          title="Planets in the well — click again to dismiss. Right-click: Add planet and names"
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
        {#if planetsMenuOpen}
          <div class="planetspop" role="menu" aria-label="Planets">
            <button
              type="button"
              class="planetsrow"
              role="menuitem"
              onclick={addPlanetFromMenu}>+ Planet</button
            >
            {#if planets.length === 0}
              <div class="planetsrow empty">No planets yet</div>
            {:else}
              {#each planets as p (p.id)}
                <button
                  type="button"
                  class="planetsrow"
                  role="menuitem"
                  onclick={() => onPlanetClick(p.id)}
                  ondblclick={() => onPlanetDblClick(p.id)}
                >
                  <!-- AGENT-PLANET leftover: the planet an agent is
                       writing on glows a little (its own color); every
                       other planet stays idle. -->
                  <span
                    class="planet-chip"
                    class:live={hackLive && hackProjectId === p.id}
                    style="background: {planetCss(p.id)}; --pglow: {planetCss(p.id)}"
                    aria-hidden="true"
                  ></span>
                  {p.name}
                </button>
              {/each}
            {/if}
          </div>
        {/if}
      </span>
    </span>
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
    <button
      class="expandbtn iconbtn speakbtn"
      class:on={speak}
      onclick={toggleSpeak}
      aria-pressed={speak}
      aria-label="Speak replies"
      title="Read bot replies aloud with the macOS system voice"
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
        {#if !speak}
          <line x1="5.6" y1="18.4" x2="18.4" y2="5.6" />
        {/if}
      </svg>
    </button>
    <!-- DOCK-TWEAK: L-click hides/shows the sprite; R-click Full ↔ Head.
         The button stays ENABLED while the sprite is off — even with
         no bust art — so L-click can always bring it back. -->
    <button
      class="expandbtn iconbtn viewbtn"
      class:on={headShowing}
      onclick={toggleSprite}
      oncontextmenu={(e) => {
        e.preventDefault();
        toggleView();
      }}
      disabled={viewBusy || (sprite && !bustOk)}
      aria-pressed={headShowing}
      aria-label="Hide sprite, or right-click for Head or full body"
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
      onclick={() => invoke("open_settings_window").catch(() => {})}
      title="Open Settings"
      aria-label="Open Settings"
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

  <form
    class="composer"
    onsubmit={(e) => {
      e.preventDefault();
      sendLine();
    }}
  >
    <div class="fieldwrap" class:talking>
      <textarea
        class="field"
        rows="1"
        bind:this={fieldEl}
        bind:value={draft}
        onkeydown={onFieldKeydown}
        placeholder="type…"
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
      disabled={sending || !draft.trim()}
      aria-label="Send"
      title="Send"
    >
      <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M12 4l8 11h-4.5L12 10.4 8.5 15H4z" />
      </svg>
    </button>
  </form>
</div>

<style>
  /* GO-ORBITTY-GHOST-DOCK: one wrap around agentrow + composer. Ghost
     paints it as the small outlined bottom slab — same paint as the
     expanded window (--bg-fill, radius 12, 1px --edge) — floating just
     off the window edges. TILE-DROP leftover: hosted dests (Launchpad /
     STARS / Planets) keep that same ring so the dock stays findable and
     grabbable on the transparent desk. Idle expanded (not hosted) still
     rides the slab fill with no extra ring. DOCK-GRIP: dock chrome
     (padding around the rows, never a control) drags on every surface.
     HOST-DOCK leftover: bottom inset (8px margin + 1px border) stays in
     every mode so dest toggle does not walk the dests. Ghost / hosting
     add the side/top float and the visible ring. */
  .ghostdock {
    position: relative;
    display: flex;
    flex-direction: column;
    margin-bottom: 8px;
    border: 1px solid transparent;
  }

  :global(.ghost) .ghostdock,
  :global(.hosting) .ghostdock {
    margin: 4px 8px 8px;
    background: var(--bg-fill, var(--bg));
    border-color: var(--edge);
    border-radius: 12px;
  }

  /* Agent row: the agent soul pick + a strip of icon toggles (Prompts,
     Speak, View, Expand/Collapse). Ghost chrome gaps drag (DOCK-GRIP);
     the controls never do. */
  .agentrow {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px 0;
  }

  /* GO-ORBITTY-DOCK-HACK: the 46% max-width was artificially wide — the
     pick now hugs the labels (Spark / Work / Chat / Hold), making room
     for the hack LED on its left. Dests still give first at the 280px
     hug; the icon hit targets are never stolen. */
  .agentpick {
    flex: none;
    width: max-content;
    padding: 3px 6px;
    font: inherit;
    font-size: 0.78rem;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 8px;
    outline: none;
  }

  /* GO-ORBITTY-DOCK-HACK leftover: always-on 7px well, dock focus
     purple (#6e4db0) not theme teal. Dim when idle; glow+pulse when
     sending or companion.hackLive. */
  .hackled {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #6e4db0;
    opacity: 0.38;
    box-shadow: none;
  }

  .hackled.on {
    opacity: 1;
    box-shadow: 0 0 7px 1px #6e4db0;
    animation: orbitty-hack-pulse 1.2s ease-in-out infinite;
  }

  @keyframes orbitty-hack-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.45;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .hackled.on,
    .planet-chip.live {
      animation: none;
    }
  }

  .agentpick:focus {
    border-color: var(--focus, #6e4db0);
  }

  /* DEST-STRIP: the old spacer becomes the destination strip — quiet
     text (rail .stack-btn energy, scaled down), one line, flex:1 so the
     cluster holds the spark→speaker gap. At the 280px min the dests give
     first (type / letter-spacing / gap) — never the icon hit targets,
     never the agentpick. */
  .dests {
    flex: 1;
    min-width: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 3px;
    white-space: nowrap;
    position: relative;
  }

  /* PLANETS-POP: the name list opens UP from the dest strip, so it
     overlays the well / stage / CC / transcript and can never slide down
     over the composer. z-index 6 sits above the well + transcript (the
     chrome ×/gear are 2). Height-capped so a long world list scrolls
     inside itself instead of reaching the window top. */
  .planetsdest-wrap {
    position: relative;
    flex: none;
    display: inline-flex;
  }

  .planetspop {
    position: absolute;
    bottom: calc(100% + 8px);
    left: 50%;
    transform: translateX(-50%);
    z-index: 6;
    display: flex;
    flex-direction: column;
    min-width: 140px;
    max-width: 220px;
    max-height: 40vh;
    overflow-y: auto;
    padding: 4px;
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 10px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.28);
  }

  .planetsrow {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    font: inherit;
    font-size: 0.72rem;
    text-align: left;
    color: var(--ink);
    background: transparent;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .planet-chip {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  /* AGENT-PLANET leftover: a small glow in the planet's own color while
     its agent is writing (hackLive + that agent's projectId). Menus
     only — habitat discs and walkers never wear this.
     AGENT-PLANET planet-pulse leftover: the live chip breathes with the
     exact dock-LED cadence (orbitty-hack-pulse, 1.2s) — never a static
     glow. Only that one planet; the class (and the pulse) drops when
     the pickup source leaves. */
  .planet-chip.live {
    box-shadow: 0 0 7px 2px var(--pglow, var(--accent));
    animation: orbitty-hack-pulse 1.2s ease-in-out infinite;
  }

  .planetsrow:hover {
    background: var(--bg-fill, var(--bg));
  }

  .planetsrow.empty {
    color: var(--dim);
    cursor: default;
  }

  .planetsrow.empty:hover {
    background: transparent;
  }

  /* GO-ORBITTY-DEST-CHROME + DOCK-TWEAK: dests wear the same outlined
     pill as the other dock icon buttons — --panel-fill, 1px --edge,
     8px radius like .expandbtn — so Expand does not eat the rings. */
  .dest {
    flex: none;
    position: relative;
    padding: 2px 3px;
    font: inherit;
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    white-space: nowrap;
    color: var(--dim);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 8px;
    cursor: pointer;
  }

  .dest:hover {
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
  }

  /* DEST-CHROME: CSS hover chip naming the dest (title= never paints on
     the transparent panel). Opens UP from the button — over the stage /
     CC / well like .planetspop, never down over the composer. Same
     1px --edge ring + 8px radius as the pill; pointer-events none so it
     never steals the dest's own hover. */
  .dest::after {
    content: attr(data-chip);
    position: absolute;
    bottom: calc(100% + 6px);
    left: 50%;
    transform: translateX(-50%);
    z-index: 6;
    padding: 2px 7px;
    font-size: 0.62rem;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 8px;
    opacity: 0;
    pointer-events: none;
  }

  .dest:hover::after {
    opacity: 1;
  }

  /* HOST-SPLIT: the dest whose well is live. */
  .dest.on {
    color: var(--accent);
  }

  /* NEXT-CARD leftover: quiet ochre pip, not a dest chip, not red. */
  .nextpip {
    flex: none;
    position: relative;
    min-width: 22px;
    padding: 2px 6px;
    border-radius: 999px;
    border: 1px solid var(--edge);
    background: var(--panel-fill, var(--panel));
    color: var(--mark, #e59e3d);
    cursor: pointer;
  }
  .nextpip-n {
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0;
  }
  .nextpip:hover {
    border-color: var(--mark, #e59e3d);
  }

  .expandbtn {
    flex: none;
    padding: 4px 12px;
    font: inherit;
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 8px;
    cursor: pointer;
  }

  .expandbtn:hover,
  .iconbtn:hover {
    border-color: var(--focus, #6e4db0);
  }

  /* Icon toggles: same pill as the old text buttons, squared up for a
     16px glyph. GO-ORBITTY-COMPANION-BRAND: the on state is per-toggle
     brand — Prompts sparkle = ochre --mark, Speak = violet --focus,
     Head silhouette = --ink (never teal). Idle icons stay --ink.
     GO-ORBITTY-HABITAT-BRAND: every on/hover RING is the deeper violet
     --focus #6e4db0 — the sparkle FILL stays ochre, pinky #c793e6 is
     --bloom (words/tags), never the ring. */
  .iconbtn {
    padding: 4px 7px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .iconbtn svg {
    width: 16px;
    height: 16px;
    display: block;
  }

  .promptsbtn.on {
    color: var(--mark, #e59e3d);
    border-color: var(--focus, #6e4db0);
  }

  .speakbtn.on {
    color: var(--focus, #6e4db0);
    border-color: var(--focus, #6e4db0);
  }

  .viewbtn.on {
    color: var(--ink);
    border-color: var(--focus, #6e4db0);
  }

  .iconbtn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  /* GEAR-DOCK: tiny corner glyph, not a dock-pill icon. */
  .gear {
    flex: none;
    align-self: center;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    margin-left: 2px;
    padding: 0;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 3px;
    cursor: pointer;
  }

  .gear svg {
    width: 11px;
    height: 11px;
    display: block;
  }

  .gear:hover {
    color: var(--ink);
  }

  /* Composer: field with the mic parked inside its right edge + Send
     outside. Ghost chrome gaps drag (DOCK-GRIP); field / mic / Send
     never do. */
  .composer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px 10px;
  }

  /* COMPOSER-WRAP: same box as the 0224c1f input — padding, font,
     border, radius untouched. box-sizing declared (the input rode the
     UA form-control border-box) so the wrap math is deterministic;
     resize off, vertical scrollbar only past the 6-line cap (inline
     overflow-y from growField). rows=1 + height auto = the idle
     one-line height, pixel-matched to the old input. */
  .field {
    flex: 1;
    min-width: 0;
    box-sizing: border-box;
    padding: 7px 10px;
    font: inherit;
    font-size: 0.82rem;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 9px;
    outline: none;
    resize: none;
    overflow-y: hidden;
  }

  .field:focus {
    border-color: var(--focus, #6e4db0);
  }

  .field::placeholder {
    color: var(--dim);
  }

  .send {
    flex: none;
    padding: 7px 12px;
    font: inherit;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--on-accent, #ffffff);
    background: var(--accent);
    border: none;
    border-radius: 9px;
    cursor: pointer;
  }

  .send svg {
    width: 14px;
    height: 14px;
    display: block;
  }

  .send:disabled {
    opacity: 0.45;
    cursor: default;
  }

  /* Mic in the field: the wrap is relative and the slot parks the mic
     (idle) or check + up-arrow (recording) inside the right edge;
     padding-right keeps typed text from running under the icons.
     Check = stop (text stays for review); up-arrow = stop + send now. */
  .fieldwrap {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
  }

  .fieldwrap .field {
    width: 100%;
    padding-right: 32px;
  }

  .fieldwrap.talking .field {
    padding-right: 56px;
  }

  .talkslot {
    position: absolute;
    right: 5px;
    top: 50%;
    transform: translateY(-50%);
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }

  .talkic {
    width: 24px;
    height: 24px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
  }

  .talkic svg {
    width: 16px;
    height: 16px;
    display: block;
  }

  .talkic:hover {
    color: var(--accent);
  }

  .fieldwrap.talking .talkic {
    color: var(--ink);
  }

  .talkic.sendnow {
    color: var(--accent);
  }
</style>
