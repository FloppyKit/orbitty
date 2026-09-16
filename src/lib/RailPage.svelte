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
  import "./chrome.css";
  import type {
    ActionShortcut,
    AgentRun,
    AgentShortcut,
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

  // S4: local Kanban + points ledger (persisted), zombie ages.
  let kanbanCards = $state<KanbanCard[]>([]);
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
  let hideTitles = $state(false);
  let hideAppIcons = $state(false);
  let spriteScale = $state(1);
  // Settings owns the toggle; the rail round-trips
  // it and rides it to the native habitat on the snapshot.
  let marbleDrift = $state(false);

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
  // bubble Esc). Embedded only, and walkers on the disc never see these
  // keys — the ring lives in the well, not the habitat.
  const menuRing = new MenuRing();

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
    if (!embedded) return;
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

  async function persist() {
    if (!stateLoaded) return;
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
      pointsLedger,
      titleOverrides,
      autohideRowChrome,
      hideTitles,
      hideAppIcons,
      spriteScale,
      marbleDrift,
      actionShortcuts,
      agentShortcuts,
      agentRuns,
      neededSplit: neededSplit,
      openSubtaskIds,
      todoTrees,
      treeGrowDays,
      companion: companionState,
      stars: starsState,
      sky: skyState,
      drawers: drawersState,
    };
    try {
      await invoke("save_state", { state });
    } catch {
      // next mutation retries; rail stays usable without persistence
    }
    syncHabitat();
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
    return p;
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
              kind: c.kind ?? "window",
              status: "todo",
              clinicalOrMaster: c.clinicalOrMaster || isClinicalOrMaster(windowId),
              lastTouchedAt: now,
            }
          : c,
      );
    } else {
      kanbanCards = [
        ...kanbanCards,
        {
          id: newCardId(),
          title,
          projectId,
          windowId,
          kind: "window" as const,
          status: "todo" as const,
          createdAt: now,
          lastTouchedAt: now,
          completedAt: null,
          goalAligned: true,
          clinicalOrMaster: isClinicalOrMaster(windowId),
          pointsAwarded: 0,
          starred: false,
        },
      ];
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
    // Award once per card (first transition to done only); 1 pt default,
    // 2 if tagged clinical/master; 0 when the daily cap would be exceeded —
    // the card still completes, quietly.
    const firstComplete = !existing?.completedAt;
    const base = clinicalOrMaster ? 2 : 1;
    const award = firstComplete && pointsToday() + base <= DAILY_CAP ? base : 0;
    const cardId =
      existing?.id ??
      `c-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    if (existing) {
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
    } else {
      kanbanCards = [
        ...kanbanCards,
        {
          id: cardId,
          title,
          projectId: project.id,
          windowId: w.id,
          status: "done",
          createdAt: now,
          lastTouchedAt: now,
          completedAt: now,
          goalAligned: true,
          clinicalOrMaster,
          pointsAwarded: award,
        },
      ];
    }
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
    // The claim SURVIVES the complete so the walker can
    // walk off the north rim — the card's done status rides the snapshot and
    // the native side narrates the walk. The eviction itself is unchanged:
    // the habitat:walked-off listener below unclaims once the walker is off
    // (and a done entry with no walker to walk off — marble, open dot —
    // emits immediately, so nothing lingers).
    persist();
    // Complete → close → sync: AX press on the window's own close button.
    // A refusal (no button, save prompt, app ignores) leaves the card done.
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

  function createProjectOnly() {
    if (!newProjectName.trim()) return;
    const p = createProject(newProjectName);
    newProjectName = "";
    creatingProject = false;
    persist();
    if (embedded) {
      sheetSlot = p.id;
      raisePlanet(p.id);
    }
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
      status = s;
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
  // else same bundle+title first unused) and rewrite the id everywhere.
  // Two claims never merge onto one window; unmatched claims stay listed as
  // orphans until their window returns — nothing is deleted.
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
  let unlistenBoardDone: (() => void) | undefined;
  let unlistenBoardTodo: (() => void) | undefined;
  let unlistenCheckin: (() => void) | undefined;
  let unlistenState: (() => void) | undefined;
  let unlistenTreeMove: (() => void) | undefined;
  let unlistenTreeGone: (() => void) | undefined;

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
  onMount(async () => {
    if (embedded) {
      detachMenuKeys = menuRing.attach(() => {
        if (claimPickerFor) {
          claimPickerFor = null;
          return true;
        }
        if (creatingProject) {
          creatingProject = false;
          return true;
        }
        return false;
      });
    }
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
      collapsedSectionIds = s.collapsedSectionIds ?? [];
      nudgeSuppressed = s.nudgeSuppressedWindowIds ?? [];
      kanbanCards = s.kanbanCards ?? [];
      pointsLedger = s.pointsLedger ?? [];
      titleOverrides = s.titleOverrides ?? [];
      autohideRowChrome = s.autohideRowChrome ?? true;
      // Legacy showFullTitles is ignored on purpose: both old values land on
      // titles visible, one line, ellipsis.
      hideTitles = s.hideTitles ?? false;
      hideAppIcons = s.hideAppIcons ?? false;
      spriteScale = clampSpriteScale(s.spriteScale ?? 1);
      marbleDrift = s.marbleDrift ?? false;
      actionShortcuts = s.actionShortcuts ?? [];
      agentShortcuts = s.agentShortcuts ?? [];
      agentRuns = s.agentRuns ?? [];
      neededSplit = s.neededSplit ?? 0.28;
      openSubtaskIds = s.openSubtaskIds ?? [];
      todoTrees = s.todoTrees ?? [];
      treeGrowDays = s.treeGrowDays ?? 14;
      companionState = s.companion ?? companionState;
      starsState = s.stars ?? { promptsOptIn: false, checkins: {} };
      skyState = s.sky ?? { muted: [] };
      drawersState = s.drawers ?? {};
    } catch {
      themeId = defaultThemeId();
    }
    stateLoaded = true;
    // ORPHAN-CARDS: pre-heal saves may carry orphan window-cards. This
    // run is a safe no-op for demotion (windowsPolled is false until the
    // first tick) — the tick below re-claims the live ones.
    if (!embedded && healOrphanWindowCards()) persist();
    refreshPacks();
    // Marble click on a planet expands the matching rail section; the
    // planet's right-click Minimize collapses it back.
    unlistenExpand = await listen<string>("habitat:expand", (e) => {
      if (isCollapsed(e.payload)) toggleCollapse(e.payload);
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
      if (windowDragHot()) return;
      try {
        const s = await invoke<PersistedState>("load_state");
        kanbanCards = s.kanbanCards ?? [];
        todoTrees = s.todoTrees ?? [];
        treeGrowDays = s.treeGrowDays ?? 14;
        pointsLedger = s.pointsLedger ?? [];
        // The native attach door's mints/dones
        // land here; the rail round-trips the slice so persist() never
        // strips a live run.
        agentRuns = s.agentRuns ?? [];
        neededSplit = s.neededSplit ?? 0.28;
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
        // Settings window writes theme / opacity / sprite / row chrome.
        const migrated = migrateThemeId(s.themeId ?? "");
        if (migrated) {
          themeId = migrated;
          themeChosen = true;
        }
        windowOpacity = clampWindowOpacity(s.windowOpacity ?? 1);
        spriteScale = clampSpriteScale(s.spriteScale ?? 1);
        autohideRowChrome = s.autohideRowChrome ?? autohideRowChrome;
        hideTitles = s.hideTitles ?? hideTitles;
        hideAppIcons = s.hideAppIcons ?? hideAppIcons;
        marbleDrift = s.marbleDrift ?? marbleDrift;
        refreshPacks();
        const diskClaims = s.claims ?? [];
        // ORPHAN-CARDS: never strand an open window-card — a local claim
        // stays while its card still exists on disk for the same project.
        // A claim drops only when disk has neither the claim nor the card
        // (a real delete, not an orphan-making write).
        claims = liveClaims.filter(
          (c) =>
            diskClaims.some(
              (d) => d.windowId === c.windowId && d.projectId === c.projectId,
            ) ||
            kanbanCards.some(
              (k) =>
                k.kind === "window" &&
                k.status !== "done" &&
                k.windowId === c.windowId &&
                (k.projectId ?? "") === c.projectId,
            ),
        );
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
            status = s;
          })
          .catch(() => {});
      }
      tick();
    })();
    timer = setInterval(tick, 800);
  });
  onDestroy(() => {
    clearInterval(timer);
    detachMenuKeys?.();
    unlistenExpand?.();
    unlistenCollapse?.();
    unlistenBeing?.();
    unlistenWalkerPack?.();
    unlistenWalkerAssign?.();
    unlistenYardAssign?.();
    unlistenWalkedOff?.();
    unlistenBoardDone?.();
    unlistenBoardTodo?.();
    unlistenCheckin?.();
    unlistenState?.();
    unlistenTreeMove?.();
    unlistenTreeGone?.();
    osMedia.removeEventListener("change", onOsThemeChange);
  });
</script>

{#snippet winRow(w: WindowInfo, project: Project | null)}
  {@const age = project ? ageClass(w.id) : "fresh"}
  {@const sleeping = !!w.sleeping}
  <li>
    <div
      class="rowwrap"
      class:dragging={dragWindowId === w.id}
      class:zfade={age === "fade"}
      class:zpark={age === "park"}
      class:zgone={age === "gone"}
      class:sleeping
      role="group"
      draggable="true"
      ondragstart={(e) => onDragStart(e, w)}
      ondragend={onDragEnd}
      title={sleeping
        ? "Asleep — the window is closed; double-click to wake it. Drag still re-homes."
        : "Drag onto a project to claim"}
    >
      {#if project}
        <button
          class="circle-btn"
          class:filling={completingIds.includes(w.id)}
          onclick={() => circleComplete(w.id, () => completeWindow(w, project))}
          title="Complete — marks the card done, retires the sprite, closes the window"
          aria-label="Complete"
          >{#if completingIds.includes(w.id)}✓{/if}</button
        >
      {/if}
      {#if renamingId === w.id}
        <span class="row editing">
          <input
            class="rename-input"
            bind:value={renameValue}
            placeholder={w.title || w.appName}
            aria-label="Rename window title (empty restores the live title)"
            autofocus
            onfocus={(e) => e.currentTarget.select()}
            onkeydown={(e) => {
              if (e.key === "Enter") commitRename(w);
              if (e.key === "Escape") renamingId = null;
            }}
            onblur={() => commitRename(w)}
          />
        </span>
      {:else}
        <button
          class="row"
          class:front={w.isFrontmost}
          class:min={w.isMinimized}
          class:sleeping
          class:locate-flash={locateFlashIds.includes(w.id)}
          onclick={() => locateRow(w)}
          ondblclick={() =>
            sleeping ? wakeSleeper(w) : w.isMinimized ? restore(w) : stow(w)}
          oncontextmenu={(e) => openCtxMenu(e, w)}
          title={sleeping
            ? `${w.appName} — ${w.title} · asleep — click glows, double-click wakes`
            : w.isMinimized
              ? `${w.appName} — ${w.title} · click glows, double-click restores`
              : `${w.appName} — ${w.title} · click glows, double-click minimizes`}
        >
          {#if w.iconBase64}
            <img class="app-icon" src="data:image/png;base64,{w.iconBase64}" alt="" />
          {:else}
            <span class="app-icon fallback" aria-hidden="true"
              >{w.appName.charAt(0) || "?"}</span
            >
          {/if}
          <span class="wtitle">{titleFor(w)}</span>
          {#if sleeping}<span class="tag sleeptag">asleep</span>{/if}
          {#if !sleeping && w.isMinimized}<span class="tag stowtag">stowed</span>{/if}
          {#each tagsFor(w.id) as t (t.id)}
            <span class="tag wtag" title="Tag: {t.name}">{t.name}</span>
          {/each}
          {#if age !== "fresh"}<span class="tag ztag">{age}</span>{/if}
          {#if w.isFrontmost}<span class="tag front-tag">front</span>{/if}
        </button>
      {/if}
    </div>
  </li>
{/snippet}

{#snippet agentRow(r: AgentRun)}
  {@const agent = agentShortcuts.find((a) => a.id === r.agentShortcutId)}
  {@const card = kanbanCards.find((k) => k.id === r.cardId)}
  {@const harness = (agent?.harness || r.herdr?.kind || "agent").toLowerCase()}
  <li>
    <div class="rowwrap" role="group" title="Live agent — click glows the droid and focuses Herdr">
      <button
        class="row"
        class:locate-flash={locateFlashIds.includes(`agent:${r.cardId}`)}
        onclick={() => locateAgent(r)}
        ondblclick={() => locateAgent(r)}
      >
        <span class="app-icon fallback" aria-hidden="true">{harness.charAt(0).toUpperCase()}</span>
        <span class="wtitle">{agent?.name ?? harness}{card?.title ? ` — ${card.title}` : ""}</span>
        <span class="tag wtag">{harness}</span>
        {#if r.state === "blocked"}<span class="tag sleeptag">blocked</span>
        {:else if r.state === "working" || r.state === "starting"}<span class="tag front-tag">{r.state}</span>
        {/if}
      </button>
    </div>
  </li>
{/snippet}

{#snippet todoRow(card: KanbanCard)}
  <li class="todo-row">
    <button
      class="circle-btn"
      class:filling={completingIds.includes(card.id)}
      onclick={() => circleComplete(card.id, () => completeTodo(card))}
      title="Complete — marks the to-do done"
      aria-label="Complete to-do"
      >{#if completingIds.includes(card.id)}✓{/if}</button
    >
    <button
      class="todo-name"
      onclick={() => openKanbanFocus(card.id)}
      title="{card.title} — open in Lists">{card.title}</button
    >
  </li>
{/snippet}

<!-- the × arms an
     in-sheet confirm chip (no window.confirm — JS dialogs don't surface on
     the companion overlay). stopPropagation on mousedown AND click so the
     companion pane-drag and the section-head collapse toggle can't steal
     the press. Shared by the Inspector On planets rows and the rail list. -->
{#snippet planetRm(p: Project)}
  {#if rmArmedFor === p.id}
    <span class="planet-rm-confirm" aria-label="Remove planet {p.name}?">
      <span class="rm-q">Remove "{p.name}"?</span>
      <button
        class="mini rm-yes"
        onmousedown={(e) => e.stopPropagation()}
        onclick={(e) => {
          e.stopPropagation();
          removeProject(p);
        }}>Yes</button
      >
      <button
        class="mini ghost"
        onmousedown={(e) => e.stopPropagation()}
        onclick={(e) => {
          e.stopPropagation();
          rmArmedFor = null;
        }}>No</button
      >
    </span>
  {:else}
    <button
      class="mini ghost planet-rm"
      onmousedown={(e) => e.stopPropagation()}
      onclick={(e) => {
        e.stopPropagation();
        armRemove(p.id);
      }}
      title="Remove planet — cards keep their project, walkers vanish with the disc"
      aria-label="Remove planet {p.name}">×</button
    >
  {/if}
{/snippet}

<!-- Hosted well — top orb strip, one body.
     Extra rows scroll inside the selected tab, not the tile. -->
{#snippet planetsSheet()}
  <div class="sheet">
  <div class="planet-strip" role="tablist" aria-label="Planets">
    <button
      type="button"
      class="planet-tab"
      class:sel={sheetSlot === "unclaimed"}
      class:mkey-sel={menuRing.activeId === "ptab-unclaimed"}
      data-mkey="ptab-unclaimed"
      role="tab"
      aria-selected={sheetSlot === "unclaimed"}
      onclick={() => (sheetSlot = "unclaimed")}
    >
      <span class="planet-tab-name">Unclaimed</span>
      <span class="section-count">{unmapped.length}</span>
    </button>
    {#each projects as p (p.id)}
      <button
        type="button"
        class="planet-tab"
        class:sel={sheetSlot === p.id}
        class:mkey-sel={menuRing.activeId === `ptab-${p.id}`}
        data-mkey={`ptab-${p.id}`}
        role="tab"
        aria-selected={sheetSlot === p.id}
        title={p.name}
        onclick={() => onSheetPlanetClick(p.id)}
        ondblclick={() => onSheetPlanetDblClick(p.id)}
      >
        <span
          class="planet-chip"
          class:live={hackProjectId === p.id}
          style="background: {planetCss(p.id)}; --pglow: {planetCss(p.id)}"
          aria-hidden="true"
        ></span>
        <span class="planet-tab-name">{p.name}</span>
        <span class="section-count">{sheetRowsFor(p.id).length}</span>
      </button>
    {/each}
    {#if creatingProject}
      <span class="newproj strip-add">
        <input
          bind:this={sheetNameEl}
          bind:value={newProjectName}
          placeholder="Planet name"
          onkeydown={(e) => {
            if (e.key === "Enter") createProjectOnly();
            if (e.key === "Escape") creatingProject = false;
          }}
        />
        <button class="mini" onclick={createProjectOnly}>Add</button>
        <button class="mini ghost" onclick={() => (creatingProject = false)}>×</button>
      </span>
    {:else}
      <button
        type="button"
        class="planet-tab add"
        class:mkey-sel={menuRing.activeId === "ptab-add"}
        data-mkey="ptab-add"
        onclick={() => (creatingProject = true)}>+ Planet</button
      >
    {/if}
  </div>

  <div class="sheet-body">
  {#if sheetSlot === "unclaimed"}
    <section class="sheet-sec" aria-label="Unclaimed windows">
      {#if unmapped.length === 0}
        <p class="sheet-empty">Every window has a planet.</p>
      {:else}
        <ul class="wins">
          {#each unmapped as w (w.id)}
            <li
              class="sheet-row"
              class:mkey-sel={menuRing.activeId === `prow-${w.id}`}
              data-mkey={`prow-${w.id}`}
            >
              <span class="app-icon-slot">
                {#if w.iconBase64}
                  <img class="app-icon" src="data:image/png;base64,{w.iconBase64}" alt="" />
                {:else}
                  <span class="app-icon fallback" aria-hidden="true"
                    >{w.appName.charAt(0) || "?"}</span
                  >
                {/if}
              </span>
              <span class="wtitle" title="{w.appName} — {w.title}">{titleFor(w)}</span>
              <button
                class="mini claim-btn"
                class:active={claimPickerFor === w.id}
                aria-expanded={claimPickerFor === w.id}
                onclick={() =>
                  (claimPickerFor = claimPickerFor === w.id ? null : w.id)}
                title="Claim to a planet">claim ▸</button
              >
            </li>
            {#if claimPickerFor === w.id}
              <li class="claim-picker">
                {#if projects.length === 0}
                  <button class="mini" onclick={() => (creatingProject = true)}
                    >No planets yet — add one above</button
                  >
                {:else}
                  {#each projects as p (p.id)}
                    <button
                      class="mini claim-pick"
                      class:mkey-sel={menuRing.activeId === `pclaim-${w.id}-${p.id}`}
                      data-mkey={`pclaim-${w.id}-${p.id}`}
                      onclick={() => sheetClaim(w.id, p.id)}
                      title="Claim to {p.name}"
                      ><span
                        class="planet-chip"
                        style="background: {planetCss(p.id)}"
                        aria-hidden="true"
                      ></span
                      >{p.name}</button
                    >
                  {/each}
                {/if}
              </li>
            {/if}
          {/each}
        </ul>
      {/if}
    </section>
  {:else}
    {@const p = projects.find((x) => x.id === sheetSlot)}
    {@const rows = p ? sheetRowsFor(p.id) : []}
    <section class="sheet-sec" aria-label={p ? `Windows on ${p.name}` : "Planet"}>
      {#if p}
        <div class="head-row">
          <h2 class="sheet-h">{p.name}</h2>
          {@render planetRm(p)}
        </div>
        {#if rmErrorFor?.id === p.id}
          <p class="rm-err" role="alert">{rmErrorFor.msg}</p>
        {/if}
      {/if}
      {#if rows.length === 0}
        <p class="sheet-empty">No windows on this planet.</p>
      {:else}
        <ul class="wins">
          {#each rows as w (w.id)}
            <li>
              <button
                class="row"
                class:front={w.isFrontmost}
                class:min={w.isMinimized}
                class:mkey-sel={menuRing.activeId === `prow-${w.id}`}
                data-mkey={`prow-${w.id}`}
                onclick={() => sheetFocus(w)}
                title="{w.appName} — {w.title} · click to focus"
              >
                {#if w.iconBase64}
                  <img class="app-icon" src="data:image/png;base64,{w.iconBase64}" alt="" />
                {:else}
                  <span class="app-icon fallback" aria-hidden="true"
                    >{w.appName.charAt(0) || "?"}</span
                  >
                {/if}
                <span class="wtitle">{titleFor(w)}</span>
                {#if w.isFrontmost}<span class="tag front-tag">front</span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}
  </div>
  </div>
{/snippet}

<main
  class="rail"
  class:embedded
  class:autohide={autohideRowChrome}
  class:hide-titles={hideTitles}
  class:hide-icons={hideAppIcons}
>
  <div class="chrome">
    <header
      class="banner"
      onmousedown={onBannerMouseDown}
    >
      <h1>
        {#if embedded}
          Planets
        {:else}
        <button
          class="wordmark"
          onclick={onWordmarkClick}
          title="Home — click to bring planets front, drag to move the rail"
          aria-label="Home — bring all planet panels to the front"
          ><span class="mark"><span class="o">O</span><span class="rest">rb</span><span class="idot">i</span><span class="pop">tty</span></span></button
        >
        {/if}
      </h1>
      {#if embedded}
        <button
          class="popout"
          onmousedown={(e) => e.stopPropagation()}
          onclick={() => onPopout?.()}
          title="Pop out to the rail window — the well closes, the companion stays"
          aria-label="Pop out Planets"
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
            <path d="M15 3h6v6" />
            <path d="M10 14 21 3" />
            <path d="M21 14v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5" />
          </svg>
        </button>
      {/if}
      {#if status?.trusted && refreshedAt}
        <span class="clock" title="last refresh">{refreshedAt}</span>
      {/if}
      <button
        class="winx"
        onmousedown={(e) => e.stopPropagation()}
        onclick={() =>
          embedded
            ? onClose?.()
            : invoke("hide_window", { label: "main" }).catch(() => {})}
        title={embedded
          ? "Dismiss the Planets well — the companion stays"
          : "Hide Orbitty — quit from the tray icon"}
        aria-label={embedded ? "Dismiss Planets" : "Hide Orbitty"}>×</button
      >
    </header>

    {#if !embedded}
      <!-- The window stack (STARS / Sky / Board /
           Companion / ⚙) — toggle_labeled, show+focus then hide. Own
           file (module law); the embedded Planets well keeps the dock's
           dest chips instead. -->
      <RailStack />
    {/if}

    {#if status?.trusted && stateLoaded && !embedded}
      <div class="list-head">
        <span class="lane-tabs" role="group" aria-label="Rail lane">
          <button
            class="mini lane-tab"
            class:active={lane === "projects"}
            role="tab"
            aria-selected={lane === "projects"}
            onclick={() => (lane = "projects")}>Projects</button
          >
          <button
            class="mini lane-tab"
            class:active={lane === "agents"}
            role="tab"
            aria-selected={lane === "agents"}
            onclick={() => (lane = "agents")}>Agents</button
          >
        </span>
        {#if lane === "projects"}
          <span class="count">{windows.length} window{windows.length === 1 ? "" : "s"}</span>
          {#if creatingProject}
            <span class="newproj">
              <input
                bind:value={newProjectName}
                placeholder="Project name"
                onkeydown={(e) => e.key === "Enter" && createProjectOnly()}
              />
              <button class="mini" onclick={createProjectOnly}>Add</button>
              <button class="mini ghost" onclick={() => (creatingProject = false)}>×</button>
            </span>
          {:else}
            <button class="mini" onclick={() => (creatingProject = true)}>+ Project</button>
          {/if}
        {/if}
      </div>
      {#if !embedded && lane === "projects" && tags.length > 0}
        <div class="tag-filters">
          <button
            class="mini"
            class:active={tagFilter === null}
            onclick={() => (tagFilter = null)}>All</button
          >
          {#each tags as t (t.id)}
            <button
              class="mini"
              class:active={tagFilter === t.id}
              onclick={() => (tagFilter = tagFilter === t.id ? null : t.id)}
              >{t.name}</button
            >
          {/each}
        </div>
      {/if}
    {/if}
  </div>

  <div class="scroll">
    {#if status && !status.trusted}
      <section class="card">
        <h2>Accessibility access required</h2>
        <p class="body">
          Orbitty needs Accessibility to list and focus windows. macOS ties the
          grant to this exact build — enable it for:
        </p>
        <p class="path">{status.bundlePath}</p>
        <p class="signing">{status.signingLine}</p>
        <div class="actions">
          <button class="primary" onclick={openSettings}>Open System Settings</button>
          <button onclick={retry}>Retry</button>
          <button class="ghost" onclick={copyPath}>{copied ? "Copied" : "Copy path"}</button>
        </div>
        <p class="hint">
          Privacy &amp; Security → Accessibility → enable the toggle for this app,
          then Retry. This view updates automatically once granted.
        </p>
      </section>
    {:else if status && stateLoaded}
      {#if embedded}
        {@render planetsSheet()}
      {:else if lane === "agents"}
        <p class="lane-hint">
          Hermes opens your existing chat app. Soul is a note you Save —
          Orbitty doesn't write it for you.
        </p>
        {#if hermesNote}
          <div class="hermes-note" class:err={hermesNote.kind === "err"}>
            <span class="hermes-text">{hermesNote.text}</span>
            <button class="mini ghost" onclick={() => (hermesNote = null)} aria-label="Dismiss">×</button>
          </div>
        {/if}
        {#if projects.length === 0}
          <p class="lane-empty">
            No projects yet — create one in Projects and its Agents show up here.
          </p>
        {:else}
          <ul class="wins">
            {#each projects as p (p.id)}
              <li class="agent-row">
                <span class="agent-name">{p.name}</span>
                <button
                  class="mini"
                  onclick={() => launchHermes(p)}
                  title="Hermes — focus or launch the existing harness with this planet's titles-only context"
                >Hermes</button>
                <button
                  class="mini"
                  class:active={soulFor === p.id}
                  onclick={() => toggleSoul(p)}
                  title="soul.md — read always; writes only via Save"
                >Soul</button>
              </li>
              {#if soulFor === p.id}
                <li>
                  <div class="soul-panel agent-soul">
                    <textarea
                      class="soul-body"
                      rows="6"
                      bind:value={soulBody}
                      spellcheck="false"
                      placeholder="Loading soul.md…"
                    ></textarea>
                    <div class="soul-actions">
                      <button
                        class="mini"
                        onclick={saveSoul}
                        title="The only writer of soul.md">Save</button>
                      <button
                        class="mini ghost"
                        onclick={openSoulEditor}
                        title={soulPath
                          ? `Open in the default editor: ${soulPath} (opens the file on disk — Save first if edited)`
                          : "Open in the default editor"}>Open in editor</button>
                      {#if soulMsg}<span class="soul-msg">{soulMsg}</span>{/if}
                    </div>
                  </div>
                </li>
              {/if}
            {/each}
          </ul>
        {/if}
      {:else if windows.length === 0 && projects.length === 0}
        <p class="empty">No windows found.</p>
      {:else}
        {#if nudgeWindow}
          <div class="nudge">
            <span class="nudge-text">
              <b>{nudgeWindow.appName}</b>
              {nudgeWindow.title || "window"} minimized — no home, no sprite. Claim
              one?
            </span>
            <div class="nudge-actions">
              {#each projects as p (p.id)}
                <button class="mini" onclick={() => nudgeClaim(p.id)}>{p.name}</button>
              {/each}
              <span class="newproj">
                <input
                  bind:value={nudgeNewName}
                  placeholder="New project…"
                  onkeydown={(e) => e.key === "Enter" && nudgeCreate()}
                />
                <button class="mini" onclick={nudgeCreate}>Claim</button>
              </span>
              <button class="mini ghost" onclick={() => (nudgeFor = null)}>Later</button>
            </div>
          </div>
        {/if}
        {#if projects.length === 0}
          <p class="proj-hint">
            No projects yet — create one with <b>+ Project</b> above, then drag
            a window onto it to claim.
          </p>
        {/if}
        {#if wakeNote}
          <div class="hermes-note" class:err={wakeNote.kind === "err"}>
            <span class="hermes-text">{wakeNote.text}</span>
            <button class="mini ghost" onclick={() => (wakeNote = null)} aria-label="Dismiss">×</button>
          </div>
        {/if}
        <ul class="wins">
          {#each projects as p (p.id)}
            {@const rows = rowsFor(p.id)}
            {@const agents = agentRowsFor(p.id)}
            <li
              class="section"
              class:drop-ok={dropTarget === p.id}
              ondragenter={(e) => onSectionDragEnter(e, p.id)}
              ondragover={(e) => onSectionDragOver(e, p.id)}
              ondragleave={(e) => onSectionDragLeave(e, p.id)}
              ondrop={(e) => onSectionDrop(e, p.id)}
            >
              <div class="head-row">
                <button
                  class="section-head"
                  onclick={() => toggleCollapse(p.id)}
                  aria-expanded={!isCollapsed(p.id)}
                  title={isCollapsed(p.id)
                    ? "Expand — you can also drop a window on this header"
                    : "Collapse"}
                >
                  <span class="chev" class:open={!isCollapsed(p.id)} aria-hidden="true">▸</span>
                  <span
                    class="planet-chip"
                    style="background: {planetCss(p.id)}"
                    title={p.name}
                    aria-hidden="true"
                  ></span>
                  <span class="section-name">{p.name}</span>
                  <span class="section-count">{visibleRows(rows).length + agents.length + todosFor(p.id).length}</span>
                </button>
                {@render planetRm(p)}
              </div>
              {#if rmErrorFor?.id === p.id}
                <p class="rm-err" role="alert">{rmErrorFor.msg}</p>
              {/if}
              {#if !isCollapsed(p.id)}
                <ul class="wins">
                  {#each visibleRows(rows) as w (w.id)}
                    {@render winRow(w, p)}
                  {/each}
                  {#each agents as r (r.cardId)}
                    {@render agentRow(r)}
                  {/each}
                  {#each todosFor(p.id) as card (card.id)}
                    {@render todoRow(card)}
                  {/each}
                  {#if visibleRows(rows).length === 0 && agents.length === 0 && todosFor(p.id).length === 0}
                    <li class="section-empty">
                      {tagFilter
                        ? "No windows with this tag"
                        : "Drop a window here to claim it"}
                    </li>
                  {/if}
                  <li class="todo-add">
                    {#if addingTodoFor === p.id}
                      <span class="newproj">
                        <input
                          bind:value={newTodoTitle}
                          placeholder="New to-do…"
                          onkeydown={(e) => e.key === "Enter" && addTodoTo(p.id)}
                        />
                        <button class="mini" onclick={() => addTodoTo(p.id)}>Add</button>
                        <button class="mini ghost" onclick={() => (addingTodoFor = null)}>×</button>
                      </span>
                    {:else}
                      <button
                        class="mini ghost"
                        onclick={() => {
                          addingTodoFor = p.id;
                          newTodoTitle = "";
                        }}>+ To-do</button
                      >
                    {/if}
                  </li>
                </ul>
              {/if}
            </li>
          {/each}

          <li
            class="section"
            class:drop-ok={dropTarget === UNMAPPED_KEY}
            ondragenter={(e) => onSectionDragEnter(e, UNMAPPED_KEY)}
            ondragover={(e) => onSectionDragOver(e, UNMAPPED_KEY)}
            ondragleave={(e) => onSectionDragLeave(e, UNMAPPED_KEY)}
            ondrop={(e) => onSectionDrop(e, null)}
          >
            <button
              class="section-head"
              onclick={() => toggleCollapse(UNMAPPED_KEY)}
              aria-expanded={!isCollapsed(UNMAPPED_KEY)}
              title={isCollapsed(UNMAPPED_KEY)
                ? "Expand — you can also drop a window on this header"
                : "Collapse"}
            >
              <span class="chev" class:open={!isCollapsed(UNMAPPED_KEY)} aria-hidden="true">▸</span>
              <span class="section-name">Unmapped</span>
              <span class="section-count">{visibleRows(unmapped).length}</span>
            </button>
            {#if !isCollapsed(UNMAPPED_KEY)}
              <ul class="wins">
                {#each visibleRows(unmapped) as w (w.id)}
                  {@render winRow(w, null)}
                {/each}
                {#if visibleRows(unmapped).length === 0}
                  <li class="section-empty">
                    {tagFilter ? "No windows with this tag" : "Drop here to unclaim"}
                  </li>
                {/if}
              </ul>
            {/if}
          </li>
        </ul>
      {/if}
    {/if}
  </div>

  {#if !embedded}
    <footer class="foot">
      Drag a row onto a project to claim · stowed home windows walk their planet
      · <a
        href="https://orbitty.space"
        onclick={(e) => {
          e.preventDefault();
          shellOpen("https://orbitty.space").catch(() => {});
        }}>orbitty.space</a>
    </footer>
  {/if}

  {#if !embedded && beingEditorFor && beingClaim()}
    {@const bw = beingWindow()}
    {@const bc = beingClaim()!}
    <div class="being-editor" role="dialog" aria-label="Being editor">
      <div class="being-head">
        <span class="being-title" title={bw ? `${bw.appName} — ${bw.title}` : bc.windowId}>
          {bw ? bw.appName : "Being"}
        </span>
        <button class="mini ghost" onclick={closeBeingEditor} aria-label="Close being editor">×</button>
      </div>

      {#if editorPreview}
        <img class="being-preview" src={editorPreview} alt="Being preview" />
      {/if}

      <div class="being-actions">
        <button class="mini" onclick={rerollBeing} title="New random being">Reroll</button>
      </div>

      <div class="being-row">
        <div class="being-slot">
          <button class="mini ghost" onclick={() => cycleKind(-1)} aria-label="Previous kind">‹</button>
          <span class="slot-name">Kind</span>
          <span class="slot-val">{kindLabel()}</span>
          <button class="mini ghost" onclick={() => cycleKind(1)} aria-label="Next kind">›</button>
        </div>
      </div>
      <div class="being-row">
        <div class="being-slot">
          <button class="mini ghost" onclick={() => cyclePalette(-1)} aria-label="Previous palette">‹</button>
          <span class="slot-name">Palette</span>
          <span class="slot-val">{paletteLabel()}</span>
          <button class="mini ghost" onclick={() => cyclePalette(1)} aria-label="Next palette">›</button>
        </div>
      </div>

      <label class="being-scale">
        Size
        <input
          type="range"
          min="0.4"
          max="2.5"
          step="0.05"
          value={beingScale()}
          oninput={onBeingScaleInput}
          onchange={onBeingScaleCommit}
        />
        <span>{beingScale().toFixed(2)}×</span>
        <button
          class="mini ghost"
          onclick={resetBeingScale}
          title="Follow the gear's global sprite size"
          disabled={bc.spriteScale == null}
        >global</button>
      </label>

      <span class="menu-label">Tags</span>
      <div class="being-actions">
        {#each tags as t (t.id)}
          <button
            class="mini"
            class:active={hasTag(bc.windowId, t.id)}
            aria-pressed={hasTag(bc.windowId, t.id)}
            onclick={() => toggleTag(bc.windowId, t.id)}>{t.name}</button
          >
        {/each}
        {#if tags.length === 0}
          <span class="slot-name">No tags yet.</span>
        {/if}
      </div>

      <span class="menu-label">Home</span>
      <div class="being-actions">
        {#each projects as p (p.id)}
          <button
            class="mini"
            class:active={bc.projectId === p.id}
            aria-pressed={bc.projectId === p.id}
            onclick={() => beingMoveHome(p.id)}>{p.name}</button
          >
        {/each}
        <button
          class="mini ghost"
          onclick={beingUnclaim}
          title="Remove the claim — the walker leaves its planet"
        >Unclaim</button>
      </div>
    </div>
  {/if}

  {#if !embedded && ctxMenu}
    {@const cw = ctxWindow()}
    {@const cp = ctxProject()}
    <button
      class="ctx-backdrop"
      aria-label="Close menu"
      onclick={() => (ctxMenu = null)}
      oncontextmenu={(e) => {
        e.preventDefault();
        ctxMenu = null;
      }}
    ></button>
    <div
      class="ctx-menu"
      role="menu"
      style="left: {ctxMenu.x}px; top: {ctxMenu.y}px"
    >
      <button
        class="ctx-item"
        role="menuitem"
        aria-expanded={ctxAssignOpen}
        onclick={() => (ctxAssignOpen = !ctxAssignOpen)}
        >Assign to…</button
      >
      {#if ctxAssignOpen}
        <div class="ctx-sub" role="group" aria-label="Assign to project">
          {#each projects as p (p.id)}
            <button
              class="ctx-item"
              class:active={cp?.id === p.id}
              role="menuitem"
              onclick={() => ctxAssign(p.id)}>{p.name}</button
            >
          {/each}
          <span class="ctx-newproj">
            <input
              bind:value={newProjectName}
              placeholder="New project…"
              onkeydown={(e) => e.key === "Enter" && ctxCreateAndClaim()}
            />
            <button class="mini" onclick={ctxCreateAndClaim}>Claim</button>
          </span>
        </div>
      {/if}
      <button class="ctx-item" role="menuitem" onclick={ctxRename}>Rename</button>
      {#if cw && !cw.sleeping}
        {#if cw.isMinimized}
          <button class="ctx-item" role="menuitem" onclick={ctxRestore}
            >Restore</button
          >
        {:else}
          <button class="ctx-item" role="menuitem" onclick={ctxMinimize}
            >Minimize</button
          >
        {/if}
        <button class="ctx-item" role="menuitem" onclick={ctxCloseWindow}
          >Close window</button
        >
      {/if}
      {#if cp}
        <div class="ctx-sep" role="separator"></div>
        <button class="ctx-item danger" role="menuitem" onclick={ctxUnclaim}
          >× Unclaim</button
        >
      {/if}
    </div>
  {/if}
</main>


<style>

  /* S4.3: every painted chrome fill is a JS-computed rgba (--bg-fill /
     --panel-fill = theme color × slider alpha, see themes/apply.ts) so the
     wallpaper shows through the whole rail. Text, icons, buttons and accent
     fills never fade; below 50% a slight solid-bg halo keeps ink readable. */
  .rail {
    height: 100vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
    background: var(--bg-fill, var(--bg));
    border-radius: 12px;
  }

  /* Fill the well pane; square corners inside
     the companion's rounded clip. */
  .rail.embedded {
    height: 100%;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    border-radius: 0;
  }

  .popout {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 6px;
    cursor: pointer;
  }

  .popout:hover {
    color: var(--accent);
    background: var(--panel-fill, var(--panel));
  }

  .popout svg {
    width: 13px;
    height: 13px;
  }

  :global(.low-fill) .rail {
    text-shadow: 0 1px 2px var(--bg-solid, transparent);
  }

  /* Fixed chrome: banner + window stack + list-head never scroll. The
     theme/settings panel lives in .scroll so it can never push content off
     the window. */
  .chrome {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 0 12px 8px;
    border-bottom: 1px solid var(--edge);
  }

  /* Real scrollport — `overflow-y: overlay` leaves WKWebView without one.
     Thin scrollbar, thumb fades in on hover. */
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px 12px 0;
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--dim) 45%, transparent) transparent;
  }

  .rail.embedded .scroll {
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 0;
  }

  .scroll::-webkit-scrollbar {
    width: 5px;
    background: transparent;
  }

  .scroll::-webkit-scrollbar-thumb {
    background: transparent;
    border-radius: 3px;
  }

  .scroll:hover::-webkit-scrollbar-thumb,
  .scroll:focus-within::-webkit-scrollbar-thumb {
    background: color-mix(in srgb, var(--dim) 55%, transparent);
  }

  /* Overlay titlebar with the traffic lights hidden: the wordmark strip is
     the window drag surface (× and gear excluded). JS startDragging on
     the banner is the one source — Overlay's strip is covered. */
  .banner {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 2px 0 6px;
    user-select: none;
    -webkit-user-select: none;
    cursor: grab;
  }
  .banner:active {
    cursor: grabbing;
  }

  .banner .winx {
    margin-left: auto;
  }

  .banner h1 {
    margin: 0;
    font-size: 0.92rem;
    font-weight: 600;
    letter-spacing: 0.04em;
  }

  /* The wordmark IS Home — bringAllFront, not a
     seventh stack button. */
  .wordmark {
    font: inherit;
    letter-spacing: inherit;
    color: var(--ink);
    background: transparent;
    border: none;
    padding: 0;
    cursor: grab;
  }

  /* Narrow rail: the clock clips away (min-width 0). */
  .clock {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    font-size: 0.68rem;
    color: var(--dim);
  }

  /* Being editor: popover card above the rail. */
  .being-editor {
    position: fixed;
    right: 10px;
    bottom: 10px;
    z-index: 60;
    width: 236px;
    max-height: 82vh;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    border: 1px solid var(--edge);
    border-radius: 10px;
    background: var(--bg-fill, var(--bg));
    box-shadow: 0 8px 28px rgb(0 0 0 / 0.35);
  }

  .being-head {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .being-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.74rem;
    font-weight: 700;
    color: var(--ink);
  }

  .being-preview {
    align-self: center;
    width: 96px;
    height: 128px;
    image-rendering: pixelated;
    border: 1px solid var(--edge);
    border-radius: 6px;
    background: var(--bg);
    object-fit: contain;
  }

  .being-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .being-slot {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 4px;
    border: 1px solid var(--edge);
    border-radius: 6px;
    padding: 2px 4px;
  }

  .being-slot .slot-name {
    font-size: 0.62rem;
    color: var(--dim);
  }

  .being-slot .slot-val {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: center;
    font-size: 0.66rem;
    font-weight: 600;
    color: var(--ink);
  }

  .being-scale {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.64rem;
    color: var(--dim);
  }

  .being-scale input[type="range"] {
    flex: 1;
    min-width: 0;
  }

  .being-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .card {
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 10px;
    padding: 14px;
  }

  .card h2 {
    margin: 0 0 8px;
    font-size: 0.92rem;
    color: var(--accent);
  }

  .card .body {
    margin: 0 0 10px;
    font-size: 0.78rem;
    line-height: 1.55;
    color: var(--dim);
  }

  .path {
    margin: 0 0 8px;
    font-size: 0.88rem;
    font-weight: 700;
    line-height: 1.45;
    word-break: break-all;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    padding: 8px;
  }

  .signing {
    margin: 0 0 12px;
    font-size: 0.74rem;
    line-height: 1.5;
    color: var(--accent);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .actions button {
    font: inherit;
    font-size: 0.74rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    padding: 6px 10px;
    cursor: pointer;
  }

  .actions button:hover {
    border-color: var(--focus, #6e4db0);
  }

  .actions .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--bg);
    font-weight: 700;
  }

  .actions .ghost {
    color: var(--dim);
  }

  .hint {
    margin: 10px 0 0;
    font-size: 0.68rem;
    line-height: 1.5;
    color: var(--dim);
  }

  .list-head {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px 8px;
  }

  .count {
    font-size: 0.68rem;
    color: var(--dim);
  }

  .list-head > .mini {
    margin-left: auto;
  }

  .mini {
    font: inherit;
    font-size: 0.68rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    padding: 3px 8px;
    cursor: pointer;
  }

  .mini:hover {
    border-color: var(--focus, #6e4db0);
  }

  .mini.active {
    border-color: var(--focus, #6e4db0);
    color: var(--accent);
    font-weight: 700;
  }

  .mini.ghost {
    color: var(--dim);
  }

  .newproj {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
  }

  .newproj input {
    font: inherit;
    font-size: 0.72rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    padding: 3px 6px;
    width: 130px;
  }

  .newproj input:focus {
    outline: none;
    border-color: var(--focus, #6e4db0);
  }

  .empty {
    color: var(--dim);
    font-size: 0.82rem;
    text-align: center;
    margin-top: 32px;
  }

  .wins {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-bottom: 6px;
    padding: 2px;
    border: 1px dashed transparent;
    border-radius: 8px;
    transition:
      border-color 0.12s,
      background 0.12s,
      box-shadow 0.12s;
  }

  .section.drop-ok {
    border: 1px solid var(--accent);
    background: var(--panel-fill, var(--panel));
    box-shadow:
      0 0 0 1px var(--accent),
      0 6px 18px rgba(0, 0, 0, 0.18);
  }

  /* Section heads are the primary drop targets — keep them tall. The whole
     head is also the collapse toggle; drop handlers live on the parent
     section, so claiming into a collapsed folder keeps working. */
  .section-head {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    min-height: 26px;
    padding: 4px 8px;
    font: inherit;
    text-align: left;
    color: var(--ink);
    background: transparent;
    border: none;
    border-radius: 6px;
    cursor: pointer;
  }

  .section-head:hover {
    background: var(--panel-fill, var(--panel));
  }

  .section-head:focus-visible {
    outline: 2px solid var(--focus, #6e4db0);
    outline-offset: 1px;
  }

  /* head + × share one
     row — the head keeps the flex, the × stays out of the collapse
     toggle so a remove can never misclick-expand. */
  .head-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .head-row .section-head {
    flex: 1;
    min-width: 0;
  }

  .planet-rm {
    flex-shrink: 0;
    padding: 1px 6px;
  }

  /* The armed state: the × becomes an in-sheet confirm chip on the row —
     WKWebView JS dialogs don't surface on the companion overlay, so the
     confirm lives in the webview (S4.6 law). */
  .planet-rm-confirm {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    min-width: 0;
    font-size: 0.68rem;
    color: var(--ink);
  }

  .planet-rm-confirm .rm-q {
    color: var(--dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 140px;
  }

  .planet-rm-confirm .rm-yes {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 700;
  }

  /* Invoke failure stays in-sheet under the row's head — the row stays. */
  .rm-err {
    margin: 2px 2px 4px;
    font-size: 0.68rem;
    line-height: 1.45;
    color: var(--accent);
    word-break: break-all;
  }

  .chev {
    flex-shrink: 0;
    font-size: 0.58rem;
    color: var(--dim);
    transition: transform 0.12s;
  }

  .chev.open {
    transform: rotate(90deg);
  }

  .section.drop-ok .section-head {
    background: var(--bg-fill, var(--bg));
  }

  .section.drop-ok .section-name {
    color: var(--accent);
  }

  .section-name {
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--dim);
  }

  /* The list wears the disc's marble color. */
  .planet-chip {
    flex-shrink: 0;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
  }

  /* a small glow in the planet's own
     color while its agent is writing (hackLive + projectId). The inset
     ring stays; menus only — never the habitat discs. */
  .planet-chip.live {
    box-shadow:
      inset 0 0 0 1px rgba(0, 0, 0, 0.25),
      0 0 8px 2px var(--pglow, var(--accent));
  }

  .section-count {
    font-size: 0.64rem;
    color: var(--dim);
    border: 1px solid var(--edge);
    border-radius: 999px;
    padding: 0 0.35rem;
  }

  /* Hosted well — one sheet, two sections. */
  .sheet-sec {
    margin-bottom: 10px;
  }

  .sheet-h {
    margin: 2px 2px 6px;
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--dim);
  }

  .sheet-empty {
    margin: 2px 4px;
    font-size: 0.68rem;
    color: var(--dim);
  }

  /* Unclaimed row: icon + title + claim ▸ (not itself clickable — the claim
     door is the only action). */
  .sheet-row {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 8px 8px;
    border-radius: 8px;
  }

  .sheet-row:hover {
    background: var(--panel-fill, var(--panel));
  }

  .sheet-row .wtitle {
    flex: 1;
    min-width: 0;
  }

  .app-icon-slot {
    flex-shrink: 0;
    display: inline-flex;
  }

  .claim-btn {
    flex-shrink: 0;
  }

  /* Inline planet picker under an unclaimed row. */
  .claim-picker {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    margin: 0 4px 4px;
    padding: 6px 8px;
    background: var(--panel-fill, var(--panel));
    border: 1px dashed var(--edge);
    border-radius: 6px;
  }

  .claim-pick {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  /* Orbs across the top, one selected body.
     Extra rows scroll in .sheet-body, not the tile. */
  .sheet {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .sheet-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 4px 8px;
    scrollbar-width: thin;
  }

  .planet-strip {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    overflow-x: auto;
    padding: 4px 8px 8px;
    scrollbar-width: thin;
    background: var(--bg-fill, var(--bg));
  }

  .planet-tab {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
    max-width: 9.5rem;
    min-height: 26px;
    padding: 4px 8px;
    font: inherit;
    font-size: 0.72rem;
    color: var(--ink);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 999px;
    cursor: pointer;
    white-space: nowrap;
  }

  .planet-tab:hover {
    background: var(--panel-fill, var(--panel));
  }

  .planet-tab.sel {
    background: var(--panel-fill, var(--panel));
    color: var(--accent);
    font-weight: 700;
    box-shadow: inset 0 0 0 1px var(--focus, #6e4db0);
  }

  .planet-tab.add {
    color: var(--dim);
    max-width: none;
  }

  .planet-tab-name {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .planet-strip .newproj.strip-add {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
  }

  /* Lane tabs on the list-head — the active lane gets the --focus edge. */
  .lane-tabs {
    display: inline-flex;
    gap: 4px;
  }

  /* Agents lane. */
  .lane-hint {
    margin: 2px 2px 8px;
    font-size: 0.68rem;
    line-height: 1.5;
    color: var(--dim);
  }

  .lane-empty {
    margin: 12px 2px;
    padding: 10px 12px;
    font-size: 0.72rem;
    line-height: 1.5;
    color: var(--dim);
    background: var(--panel-fill, var(--panel));
    border: 1px dashed var(--edge);
    border-radius: 8px;
  }

  .agent-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 26px;
    padding: 3px 8px;
  }

  .agent-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--ink);
  }

  .agent-soul {
    margin-left: 0;
  }

  /* S5 soul.md panel: read always, Save is the only writer. */
  .soul-panel {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 2px 0 4px 11px;
    padding: 6px 8px;
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 8px;
  }

  .soul-body {
    width: 100%;
    box-sizing: border-box;
    font: inherit;
    font-size: 0.72rem;
    line-height: 1.45;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    padding: 6px 8px;
    resize: vertical;
    min-height: 72px;
  }

  .soul-body:focus {
    outline: none;
    border-color: var(--focus, #6e4db0);
  }

  .soul-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .soul-msg {
    font-size: 0.64rem;
    color: var(--dim);
  }

  /* S5 Hermes launch/focus result (ok or path-hint error), dismissible. */
  .hermes-note {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 2px 2px 8px;
    padding: 6px 10px;
    font-size: 0.68rem;
    line-height: 1.45;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 8px;
    word-break: break-all;
  }

  .hermes-note.err {
    border-color: var(--accent);
  }

  .hermes-text {
    flex: 1;
    min-width: 0;
  }

  /* Empty sections stay a real drop target, not a one-line label. */
  .section-empty {
    display: flex;
    align-items: center;
    min-height: 40px;
    margin: 2px 4px;
    padding: 4px 8px 4px 11px;
    font-size: 0.68rem;
    color: var(--dim);
    border: 1px dashed var(--edge);
    border-radius: 6px;
  }

  .section.drop-ok .section-empty {
    border-color: var(--accent);
    color: var(--accent);
  }

  .proj-hint {
    margin: 4px 2px 8px;
    padding: 8px 10px;
    font-size: 0.72rem;
    line-height: 1.5;
    color: var(--dim);
    background: var(--panel-fill, var(--panel));
    border: 1px dashed var(--edge);
    border-radius: 8px;
  }

  .proj-hint b {
    color: var(--accent);
  }

  /* Row layout: the title ellipsizes inside .row (flex:1, min-width:0) while
     the action controls stay visible (flex-shrink:0); if the rail gets too
     narrow the actions wrap to a second line instead of clipping Done off
     the right edge. */
  .rowwrap {
    display: flex;
    flex-wrap: wrap;
    align-items: stretch;
    gap: 2px;
    border-radius: 4px;
  }

  .rowwrap.dragging {
    opacity: 0.45;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: 1;
    min-width: 0;
    text-align: left;
    font: inherit;
    color: var(--ink);
    background: transparent;
    border: none;
    border-left: 3px solid transparent;
    border-radius: 4px;
    padding: 6px 8px;
    cursor: pointer;
  }

  .row:hover {
    background: var(--panel-fill, var(--panel));
  }

  .row.front {
    border-left-color: var(--focus, #6e4db0);
    background: var(--panel-fill, var(--panel));
  }

  .row.front .wtitle {
    font-weight: 700;
  }

  /* Inline rename: the row swaps its button for an input in the same box. */
  .row.editing {
    padding: 4px 8px;
  }

  .rename-input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 0.8rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--focus, #6e4db0);
    border-radius: 4px;
    padding: 2px 6px;
  }

  .rename-input:focus {
    outline: none;
  }

  /* a minimized window is stowed, not
     asleep — the walker still walks its planet, so the row stays readable
     (0.85), never a ghost. Zombie age classes (.rowwrap.zpark/.zgone)
     still dim harder, and true sleepers keep .row.sleeping's 0.6. */
  .row.min {
    opacity: 0.85;
  }

  .app-icon {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    border-radius: 4px;
    object-fit: contain;
  }

  .app-icon.fallback {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    color: var(--accent);
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
  }

  /* S4.6 circle-complete: an open circle at the far left of claimed window
     rows and todo rows — the only door that retires a sprite and can mint
     points. One click fills it with a check (the fill is the confirm), then
     the row completes and leaves the rail. */
  .circle-btn {
    flex-shrink: 0;
    align-self: center;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    font: inherit;
    font-size: 0.62rem;
    font-weight: 700;
    line-height: 1;
    color: var(--bg);
    background: transparent;
    border: 1.5px solid var(--dim);
    border-radius: 999px;
    cursor: pointer;
    transition:
      background 0.18s,
      border-color 0.18s;
  }

  .circle-btn:hover {
    border-color: var(--focus, #6e4db0);
  }

  .circle-btn.filling {
    background: var(--accent);
    border-color: var(--accent);
  }

  /* Zombie aging (TS-7): the rail keeps faded rows listed, quietly dimmed;
     the habitat handles the sprite-side fade/park/cull. */
  .rowwrap.zfade .row {
    opacity: 0.75;
  }

  .rowwrap.zpark .row {
    opacity: 0.6;
  }

  .rowwrap.zgone .row {
    opacity: 0.45;
  }

  .ztag {
    font-style: normal;
    opacity: 0.8;
  }

  /* A closed claimed window keeps an obvious asleep
     row — dimmed, italic title, dashed "asleep" tag — never a ghost hole. */
  .row.sleeping {
    opacity: 0.6;
  }

  .row.sleeping .wtitle {
    font-style: italic;
  }

  .sleeptag {
    border-style: dashed;
    font-style: normal;
  }

  /* Stowed (minimized, still claimed): quiet solid pill, full-ink text —
     distinct from the dashed asleep tag it must never be read as. */
  .stowtag {
    color: var(--ink);
  }

  /* S4.6 todo rows: loose project todos listed under the project's windows,
     same leading circle. The title opens the Lists window. */
  .todo-row {
    display: flex;
    align-items: center;
    gap: 7px;
    min-height: 26px;
  }

  .todo-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font: inherit;
    font-size: 0.78rem;
    text-align: left;
    color: var(--ink);
    background: transparent;
    border: none;
    border-radius: 4px;
    padding: 5px 8px;
    cursor: pointer;
  }

  .todo-name:hover {
    color: var(--accent);
    background: var(--panel-fill, var(--panel));
  }

  .todo-add {
    display: flex;
    padding: 2px 4px 2px 25px;
  }

  .todo-add .newproj {
    margin-left: 0;
  }

  /* Claim-first nudge: shown once per unmapped minimized window. */
  .nudge {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 2px 2px 8px;
    padding: 8px 10px;
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--accent);
    border-radius: 8px;
  }

  .nudge-text {
    font-size: 0.74rem;
    line-height: 1.5;
    color: var(--ink);
  }

  .nudge-text b {
    color: var(--accent);
  }

  .nudge-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }

  .nudge-actions .newproj {
    margin-left: 0;
  }

  .menu-label {
    flex-basis: 100%;
    margin-top: 2px;
    font-size: 0.62rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--dim);
  }

  .menu-label:first-child {
    margin-top: 0;
  }

  .tag-filters {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  /* min-width keeps tags from crushing the title to 0px — it ellipsizes
     instead of vanishing. */
  .wtitle {
    flex: 1;
    min-width: 4.5em;
    font-size: 0.8rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Hide titles: the title string leaves the layout entirely — icon +
     chrome only. The row button keeps its height and hit target. */
  .rail.hide-titles .wtitle {
    display: none;
  }

  /* Auto-hide row chrome (default on): the complete-circle stays hidden
     until the row is hovered or focused within (todo circles follow the
     same rule on their row). Layout is kept — buttons fade, never reflow. */
  .rail.autohide .circle-btn {
    opacity: 0;
    pointer-events: none;
    transition:
      opacity 0.12s,
      background 0.18s,
      border-color 0.18s;
  }

  .rail.autohide .rowwrap:hover .circle-btn,
  .rail.autohide .rowwrap:focus-within .circle-btn,
  .rail.autohide .todo-row:hover .circle-btn,
  .rail.autohide .todo-row:focus-within .circle-btn {
    opacity: 1;
    pointer-events: auto;
  }

  /* Hide icons: app icons leave the layout entirely (title-only rows). */
  .rail.hide-icons .app-icon {
    display: none;
  }

  .tag {
    flex-shrink: 0;
    font-size: 0.64rem;
    color: var(--dim);
    border: 1px solid var(--edge);
    border-radius: 999px;
    padding: 0 0.4rem;
  }

  .front-tag {
    color: var(--accent);
    border-color: var(--accent);
  }

  .wtag {
    max-width: 72px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: var(--panel-fill, var(--panel));
  }

  /* Right-click window-title menu: fixed inside the rail, above the list. */
  .ctx-backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    padding: 0;
    background: transparent;
    border: none;
    cursor: default;
  }

  .ctx-menu {
    position: fixed;
    z-index: 41;
    display: flex;
    flex-direction: column;
    min-width: 140px;
    padding: 4px;
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28);
  }

  .ctx-item {
    font: inherit;
    font-size: 0.74rem;
    text-align: left;
    color: var(--ink);
    background: transparent;
    border: none;
    border-radius: 5px;
    padding: 5px 10px;
    cursor: pointer;
  }

  .ctx-item:hover {
    color: var(--bg);
    background: var(--accent);
  }

  .ctx-item.active {
    color: var(--accent);
  }

  .ctx-item.active:hover {
    color: var(--bg);
  }

  .ctx-item.danger:hover {
    color: #fff;
    background: #b3432b;
  }

  .ctx-sub {
    display: flex;
    flex-direction: column;
    margin: 1px 0 2px;
    padding: 2px 0 2px 10px;
    border-left: 2px solid var(--edge);
    margin-left: 10px;
  }

  .ctx-newproj {
    display: flex;
    gap: 4px;
    padding: 3px 6px 3px 10px;
    align-items: center;
  }

  .ctx-newproj input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 0.74rem;
    color: var(--ink);
    background: var(--bg);
    border: 1px solid var(--edge);
    border-radius: 5px;
    padding: 3px 6px;
  }

  .ctx-sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--edge);
  }

  /* Sleeper-without-walker locate fallback —
     a ~500ms row flash (the walker stroke / overlay ring are native). */
  @keyframes locate-flash {
    0%,
    100% {
      box-shadow: inset 0 0 0 0 transparent;
    }
    25%,
    75% {
      box-shadow: inset 0 0 0 2px var(--accent);
    }
  }

  .row.locate-flash {
    border-radius: 7px;
    animation: locate-flash 0.5s ease-in-out;
  }

  .foot {
    flex-shrink: 0;
    padding: 6px 12px 8px;
    font-size: 0.66rem;
    color: var(--dim);
    text-align: center;
  }

  .foot a {
    color: var(--accent);
    text-decoration: none;
  }

  .foot a:hover {
    text-decoration: underline;
  }
</style>
