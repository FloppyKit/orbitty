<script lang="ts">
  // Stars: planet column, five-spoke radar, trackers, journal / sky folio.
  // Check-ins are explicit. Today with nothing saved is a draft at 3 and
  // is not written until a touch or Save. Past empty days stay empty.
  // The day cursor is session-only. Writes are load → mutate → save of the
  // whole state object.
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import RadarDial from "./stars/RadarDial.svelte";
  import AlmanacFolio from "./stars/AlmanacFolio.svelte";
  import DayPositives from "./stars/DayPositives.svelte";
  import StarsTrackers from "./stars/StarsTrackers.svelte";
  import ReflectSheet from "./stars/ReflectSheet.svelte";
  import StarsPlanetColumn from "./stars/StarsPlanetColumn.svelte";
  import { ledgerBody, withNotes } from "./stars/ledgerBody";
  import { reorderProjects } from "./planetOrder";
  import {
    STARS_SPOKES,
    radarDraft,
    stepSpoke,
    type SpokeKey,
  } from "./stars/radarGeom";
  import { starsColumns, starsLayout, starsScopeTargets } from "./stars/starsLayout";
  import { columnNext, segmentNext, withinNext, type StarsCol } from "./stars/starsFocus";
  import { folioFace, reflectTarget, type StarsView } from "./stars/folioDay";
  import { buildDocs, epochSecToDayKey } from "./skyBag";
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

  const KNOWN_THEMES = new Set(["systemLight", "systemDark", "terminal"]);

  function osThemeId(): string {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "systemDark"
      : "systemLight";
  }

  let themeId = $state("");
  let windowOpacity = $state(1.0);
  let starsState = $state<StarsSlice>({ promptsOptIn: false, checkins: {} });
  let projects = $state<Project[]>([]);
  let bagState = $state<PersistedState | null>(null);
  let planetDragging = false;
  let orderDirty = false;
  let trackerScope = $state("overall");
  let mainEl: HTMLElement | null = $state(null);
  let mainW = $state(1040);
  let segment = $state<"checkin" | "trackers" | "journal">("checkin");
  let focusCol = $state<StarsCol>("planets");
  let focusIdx = $state(0);
  let spokeLive = $state(false);
  let reflectOpen = $state(false);
  let reflectHint = $state("");
  let reflectPlan = $state<{ dayKey: string; dayKeyEnd?: string } | null>(null);
  let starsLoaded = $state(false);
  let starsDraft = $state<StarsCheckin>(emptyCheckin());
  let starsSavedDay = $state("");
  let draftDirty = $state(false);

  const layout = $derived(starsLayout(mainW));
  const cols = $derived(starsColumns(layout));
  const gridCols = $derived(
    layout === "single"
      ? "minmax(0, 1fr)"
      : `${cols.planet}px ${cols.middle} clamp(${cols.rightMin}px, ${cols.right}px, ${cols.rightMax}px)`,
  );

  function emptyCheckin(): StarsCheckin {
    return { sleep: 0, terroir: 0, athleticism: 0, relationships: 0, self: 0 };
  }

  function todayKey(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }

  let cursorDay = $state(todayKey());
  const isToday = $derived(cursorDay === todayKey());
  const cursorSaved = $derived(starsState.checkins[cursorDay]);
  type StarsNav = StarsView;
  let navView = $state<StarsNav>("day");
  let editingToday = $state(false);
  const WINDOWS: Record<Exclude<StarsNav, "day" | "custom">, number> = {
    week: 7,
    month: 30,
    year: 365,
  };
  const avgFrom = $derived(starsState.averageFrom || "");
  const avgTo = $derived(starsState.averageTo || "");
  const bounds = $derived.by(() => {
    if (navView === "day") return { from: cursorDay, to: cursorDay };
    if (navView === "custom") {
      let start = avgFrom || shiftDay(cursorDay, -29);
      let end = avgTo || cursorDay;
      if (start > end) [start, end] = [end, start];
      return { from: start, to: end };
    }
    const key: Exclude<StarsNav, "day" | "custom"> =
      navView === "year" || navView === "month" ? navView : "week";
    return { from: shiftDay(cursorDay, -(WINDOWS[key] - 1)), to: cursorDay };
  });
  const windowSaves = $derived.by(() => {
    const { from, to } = bounds;
    return Object.keys(starsState.checkins)
      .filter((d) => d >= from && d <= to)
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
      mean[key] = vals.length ? vals.reduce((acc, n) => acc + n, 0) / vals.length : 0;
    }
    return mean;
  });
  const radarShown = $derived<StarsCheckin | null>(
    avgMode ? windowMean : isToday ? starsDraft : (cursorSaved ?? null),
  );
  const radarIsDraft = $derived(isToday && !avgMode && !draftDirty && !cursorSaved);
  const shown = $derived<StarsCheckin | null>(isToday ? starsDraft : (cursorSaved ?? null));

  const dayFace = $derived(
    folioFace(
      cursorDay,
      starsState.ledgerPages ?? [],
      // MICROTAGS: planet-aware journals plug in here
      () => true,
    ),
  );
  const wordDays = $derived.by(() => {
    const set = new Set<string>();
    const s = bagState;
    if (!s) return set;
    const docs = buildDocs(
      s,
      undefined,
      // MICROTAGS: planet-aware journals plug in here
      () => true,
    );
    for (const d of docs) {
      if (!d.stamped || !d.entries.length) continue;
      set.add(epochSecToDayKey(d.at));
    }
    return set;
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
    if (n <= todayKey()) cursorDay = n;
  }

  function syncStarsDraft() {
    const today = todayKey();
    const saved = starsState.checkins[today];
    const d = radarDraft(today, today, starsState.checkins);
    if (saved) starsDraft = { ...emptyCheckin(), ...saved };
    else if (d.values) starsDraft = { ...emptyCheckin(), ...d.values };
    else starsDraft = emptyCheckin();
    draftDirty = false;
  }

  async function loadState() {
    try {
      const s = await invoke<PersistedState>("load_state");
      themeId = KNOWN_THEMES.has(s.themeId) ? s.themeId : osThemeId();
      windowOpacity = clampWindowOpacity(s.windowOpacity ?? 1);
      starsState = s.stars ?? { promptsOptIn: false, checkins: {} };
      bagState = s;
      if (!planetDragging) projects = s.projects ?? [];
      if (trackerScope !== "overall" && !projects.some((p) => p.id === trackerScope)) {
        trackerScope = "overall";
      }
      if (!draftDirty) syncStarsDraft();
      starsLoaded = true;
    } catch {
      // next state:changed / visibility refresh retries
    }
  }

  function setSpoke(key: SpokeKey, v: number) {
    if (!dayEditable) return;
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

  async function saveStarsToday() {
    if (!isToday) return;
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
      bagState = s;
      starsSavedDay = day;
      draftDirty = false;
      editingToday = false;
    } catch {
      // next Save retries; the draft stays put
    }
  }

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
      bagState = s;
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

  const menuRing = new MenuRing();
  const NAV_ORDER: StarsNav[] = ["day", "week", "month", "year", "custom"];

  function cycleView(delta: number) {
    const i = NAV_ORDER.indexOf(navView);
    const next = NAV_ORDER[(i + delta + NAV_ORDER.length) % NAV_ORDER.length];
    navView = next;
    if (next === "day") return;
    if (next === "custom") {
      void setAverageWindow("custom", avgFrom || shiftDay(cursorDay, -29), avgTo || cursorDay);
      return;
    }
    void setAverageWindow(next);
  }

  function bumpSpoke(key: SpokeKey, delta: number) {
    if (!isToday || avgMode) return;
    const cur = starsDraft[key] ?? 0;
    if (!dayEditable) {
      editingToday = true;
      if (cur >= 1) return;
    }
    setSpoke(key, stepSpoke(cur, delta));
  }

  function saveFromRing() {
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
          return false;
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
    invoke("ensure_glass").catch(() => {});
  });

  $effect(() => {
    if (!themeId) return;
    applyWindowOpacity(windowOpacity);
  });

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

  function trackerLines(): { planet: string; label: string; value: string }[] {
    const now = Date.now();
    const out: { planet: string; label: string; value: string }[] = [];
    for (const p of projects) {
      for (const tr of p.trackers ?? []) {
        const value = isTimer(tr) ? formatElapsed(elapsedSec(tr, now)) : String(tr.value ?? 0);
        out.push({ planet: p.name, label: tr.label, value });
      }
    }
    return out;
  }

  function openReflect() {
    const plan = reflectTarget(navView, cursorDay, bounds.from, bounds.to, todayKey());
    if (!plan.ok) {
      reflectHint = plan.hint;
      reflectOpen = false;
      return;
    }
    reflectHint = "";
    reflectPlan = { dayKey: plan.dayKey, dayKeyEnd: plan.dayKeyEnd };
    reflectOpen = true;
  }

  async function writeLedgerPage(flags: MetricFlags, notes = "") {
    const plan = reflectPlan ?? reflectTarget(navView, cursorDay, bounds.from, bounds.to, todayKey());
    if (!("dayKey" in plan)) return;
    const day = plan.dayKey;
    const checkin = day === todayKey() ? starsDraft : (starsState.checkins[day] ?? emptyCheckin());
    const body = withNotes(
      ledgerBody({
        dayKey: plan.dayKeyEnd ? `${day}..${plan.dayKeyEnd}` : day,
        scopeLabel: "All",
        flags,
        spokes: STARS_SPOKES.map((s) => ({ label: s.label, value: checkin[s.key] ?? 0 })),
        wins: [checkin.win1, checkin.win2].map((w) => (w ?? "").trim()).filter(Boolean),
        grateful: (checkin.grateful ?? "").trim(),
        trackers: trackerLines(),
      }),
      notes,
    );
    if (!body.trim()) return;
    const page: LedgerPage = {
      id: newId("led"),
      createdAt: Date.now(),
      dayKey: day,
      scope: "overall",
      body,
      metricFlags: flags,
    };
    if (plan.dayKeyEnd) page.dayKeyEnd = plan.dayKeyEnd;
    try {
      const s = await invoke<PersistedState>("load_state");
      const stars = s.stars ?? { promptsOptIn: false, checkins: {} };
      stars.ledgerPages = [...(stars.ledgerPages ?? []), page];
      s.stars = stars;
      await invoke("save_state", { state: s });
      starsState = stars;
      bagState = s;
      reflectOpen = false;
    } catch {
      // next write retries; flags stay on the sheet
    }
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
      const next = order.map((id) => byId.get(id)).filter((p): p is Project => !!p);
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
      bagState = s;
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
        trackers: (p.trackers ?? []).map((t) => (t.id === trackerId ? fn(t) : t)),
      };
    });
  }

  function addTracker(projectId: string, raw: string, kind: "number" | "timer") {
    const label = raw.trim().slice(0, 7);
    if (!label || !projectId) return;
    const tr: Tracker = { id: newId("tr"), label, value: 0, kind };
    void patchProjects((list) =>
      list.map((p) => (p.id === projectId ? { ...p, trackers: [...(p.trackers ?? []), tr] } : p)),
    );
  }

  function bumpTracker(projectId: string, trackerId: string, delta: number) {
    void patchProjects((list) =>
      mapTracker(list, projectId, trackerId, (t) => ({ ...t, value: t.value + delta })),
    );
  }

  function setTrackerValue(projectId: string, trackerId: string, raw: string) {
    const n = parseInt(raw.trim(), 10);
    if (!Number.isFinite(n)) return;
    void patchProjects((list) =>
      mapTracker(list, projectId, trackerId, (t) => (t.value === n ? t : { ...t, value: n })),
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
        const next: Tracker = { ...t, kind: "timer", value: Math.round(elapsedSec(t, stamp)) };
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
        if (spokeLive) {
          spokeLive = false;
          focusCol = "middle";
          focusStarsCol("middle");
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
      if (spokeLive) {
        spokeLive = false;
        focusCol = "middle";
        focusStarsCol("middle");
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

  onMount(() => {
    const el = mainEl;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width ?? el.clientWidth;
      if (w > 0) mainW = w;
    });
    ro.observe(el);
    if (el.clientWidth > 0) mainW = el.clientWidth;
    return () => ro.disconnect();
  });

  onMount(() => {
    window.addEventListener("keydown", onStarsKey);
    return () => window.removeEventListener("keydown", onStarsKey);
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

  function starsRows(col: StarsCol): HTMLElement[] {
    const root = mainEl;
    if (!root) return [];
    const sel =
      col === "planets"
        ? ".planet-col [data-stars-row]"
        : col === "trackers"
          ? ".tracker-col [data-stars-row]"
          : ".mid-col [data-stars-row]";
    return [...root.querySelectorAll<HTMLElement>(sel)];
  }

  function focusStarsCol(col: StarsCol) {
    const root = mainEl;
    if (!root) return;
    const sel =
      col === "planets" ? ".planet-col" : col === "middle" ? ".mid-col" : ".tracker-col";
    root.querySelector<HTMLElement>(sel)?.focus();
  }

  function onStarsKey(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return;
    const t = e.target;
    if (t instanceof Element && t.closest("input, textarea, select, [contenteditable='true']")) return;
    if (layout === "single" && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      e.preventDefault();
      segment = segmentNext(segment, e.key === "ArrowRight" ? "right" : "left");
      mainEl?.querySelector<HTMLElement>(`[data-seg="${segment}"]`)?.focus();
      return;
    }
    if (e.key === "Enter" && focusCol === "middle" && !spokeLive) {
      if (t instanceof Element && t.closest("button, a")) return;
      e.preventDefault();
      spokeLive = true;
      mainEl?.querySelector<HTMLElement>(".handle")?.focus();
      return;
    }
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight" && e.key !== "ArrowUp" && e.key !== "ArrowDown") {
      return;
    }
    if (spokeLive && (e.key === "ArrowUp" || e.key === "ArrowDown")) return;
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      if (layout === "single") return;
      const dir = e.key === "ArrowRight" ? "right" : "left";
      const next = columnNext(layout, focusCol, dir, { embedded, outward: outwardDir });
      if (next.exit) {
        e.preventDefault();
        onRingExit?.();
        return;
      }
      if (next.col !== focusCol) {
        e.preventDefault();
        spokeLive = false;
        focusCol = next.col;
        focusIdx = 0;
        focusStarsCol(next.col);
      }
      return;
    }
    if (focusCol === "planets") return;
    const rows = starsRows(focusCol);
    if (rows.length === 0) return;
    e.preventDefault();
    focusIdx = withinNext(focusCol, focusIdx, e.key === "ArrowDown" ? "down" : "up", rows.length);
    rows[focusIdx]?.focus();
  }

  const showCheckin = $derived(layout !== "single" || segment === "checkin");
  const showTrackers = $derived(layout !== "single" || segment === "trackers");
  const showJournal = $derived(layout !== "single" || segment === "journal");
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

<main class="stars orbit-field" class:embedded bind:this={mainEl} data-layout={layout} data-ready={starsLoaded ? "1" : "0"}>
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

  {#if layout === "single"}
    <div class="segments" role="tablist" aria-label="Stars sections">
      <button type="button" role="tab" data-seg="checkin" aria-selected={segment === "checkin"} onclick={() => (segment = "checkin")}>Check-in</button>
      <button type="button" role="tab" data-seg="trackers" aria-selected={segment === "trackers"} onclick={() => (segment = "trackers")}>Trackers</button>
      <button type="button" role="tab" data-seg="journal" aria-selected={segment === "journal"} onclick={() => (segment = "journal")}>Journal</button>
    </div>
  {/if}

  <div class="s-body" style="grid-template-columns: {gridCols}">
    {#if layout !== "single"}
      <StarsPlanetColumn
        {projects}
        scope={trackerScope}
        dots={layout === "dots"}
        active={focusCol === "planets" && !spokeLive}
        onScope={(id) => (trackerScope = id)}
        onReorder={reorderPlanetsLive}
        onReorderCommit={commitPlanetOrder}
      />
    {/if}
    {#if showCheckin || showJournal}
      <div class="mid-col" role="listbox" aria-label="Radar and folio" tabindex="0" data-stars-col="middle">
        {#if showCheckin}
          <section class="checkin" aria-label="Check-in" data-stars-row="0" tabindex="-1">
            <RadarDial
              spokes={STARS_SPOKES}
              shown={radarShown}
              editable={dayEditable}
              {avgMode}
              draft={radarIsDraft}
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
                class:dirty={draftDirty || radarIsDraft}
                disabled={!isToday || avgMode || (!dayEditable && !!cursorSaved)}
                onclick={saveStarsToday}
              >Save today</button>
              <button
                type="button"
                class="mini quiet"
                disabled={!isToday || avgMode || !cursorSaved || dayEditable}
                onclick={() => (editingToday = true)}
              >Edit</button>
              {#if isToday}
                {#if starsSavedDay}
                  <span class="stars-note">Saved for {starsSavedDay}</span>
                {:else if radarIsDraft}
                  <span class="stars-note">Draft</span>
                {/if}
              {:else if cursorSaved}
                <span class="stars-note">Saved for {cursorDay}</span>
              {:else}
                <span class="stars-note">No save</span>
              {/if}
            </div>
          </section>
        {/if}
        {#if showJournal}
          <div class="folio-wrap" data-stars-row="1" tabindex="-1">
          <AlmanacFolio
            pages={starsState.ledgerPages ?? []}
            {projects}
            dayKey={cursorDay}
            rangeFrom={bounds.from}
            rangeTo={bounds.to}
            view={navView}
            today={todayKey()}
            face={dayFace}
            hint={reflectHint}
            hasWords={(day) => wordDays.has(day)}
            onReflect={openReflect}
            onSelectDay={(day) => (cursorDay = day)}
          />
          </div>
        {/if}
      </div>
    {/if}
    {#if showTrackers}
      <div class="tracker-col" role="listbox" aria-label="Trackers" tabindex="0" data-stars-col="trackers">
        <StarsTrackers
          {projects}
          scope={starsScopeTargets(trackerScope).trackers}
          showPlanetMenu={layout === "single"}
          onScope={(id) => (trackerScope = id)}
          onAdd={addTracker}
          onBump={bumpTracker}
          onSetValue={setTrackerValue}
          onRemove={removeTracker}
          onPlay={playTimer}
          onStop={stopTimer}
        />
      </div>
    {/if}
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
    background: var(--bg);
    font-family:
      "SF Pro Text", "SF Pro Display", system-ui, -apple-system,
      BlinkMacSystemFont, "Helvetica Neue", sans-serif;
  }

  .stars.embedded {
    height: 100%;
    flex: 1;
    min-height: 0;
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

  .nav-view,
  .nav-date,
  .segments button,
  .mini,
  .chron-prompts {
    font: inherit;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 7px;
  }

  .nav-view {
    font-size: 0.72rem;
    padding: 2px 6px;
    margin-left: 6px;
  }

  .nav-date {
    font-size: 0.68rem;
    padding: 1px 4px;
    max-width: 9.2rem;
    background: var(--panel-fill, var(--panel));
  }

  .s-banner .winx {
    margin-left: 4px;
  }

  .segments {
    display: flex;
    gap: 4px;
    padding: 8px 12px 0;
    flex: none;
  }

  .segments button {
    font-size: 0.72rem;
    min-height: 28px;
    padding: 2px 10px;
    background: transparent;
    cursor: pointer;
  }

  .segments button[aria-selected="true"] {
    background: var(--panel-fill);
  }

  .s-body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: grid;
    align-items: stretch;
    gap: 10px;
    padding: 10px 12px 12px;
  }

  .mid-col,
  .tracker-col {
    min-width: 0;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .mid-col:focus-visible,
  .tracker-col:focus-visible,
  .segments button:focus-visible {
    outline: none;
    box-shadow: var(--focus-ring);
  }

  .checkin {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 0;
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
    flex-wrap: wrap;
  }

  .mini {
    font-size: 0.68rem;
    background: transparent;
    padding: 3px 8px;
    min-height: 24px;
    cursor: pointer;
  }

  .mini.quiet {
    border-color: transparent;
    color: var(--dim);
  }

  .mini:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .stars-save:not(:disabled) {
    font-weight: 700;
    background: color-mix(in srgb, var(--accent) 24%, transparent);
    color: var(--ink);
    border-color: var(--accent);
  }

  .chron-prompts {
    flex: none;
    font-size: 0.68rem;
    font-weight: 650;
    color: var(--dim);
    background: transparent;
    border-radius: 999px;
    padding: 1px 8px;
    cursor: pointer;
  }

  .chron-prompts.on {
    color: var(--ink);
    font-weight: 700;
    background: var(--panel-fill, var(--panel));
  }

  .stars-note {
    font-size: 0.64rem;
    color: var(--dim);
  }
</style>
