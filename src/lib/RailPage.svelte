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
  import { systemTokens } from "../themes/host";
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
    spinConfirm,
    spinItems,
    spinStops,
  } from "./rail/stageLook";
  import {
    ARCH_IDLE,
    archArrow,
    archBack,
    archConfirm,
    archEnter,
    enterBoard,
    archFocusPid,
    archLayout,
    archPainted,
    CLAIM_FLASH_MS,
    claimLanded,
    EDGE_GRACE_MS,
    EDGE_HIDE_MS,
    neededAct,
    edgeHover,
    edgeLeaveSide,
    mirrorItems,
    type ArchState,
    type ClaimSnap,
    type EdgeHoverEvent,
    type EdgeHoverState,
    type Layer2Item,
  } from "./rail/stageArch";
  import {
    besideFront,
    besideNext,
    keyOwnerNext,
    outwardDir,
    pickFromKind,
    type Beside,
    type BesidePick,
    type KeyOwner,
    type WalkTarget,
  } from "./rail/stageFocus";
  import { depthOut, depthOutStop, domButtonFromButtonNumber, emptyDismiss, inputIntent, spinClickTarget, type CentreCause } from "./rail/railInput";
  import type { BoardFocus } from "./rail/stageArch";
  import {
    applyPlanetOpen,
    autoHideWanted,
    habitatConfirm,
    normalizeCols,
    planetCount,
    raisePlan,
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
  import { nowEpoch as nowEpochBody, todayKey as todayKeyBody, ageClass as ageClassBody, pointsToday as pointsTodayBody, touchClaim as touchClaimBody, windowDragHot as windowDragHotBody, absorbPendingCards as absorbPendingCardsBody, todosFor as todosForBody, rowsFor as rowsForBody, visibleRows as visibleRowsBody, hasTag as hasTagBody, tagsFor as tagsForBody, isCollapsed as isCollapsedBody, toggleCollapse as toggleCollapseBody, sectionIds as sectionIdsBody, openSectionId as openSectionIdBody } from "./rail/pageRows.svelte";
  import { stageRole as stageRoleBody, ensureExpand as ensureExpandBody, collapsePlanet as collapsePlanetBody, currentBeside as currentBesideBody, hideBesidePopouts as hideBesidePopoutsBody, clearArchToFront as clearArchToFrontBody, ownerStop as ownerStopBody, focusMkey as focusMkeyBody, releaseKeysToRail as releaseKeysToRailBody, openStageDest as openStageDestBody, popOutStars as popOutStarsBody, applyBeside as applyBesideBody, walkInto as walkIntoBody, walkOut as walkOutBody, closeStageEmbed as closeStageEmbedBody, popOutSettings as popOutSettingsBody, popOutBoard as popOutBoardBody, coachIsDue as coachIsDueBody, maybeRevealCoach as maybeRevealCoachBody, noteAx as noteAxBody, coachClaim as coachClaimBody, coachSkip as coachSkipBody, showStage as showStageBody, parkStage as parkStageBody, scheduleGraceRecheck as scheduleGraceRecheckBody, maybeAutoHide as maybeAutoHideBody, parkAfterRaise as parkAfterRaiseBody, armNeededMouth as armNeededMouthBody, stageFocus as stageFocusBody, previewStop as previewStopBody } from "./rail/pageStage.svelte";
  import { overPainted as overPaintedBody, armEdgeTimer as armEdgeTimerBody, applyEdge as applyEdgeBody, noteClaims as noteClaimsBody, onStagePointerMove as onStagePointerMoveBody, noteWheelSpin as noteWheelSpinBody, wheelAnimating as wheelAnimatingBody, nearHit as nearHitBody, focusBoardLanding as focusBoardLandingBody, takeMouseButton as takeMouseButtonBody, onStagePointerDown as onStagePointerDownBody, onStagePointerUp as onStagePointerUpBody, onStageAux as onStageAuxBody, onStageMouseLeave as onStageMouseLeaveBody, onWinBlur as onWinBlurBody, onExpandedFieldEscape as onExpandedFieldEscapeBody, applyStagePrefs as applyStagePrefsBody, sheetRowsFor as sheetRowsForBody, raisePlanet as raisePlanetBody, onSheetPlanetClick as onSheetPlanetClickBody, onSheetPlanetDblClick as onSheetPlanetDblClickBody, sheetClaim as sheetClaimBody, sheetFocus as sheetFocusBody, toggleBoardCol as toggleBoardColBody, focusRow as focusRowBody, raisePlanetWindows as raisePlanetWindowsBody, openPlanetTodos as openPlanetTodosBody, rowById as rowByIdBody } from "./rail/pagePointer.svelte";
  import { archDo as archDoBody, planetCards as planetCardsBody, layer2For as layer2ForBody, focusMirrorBoard as focusMirrorBoardBody, confirmLayer2Stop as confirmLayer2StopBody, noteRailAct as noteRailActBody, confirmHabitatStop as confirmHabitatStopBody, takeReselect as takeReselectBody, syncInspectorRing as syncInspectorRingBody, confirmRingTab as confirmRingTabBody, onRailEscape as onRailEscapeBody, reclaimRailRing as reclaimRailRingBody, toggleTag as toggleTagBody } from "./rail/pageRing.svelte";
  import { titleFor as titleForBody, startRename as startRenameBody, openCtxMenu as openCtxMenuBody, ctxWindow as ctxWindowBody, ctxProject as ctxProjectBody, ctxAssign as ctxAssignBody, ctxCreateAndClaim as ctxCreateAndClaimBody, ctxRename as ctxRenameBody, ctxMinimize as ctxMinimizeBody, ctxRestore as ctxRestoreBody, ctxCloseWindow as ctxCloseWindowBody, ctxUnclaim as ctxUnclaimBody, commitRename as commitRenameBody, setWorkspaceRoot as setWorkspaceRootBody, persist as persistBody, sleeperRowFor as sleeperRowForBody, boardCardsFor as boardCardsForBody, habitatEntryFor as habitatEntryForBody, runIsSeated as runIsSeatedBody, goToken as goTokenBody, goNeededOnPlanet as goNeededOnPlanetBody, agentRowsFor as agentRowsForBody, locateAgent as locateAgentBody, agentEntriesFor as agentEntriesForBody, habitatPlanetFor as habitatPlanetForBody, habitatSnapshotPayload as habitatSnapshotPayloadBody, syncHabitat as syncHabitatBody, lastKnownRowFor as lastKnownRowForBody, syncHabitatLastKnown as syncHabitatLastKnownBody, flashRow as flashRowBody, locateRow as locateRowBody, stow as stowBody, restore as restoreBody, wakeSleeper as wakeSleeperBody, nudgeClaim as nudgeClaimBody, nudgeCreate as nudgeCreateBody, clampSpriteScale as clampSpriteScaleBody, openKanban as openKanbanBody, bringAllFront as bringAllFrontBody, onBannerMouseDown as onBannerMouseDownBody, onWordmarkClick as onWordmarkClickBody, openKanbanFocus as openKanbanFocusBody, launchHermes as launchHermesBody, toggleSoul as toggleSoulBody, saveSoul as saveSoulBody, openSoulEditor as openSoulEditorBody, createProject as createProjectBody, revealPlanet as revealPlanetBody, newCardId as newCardIdBody, isClinicalOrMaster as isClinicalOrMasterBody, titleIndexFor as titleIndexForBody, claimStableKey as claimStableKeyBody, isHermesClass as isHermesClassBody, sleeperStamps as sleeperStampsBody, rollCharSeed as rollCharSeedBody } from "./rail/pageClaim.svelte";
  import { claim as claimBody, unclaim as unclaimBody, completeWindow as completeWindowBody, completeTodo as completeTodoBody, circleComplete as circleCompleteBody, logEvent as logEventBody, addTodoTo as addTodoToBody, occupancyOf as occupancyOfBody, setSpritePack as setSpritePackBody, beingClaim as beingClaimBody, beingWindow as beingWindowBody, refreshBeingPreview as refreshBeingPreviewBody, commitBeingLook as commitBeingLookBody, openBeingEditor as openBeingEditorBody, closeBeingEditor as closeBeingEditorBody, beingKindIndex as beingKindIndexBody, beingPaletteIndex as beingPaletteIndexBody, cycleKind as cycleKindBody, cyclePalette as cyclePaletteBody, kindLabel as kindLabelBody, paletteLabel as paletteLabelBody, rerollBeing as rerollBeingBody, beingScale as beingScaleBody, onBeingScaleInput as onBeingScaleInputBody, onBeingScaleCommit as onBeingScaleCommitBody, resetBeingScale as resetBeingScaleBody, beingMoveHome as beingMoveHomeBody, beingUnclaim as beingUnclaimBody, createProjectOnly as createProjectOnlyBody, armRemove as armRemoveBody, removeProject as removeProjectBody } from "./rail/pageBeing.svelte";
  import { onDragStart as onDragStartBody, onDragEnd as onDragEndBody, isWindowDrag as isWindowDragBody, onSectionDragEnter as onSectionDragEnterBody, onSectionDragOver as onSectionDragOverBody, onSectionDragLeave as onSectionDragLeaveBody, onSectionDrop as onSectionDropBody, tick as tickBody, rematchClaims as rematchClaimsBody, rewriteWindowId as rewriteWindowIdBody, healOrphanWindowCards as healOrphanWindowCardsBody, checkNudge as checkNudgeBody, openSettings as openSettingsBody, retry as retryBody, copyPath as copyPathBody } from "./rail/pageDrag.svelte";


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
  let claimFlash = $state<{ pid: string; title: string; until: number } | null>(null);
  let lastClaimsSeen: ClaimSnap[] | null = null;
  let edgeHoverState: EdgeHoverState = { revealed: false, revealedAt: 0, outsideSince: null };
  let reduceMotion = $state(false);
  let arch = $state<ArchState>({ ...ARCH_IDLE });
  let stageEmbed = $state<null | "settings" | "stars">(null);
  let keyOwner = $state<KeyOwner>("rail");
  let reselectStop: string | null = null;
  /** Wheel-centre changes from a spin may open the beside panel. A depth-out must not. */
  let centreCause: CentreCause = "spin";
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

  function nowEpoch() {
    return nowEpochBody({});
  }

  function todayKey() {
    return todayKeyBody({});
  }

  // Zombie aging (TS-7): visual only, titles only, never auto-kill.
  function ageClass(windowId: string) {
    return ageClassBody({FADE_AFTER_S, GONE_AFTER_S, PARK_AFTER_S, get claims() { return claims; }, set claims(v) { claims = v; }, nowEpoch}, windowId);
  }

  function pointsToday() {
    return pointsTodayBody({get pointsLedger() { return pointsLedger; }, set pointsLedger(v) { pointsLedger = v; }, todayKey});
  }

  // Touch bumps: explicit actions persist immediately; the 800ms frontmost
  // tick funnels through a 30s throttle so the poll can't thrash the disk.
  let lastTouchPersist = 0;
  function touchClaim(windowId: string, force = false) {
    return touchClaimBody({get claims() { return claims; }, set claims(v) { claims = v; }, get lastTouchPersist() { return lastTouchPersist; }, set lastTouchPersist(v) { lastTouchPersist = v; }, nowEpoch, persist}, windowId, force);
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
  function windowDragHot() {
    return windowDragHotBody({get dragStartedAt() { return dragStartedAt; }, set dragStartedAt(v) { dragStartedAt = v; }, get dragWindowId() { return dragWindowId; }, set dragWindowId(v) { dragWindowId = v; }});
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
    return absorbPendingCardsBody({get agentRuns() { return agentRuns; }, set agentRuns(v) { agentRuns = v; }, get cardTombstones() { return cardTombstones; }, set cardTombstones(v) { cardTombstones = v; }, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, get pendingDisk() { return pendingDisk; }, set pendingDisk(v) { pendingDisk = v; }, get pointsLedger() { return pointsLedger; }, set pointsLedger(v) { pointsLedger = v; }});
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
  function todosFor(projectId: string) {
    return todosForBody({get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }}, projectId);
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

  function rowsFor(projectId: string) {
    return rowsForBody({claimStableKey, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, get windows() { return windows; }, set windows(v) { windows = v; }}, projectId);
  }

  // The tag filter narrows rows inside each section; grouping stays by home
  // project — tags never create sections.
  function visibleRows(rows: WindowInfo[]) {
    return visibleRowsBody({hasTag, get tagFilter() { return tagFilter; }, set tagFilter(v) { tagFilter = v; }}, rows);
  }

  function hasTag(windowId: string, tagId: string) {
    return hasTagBody({get liveWindowTags() { return liveWindowTags; }, set liveWindowTags(v) { liveWindowTags = v; }}, windowId, tagId);
  }

  function tagsFor(windowId: string) {
    return tagsForBody({get liveWindowTags() { return liveWindowTags; }, set liveWindowTags(v) { liveWindowTags = v; }, get tags() { return tags; }, set tags(v) { tags = v; }}, windowId);
  }

  function isCollapsed(sectionId: string) {
    return isCollapsedBody({get collapsedSectionIds() { return collapsedSectionIds; }, set collapsedSectionIds(v) { collapsedSectionIds = v; }}, sectionId);
  }

  function toggleCollapse(sectionId: string) {
    return toggleCollapseBody({get collapsedSectionIds() { return collapsedSectionIds; }, set collapsedSectionIds(v) { collapsedSectionIds = v; }, isCollapsed, persist}, sectionId);
  }

  function sectionIds() {
    return sectionIdsBody({UNMAPPED_KEY, get projects() { return projects; }, set projects(v) { projects = v; }});
  }

  function openSectionId() {
    return openSectionIdBody({isCollapsed, sectionIds});
  }

  function stageRole(pid: string) {
    return stageRoleBody({openSectionId, sectionIds, stageRoles}, pid);
  }

  function ensureExpand(pid: string) {
    return ensureExpandBody({applyPlanetOpen, get collapsedSectionIds() { return collapsedSectionIds; }, set collapsedSectionIds(v) { collapsedSectionIds = v; }, openSectionId, persist, sectionIds, showStage}, pid);
  }

  function collapsePlanet(pid: string) {
    return collapsePlanetBody({get collapsedSectionIds() { return collapsedSectionIds; }, set collapsedSectionIds(v) { collapsedSectionIds = v; }, isCollapsed, persist, sectionIds}, pid);
  }

  function currentBeside() {
    return currentBesideBody({UNMAPPED_KEY, get arch() { return arch; }, set arch(v) { arch = v; }, get stageEmbed() { return stageEmbed; }, set stageEmbed(v) { stageEmbed = v; }});
  }

  function hideBesidePopouts() {
    return hideBesidePopoutsBody({invoke});
  }

  function clearArchToFront() {
    return clearArchToFrontBody({get arch() { return arch; }, set arch(v) { arch = v; }, archBack});
  }

  function ownerStop(owner: KeyOwner) {
    return ownerStopBody({UNMAPPED_KEY}, owner);
  }

  function focusMkey(id: string) {
    return focusMkeyBody({}, id);
  }

  function releaseKeysToRail() {
    return releaseKeysToRailBody({get keyOwner() { return keyOwner; }, set keyOwner(v) { keyOwner = v; }, keyOwnerNext, reclaimRailRing});
  }

  function openStageDest(label: "stars" | "settings") {
    return openStageDestBody({applyBeside}, label);
  }

  function popOutStars() {
    return popOutStarsBody({invoke, get keyOwner() { return keyOwner; }, set keyOwner(v) { keyOwner = v; }, get stageEmbed() { return stageEmbed; }, set stageEmbed(v) { stageEmbed = v; }, walkOut});
  }

  /** One applier for every selection that can change the beside owner. */
  function applyBeside(pick: BesidePick) {
    return applyBesideBody({besideFront, besideNext, clearArchToFront, currentBeside, hideBesidePopouts, releaseKeysToRail, get stageEmbed() { return stageEmbed; }, set stageEmbed(v) { stageEmbed = v; }}, pick);
  }

  function walkInto(target: WalkTarget) {
    return walkIntoBody({applyBeside, get keyOwner() { return keyOwner; }, set keyOwner(v) { keyOwner = v; }, keyOwnerNext, get reselectStop() { return reselectStop; }, set reselectStop(v) { reselectStop = v; }}, target);
  }

  function walkOut() {
    return walkOutBody({get arch() { return arch; }, set arch(v) { arch = v; }, archDo, get keyOwner() { return keyOwner; }, set keyOwner(v) { keyOwner = v; }, keyOwnerNext, ownerStop, reclaimRailRing, get reselectStop() { return reselectStop; }, set reselectStop(v) { reselectStop = v; }});
  }

  function closeStageEmbed() {
    return closeStageEmbedBody({focusMkey, get keyOwner() { return keyOwner; }, set keyOwner(v) { keyOwner = v; }, keyOwnerNext, menuRing, reclaimRailRing, get reselectStop() { return reselectStop; }, set reselectStop(v) { reselectStop = v; }, get stageEmbed() { return stageEmbed; }, set stageEmbed(v) { stageEmbed = v; }});
  }

  function popOutSettings() {
    return popOutSettingsBody({invoke, get keyOwner() { return keyOwner; }, set keyOwner(v) { keyOwner = v; }, get stageEmbed() { return stageEmbed; }, set stageEmbed(v) { stageEmbed = v; }, walkOut});
  }

  function popOutBoard() {
    return popOutBoardBody({invoke});
  }

  function coachIsDue() {
    return coachIsDueBody({get claims() { return claims; }, set claims(v) { claims = v; }, coachDue, get embedded() { return embedded; }, get starterStory() { return starterStory; }, set starterStory(v) { starterStory = v; }, get stateLoaded() { return stateLoaded; }, set stateLoaded(v) { stateLoaded = v; }, get status() { return status; }, set status(v) { status = v; }});
  }

  function maybeRevealCoach() {
    return maybeRevealCoachBody({get bootParkDone() { return bootParkDone; }, set bootParkDone(v) { bootParkDone = v; }, coachIsDue, get coachRevealed() { return coachRevealed; }, set coachRevealed(v) { coachRevealed = v; }, get embedded() { return embedded; }, showStage});
  }

  function noteAx(s: AxStatus) {
    return noteAxBody({maybeRevealCoach, get status() { return status; }, set status(v) { status = v; }}, s);
  }

  async function coachClaim() {
    return coachClaimBody({get coachError() { return coachError; }, set coachError(v) { coachError = v; }, get coachErrorTimer() { return coachErrorTimer; }, set coachErrorTimer(v) { coachErrorTimer = v; }, invoke});
  }

  async function coachSkip() {
    return coachSkipBody({invoke});
  }

  async function showStage() {
    return showStageBody({get embedded() { return embedded; }, invoke, get stageRevealed() { return stageRevealed; }, set stageRevealed(v) { stageRevealed = v; }});
  }

  async function parkStage() {
    return parkStageBody({get embedded() { return embedded; }, invoke, get stageRevealed() { return stageRevealed; }, set stageRevealed(v) { stageRevealed = v; }});
  }

  let graceRecheck: ReturnType<typeof setTimeout> | null = null;

  function scheduleGraceRecheck() {
    return scheduleGraceRecheckBody({get graceRecheck() { return graceRecheck; }, set graceRecheck(v) { graceRecheck = v; }, maybeAutoHide, get revealGraceUntil() { return revealGraceUntil; }, set revealGraceUntil(v) { revealGraceUntil = v; }});
  }

  function maybeAutoHide() {
    return maybeAutoHideBody({autoHideWanted, get embedded() { return embedded; }, get focusInside() { return focusInside; }, set focusInside(v) { focusInside = v; }, parkStage, get pointerInside() { return pointerInside; }, set pointerInside(v) { pointerInside = v; }, get railAutoHideOnBlur() { return railAutoHideOnBlur; }, set railAutoHideOnBlur(v) { railAutoHideOnBlur = v; }, get revealGraceUntil() { return revealGraceUntil; }, set revealGraceUntil(v) { revealGraceUntil = v; }, scheduleGraceRecheck, get stageRevealed() { return stageRevealed; }, set stageRevealed(v) { stageRevealed = v; }});
  }

  function parkAfterRaise() {
    return parkAfterRaiseBody({invoke});
  }

  function armNeededMouth(item: { kind?: string; id?: string; title?: string }) {
    return armNeededMouthBody({invoke, neededAct}, item);
  }

  function stageFocus(inside: boolean) {
    return stageFocusBody({get embedded() { return embedded; }, get focusInside() { return focusInside; }, set focusInside(v) { focusInside = v; }, maybeAutoHide}, inside);
  }

  function previewStop(id: string | null) {
    return previewStopBody({}, id);
  }

  function overPainted(e: PointerEvent) {
    return overPaintedBody({get arch() { return arch; }, set arch(v) { arch = v; }, archLayout, archPainted, get railEdge() { return railEdge; }, set railEdge(v) { railEdge = v; }, get stageEmbed() { return stageEmbed; }, set stageEmbed(v) { stageEmbed = v; }, get stageRevealed() { return stageRevealed; }, set stageRevealed(v) { stageRevealed = v; }}, e);
  }

  let edgeTimer: ReturnType<typeof setTimeout> | null = null;
  let revealGraceUntil = 0;
  let edgePointerSeed = false;
  let claimFlashTimer: ReturnType<typeof setTimeout> | null = null;

  function armEdgeTimer() {
    return armEdgeTimerBody({EDGE_GRACE_MS, EDGE_HIDE_MS, applyEdge, get edgeHoverState() { return edgeHoverState; }, set edgeHoverState(v) { edgeHoverState = v; }, get edgeTimer() { return edgeTimer; }, set edgeTimer(v) { edgeTimer = v; }});
  }

  function applyEdge(ev: EdgeHoverEvent, now = Date.now()) {
    return applyEdgeBody({armEdgeTimer, edgeHover, get edgeHoverState() { return edgeHoverState; }, set edgeHoverState(v) { edgeHoverState = v; }, maybeAutoHide, showStage}, ev, now);
  }

  function noteClaims(next: Claim[]) {
    return noteClaimsBody({CLAIM_FLASH_MS, get claimFlash() { return claimFlash; }, set claimFlash(v) { claimFlash = v; }, get claimFlashTimer() { return claimFlashTimer; }, set claimFlashTimer(v) { claimFlashTimer = v; }, claimLanded, invoke, get lastClaimsSeen() { return lastClaimsSeen; }, set lastClaimsSeen(v) { lastClaimsSeen = v; }, titleFor, get windows() { return windows; }, set windows(v) { windows = v; }}, next);
  }

  function onStagePointerMove(e: PointerEvent) {
    return onStagePointerMoveBody({applyEdge, get edgePointerSeed() { return edgePointerSeed; }, set edgePointerSeed(v) { edgePointerSeed = v; }, get embedded() { return embedded; }, overPainted, get pointerInside() { return pointerInside; }, set pointerInside(v) { pointerInside = v; }, get stageRevealed() { return stageRevealed; }, set stageRevealed(v) { stageRevealed = v; }}, e);
  }

  let pointerDownAt: { x: number; y: number } | null = null;
  let wheelSpinUntil = 0;
  let mouseButtonAt = 0;

  function noteWheelSpin() {
    return noteWheelSpinBody({get wheelSpinUntil() { return wheelSpinUntil; }, set wheelSpinUntil(v) { wheelSpinUntil = v; }});
  }

  function wheelAnimating() {
    return wheelAnimatingBody({get wheelSpinUntil() { return wheelSpinUntil; }, set wheelSpinUntil(v) { wheelSpinUntil = v; }});
  }

  function nearHit(e: { clientX: number; clientY: number; target: EventTarget | null }) {
    return nearHitBody({}, e);
  }

  function focusBoardLanding(focus: BoardFocus) {
    return focusBoardLandingBody({menuRing}, focus);
  }

  function confirmActive() {
    return menuRing.confirmActive();
  }

  function textFieldFocused() {
    const el = document.activeElement;
    return el instanceof Element && !!el.closest("input, textarea, select");
  }

  function takeMouseButton(button: number) {
    return takeMouseButtonBody({confirmActive, inputIntent, get mouseButtonAt() { return mouseButtonAt; }, set mouseButtonAt(v) { mouseButtonAt = v; }, onRailEscape, textFieldFocused}, button);
  }

  function onStagePointerDown(e: PointerEvent) {
    return onStagePointerDownBody({get embedded() { return embedded; }, nearHit, overPainted, get pointerDownAt() { return pointerDownAt; }, set pointerDownAt(v) { pointerDownAt = v; }, get pointerInside() { return pointerInside; }, set pointerInside(v) { pointerInside = v; }, get stageRevealed() { return stageRevealed; }, set stageRevealed(v) { stageRevealed = v; }}, e);
  }

  function onStagePointerUp(e: PointerEvent) {
    return onStagePointerUpBody({get arch() { return arch; }, set arch(v) { arch = v; }, confirmHabitatStop, confirmLayer2Stop, get embedded() { return embedded; }, emptyDismiss, menuRing, nearHit, onRailEscape, get pointerDownAt() { return pointerDownAt; }, set pointerDownAt(v) { pointerDownAt = v; }, spinClickTarget, get stageRevealed() { return stageRevealed; }, set stageRevealed(v) { stageRevealed = v; }, takeMouseButton, wheelAnimating}, e);
  }

  function onStageAux(e: MouseEvent) {
    return onStageAuxBody({get embedded() { return embedded; }, get stageRevealed() { return stageRevealed; }, set stageRevealed(v) { stageRevealed = v; }, takeMouseButton}, e);
  }

  function onStageMouseLeave(e: MouseEvent) {
    return onStageMouseLeaveBody({applyEdge, edgeLeaveSide, get edgePointerSeed() { return edgePointerSeed; }, set edgePointerSeed(v) { edgePointerSeed = v; }, get embedded() { return embedded; }, get pointerInside() { return pointerInside; }, set pointerInside(v) { pointerInside = v; }, get railEdge() { return railEdge; }, set railEdge(v) { railEdge = v; }}, e);
  }

  function onWinBlur() {
    return onWinBlurBody({stageFocus});
  }

  // Esc inside a board field is not a ring key (the field owns it). Return
  // focus to the centred layer-2 icon instead of letting the field swallow it.
  function onExpandedFieldEscape(e: KeyboardEvent) {
    return onExpandedFieldEscapeBody({get arch() { return arch; }, set arch(v) { arch = v; }, get embedded() { return embedded; }, layer2For}, e);
  }

  function applyStagePrefs(raw: {
    railEdge?: unknown;
    railAutoHideOnBlur?: unknown;
    autohideRowChrome?: unknown;
  }) {
    return applyStagePrefsBody({get autohideRowChrome() { return autohideRowChrome; }, set autohideRowChrome(v) { autohideRowChrome = v; }, get railAutoHideOnBlur() { return railAutoHideOnBlur; }, set railAutoHideOnBlur(v) { railAutoHideOnBlur = v; }, get railEdge() { return railEdge; }, set railEdge(v) { railEdge = v; }, stagePrefs}, raw);
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
  function sheetRowsFor(projectId: string) {
    return sheetRowsForBody({get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, get windows() { return windows; }, set windows(v) { windows = v; }}, projectId);
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
    return raisePlanetBody({invoke}, id);
  }

  function onSheetPlanetClick(id: string) {
    return onSheetPlanetClickBody({raisePlanet, get sheetSlot() { return sheetSlot; }, set sheetSlot(v) { sheetSlot = v; }}, id);
  }

  function onSheetPlanetDblClick(id: string) {
    return onSheetPlanetDblClickBody({emitTo, isCollapsed}, id);
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
    return sheetClaimBody({claim, get claimPickerFor() { return claimPickerFor; }, set claimPickerFor(v) { claimPickerFor = v; }}, windowId, projectId);
  }

  // On-planets row click → focus the OS window (deminiaturize + raise).
  function sheetFocus(w: WindowInfo) {
    return sheetFocusBody({invoke}, w);
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
    return toggleBoardColBody({persist, get railBoardCols() { return railBoardCols; }, set railBoardCols(v) { railBoardCols = v; }, toggleCol}, col);
  }

  async function focusRow(w: WindowInfo) {
    return focusRowBody({invoke, tick, touchClaim}, w);
  }

  async function raisePlanetWindows(pid: string) {
    return raisePlanetWindowsBody({invoke, parkAfterRaise, raisePlan, raisePlanet, rowsFor, tick}, pid);
  }

  async function openPlanetTodos(pid: string) {
    return openPlanetTodosBody({emitTo, openKanban}, pid);
  }

  function rowById(id: string) {
    return rowByIdBody({get projects() { return projects; }, set projects(v) { projects = v; }, rowsFor, get windows() { return windows; }, set windows(v) { windows = v; }}, id);
  }

  function archDo(next: ArchState) {
    return archDoBody({UNMAPPED_KEY, applyBeside, get arch() { return arch; }, set arch(v) { arch = v; }}, next);
  }

  function planetCards(pid: string) {
    return planetCardsBody({get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }}, pid);
  }

  function layer2For(pid: string | null) {
    return layer2ForBody({UNMAPPED_KEY, mirrorItems, planetCards, rowsFor, titleFor}, pid);
  }

  function focusMirrorBoard() {
    return focusMirrorBoardBody({});
  }

  function confirmLayer2Stop(id: string) {
    return confirmLayer2StopBody({get arch() { return arch; }, set arch(v) { arch = v; }, archConfirm, archDo, armNeededMouth, enterBoard, focusBoardLanding, focusMirrorBoard, focusRow, layer2For, parkAfterRaise, planetCards, restore, rowsFor, wakeSleeper, walkInto}, id);
  }

  let lastNoteAct = 0;
  function noteRailAct() {
    return noteRailActBody({get embedded() { return embedded; }, invoke, get lastNoteAct() { return lastNoteAct; }, set lastNoteAct(v) { lastNoteAct = v; }});
  }

  function confirmHabitatStop(id: string) {
    return confirmHabitatStopBody({UNMAPPED_KEY, applyBeside, get arch() { return arch; }, set arch(v) { arch = v; }, archDo, archEnter, coachClaim, coachSkip, get creatingProject() { return creatingProject; }, set creatingProject(v) { creatingProject = v; }, ensureExpand, focusRow, habitatConfirm, layer2For, openKanbanFocus, openPlanetTodos, openStageDest, parkAfterRaise, get projects() { return projects; }, set projects(v) { projects = v; }, raisePlanetWindows, rowById, spinConfirm, spinItems, toggleBoardCol, wakeSleeper}, id);
  }

  function takeReselect() {
    return takeReselectBody({focusMkey, menuRing, get reselectStop() { return reselectStop; }, set reselectStop(v) { reselectStop = v; }});
  }

  function syncInspectorRing() {
    return syncInspectorRingBody({UNMAPPED_KEY, applyBeside, get arch() { return arch; }, set arch(v) { arch = v; }, archArrow, archDo, archFocusPid, armNeededMouth, coachIsDue, confirmHabitatStop, confirmLayer2Stop, get embedded() { return embedded; }, focusBoardLanding, focusMkey, get keyOwner() { return keyOwner; }, set keyOwner(v) { keyOwner = v; }, get lane() { return lane; }, set lane(v) { lane = v; }, layer2For, locateRow, menuRing, outwardDir, pickFromKind, planetCards, get projects() { return projects; }, set projects(v) { projects = v; }, get railEdge() { return railEdge; }, set railEdge(v) { railEdge = v; }, get spinCenter() { return spinCenter; }, set spinCenter(v) { spinCenter = v; }, spinItems, spinStops, takeReselect, get unmapped() { return unmapped; }, set unmapped(v) { unmapped = v; }, visibleRows, wakeSleeper, walkInto, walkOut});
  }

  function confirmRingTab(id: string) {
    return confirmRingTabBody({get creatingProject() { return creatingProject; }, set creatingProject(v) { creatingProject = v; }, onSheetPlanetClick, get sheetSlot() { return sheetSlot; }, set sheetSlot(v) { sheetSlot = v; }}, id);
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

  function onRailEscape() {
    return onRailEscapeBody({get arch() { return arch; }, set arch(v) { arch = v; }, archDo, get centreCause() { return centreCause; }, set centreCause(v) { centreCause = v; }, get claimPickerFor() { return claimPickerFor; }, set claimPickerFor(v) { claimPickerFor = v; }, closeStageEmbed, coachSkip, get creatingProject() { return creatingProject; }, set creatingProject(v) { creatingProject = v; }, depthOut, depthOutStop, get embedded() { return embedded; }, get keyOwner() { return keyOwner; }, set keyOwner(v) { keyOwner = v; }, layer2For, menuRing, openSectionId, parkStage, previewStop, get reselectStop() { return reselectStop; }, set reselectStop(v) { reselectStop = v; }, get stageEmbed() { return stageEmbed; }, set stageEmbed(v) { stageEmbed = v; }, get stageRevealed() { return stageRevealed; }, set stageRevealed(v) { stageRevealed = v; }, walkOut});
  }

  function reclaimRailRing() {
    return reclaimRailRingBody({get detachMenuKeys() { return detachMenuKeys; }, set detachMenuKeys(v) { detachMenuKeys = v; }, menuRing, onRailEscape});
  }

  function toggleTag(windowId: string, tagId: string) {
    return toggleTagBody({get liveWindowTags() { return liveWindowTags; }, set liveWindowTags(v) { liveWindowTags = v; }, persist, get windowTags() { return windowTags; }, set windowTags(v) { windowTags = v; }}, windowId, tagId);
  }

  // What a row shows: the persisted rename when one exists, else the live
  // AX title, else the app name — never blank, never a lonely "—".
  function titleFor(w: WindowInfo) {
    return titleForBody({get titleOverrides() { return titleOverrides; }, set titleOverrides(v) { titleOverrides = v; }}, w);
  }

  function startRename(w: WindowInfo) {
    return startRenameBody({get renameValue() { return renameValue; }, set renameValue(v) { renameValue = v; }, get renamingId() { return renamingId; }, set renamingId(v) { renamingId = v; }, get titleOverrides() { return titleOverrides; }, set titleOverrides(v) { titleOverrides = v; }}, w);
  }

  // The row menu — Assign to… / Rename /
  // Minimize XOR Restore / Close window / × Unclaim (claimed only).
  let ctxAssignOpen = $state(false);

  function openCtxMenu(e: MouseEvent, w: WindowInfo) {
    return openCtxMenuBody({get ctxAssignOpen() { return ctxAssignOpen; }, set ctxAssignOpen(v) { ctxAssignOpen = v; }, get ctxMenu() { return ctxMenu; }, set ctxMenu(v) { ctxMenu = v; }, get newProjectName() { return newProjectName; }, set newProjectName(v) { newProjectName = v; }}, e, w);
  }

  function ctxWindow() {
    return ctxWindowBody({get ctxMenu() { return ctxMenu; }, set ctxMenu(v) { ctxMenu = v; }, get windows() { return windows; }, set windows(v) { windows = v; }});
  }

  function ctxProject() {
    return ctxProjectBody({get ctxMenu() { return ctxMenu; }, set ctxMenu(v) { ctxMenu = v; }, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, get projects() { return projects; }, set projects(v) { projects = v; }});
  }

  function ctxAssign(projectId: string) {
    return ctxAssignBody({claim, get ctxMenu() { return ctxMenu; }, set ctxMenu(v) { ctxMenu = v; }, ctxWindow}, projectId);
  }

  function ctxCreateAndClaim() {
    return ctxCreateAndClaimBody({claim, createProject, get ctxMenu() { return ctxMenu; }, set ctxMenu(v) { ctxMenu = v; }, ctxWindow, get newProjectName() { return newProjectName; }, set newProjectName(v) { newProjectName = v; }});
  }

  function ctxRename() {
    return ctxRenameBody({get ctxMenu() { return ctxMenu; }, set ctxMenu(v) { ctxMenu = v; }, ctxWindow, startRename});
  }

  function ctxMinimize() {
    return ctxMinimizeBody({get ctxMenu() { return ctxMenu; }, set ctxMenu(v) { ctxMenu = v; }, ctxWindow, stow});
  }

  function ctxRestore() {
    return ctxRestoreBody({get ctxMenu() { return ctxMenu; }, set ctxMenu(v) { ctxMenu = v; }, ctxWindow, restore});
  }

  // Close = the AX close button only — never Complete (the circle is the
  // only Done door). Claimed windows become sleepers on the next poll.
  function ctxCloseWindow() {
    return ctxCloseWindowBody({get ctxMenu() { return ctxMenu; }, set ctxMenu(v) { ctxMenu = v; }, ctxWindow, invoke, tick});
  }

  // Unclaim ≠ Close: the OS window stays exactly where it is.
  function ctxUnclaim() {
    return ctxUnclaimBody({get ctxMenu() { return ctxMenu; }, set ctxMenu(v) { ctxMenu = v; }, ctxWindow, unclaim});
  }

  // Empty commit clears the override — the row falls back to the live title.
  function commitRename(w: WindowInfo) {
    return commitRenameBody({persist, get renameValue() { return renameValue; }, set renameValue(v) { renameValue = v; }, get renamingId() { return renamingId; }, set renamingId(v) { renamingId = v; }, get titleOverrides() { return titleOverrides; }, set titleOverrides(v) { titleOverrides = v; }}, w);
  }

  async function setWorkspaceRoot(projectId: string, path: string) {
    return setWorkspaceRootBody({invoke, get projects() { return projects; }, set projects(v) { projects = v; }}, projectId, path);
  }

  async function persist() {
    return persistBody({absorbPendingCards, get actionShortcuts() { return actionShortcuts; }, set actionShortcuts(v) { actionShortcuts = v; }, get agentRuns() { return agentRuns; }, set agentRuns(v) { agentRuns = v; }, get agentShortcuts() { return agentShortcuts; }, set agentShortcuts(v) { agentShortcuts = v; }, get agentTerminal() { return agentTerminal; }, set agentTerminal(v) { agentTerminal = v; }, get autohideRowChrome() { return autohideRowChrome; }, set autohideRowChrome(v) { autohideRowChrome = v; }, get cardTombstones() { return cardTombstones; }, set cardTombstones(v) { cardTombstones = v; }, get collapsedSectionIds() { return collapsedSectionIds; }, set collapsedSectionIds(v) { collapsedSectionIds = v; }, get companionState() { return companionState; }, set companionState(v) { companionState = v; }, get drawersState() { return drawersState; }, set drawersState(v) { drawersState = v; }, get hideAppIcons() { return hideAppIcons; }, set hideAppIcons(v) { hideAppIcons = v; }, get hideTitles() { return hideTitles; }, set hideTitles(v) { hideTitles = v; }, invoke, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, get liveWindowTags() { return liveWindowTags; }, set liveWindowTags(v) { liveWindowTags = v; }, get marbleDrift() { return marbleDrift; }, set marbleDrift(v) { marbleDrift = v; }, get neededSplit() { return neededSplit; }, set neededSplit(v) { neededSplit = v; }, get nudgeSuppressed() { return nudgeSuppressed; }, set nudgeSuppressed(v) { nudgeSuppressed = v; }, get openSubtaskIds() { return openSubtaskIds; }, set openSubtaskIds(v) { openSubtaskIds = v; }, get peerChrome() { return peerChrome; }, set peerChrome(v) { peerChrome = v; }, get pendingDisk() { return pendingDisk; }, set pendingDisk(v) { pendingDisk = v; }, get persistDeferred() { return persistDeferred; }, set persistDeferred(v) { persistDeferred = v; }, get planetIdle() { return planetIdle; }, set planetIdle(v) { planetIdle = v; }, get pointsLedger() { return pointsLedger; }, set pointsLedger(v) { pointsLedger = v; }, get previewWidth() { return previewWidth; }, set previewWidth(v) { previewWidth = v; }, get projects() { return projects; }, set projects(v) { projects = v; }, get railBoardCols() { return railBoardCols; }, set railBoardCols(v) { railBoardCols = v; }, get skyState() { return skyState; }, set skyState(v) { skyState = v; }, get spriteScale() { return spriteScale; }, set spriteScale(v) { spriteScale = v; }, get starsState() { return starsState; }, set starsState(v) { starsState = v; }, get stateLoaded() { return stateLoaded; }, set stateLoaded(v) { stateLoaded = v; }, syncHabitat, get tags() { return tags; }, set tags(v) { tags = v; }, get themeChosen() { return themeChosen; }, set themeChosen(v) { themeChosen = v; }, get themeId() { return themeId; }, set themeId(v) { themeId = v; }, get titleOverrides() { return titleOverrides; }, set titleOverrides(v) { titleOverrides = v; }, get todoTrees() { return todoTrees; }, set todoTrees(v) { todoTrees = v; }, get trackerSplit() { return trackerSplit; }, set trackerSplit(v) { trackerSplit = v; }, get treeGrowDays() { return treeGrowDays; }, set treeGrowDays(v) { treeGrowDays = v; }, windowDragHot, get windowOpacity() { return windowOpacity; }, set windowOpacity(v) { windowOpacity = v; }});
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
  function sleeperRowFor(c: Claim) {
    return sleeperRowForBody({claimStableKey}, c);
  }

  // The back-board's ticks — this project's
  // kanban (Needed cards never land), capped ~12: oldest open first,
  // then recent done. Ids + columns only; titles never ride.
  function boardCardsFor(p: Project) {
    return boardCardsForBody({get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }}, p);
  }

  function habitatEntryFor(w: WindowInfo, p: Project) {
    return habitatEntryForBody({ageClass, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }}, w, p);
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
  function runIsSeated(r: AgentRun) {
    return runIsSeatedBody({}, r);
  }

  function goToken(title: string) {
    return goTokenBody({}, title);
  }

  function goNeededOnPlanet(card: KanbanCard | undefined, planetId: string) {
    return goNeededOnPlanetBody({goToken, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }}, card, planetId);
  }

  function agentRowsFor(projectId: string) {
    return agentRowsForBody({get agentRuns() { return agentRuns; }, set agentRuns(v) { agentRuns = v; }, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, runIsSeated}, projectId);
  }

  async function locateAgent(r: AgentRun) {
    return locateAgentBody({invoke}, r);
  }

  function agentEntriesFor(p: Project) {
    return agentEntriesForBody({get agentRuns() { return agentRuns; }, set agentRuns(v) { agentRuns = v; }, get agentShortcuts() { return agentShortcuts; }, set agentShortcuts(v) { agentShortcuts = v; }, goNeededOnPlanet, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, runIsSeated}, p);
  }

  function habitatPlanetFor(p: Project, rows: WindowInfo[]) {
    return habitatPlanetForBody({agentEntriesFor, boardCardsFor, habitatEntryFor, isCollapsed, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, get todoTrees() { return todoTrees; }, set todoTrees(v) { todoTrees = v; }}, p, rows);
  }

  function habitatSnapshotPayload(planets: ReturnType<typeof habitatPlanetFor>[]) {
    return habitatSnapshotPayloadBody({habitatPlanetFor, get marbleDrift() { return marbleDrift; }, set marbleDrift(v) { marbleDrift = v; }, get projects() { return projects; }, set projects(v) { projects = v; }, get spriteScale() { return spriteScale; }, set spriteScale(v) { spriteScale = v; }, get treeGrowDays() { return treeGrowDays; }, set treeGrowDays(v) { treeGrowDays = v; }}, planets);
  }

  async function syncHabitat() {
    return syncHabitatBody({ageClass, habitatPlanetFor, habitatSnapshotPayload, invoke, get lastHabitatSig() { return lastHabitatSig; }, set lastHabitatSig(v) { lastHabitatSig = v; }, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, get marbleDrift() { return marbleDrift; }, set marbleDrift(v) { marbleDrift = v; }, occupancyOf, get packs() { return packs; }, set packs(v) { packs = v; }, get projects() { return projects; }, set projects(v) { projects = v; }, sleeperRowFor, get spriteScale() { return spriteScale; }, set spriteScale(v) { spriteScale = v; }, get starterStory() { return starterStory; }, set starterStory(v) { starterStory = v; }, get stateLoaded() { return stateLoaded; }, set stateLoaded(v) { stateLoaded = v; }, get status() { return status; }, set status(v) { status = v; }, get windows() { return windows; }, set windows(v) { windows = v; }});
  }

  // Last-known boot: same as a sleeper row but AWAKE so walkers leave
  // the igloo on first blit — not dim/still until AX. Discs still skip
  // list_windows. lastHabitatSig stays untouched so the live tick syncs.
  function lastKnownRowFor(c: Claim) {
    return lastKnownRowForBody({sleeperRowFor}, c);
  }

  async function syncHabitatLastKnown() {
    return syncHabitatLastKnownBody({get embedded() { return embedded; }, habitatPlanetFor, habitatSnapshotPayload, invoke, lastKnownRowFor, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, occupancyOf, get projects() { return projects; }, set projects(v) { projects = v; }, get stateLoaded() { return stateLoaded; }, set stateLoaded(v) { stateLoaded = v; }});
  }

  // Row left-click = locate (glow only — never
  // focus, never wake, never the Being editor). Open window → Orbitty
  // overlay ring on its AX frame; minimized claimed → walker stroke on its
  // planet; sleeper → the dim walker if it still has a planet, else a CSS
  // flash on the row. A real drag never reaches here (no click fires).
  let locateFlashIds = $state<string[]>([]);

  function flashRow(windowId: string) {
    return flashRowBody({get locateFlashIds() { return locateFlashIds; }, set locateFlashIds(v) { locateFlashIds = v; }}, windowId);
  }

  async function locateRow(w: WindowInfo) {
    return locateRowBody({flashRow, invoke, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, get projects() { return projects; }, set projects(v) { projects = v; }}, w);
  }

  async function stow(w: WindowInfo) {
    return stowBody({invoke, tick}, w);
  }

  async function restore(w: WindowInfo) {
    return restoreBody({invoke, tick, touchClaim}, w);
  }

  // Click an asleep row → wake through the claim's
  // stamps (live → file → Hermes session → recipe → app). One honest line
  // comes back either way; the click counts as claim activity.
  let wakeNote = $state<{ kind: "ok" | "err"; text: string } | null>(null);

  async function wakeSleeper(w: WindowInfo) {
    return wakeSleeperBody({invoke, tick, touchClaim, get wakeNote() { return wakeNote; }, set wakeNote(v) { wakeNote = v; }}, w);
  }

  function nudgeClaim(projectId: string) {
    return nudgeClaimBody({claim, get nudgeFor() { return nudgeFor; }, set nudgeFor(v) { nudgeFor = v; }}, projectId);
  }

  function nudgeCreate() {
    return nudgeCreateBody({claim, createProject, get nudgeFor() { return nudgeFor; }, set nudgeFor(v) { nudgeFor = v; }, get nudgeNewName() { return nudgeNewName; }, set nudgeNewName(v) { nudgeNewName = v; }});
  }

  function clampSpriteScale(n: number) {
    return clampSpriteScaleBody({}, n);
  }

  // Board and STARS share the one 780×520 `kanban`
  // overlay — the native side persists the mode, shows the pane, and emits
  // the swap; this webview (when it IS the pane) mounts KanbanPage.
  async function openKanban() {
    return openKanbanBody({invoke});
  }

  // The rail no longer opens the Sky window — the
  // word field lives inside STARS Review. The sky webview itself stays
  // (hide-not-destroy).

  // Home button: order every planet panel front (peer order preserved
  // native-side; other apps keep their relative stacking).
  async function bringAllFront() {
    return bringAllFrontBody({invoke});
  }

  // One drag source: JS startDragging on the banner. Overlay's ~28px
  // native strip is covered by this chrome, so it cannot be the handle.
  // Dual-drag (JS + data-tauri-drag-region) used to jump the rail off
  // the click — do not put the region back. × stays click-only.
  // Wordmark: click = Home, drag past 5px = move.
  let wordmarkDidDrag = false;

  function onBannerMouseDown(e: MouseEvent) {
    return onBannerMouseDownBody({startPaneDrag, get wordmarkDidDrag() { return wordmarkDidDrag; }, set wordmarkDidDrag(v) { wordmarkDidDrag = v; }}, e);
  }

  function onWordmarkClick() {
    return onWordmarkClickBody({bringAllFront, get wordmarkDidDrag() { return wordmarkDidDrag; }, set wordmarkDidDrag(v) { wordmarkDidDrag = v; }});
  }

  // Open Lists focused on one card: open_kanban puts the shared pane in
  // board mode (KanbanPage), which listens for `orbitty:focus-card`,
  // selects the card's project and opens its detail. The delayed re-emit
  // covers a kanban webview that is still mid-load (or mid page-swap) when
  // the first event lands.
  async function openKanbanFocus(cardId: string) {
    return openKanbanFocusBody({emitTo, openKanban}, cardId);
  }

  // S5 Hermes: launch/focus the EXISTING harness with a titles-only context
  // file (claimed window titles + open to-do titles — never file contents).
  async function launchHermes(p: Project) {
    return launchHermesBody({get hermesNote() { return hermesNote; }, set hermesNote(v) { hermesNote = v; }, invoke, rowsFor, todosFor}, p);
  }

  // S5 soul.md: read always; writes only through the explicit Save button.
  async function toggleSoul(p: Project) {
    return toggleSoulBody({invoke, get soulBody() { return soulBody; }, set soulBody(v) { soulBody = v; }, get soulFor() { return soulFor; }, set soulFor(v) { soulFor = v; }, get soulMsg() { return soulMsg; }, set soulMsg(v) { soulMsg = v; }, get soulPath() { return soulPath; }, set soulPath(v) { soulPath = v; }}, p);
  }

  async function saveSoul() {
    return saveSoulBody({invoke, get soulBody() { return soulBody; }, set soulBody(v) { soulBody = v; }, get soulFor() { return soulFor; }, set soulFor(v) { soulFor = v; }, get soulMsg() { return soulMsg; }, set soulMsg(v) { soulMsg = v; }});
  }

  async function openSoulEditor() {
    return openSoulEditorBody({invoke, get projects() { return projects; }, set projects(v) { projects = v; }, get soulFor() { return soulFor; }, set soulFor(v) { soulFor = v; }, get soulMsg() { return soulMsg; }, set soulMsg(v) { soulMsg = v; }});
  }

  function createProject(name: string) {
    return createProjectBody({get collapsedSectionIds() { return collapsedSectionIds; }, set collapsedSectionIds(v) { collapsedSectionIds = v; }, get projects() { return projects; }, set projects(v) { projects = v; }}, name);
  }

  function revealPlanet(projectId: string) {
    return revealPlanetBody({get collapsedSectionIds() { return collapsedSectionIds; }, set collapsedSectionIds(v) { collapsedSectionIds = v; }, sectionIds}, projectId);
  }

  function newCardId() {
    return newCardIdBody({});
  }

  function isClinicalOrMaster(windowId: string) {
    return isClinicalOrMasterBody({tagsFor}, windowId);
  }

  // Reboot-rematch fingerprint: the 1-based position of this window among
  // live windows sharing its bundle+title — the same `n` the native
  // stableKey carries. Stamped from the live AX title, never the override.
  function titleIndexFor(w: WindowInfo) {
    return titleIndexForBody({get windows() { return windows; }, set windows(v) { windows = v; }}, w);
  }

  function claimStableKey(c: Claim) {
    return claimStableKeyBody({}, c);
  }

  // Hermes-class = the bundle id or app name says so.
  function isHermesClass(w: WindowInfo) {
    return isHermesClassBody({}, w);
  }

  // Wake-trail stamps, taken from the LIVE window only (never invented):
  // the AX document path, and the Hermes session hint from the AX title.
  // Re-claiming keeps whatever a previous claim stamped (prev).
  function sleeperStamps(w: WindowInfo, prev?: Claim) {
    return sleeperStampsBody({isHermesClass}, w, prev);
  }

  // A fresh roll seed for a brand-new claim (u32 range —
  // the native side takes u64). The look derives from it until the Being
  // editor materializes explicit picks.
  function rollCharSeed() {
    return rollCharSeedBody({});
  }

  // Claim lands as Todo — walker comes from the igloo and stands at the
  // board. Doing (field walk) is an explicit column move, not the claim.
  function claim(windowId: string, projectId: string) {
    return claimBody({get claims() { return claims; }, set claims(v) { claims = v; }, isClinicalOrMaster, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, noteClaims, nowEpoch, persist, revealPlanet, rollCharSeed, sleeperStamps, titleIndexFor, get titleOverrides() { return titleOverrides; }, set titleOverrides(v) { titleOverrides = v; }, get windows() { return windows; }, set windows(v) { windows = v; }}, windowId, projectId);
  }

  // Unclaim never deletes the card: it stays on the project's list as a
  // loose todo without a window.
  function unclaim(windowId: string) {
    return unclaimBody({get claims() { return claims; }, set claims(v) { claims = v; }, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, nowEpoch, persist}, windowId);
  }

  // S4 complete — the only door that retires a sprite and can mint points.
  // Explicit click + confirm on a home-claimed row; never from dwell,
  // minimize, or title heuristics. Also AX-closes the OS window (its own
  // close button, never a pid kill); if the app refuses, the card still
  // completes — no panic.
  async function completeWindow(w: WindowInfo, project: Project) {
    return completeWindowBody({DAILY_CAP, invoke, isClinicalOrMaster, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, newCardId, nowEpoch, persist, get pointsLedger() { return pointsLedger; }, set pointsLedger(v) { pointsLedger = v; }, pointsToday, tick, get titleOverrides() { return titleOverrides; }, set titleOverrides(v) { titleOverrides = v; }, todayKey}, w, project);
  }

  // Completing a free todo from a rail project section: same points rules
  // as a window card (award once per card, daily cap), minus the walk-off.
  function completeTodo(card: KanbanCard) {
    return completeTodoBody({DAILY_CAP, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, nowEpoch, persist, get pointsLedger() { return pointsLedger; }, set pointsLedger(v) { pointsLedger = v; }, pointsToday, todayKey}, card);
  }

  // S4.6: one click on the open circle fills it (the fill is the confirm),
  // then the row completes and leaves the rail.
  function circleComplete(key: string, fn: () => void) {
    return circleCompleteBody({get completingIds() { return completingIds; }, set completingIds(v) { completingIds = v; }}, key, fn);
  }

  // A loose-todo mint is a kind-420 board proof
  // (`t=orbitty`) — the log_event match arm kinds "board-add". Fire-and-
  // forget in the same tick as the save: a log failure never blocks a mint.
  function logEvent(eType: string, content: Record<string, unknown>, projectId?: string) {
    return logEventBody({invoke}, eType, content, projectId);
  }

  // "+ To-do" under a project section: a loose todo card on that project.
  // Creating one opens Lists focused on the new card.
  function addTodoTo(projectId: string) {
    return addTodoToBody({get addingTodoFor() { return addingTodoFor; }, set addingTodoFor(v) { addingTodoFor = v; }, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, logEvent, newCardId, get newTodoTitle() { return newTodoTitle; }, set newTodoTitle(v) { newTodoTitle = v; }, nowEpoch, openKanbanFocus, persist, revealPlanet}, projectId);
  }

  // Sprite assignment: the pack id lives on the
  // claim and rides the habitat snapshot; null/"" = the generated being.
  // The picker UI is the planet walker's right-click NSMenu — the rail
  // receives `habitat:walker-pack` and writes the claim here.
  // One windowId, one projectId — occupancy IS the
  // claim. The walkerHome occupancy teleport is gone; the field stays on
  // Claim only so old saves still load, and nothing reads it.
  function occupancyOf(c: Claim) {
    return occupancyOfBody({}, c);
  }

  function setSpritePack(windowId: string, packId: string | null) {
    return setSpritePackBody({get claims() { return claims; }, set claims(v) { claims = v; }, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, persist}, windowId, packId);
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

  function beingClaim() {
    return beingClaimBody({get beingEditorFor() { return beingEditorFor; }, set beingEditorFor(v) { beingEditorFor = v; }, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }});
  }

  function beingWindow() {
    return beingWindowBody({get beingEditorFor() { return beingEditorFor; }, set beingEditorFor(v) { beingEditorFor = v; }, get windows() { return windows; }, set windows(v) { windows = v; }});
  }

  async function refreshBeingPreview() {
    return refreshBeingPreviewBody({get editorLook() { return editorLook; }, set editorLook(v) { editorLook = v; }, get editorPreview() { return editorPreview; }, set editorPreview(v) { editorPreview = v; }, invoke});
  }

  // Write the draft look (+ seed) onto the claim and redraw everywhere.
  async function commitBeingLook(seed?: number) {
    return commitBeingLookBody({get beingEditorFor() { return beingEditorFor; }, set beingEditorFor(v) { beingEditorFor = v; }, get claims() { return claims; }, set claims(v) { claims = v; }, get editorLook() { return editorLook; }, set editorLook(v) { editorLook = v; }, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, persist, refreshBeingPreview}, seed);
  }

  async function openBeingEditor(windowId: string) {
    return openBeingEditorBody({get beingEditorFor() { return beingEditorFor; }, set beingEditorFor(v) { beingEditorFor = v; }, get bittyKinds() { return bittyKinds; }, set bittyKinds(v) { bittyKinds = v; }, get claims() { return claims; }, set claims(v) { claims = v; }, commitBeingLook, get editorLook() { return editorLook; }, set editorLook(v) { editorLook = v; }, invoke, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, rollCharSeed}, windowId);
  }

  function closeBeingEditor() {
    return closeBeingEditorBody({get beingEditorFor() { return beingEditorFor; }, set beingEditorFor(v) { beingEditorFor = v; }, get editorLook() { return editorLook; }, set editorLook(v) { editorLook = v; }, get editorPreview() { return editorPreview; }, set editorPreview(v) { editorPreview = v; }});
  }

  // Kind + Palette are the only cyclers. Indices
  // wrap mod the table sizes; the palette index carries across kinds (every
  // kind has exactly 4 palettes).
  function beingKindIndex() {
    return beingKindIndexBody({get editorLook() { return editorLook; }, set editorLook(v) { editorLook = v; }});
  }

  function beingPaletteIndex() {
    return beingPaletteIndexBody({get editorLook() { return editorLook; }, set editorLook(v) { editorLook = v; }});
  }

  function cycleKind(dir: 1 | -1) {
    return cycleKindBody({beingKindIndex, beingPaletteIndex, get bittyKinds() { return bittyKinds; }, set bittyKinds(v) { bittyKinds = v; }, commitBeingLook, get editorLook() { return editorLook; }, set editorLook(v) { editorLook = v; }}, dir);
  }

  function cyclePalette(dir: 1 | -1) {
    return cyclePaletteBody({beingKindIndex, beingPaletteIndex, get bittyKinds() { return bittyKinds; }, set bittyKinds(v) { bittyKinds = v; }, commitBeingLook, get editorLook() { return editorLook; }, set editorLook(v) { editorLook = v; }}, dir);
  }

  function kindLabel() {
    return kindLabelBody({beingKindIndex, get bittyKinds() { return bittyKinds; }, set bittyKinds(v) { bittyKinds = v; }});
  }

  function paletteLabel() {
    return paletteLabelBody({beingKindIndex, beingPaletteIndex, get bittyKinds() { return bittyKinds; }, set bittyKinds(v) { bittyKinds = v; }});
  }

  async function rerollBeing() {
    return rerollBeingBody({commitBeingLook, get editorLook() { return editorLook; }, set editorLook(v) { editorLook = v; }, invoke, rollCharSeed});
  }

  // Per-walker size: null = follow the gear's global slider.
  function beingScale() {
    return beingScaleBody({beingClaim, get spriteScale() { return spriteScale; }, set spriteScale(v) { spriteScale = v; }});
  }

  function onBeingScaleInput(e: Event) {
    return onBeingScaleInputBody({get beingEditorFor() { return beingEditorFor; }, set beingEditorFor(v) { beingEditorFor = v; }, get claims() { return claims; }, set claims(v) { claims = v; }, clampSpriteScale, get lastHabitatSig() { return lastHabitatSig; }, set lastHabitatSig(v) { lastHabitatSig = v; }, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, syncHabitat}, e);
  }

  function onBeingScaleCommit() {
    return onBeingScaleCommitBody({persist});
  }

  function resetBeingScale() {
    return resetBeingScaleBody({get beingEditorFor() { return beingEditorFor; }, set beingEditorFor(v) { beingEditorFor = v; }, get claims() { return claims; }, set claims(v) { claims = v; }, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, persist});
  }

  function beingMoveHome(projectId: string) {
    return beingMoveHomeBody({get beingEditorFor() { return beingEditorFor; }, set beingEditorFor(v) { beingEditorFor = v; }, claim}, projectId);
  }

  function beingUnclaim() {
    return beingUnclaimBody({get beingEditorFor() { return beingEditorFor; }, set beingEditorFor(v) { beingEditorFor = v; }, closeBeingEditor, unclaim});
  }

  async function createProjectOnly() {
    return createProjectOnlyBody({createProject, get creatingProject() { return creatingProject; }, set creatingProject(v) { creatingProject = v; }, get embedded() { return embedded; }, invoke, get newProjectName() { return newProjectName; }, set newProjectName(v) { newProjectName = v; }, persist, get sheetSlot() { return sheetSlot; }, set sheetSlot(v) { sheetSlot = v; }});
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
    return armRemoveBody({get rmArmedFor() { return rmArmedFor; }, set rmArmedFor(v) { rmArmedFor = v; }, get rmErrorFor() { return rmErrorFor; }, set rmErrorFor(v) { rmErrorFor = v; }}, id);
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
    return removeProjectBody({invoke, get projects() { return projects; }, set projects(v) { projects = v; }, get rmArmedFor() { return rmArmedFor; }, set rmArmedFor(v) { rmArmedFor = v; }, get rmErrorFor() { return rmErrorFor; }, set rmErrorFor(v) { rmErrorFor = v; }, get sheetSlot() { return sheetSlot; }, set sheetSlot(v) { sheetSlot = v; }, syncHabitat}, p);
  }

  function onDragStart(e: DragEvent, w: WindowInfo) {
    return onDragStartBody({get ctxMenu() { return ctxMenu; }, set ctxMenu(v) { ctxMenu = v; }, get dragStartedAt() { return dragStartedAt; }, set dragStartedAt(v) { dragStartedAt = v; }, get dragWindowId() { return dragWindowId; }, set dragWindowId(v) { dragWindowId = v; }}, e, w);
  }

  function onDragEnd() {
    return onDragEndBody({absorbPendingCards, get dragStartedAt() { return dragStartedAt; }, set dragStartedAt(v) { dragStartedAt = v; }, get dragWindowId() { return dragWindowId; }, set dragWindowId(v) { dragWindowId = v; }, get dropTarget() { return dropTarget; }, set dropTarget(v) { dropTarget = v; }, get pendingDisk() { return pendingDisk; }, set pendingDisk(v) { pendingDisk = v; }, persist, get persistDeferred() { return persistDeferred; }, set persistDeferred(v) { persistDeferred = v; }, windowDragHot});
  }

  // Window drags must keep working even if Svelte state was lost mid-gesture
  // (the 800ms poll can mutate the source row and cancel the HTML5 session):
  // trust the dataTransfer type list, not just dragWindowId.
  function isWindowDrag(e: DragEvent) {
    return isWindowDragBody({get dragWindowId() { return dragWindowId; }, set dragWindowId(v) { dragWindowId = v; }}, e);
  }

  function onSectionDragEnter(e: DragEvent, key: string) {
    return onSectionDragEnterBody({get dropTarget() { return dropTarget; }, set dropTarget(v) { dropTarget = v; }, isWindowDrag}, e, key);
  }

  function onSectionDragOver(e: DragEvent, key: string) {
    return onSectionDragOverBody({get dropTarget() { return dropTarget; }, set dropTarget(v) { dropTarget = v; }, isWindowDrag}, e, key);
  }

  function onSectionDragLeave(e: DragEvent, key: string) {
    return onSectionDragLeaveBody({get dropTarget() { return dropTarget; }, set dropTarget(v) { dropTarget = v; }}, e, key);
  }

  function onSectionDrop(e: DragEvent, projectId: string | null) {
    return onSectionDropBody({claim, get dragWindowId() { return dragWindowId; }, set dragWindowId(v) { dragWindowId = v; }, get dropTarget() { return dropTarget; }, set dropTarget(v) { dropTarget = v; }, unclaim}, e, projectId);
  }

  async function tick() {
    return tickBody({checkNudge, get dragWindowId() { return dragWindowId; }, set dragWindowId(v) { dragWindowId = v; }, get embedded() { return embedded; }, healOrphanWindowCards, invoke, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, noteAx, persist, get refreshedAt() { return refreshedAt; }, set refreshedAt(v) { refreshedAt = v; }, rematchClaims, syncHabitat, touchClaim, windowDragHot, get windows() { return windows; }, set windows(v) { windows = v; }, get windowsPolled() { return windowsPolled; }, set windowsPolled(v) { windowsPolled = v; }});
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
    return rematchClaimsBody({claimStableKey, isHermesClass, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, persist, rewriteWindowId, get stateLoaded() { return stateLoaded; }, set stateLoaded(v) { stateLoaded = v; }, titleIndexFor, get windows() { return windows; }, set windows(v) { windows = v; }});
  }

  // One id swap, all slices: claim (with a refreshed fingerprint and wake
  // trail), cards, tags, title overrides, nudge suppression.
  function rewriteWindowId(oldId: string, w: WindowInfo) {
    return rewriteWindowIdBody({get claims() { return claims; }, set claims(v) { claims = v; }, isHermesClass, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, nowEpoch, get nudgeSuppressed() { return nudgeSuppressed; }, set nudgeSuppressed(v) { nudgeSuppressed = v; }, titleIndexFor, get titleOverrides() { return titleOverrides; }, set titleOverrides(v) { titleOverrides = v; }, get windowTags() { return windowTags; }, set windowTags(v) { windowTags = v; }}, oldId, w);
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

  function healOrphanWindowCards() {
    return healOrphanWindowCardsBody({claim, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, nowEpoch, get projects() { return projects; }, set projects(v) { projects = v; }, get stateLoaded() { return stateLoaded; }, set stateLoaded(v) { stateLoaded = v; }, get windows() { return windows; }, set windows(v) { windows = v; }, get windowsPolled() { return windowsPolled; }, set windowsPolled(v) { windowsPolled = v; }});
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
    return checkNudgeBody({get liveClaims() { return liveClaims; }, set liveClaims(v) { liveClaims = v; }, get nudgeFor() { return nudgeFor; }, set nudgeFor(v) { nudgeFor = v; }, get nudgeNewName() { return nudgeNewName; }, set nudgeNewName(v) { nudgeNewName = v; }, get nudgePrimed() { return nudgePrimed; }, set nudgePrimed(v) { nudgePrimed = v; }, get nudgeSuppressed() { return nudgeSuppressed; }, set nudgeSuppressed(v) { nudgeSuppressed = v; }, persist, get prevMinimized() { return prevMinimized; }, set prevMinimized(v) { prevMinimized = v; }, get stateLoaded() { return stateLoaded; }, set stateLoaded(v) { stateLoaded = v; }, get windows() { return windows; }, set windows(v) { windows = v; }});
  }

  // Rail drag lives on the banner (onBannerMouseDown). Overlay's native
  // strip is covered by this chrome, so it cannot be the handle.

  async function openSettings() {
    return openSettingsBody({invoke});
  }

  async function retry() {
    return retryBody({invoke, tick});
  }

  async function copyPath() {
    return copyPathBody({get copied() { return copied; }, set copied(v) { copied = v; }, get status() { return status; }, set status(v) { status = v; }});
  }

  $effect(() => {
    if (!themeId) return;
    document.documentElement.dataset.theme = themeId;
    applyHostTokens(themeId);
    const light = systemTokens(themeId)?.colorScheme === "light";
    invoke("set_vibrancy", { enabled: true, light }).catch(() => {});
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
  let unlistenMouse: (() => void) | undefined;
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
    window.addEventListener("keydown", onExpandedFieldEscape);
    window.addEventListener("wheel", onNoteWheel, { passive: true });
    detachMenuKeys = menuRing.attach(onRailEscape);
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
      noteClaims(claims);
    } catch {
      themeId = defaultThemeId();
      noteClaims(claims);
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
    unlistenStage = await listen<{ revealed?: boolean; source?: string }>("orbitty:rail-stage", (e) => {
      const revealed = !!e.payload?.revealed;
      stageRevealed = revealed;
      if (revealed) {
        revealGraceUntil = Date.now() + EDGE_GRACE_MS;
        if (e.payload?.source === "edge") {
          pointerInside = true;
          edgePointerSeed = true;
          setTimeout(() => {
            if (!edgePointerSeed) return;
            edgePointerSeed = false;
            pointerInside = false;
            maybeAutoHide();
          }, EDGE_GRACE_MS);
        }
      } else {
        edgePointerSeed = false;
      }
      applyEdge({ kind: revealed ? "revealed" : "parked" });
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
    unlistenMouse = await listen<number>("rail:mouse-button", (e) => {
      const button = domButtonFromButtonNumber(e.payload);
      if (button == null || embedded || !stageRevealed) return;
      takeMouseButton(button);
    });
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
        noteClaims(claims);
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
    if (edgeTimer) clearTimeout(edgeTimer);
    if (claimFlashTimer) clearTimeout(claimFlashTimer);
    motionMedia.removeEventListener("change", onMotionChange);
    window.removeEventListener("keydown", onNoteKey);
    window.removeEventListener("keydown", onExpandedFieldEscape);
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
    unlistenMouse?.();
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
    get stageEmbed() { return stageEmbed; },
    closeStageEmbed,
    applyBeside,
    get keyOwner() { return keyOwner; },
    walkInto,
    walkOut,
    popOutSettings,
    popOutBoard,
    get claimFlash() { return claimFlash; },
    archDo,
    planetCards,
    focusBoardLanding,
    noteWheelSpin,
    wheelAnimating,
    get spinCenter() { return spinCenter; },
    set spinCenter(v: number) { spinCenter = v; },
    get centreCause() { return centreCause; },
    set centreCause(v: CentreCause) { centreCause = v; },
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
    popOutStars,
    parkAfterRaise,
    armNeededMouth,
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
  onpointerup={onStagePointerUp}
  onauxclick={onStageAux}
  onmouseleave={(e) => onStageMouseLeave(e)}
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
