<script lang="ts">
  // STARS S1.1: the check-in radar lives in its own window (like Lists),
  // never stuffed into the rail. Five spokes 0–4 keyed by local YYYY-MM-DD;
  // explicit check-in only. GO-ORBITTY-STARS-LOOKBACK: an in-session day
  // cursor pages the saved map — past days are read-only, Save only ever
  // writes today, and the cursor itself is never persisted.
  // GO-ORBITTY-STARS-AVERAGE: one quiet mean line under the chip strip —
  // a rolling 7/14/30-day window ending at the day cursor, mean of saved
  // days only (a saved 0 counts; blanks shrink the denominator, never a
  // zero-fill). The switcher persists as stars.averageWindow, nothing else.
  // GO-ORBITTY-RAIL-REVIEW: banner segment Check-in | Review (default
  // Check-in). Check-in = the day pentagon + 0–4 + Save + Prompts on
  // --bg-fill. Review = the mean pentagon for averageWindow (the window
  // chips live here only) with the Cloud word field (.orbit-field) under
  // it — no 0–4 in Review, no mean pentagon in Check-in.
  // Same state.json discipline as Lists: every
  // write is load → mutate → save of the WHOLE loaded object, so rail-owned
  // slices (hideTitles included) round-trip untouched — this window only
  // ever touches the stars slice. The companion's Prompts strip reads the
  // same slice via native commands; its toggles land here on state:changed.
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount, onDestroy } from "svelte";
  import SkyField from "./SkyField.svelte";
  import { listenPaneVis } from "./paneVis";
  import {
    applyHostTokens,
    applyWindowOpacity,
    clampWindowOpacity,
  } from "../themes/apply";
  import type {
    AverageWindow,
    PersistedState,
    StarsCheckin,
    StarsSlice,
  } from "./persist";

  // GO-ORBITTY-BOARD-STARS: this page now mounts inside the one shared
  // 780×520 `kanban` overlay (stars mode) — hideLabel is the HOST window's
  // label, so the × and the re-show reset talk to the pane we're in. The
  // legacy 420×560 `stars` window passes nothing and keeps "stars".
  // GO-ORBITTY-HOST-SPLIT: embedded mounts this page in the companion's
  // STARS well — × calls onClose (never hide_window) and there is NO
  // pop-out. hideLabel stays the overlay's affair.
  let {
    hideLabel = "stars",
    embedded = false,
    onClose,
  }: {
    hideLabel?: string;
    embedded?: boolean;
    onClose?: () => void;
  } = $props();



  // GO-ORBITTY-COMPANION-BRAND: the seven palette skins are retired; a
  // dead saved id falls back to the OS pair. Terminal stays valid.
  const KNOWN_THEMES = new Set(["systemLight", "systemDark", "terminal"]);

  function osThemeId(): string {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "systemDark"
      : "systemLight";
  }

  let themeId = $state("");
  let windowOpacity = $state(1.0);

  const STARS_SPOKES = [
    { key: "sleep", label: "Sleep" },
    { key: "terroir", label: "Terroir" },
    { key: "athleticism", label: "Athleticism" },
    { key: "relationships", label: "Relationships" },
    { key: "self", label: "Self" },
  ] as const;
  type SpokeKey = (typeof STARS_SPOKES)[number]["key"];
  const emptyCheckin = (): StarsCheckin => ({
    sleep: 0,
    terroir: 0,
    athleticism: 0,
    relationships: 0,
    self: 0,
  });

  let starsState = $state<StarsSlice>({ promptsOptIn: false, checkins: {} });
  // The draft is the working copy — it only becomes state via Save today.
  let starsDraft = $state<StarsCheckin>(emptyCheckin());
  let starsSavedDay = $state("");
  // Mid-edit guard: a state:changed echo (companion Prompts toggle / strip
  // ack) must not wipe spoke clicks that were never saved.
  let draftDirty = false;

  function todayKey(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }

  // Banner segment (GO-ORBITTY-RAIL-REVIEW): Check-in is the default.
  // Session-only — a re-show always lands back on Check-in.
  let mode = $state<"checkin" | "review">("checkin");

  // Day cursor: which saved day the radar + spoke rows are showing.
  let cursorDay = $state(todayKey());
  const isToday = $derived(cursorDay === todayKey());
  const cursorSaved = $derived(starsState.checkins[cursorDay]);
  // Today shows the live draft; a past day shows its save, or null for an
  // empty day (blank pentagon / no active 0–4 — never a warning).
  const shown = $derived<StarsCheckin | null>(
    isToday ? starsDraft : (cursorSaved ?? null),
  );
  // Saved-day strip: dates that actually have a check-in, newest last,
  // capped ~14 chips — anything older is reachable via Prev.
  const savedDays = $derived(Object.keys(starsState.checkins).sort());
  const chipDays = $derived(savedDays.slice(-14));

  // Range average (GO-ORBITTY-STARS-AVERAGE): rolling calendar window of
  // 7/14/30 days ending at the day cursor. Only saved days enter the mean
  // — an unsaved day shrinks the denominator, a saved 0 still counts.
  const WINDOWS: Record<AverageWindow, number> = { week: 7, "14": 14, month: 30 };
  const WINDOW_CHOICES: AverageWindow[] = ["week", "14", "month"];
  // Spoke initials in radar order — Sleep·Terroir·Athleticism·Relationships·Self.
  const SPOKE_LETTERS = ["S", "T", "A", "R", "S"] as const;
  const averageWindow = $derived<AverageWindow>(
    starsState.averageWindow === "week" || starsState.averageWindow === "month"
      ? starsState.averageWindow
      : "14",
  );
  const windowSaves = $derived.by(() => {
    const start = shiftDay(cursorDay, -(WINDOWS[averageWindow] - 1));
    return Object.keys(starsState.checkins)
      .filter((d) => d >= start && d <= cursorDay)
      .map((d) => starsState.checkins[d]);
  });
  // null = empty window → "No saves in this window", never a zero pentagon.
  const windowMeanLine = $derived.by(() => {
    const saves = windowSaves;
    if (!saves.length) return null;
    const parts = STARS_SPOKES.map(({ key }, i) => {
      const sum = saves.reduce((acc, c) => acc + (c[key] ?? 0), 0);
      return `${SPOKE_LETTERS[i]} ${(sum / saves.length).toFixed(1)}`;
    });
    return `${WINDOWS[averageWindow]}-day mean · ${parts.join("  ")}`;
  });
  // Review mode draws the mean itself — same window, same saved-days rule
  // as the line above. null = empty window → grid only, never zero-fill.
  const windowMean = $derived.by<StarsCheckin | null>(() => {
    const saves = windowSaves;
    if (!saves.length) return null;
    const mean = emptyCheckin();
    for (const { key } of STARS_SPOKES) {
      mean[key] =
        saves.reduce((acc, c) => acc + (c[key] ?? 0), 0) / saves.length;
    }
    return mean;
  });

  function shiftDay(key: string, delta: number): string {
    const [y, m, d] = key.split("-").map(Number);
    const dt = new Date(y, m - 1, d);
    dt.setDate(dt.getDate() + delta);
    return `${dt.getFullYear()}-${String(dt.getMonth() + 1).padStart(2, "0")}-${String(dt.getDate()).padStart(2, "0")}`;
  }
  function prevDay() {
    cursorDay = shiftDay(cursorDay, -1);
  }
  function nextDay() {
    const n = shiftDay(cursorDay, 1);
    if (n <= todayKey()) cursorDay = n; // never invent tomorrow
  }

  function syncStarsDraft() {
    starsDraft = { ...emptyCheckin(), ...starsState.checkins[todayKey()] };
    draftDirty = false;
  }

  async function loadState() {
    try {
      const s = await invoke<PersistedState>("load_state");
      // "" means the rail follows the OS; retired ids resolve the same way.
      themeId = KNOWN_THEMES.has(s.themeId) ? s.themeId : osThemeId();
      windowOpacity = clampWindowOpacity(s.windowOpacity ?? 1);
      starsState = s.stars ?? { promptsOptIn: false, checkins: {} };
      if (!draftDirty) syncStarsDraft();
    } catch {
      // next state:changed / visibility refresh retries
    }
  }

  function setSpoke(key: SpokeKey, v: number) {
    if (!isToday) return; // past days are read-only
    starsDraft = { ...starsDraft, [key]: v };
    starsSavedDay = "";
    draftDirty = true;
  }

  // Save today: load fresh, replace only today's check-in inside the stars
  // slice, save the whole object back — every other slice survives byte-for-
  // byte (no defaults are ever invented here).
  async function saveStarsToday() {
    if (!isToday) return; // Save only ever writes today
    const day = todayKey();
    const clean = emptyCheckin();
    for (const { key } of STARS_SPOKES) {
      clean[key] = Math.max(0, Math.min(4, Math.round(starsDraft[key] ?? 0)));
    }
    try {
      const s = await invoke<PersistedState>("load_state");
      const stars = s.stars ?? { promptsOptIn: false, checkins: {} };
      stars.checkins = { ...stars.checkins, [day]: clean };
      s.stars = stars;
      await invoke("save_state", { state: s });
      starsState = stars;
      starsSavedDay = day;
      draftDirty = false;
    } catch {
      // next Save retries; the draft stays put
    }
  }

  // Window switcher: same load → mutate → save-whole discipline as Save
  // today, but it only ever sets stars.averageWindow — checkins untouched.
  async function setAverageWindow(w: AverageWindow) {
    if (w === averageWindow) return;
    starsState = { ...starsState, averageWindow: w };
    try {
      const s = await invoke<PersistedState>("load_state");
      const stars = s.stars ?? { promptsOptIn: false, checkins: {} };
      stars.averageWindow = w;
      s.stars = stars;
      await invoke("save_state", { state: s });
      starsState = stars;
    } catch {
      // state:changed / next load restores the truth
    }
  }

  // Prompts opt-in rides the native load-modify-save command (same one the
  // companion uses) — never a wholesale state write from here.
  async function toggleStarsPrompts() {
    const next = !starsState.promptsOptIn;
    starsState = { ...starsState, promptsOptIn: next };
    try {
      await invoke("set_stars_prompts", { enabled: next });
    } catch {
      // state:changed / next load restores the truth
    }
  }

  // Radar geometry: five axes from the top, 72° apart; value 0–4 maps to
  // 0–R. Even polygon = calm; a thin spoke just reads short — no alarm
  // color anywhere (chunky teal fill / ochre stroke, like the mark).
  const RADAR_R = 52;
  function radarVertex(i: number, v: number): [number, number] {
    const a = -Math.PI / 2 + (i * 2 * Math.PI) / STARS_SPOKES.length;
    const r = (Math.max(0, Math.min(4, v)) / 4) * RADAR_R;
    return [Math.cos(a) * r, Math.sin(a) * r];
  }
  function radarRing(v: number): string {
    return STARS_SPOKES.map((_, i) => radarVertex(i, v).join(",")).join(" ");
  }
  function radarPointsFor(c: StarsCheckin | null): string {
    return c
      ? STARS_SPOKES.map((s, i) => radarVertex(i, c[s.key]).join(",")).join(
          " ",
        )
      : "";
  }
  function radarDotsFor(c: StarsCheckin | null) {
    return c
      ? STARS_SPOKES.map((s, i) => {
          const [x, y] = radarVertex(i, c[s.key]);
          return { key: s.key, x, y };
        })
      : [];
  }
  const radarPoints = $derived(radarPointsFor(shown));
  const radarDots = $derived(radarDotsFor(shown));
  // Review: the mean pentagon — never the day, never a zero-fill.
  const reviewPoints = $derived(radarPointsFor(windowMean));
  const reviewDots = $derived(radarDotsFor(windowMean));

  $effect(() => {
    if (!themeId) return;
    document.documentElement.dataset.theme = themeId;
    applyHostTokens(themeId);
    // Re-assert the native transparency surgery post-load.
    invoke("ensure_glass").catch(() => {});
  });

  // Reads themeId too so a theme flip re-derives the rgba fills (the theme
  // effect above runs first and sets the tokens).
  $effect(() => {
    if (!themeId) return;
    applyWindowOpacity(windowOpacity);
  });

  // The host window is created hidden and hidden again on close — a
  // re-show reloads from disk, resets the draft to the saved day, and
  // returns the lookback cursor to today (lookback is in-session only).
  // Hide-not-destroy panes never flip document.hidden, so the native
  // pane-vis event is the real re-show signal; visibilitychange stays as a
  // belt-and-suspenders for genuine occlusion flips.
  const onReshow = () => {
    starsSavedDay = "";
    draftDirty = false;
    cursorDay = todayKey();
    loadState();
  };
  const onVisibility = () => {
    if (!document.hidden) onReshow();
  };

  let unlisten: (() => void) | undefined;
  let unlistenVis: (() => void) | undefined;
  onMount(async () => {
    invoke("ensure_glass").catch(() => {});
    await loadState();
    syncStarsDraft();
    unlisten = await listen("state:changed", loadState);
    // HOST-SPLIT: embedded in the companion well there is no host-pane
    // vis channel to gate on — state:changed keeps the slice fresh.
    if (!embedded) {
      unlistenVis = await listenPaneVis(hideLabel, (visible) => {
        if (visible) onReshow();
      });
    }
    document.addEventListener("visibilitychange", onVisibility);
  });
  onDestroy(() => {
    unlisten?.();
    unlistenVis?.();
    document.removeEventListener("visibilitychange", onVisibility);
  });

  function onBannerMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    const t = e.target as HTMLElement | null;
    if (t?.closest("button, input, a, select, textarea")) return;
    e.preventDefault();
    getCurrentWindow()
      .startDragging()
      .catch(() => {});
  }
</script>

<main class="stars" class:embedded>
  <!-- One drag source: JS startDragging on this banner. Drawers are
       borderless — no native Overlay strip competing with the click.
       HOST-SPLIT: embedded, this drags the host companion window. -->
  <header class="s-banner" onmousedown={onBannerMouseDown}>
    <button
      class="winx"
      onmousedown={(e) => e.stopPropagation()}
      onclick={() =>
        embedded
          ? onClose?.()
          : invoke("hide_window", { label: hideLabel }).catch(() => {})}
      title={embedded
        ? "Dismiss the STARS well — the companion stays"
        : "Hide STARS — reopen from the rail"}
      aria-label={embedded ? "Dismiss STARS" : "Hide STARS"}>×</button
    >
    <h1><span class="mark"><span class="o">O</span><span class="rest">rb</span><span class="idot">i</span><span class="pop">ty</span></span> STARS</h1>
    <span class="seg" role="group" aria-label="STARS mode">
      <button
        class="seg-btn"
        class:active={mode === "checkin"}
        aria-pressed={mode === "checkin"}
        onmousedown={(e) => e.stopPropagation()}
        onclick={() => (mode = "checkin")}>Check-in</button
      >
      <button
        class="seg-btn"
        class:active={mode === "review"}
        aria-pressed={mode === "review"}
        onmousedown={(e) => e.stopPropagation()}
        onclick={() => (mode = "review")}>Review</button
      >
    </span>
  </header>

  <div class="s-body" class:review={mode === "review"}>
    {#if mode === "checkin"}
      <section class="stars-panel" aria-label="STARS check-in">
        <h2>STARS — Spiritual / Self is the fifth spoke</h2>
        <div class="day-nav">
          <button class="mini" onclick={prevDay} title="Previous day" aria-label="Previous day">‹</button>
          <span class="day-label">{isToday ? `Today · ${cursorDay}` : cursorDay}</span>
          <button
            class="mini"
            onclick={nextDay}
            disabled={isToday}
            title="Next day"
            aria-label="Next day">›</button>
        </div>
        <div class="checkin-grid">
          <div class="stars-radar">
            <svg
              viewBox="-62 -62 124 124"
              role="img"
              aria-label={isToday ? "Today's STARS radar" : `STARS radar for ${cursorDay}`}
            >
              <polygon class="grid" points={radarRing(4)} />
              <polygon class="grid inner" points={radarRing(2)} />
              {#each STARS_SPOKES as _, i (i)}
                {@const [ax, ay] = radarVertex(i, 4)}
                <line class="axis" x1="0" y1="0" x2={ax} y2={ay} />
              {/each}
              {#if shown}
                <polygon class="shape" points={radarPoints} />
                {#each radarDots as d (d.key)}
                  <circle class="dot" cx={d.x} cy={d.y} r="2.6" />
                {/each}
              {/if}
            </svg>
          </div>
          <div class="stars-controls">
            {#each STARS_SPOKES as spoke (spoke.key)}
              <div class="spoke-row">
                <span class="spoke-label">{spoke.label}</span>
                <span class="spoke-btns" role="group" aria-label="{spoke.label} 0 to 4">
                  {#each [0, 1, 2, 3, 4] as v (v)}
                    <button
                      class="spoke-val"
                      class:active={shown?.[spoke.key] === v}
                      aria-pressed={shown?.[spoke.key] === v}
                      disabled={!isToday}
                      onclick={() => setSpoke(spoke.key, v)}
                    >{v}</button>
                  {/each}
                </span>
              </div>
            {/each}
          </div>
        </div>
        {#if chipDays.length}
          <div class="saved-strip" aria-label="Days with a saved check-in">
            {#each chipDays as day (day)}
              <button
                class="mini chip"
                class:active={cursorDay === day}
                aria-pressed={cursorDay === day}
                title={day}
                onclick={() => (cursorDay = day)}
              >{day.slice(5)}</button>
            {/each}
          </div>
        {/if}
        <div class="stars-foot">
          <button class="mini stars-save" disabled={!isToday} onclick={saveStarsToday}>
            Save today
          </button>
          <button
            class="mini"
            class:active={starsState.promptsOptIn}
            aria-pressed={starsState.promptsOptIn}
            onclick={toggleStarsPrompts}
            title="Companion prompts — one quiet strip when today's check-in has a thin spoke. Off by default."
          >Prompts: {starsState.promptsOptIn ? "On" : "Off"}</button>
          {#if isToday}
            {#if starsSavedDay}
              <span class="stars-note">Saved for {starsSavedDay}</span>
            {/if}
          {:else if cursorSaved}
            <span class="stars-note">Saved for {cursorDay}</span>
          {:else}
            <span class="stars-note">No save</span>
          {/if}
        </div>
      </section>
    {:else}
      <section class="stars-panel" aria-label="STARS review">
        <h2>STARS — Spiritual / Self is the fifth spoke</h2>
        <div class="stars-radar">
          <svg
            viewBox="-62 -62 124 124"
            role="img"
            aria-label="Mean STARS radar — {WINDOWS[averageWindow]}-day window ending at the shown day"
          >
            <polygon class="grid" points={radarRing(4)} />
            <polygon class="grid inner" points={radarRing(2)} />
            {#each STARS_SPOKES as _, i (i)}
              {@const [ax, ay] = radarVertex(i, 4)}
              <line class="axis" x1="0" y1="0" x2={ax} y2={ay} />
            {/each}
            {#if windowMean}
              <polygon class="shape" points={reviewPoints} />
              {#each reviewDots as d (d.key)}
                <circle class="dot" cx={d.x} cy={d.y} r="2.6" />
              {/each}
            {/if}
          </svg>
        </div>
        <div class="avg-row">
          <span class="avg-line" aria-live="polite">
            {windowMeanLine ?? "No saves in this window"}
          </span>
          <span class="avg-switch" role="group" aria-label="Average window">
            {#each WINDOW_CHOICES as w (w)}
              <button
                class="mini chip avg-chip"
                class:active={averageWindow === w}
                aria-pressed={averageWindow === w}
                title={`Mean over the last ${WINDOWS[w]} days ending at the shown day`}
                onclick={() => setAverageWindow(w)}
              >{w}</button>
            {/each}
          </span>
        </div>
      </section>
      <div class="review-sky">
        <SkyField />
      </div>
    {/if}
  </div>
</main>

<style>
  .stars {
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

  :global(.low-fill) .stars {
    text-shadow: 0 1px 2px var(--bg-solid, transparent);
  }

  /* GO-ORBITTY-HOST-SPLIT: embedded in the companion's STARS well — fill
     the well pane (the host window owns the outer size), square corners
     inside the companion's rounded clip. */
  .stars.embedded {
    height: auto;
    flex: 1;
    min-height: 0;
    border-radius: 0;
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

  /* The only close affordance: hides STARS, never quits (tray → Quit). */
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

  /* GO-ORBITTY-WORDMARK: ochre i-dot / ink O / accent ty. */
  .mark {
    text-shadow:
      1px 1px 0 rgba(0, 0, 0, 0.28),
      -1px -1px 0 rgba(255, 255, 255, 0.09);
  }

  .o {
    color: var(--ink);
    text-shadow:
      1px 1px 0 rgba(0, 0, 0, 0.42),
      -1px -1px 0 rgba(255, 255, 255, 0.14);
  }

  .idot {
    color: #e59e3d;
    text-shadow:
      1px 1px 0 rgba(0, 0, 0, 0.42),
      -1px -1px 0 rgba(255, 255, 255, 0.14);
  }

  .rest {
    color: var(--ink);
  }

  .pop {
    color: var(--accent);
  }

  /* Banner segment (GO-ORBITTY-RAIL-REVIEW): Check-in | Review. */
  .seg {
    margin-left: auto;
    display: inline-flex;
    gap: 2px;
    padding: 1px;
    border: 1px solid var(--edge);
    border-radius: 8px;
  }

  .seg-btn {
    font: inherit;
    font-size: 0.66rem;
    padding: 2px 8px;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 6px;
    cursor: pointer;
  }

  .seg-btn:hover {
    color: var(--ink);
  }

  .seg-btn.active {
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
  }

  .s-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 14px 14px;
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--dim) 45%, transparent) transparent;
  }

  /* Chunky radar (teal fill / ochre stroke, the mark's fixed colors) + five
     obvious 0–4 rows. Even polygon = calm; a thin spoke just reads short —
     never a red alarm. */
  .stars-panel {
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 10px;
    padding: 10px 12px 12px;
  }

  .stars-panel h2 {
    margin: 0 0 6px;
    font-size: 0.78rem;
    color: var(--accent);
  }

  .day-nav {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin: 2px 0 4px;
  }

  .day-label {
    min-width: 148px;
    text-align: center;
    font-size: 0.72rem;
    color: var(--ink);
    font-variant-numeric: tabular-nums;
  }

  .saved-strip {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 4px;
    margin-top: 2px;
  }

  .chip {
    padding: 2px 6px;
    font-variant-numeric: tabular-nums;
  }

  /* Range average (Review only): one quiet line under the mean radar,
     switcher as three tiny chips beside it — never a new window. */
  .avg-row {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-wrap: wrap;
    gap: 4px 8px;
    margin-top: 6px;
  }

  .avg-line {
    font-size: 0.66rem;
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }

  .avg-switch {
    display: inline-flex;
    gap: 3px;
  }

  .avg-chip {
    padding: 1px 6px;
    font-size: 0.62rem;
  }

  .stars-radar {
    display: flex;
    justify-content: center;
    padding: 4px 0 2px;
  }

  .stars-radar svg {
    width: 168px;
    height: 168px;
    display: block;
  }

  .stars-radar .grid {
    fill: none;
    stroke: var(--edge);
    stroke-width: 1.5;
  }

  .stars-radar .grid.inner {
    stroke-dasharray: 3 4;
    opacity: 0.7;
  }

  .stars-radar .axis {
    stroke: var(--edge);
    stroke-width: 1;
    opacity: 0.7;
  }

  .stars-radar .shape {
    fill: rgba(61, 184, 176, 0.38);
    stroke: #c8a45c;
    stroke-width: 3;
    stroke-linejoin: round;
  }

  .stars-radar .dot {
    fill: #e59e3d;
  }

  .stars-controls {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-top: 6px;
  }

  .spoke-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .spoke-label {
    flex: none;
    width: 92px;
    font-size: 0.72rem;
    color: var(--ink);
    text-align: right;
  }

  .spoke-btns {
    display: inline-flex;
    gap: 3px;
  }

  .spoke-val {
    font: inherit;
    font-size: 0.72rem;
    width: 26px;
    height: 24px;
    padding: 0;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    cursor: pointer;
  }

  .spoke-val:hover {
    border-color: var(--focus, #6e4db0);
  }

  .spoke-val.active {
    background: var(--accent);
    border-color: var(--focus, #6e4db0);
    color: var(--bg);
    font-weight: 700;
  }

  /* Past days are read-only: dead buttons, but a saved value stays lit. */
  .spoke-val:disabled {
    cursor: default;
  }

  .spoke-val:disabled:not(.active) {
    opacity: 0.5;
  }

  .spoke-val:disabled:not(.active):hover {
    border-color: var(--edge);
  }

  .stars-foot {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 10px;
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
    border-color: var(--focus, #6e4db0);
  }

  .mini.active {
    background: var(--accent);
    border-color: var(--focus, #6e4db0);
    color: var(--bg);
  }

  .mini:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .mini:disabled:hover {
    border-color: var(--edge);
  }

  .stars-save {
    font-weight: 700;
  }

  .stars-note {
    font-size: 0.64rem;
    color: var(--dim);
  }

  /* Review: the Cloud word field under the mean pentagon. A solid pane,
     so the field inside opts into .orbit-field; this box only gives it
     height inside the scrolling body. */
  .review-sky {
    display: flex;
    flex-direction: column;
    height: 300px;
    margin-top: 10px;
  }

  /* GO-ORBITTY-BOARD-STARS: hosted in the shared 780×520 pane — pad, don't
     squash. The radar stays a square SVG (honest pentagon, just bigger),
     Check-in puts the 0–4 rows beside it, Review puts the Cloud beside the
     mean panel, and the column letterboxes inside the extra field. */
  @media (min-width: 640px) {
    .s-body {
      padding: 16px 24px 20px;
    }

    .stars-panel {
      max-width: 660px;
      margin: 0 auto;
      padding: 12px 18px 14px;
    }

    .checkin-grid {
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 22px;
    }

    .checkin-grid .stars-radar svg {
      width: 224px;
      height: 224px;
    }

    .s-body.review {
      display: flex;
      align-items: stretch;
      justify-content: center;
      gap: 14px;
    }

    .s-body.review .stars-panel {
      flex: 0 1 360px;
      margin: 0;
    }

    .s-body.review .stars-radar svg {
      width: 200px;
      height: 200px;
    }

    .s-body.review .review-sky {
      flex: 1 1 0;
      height: auto;
      min-height: 0;
      margin-top: 0;
    }
  }
</style>
