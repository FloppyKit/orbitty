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
  // Left: one check-in card (radar, wins, save) and a trackers card.
  // Right: one folio — cloud cover, journal over it. Center of the radar is unset.
  // Average pills sit under the day chips. No Check-in | Review.
  // Same state.json discipline as Lists: every
  // write is load → mutate → save of the WHOLE loaded object, so rail-owned
  // slices round-trip untouched. This window writes the stars slice and
  // planet trackers. The companion's Prompts strip reads the same stars
  // slice via native commands; its toggles land here on state:changed.
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import RadarDial from "./stars/RadarDial.svelte";
  import AlmanacFolio from "./stars/AlmanacFolio.svelte";
  import DayPositives from "./stars/DayPositives.svelte";
  import StarsTrackers from "./stars/StarsTrackers.svelte";
  import ReflectSheet from "./stars/ReflectSheet.svelte";
  import NudgeStrip from "./stars/NudgeStrip.svelte";
  import { ledgerBody } from "./stars/ledgerBody";
  import { reorderProjects } from "./planetOrder";
  import { stepSpoke } from "./stars/radarGeom";
  import { listenPaneVis } from "./paneVis";
  import { startPaneDrag } from "./dragPane";
  import WindowTitleRow from "./WindowTitleRow.svelte";
  import { chromeMode, navPlatform, navUa } from "./windowChrome";
  import { MenuRing, type RingDir, type RingStop } from "./menuKeys.svelte";
  import {
    applyHostTokens,
    applyWindowOpacity,
    clampWindowOpacity,
  } from "../themes/apply";
  import "./chrome.css";
  import "./orbit-field.css";
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
  import { elapsedSec, formatElapsed, isTimer } from "./stars/timer";

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
    ringActive = false,
    onRingExit,
    outwardDir = "left" as RingDir,
  }: {
    hideLabel?: string;
    embedded?: boolean;
    onClose?: () => void;
    ringActive?: boolean;
    onRingExit?: () => void;
    outwardDir?: RingDir;
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
  // Check-in fraction of the left column, and left-column fraction of the body.
  const ACT_SPLIT = 0.62;
  const EAST_SPLIT = 0.52;
  let starsActSplit = $state(ACT_SPLIT);
  let starsEastSplit = $state(EAST_SPLIT);
  let splitDragging = false;
  let planetDragging = false;
  let orderDirty = false;
  let bodyEl: HTMLDivElement | null = $state(null);
  let actEl: HTMLDivElement | null = $state(null);
  let nudgeScope = $state("overall");

  function clampSplit(
    v: number | undefined,
    fallback: number,
    lo: number,
    hi: number,
  ): number {
    if (typeof v !== "number" || !Number.isFinite(v)) return fallback;
    return Math.min(hi, Math.max(lo, v));
  }
  let reflectOpen = $state(false);
  let starsLoaded = $state(false);
  // The draft is the working copy — it only becomes state via Save today.
  let starsDraft = $state<StarsCheckin>(emptyCheckin());
  let starsSavedDay = $state("");
  // Mid-edit guard: a state:changed echo (companion Prompts toggle / strip
  // ack) must not wipe spoke clicks that were never saved.
  let draftDirty = $state(false);

  function todayKey(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }

  // Day cursor: which saved day the radar + spoke rows are showing.
  let cursorDay = $state(todayKey());
  const isToday = $derived(cursorDay === todayKey());
  const cursorSaved = $derived(starsState.checkins[cursorDay]);
  // Today shows the live draft; a past day shows its save, or null for an
  // empty day (cage only, no fill — never a warning).
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
      const vals = saves.map((c) => c[key] ?? 0).filter((n) => n >= 1);
      mean[key] = vals.length
        ? vals.reduce((acc, n) => acc + n, 0) / vals.length
        : 0;
    }
    return mean;
  });
  // Gold edge tracks a saved shape. A dirty today stays plain until Save.
  const plated = $derived.by(() => {
    const src = avgMode ? windowMean : shown;
    if (!src || !STARS_SPOKES.some(({ key }) => (src[key] ?? 0) >= 1)) return false;
    if (avgMode || !isToday) return true;
    return !draftDirty && cursorSaved != null;
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
      if (!planetDragging) projects = s.projects ?? [];
      if (!splitDragging) {
        starsActSplit = clampSplit(
          starsState.starsActSplit,
          ACT_SPLIT,
          0.34,
          0.82,
        );
        starsEastSplit = clampSplit(
          starsState.starsEastSplit,
          EAST_SPLIT,
          0.3,
          0.74,
        );
      }
      if (
        nudgeScope !== "overall" &&
        !projects.some((p) => p.id === nudgeScope)
      ) {
        nudgeScope = "overall";
      }
      if (!draftDirty) syncStarsDraft();
      starsLoaded = true;
    } catch {
      // next state:changed / visibility refresh retries
    }
  }

  function setSpoke(key: SpokeKey, v: number) {
    if (!dayEditable) return; // past days and a locked today stay read-only
    const n = v <= 0 ? 0 : Math.max(1, Math.min(5, Math.round(v)));
    starsDraft = { ...starsDraft, [key]: n };
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
      const n = Math.round(starsDraft[key] ?? 0);
      clean[key] = n >= 1 ? Math.min(5, n) : 0;
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
    const cur = starsDraft[key] ?? 0;
    if (!dayEditable) {
      editingToday = true;
      if (cur >= 1) return; // a saved spoke unlocks first; the next arrow steps
    }
    setSpoke(key, stepSpoke(cur, delta));
  }

  function saveFromRing() {
    // The Save today button's enabled gate, mirrored.
    if (!isToday || avgMode || (!dayEditable && !!cursorSaved)) return;
    void saveStarsToday();
  }

  $effect(() => {
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

  function scopeLabel(): string {
    if (nudgeScope === "overall") return "All";
    return projects.find((p) => p.id === nudgeScope)?.name ?? "All";
  }

  function trackerLines(): { planet: string; label: string; value: string }[] {
    const now = Date.now();
    const list =
      nudgeScope === "overall"
        ? projects
        : projects.filter((p) => p.id === nudgeScope);
    const out: { planet: string; label: string; value: string }[] = [];
    for (const p of list) {
      for (const tr of p.trackers ?? []) {
        const value = isTimer(tr)
          ? formatElapsed(elapsedSec(tr, now))
          : String(tr.value ?? 0);
        out.push({ planet: p.name, label: tr.label, value });
      }
    }
    return out;
  }

  async function writeLedgerPage(flags: MetricFlags) {
    const body = ledgerBody({
      dayKey: todayKey(),
      scopeLabel: scopeLabel(),
      flags,
      spokes: STARS_SPOKES.map((s) => ({
        label: s.label,
        value: starsDraft[s.key] ?? 0,
      })),
      wins: [starsDraft.win1, starsDraft.win2]
        .map((w) => (w ?? "").trim())
        .filter(Boolean),
      grateful: (starsDraft.grateful ?? "").trim(),
      trackers: trackerLines(),
    });
    if (!body.trim()) return;
    const page: LedgerPage = {
      id: newId("led"),
      createdAt: Date.now(),
      dayKey: todayKey(),
      scope: nudgeScope,
      body,
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

  function onActSplitDown(e: PointerEvent) {
    const el = actEl;
    if (!el) return;
    e.preventDefault();
    splitDragging = true;
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    const startY = e.clientY;
    const start = starsActSplit;
    const h = Math.max(el.clientHeight, 1);
    const move = (ev: PointerEvent) => {
      starsActSplit = clampSplit(
        start + (ev.clientY - startY) / h,
        ACT_SPLIT,
        0.34,
        0.82,
      );
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      splitDragging = false;
      void persistLayout();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  function resetActSplit() {
    starsActSplit = ACT_SPLIT;
    void persistLayout();
  }

  function onEastSplitDown(e: PointerEvent) {
    const el = bodyEl;
    if (!el) return;
    e.preventDefault();
    splitDragging = true;
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    const startX = e.clientX;
    const start = starsEastSplit;
    const w = Math.max(el.clientWidth, 1);
    const move = (ev: PointerEvent) => {
      starsEastSplit = clampSplit(
        start + (ev.clientX - startX) / w,
        EAST_SPLIT,
        0.3,
        0.74,
      );
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      splitDragging = false;
      void persistLayout();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  function resetEastSplit() {
    starsEastSplit = EAST_SPLIT;
    void persistLayout();
  }

  let layoutQueue: Promise<void> = Promise.resolve();

  function persistLayout() {
    const act = starsActSplit;
    const east = starsEastSplit;
    layoutQueue = layoutQueue
      .then(async () => {
        const s = await invoke<PersistedState>("load_state");
        const stars = s.stars ?? { promptsOptIn: false, checkins: {} };
        stars.starsActSplit = act;
        stars.starsEastSplit = east;
        s.stars = stars;
        await invoke("save_state", { state: s });
        if (starsActSplit === act && starsEastSplit === east) {
          starsState = { ...starsState, starsActSplit: act, starsEastSplit: east };
        }
      })
      .catch(() => {
        // next drag retries
      });
  }

  function reorderPlanetsLive(fromId: string, beforeId: string | null) {
    planetDragging = true;
    const next = reorderProjects(projects, fromId, beforeId);
    if (next === projects) return;
    projects = next;
    orderDirty = true;
  }

  function commitPlanetOrder() {
    const dirty = orderDirty;
    const order = projects.map((p) => p.id);
    orderDirty = false;
    if (!dirty) {
      planetDragging = false;
      return;
    }
    void patchProjects((list) => {
      const byId = new Map(list.map((p) => [p.id, p]));
      const next = order
        .map((id) => byId.get(id))
        .filter((p): p is Project => !!p);
      const seen = new Set(next.map((p) => p.id));
      for (const p of list) {
        if (!seen.has(p.id)) next.push(p);
      }
      return next;
    }).finally(() => {
      planetDragging = false;
    });
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
    // Embedded: Esc is not eaten, so CompanionPage's bubble handler
    // dismisses the well. Overlay: an open Reflect sheet closes first
    // and the window stays up. The next Esc hides this window (same
    // hide_window the × button uses).
    if (!embedded) {
      detachMenuKeys = menuRing.attach(() => {
        if (reflectOpen) {
          reflectOpen = false;
          return true;
        }
        invoke("hide_window", { label: hideLabel }).catch(() => {});
        return true;
      });
    }
    if (embedded) {
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
  $effect(() => {
    if (!embedded || !ringActive) return;
    const outward = outwardDir;
    menuRing.onEdge = (dir) => {
      if (dir !== outward) return false;
      onRingExit?.();
      return true;
    };
    detachMenuKeys?.();
    detachMenuKeys = menuRing.attach(() => {
      if (reflectOpen) {
        reflectOpen = false;
        return true;
      }
      onRingExit?.();
      return true;
    });
    menuRing.select("stars-nav");
    return () => {
      detachMenuKeys?.();
      detachMenuKeys = undefined;
      menuRing.onEdge = null;
    };
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

{#snippet dayNav()}
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
    <button
      type="button"
      class="chron-prompts"
      class:on={starsState.promptsOptIn}
      aria-pressed={starsState.promptsOptIn}
      onmousedown={(e) => e.stopPropagation()}
      onclick={toggleStarsPrompts}
      title="Companion prompts, for every planet. Off by default."
    >{starsState.promptsOptIn ? "Prompts on" : "Prompts"}</button>
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
{/snippet}

<main class="stars orbit-field" class:embedded>
  {#if embedded}
  <!-- Embedded STARS well keeps today's banner and ×. -->
  <header class="s-banner" onmousedown={onBannerMouseDown}>
    <h1>STARS</h1>
    {@render dayNav()}
    <button
      class="winx"
      onmousedown={(e) => e.stopPropagation()}
      onclick={() => onClose?.()}
      title="Dismiss the STARS well — the companion stays"
      aria-label="Dismiss STARS">×</button
    >
  </header>
  {:else}
  <WindowTitleRow
    title="Stars"
    mode={chromeMode(navUa(), navPlatform())}
    label="stars"
    closeLabel="Hide STARS"
    onClose={() => invoke("hide_window", { label: hideLabel }).catch(() => {})}
    chips={dayNav}
  />
  {/if}

  <NudgeStrip
    {projects}
    scope={nudgeScope}
    onScope={(id) => (nudgeScope = id)}
    onReorder={reorderPlanetsLive}
    onReorderCommit={commitPlanetOrder}
  />

  <div class="s-body" bind:this={bodyEl}>
    <div class="act-col" bind:this={actEl} style="flex: {starsEastSplit} 1 0%">
      <section
        class="act-tile checkin"
        style="flex: {starsActSplit} 1 0%"
        aria-label="Check-in"
      >
        <h2>Check-in</h2>
        <RadarDial
          spokes={STARS_SPOKES}
          shown={avgMode ? windowMean : shown}
          editable={dayEditable}
          {avgMode}
          {plated}
          activeId={menuRing.activeId}
          onSet={(key, v) => setSpoke(key as SpokeKey, v)}
        />
        <DayPositives
          win1={shown?.win1 ?? ""}
          win2={shown?.win2 ?? ""}
          grateful={shown?.grateful ?? ""}
          editable={dayEditable}
          onChange={setWin}
        />
        <div class="stars-foot">
          <button
            class="mini stars-save"
            class:dirty={draftDirty}
            disabled={!isToday || avgMode || (!dayEditable && !!cursorSaved)}
            onclick={saveStarsToday}
          >
            Save today
          </button>
          <button
            type="button"
            class="mini quiet"
            disabled={!isToday || avgMode || !cursorSaved || dayEditable}
            onclick={() => (editingToday = true)}
          >Edit</button>
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
      <div
        class="act-split"
        role="separator"
        aria-orientation="horizontal"
        aria-label="Resize Check-in and Trackers"
        title="Drag to resize · double-click resets"
        onpointerdown={onActSplitDown}
        ondblclick={resetActSplit}
      ></div>
      <section
        class="act-tile trackers"
        style="flex: {1 - starsActSplit} 1 0%"
        aria-label="Trackers"
      >
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
    <div
      class="east-split"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize check-in column and folio"
      title="Drag to resize · double-click resets"
      onpointerdown={onEastSplitDown}
      ondblclick={resetEastSplit}
    ></div>
    <div class="folio-col" style="flex: {1 - starsEastSplit} 1 0%">
      <AlmanacFolio
        pages={starsState.ledgerPages ?? []}
        {projects}
        loaded={starsLoaded}
        dayKey={todayKey()}
        onReflect={() => (reflectOpen = true)}
      />
    </div>
  </div>
  {#if reflectOpen}
    <ReflectSheet onClose={() => (reflectOpen = false)} onWrite={writeLedgerPage} />
  {/if}
</main>

<style>
  .stars {
    --depth-0: #0b1020;
    --depth-1-fill: rgba(232, 236, 248, 0.1);
    --depth-1-line: rgba(232, 236, 248, 0.22);
    --depth-1-shadow: 0 1px 0 rgba(232, 236, 248, 0.06), 0 8px 20px rgba(0, 0, 0, 0.28);
    --depth-2-shadow: 0 12px 28px rgba(0, 0, 0, 0.45);
    --depth-2-shadow-hover: 0 18px 36px rgba(0, 0, 0, 0.55);
    --gold: #c8a45c;
    --paper: #e7e3f4;
    --ink: #e8ecf8;
    --dim: #b7c0d8;
    --edge: rgba(232, 236, 248, 0.2);
    --bg: #0b1020;
    --panel: #14162c;
    --bg-fill: transparent;
    --panel-fill: rgba(232, 236, 248, 0.1);
    position: relative;
    height: 100vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
    color: var(--ink);
    background-color: var(--depth-0);
    border-radius: 0;
    color-scheme: dark;
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

  .s-banner .day-nav,
  :global(.wtr-chips) .day-nav {
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
    gap: 0;
    padding: 10px 12px 12px;
  }

  .stars.embedded .s-body {
    flex: 1;
  }

  .act-col {
    min-width: 0;
    min-height: 0;
    height: 100%;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .act-split,
  .east-split {
    flex: 0 0 6px;
    position: relative;
    background: transparent;
    touch-action: none;
  }

  .act-split {
    cursor: ns-resize;
  }

  .east-split {
    cursor: col-resize;
    align-self: stretch;
  }

  .act-split::after,
  .east-split::after {
    content: "";
    position: absolute;
    border-radius: 1px;
    background: var(--dim);
    opacity: 0;
  }

  .act-split::after {
    left: 18%;
    right: 18%;
    top: 2px;
    height: 2px;
  }

  .east-split::after {
    top: 18%;
    bottom: 18%;
    left: 2px;
    width: 2px;
  }

  .act-split:hover::after,
  .act-split:active::after,
  .east-split:hover::after,
  .east-split:active::after {
    opacity: 0.45;
  }

  .folio-col {
    min-width: 0;
    min-height: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .folio-col :global(.folio) {
    flex: 1 1 auto;
    height: 100%;
    min-height: 0;
  }

  .act-tile {
    min-height: 0;
    overflow: auto;
    box-sizing: border-box;
    background: var(--depth-1-fill);
    border: 1px solid var(--depth-1-line);
    border-radius: 10px;
    box-shadow: var(--depth-1-shadow);
    padding: 8px 10px 10px;
  }

  .act-tile.checkin {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .act-tile.trackers {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .act-tile h2 {
    margin: 0;
    flex: none;
    font-size: 0.66rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--dim);
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
    background: transparent;
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 3px 8px;
    min-height: 24px;
    cursor: pointer;
  }

  .mini.quiet {
    border-color: transparent;
    color: var(--dim);
    font-weight: 500;
  }

  .mini:hover {
    border-color: var(--focus, #6e4db0);
  }

  .mini:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .mini:disabled:hover {
    border-color: var(--edge);
  }

  .stars-save:not(:disabled) {
    font-weight: 700;
    background: var(--accent);
    border-color: transparent;
    color: var(--bg);
    box-shadow: var(--depth-2-shadow);
  }

  .stars-save.dirty:not(:disabled) {
    box-shadow: 0 0 0 1px var(--gold), var(--depth-2-shadow);
  }

  .chron-prompts {
    flex: none;
    font: inherit;
    font-size: 0.68rem;
    font-weight: 650;
    color: var(--dim);
    background: transparent;
    border: 1px solid var(--edge);
    border-radius: 999px;
    padding: 1px 8px;
    cursor: pointer;
  }

  .chron-prompts.on {
    color: var(--ink);
    font-weight: 700;
    border-color: color-mix(in srgb, var(--ink) 45%, var(--edge));
    background: var(--panel-fill, var(--panel));
  }

  .stars-note {
    font-size: 0.64rem;
    color: var(--dim);
  }
</style>
