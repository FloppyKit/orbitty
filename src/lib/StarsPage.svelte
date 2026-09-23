<script lang="ts">
  // The check-in radar lives in its own window (like Lists),
  // never stuffed into the rail. Five spokes 1–5 keyed by local YYYY-MM-DD;
  // explicit check-in only. GO-ORBITTY-STARS-FIVE: the scale is 1–5 (was
  // 0–4) — legacy saves migrate n→n+1 once native-side at setup (scaleV
  // flag); an unsaved day is still empty (draft 0 = unset, lights nothing).
  // An in-session day
  // cursor pages the saved map — past days are read-only, Save only ever
  // writes today, and the cursor itself is never persisted.
  // Rolling 7/14/30-day window ending at the
  // day cursor, mean of saved days only (a saved 1 counts; blanks shrink
  // the denominator, never a zero-fill). Persists as stars.averageWindow.
  // One screen — left act (radar, wins, trackers), right read (cloud, ledger).
  // Average pills sit under the day chips. No Check-in | Review.
  // Same state.json discipline as Lists: every
  // write is load → mutate → save of the WHOLE loaded object, so rail-owned
  // slices round-trip untouched. This window writes the stars slice and
  // planet trackers. The companion's Prompts strip reads the same stars
  // slice via native commands; its toggles land here on state:changed.
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import SkyField from "./SkyField.svelte";
  import RadarDial from "./stars/RadarDial.svelte";
  import DayPositives from "./stars/DayPositives.svelte";
  import StarsTrackers from "./stars/StarsTrackers.svelte";
  import LedgerStack from "./stars/LedgerStack.svelte";
  import ReflectSheet from "./stars/ReflectSheet.svelte";
  import NudgeStrip from "./stars/NudgeStrip.svelte";
  import NudgeCards from "./stars/NudgeCards.svelte";
  import { listenPaneVis } from "./paneVis";
  import { startPaneDrag } from "./dragPane";
  import { MenuRing, type RingStop } from "./menuKeys.svelte";
  import {
    applyHostTokens,
    applyWindowOpacity,
    clampWindowOpacity,
  } from "../themes/apply";
  import "./chrome.css";
  import type {
    AverageWindow,
    LedgerPage,
    MetricFlags,
    PersistedState,
    Project,
    StarsCheckin,
    StarsSlice,
    Tracker,
  } from "./persist";
  import { elapsedSec } from "./stars/timer";

  // This page now mounts inside the one shared
  // 780×520 `kanban` overlay (stars mode) — hideLabel is the HOST window's
  // label, so the × and the re-show reset talk to the pane we're in. The
  // legacy 420×560 `stars` window passes nothing and keeps "stars".
  // Embedded mounts this page in the companion's
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



  // The seven palette skins are retired; a
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
  // Draft sentinel 0 = unset (below the 1–5 floor): an unsaved today
  // lights no button and draws an empty radar. Saved check-ins are 1–5.
  const emptyCheckin = (): StarsCheckin => ({
    sleep: 0,
    terroir: 0,
    athleticism: 0,
    relationships: 0,
    self: 0,
  });

  let starsState = $state<StarsSlice>({ promptsOptIn: false, checkins: {} });
  let projects = $state<Project[]>([]);
  let nudgeScope = $state("overall");
  let reflectOpen = $state(false);
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

  // Day cursor: which saved day the radar + spoke rows are showing.
  let cursorDay = $state(todayKey());
  const isToday = $derived(cursorDay === todayKey());
  const cursorSaved = $derived(starsState.checkins[cursorDay]);
  // Today shows the live draft; a past day shows its save, or null for an
  // empty day (blank pentagon / no active 1–5 — never a warning).
  const shown = $derived<StarsCheckin | null>(
    isToday ? starsDraft : (cursorSaved ?? null),
  );
  // Banner nav: Day (‹ › dates) or an average window. Average windows
  // persist as stars.averageWindow; Day is session-only.
  type StarsNav = "day" | "week" | "month" | "year" | "custom";
  let navView = $state<StarsNav>("day");
  let editingToday = $state(false);
  const WINDOWS: Record<Exclude<StarsNav, "day" | "custom">, number> = {
    week: 7,
    month: 30,
    year: 365,
  };
  const avgFrom = $derived(starsState.averageFrom || "");
  const avgTo = $derived(starsState.averageTo || "");
  const windowSaves = $derived.by(() => {
    let start: string;
    let end: string;
    if (navView === "custom") {
      start = avgFrom || shiftDay(cursorDay, -29);
      end = avgTo || cursorDay;
      if (start > end) [start, end] = [end, start];
    } else {
      const key: Exclude<StarsNav, "day" | "custom"> =
        navView === "year" || navView === "month" ? navView : "week";
      start = shiftDay(cursorDay, -(WINDOWS[key] - 1));
      end = cursorDay;
    }
    return Object.keys(starsState.checkins)
      .filter((d) => d >= start && d <= end)
      .map((d) => starsState.checkins[d]);
  });
  const avgMode = $derived(navView !== "day");
  const dayEditable = $derived(
    isToday && !avgMode && (!cursorSaved || editingToday || draftDirty),
  );
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
      projects = s.projects ?? [];
      if (
        nudgeScope !== "overall" &&
        !projects.some((p) => p.id === nudgeScope)
      ) {
        nudgeScope = "overall";
      }
      if (!draftDirty) syncStarsDraft();
    } catch {
      // next state:changed / visibility refresh retries
    }
  }

  function setSpoke(key: SpokeKey, v: number) {
    if (!dayEditable) return; // past days and a locked today stay read-only
    starsDraft = { ...starsDraft, [key]: v };
    starsSavedDay = "";
    draftDirty = true;
  }

  function setWin(field: "win1" | "win2" | "grateful", raw: string) {
    if (!dayEditable) return;
    starsDraft = { ...starsDraft, [field]: raw };
    starsSavedDay = "";
    draftDirty = true;
  }

  function filled(raw: string | undefined): string | undefined {
    const t = (raw ?? "").trim();
    return t ? t : undefined;
  }

  // Save today: load fresh, replace only today's check-in inside the stars
  // slice, save the whole object back — every other slice survives byte-for-
  // byte (no defaults are ever invented here).
  async function saveStarsToday() {
    if (!isToday) return; // Save only ever writes today
    const day = todayKey();
    const clean = emptyCheckin();
    for (const { key } of STARS_SPOKES) {
      clean[key] = Math.max(1, Math.min(5, Math.round(starsDraft[key] ?? 0)));
    }
    const win1 = filled(starsDraft.win1);
    const win2 = filled(starsDraft.win2);
    const grateful = filled(starsDraft.grateful);
    if (win1) clean.win1 = win1;
    if (win2) clean.win2 = win2;
    if (grateful) clean.grateful = grateful;
    try {
      const s = await invoke<PersistedState>("load_state");
      const stars = s.stars ?? { promptsOptIn: false, checkins: {} };
      stars.checkins = { ...stars.checkins, [day]: clean };
      s.stars = stars;
      await invoke("save_state", { state: s });
      starsState = stars;
      starsSavedDay = day;
      draftDirty = false;
      editingToday = false;
    } catch {
      // next Save retries; the draft stays put
    }
  }

  // Window switcher: same load → mutate → save-whole discipline as Save
  // today, but it only ever sets stars.averageWindow — checkins untouched.
  async function setAverageWindow(w: AverageWindow, from?: string, to?: string) {
    starsState = {
      ...starsState,
      averageWindow: w,
      averageFrom: from ?? starsState.averageFrom,
      averageTo: to ?? starsState.averageTo,
    };
    try {
      const s = await invoke<PersistedState>("load_state");
      const stars = s.stars ?? { promptsOptIn: false, checkins: {} };
      stars.averageWindow = w;
      if (from !== undefined) stars.averageFrom = from;
      if (to !== undefined) stars.averageTo = to;
      s.stars = stars;
      await invoke("save_state", { state: s });
      starsState = stars;
    } catch {
      // state:changed / next load restores the truth
    }
  }

  function onNavChange(e: Event) {
    const v = (e.target as HTMLSelectElement).value as StarsNav;
    navView = v;
    if (v === "day") return;
    if (v === "custom") {
      const from = avgFrom || shiftDay(cursorDay, -29);
      const to = avgTo || cursorDay;
      void setAverageWindow("custom", from, to);
      return;
    }
    void setAverageWindow(v);
  }

  function onCustomDate(which: "from" | "to", raw: string) {
    const from = which === "from" ? raw : avgFrom || shiftDay(cursorDay, -29);
    const to = which === "to" ? raw : avgTo || cursorDay;
    void setAverageWindow("custom", from, to);
  }

  // The STARS ring — view switcher + five spokes,
  // per the pad lock: left/right picks a stop, up/down adjusts what's
  // highlighted (spoke value 1–5, or the view), Enter saves today under
  // the Save button's own gate, Esc backs out of the well (CompanionPage
  // owns it — nothing inner to close here). Past days and averages stay
  // read-only; a first bump on a saved today unlocks editing, same as
  // clicking the number. STARS-FIVE stays a scale change, not this.
  const menuRing = new MenuRing();
  const NAV_ORDER: StarsNav[] = ["day", "week", "month", "year", "custom"];

  function cycleView(delta: number) {
    const i = NAV_ORDER.indexOf(navView);
    const next =
      NAV_ORDER[(i + delta + NAV_ORDER.length) % NAV_ORDER.length];
    navView = next;
    if (next === "day") return;
    if (next === "custom") {
      void setAverageWindow(
        "custom",
        avgFrom || shiftDay(cursorDay, -29),
        avgTo || cursorDay,
      );
      return;
    }
    void setAverageWindow(next);
  }

  function bumpSpoke(key: SpokeKey, delta: number) {
    if (!isToday || avgMode) return; // past days / averages are read-only
    if (!dayEditable) {
      editingToday = true;
      return;
    }
    setSpoke(key, Math.max(1, Math.min(5, (starsDraft[key] ?? 0) + delta)));
  }

  function saveFromRing() {
    // The Save today button's enabled gate, mirrored.
    if (!isToday || avgMode || (!dayEditable && !!cursorSaved)) return;
    void saveStarsToday();
  }

  $effect(() => {
    if (!embedded) return;
    const stops: RingStop[] = [
      {
        id: "stars-nav",
        confirm: () => cycleView(1),
        arrow: (dir) => {
          if (dir === "up") {
            cycleView(-1);
            return true;
          }
          if (dir === "down") {
            cycleView(1);
            return true;
          }
          return false; // left/right walk onto the spokes
        },
      },
    ];
    for (const s of STARS_SPOKES) {
      stops.push({
        id: `spoke-${s.key}`,
        confirm: saveFromRing,
        arrow: (dir) => {
          if (dir === "up") {
            bumpSpoke(s.key, 1);
            return true;
          }
          if (dir === "down") {
            bumpSpoke(s.key, -1);
            return true;
          }
          return false;
        },
      });
    }
    menuRing.setStops(stops);
  });

  let detachMenuKeys: (() => void) | undefined;

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
    editingToday = false;
    reflectOpen = false;
    cursorDay = todayKey();
    loadState();
  };

  function newId(prefix: string): string {
    return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
  }

  async function writeLedgerPage(flags: MetricFlags) {
    const page: LedgerPage = {
      id: newId("led"),
      createdAt: Date.now(),
      dayKey: todayKey(),
      scope: nudgeScope,
      metricFlags: flags,
    };
    try {
      const s = await invoke<PersistedState>("load_state");
      const stars = s.stars ?? { promptsOptIn: false, checkins: {} };
      stars.ledgerPages = [...(stars.ledgerPages ?? []), page];
      s.stars = stars;
      await invoke("save_state", { state: s });
      starsState = stars;
      reflectOpen = false;
    } catch {
      // next write retries; flags stay on the sheet
    }
  }

  async function patchProjects(fn: (projects: Project[]) => Project[]) {
    try {
      const s = await invoke<PersistedState>("load_state");
      s.projects = fn(s.projects ?? []);
      await invoke("save_state", { state: s });
      projects = s.projects;
    } catch {
      // next edit retries
    }
  }

  function mapTracker(
    list: Project[],
    projectId: string,
    trackerId: string,
    fn: (t: Tracker) => Tracker,
  ): Project[] {
    return list.map((p) => {
      if (p.id !== projectId) return p;
      return {
        ...p,
        trackers: (p.trackers ?? []).map((t) =>
          t.id === trackerId ? fn(t) : t,
        ),
      };
    });
  }

  function addTracker(projectId: string, raw: string, kind: "number" | "timer") {
    const label = raw.trim().slice(0, 7);
    if (!label || !projectId) return;
    const tr: Tracker = {
      id: newId("tr"),
      label,
      value: 0,
      kind,
    };
    void patchProjects((list) =>
      list.map((p) =>
        p.id === projectId
          ? { ...p, trackers: [...(p.trackers ?? []), tr] }
          : p,
      ),
    );
  }

  function bumpTracker(projectId: string, trackerId: string, delta: number) {
    void patchProjects((list) =>
      mapTracker(list, projectId, trackerId, (t) => ({
        ...t,
        value: t.value + delta,
      })),
    );
  }

  function setTrackerValue(projectId: string, trackerId: string, raw: string) {
    const n = parseInt(raw.trim(), 10);
    if (!Number.isFinite(n)) return;
    void patchProjects((list) =>
      mapTracker(list, projectId, trackerId, (t) =>
        t.value === n ? t : { ...t, value: n },
      ),
    );
  }

  function removeTracker(projectId: string, trackerId: string) {
    void patchProjects((list) =>
      list.map((p) => {
        if (p.id !== projectId) return p;
        const next = (p.trackers ?? []).filter((t) => t.id !== trackerId);
        return { ...p, trackers: next.length ? next : undefined };
      }),
    );
  }

  function playTimer(projectId: string, trackerId: string) {
    const stamp = Date.now();
    void patchProjects((list) =>
      mapTracker(list, projectId, trackerId, (t) => ({
        ...t,
        kind: "timer",
        runningSince: t.runningSince ?? stamp,
      })),
    );
  }

  function stopTimer(projectId: string, trackerId: string) {
    const stamp = Date.now();
    void patchProjects((list) =>
      mapTracker(list, projectId, trackerId, (t) => {
        const next: Tracker = {
          ...t,
          kind: "timer",
          value: Math.round(elapsedSec(t, stamp)),
        };
        delete next.runningSince;
        return next;
      }),
    );
  }
  const onVisibility = () => {
    if (!document.hidden) onReshow();
  };

  let unlisten: (() => void) | undefined;
  let unlistenVis: (() => void) | undefined;
  onMount(async () => {
    if (!embedded) invoke("ensure_glass").catch(() => {});
    unlisten = await listen("state:changed", loadState);
    if (embedded) {
      detachMenuKeys = menuRing.attach();
      void loadState().then(() => syncStarsDraft());
      document.addEventListener("visibilitychange", onVisibility);
      return;
    }
    await loadState();
    syncStarsDraft();
    unlistenVis = await listenPaneVis(hideLabel, (visible) => {
      if (visible) onReshow();
    });
    document.addEventListener("visibilitychange", onVisibility);
  });
  onDestroy(() => {
    unlisten?.();
    unlistenVis?.();
    detachMenuKeys?.();
    document.removeEventListener("visibilitychange", onVisibility);
  });

  function onBannerMouseDown(e: MouseEvent) {
    startPaneDrag(e);
  }
</script>

<main class="stars" class:embedded>
  <!-- One drag source: JS startDragging on this banner. Drawers are
       borderless — no native Overlay strip competing with the click.
       HOST-SPLIT: embedded, this drags the host companion window. -->
  <header class="s-banner" onmousedown={onBannerMouseDown}>
    <h1>STARS</h1>
    <div class="day-nav">
      <button class="mini" onclick={prevDay} title="Previous day" aria-label="Previous day">‹</button>
      <span class="day-label">{isToday ? `Today · ${cursorDay}` : cursorDay}</span>
      <button
        class="mini"
        onclick={nextDay}
        disabled={isToday}
        title="Next day"
        aria-label="Next day">›</button>
      <select
        class="nav-view"
        class:mkey-sel={menuRing.activeId === "stars-nav"}
        data-mkey="stars-nav"
        value={navView}
        onchange={onNavChange}
        aria-label="STARS view"
      >
        <option value="day">Day</option>
        <option value="week">7 days</option>
        <option value="month">30 days</option>
        <option value="year">1 year</option>
        <option value="custom">Custom…</option>
      </select>
      {#if navView === "custom"}
        <input
          class="nav-date"
          type="date"
          value={avgFrom || shiftDay(cursorDay, -29)}
          max={avgTo || cursorDay}
          onchange={(e) => onCustomDate("from", (e.currentTarget as HTMLInputElement).value)}
          aria-label="Average from"
        />
        <input
          class="nav-date"
          type="date"
          value={avgTo || cursorDay}
          min={avgFrom || ""}
          max={todayKey()}
          onchange={(e) => onCustomDate("to", (e.currentTarget as HTMLInputElement).value)}
          aria-label="Average to"
        />
      {/if}
    </div>
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
  </header>

  <NudgeStrip
    {projects}
    scope={nudgeScope}
    onScope={(id) => (nudgeScope = id)}
    onReflect={() => (reflectOpen = true)}
  />
  <NudgeCards {projects} scope={nudgeScope} />

  <div class="s-body">
    <div class="act-col">
      <section class="act-tile" aria-label="Stars">
        <h2>Stars</h2>
        <RadarDial
          spokes={STARS_SPOKES}
          shown={avgMode ? windowMean : shown}
          editable={dayEditable}
          {avgMode}
          activeId={menuRing.activeId}
          onSet={(key, v) => setSpoke(key as SpokeKey, v)}
        />
        <div class="stars-foot">
          <button
            class="mini stars-save"
            disabled={!isToday || avgMode || (!dayEditable && !!cursorSaved)}
            onclick={saveStarsToday}
          >
            Save today
          </button>
          <button
            type="button"
            class="mini"
            disabled={!isToday || avgMode || !cursorSaved || dayEditable}
            onclick={() => (editingToday = true)}
          >Edit</button>
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
      <section class="act-tile" aria-label="Wins">
        <h2>Wins</h2>
        <DayPositives
          win1={shown?.win1 ?? ""}
          win2={shown?.win2 ?? ""}
          grateful={shown?.grateful ?? ""}
          editable={dayEditable}
          onChange={setWin}
        />
      </section>
      <section class="act-tile" aria-label="Trackers">
        <StarsTrackers
          {projects}
          scope={nudgeScope}
          onAdd={addTracker}
          onBump={bumpTracker}
          onSetValue={setTrackerValue}
          onRemove={removeTracker}
          onPlay={playTimer}
          onStop={stopTimer}
        />
      </section>
    </div>
    <div class="cloud-col" aria-label="Word cloud and ledger">
      <div class="cloud-sky">
        <SkyField />
      </div>
      <LedgerStack pages={starsState.ledgerPages ?? []} {projects} />
    </div>
  </div>
  {#if reflectOpen}
    <ReflectSheet onClose={() => (reflectOpen = false)} onWrite={writeLedgerPage} />
  {/if}
</main>

<style>
  .stars {
    position: relative;
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

  .stars.embedded {
    height: 100%;
    flex: 1;
    min-height: 0;
    border-radius: 0;
    overflow: hidden;
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

  .s-banner h1 {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  .s-banner .day-nav {
    flex: 1;
    margin: 0;
    justify-content: flex-start;
    flex-wrap: nowrap;
    min-width: 0;
  }

  .nav-view {
    font: inherit;
    font-size: 0.72rem;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 2px 6px;
    margin-left: 6px;
  }

  .nav-date {
    font: inherit;
    font-size: 0.68rem;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 1px 4px;
    max-width: 9.2rem;
  }

  .s-banner .winx {
    margin-left: 4px;
  }

  .s-body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: row;
    align-items: stretch;
    gap: 10px;
    padding: 10px 12px 12px;
  }

  .stars.embedded .s-body {
    flex: 1;
  }

  .act-col {
    flex: 1 1 0;
    min-width: 0;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .act-tile {
    flex: 0 0 auto;
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 10px;
    padding: 8px 10px 10px;
  }

  .act-tile h2 {
    margin: 0 0 4px;
    font-size: 0.72rem;
    font-weight: 700;
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

  .stars-foot {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    flex: 0 0 auto;
    flex-wrap: wrap;
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

  .cloud-col {
    flex: 1 1 0;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .cloud-sky {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
</style>
