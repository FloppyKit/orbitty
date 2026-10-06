<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { emitTo, listen } from "@tauri-apps/api/event";
  import { open as shellOpen } from "@tauri-apps/plugin-shell";
  import { onMount, onDestroy } from "svelte";
  import {
    applyHostTokens,
    applyWindowOpacity,
    clampWindowOpacity,
  } from "../themes/apply";
  import { planetCss } from "./planetColor";
  import { startPaneDrag } from "./dragPane";
  import { MenuRing, type RingStop } from "./menuKeys.svelte";
  import RailStack from "./RailStack.svelte";
  import { maybeGraduatePeerChrome, peerChromeOn } from "./peerChrome";
  import "./chrome.css";
  import "./rail/rail.css";
  import RailChrome from "./rail/Chrome.svelte";
  import RailList from "./rail/List.svelte";
  import { coachDue, normalizeStory, type CoachStory } from "./rail/coach";
  import RailClaimUi from "./rail/ClaimUi.svelte";
  import {
    PIN_H,
    PREVIEW_MARGIN,
    WHEEL_COL,
    WHEEL_ROW,
    paintedZoneHit,
    spinConfirm,
    spinItems,
    spinStops,
    wheelLayout,
  } from "./rail/stageLook";
  import {
    ARCH_IDLE,
    archArrow,
    archBack,
    archConfirm,
    ARCH_DWELL_MS,
    archFocusPid,
    archLayer2Items,
    archLayout,
    archOpenBoard,
    archPointer,
    archZone,
    type ArchState,
    type ArchZone,
    type Layer2Item,
  } from "./rail/stageArch";
  import {
    applyPlanetOpen,
    autoHideWanted,
    habitatConfirm,
    normalizeCols,
    planetCount,
    raisePlan,
    revealWanted,
    stageCollapsedIds,
    stagePrefs,
    stageRoles,
    toggleCol,
    type BoardCol,
    type RailEdge,
    type StageRole,
  } from "./rail/habitat";
  import type {
    ActionShortcut,
    AgentRun,
    AgentShortcut,
    CardTombstone,
    CharLook,
    Claim,
    CompanionSlice,
    DrawerSlice,
    KanbanCard,
    PersistedState,
    PointsEntry,
    Project,
    SkySlice,
    StarsSlice,
    Tag,
    TitleOverride,
    WindowInfo,
    WindowTags,
  } from "./persist";

  // Rail brain — mounted when +page.svelte dispatches the `main` window,
  // and again embedded in the companion inspector well.
  // Overlay windows never load this file.

  // Embedded mounts this page in the companion's
  // inspector well — × calls onClose (never hide_window), the banner
  // title is Planets, and a pop-out control (onPopout) hands the
  // chrome back to the real rail window. The overlay passes nothing.
  let {
    embedded = false,
    onClose,
    onPopout,
    createPlanetTick = 0,
  }: {
    embedded?: boolean;
    onClose?: () => void;
    onPopout?: () => void;
    createPlanetTick?: number;
  } = $props();

  interface AxStatus {
    trusted: boolean;
    bundlePath: string;
    signingLine: string;
    inBundle: boolean;
    isAdhoc: boolean;
  }

  // One Season 1 kind: display name, pack slug base, and its palette names
  // ("Default" + 3 recolors) — the Being editor's only silhouette controls.
  interface BittyKind {
    name: string;
    slug: string;
    palettes: string[];
  }

  interface Theme {
    id: string;
    name: string;
    block: "System" | "Custom";
  }

  // System-first: the primary pair keeps the Tahoe semantic fills and wears
  // the Orbitty brand triad for accent/mark/focus.
  // S4.2 retired Clear — translucency is the Background opacity slider, not a
  // theme. Custom holds Terminal only; the seven old palette skins are gone.
  const THEMES: Theme[] = [
    { id: "systemLight", name: "Light", block: "System" },
    { id: "systemDark", name: "Dark", block: "System" },
    { id: "terminal", name: "Terminal", block: "Custom" },
  ];
  const THEME_IDS = new Set(THEMES.map((t) => t.id));

  function osThemeId(): string {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "systemDark"
      : "systemLight";
  }

  function defaultThemeId(): string {
    return osThemeId();
  }

  // Old saves may name retired ids (clear / liquidGlass included); anything
  // unrecognized falls back to OS Light/Dark, keeping the saved opacity.
  function migrateThemeId(id: string): string {
    return THEME_IDS.has(id) ? id : "";
  }

  let status = $state<AxStatus | null>(null);
  let windows = $state<WindowInfo[]>([]);
  let refreshedAt = $state("");
  let copied = $state(false);

  let themeId = $state("");
  // True once the theme came from a saved state or an explicit pick. While
  // false the theme is the OS default and tracks live OS light/dark flips.
  let themeChosen = false;
  // S4.3: window background opacity (0.2–1.0, default 1.0). Fades only the
  // chrome fills via JS-computed --bg-fill/--panel-fill, never text/icons.
  let windowOpacity = $state(1.0);
  let projects = $state<Project[]>([]);
  let claims = $state<Claim[]>([]);
  let tags = $state<Tag[]>([]);
  let windowTags = $state<WindowTags[]>([]);
  let collapsedSectionIds = $state<string[]>([]);
  // Claim-first nudge: shown once per window id, then silent (persisted).
  let nudgeSuppressed = $state<string[]>([]);
  let nudgeFor = $state<string | null>(null);
  let nudgeNewName = $state("");
  let stateLoaded = $state(false);
  let starterStory = $state<CoachStory>(normalizeStory(null));
  let coachError = $state("");
  let coachRevealed = false;
  let bootParkDone = false;
  let coachErrorTimer: ReturnType<typeof setTimeout> | undefined;

  // S4: local Kanban + points ledger (persisted), zombie ages.
  let kanbanCards = $state<KanbanCard[]>([]);
  let cardTombstones = $state<CardTombstone[]>([]);
  let pointsLedger = $state<PointsEntry[]>([]);
  // Display-only window title renames, persisted; rail is the only writer.
  let titleOverrides = $state<TitleOverride[]>([]);
  // Inline rename: window id being edited + the draft string.
  let renamingId = $state<string | null>(null);
  let renameValue = $state("");
  // Right-click on a window title: tiny context menu (View on board / Rename).
  let ctxMenu = $state<{ x: number; y: number; windowId: string } | null>(null);
  // S4.6 circle-complete: ids mid fill-animation, then the row completes.
  let completingIds = $state<string[]>([]);
  // Per-section "+ To-do" inline form.
  let addingTodoFor = $state<string | null>(null);
  let newTodoTitle = $state("");

  // S5: Hermes coordinator note (dismissible) + per-project soul.md panel.
  // Both live in the Agents lane now — never on project headers.
  let hermesNote = $state<{ kind: "ok" | "err"; text: string } | null>(null);
  let soulFor = $state<string | null>(null);
  let soulBody = $state("");
  let soulPath = $state("");
  let soulMsg = $state("");

  // Settings (gear): chrome toggles, persisted in state.json.
  let autohideRowChrome = $state(true);
  let railEdge = $state<RailEdge>("left");
  let railAutoHideOnBlur = $state(true);
  let stageRevealed = $state(false);
  let reduceMotion = $state(false);
  let arch = $state<ArchState>({ ...ARCH_IDLE });
  let spinCenter = $state(0);
  let pointerInside = false;
  let focusInside = false;
  let hideTitles = $state(false);
  let hideAppIcons = $state(false);
  let railBoardCols = $state<BoardCol[]>(["todo"]);
  let spriteScale = $state(1);
  // Settings owns the toggle; the rail round-trips
  // it and rides it to the native habitat on the snapshot.
  let peerChrome = $state(false);
  let marbleDrift = $state(false);
  let planetIdle = $state("4h");
  // No-herdr terminal. Round-tripped so a rail save does not drop it.
  let agentTerminal = $state("");

  // Settings / Launchpad own the writers. Rail only round-trips these so
  // persist() never strips the arrays.
  let actionShortcuts = $state<ActionShortcut[]>([]);
  let agentShortcuts = $state<AgentShortcut[]>([]);
  // The doing-run rows (agent walkers). The
  // native attach door owns mint/done/drop; the rail round-trips the
  // slice, mirrors it into the habitat snapshot, and evicts the done row
  // on habitat:walked-off.
  let agentRuns = $state<AgentRun[]>([]);
  let neededSplit = $state(0.28);
  let previewWidth = $state(280);
  let trackerSplit = $state(0.20);
  let openSubtaskIds = $state<string[]>([]);
  let todoTrees = $state<import("./persist").TodoTree[]>([]);
  let treeGrowDays = $state(14);

  // Companion slice, round-tripped opaquely (the
  // companion window and the native side own the contents).
  let companionState = $state<CompanionSlice>({
    visible: false,
    bounds: null,
    imagePackId: null,
    state: "idle",
    // required key — a constructed slice must carry
    // the flag (default off) so persist() can never omit it.
    freeMode: false,
  });
  // the planet an agent is writing on
  // (companion.hackLive + that agent's pinned projectId) glows a little
  // in the planet menus — the On planets sheet heads only, never the
  // habitat discs, never the walkers.
  const hackProjectId = $derived(
    companionState?.hackLive ? (companionState?.hackProjectId ?? null) : null,
  );

  // The STARS window owns check-ins (Save today)
  // and the Prompts toggle; the companion rides promptsOptIn/promptsAck
  // through native commands. The rail only round-trips the slice — loaded
  // once, adopted on state:changed, written back untouched by persist().
  let starsState = $state<StarsSlice>({ promptsOptIn: false, checkins: {} });

  // The Sky window owns the mute list; the rail only
  // round-trips it so persist() never strips the slice.
  let skyState = $state<SkySlice>({ muted: [] });

  // The native leash owns dock/pluck state; the
  // rail only round-trips it so persist() never strips the slice.
  let drawersState = $state<Record<string, DrawerSlice>>({});

  // Pack ids ride the habitat signature so an import/remove redraws
  // walkers. Import UI is Settings + the walker NSMenu (inline invoke).
  interface PackInfo {
    id: string;
    name: string;
    license: string | null;
    warnLicense: boolean;
    frameCount: number;
    path: string;
    role: string;
  }
  let packs = $state<PackInfo[]>([]);

  async function refreshPacks() {
    try {
      packs = await invoke<PackInfo[]>("list_packs");
    } catch {
      // packs are decorative; the rail never blocks on them
    }
  }

  // Rail lanes (list-head tabs): Projects is the default window list;
  // Agents holds Hermes/Soul. Session-only.
  let lane = $state<"projects" | "agents">("projects");

  const DAY_S = 86400;
  const DAILY_CAP = 12;
  const FADE_AFTER_S = 7 * DAY_S;
  const PARK_AFTER_S = 14 * DAY_S;
  const GONE_AFTER_S = 21 * DAY_S;

  function nowEpoch(): number {
    return Date.now() / 1000;
  }

  function todayKey(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }

  // Zombie aging (TS-7): visual only, titles only, never auto-kill.
  function ageClass(windowId: string): "fresh" | "fade" | "park" | "gone" {
    const t = claims.find((c) => c.windowId === windowId)?.lastTouchedAt ?? 0;
    if (t <= 0) return "fresh";
    const age = nowEpoch() - t;
    if (age >= GONE_AFTER_S) return "gone";
    if (age >= PARK_AFTER_S) return "park";
    if (age >= FADE_AFTER_S) return "fade";
    return "fresh";
  }

  function pointsToday(): number {
    const key = todayKey();
    return pointsLedger
      .filter((e) => e.day === key)
      .reduce((a, e) => a + e.amount, 0);
  }

  // Touch bumps: explicit actions persist immediately; the 800ms frontmost
  // tick funnels through a 30s throttle so the poll can't thrash the disk.
  let lastTouchPersist = 0;
  function touchClaim(windowId: string, force = false) {
    const now = nowEpoch();
    claims = claims.map((c) =>
      c.windowId === windowId ? { ...c, lastTouchedAt: now } : c,
    );
    if (force || now - lastTouchPersist > 30) {
      lastTouchPersist = now;
      persist();
    }
  }


  let newProjectName = $state("");
  let creatingProject = $state(false);
  // List-head filter chip: null = All. Session-only, not persisted.
  let tagFilter = $state<string | null>(null);

  // Drag-and-drop claim: window id being dragged + section key under the cursor.
  const UNMAPPED_KEY = "__unmapped__";
  let dragWindowId = $state<string | null>(null);
  let dropTarget = $state<string | null>(null);
  // WORLD-TOWN LEFTOVER hitch: while a window drag is in flight — or within
  // 400ms of dragstart (Svelte state can be lost mid-gesture) — the rail
  // freezes AX polls and habitat syncs so the HTML5 session is never
  // cancelled and the drop never beach-balls. claim() still fires on drop.
  let dragStartedAt = 0;
  function windowDragHot(): boolean {
    return !!dragWindowId || Date.now() - dragStartedAt < 400;
  }
  // A state:changed that arrives mid-drag is stashed, not applied. Applying
  // card rows rebuilds the section and cancels the HTML5 drag. persist()
  // and drag-end absorb the stash first, so a save never writes cards that
  // skipped the reload. A local edit wins only when its lastTouchedAt is
  // strictly newer than the disk card.
  let persistDeferred = false;
  let pendingDisk: {
    kanbanCards: KanbanCard[];
    pointsLedger: PointsEntry[];
    agentRuns: AgentRun[];
    cardTombstones: CardTombstone[];
  } | null = null;

  function absorbPendingCards() {
    const disk = pendingDisk;
    if (!disk) return;
    pendingDisk = null;
    const local = kanbanCards;
    const merged = new Map<string, KanbanCard>();
    for (const c of disk.kanbanCards) merged.set(c.id, c);
    for (const c of local) {
      const have = merged.get(c.id);
      if (!have || c.lastTouchedAt > have.lastTouchedAt) merged.set(c.id, c);
    }
    for (const t of disk.cardTombstones) {
      const c = merged.get(t.id);
      const localNewer = local.some((x) => x.id === t.id && x.lastTouchedAt > t.lastTouchedAt);
      if (c && t.lastTouchedAt >= c.lastTouchedAt && !localNewer) merged.delete(t.id);
    }
    kanbanCards = [...merged.values()];
    pointsLedger = disk.pointsLedger;
    agentRuns = disk.agentRuns;
    cardTombstones = disk.cardTombstones;
  }

  // Claim counts only when its project still exists.
  let liveClaims = $derived(
    claims.filter((c) => projects.some((p) => p.id === c.projectId)),
  );

  let unmapped = $derived(
    windows.filter((w) => !liveClaims.some((c) => c.windowId === w.id)),
  );

  // S4.6: open todos live inside their project section on the rail, oldest
  // first — the same cards the Lists window shows.
  function todosFor(projectId: string): KanbanCard[] {
    return kanbanCards
      .filter(
        // Needed cards never count as rail todos.
        (c) =>
          c.kind !== "needed" &&
          c.status !== "done" &&
          !c.windowId &&
          c.projectId === projectId,
      )
      .sort((a, b) => a.createdAt - b.createdAt);
  }

  let nudgeWindow = $derived(
    nudgeFor ? windows.find((w) => w.id === nudgeFor) : undefined,
  );

  // Tag assignments pruned to tags that still exist (same pattern as claims).
  let liveWindowTags = $derived(
    windowTags
      .map((wt) => ({
        windowId: wt.windowId,
        tagIds: wt.tagIds.filter((id) => tags.some((t) => t.id === id)),
      }))
      .filter((wt) => wt.tagIds.length > 0),
  );

  function rowsFor(projectId: string): WindowInfo[] {
    const live = windows.filter((w) =>
      liveClaims.some((c) => c.windowId === w.id && c.projectId === projectId),
    );
    // A claimed window that closed stays listed as an
    // asleep row, synthesized from the claim fingerprint (icon may be gone —
    // that's fine). Done/Unclaim is the only way it leaves.
    const sleepers: WindowInfo[] = liveClaims
      .filter(
        (c) =>
          c.projectId === projectId &&
          !windows.some((w) => w.id === c.windowId),
      )
      .map((c) => ({
        id: c.windowId,
        pid: 0,
        appName: c.appName ?? "App",
        title: c.titleAtClaim || c.appName || "",
        bundleId: c.bundleId ?? null,
        stableKey: claimStableKey(c) ?? "",
        iconBase64: null,
        isFrontmost: false,
        isMinimized: true,
        documentPath: c.filePath ?? null,
        sleeping: true,
      }));
    return [...live, ...sleepers];
  }

  // The tag filter narrows rows inside each section; grouping stays by home
  // project — tags never create sections.
  function visibleRows(rows: WindowInfo[]): WindowInfo[] {
    if (!tagFilter) return rows;
    return rows.filter((w) => hasTag(w.id, tagFilter!));
  }

  function hasTag(windowId: string, tagId: string): boolean {
    return liveWindowTags.some(
      (wt) => wt.windowId === windowId && wt.tagIds.includes(tagId),
    );
  }

  function tagsFor(windowId: string): Tag[] {
    const ids =
      liveWindowTags.find((wt) => wt.windowId === windowId)?.tagIds ?? [];
    return tags.filter((t) => ids.includes(t.id));
  }

  function isCollapsed(sectionId: string): boolean {
    return collapsedSectionIds.includes(sectionId);
  }

  function toggleCollapse(sectionId: string) {
    collapsedSectionIds = isCollapsed(sectionId)
      ? collapsedSectionIds.filter((id) => id !== sectionId)
      : [...collapsedSectionIds, sectionId];
    persist();
  }

  function sectionIds(): string[] {
    return [...projects.map((p) => p.id), UNMAPPED_KEY];
  }

  function openSectionId(): string | null {
    const open = sectionIds().filter((id) => !isCollapsed(id));
    return open.length === 1 ? open[0] : null;
  }

  function stageRole(pid: string): StageRole {
    return stageRoles(sectionIds(), openSectionId()).find((r) => r.id === pid)?.role ?? "icon";
  }

  function ensureExpand(pid: string) {
    const ids = sectionIds();
    if (!ids.includes(pid)) return;
    const nextOpen = applyPlanetOpen(openSectionId(), ids, pid);
    collapsedSectionIds = nextOpen ? ids.filter((id) => id !== nextOpen) : [...ids];
    persist();
    void showStage();
  }

  function collapsePlanet(pid: string) {
    if (!sectionIds().includes(pid) || isCollapsed(pid)) return;
    collapsedSectionIds = [...collapsedSectionIds, pid];
    persist();
  }

  function openStageDest(label: "stars" | "settings") {
    void invoke("toggle_labeled", { label });
  }

  function coachIsDue(): boolean {
    return coachDue({
      trusted: !!status?.trusted,
      stateLoaded,
      claimsCount: claims.length,
      story: starterStory,
      embedded,
    });
  }

  function maybeRevealCoach() {
    if (!bootParkDone || coachRevealed || embedded) return;
    if (!coachIsDue()) return;
    coachRevealed = true;
    void showStage();
  }

  function noteAx(s: AxStatus) {
    status = s;
    maybeRevealCoach();
  }

  async function coachClaim() {
    coachError = "";
    try {
      await invoke("claim_front_window");
    } catch (e) {
      const text = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
      if (text.includes("cancelled")) return;
      coachError = text;
      if (coachErrorTimer) clearTimeout(coachErrorTimer);
      coachErrorTimer = setTimeout(() => {
        coachError = "";
      }, 6000);
    }
  }

  async function coachSkip() {
    try {
      await invoke("set_starter_coach_done");
    } catch {
      /* the flag lands on the next state:changed */
    }
  }

  async function showStage() {
    if (embedded) return;
    stageRevealed = true;
    try {
      await invoke("rail_stage_set", { revealed: true });
    } catch {
      /* the strip still paints if the native place is unavailable */
    }
  }

  async function parkStage() {
    if (embedded) return;
    stageRevealed = false;
    try {
      await invoke("rail_stage_set", { revealed: false });
    } catch {
      /* css parks the strip */
    }
  }

  function maybeAutoHide() {
    if (embedded || !stageRevealed) return;
    if (!autoHideWanted({ railAutoHideOnBlur, focusInside, pointerInside })) return;
    void parkStage();
  }

  function stagePointer(inside: boolean) {
    if (embedded) return;
    pointerInside = inside;
    if (inside && revealWanted(true, false)) void showStage();
    else maybeAutoHide();
  }

  function stageFocus(inside: boolean) {
    if (embedded) return;
    focusInside = inside;
    if (!inside) maybeAutoHide();
  }

  function previewStop(id: string | null): boolean {
    if (!id) return false;
    return (
      id.startsWith("rraise-") ||
      id.startsWith("rtile-") ||
      id.startsWith("rsnap-") ||
      id.startsWith("rtodos-")
    );
  }

  function overPainted(e: PointerEvent): boolean {
    const host = e.currentTarget;
    if (!(host instanceof HTMLElement)) return false;
    const rect = host.getBoundingClientRect();
    const layout = wheelLayout(railEdge, rect.width, rect.height);
    return paintedZoneHit(
      { x: e.clientX - rect.left, y: e.clientY - rect.top },
      layout,
      stageRevealed,
    );
  }

  let dwellTimer: ReturnType<typeof setTimeout> | null = null;

  function archZoneAt(e: PointerEvent): ArchZone {
    const host = e.currentTarget;
    if (!(host instanceof HTMLElement)) return "none";
    const rect = host.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return "none";
    return archZone(
      archLayout(railEdge, rect.width, rect.height),
      { x: e.clientX - rect.left, y: e.clientY - rect.top },
      arch.front,
    );
  }

  function pokeArch(zone: ArchZone) {
    const now = Date.now();
    const prev = arch;
    const next = archPointer(prev, zone, now);
    const armed =
      !!next.pending &&
      (prev.pending?.zone !== next.pending.zone || prev.pending?.since !== next.pending.since);
    archDo(next);
    if (!next.pending) {
      if (dwellTimer) clearTimeout(dwellTimer);
      dwellTimer = null;
      return;
    }
    if (!armed) return;
    if (dwellTimer) clearTimeout(dwellTimer);
    const wait = Math.max(0, ARCH_DWELL_MS - (now - next.pending.since));
    const z = next.pending.zone;
    dwellTimer = setTimeout(() => {
      dwellTimer = null;
      archDo(archPointer(arch, z, Date.now()));
    }, wait);
  }

  function onStagePointerMove(e: PointerEvent) {
    if (embedded) return;
    if (!stageRevealed) {
      if (!pointerInside) stagePointer(true);
      return;
    }
    pokeArch(archZoneAt(e));
    const hit = overPainted(e);
    if (hit !== pointerInside) stagePointer(hit);
  }

  function onStagePointerDown(e: PointerEvent) {
    if (embedded || !stageRevealed) return;
    if (overPainted(e)) return;
    pointerInside = false;
    stageFocus(false);
  }

  function onWinBlur() {
    stageFocus(false);
  }

  function applyStagePrefs(raw: {
    railEdge?: unknown;
    railAutoHideOnBlur?: unknown;
    autohideRowChrome?: unknown;
  }) {
    const prefs = stagePrefs({
      railEdge: raw.railEdge,
      railAutoHideOnBlur: raw.railAutoHideOnBlur,
      autohideRowChrome:
        raw.autohideRowChrome === undefined ? autohideRowChrome : raw.autohideRowChrome,
    });
    railEdge = prefs.railEdge;
    railAutoHideOnBlur = prefs.railAutoHideOnBlur;
  }

  // The hosted well (embedded) is a top strip of
  // planet tabs (Unclaimed · orbs · +) and one body. Windows only: no
  // Agents lane, tags, +To-do, Actions, or being editor in the well (the
  // pop-out rail keeps the full inspector). + Planet is the one mint.
  // Tab click raises/locates that disc (old dest-menu click). Double-click
  // emits habitat:expand/collapse to main so the rail-owned collapse flag
  // turns the disc into a marble. Unclaimed is slot 0.
  // Live windows claimed onto one planet. Sleepers (closed claims) stay on
  // the pop-out inspector — the sheet lists focusable windows only.
  function sheetRowsFor(projectId: string): WindowInfo[] {
    return windows.filter((w) =>
      liveClaims.some((c) => c.windowId === w.id && c.projectId === projectId),
    );
  }

  // Unclaimed row → claim ▸ opens the planet picker for that window; the
  // pick lands through the exclusive claim() door (CLAIM-ONE). New planets
  // mint through createProjectOnly — the strip + and createPlanetTick are
  // the same door.
  let claimPickerFor = $state<string | null>(null);
  let sheetNameEl: HTMLInputElement | null = $state(null);
  // PLANETS-STRIP: which tab's windows fill the body. Unclaimed first.
  let sheetSlot = $state<string>("unclaimed");

  function raisePlanet(id: string) {
    invoke("bring_planet_front", { id }).catch(() => {});
    invoke("locate_planet", { id }).catch(() => {});
  }

  function onSheetPlanetClick(id: string) {
    sheetSlot = id;
    raisePlanet(id);
  }

  function onSheetPlanetDblClick(id: string) {
    const collapsed = isCollapsed(id);
    emitTo("main", collapsed ? "habitat:expand" : "habitat:collapse", id).catch(
      () => {},
    );
  }

  $effect(() => {
    if (embedded && createPlanetTick > 0) creatingProject = true;
  });
  $effect(() => {
    if (embedded && creatingProject) {
      queueMicrotask(() => sheetNameEl?.focus());
    }
  });

  function sheetClaim(windowId: string, projectId: string) {
    claimPickerFor = null;
    claim(windowId, projectId);
  }

  // On-planets row click → focus the OS window (deminiaturize + raise).
  function sheetFocus(w: WindowInfo) {
    invoke("focus_window", { id: w.id }).catch(() => {});
  }

  // The Planets-pop ring — orb tabs across the
  // top, window rows under the selected tab. Left/right walks the strip,
  // down drops into the rows, up climbs back to the tab; Enter is the
  // click (tab = select + raise/locate the disc, row = focus the window,
  // Unclaimed row = the claim picker, pick = claim). Esc closes the
  // picker / the + Planet mint first, then the well (CompanionPage's
  // bubble Esc). The Inspector window attaches the same ring. Habitat
  // walkers are not on this key path.
  const menuRing = new MenuRing();

  function toggleBoardCol(col: string) {
    railBoardCols = toggleCol(railBoardCols, col);
    persist();
  }

  async function focusRow(w: WindowInfo) {
    try {
      await invoke("focus_window", { id: w.id });
    } catch {
      /* app may refuse; the next tick shows where it landed */
    }
    touchClaim(w.id, true);
    await tick();
  }

  async function raisePlanetWindows(pid: string) {
    const plan = raisePlan(rowsFor(pid));
    for (const id of plan.restore) {
      try {
        await invoke("restore_window", { id });
      } catch {
        /* a closed window drops out; focus still runs */
      }
    }
    for (let i = 0; i < plan.focus.length; i++) {
      try {
        await invoke("focus_window", { id: plan.focus[i] });
      } catch {
        /* skip a window the OS will not raise */
      }
      if (i + 1 < plan.focus.length) {
        await new Promise((r) => setTimeout(r, 40));
      }
    }
    raisePlanet(pid);
    await tick();
  }

  async function openPlanetTodos(pid: string) {
    await openKanban();
    emitTo("kanban", "orbitty:focus-project", pid).catch(() => {});
    setTimeout(
      () => emitTo("kanban", "orbitty:focus-project", pid).catch(() => {}),
      400,
    );
  }

  function rowById(id: string): WindowInfo | undefined {
    for (const p of projects) {
      const hit = rowsFor(p.id).find((w) => w.id === id);
      if (hit) return hit;
    }
    return windows.find((w) => w.id === id);
  }

  function archDo(next: ArchState) {
    const pendingSame =
      (next.pending == null && arch.pending == null) ||
      (!!next.pending &&
        !!arch.pending &&
        next.pending.zone === arch.pending.zone &&
        next.pending.since === arch.pending.since);
    if (
      next.front === arch.front &&
      next.pid === arch.pid &&
      next.center2 === arch.center2 &&
      pendingSame
    ) {
      return;
    }
    arch = next;
  }

  function planetCards(pid: string): KanbanCard[] {
    return kanbanCards.filter((c) => c.projectId === pid);
  }

  function layer2For(pid: string | null): Layer2Item[] {
    if (!pid || pid === UNMAPPED_KEY) return [];
    return archLayer2Items({
      pid,
      cards: planetCards(pid),
      rows: rowsFor(pid).map((w) => ({
        id: w.id,
        title: titleFor(w),
        sleeping: !!w.sleeping,
        isMinimized: !!w.isMinimized,
        iconBase64: w.iconBase64,
        appName: w.appName,
      })),
    });
  }

  function confirmLayer2Stop(id: string) {
    const item = layer2For(arch.pid).find((it) => it.stop === id);
    if (!item || !arch.pid) return;
    const action = archConfirm(item);
    if (action === "board") {
      archDo(archOpenBoard(arch, UNMAPPED_KEY));
      return;
    }
    const w = rowsFor(arch.pid).find((row) => row.id === item.id);
    if (!w) return;
    if (action === "wake") void wakeSleeper(w);
    else if (action === "restore") void restore(w).then(() => focusRow(w));
    else void focusRow(w);
  }

  let lastNoteAct = 0;
  function noteRailAct() {
    if (embedded) return;
    const now = Date.now();
    if (now - lastNoteAct < 1000) return;
    lastNoteAct = now;
    invoke("starter_note_act").catch(() => {});
  }

  function confirmHabitatStop(id: string) {
    const spin = spinConfirm(id);
    if (spin === "stars") {
      openStageDest("stars");
      return;
    }
    if (spin === "settings") {
      openStageDest("settings");
      return;
    }
    if (spin === "newProject") {
      creatingProject = true;
      return;
    }
    if (spin === "coachClaim") {
      void coachClaim();
      return;
    }
    if (spin === "coachSkip") {
      void coachSkip();
      return;
    }
    if (spin === "centre") return;
    if (spin === "raise") {
      void raisePlanetWindows(id.slice("rhead-".length));
      return;
    }
    const action = habitatConfirm(id);
    if (action === "toggleCol") {
      toggleBoardCol(id.slice("rcol-".length));
      return;
    }
    if (action === "stars") {
      openStageDest("stars");
      return;
    }
    if (action === "settings") {
      openStageDest("settings");
      return;
    }
    if (action === "expand") {
      ensureExpand(id.slice("rhead-".length));
      return;
    }
    if (action === "raise") {
      void raisePlanetWindows(id.slice("rraise-".length));
      return;
    }
    if (action === "focusTile") {
      const w = rowById(id.slice("rtile-".length));
      if (!w) return;
      if (w.sleeping) void wakeSleeper(w);
      else void focusRow(w);
      return;
    }
    if (action === "openSnap") {
      void openKanbanFocus(id.slice("rsnap-".length));
      return;
    }
    if (action === "todos") void openPlanetTodos(id.slice("rtodos-".length));
  }

  function syncInspectorRing() {
    if (lane !== "projects") {
      menuRing.setStops([]);
      return;
    }
    const items = spinItems(projects.map((p) => p.id), UNMAPPED_KEY);
    const due = coachIsDue();
    const cIdx = Math.min(Math.max(spinCenter, 0), Math.max(items.length - 1, 0));
    const l2 = layer2For(archFocusPid(arch, items[cIdx]));
    const nested = (arch.front === "layer2" || arch.front === "board") && l2.length > 0;
    const applyArrow = (activeId: string, dir: string) => {
      const result = archArrow(arch, items, spinCenter, dir, railEdge, l2, activeId, due);
      if (!result) return;
      archDo(result.state);
      if (result.stop) menuRing.select(result.stop);
    };
    const stops: RingStop[] = nested
      ? l2.map((item) => ({
          id: item.stop,
          group: "layer2",
          confirm: () => confirmLayer2Stop(item.stop),
          arrow: (dir) => {
            applyArrow(item.stop, dir);
            return true;
          },
        }))
      : spinStops(items, due).map((stop) => ({
          id: stop.id,
          group: stop.group,
          confirm: () => confirmHabitatStop(stop.id),
          arrow: (dir) => {
            applyArrow(stop.id, dir);
            return true;
          },
        }));
    menuRing.setStops(stops);
    if (nested) {
      const active = menuRing.activeId;
      const onL2 = !!active && l2.some((it) => it.stop === active);
      if (!onL2) {
        const want = l2[Math.min(Math.max(arch.center2, 0), l2.length - 1)];
        if (want) menuRing.select(want.stop);
      }
    } else if (arch.front === "arch") {
      const active = menuRing.activeId;
      const onWheel =
        !!active &&
        (items.some((it) => it.stop === active) ||
          active === "rcoach-claim" ||
          active === "rcoach-skip");
      if (active && !onWheel) {
        const back =
          items.find((it) => it.pid != null && it.pid === arch.pid) ??
          items[Math.min(Math.max(spinCenter, 0), Math.max(items.length - 1, 0))];
        if (back) menuRing.select(back.stop);
      }
    }
  }

  function confirmRingTab(id: string) {
    if (id === "ptab-add") {
      creatingProject = true;
      return;
    }
    if (id === "ptab-unclaimed") {
      sheetSlot = "unclaimed";
      return;
    }
    onSheetPlanetClick(id.slice("ptab-".length));
  }

  $effect(() => {
    if (!embedded) {
      syncInspectorRing();
      return;
    }
    // Rows of the current slot first — the tab arrows drop into them.
    const rows: { id: string; confirm: () => void }[] = [];
    if (sheetSlot === "unclaimed") {
      for (const w of unmapped) {
        rows.push({
          id: `prow-${w.id}`,
          confirm: () => {
            claimPickerFor = claimPickerFor === w.id ? null : w.id;
          },
        });
        if (claimPickerFor === w.id) {
          for (const p of projects) {
            rows.push({
              id: `pclaim-${w.id}-${p.id}`,
              confirm: () => sheetClaim(w.id, p.id),
            });
          }
        }
      }
    } else {
      for (const w of sheetRowsFor(sheetSlot)) {
        rows.push({ id: `prow-${w.id}`, confirm: () => sheetFocus(w) });
      }
    }
    const rowIds = rows.map((r) => r.id);
    const slotTabId =
      sheetSlot === "unclaimed" ? "ptab-unclaimed" : `ptab-${sheetSlot}`;

    const stops: RingStop[] = [];
    const tabIds = [
      "ptab-unclaimed",
      ...projects.map((p) => `ptab-${p.id}`),
      "ptab-add",
    ];
    tabIds.forEach((id, i) => {
      stops.push({
        id,
        confirm: () => confirmRingTab(id),
        arrow: (dir) => {
          if (dir === "left" || dir === "right") {
            const n = tabIds.length;
            menuRing.select(tabIds[(i + (dir === "left" ? -1 : 1) + n) % n]);
          } else if (dir === "down" && rowIds.length > 0) {
            menuRing.select(rowIds[0]);
          }
          return true; // the strip eats all four — nothing above it
        },
      });
    });
    rows.forEach((r, i) => {
      stops.push({
        id: r.id,
        confirm: r.confirm,
        arrow: (dir) => {
          if (dir === "up") menuRing.select(i > 0 ? rowIds[i - 1] : slotTabId);
          else if (dir === "down" && i < rowIds.length - 1)
            menuRing.select(rowIds[i + 1]);
          return true; // left/right park on a row
        },
      });
    });
    menuRing.setStops(stops);
  });

  let detachMenuKeys: (() => void) | undefined;

  function toggleTag(windowId: string, tagId: string) {
    const cur =
      liveWindowTags.find((wt) => wt.windowId === windowId)?.tagIds ?? [];
    const next = cur.includes(tagId)
      ? cur.filter((id) => id !== tagId)
      : [...cur, tagId];
    windowTags = [
      ...liveWindowTags.filter((wt) => wt.windowId !== windowId),
      ...(next.length ? [{ windowId, tagIds: next }] : []),
    ];
    persist();
  }

  // What a row shows: the persisted rename when one exists, else the live
  // AX title, else the app name — never blank, never a lonely "—".
  function titleFor(w: WindowInfo): string {
    return (
      titleOverrides.find((o) => o.windowId === w.id)?.title ||
      w.title ||
      w.appName
    );
  }

  function startRename(w: WindowInfo) {
    renamingId = w.id;
    renameValue =
      titleOverrides.find((o) => o.windowId === w.id)?.title ?? w.title;
  }

  // The row menu — Assign to… / Rename /
  // Minimize XOR Restore / Close window / × Unclaim (claimed only).
  let ctxAssignOpen = $state(false);

  function openCtxMenu(e: MouseEvent, w: WindowInfo) {
    e.preventDefault();
    e.stopPropagation();
    // Clamp inside the rail so the menu never opens under the cursor's edge.
    const x = Math.min(e.clientX, window.innerWidth - 170);
    const y = Math.min(e.clientY, window.innerHeight - 220);
    ctxMenu = { x, y, windowId: w.id };
    ctxAssignOpen = false;
    newProjectName = "";
  }

  function ctxWindow(): WindowInfo | undefined {
    return windows.find((w) => w.id === ctxMenu?.windowId);
  }

  function ctxProject(): Project | undefined {
    const id = ctxMenu?.windowId;
    if (!id) return undefined;
    const projectId = liveClaims.find((c) => c.windowId === id)?.projectId;
    return projects.find((p) => p.id === projectId);
  }

  function ctxAssign(projectId: string) {
    const w = ctxWindow();
    ctxMenu = null;
    if (w) claim(w.id, projectId);
  }

  function ctxCreateAndClaim() {
    const w = ctxWindow();
    if (!w || !newProjectName.trim()) return;
    const p = createProject(newProjectName);
    ctxMenu = null;
    claim(w.id, p.id);
    newProjectName = "";
  }

  function ctxRename() {
    const w = ctxWindow();
    ctxMenu = null;
    if (w) startRename(w);
  }

  function ctxMinimize() {
    const w = ctxWindow();
    ctxMenu = null;
    // Claimed → the walker walks its planet; unmapped → Dock only (the
    // habitat snapshot never sees unclaimed windows).
    if (w) stow(w);
  }

  function ctxRestore() {
    const w = ctxWindow();
    ctxMenu = null;
    if (w) restore(w);
  }

  // Close = the AX close button only — never Complete (the circle is the
  // only Done door). Claimed windows become sleepers on the next poll.
  function ctxCloseWindow() {
    const w = ctxWindow();
    ctxMenu = null;
    if (!w) return;
    invoke("close_window", { id: w.id })
      .catch(() => {})
      .then(() => tick());
  }

  // Unclaim ≠ Close: the OS window stays exactly where it is.
  function ctxUnclaim() {
    const w = ctxWindow();
    ctxMenu = null;
    if (w) unclaim(w.id);
  }

  // Empty commit clears the override — the row falls back to the live title.
  function commitRename(w: WindowInfo) {
    const next = renameValue.trim();
    const cur = titleOverrides.find((o) => o.windowId === w.id)?.title ?? "";
    if (next !== cur) {
      titleOverrides = next
        ? [
            ...titleOverrides.filter((o) => o.windowId !== w.id),
            { windowId: w.id, title: next },
          ]
        : titleOverrides.filter((o) => o.windowId !== w.id);
      persist();
    }
    renamingId = null;
  }

  async function setWorkspaceRoot(projectId: string, path: string) {
    try {
      await invoke("set_workspace_root", { projectId, path });
      projects = projects.map((p) =>
        p.id === projectId ? { ...p, workspaceRoot: path.trim() || undefined } : p,
      );
    } catch {
      /* void refuses a root */
    }
  }

  async function persist() {
    if (!stateLoaded) return;
    // A drag-hot store may have skipped state:changed. Don't write it.
    // Drop/claim still mutates memory; drag-end flushes after the hot window.
    if (windowDragHot()) {
      persistDeferred = true;
      return;
    }
    if (pendingDisk) absorbPendingCards();
    persistDeferred = false;
    const state: PersistedState = {
      // Only an explicit choice is saved; an untouched theme stays "" so the
      // next launch keeps following the OS instead of freezing today's pick.
      themeId: themeChosen ? themeId : "",
      windowOpacity,
      projects,
      claims: liveClaims,
      tags,
      windowTags: liveWindowTags,
      collapsedSectionIds,
      nudgeSuppressedWindowIds: nudgeSuppressed,
      kanbanCards,
      cardTombstones,
      pointsLedger,
      titleOverrides,
      autohideRowChrome,
      hideTitles,
      hideAppIcons,
      railBoardCols,
      spriteScale,
      marbleDrift,
      peerChrome,
      planetIdle,
      agentTerminal,
      actionShortcuts,
      agentShortcuts,
      agentRuns,
      neededSplit: neededSplit,
      previewWidth: previewWidth,
      trackerSplit: trackerSplit,
      openSubtaskIds,
      todoTrees,
      treeGrowDays,
      companion: companionState,
      stars: starsState,
      sky: skyState,
      drawers: drawersState,
    };
    try {
      await invoke("save_state", { state, origin: "rail" });
    } catch {
      // next mutation retries; rail stays usable without persistence
    }
    await syncHabitat();
  }

  // Habitat: push a snapshot of claimed projects to the native planet panels.
  // Signature-gated so the 800ms poll only invokes on real change. Tags never
  // reach the habitat — only home claims make sprites. Zombie `gone` claims
  // (21+ days untouched) stay listed in the rail but leave the planet; a
  // project whose claims all went gone loses its (now ghost) planet until
  // something is touched again. Sleepers (closed claimed
  // windows) are the exception — `gone` never evicts a sleeper; it stays on
  // its planet, asleep and dim, until Done/Unclaim.
  let lastHabitatSig = "";
  // One sleeper row from a claim
  // fingerprint — a closed claim keeps a dim, still walker until
  // Done/Unclaim (without it the body outlived its row on the planet:
  // the drag-into-hang ghost). Shared by the live snapshot's orphan
  // claims and by WORLD-FIRST-PAINT's every-claim cheap pass.
  function sleeperRowFor(c: Claim): WindowInfo {
    return {
      id: c.windowId,
      pid: 0,
      appName: c.appName ?? "App",
      title: c.titleAtClaim || c.appName || "",
      bundleId: c.bundleId ?? null,
      stableKey: claimStableKey(c) ?? "",
      iconBase64: null,
      isFrontmost: false,
      isMinimized: true,
      documentPath: c.filePath ?? null,
      sleeping: true,
    };
  }

  // The back-board's ticks — this project's
  // kanban (Needed cards never land), capped ~12: oldest open first,
  // then recent done. Ids + columns only; titles never ride.
  function boardCardsFor(p: Project) {
    // ORPHAN-CARDS: a kind:"window" card ticks only while a live claim
    // carries its windowId — an orphan can't be opened from anywhere, so
    // it must not paint the sign. Loose todos still tick.
    const mine = kanbanCards.filter(
      (k) =>
        (k.projectId ?? "") === p.id &&
        k.kind !== "needed" &&
        (k.kind !== "window" ||
          (!!k.windowId && liveClaims.some((c) => c.windowId === k.windowId))),
    );
    const open = mine
      .filter((k) => k.status !== "done")
      .sort((a, b) => a.createdAt - b.createdAt);
    const done = mine
      .filter((k) => k.status === "done")
      .sort((a, b) => b.lastTouchedAt - a.lastTouchedAt);
    return [...open, ...done].slice(0, 12).map((k) => ({
      id: k.id,
      status: k.status,
      windowId: k.windowId ?? null,
    }));
  }

  function habitatEntryFor(w: WindowInfo, p: Project) {
    const c = liveClaims.find((x) => x.windowId === w.id);
    // The window's board truth rides the snapshot
    // (one channel — sync_habitat). The window-linked card wins for
    // this window id: an open card first, else the latest done one
    // (so the walk-off still shows between Complete and the rail's
    // unclaim). Needed cards never pose as the linked card — their
    // status is a dummy — they only sit the walker via neededState.
    const linked = kanbanCards.filter(
      (k) => k.windowId === w.id && k.kind !== "needed",
    );
    const openCard = linked.find((k) => k.status !== "done");
    const doneCard = openCard
      ? undefined
      : linked
          .filter((k) => k.status === "done")
          .sort((a, b) => b.lastTouchedAt - a.lastTouchedAt)[0];
    const card = openCard ?? doneCard;
    // A Needed (open/later) naming this window — or this project
    // with no window — sits the walker. Needed wins over Doing.
    const needed = kanbanCards.find(
      (k) =>
        k.kind === "needed" &&
        (k.neededState === "open" || k.neededState === "later") &&
        (k.windowId === w.id ||
          (k.windowId == null && (k.projectId ?? "") === p.id)),
    );
    return {
      id: w.id,
      appName: w.appName,
      title: w.title,
      bundleId: w.bundleId,
      minimized: w.isMinimized,
      iconBase64: w.iconBase64,
      age: ageClass(w.id),
      asleep: !!w.sleeping,
      spritePackId: c?.spritePackId ?? "",
      // The generated being + per-walker size ride
      // the snapshot; 0 scale = inherit the gear's global slider.
      charSeed: c?.charSeed ?? 0,
      charLook: c?.charLook ?? null,
      spriteScale: c?.spriteScale ?? 0,
      cardStatus: card?.status ?? "",
      cardKind: card?.kind ?? "",
      neededState: needed ? (needed.neededState ?? "open") : "",
      // Window walkers never bake the droid —
      // the empty seed keeps one entry shape for the sig.
      agentSeed: "",
    };
  }

  // One pseudo-entry per doing-run on this
  // planet — the agent walker. The pose rides the SAME card fields as a
  // window walker (one channel, sync_habitat): working/starting → the
  // disc; a blocked run → sit the near rim; done → the far-rim walk-off
  // (the native side walks it, then the walked-off door below evicts the
  // row). agentSeed marks the entry as the baked droid (bitty sheets
  // stay on window walkers); the native side bakes the atlas once off
  // its hash.
  // the agent walker's neededState is its OWN
  // run's blocked signal, never the planet NeededInbox — an open Needed
  // on this planet must not steal a WORKING droid off the disc. Needed
  // wins over Doing for WINDOW walkers only (the CADENCE sit in
  // habitatEntryFor is unchanged). Blocked still sits; done still exits.
  function runIsSeated(r: AgentRun): boolean {
    return (
      !!(r.agentShortcutId || "").trim() || !!(r.herdr?.agentName || "").trim()
    );
  }

  function goToken(title: string): string | null {
    const m = title.match(/GO-[A-Z0-9]+(?:-[A-Z0-9]+)*/i);
    return m ? m[0].toUpperCase() : null;
  }

  function goNeededOnPlanet(card: KanbanCard | undefined, planetId: string): boolean {
    if (!card) return false;
    const go = goToken(card.title);
    if (!go) return false;
    return kanbanCards.some(
      (c) =>
        c.kind === "needed" &&
        (c.neededState === "open" || c.neededState === "later") &&
        (c.projectId || "") === planetId &&
        ((c.title || "").toUpperCase().includes(go) ||
          (c.neededWhy || "").toUpperCase().includes(go)),
    );
  }

  function agentRowsFor(projectId: string) {
    return agentRuns.filter(
      (r) =>
        r.planetId === projectId &&
        r.state !== "done" &&
        runIsSeated(r) &&
        kanbanCards.some((k) => k.id === r.cardId),
    );
  }

  async function locateAgent(r: AgentRun) {
    const entryId = `agent:${r.cardId}`;
    try {
      await invoke("locate_walker", { entryId });
    } catch {
      /* habitat may be mid-sync */
    }
    try {
      await invoke("inspect_droid", { entryId });
    } catch {
      /* no herdr pane yet */
    }
  }

  function agentEntriesFor(p: Project) {
    return agentRuns
      .filter(
        (r) =>
          r.planetId === p.id &&
          kanbanCards.some((k) => k.id === r.cardId) &&
          (runIsSeated(r) || r.state === "done"),
      )
      .map((r) => {
        const card = kanbanCards.find((k) => k.id === r.cardId);
        const agent = agentShortcuts.find((a) => a.id === r.agentShortcutId);
        const needsYou = goNeededOnPlanet(card, p.id);
        return {
          id: `agent:${r.cardId}`,
          appName: agent?.name ?? "agent",
          title: card?.title ?? "",
          bundleId: null,
          minimized: false,
          iconBase64: null,
          age: "fresh",
          asleep: false,
          spritePackId: "",
          charSeed: 0,
          charLook: null,
          spriteScale: 0,
          cardStatus: r.state === "done" ? "done" : "doing",
          cardKind: "todo",
          // blocked vs needs-you vs working.
          // Window walkers still use open/later for planet Neededs.
          neededState:
            r.state === "blocked" ? "blocked" : needsYou ? "open" : "",
          agentSeed: `${r.planetId}:${r.cardId}`,
        };
      });
  }

  function habitatPlanetFor(p: Project, rows: WindowInfo[]) {
    return {
      projectId: p.id,
      name: p.name,
      collapsed: isCollapsed(p.id),
      boardCards: boardCardsFor(p),
      todoTrees: todoTrees
        .filter((t) => t.projectId === p.id)
        .map((t) => {
          const card = kanbanCards.find((k) => k.id === t.cardId);
          return {
            cardId: t.cardId,
            x: t.x,
            y: t.y,
            plantedAt: t.plantedAt,
            dueAt: card?.dueAt ?? null,
            status: card?.status ?? "todo",
          };
        }),
      entries: [
        ...rows.map((w) => habitatEntryFor(w, p)),
        ...agentEntriesFor(p),
      ],
    };
  }

  function habitatSnapshotPayload(
    planets: ReturnType<typeof habitatPlanetFor>[],
  ) {
    return {
      planets,
      spriteScale,
      marbleDrift,
      allPlanets: projects.map((p) => ({ projectId: p.id, name: p.name })),
      treeGrowDays,
    };
  }

  async function syncHabitat() {
    if (!stateLoaded || !status?.trusted) return;
    const planets = projects
      .map((p) => ({
        p,
        rows: [
          // Live claimed windows — every claim is a walker now (open or
          // minimized); the native side parks only age-`park` zombies.
          ...windows.filter((w) =>
            liveClaims.some(
              (c) =>
                c.windowId === w.id &&
                occupancyOf(c) === p.id &&
                ageClass(w.id) !== "gone",
            ),
          ),
          // Orphan claims (window closed) ride the snapshot as sleeper
          // rows — see sleeperRowFor.
          ...liveClaims
            .filter(
              (c) =>
                occupancyOf(c) === p.id &&
                !windows.some((w) => w.id === c.windowId),
            )
            .map(sleeperRowFor),
        ],
      }))
      // NO empty-planet filter — a mint
      // with zero claims still rides the snapshot (entries: []) so the
      // native side gives it a disc (empty town / marble). Filtering here
      // tore the panel down and the name "vanished" on the dest click.
      .map(({ p, rows }) => habitatPlanetFor(p, rows));
    // Pack ids join the signature: an import/remove changes the frame pool
    // the habitat draws from, so it must re-sync even when no window moved.
    const sig = JSON.stringify([
      planets.map((p) => [
        p.projectId,
        p.collapsed,
        p.boardCards,
        p.todoTrees,
        p.entries.map((e) => [
          e.id,
          e.minimized,
          !!e.iconBase64,
          e.age,
          e.asleep,
          e.spritePackId,
          e.charSeed,
          e.charLook,
          e.spriteScale,
          e.cardStatus,
          e.cardKind,
          e.neededState,
          e.agentSeed ?? "",
        ]),
      ]),
      packs.map((pk) => pk.id),
      spriteScale,
      marbleDrift,
      starterStory.commsBuilt,
      starterStory.starsBuilt,
    ]);
    if (sig === lastHabitatSig) return;
    lastHabitatSig = sig;
    try {
      await invoke("sync_habitat", {
        snapshot: habitatSnapshotPayload(planets),
      });
    } catch {
      lastHabitatSig = ""; // retry next tick
    }
  }

  // Last-known boot: same as a sleeper row but AWAKE so walkers leave
  // the igloo on first blit — not dim/still until AX. Discs still skip
  // list_windows. lastHabitatSig stays untouched so the live tick syncs.
  function lastKnownRowFor(c: Claim): WindowInfo {
    return { ...sleeperRowFor(c), sleeping: false };
  }

  async function syncHabitatLastKnown() {
    if (!stateLoaded || embedded) return;
    const planets = projects
      .map((p) => ({
        p,
        rows: liveClaims
          .filter((c) => occupancyOf(c) === p.id)
          .map(lastKnownRowFor),
      }))
      // PROJECT-ADD leftover: empty planets boot as discs too — same
      // no-filter rule as syncHabitat, so a fresh mint survives restart.
      .map(({ p, rows }) => habitatPlanetFor(p, rows));
    if (planets.length === 0) return;
    try {
      await invoke("sync_habitat", {
        snapshot: habitatSnapshotPayload(planets),
      });
    } catch {
      /* the live tick retries */
    }
  }

  // Row left-click = locate (glow only — never
  // focus, never wake, never the Being editor). Open window → Orbitty
  // overlay ring on its AX frame; minimized claimed → walker stroke on its
  // planet; sleeper → the dim walker if it still has a planet, else a CSS
  // flash on the row. A real drag never reaches here (no click fires).
  let locateFlashIds = $state<string[]>([]);

  function flashRow(windowId: string) {
    if (locateFlashIds.includes(windowId)) return;
    locateFlashIds = [...locateFlashIds, windowId];
    setTimeout(() => {
      locateFlashIds = locateFlashIds.filter((x) => x !== windowId);
    }, 550);
  }

  async function locateRow(w: WindowInfo) {
    const claim = liveClaims.find((c) => c.windowId === w.id);
    const hasPlanet =
      !!claim && projects.some((p) => p.id === claim.projectId);
    if (w.sleeping || w.isMinimized) {
      if (hasPlanet) {
        try {
          await invoke("locate_walker", { entryId: w.id });
        } catch {
          // habitat may be mid-sync; the glow is best-effort
        }
      } else {
        flashRow(w.id);
      }
      return;
    }
    try {
      await invoke("locate_window", { id: w.id });
    } catch {
      flashRow(w.id);
    }
  }

  async function stow(w: WindowInfo) {
    try {
      await invoke("minimize_window", { id: w.id });
    } catch {
      // app may refuse or exit; next tick reflects reality
    }
    await tick();
  }

  async function restore(w: WindowInfo) {
    try {
      await invoke("restore_window", { id: w.id });
    } catch {}
    touchClaim(w.id, true);
    await tick();
  }

  // Click an asleep row → wake through the claim's
  // stamps (live → file → Hermes session → recipe → app). One honest line
  // comes back either way; the click counts as claim activity.
  let wakeNote = $state<{ kind: "ok" | "err"; text: string } | null>(null);

  async function wakeSleeper(w: WindowInfo) {
    wakeNote = null;
    try {
      const msg = await invoke<string>("wake_sleeper", { id: w.id });
      wakeNote = { kind: "ok", text: msg };
    } catch (e) {
      wakeNote = { kind: "err", text: `${e}` };
    }
    touchClaim(w.id, true);
    await tick();
  }

  function nudgeClaim(projectId: string) {
    if (!nudgeFor) return;
    claim(nudgeFor, projectId);
    nudgeFor = null;
  }

  function nudgeCreate() {
    if (!nudgeFor || !nudgeNewName.trim()) return;
    const p = createProject(nudgeNewName);
    claim(nudgeFor, p.id);
    nudgeFor = null;
    nudgeNewName = "";
  }

  function clampSpriteScale(n: number) {
    if (!Number.isFinite(n)) return 1;
    return Math.min(2.5, Math.max(0.4, n));
  }

  // Board and STARS share the one 780×520 `kanban`
  // overlay — the native side persists the mode, shows the pane, and emits
  // the swap; this webview (when it IS the pane) mounts KanbanPage.
  async function openKanban() {
    try {
      await invoke("open_kanban");
    } catch {
      // window may already exist mid-creation; next click retries
    }
  }

  // The rail no longer opens the Sky window — the
  // word field lives inside STARS Review. The sky webview itself stays
  // (hide-not-destroy).

  // Home button: order every planet panel front (peer order preserved
  // native-side; other apps keep their relative stacking).
  async function bringAllFront() {
    try {
      await invoke("bring_all_front");
    } catch {
      // no planets yet — harmless
    }
  }

  // One drag source: JS startDragging on the banner. Overlay's ~28px
  // native strip is covered by this chrome, so it cannot be the handle.
  // Dual-drag (JS + data-tauri-drag-region) used to jump the rail off
  // the click — do not put the region back. × stays click-only.
  // Wordmark: click = Home, drag past 5px = move.
  let wordmarkDidDrag = false;

  function onBannerMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    const t = e.target as HTMLElement | null;
    if (t?.closest(".winx, .popout")) return;

    if (t?.closest(".wordmark")) {
      wordmarkDidDrag = false;
      const sx = e.screenX;
      const sy = e.screenY;
      const onMove = (ev: MouseEvent) => {
        if (Math.hypot(ev.screenX - sx, ev.screenY - sy) < 5) return;
        wordmarkDidDrag = true;
        window.removeEventListener("mousemove", onMove);
        startPaneDrag(e);
      };
      const onUp = () => {
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
      };
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
      return;
    }

    startPaneDrag(e);
  }

  function onWordmarkClick() {
    if (wordmarkDidDrag) return;
    bringAllFront();
  }

  // Open Lists focused on one card: open_kanban puts the shared pane in
  // board mode (KanbanPage), which listens for `orbitty:focus-card`,
  // selects the card's project and opens its detail. The delayed re-emit
  // covers a kanban webview that is still mid-load (or mid page-swap) when
  // the first event lands.
  async function openKanbanFocus(cardId: string) {
    await openKanban();
    emitTo("kanban", "orbitty:focus-card", cardId).catch(() => {});
    setTimeout(
      () => emitTo("kanban", "orbitty:focus-card", cardId).catch(() => {}),
      400,
    );
  }

  // S5 Hermes: launch/focus the EXISTING harness with a titles-only context
  // file (claimed window titles + open to-do titles — never file contents).
  async function launchHermes(p: Project) {
    const titles = [
      ...rowsFor(p.id).map((w) =>
        w.title ? `${w.appName} — ${w.title}` : w.appName,
      ),
      ...todosFor(p.id).map((c) => `To-do: ${c.title}`),
    ];
    try {
      const route = await invoke<string>("launch_coordinator", {
        projectId: p.id,
        projectName: p.name,
        titles,
      });
      hermesNote = { kind: "ok", text: `${p.name}: ${route}` };
    } catch (e) {
      hermesNote = { kind: "err", text: `${e}` };
    }
  }

  // S5 soul.md: read always; writes only through the explicit Save button.
  async function toggleSoul(p: Project) {
    if (soulFor === p.id) {
      soulFor = null;
      return;
    }
    soulFor = p.id;
    soulBody = "";
    soulPath = "";
    soulMsg = "Loading…";
    try {
      soulBody = await invoke<string>("load_soul", {
        projectId: p.id,
        projectName: p.name,
      });
      soulPath = await invoke<string>("soul_path", { projectId: p.id });
      soulMsg = "";
    } catch (e) {
      soulMsg = `${e}`;
    }
  }

  async function saveSoul() {
    if (!soulFor) return;
    try {
      await invoke("save_soul", { projectId: soulFor, body: soulBody });
      soulMsg = "Saved.";
      setTimeout(() => {
        if (soulMsg === "Saved.") soulMsg = "";
      }, 1500);
    } catch (e) {
      soulMsg = `${e}`;
    }
  }

  async function openSoulEditor() {
    if (!soulFor) return;
    const p = projects.find((x) => x.id === soulFor);
    try {
      await invoke("open_soul_in_editor", {
        projectId: soulFor,
        projectName: p?.name ?? null,
      });
    } catch (e) {
      soulMsg = `${e}`;
    }
  }

  function createProject(name: string): Project {
    const p: Project = {
      id: `p-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
      name: name.trim(),
    };
    projects = [...projects, p];
    // No walker yet: a marble, not an open planet. A claim or a todo opens it.
    if (!collapsedSectionIds.includes(p.id)) {
      collapsedSectionIds = [...collapsedSectionIds, p.id];
    }
    return p;
  }

  function revealPlanet(projectId: string) {
    const ids = sectionIds();
    if (!ids.includes(projectId)) return;
    collapsedSectionIds = [
      ...collapsedSectionIds.filter((id) => !ids.includes(id)),
      ...ids.filter((id) => id !== projectId),
    ];
  }

  function newCardId(): string {
    return `c-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
  }

  function isClinicalOrMaster(windowId: string): boolean {
    return tagsFor(windowId).some((t) => {
      const n = t.name.trim().toLowerCase();
      return n === "clinical" || n === "master";
    });
  }

  // Reboot-rematch fingerprint: the 1-based position of this window among
  // live windows sharing its bundle+title — the same `n` the native
  // stableKey carries. Stamped from the live AX title, never the override.
  function titleIndexFor(w: WindowInfo): number {
    const bundle = w.bundleId || w.appName;
    return (
      windows
        .filter(
          (x) => (x.bundleId || x.appName) === bundle && x.title === w.title,
        )
        .findIndex((x) => x.id === w.id) + 1
    );
  }

  function claimStableKey(c: Claim): string | null {
    const bundle = c.bundleId || c.appName;
    if (!bundle || c.titleAtClaim == null || !c.titleIndex) return null;
    return `${bundle}|${c.titleAtClaim}|${c.titleIndex}`;
  }

  // Hermes-class = the bundle id or app name says so.
  function isHermesClass(w: WindowInfo): boolean {
    return /hermes/i.test(w.bundleId ?? "") || /hermes/i.test(w.appName);
  }

  // Wake-trail stamps, taken from the LIVE window only (never invented):
  // the AX document path, and the Hermes session hint from the AX title.
  // Re-claiming keeps whatever a previous claim stamped (prev).
  function sleeperStamps(w: WindowInfo, prev?: Claim): Partial<Claim> {
    return {
      filePath: w.documentPath ?? prev?.filePath ?? null,
      hermesProfile: prev?.hermesProfile ?? null,
      hermesSession:
        isHermesClass(w) && w.title
          ? w.title
          : (prev?.hermesSession ?? null),
      actionId: prev?.actionId ?? null,
      command: prev?.command ?? null,
      cwd: prev?.cwd ?? null,
    };
  }

  // A fresh roll seed for a brand-new claim (u32 range —
  // the native side takes u64). The look derives from it until the Being
  // editor materializes explicit picks.
  function rollCharSeed(): number {
    return Math.floor(Math.random() * 0xffffffff) >>> 0;
  }

  // Claim lands as Todo — walker comes from the igloo and stands at the
  // board. Doing (field walk) is an explicit column move, not the claim.
  function claim(windowId: string, projectId: string) {
    const now = nowEpoch();
    const w = windows.find((x) => x.id === windowId);
    // The sprite pick / generated being / per-walker size are per-window
    // preferences — re-claiming into another project keeps them; a
    // first-time claim rolls a fresh person.
    const prev = liveClaims.find((c) => c.windowId === windowId);
    claims = [
      ...liveClaims.filter((c) => c.windowId !== windowId),
      {
        windowId,
        projectId,
        lastTouchedAt: now,
        spritePackId: prev?.spritePackId ?? null,
        charSeed: prev?.charSeed ?? rollCharSeed(),
        charLook: prev?.charLook ?? null,
        spriteScale: prev?.spriteScale ?? null,
        // Claiming (rail Assign or walker
        // Assign-to-planet — same door) clears any leftover teleport
        // home. Occupancy is projectId only.
        walkerHome: null,
        ...(w
          ? {
              bundleId: w.bundleId ?? null,
              appName: w.appName,
              titleAtClaim: w.title,
              titleIndex: titleIndexFor(w),
              ...sleeperStamps(w, prev),
            }
          : {}),
      },
    ];
    revealPlanet(projectId);
    // Bare claim = grass only. A board card is born only when a todo is
    // assigned to this window. If a non-done card already names it, keep
    // the link and retarget the project — never force status back to todo.
    const shown = w
      ? titleOverrides.find((o) => o.windowId === w.id)?.title || w.title
      : "";
    const title = w ? (shown ? `${w.appName} — ${shown}` : w.appName) : "Window";
    const open = kanbanCards.find(
      (c) => c.windowId === windowId && c.status !== "done",
    );
    if (open) {
      kanbanCards = kanbanCards.map((c) =>
        c.id === open.id
          ? {
              ...c,
              projectId,
              windowId,
              title: c.title || title,
              clinicalOrMaster: c.clinicalOrMaster || isClinicalOrMaster(windowId),
              lastTouchedAt: now,
            }
          : c,
      );
    }
    persist();
  }

  // Unclaim never deletes the card: it stays on the project's list as a
  // loose todo without a window.
  function unclaim(windowId: string) {
    claims = liveClaims.filter((c) => c.windowId !== windowId);
    const now = nowEpoch();
    kanbanCards = kanbanCards.map((c) =>
      c.windowId === windowId && c.status !== "done"
        ? { ...c, windowId: null, status: "todo", lastTouchedAt: now }
        : c,
    );
    persist();
  }

  // S4 complete — the only door that retires a sprite and can mint points.
  // Explicit click + confirm on a home-claimed row; never from dwell,
  // minimize, or title heuristics. Also AX-closes the OS window (its own
  // close button, never a pid kill); if the app refuses, the card still
  // completes — no panic.
  async function completeWindow(w: WindowInfo, project: Project) {
    const now = nowEpoch();
    const clinicalOrMaster = isClinicalOrMaster(w.id);
    const shown = titleOverrides.find((o) => o.windowId === w.id)?.title || w.title;
    const title = shown ? `${w.appName} — ${shown}` : w.appName;
    const existing =
      kanbanCards.find(
        (c) => c.windowId === w.id && c.projectId === project.id,
      ) ?? kanbanCards.find((c) => c.windowId === w.id);
    // Board walk-off only when a real todo is bound (kind todo, or any
    // non-window kind with a windowId). Bare claim / legacy kind:window
    // tickets: close + unclaim, no Done parade to the sign.
    const realTodo =
      !!existing &&
      existing.kind !== "window" &&
      existing.kind !== "needed" &&
      existing.status !== "done";
    if (!realTodo) {
      // Bare claim exit: kind:window Done keeps the claim so the walker
      // can walk home/off (native skips the board lip for non-todo).
      // No instant vanish; no sign checkout.
      if (existing) {
        kanbanCards = kanbanCards.map((c) =>
          c.id === existing.id
            ? {
                ...c,
                kind: c.kind === "todo" ? c.kind : ("window" as const),
                status: "done",
                windowId: w.id,
                title: c.title || title,
                lastTouchedAt: now,
                completedAt: c.completedAt ?? now,
              }
            : c,
        );
      } else {
        kanbanCards = [
          ...kanbanCards,
          {
            id: newCardId(),
            title,
            projectId: project.id,
            windowId: w.id,
            kind: "window" as const,
            status: "done" as const,
            createdAt: now,
            lastTouchedAt: now,
            completedAt: now,
            goalAligned: true,
            clinicalOrMaster,
            pointsAwarded: 0,
            starred: false,
          },
        ];
      }
      persist();
      try {
        await invoke("close_window", { id: w.id });
      } catch {}
      await tick();
      return;
    }
    // Award once per card (first transition to done only); 1 pt default,
    // 2 if tagged clinical/master; 0 when the daily cap would be exceeded —
    // the card still completes, quietly.
    const firstComplete = !existing?.completedAt;
    const base = clinicalOrMaster ? 2 : 1;
    const award = firstComplete && pointsToday() + base <= DAILY_CAP ? base : 0;
    const cardId = existing!.id;
    kanbanCards = kanbanCards.map((c) =>
      c.id === cardId
        ? {
            ...c,
            status: "done",
            title: c.title || title,
            clinicalOrMaster: c.clinicalOrMaster || clinicalOrMaster,
            lastTouchedAt: now,
            completedAt: c.completedAt ?? now,
            pointsAwarded: c.pointsAwarded + award,
          }
        : c,
    );
    if (award > 0) {
      pointsLedger = [
        ...pointsLedger,
        {
          id: `pt-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
          cardId,
          amount: award,
          day: todayKey(),
          at: now,
        },
      ];
    }
    // Real todo: claim SURVIVES so the walker can walk to the board then
    // off — habitat:walked-off unclaims once the walker is gone.
    persist();
    try {
      await invoke("close_window", { id: w.id });
    } catch {}
    await tick();
  }

  // Completing a free todo from a rail project section: same points rules
  // as a window card (award once per card, daily cap), minus the walk-off.
  function completeTodo(card: KanbanCard) {
    const now = nowEpoch();
    const alreadyAwarded = pointsLedger.some((e) => e.cardId === card.id);
    const base = card.clinicalOrMaster ? 2 : 1;
    const award =
      !alreadyAwarded && pointsToday() + base <= DAILY_CAP ? base : 0;
    kanbanCards = kanbanCards.map((c) =>
      c.id === card.id
        ? {
            ...c,
            status: "done",
            lastTouchedAt: now,
            completedAt: now,
            pointsAwarded: c.pointsAwarded + award,
          }
        : c,
    );
    if (award > 0) {
      pointsLedger = [
        ...pointsLedger,
        {
          id: `pt-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
          cardId: card.id,
          amount: award,
          day: todayKey(),
          at: now,
        },
      ];
    }
    persist();
  }

  // S4.6: one click on the open circle fills it (the fill is the confirm),
  // then the row completes and leaves the rail.
  function circleComplete(key: string, fn: () => void) {
    if (completingIds.includes(key)) return;
    completingIds = [...completingIds, key];
    setTimeout(() => {
      completingIds = completingIds.filter((k) => k !== key);
      fn();
    }, 240);
  }

  // A loose-todo mint is a kind-420 board proof
  // (`t=orbitty`) — the log_event match arm kinds "board-add". Fire-and-
  // forget in the same tick as the save: a log failure never blocks a mint.
  function logEvent(eType: string, content: Record<string, unknown>, projectId?: string) {
    invoke("log_event", {
      eType,
      content,
      projectId: projectId || null,
      actor: "user",
    }).catch(() => {});
  }

  // "+ To-do" under a project section: a loose todo card on that project.
  // Creating one opens Lists focused on the new card.
  function addTodoTo(projectId: string) {
    const title = newTodoTitle.trim();
    if (!title) return;
    const now = nowEpoch();
    const id = newCardId();
    kanbanCards = [
      ...kanbanCards,
      {
        id,
        title,
        projectId,
        windowId: null,
        kind: "todo" as const,
        status: "todo" as const,
        createdAt: now,
        lastTouchedAt: now,
        completedAt: null,
        goalAligned: true,
        clinicalOrMaster: false,
        pointsAwarded: 0,
        starred: false,
      },
    ];
    newTodoTitle = "";
    addingTodoFor = null;
    revealPlanet(projectId);
    persist();
    logEvent("board-add", { cardId: id, title }, projectId);
    openKanbanFocus(id);
  }

  // Sprite assignment: the pack id lives on the
  // claim and rides the habitat snapshot; null/"" = the generated being.
  // The picker UI is the planet walker's right-click NSMenu — the rail
  // receives `habitat:walker-pack` and writes the claim here.
  // One windowId, one projectId — occupancy IS the
  // claim. The walkerHome occupancy teleport is gone; the field stays on
  // Claim only so old saves still load, and nothing reads it.
  function occupancyOf(c: Claim): string {
    return c.projectId;
  }

  function setSpritePack(windowId: string, packId: string | null) {
    claims = liveClaims.map((c) =>
      c.windowId === windowId ? { ...c, spritePackId: packId || null } : c,
    );
    persist();
  }

  // Being editor
  // The door is the planet walker's right-click menu → "Open being…"
  // (habitat:being). Every change writes the
  // claim straight away (persist → syncHabitat), so the planet redraws
  // live; there is no Save/Cancel. Double-click on the walker still
  // restores the window.

  let beingEditorFor = $state<string | null>(null);
  let bittyKinds = $state<BittyKind[]>([]);
  let editorLook = $state<CharLook | null>(null);
  let editorPreview = $state("");

  function beingClaim(): Claim | undefined {
    return beingEditorFor
      ? liveClaims.find((c) => c.windowId === beingEditorFor)
      : undefined;
  }

  function beingWindow(): WindowInfo | undefined {
    return beingEditorFor
      ? windows.find((w) => w.id === beingEditorFor)
      : undefined;
  }

  async function refreshBeingPreview() {
    if (!editorLook) {
      editorPreview = "";
      return;
    }
    try {
      const b64 = await invoke<string>("char_preview", {
        look: editorLook,
        px: 8,
      });
      editorPreview = `data:image/png;base64,${b64}`;
    } catch {
      editorPreview = ""; // preview is decorative; the planet is the truth
    }
  }

  // Write the draft look (+ seed) onto the claim and redraw everywhere.
  async function commitBeingLook(seed?: number) {
    const id = beingEditorFor;
    if (!id || !editorLook) return;
    claims = liveClaims.map((c) =>
      c.windowId === id
        ? { ...c, charLook: { ...editorLook! }, ...(seed != null ? { charSeed: seed } : {}) }
        : c,
    );
    persist();
    await refreshBeingPreview();
  }

  async function openBeingEditor(windowId: string) {
    const c = liveClaims.find((x) => x.windowId === windowId);
    if (!c) return; // only claimed windows have beings
    if (bittyKinds.length === 0) {
      try {
        bittyKinds = await invoke<BittyKind[]>("bitty_catalog");
      } catch {
        bittyKinds = [];
      }
    }
    // Materialize: a claim without explicit picks derives them from its
    // seed once, so the cyclers edit concrete indices from then on.
    let seed = c.charSeed ?? 0;
    if (!seed) {
      seed = rollCharSeed();
      claims = liveClaims.map((x) =>
        x.windowId === windowId ? { ...x, charSeed: seed } : x,
      );
    }
    let look = c.charLook;
    if (!look) {
      try {
        look = await invoke<CharLook>("roll_char_look", { seed });
      } catch {
        look = { species: 0, body: 0, clothes: 0, eyes: 0, mouth: 0, hat: 0, colors: 0, kind: 0, palette: 0 };
      }
    }
    // Pre-S1 looks carry no kind/palette — the native side maps the legacy
    // species onto the nearest Kind+Palette so the cyclers start concrete.
    try {
      look = await invoke<CharLook>("normalize_char_look", { look });
    } catch {
      // keep the un-normalized look; the planet still resolves it
    }
    beingEditorFor = windowId;
    editorLook = { ...look };
    await commitBeingLook();
  }

  function closeBeingEditor() {
    beingEditorFor = null;
    editorLook = null;
    editorPreview = "";
  }

  // Kind + Palette are the only cyclers. Indices
  // wrap mod the table sizes; the palette index carries across kinds (every
  // kind has exactly 4 palettes).
  function beingKindIndex(): number {
    return editorLook?.kind ?? 0;
  }

  function beingPaletteIndex(): number {
    return editorLook?.palette ?? 0;
  }

  function cycleKind(dir: 1 | -1) {
    if (!editorLook || bittyKinds.length === 0) return;
    const n = bittyKinds.length;
    const k = (((beingKindIndex() + dir) % n) + n) % n;
    const p = beingPaletteIndex() % (bittyKinds[k]?.palettes.length || 1);
    editorLook = { ...editorLook, kind: k, palette: p };
    commitBeingLook();
  }

  function cyclePalette(dir: 1 | -1) {
    if (!editorLook || bittyKinds.length === 0) return;
    const n = bittyKinds[beingKindIndex()]?.palettes.length || 1;
    const p = (((beingPaletteIndex() + dir) % n) + n) % n;
    editorLook = { ...editorLook, palette: p };
    commitBeingLook();
  }

  function kindLabel(): string {
    return bittyKinds[beingKindIndex()]?.name ?? "";
  }

  function paletteLabel(): string {
    return bittyKinds[beingKindIndex()]?.palettes[beingPaletteIndex()] ?? "";
  }

  async function rerollBeing() {
    const seed = rollCharSeed();
    try {
      editorLook = await invoke<CharLook>("roll_char_look", { seed });
    } catch {
      return;
    }
    await commitBeingLook(seed);
  }

  // Per-walker size: null = follow the gear's global slider.
  function beingScale(): number {
    return beingClaim()?.spriteScale ?? spriteScale;
  }

  function onBeingScaleInput(e: Event) {
    const id = beingEditorFor;
    if (!id) return;
    const v = clampSpriteScale(+(e.target as HTMLInputElement).value);
    claims = liveClaims.map((c) =>
      c.windowId === id ? { ...c, spriteScale: v } : c,
    );
    lastHabitatSig = "";
    syncHabitat();
  }

  function onBeingScaleCommit() {
    persist();
  }

  function resetBeingScale() {
    const id = beingEditorFor;
    if (!id) return;
    claims = liveClaims.map((c) =>
      c.windowId === id ? { ...c, spriteScale: null } : c,
    );
    persist();
  }

  function beingMoveHome(projectId: string) {
    const id = beingEditorFor;
    if (!id) return;
    claim(id, projectId);
  }

  function beingUnclaim() {
    const id = beingEditorFor;
    if (!id) return;
    unclaim(id);
    closeBeingEditor();
  }

  async function createProjectOnly() {
    if (!newProjectName.trim()) return;
    const p = createProject(newProjectName);
    newProjectName = "";
    creatingProject = false;
    if (embedded) sheetSlot = p.id;
    // Marble is already in the snapshot. Lift it so it is visible when
    // planets are not muted. No locate glow — that pops the name.
    await persist();
    invoke("bring_planet_front", { id: p.id }).catch(() => {});
    setTimeout(() => invoke("bring_planet_front", { id: p.id }).catch(() => {}), 50);
  }

  // the × on a planet
  // row arms an in-sheet confirm — WKWebView JS dialogs (confirm/alert) do
  // not surface on the companion overlay window, so window.confirm swallowed
  // the press and window.alert had the same hole (S4.6 law: the confirm
  // lives in the webview). First × arms "Remove …?" with Yes / No; Yes
  // invokes, No or a click away disarms. One planet armed at a time. The
  // native command is the one save door (same as ctl) — cards keep their
  // projectId, walkers vanish with the disc on the next habitat sync. The
  // local list only mirrors that disk write; no persist() here. An invoke
  // error stays in-sheet on that row and the row stays.
  let rmArmedFor = $state<string | null>(null);
  let rmErrorFor = $state<{ id: string; msg: string } | null>(null);

  function armRemove(id: string) {
    rmArmedFor = id;
    rmErrorFor = null;
  }

  // Click-away disarm: capture on document so a row's stopPropagation can't
  // hide the press; a press inside the armed chip IS the confirm, not away.
  $effect(() => {
    if (!rmArmedFor) return;
    const onDocDown = (e: PointerEvent) => {
      const t = e.target as HTMLElement | null;
      if (t?.closest(".planet-rm-confirm")) return;
      rmArmedFor = null;
    };
    document.addEventListener("pointerdown", onDocDown, true);
    return () => document.removeEventListener("pointerdown", onDocDown, true);
  });

  async function removeProject(p: Project) {
    try {
      await invoke("project_rm", { id: p.id });
    } catch (e) {
      rmArmedFor = null;
      rmErrorFor = { id: p.id, msg: `${e}` };
      return;
    }
    rmArmedFor = null;
    rmErrorFor = null;
    projects = projects.filter((x) => x.id !== p.id);
    if (sheetSlot === p.id) sheetSlot = "unclaimed";
    syncHabitat();
  }

  function onDragStart(e: DragEvent, w: WindowInfo) {
    dragWindowId = w.id;
    dragStartedAt = Date.now();
    ctxMenu = null;
    e.dataTransfer?.setData("text/plain", w.id);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }

  function onDragEnd() {
    dragWindowId = null;
    dropTarget = null;
    if (!pendingDisk && !persistDeferred) return;
    const wait = Math.max(0, 420 - (Date.now() - dragStartedAt));
    setTimeout(() => {
      if (windowDragHot()) return;
      if (pendingDisk) absorbPendingCards();
      if (persistDeferred) persist();
    }, wait);
  }

  // Window drags must keep working even if Svelte state was lost mid-gesture
  // (the 800ms poll can mutate the source row and cancel the HTML5 session):
  // trust the dataTransfer type list, not just dragWindowId.
  function isWindowDrag(e: DragEvent): boolean {
    return (
      !!dragWindowId || !!e.dataTransfer?.types?.includes("text/plain")
    );
  }

  function onSectionDragEnter(e: DragEvent, key: string) {
    if (!isWindowDrag(e)) return;
    e.preventDefault();
    dropTarget = key;
  }

  function onSectionDragOver(e: DragEvent, key: string) {
    if (!isWindowDrag(e)) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    if (dropTarget !== key) dropTarget = key;
  }

  function onSectionDragLeave(e: DragEvent, key: string) {
    // dragleave fires when moving between children; only clear when the
    // pointer actually leaves the section.
    const next = e.relatedTarget as Node | null;
    if (
      next &&
      e.currentTarget instanceof HTMLElement &&
      e.currentTarget.contains(next)
    )
      return;
    if (dropTarget === key) dropTarget = null;
  }

  function onSectionDrop(e: DragEvent, projectId: string | null) {
    e.preventDefault();
    // dataTransfer is authoritative inside drop; Svelte state is the fallback
    // in case the 800ms poll tore down and rebuilt the source row mid-gesture.
    const id = e.dataTransfer?.getData("text/plain") || dragWindowId;
    if (id) {
      if (projectId) {
        claim(id, projectId);
      } else {
        unclaim(id);
      }
    }
    dragWindowId = null;
    dropTarget = null;
  }

  async function tick() {
    // Frozen while a window drag is hot: no AX status, no list_windows, no
    // syncHabitat — mutating the dragged row's DOM cancels the HTML5 drag
    // session in WKWebView, and the sync is what beach-balled the drop.
    if (windowDragHot()) return;
    try {
      const s = await invoke<AxStatus>("accessibility_status");
      noteAx(s);
      // Don't swap the rows array mid-drag: mutating the dragged row's DOM
      // can cancel the HTML5 drag session in WKWebView.
      if (s.trusted && !dragWindowId) {
        windows = await invoke<WindowInfo[]>("list_windows");
        refreshedAt = new Date().toLocaleTimeString();
        windowsPolled = true;
        // INSPECTOR-HOST: habitat sync / rematch stay on the main rail
        // instance so two mounts don't double-write. Embedded still lists
        // windows so the well stays live.
        if (embedded) return;
        rematchClaims();
        // ORPHAN-CARDS: heal right after rematch — windows is fresh, so
        // live orphans re-claim and dead ones demote on this same pass.
        if (healOrphanWindowCards()) persist();
        // Frontmost tick: a claimed window the human is actively in counts as
        // touched (throttled persist inside), so zombies self-heal by use.
        const front = windows.find((w) => w.isFrontmost);
        if (front && liveClaims.some((c) => c.windowId === front.id)) {
          touchClaim(front.id);
        }
        checkNudge();
        syncHabitat();
      }
    } catch {
      // keep last good state; next poll retries
    }
  }

  // Reboot rematch. Live ids are pid:cgWindowID —
  // they die on reboot. Claims carry a bundle+title fingerprint; when a
  // claim's id is gone, adopt the best unused live window (exact stableKey,
  // else same bundle+title, else same documentPath, else the sole unused
  // window of that bundle — title drifts on browsers/wallets/Finder) and
  // rewrite the id everywhere. Two claims never merge onto one window;
  // unmatched claims stay listed as orphans until their window returns —
  // nothing is deleted.
  function rematchClaims() {
    if (!stateLoaded) return;
    const liveIds = new Set(windows.map((w) => w.id));
    const used = new Set<string>();
    let changed = false;
    // Live claims keep their ids; backfill the fingerprint on pre-GO claims
    // so they survive the NEXT reboot too. While a claim is live, refresh
    // its sleeper wake trail (file path / Hermes session) from the window.
    for (const c of liveClaims) {
      if (!liveIds.has(c.windowId)) continue;
      used.add(c.windowId);
      const w = windows.find((x) => x.id === c.windowId);
      if (!w) continue;
      if (c.titleAtClaim == null) {
        c.bundleId = w.bundleId ?? null;
        c.appName = w.appName;
        c.titleAtClaim = w.title;
        c.titleIndex = titleIndexFor(w);
        changed = true;
      }
      if (w.documentPath && c.filePath !== w.documentPath) {
        c.filePath = w.documentPath;
        changed = true;
      }
      if (isHermesClass(w) && w.title && c.hermesSession !== w.title) {
        c.hermesSession = w.title;
        changed = true;
      }
    }
    for (const c of liveClaims) {
      if (liveIds.has(c.windowId)) continue;
      const key = claimStableKey(c);
      let pick: WindowInfo | undefined;
      if (key) pick = windows.find((w) => !used.has(w.id) && w.stableKey === key);
      if (!pick && (c.bundleId || c.appName) && c.titleAtClaim != null) {
        const bundle = c.bundleId || c.appName;
        pick = windows.find(
          (w) =>
            !used.has(w.id) &&
            (w.bundleId || w.appName) === bundle &&
            w.title === c.titleAtClaim,
        );
      }
      // Document path (Preview / editors): title may drift; the stamped
      // file is stronger than the AX title.
      if (!pick && c.filePath) {
        pick = windows.find(
          (w) => !used.has(w.id) && w.documentPath === c.filePath,
        );
      }
      // Soft rematch: title often drifts (browser nav, wallet rename,
      // Finder folder). If exactly one unused live window shares this
      // claim's bundle, adopt it — otherwise leave the orphan (ambiguous).
      if (!pick) {
        const bundle = c.bundleId || c.appName;
        if (bundle) {
          const same = windows.filter(
            (w) => !used.has(w.id) && (w.bundleId || w.appName) === bundle,
          );
          if (same.length === 1) pick = same[0];
        }
      }
      if (!pick) continue; // orphan — waits for its window to return
      used.add(pick.id);
      rewriteWindowId(c.windowId, pick);
      changed = true;
    }
    if (changed) persist();
  }

  // One id swap, all slices: claim (with a refreshed fingerprint and wake
  // trail), cards, tags, title overrides, nudge suppression.
  function rewriteWindowId(oldId: string, w: WindowInfo) {
    // Rematch / sleeper wake is claim activity — refresh lastTouched so
    // zombie fade dim clears when the walker comes back live.
    const touched = nowEpoch();
    claims = claims.map((c) =>
      c.windowId === oldId
        ? {
            ...c,
            windowId: w.id,
            bundleId: w.bundleId ?? null,
            appName: w.appName,
            titleAtClaim: w.title,
            titleIndex: titleIndexFor(w),
            filePath: w.documentPath ?? c.filePath ?? null,
            hermesSession:
              isHermesClass(w) && w.title ? w.title : (c.hermesSession ?? null),
            lastTouchedAt: touched,
          }
        : c,
    );
    kanbanCards = kanbanCards.map((c) =>
      c.windowId === oldId ? { ...c, windowId: w.id } : c,
    );
    windowTags = windowTags.map((wt) =>
      wt.windowId === oldId ? { ...wt, windowId: w.id } : wt,
    );
    titleOverrides = titleOverrides.map((o) =>
      o.windowId === oldId ? { ...o, windowId: w.id } : o,
    );
    nudgeSuppressed = nudgeSuppressed.map((id) => (id === oldId ? w.id : id));
  }

  // orphan window-card heal. An open
  // kind:"window" card with a windowId but no live claim is invisible
  // everywhere except the billboard tick — todosFor skips windowId cards,
  // Launchpad hides kind:"window", the habitat only draws claims. Heal:
  // a LIVE window gets its claim re-minted onto the card's project through
  // claim() (the one door — it adopts the existing card, never mints a
  // second; the walker appears on the planet and Unclaimed loses the row);
  // a GONE window demotes the card to a loose todo so the rail lists it
  // and it stays completable. Demotion waits for the first real window
  // poll — at load_state time every window looks gone.
  let windowsPolled = false;

  function healOrphanWindowCards(): boolean {
    if (!stateLoaded) return false;
    let changed = false;
    const orphans = kanbanCards.filter(
      (k) =>
        k.kind === "window" &&
        k.status !== "done" &&
        !!k.windowId &&
        !liveClaims.some((c) => c.windowId === k.windowId),
    );
    for (const card of orphans) {
      const w = windows.find((x) => x.id === card.windowId);
      if (w && projects.some((p) => p.id === card.projectId)) {
        claim(w.id, card.projectId); // the one door; persists itself
        changed = true;
      } else if (!w && windowsPolled) {
        const now = nowEpoch();
        kanbanCards = kanbanCards.map((k) =>
          k.id === card.id
            ? { ...k, kind: "todo" as const, windowId: null, lastTouchedAt: now }
            : k,
        );
        changed = true;
      }
    }
    return changed;
  }

  // Claim-first nudge: an OS minimize (yellow / ⌘M / Dock) of an unmapped
  // window asks once — pick or create a home, or dismiss. After that the
  // window id is silent forever (persisted). Never auto-claim.
  //
  // Windows ALREADY minimized on the first poll after
  // launch never nudge — their ids AND stableKeys go straight to
  // nudgeSuppressed (the stableKey keeps them silent across reboots too).
  // After that, only a visible → minimized transition can nudge, once per id.
  let nudgePrimed = false;
  let prevMinimized = new Set<string>();

  function checkNudge() {
    if (!stateLoaded) return;
    const minimizedNow = new Set(
      windows.filter((w) => w.isMinimized).map((w) => w.id),
    );
    if (!nudgePrimed) {
      nudgePrimed = true;
      prevMinimized = minimizedNow;
      const alreadyMin = windows.filter((w) => w.isMinimized);
      if (alreadyMin.length > 0) {
        const extra = alreadyMin.flatMap((w) => [w.id, w.stableKey]);
        nudgeSuppressed = [...new Set([...nudgeSuppressed, ...extra])];
        persist();
      }
      return; // first poll never nudges
    }
    if (nudgeFor && !minimizedNow.has(nudgeFor)) {
      nudgeFor = null; // restored or gone before acting on the banner
    }
    if (!nudgeFor) {
      const candidate = windows.find(
        (w) =>
          w.isMinimized &&
          !prevMinimized.has(w.id) && // a real visible → minimized transition
          !liveClaims.some((c) => c.windowId === w.id) &&
          !nudgeSuppressed.includes(w.id) &&
          !nudgeSuppressed.includes(w.stableKey),
      );
      if (candidate) {
        nudgeSuppressed = [
          ...new Set([...nudgeSuppressed, candidate.id, candidate.stableKey]),
        ];
        nudgeFor = candidate.id;
        nudgeNewName = "";
        persist();
      }
    }
    prevMinimized = minimizedNow;
  }

  // Rail drag lives on the banner (onBannerMouseDown). Overlay's native
  // strip is covered by this chrome, so it cannot be the handle.

  async function openSettings() {
    try {
      await invoke("open_accessibility_settings");
    } catch {}
  }

  async function retry() {
    try {
      await invoke("prompt_accessibility");
    } catch {}
    await tick();
  }

  async function copyPath() {
    if (!status) return;
    try {
      await navigator.clipboard.writeText(status.bundlePath);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {}
  }

  $effect(() => {
    if (!themeId) return;
    document.documentElement.dataset.theme = themeId;
    applyHostTokens(themeId);
    // S4.2: no Clear material — vibrancy stays off for every theme.
    invoke("set_vibrancy", { enabled: false }).catch(() => {});
    // S4.4: re-assert the native transparency surgery (WKWebView
    // drawsBackground can come back up white after load).
    invoke("ensure_glass").catch(() => {});
  });

  // Reads themeId too: the rgba fills are computed from the resolved theme
  // tokens, so a theme flip must re-derive them (theme effect above runs
  // first and sets the tokens).
  $effect(() => {
    if (!themeId) return;
    applyWindowOpacity(windowOpacity);
  });

  let timer: ReturnType<typeof setInterval>;
  let unlistenExpand: (() => void) | undefined;
  let unlistenCollapse: (() => void) | undefined;
  let unlistenBeing: (() => void) | undefined;
  let unlistenWalkerPack: (() => void) | undefined;
  let unlistenWalkerAssign: (() => void) | undefined;
  let unlistenYardAssign: (() => void) | undefined;
  let unlistenWalkedOff: (() => void) | undefined;
  let unlistenSleeperWoke: (() => void) | undefined;
  let unlistenBoardDone: (() => void) | undefined;
  let unlistenBoardTodo: (() => void) | undefined;
  let unlistenCheckin: (() => void) | undefined;
  let unlistenState: (() => void) | undefined;
  let unlistenTreeMove: (() => void) | undefined;
  let unlistenTreeGone: (() => void) | undefined;
  let unlistenStage: (() => void) | undefined;
  let unlistenStagePrefs: (() => void) | undefined;

  // Walker right-click menu pick. `kind` = import
  // first ("image" / "folder"); `packId` = direct pick (null = Generated).
  interface WalkerPackPick {
    entryId: string;
    kind?: string | null;
    packId?: string | null;
  }
  const osMedia = window.matchMedia("(prefers-color-scheme: dark)");
  const onOsThemeChange = () => {
    if (!themeChosen) themeId = osThemeId();
  };
  const onNoteKey = () => noteRailAct();
  const onNoteWheel = () => noteRailAct();
  const motionMedia = window.matchMedia("(prefers-reduced-motion: reduce)");
  const onMotionChange = () => {
    reduceMotion = motionMedia.matches;
  };
  onMount(async () => {
    reduceMotion = motionMedia.matches;
    motionMedia.addEventListener("change", onMotionChange);
    window.addEventListener("keydown", onNoteKey);
    window.addEventListener("wheel", onNoteWheel, { passive: true });
    detachMenuKeys = menuRing.attach(() => {
      if (claimPickerFor) {
        claimPickerFor = null;
        return true;
      }
      if (creatingProject) {
        creatingProject = false;
        return true;
      }
      if (!embedded && arch.front !== "arch") {
        archDo(archBack(arch));
        return true;
      }
      if (
        menuRing.activeId === "rcoach-claim" ||
        menuRing.activeId === "rcoach-skip"
      ) {
        void coachSkip();
        return true;
      }
      if (!embedded && previewStop(menuRing.activeId)) {
        const pid = openSectionId();
        if (pid) {
          menuRing.select(`rhead-${pid}`);
          return true;
        }
      }
      return false;
    });
    try {
      const s = await invoke<PersistedState>("load_state");
      const migrated = migrateThemeId(s.themeId ?? "");
      if (migrated) {
        themeId = migrated;
        themeChosen = true;
      } else {
        themeId = defaultThemeId();
      }
      windowOpacity = clampWindowOpacity(s.windowOpacity ?? 1);
      projects = s.projects ?? [];
      // Pre-S4 claims carry lastTouchedAt 0 — no earlier data exists, so the
      // zombie clock honestly starts at first S4 launch.
      const firstSeen = Date.now() / 1000;
      claims = (s.claims ?? []).map((c) =>
        c.lastTouchedAt > 0 ? c : { ...c, lastTouchedAt: firstSeen },
      );
      // Old saves have no charSeed — roll one per claim
      // so every existing walker keeps a stable being from here on.
      {
        let seeded = false;
        claims = claims.map((c) => {
          if (c.charSeed) return c;
          seeded = true;
          return { ...c, charSeed: rollCharSeed() };
        });
        if (seeded) persist();
      }
      tags = s.tags ?? [];
      windowTags = s.windowTags ?? [];
      collapsedSectionIds = stageCollapsedIds(s.collapsedSectionIds ?? [], [
        ...(s.projects ?? []).map((p) => p.id),
        UNMAPPED_KEY,
      ]);
      applyStagePrefs(s);
      nudgeSuppressed = s.nudgeSuppressedWindowIds ?? [];
      kanbanCards = s.kanbanCards ?? [];
      cardTombstones = s.cardTombstones ?? [];
      pointsLedger = s.pointsLedger ?? [];
      titleOverrides = s.titleOverrides ?? [];
      autohideRowChrome = s.autohideRowChrome ?? true;
      // Legacy showFullTitles is ignored on purpose: both old values land on
      // titles visible, one line, ellipsis.
      hideTitles = s.hideTitles ?? false;
      hideAppIcons = s.hideAppIcons ?? false;
      railBoardCols = normalizeCols(s.railBoardCols);
      spriteScale = clampSpriteScale(s.spriteScale ?? 1);
      marbleDrift = s.marbleDrift ?? false;
      if (maybeGraduatePeerChrome(s)) {
        peerChrome = true;
        void invoke("set_peer_chrome", { on: true }).catch(() => {});
      } else {
        peerChrome = peerChromeOn(s);
      }
      planetIdle = s.planetIdle ?? "4h";
      agentTerminal = s.agentTerminal ?? "";
      actionShortcuts = s.actionShortcuts ?? [];
      agentShortcuts = s.agentShortcuts ?? [];
      agentRuns = s.agentRuns ?? [];
      neededSplit = s.neededSplit ?? 0.28;
      previewWidth = s.previewWidth ?? 280;
      trackerSplit = s.trackerSplit ?? 0.20;
      openSubtaskIds = s.openSubtaskIds ?? [];
      todoTrees = s.todoTrees ?? [];
      treeGrowDays = s.treeGrowDays ?? 14;
      companionState = s.companion ?? companionState;
      starsState = s.stars ?? { promptsOptIn: false, checkins: {} };
      skyState = s.sky ?? { muted: [] };
      drawersState = s.drawers ?? {};
      starterStory = normalizeStory(s.starterStory);
    } catch {
      themeId = defaultThemeId();
    }
    stateLoaded = true;
    if (!embedded) {
      void parkStage().finally(() => {
        bootParkDone = true;
        maybeRevealCoach();
      });
      window.addEventListener("blur", onWinBlur);
    }
    // ORPHAN-CARDS: pre-heal saves may carry orphan window-cards. This
    // run is a safe no-op for demotion (windowsPolled is false until the
    // first tick) — the tick below re-claims the live ones.
    if (!embedded && healOrphanWindowCards()) persist();
    refreshPacks();
    // Marble click on a planet expands the matching rail section; the
    // planet's right-click Minimize collapses it back.
    unlistenExpand = await listen<string>("habitat:expand", (e) => {
      if (!e.payload) return;
      revealPlanet(e.payload);
      persist();
      void showStage();
    });
    unlistenStage = await listen<{ revealed?: boolean }>("orbitty:rail-stage", (e) => {
      stageRevealed = !!e.payload?.revealed;
    });
    unlistenStagePrefs = await listen<{
      railEdge?: string;
      railAutoHideOnBlur?: boolean;
    }>("orbitty:rail-stage-prefs", (e) => {
      applyStagePrefs(e.payload ?? {});
    });
    unlistenCollapse = await listen<string>("habitat:collapse", (e) => {
      if (!isCollapsed(e.payload)) toggleCollapse(e.payload);
    });
    // The walker right-click menu's "Open
    // being…" is the Being editor's only door (walker single-click is the
    // locate stroke now).
    unlistenBeing = await listen<string>("habitat:being", (e) => {
      openBeingEditor(e.payload);
    });
    // Walker right-click menu picks land here: import-first verbs run the
    // native picker then skin the walker with the new pack; a direct pick
    // writes the claim (null = back to the generated being). The native
    // side never writes claims.
    unlistenWalkerPack = await listen<WalkerPackPick>(
      "habitat:walker-pack",
      async (e) => {
        const { entryId, kind, packId } = e.payload;
        if (kind === "image" || kind === "folder") {
          try {
            const p = await invoke<PackInfo | null>(
              kind === "image" ? "import_pack_image" : "import_pack_folder",
            );
            if (p) {
              setSpritePack(entryId, p.id);
              await refreshPacks();
            }
          } catch (err) {
            wakeNote = { kind: "err", text: `${err}` };
          }
          return;
        }
        setSpritePack(entryId, packId ?? null);
      },
    );
    unlistenTreeMove = await listen<{ cardId: string; x: number; y: number }>(
      "habitat:todo-tree-move",
      (e) => {
        const { cardId, x, y } = e.payload;
        todoTrees = todoTrees.map((t) =>
          t.cardId === cardId ? { ...t, x, y } : t,
        );
        persist();
      },
    );
    unlistenTreeGone = await listen<{ cardId: string }>(
      "habitat:todo-tree-gone",
      (e) => {
        todoTrees = todoTrees.filter((t) => t.cardId !== e.payload.cardId);
        persist();
      },
    );
    // Walker Assign-to-planet IS the rail claim —
    // the same persist door. claim() rewrites projectId (exclusive per
    // windowId), moves the open kanban window card with it, and the sprite
    // follows the claim. No occupancy teleport, no dual home.
    unlistenWalkerAssign = await listen<{ entryId: string; projectId: string }>(
      "habitat:walker-assign",
      (e) => {
        claim(e.payload.entryId, e.payload.projectId);
      },
    );
    // A yard cat's right-click "Assign to planet"
    // lands here — the SAME claim() door as the walker menu (one windowId,
    // one planet; persist + habitat sync follow). The yard native side
    // never writes claims; its cat leaves on the pick and the next yard
    // poll agrees. The walker the planet then draws is the planet's own
    // pack/being, never the yard cat.
    unlistenYardAssign = await listen<{ windowId: string; projectId: string }>(
      "yard:assign",
      (e) => {
        claim(e.payload.windowId, e.payload.projectId);
      },
    );
    // A Done walker crossed the north rim (or had no
    // walker to walk off — marble / open dot). The walk-off is narration;
    // THIS is the existing Done eviction: unclaim, same as the pre-cadence
    // complete path. Idempotent (the native side re-emits while a done
    // entry lingers), and a re-claim mid-walk (a new open card for the
    // window) cancels the eviction so a live claim is never yanked.
    unlistenBoardDone = await listen<string>("habitat:board-done", (e) => {
      const project = projects.find((p) => p.id === e.payload);
      if (!project) return;
      for (const c of liveClaims.filter((x) => x.projectId === project.id)) {
        const open = kanbanCards.find(
          (k) =>
            k.windowId === c.windowId &&
            k.status !== "done" &&
            k.kind !== "needed",
        );
        if (!open) continue;
        const w = windows.find((x) => x.id === c.windowId);
        if (w) completeWindow(w, project);
      }
    });
    unlistenWalkedOff = await listen<string>("habitat:walked-off", (e) => {
      const windowId = e.payload;
      // An agent walker's far-rim exit evicts
      // the run row (the droid has no claim to unclaim). Idempotent —
      // the native side re-delivers while a walker lingers.
      if (windowId.startsWith("agent:")) {
        const cardId = windowId.slice("agent:".length);
        if (agentRuns.some((r) => r.cardId === cardId)) {
          agentRuns = agentRuns.filter((r) => r.cardId !== cardId);
          persist();
        }
        return;
      }
      const open = kanbanCards.find(
        (c) => c.windowId === windowId && c.status !== "done",
      );
      if (open) return;
      if (!liveClaims.some((c) => c.windowId === windowId)) return;
      claims = liveClaims.filter((c) => c.windowId !== windowId);
      persist();
    });
    // A Todo walker touched the near lip of
    // the board — the board walk IS the check-in. If that window's card
    // is still todo, flip it to doing and persist; syncHabitat then
    // sends Work and the walker leaves the board for the field on its
    // own (no idle park waiting for a human drag). claim() still lands
    // Todo; the native side emits once per claim journey, so a manual
    // drag back to Todo stays Todo. Launchpad drags are untouched.
    unlistenSleeperWoke = await listen<string>("habitat:sleeper-woke", (e) => {
      const id = e.payload;
      if (!id) return;
      // Planet click wake never goes through wakeSleeper() — bump touch
      // so fade dim clears even before rematch rewrites the id.
      touchClaim(id, true);
    });
    unlistenCheckin = await listen<string>("habitat:board-checkin", (e) => {
      const windowId = e.payload;
      const card = kanbanCards.find(
        (c) => c.windowId === windowId && c.status === "todo",
      );
      if (!card) return;
      const now = nowEpoch();
      kanbanCards = kanbanCards.map((c) =>
        c.id === card.id ? { ...c, status: "doing", lastTouchedAt: now } : c,
      );
      persist();
    });
    // a walker dropped on
    // the Todo column returns its card to todo — the drop on the board
    // IS the status move. doing→todo only: a todo card stays (no-op), a
    // done card's walk-off ritual owns it. The native side already spent
    // this roam's check-in, so the walker stays parked at the board lip.
    unlistenBoardTodo = await listen<string>("habitat:board-todo", (e) => {
      const windowId = e.payload;
      const card = kanbanCards.find(
        (c) => c.windowId === windowId && c.status === "doing",
      );
      if (!card) return;
      const now = nowEpoch();
      kanbanCards = kanbanCards.map((c) =>
        c.id === card.id ? { ...c, status: "todo", lastTouchedAt: now } : c,
      );
      persist();
    });
    // The Lists window writes state.json too (todos, checkbox completes).
    // Adopt its card/ledger writes and any claims it removed; rail-owned
    // slices (projects, tags, UI state) stay local. Echoes of our own saves
    // reload identical data — harmless. Skipped mid-drag so a window drag
    // can't be cancelled by a DOM swap.
    unlistenState = await listen("state:changed", async () => {
      if (windowDragHot()) {
        try {
          const s = await invoke<PersistedState>("load_state");
          pendingDisk = {
            kanbanCards: s.kanbanCards ?? [],
            pointsLedger: s.pointsLedger ?? [],
            agentRuns: s.agentRuns ?? [],
            cardTombstones: s.cardTombstones ?? [],
          };
        } catch {
          // Leave the previous stash. persist() will not write until it lands.
        }
        return;
      }
      pendingDisk = null;
      try {
        const s = await invoke<PersistedState>("load_state");
        kanbanCards = s.kanbanCards ?? [];
        cardTombstones = s.cardTombstones ?? [];
        todoTrees = s.todoTrees ?? [];
        treeGrowDays = s.treeGrowDays ?? 14;
        pointsLedger = s.pointsLedger ?? [];
        // The native attach door's mints/dones
        // land here; the rail round-trips the slice so persist() never
        // strips a live run.
        agentRuns = s.agentRuns ?? [];
        neededSplit = s.neededSplit ?? 0.28;
        previewWidth = s.previewWidth ?? 280;
        trackerSplit = s.trackerSplit ?? 0.20;
        openSubtaskIds = s.openSubtaskIds ?? [];
        companionState = s.companion ?? companionState;
        // STARS: the STARS window's check-in saves and the companion's
        // Prompts toggle / strip ack land here; the rail only round-trips.
        starsState = s.stars ?? starsState;
        // Sky: the Sky window's mute writes land here; round-trip only.
        skyState = s.sky ?? skyState;
        // Rail-drawer leash writes (pluck / re-tether / dx/dy) land here;
        // round-trip only — the native side owns the slice.
        drawersState = s.drawers ?? drawersState;
        starterStory = normalizeStory(s.starterStory);
        // Settings window writes theme / opacity / sprite / row chrome.
        const migrated = migrateThemeId(s.themeId ?? "");
        if (migrated) {
          themeId = migrated;
          themeChosen = true;
        }
        windowOpacity = clampWindowOpacity(s.windowOpacity ?? 1);
        spriteScale = clampSpriteScale(s.spriteScale ?? 1);
        autohideRowChrome = s.autohideRowChrome ?? autohideRowChrome;
        applyStagePrefs(s);
        hideTitles = s.hideTitles ?? hideTitles;
        hideAppIcons = s.hideAppIcons ?? hideAppIcons;
        railBoardCols =
          s.railBoardCols == null ? railBoardCols : normalizeCols(s.railBoardCols);
        marbleDrift = s.marbleDrift ?? marbleDrift;
        planetIdle = s.planetIdle ?? planetIdle;
        agentTerminal = s.agentTerminal ?? agentTerminal;
        refreshPacks();
        const diskClaims = s.claims ?? [];
        // Adopt disk claims (tray / orbittyctl claim lands here). Keep a
        // local-only claim only while an open kind:window card still
        // names it — never drop a brand-new disk claim on the floor.
        const fromDisk = diskClaims.filter((d) =>
          (s.projects ?? []).some((p) => p.id === d.projectId),
        );
        const localOrphans = liveClaims.filter(
          (c) =>
            !fromDisk.some(
              (d) => d.windowId === c.windowId && d.projectId === c.projectId,
            ) &&
            kanbanCards.some(
              (k) =>
                k.kind === "window" &&
                k.status !== "done" &&
                k.windowId === c.windowId &&
                (k.projectId ?? "") === c.projectId,
            ),
        );
        claims = [...fromDisk, ...localOrphans];
        // Adopting disk can reveal (or create) orphans — run the same
        // heal. Main rail only: the embedded sheet never writes claims.
        if (!embedded && healOrphanWindowCards()) persist();
        // CADENCE-FIX: sig-gated, not a blind lastHabitatSig nuke — a
        // loose-card move (no window, no claim) recomputes to the same
        // signature and skips the invoke entirely, so it can't stall the
        // app. Real adopted changes (claims, linked cards) change the sig
        // and sync here, ahead of the 800ms poll.
        syncHabitat();
      } catch {
        // next change retries
      }
    });
    osMedia.addEventListener("change", onOsThemeChange);
    // Last-known discs + walking fire immediately (do not await — that
    // wait was a 6–8s empty desk). Companion already showed from
    // native setup. AX status fills in beside the tick.
    void (async () => {
      if (!embedded) {
        void syncHabitatLastKnown();
        invoke<AxStatus>("accessibility_status")
          .then((s) => {
            noteAx(s);
          })
          .catch(() => {});
      }
      tick();
    })();
    timer = setInterval(tick, 800);
  });
  onDestroy(() => {
    if (dwellTimer) clearTimeout(dwellTimer);
    motionMedia.removeEventListener("change", onMotionChange);
    window.removeEventListener("keydown", onNoteKey);
    window.removeEventListener("wheel", onNoteWheel);
    clearInterval(timer);
    detachMenuKeys?.();
    unlistenExpand?.();
    unlistenCollapse?.();
    unlistenBeing?.();
    unlistenWalkerPack?.();
    unlistenWalkerAssign?.();
    unlistenYardAssign?.();
    unlistenWalkedOff?.();
    unlistenSleeperWoke?.();
    unlistenBoardDone?.();
    unlistenBoardTodo?.();
    unlistenCheckin?.();
    unlistenState?.();
    unlistenTreeMove?.();
    unlistenTreeGone?.();
    unlistenStage?.();
    unlistenStagePrefs?.();
    window.removeEventListener("blur", onWinBlur);
    osMedia.removeEventListener("change", onOsThemeChange);
  });
  // DIET-RAIL-SPLIT: surface for rail/* modules (getters keep $state live).
  const rail: import("./rail/api").RailApi = {
    UNMAPPED_KEY,
    get arch() { return arch; },
    archDo,
    planetCards,
    get spinCenter() { return spinCenter; },
    set spinCenter(v: number) { spinCenter = v; },
    get reduceMotion() { return reduceMotion; },
    coachClaim,
    coachSkip,
    get coachError() { return coachError; },
    get starterStory() { return starterStory; },
    get claims() { return claims; },
    addTodoTo,
    get addingTodoFor() { return addingTodoFor; },
    set addingTodoFor(v) { addingTodoFor = v; },
    ageClass,
    agentRowsFor,
    get agentShortcuts() { return agentShortcuts; },
    set agentShortcuts(v) { agentShortcuts = v; },
    armRemove,
    beingClaim,
    get beingEditorFor() { return beingEditorFor; },
    set beingEditorFor(v) { beingEditorFor = v; },
    beingMoveHome,
    beingScale,
    beingUnclaim,
    beingWindow,
    circleComplete,
    get claimPickerFor() { return claimPickerFor; },
    set claimPickerFor(v) { claimPickerFor = v; },
    closeBeingEditor,
    commitRename,
    completeTodo,
    completeWindow,
    get completingIds() { return completingIds; },
    set completingIds(v) { completingIds = v; },
    get copied() { return copied; },
    set copied(v) { copied = v; },
    copyPath,
    createProjectOnly,
    get creatingProject() { return creatingProject; },
    set creatingProject(v) { creatingProject = v; },
    ctxAssign,
    get ctxAssignOpen() { return ctxAssignOpen; },
    set ctxAssignOpen(v) { ctxAssignOpen = v; },
    ctxCloseWindow,
    ctxCreateAndClaim,
    get ctxMenu() { return ctxMenu; },
    set ctxMenu(v) { ctxMenu = v; },
    ctxMinimize,
    ctxProject,
    ctxRename,
    ctxRestore,
    ctxUnclaim,
    ctxWindow,
    cycleKind,
    cyclePalette,
    get dragWindowId() { return dragWindowId; },
    set dragWindowId(v) { dragWindowId = v; },
    get dropTarget() { return dropTarget; },
    set dropTarget(v) { dropTarget = v; },
    get editorPreview() { return editorPreview; },
    set editorPreview(v) { editorPreview = v; },
    get embedded() { return embedded; },
    set embedded(v) { embedded = v; },
    hackProjectId,
    hasTag,
    get hermesNote() { return hermesNote; },
    set hermesNote(v) { hermesNote = v; },
    invoke,
    isCollapsed,
    get kanbanCards() { return kanbanCards; },
    set kanbanCards(v) { kanbanCards = v; },
    kindLabel,
    get lane() { return lane; },
    set lane(v) { lane = v; },
    launchHermes,
    locateAgent,
    get locateFlashIds() { return locateFlashIds; },
    set locateFlashIds(v) { locateFlashIds = v; },
    locateRow,
    menuRing,
    get newProjectName() { return newProjectName; },
    set newProjectName(v) { newProjectName = v; },
    get newTodoTitle() { return newTodoTitle; },
    set newTodoTitle(v) { newTodoTitle = v; },
    nudgeClaim,
    nudgeCreate,
    get nudgeFor() { return nudgeFor; },
    set nudgeFor(v) { nudgeFor = v; },
    get nudgeNewName() { return nudgeNewName; },
    set nudgeNewName(v) { nudgeNewName = v; },
    get nudgeWindow() { return nudgeWindow; },
    set nudgeWindow(v) { nudgeWindow = v; },
    onBannerMouseDown,
    onBeingScaleCommit,
    onBeingScaleInput,
    get onClose() { return onClose; },
    set onClose(v) { onClose = v; },
    onDragEnd,
    onDragStart,
    get onPopout() { return onPopout; },
    set onPopout(v) { onPopout = v; },
    onSectionDragEnter,
    onSectionDragLeave,
    onSectionDragOver,
    onSectionDrop,
    onSheetPlanetClick,
    onSheetPlanetDblClick,
    onWordmarkClick,
    openCtxMenu,
    openKanbanFocus,
    openSettings,
    openSoulEditor,
    paletteLabel,
    get peerChrome() { return peerChrome; },
    set peerChrome(v) { peerChrome = v; },
    planetCss,
    get projects() { return projects; },
    set projects(v) { projects = v; },
    get refreshedAt() { return refreshedAt; },
    set refreshedAt(v) { refreshedAt = v; },
    removeProject,
    get renameValue() { return renameValue; },
    set renameValue(v) { renameValue = v; },
    get renamingId() { return renamingId; },
    set renamingId(v) { renamingId = v; },
    rerollBeing,
    resetBeingScale,
    restore,
    retry,
    get rmArmedFor() { return rmArmedFor; },
    set rmArmedFor(v) { rmArmedFor = v; },
    get rmErrorFor() { return rmErrorFor; },
    set rmErrorFor(v) { rmErrorFor = v; },
    rowsFor,
    saveSoul,
    setWorkspaceRoot,
    sheetClaim,
    sheetFocus,
    get sheetNameEl() { return sheetNameEl; },
    set sheetNameEl(v) { sheetNameEl = v; },
    sheetRowsFor,
    get sheetSlot() { return sheetSlot; },
    set sheetSlot(v) { sheetSlot = v; },
    shellOpen,
    get soulBody() { return soulBody; },
    set soulBody(v) { soulBody = v; },
    get soulFor() { return soulFor; },
    set soulFor(v) { soulFor = v; },
    get soulMsg() { return soulMsg; },
    set soulMsg(v) { soulMsg = v; },
    get soulPath() { return soulPath; },
    set soulPath(v) { soulPath = v; },
    get stateLoaded() { return stateLoaded; },
    set stateLoaded(v) { stateLoaded = v; },
    get status() { return status; },
    set status(v) { status = v; },
    stow,
    get tagFilter() { return tagFilter; },
    set tagFilter(v) { tagFilter = v; },
    get tags() { return tags; },
    set tags(v) { tags = v; },
    tagsFor,
    titleFor,
    todosFor,
    raisePlanet,
    raisePlanetWindows,
    ensureExpand,
    collapsePlanet,
    openStageDest,
    stageRole,
    openSectionId,
    get railEdge() { return railEdge; },
    get stageRevealed() { return stageRevealed; },
    openPlanetTodos,
    focusRow,
    toggleBoardCol,
    planetCount: (pid: string) => planetCount(kanbanCards, pid),
    get railBoardCols() {
      return railBoardCols;
    },
    set railBoardCols(v) {
      railBoardCols = normalizeCols(v);
    },
    toggleCollapse,
    toggleSoul,
    toggleTag,
    get unmapped() { return unmapped; },
    set unmapped(v) { unmapped = v; },
    visibleRows,
    get wakeNote() { return wakeNote; },
    set wakeNote(v) { wakeNote = v; },
    wakeSleeper,
    get windows() { return windows; },
    set windows(v) { windows = v; },
  };

</script>

<main
  class="rail"
  class:embedded
  class:stage-shell={!embedded}
  class:stage-revealed={!embedded && stageRevealed}
  class:stage-parked={!embedded && !stageRevealed}
  class:reduce-motion={reduceMotion}
  data-edge={railEdge}
  style={`--wheel-col:${WHEEL_COL}px;--wheel-row:${WHEEL_ROW}px;--pin-h:${PIN_H}px;--preview-m:${PREVIEW_MARGIN}px`}
  class:autohide={autohideRowChrome}
  class:hide-titles={hideTitles}
  class:hide-icons={hideAppIcons}
  onpointermove={onStagePointerMove}
  onpointerdown={(e) => {
    noteRailAct();
    onStagePointerDown(e);
  }}
  onmouseleave={() => {
    pokeArch("none");
    stagePointer(false);
  }}
  onfocusin={() => stageFocus(true)}
  onfocusout={(e) => {
    const next = e.relatedTarget;
    const stay =
      next instanceof Node && e.currentTarget instanceof Node && e.currentTarget.contains(next);
    stageFocus(stay);
  }}
>
  <RailChrome {rail} />
  <RailList {rail} />
  <RailClaimUi {rail} />
</main>
