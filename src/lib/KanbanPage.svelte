<script lang="ts">
  // S4.4 Lists window — board only: Hive-like Todo · Doing · Done columns
  // with drag-between-columns status moves. Clicking a card opens a
  // Habitica-thin detail drawer. Same state.json discipline: every
  // mutation is load → mutate → save so rail-owned slices are never
  // clobbered. Needed stays its own column in front of Todo (questions,
  // not quests). Mint a todo from the companion composer
  // (`todo: … -planet --agent`).
  import { invoke } from "@tauri-apps/api/core";
  import { emitTo, listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy, untrack } from "svelte";
  import { applyHostTokens, applyWindowOpacity, clampWindowOpacity } from "../themes/apply";
  import { listenPaneVis } from "./paneVis";
  import { planetCss } from "./planetColor";
  import { boardTitleDot, boardWindowTitle } from "./windowChrome";
  import { reorderProjects } from "./planetOrder";
  import { startPaneDrag } from "./dragPane";
  import NeededInbox, { neededFor } from "./NeededInbox.svelte";
  import BoardChrome from "./board/Chrome.svelte";
  import BoardColumns from "./board/Columns.svelte";
  import BoardPreview from "./board/Preview.svelte";
  import PlanetColumn from "./board/PlanetColumn.svelte";
  import { resolveFocus } from "./board/focusProject";
  import {
    beforeIdAt,
    columnArrow,
    columnMode,
    columnPx,
  } from "./board/planetColumn";
  import "./board/board.css";
  import { MenuRing, type RingDir, type RingStop } from "./menuKeys.svelte";
  import { mdLite } from "./mdLite";
  import { linkErrText, requestLink } from "./linkOpen.svelte";
  import "./chrome.css";
  import "./boardStack.css";
  import "./mdLite.css";
  import type {
    Actor,
    AgentRun,
    AgentShortcut,
    Claim,
    KanbanCard,
    NeededState,
    PersistedState,
    PointsEntry,
    Project,
    Tag,
    TitleOverride,
    WindowInfo,
  } from "./persist";
  import { raisePlanet as raisePlanetBody, projectIdList as projectIdListBody, applyFocusProject as applyFocusProjectBody, applyPendingFocus as applyPendingFocusBody, onPlanetTabClick as onPlanetTabClickBody, reorderPlanetsLive as reorderPlanetsLiveBody, persistPlanetOrder as persistPlanetOrderBody, planetBeforeAt as planetBeforeAtBody, onPlanetPointerDown as onPlanetPointerDownBody, toggleDoneChip as toggleDoneChipBody, onPlanetTabDblClick as onPlanetTabDblClickBody } from "./board/pagePlanets.svelte";
  import { todayKey as todayKeyBody, startOfToday as startOfTodayBody, isDueToday as isDueTodayBody, pointsToday as pointsTodayBody, listOf as listOfBody, actorSide as actorSideBody, inDoneFilter as inDoneFilterBody, livePlanetId as livePlanetIdBody, inScope as inScopeBody } from "./board/pageScope.svelte";
  import { startGrab as startGrabBody, grabArrow as grabArrowBody, dropGrab as dropGrabBody, persistColumnOrder as persistColumnOrderBody, ungrab as ungrabBody } from "./board/pageGrab.svelte";
  import { sideCount as sideCountBody, projectName as projectNameBody, winFor as winForBody, doneChip as doneChipBody, notesSnippet as notesSnippetBody, subProgress as subProgressBody, agentName as agentNameBody, runState as runStateBody, runIsLive as runIsLiveBody, isWriting as isWritingBody, planetIsLive as planetIsLiveBody, hasAgent as hasAgentBody, goToken as goTokenBody, neededForTodo as neededForTodoBody, isDoingRow as isDoingRowBody, ageLabel as ageLabelBody, dueInfo as dueInfoBody, showProj as showProjBody, osThemeId as osThemeIdBody, loadState as loadStateBody } from "./board/pageChips.svelte";
  import DockCardPop from "./companion/DockCardPop.svelte";
  import { logEvent as logEventBody, mutate as mutateBody, errText as errTextBody, showLinkToast as showLinkToastBody, openCardLink as openCardLinkBody, openUrl as openUrlBody, addCardLink as addCardLinkBody, removeCardLink as removeCardLinkBody, completeCard as completeCardBody, toggleCard as toggleCardBody, moveCard as moveCardBody, openDetail as openDetailBody, toggleDoing as toggleDoingBody, skipNeededCard as skipNeededCardBody, laterNeededCard as laterNeededCardBody, onSplitDown as onSplitDownBody, columnWidthNow as columnWidthNowBody, mailMinPx as mailMinPxBody, togglePlanetCol as togglePlanetColBody, previewMinPx as previewMinPxBody, clampPreview as clampPreviewBody, onEastSplitDown as onEastSplitDownBody, resetPreviewWidth as resetPreviewWidthBody } from "./board/pageMutate.svelte";
  import { drawerStops as drawerStopsBody, saveTitle as saveTitleBody, undoThisTask as undoThisTaskBody, saveNotes as saveNotesBody, setPlanet as setPlanetBody, setAgent as setAgentBody, newCardId as newCardIdBody, childrenOf as childrenOfBody, remainingKids as remainingKidsBody, toggleOpenSubtasks as toggleOpenSubtasksBody, wouldCycle as wouldCycleBody, linkableTodos as linkableTodosBody, mintTodo as mintTodoBody, attachChild as attachChildBody, detachChild as detachChildBody, setDue as setDueBody, dateInputValue as dateInputValueBody, cardHasTag as cardHasTagBody, addTagToCard as addTagToCardBody, toggleCardTag as toggleCardTagBody, toggleSubtask as toggleSubtaskBody, deleteTodo as deleteTodoBody, setRating as setRatingBody, deleteSubtask as deleteSubtaskBody } from "./board/pageFields.svelte";
  import { onCardDragStart as onCardDragStartBody, onCardDragEnd as onCardDragEndBody, isCardDrag as isCardDragBody, onColDragOver as onColDragOverBody, onColDragLeave as onColDragLeaveBody, onColDrop as onColDropBody, onCardPointerDown as onCardPointerDownBody, onPtrMove as onPtrMoveBody, onPtrUp as onPtrUpBody, onPtrCancel as onPtrCancelBody, ptrEnd as ptrEndBody, onPtrKey as onPtrKeyBody, overStartStrip as overStartStripBody, colAtPoint as colAtPointBody, rowAtPoint as rowAtPointBody, onCardClick as onCardClickBody, refreshWindows as refreshWindowsBody, focusCard as focusCardBody, launchHermes as launchHermesBody } from "./board/pagePointer.svelte";


  // Embedded mounts this page in the companion's
  // Launchpad well — × calls onClose (never hide_window), the banner
  // wordmark is Launchpad, and a pop-out control (onPopout) hands the
  // board back to the real kanban window. The overlay passes nothing.
  let {
    embedded = false,
    onClose,
    onPopout,
    onNeededFocus,
    onNeededAnswer,
    railPlanet = undefined,
    ringActive = false,
    onRingExit,
    outwardDir = "left" as RingDir,
  }: {
    embedded?: boolean;
    onClose?: () => void;
    onPopout?: () => void;
    onNeededFocus?: (id: string | null, title: string | null) => void;
    // MENU-KEYS: Enter on a Needed row — the host lands the caret in the
    // composer so the next typed line answers it.
    onNeededAnswer?: () => void;
    // Stage rail embed. Absent = the Lists window and the Launchpad well.
    railPlanet?: string;
    ringActive?: boolean;
    onRingExit?: () => void;
    outwardDir?: RingDir;
  } = $props();

  type Status = "todo" | "doing" | "done";
  // Happened chip filter: All default; mine = user|missing; behalf =
  // agent|action|vps. Session-only, like the other view state.
  type DoneFilter = "all" | "mine" | "behalf";

  const DAILY_CAP = 12;
  const COLUMNS: { id: Status; name: string; hint: string }[] = [
    { id: "todo", name: "Todo", hint: "No to-dos — drag a card here." },
    { id: "doing", name: "Doing", hint: "Nothing in progress — drag a card here." },
    { id: "done", name: "Done", hint: "Nothing done yet — check a to-do." },
  ];

  // Seed the OS theme synchronously so --bg exists
  // before load_state lands — an empty themeId on an opaque drawer is a
  // white-on-white first paint. loadState overrides with the saved id.
  let themeId = $state(osThemeId());
  let windowOpacity = $state(1.0);
  let projects = $state<Project[]>([]);
  let tags = $state<Tag[]>([]);
  let kanbanCards = $state<KanbanCard[]>([]);
  let pointsLedger = $state<PointsEntry[]>([]);
  let windows = $state<WindowInfo[]>([]);
  let titleOverrides = $state<TitleOverride[]>([]);
  let agentShortcuts = $state<AgentShortcut[]>([]);
  let agentRuns = $state<AgentRun[]>([]);
  // writer on a planet glows that chip in the
  // planet column (same flag the inspector sheet tabs already read).
  let hackProjectId = $state<string | null>(null);
  let selectedBeforeDone = $state("inbox");
  let neededSplit = $state(0.28);
  let previewWidth = $state(280);
  let mailEl: HTMLDivElement | null = $state(null);
  let bodyEl: HTMLDivElement | null = $state(null);
  let kanbanEl: HTMLElement | null = $state(null);
  // Session-only. "auto" collapses under COLUMN_AUTO_MIN_PX; a click wins.
  let planetColPref = $state<"auto" | "open" | "icons" | "hidden">("auto");
  let boardPx = $state(900);
  const planetColMode = $derived(columnMode(boardPx, planetColPref));
  let addingTodo = $state(false);
  let newTodoTitle = $state("");
  let openSubtaskIds = $state<string[]>([]);
  let newTagName = $state("");
  let neededEl: HTMLDivElement | null = $state(null);
  let migratingCardTrackers = false;
  let previewEdit = $state(false);
  let planetDdOpen = $state(false);
  let planetDragId = $state<string | null>(null);
  let planetDropId = $state<string | null>(null);
  let planetDidDrag = false;

  // No Actions well here — actions are a Settings
  // list + `orbittyctl action` verbs; the slice still round-trips via
  // load → mutate → save.
  // Shared so the Needed composer stays one toggle.
  let addingNeeded = $state(false);

  // Sidebar selection: a smart filter, Inbox, or a project id. Session-only.
  let selected = $state("inbox");
  // Rail embed: the wheel owns the planet. No raise, no focus-project apply.
  $effect(() => {
    if (railPlanet == null) return;
    if (selected !== railPlanet) selected = railPlanet;
    if (planetColPref !== "hidden") planetColPref = "hidden";
  });
  /** BOARD-MARK: apply after loadState / pane show (projects may be empty on first emit). */
  let pendingFocusProject = $state<string | null>(null);
  // PLANETS-STRIP: rail-owned marble flag, read-only here — tab double-click
  // emits to main; the rail persist is the writer.
  let collapsedSectionIds = $state<string[]>([]);

  let detailId = $state<string | null>(null);
  let linkToast = $state("");
  let linkToastTimer: ReturnType<typeof setTimeout> | undefined;

  // Launchpad ring: arrows walk, Enter opens, Esc backs, Shift+Enter grabs.
  const menuRing = new MenuRing();
  let neededSelectedId = $state<string | null>(null);
  // LP-SPLIT: the Needed pane filters with the planet tab — Inbox and
  // the smart tabs ("") paint the consolidated all-planets pile, a
  // planet tab paints that planet's Neededs. neededFor is the one
  // filter; the pane paints and this ring walks the SAME rows.
  const neededProjectId = $derived(
    projects.some((p) => p.id === selected) ? selected : "",
  );
  const neededRows = $derived(neededFor(kanbanCards, neededProjectId));
  // Tracks the drawer surface's push/pop pairing on the ring.
  let drawerOnRing = false;

  $effect(() => {
    if (!ringActive) return;
    const outward = outwardDir;
    menuRing.onEdge = (dir) => {
      if (dir !== outward) return false;
      onRingExit?.();
      return true;
    };
    detachMenuKeys?.();
    detachMenuKeys = menuRing.attach(() => {
      if (grabbedId) {
        ungrab();
        return true;
      }
      onRingExit?.();
      return true;
    });
    return () => {
      detachMenuKeys?.();
      detachMenuKeys = undefined;
      menuRing.onEdge = null;
    };
  });

  $effect(() => {
    const drawer = detailId
      ? kanbanCards.find((c) => c.id === detailId)
      : undefined;
    if (detailId && !drawer) detailId = null;
    // Preview is always on — not a modal surface. Drag stays on the
    // board ring. Esc does not close the pane.
    if (drawerOnRing) {
      drawerOnRing = false;
      menuRing.popSurface();
    }
    // KANBAN-GRAB: the held card left the board (a state reload, a
    // sidebar flip mid-hold) — let go rather than pin a ghost.
    if (
      grabbedId &&
      !COLUMNS.some((c) => boardCols[c.id].some((k) => k.id === grabbedId))
    ) {
      ungrab();
    }
    const exitActive = ringActive;
    const exitDir = outwardDir;
    const exit = onRingExit;
    const stops: RingStop[] = [];
    // Planet column is one surface, and the first one, so Tab walks
    // planets → Needed → Todo → Doing → Done. Up/down walk the rows
    // and wrap. Right drops into the first card of the first non-empty
    // column, else the first Needed row. Left stays put.
    const tabIds = ["inbox", "today", "done", ...projects.map((p) => p.id)];
    const firstCard =
      COLUMNS.map((c) => boardCols[c.id]).find((l) => l.length > 0)?.[0] ??
      null;
    const firstNeededId = neededRows[0]?.id ?? null;
    for (const id of tabIds) {
      stops.push({
        id: `listtab-${id}`,
        group: "tabs",
        confirm: () => {
          if (projects.some((p) => p.id === id)) onPlanetTabClick(id);
          else selected = id;
        },
        arrow: (dir) => {
          const next = columnArrow(tabIds, id, dir, {
            firstCardId: firstCard?.id ?? null,
            firstNeededId,
          });
          if (exitActive && dir === exitDir && next == null) {
            exit?.();
            return true;
          }
          if (next == null) return true;
          if (dir === "right") {
            if (firstCard) menuRing.select(`card-${firstCard.id}`);
            else menuRing.select(`needed-${next}`);
            return true;
          }
          menuRing.select(`listtab-${next}`);
          return true;
        },
      });
    }
    // The Needed column is always open — the pile rows ride the ring
    // directly (no toggle stop). Enter on a row arms the composer
    // answer (the companion text box is the mouth) and lands the caret.
    for (const c of neededRows) {
      stops.push({
        id: `needed-${c.id}`,
        group: "needed",
        confirm: () => {
          neededSelectedId = c.id;
          armNeededMouth(c.id, c.title);
          onNeededAnswer?.();
        },
      });
    }
    const cols = COLUMNS.map((c) => boardCols[c.id]);
    cols.forEach((colCards, ci) => {
      colCards.forEach((card, ri) => {
        stops.push({
          id: `card-${card.id}`,
          group: COLUMNS[ci].id,
          confirm: () => {
            // KANBAN-GRAB: Enter (pad A) on the held card is the drop,
            // not the drawer. The highlight is pinned to the held card,
            // so no other stop can take this branch.
            if (grabbedId) {
              if (card.id === grabbedId) dropGrab();
              return;
            }
            openDetail(card);
          },
          arrow: (dir) => {
            // KANBAN-GRAB: while held, arrows move the CARD — the ring's
            // default walk never runs, the highlight stays pinned.
            if (grabbedId) {
              if (card.id === grabbedId) grabArrow(dir);
              return true;
            }
            if (exitActive && dir === exitDir && (dir === "left" || dir === "right")) {
              const step = dir === "left" ? -1 : 1;
              if (ci + step < 0 || ci + step >= cols.length) {
                exit?.();
                return true;
              }
            }
            if (dir !== "left" && dir !== "right") return false;
            const step = dir === "left" ? -1 : 1;
            for (let k = 1; k < cols.length; k++) {
              const t = (((ci + step * k) % cols.length) + cols.length) % cols.length;
              if (cols[t].length === 0) continue;
              const target = cols[t][Math.min(ri, cols[t].length - 1)];
              menuRing.select(`card-${target.id}`);
              break;
            }
            return true;
          },
          // X (Shift+Enter): pick up this Todo — it follows the focus.
          grab: () => startGrab(card),
        });
      });
    });
    menuRing.setStops(stops);
  });

  let detachMenuKeys: (() => void) | undefined;

  // Board drag: card id in flight + column under the cursor.
  let dragCardId = $state<string | null>(null);
  let dropCol = $state<Status | null>(null);

  // S5: Hermes launch/focus for the selected project list (titles only).
  let hermesNote = $state<{ kind: "ok" | "err"; text: string } | null>(null);
  let selectedProject = $derived(projects.find((p) => p.id === selected));

  function raisePlanet(id: string) {
    return raisePlanetBody({invoke}, id);
  }


  function projectIdList() {
    return projectIdListBody({get projects() { return projects; }, set projects(v) { projects = v; }});
  }

  function applyFocusProject(raw: string | null | undefined) {
    return applyFocusProjectBody({get pendingFocusProject() { return pendingFocusProject; }, set pendingFocusProject(v) { pendingFocusProject = v; }, projectIdList, get projects() { return projects; }, set projects(v) { projects = v; }, raisePlanet, resolveFocus, get selected() { return selected; }, set selected(v) { selected = v; }}, raw);
  }

  /** loadState / pane show: apply a still-pending board click once, then clear.
   * A loaded list that lacks the id drops it (deleted planet) and does not
   * move the selection. */
  function applyPendingFocus() {
    return applyPendingFocusBody({get pendingFocusProject() { return pendingFocusProject; }, set pendingFocusProject(v) { pendingFocusProject = v; }, projectIdList, get projects() { return projects; }, set projects(v) { projects = v; }, raisePlanet, resolveFocus, get selected() { return selected; }, set selected(v) { selected = v; }});
  }

  function onPlanetTabClick(id: string) {
    return onPlanetTabClickBody({get pendingFocusProject() { return pendingFocusProject; }, set pendingFocusProject(v) { pendingFocusProject = v; }, get planetDidDrag() { return planetDidDrag; }, set planetDidDrag(v) { planetDidDrag = v; }, raisePlanet, get selected() { return selected; }, set selected(v) { selected = v; }}, id);
  }

  function reorderPlanetsLive(fromId: string, beforeId: string | null) {
    return reorderPlanetsLiveBody({get projects() { return projects; }, set projects(v) { projects = v; }, reorderProjects}, fromId, beforeId);
  }

  function persistPlanetOrder() {
    return persistPlanetOrderBody({mutate, get projects() { return projects; }, set projects(v) { projects = v; }});
  }

  function planetBeforeAt(clientY: number) {
    return planetBeforeAtBody({beforeIdAt}, clientY);
  }

  function onPlanetPointerDown(e: PointerEvent, id: string) {
    return onPlanetPointerDownBody({persistPlanetOrder, planetBeforeAt, get planetDidDrag() { return planetDidDrag; }, set planetDidDrag(v) { planetDidDrag = v; }, get planetDragId() { return planetDragId; }, set planetDragId(v) { planetDragId = v; }, get planetDropId() { return planetDropId; }, set planetDropId(v) { planetDropId = v; }, reorderPlanetsLive}, e, id);
  }

  function toggleDoneChip() {
    return toggleDoneChipBody({get selected() { return selected; }, set selected(v) { selected = v; }, get selectedBeforeDone() { return selectedBeforeDone; }, set selectedBeforeDone(v) { selectedBeforeDone = v; }});
  }

  function onPlanetTabDblClick(id: string) {
    return onPlanetTabDblClickBody({get collapsedSectionIds() { return collapsedSectionIds; }, set collapsedSectionIds(v) { collapsedSectionIds = v; }, emitTo}, id);
  }

  function todayKey() {
    return todayKeyBody({});
  }

  function startOfToday() {
    return startOfTodayBody({});
  }

  function isDueToday(c: KanbanCard) {
    return isDueTodayBody({startOfToday}, c);
  }

  function pointsToday() {
    return pointsTodayBody({get pointsLedger() { return pointsLedger; }, set pointsLedger(v) { pointsLedger = v; }, todayKey});
  }

  // A card whose project vanished falls back to Inbox — never invisible.
  function listOf(c: KanbanCard) {
    return listOfBody({get projects() { return projects; }, set projects(v) { projects = v; }}, c);
  }

  // Needed is never a quest: every quest view filters kind first (Needed
  // cards carry a dummy status "todo" only so kind-blind code can't crash).
  // LP-INBOX: kind "window" never paints on Launchpad — the Planets sheet
  // owns windows.
  let openCards = $derived(
    kanbanCards.filter(
      (c) => c.kind !== "needed" && c.kind !== "window" && c.status !== "done",
    ),
  );
  let doneCards = $derived(
    kanbanCards
      .filter(
        (c) => c.kind !== "needed" && c.kind !== "window" && c.status === "done" && c.completedAt,
      )
      .sort((a, b) => (b.completedAt ?? 0) - (a.completedAt ?? 0)),
  );

  // Happened chips (sidebar Done + Board Done header).
  let doneFilter = $state<DoneFilter>("all");

  function actorSide(card: KanbanCard) {
    return actorSideBody({}, card);
  }

  function inDoneFilter(c: KanbanCard) {
    return inDoneFilterBody({actorSide, get doneFilter() { return doneFilter; }, set doneFilter(v) { doneFilter = v; }}, c);
  }

  // LP-INBOX: planet color dot — only when projectId is a LIVE project
  // (a vanished project falls back to Inbox and gets no dot).
  function livePlanetId(card: KanbanCard) {
    return livePlanetIdBody({get projects() { return projects; }, set projects(v) { projects = v; }}, card);
  }

  // Board scope mirrors the sidebar selection; the Done smart filter boards
  // every card so all three columns stay meaningful.
  function inScope(c: KanbanCard) {
    return inScopeBody({isDueToday, listOf, get selected() { return selected; }, set selected(v) { selected = v; }}, c);
  }

  let boardCols = $derived.by(() => {
    // Array order is the persisted lane order (persistColumnOrder).
    // Age-sort fought drag and the needed-doing pin.
    const scoped = kanbanCards.filter((c) => inScope(c) && !c.parentId);
    return {
      todo: scoped.filter((c) => c.status === "todo"),
      doing: scoped.filter((c) => c.status === "doing"),
      done: scoped
        .filter((c) => c.status === "done")
        .filter(inDoneFilter)
        .sort((a, b) => (b.completedAt ?? 0) - (a.completedAt ?? 0)),
    } as Record<Status, KanbanCard[]>;
  });

  // The held card. grabbedId pins the ring
  // highlight; grabCol/grabIdx are the grab cursor the card paints at
  // (viewCols). Nothing mutates until Enter drops — Esc just lets go.
  let grabbedId = $state<string | null>(null);
  let grabCol = $state<Status | null>(null);
  let grabIdx = $state(0);

  // While held, the card paints at the grab cursor instead of its real
  // column — the board itself is unchanged until the drop. Not held:
  // viewCols IS boardCols (same reference, no churn).
  // Pointer drag floats the card (grabGhost); the list stays put so
  // the tile does not jump slot-to-slot. Keyboard grab still paints
  // the card at the grab cursor.
  let grabGhost = $state<{
    x: number;
    y: number;
    w: number;
    h: number;
  } | null>(null);

  let viewCols = $derived.by(() => {
    if (!grabbedId || !grabCol) return boardCols;
    const card = kanbanCards.find((c) => c.id === grabbedId);
    // Keep the Start strip up while the card is floating over it.
    // Same-column reorder still moves the slot.
    if (grabGhost && card && grabCol !== card.status) return boardCols;
    if (!card) return boardCols;
    const out: Record<Status, KanbanCard[]> = {
      todo: boardCols.todo.filter((c) => c.id !== grabbedId),
      doing: boardCols.doing.filter((c) => c.id !== grabbedId),
      done: boardCols.done.filter((c) => c.id !== grabbedId),
    };
    const list = [...out[grabCol]];
    list.splice(Math.min(grabIdx, list.length), 0, card);
    out[grabCol] = list;
    return out;
  });

  $effect(() => {
    if (neededSelectedId) detailId = neededSelectedId;
  });

  // Overlay Launchpad is its own window — the embedded onNeededFocus
  // prop is not this path. Native raise makes companion key; the
  // companion webview then lands the caret. Do not fire on boot.
  let dockCard = $state<KanbanCard | null>(null);

  function armNeededMouth(id: string | null, title: string | null) {
    if (embedded && id) {
      dockCard = kanbanCards.find((card) => card.id === id) ?? dockCard;
    }
    if (!id) dockCard = null;
    onNeededFocus?.(id, title);
    emitTo("companion", "orbitty:needed-focus", { id, title }).catch(() => {});
    if (id) {
      invoke("arm_companion_mouth", { id, title }).catch(() => {});
    }
  }

  $effect(() => {
    const pile = [...viewCols.doing, ...viewCols.todo, ...neededRows];
    if (detailId && pile.some((c) => c.id === detailId)) return;
    if (
      detailId &&
      selected === "done" &&
      viewCols.done.some((c) => c.id === detailId)
    )
      return;
    detailId = pile[0]?.id ?? null;
  });

  function startGrab(card: KanbanCard) {
    return startGrabBody({get boardCols() { return boardCols; }, set boardCols(v) { boardCols = v; }, get grabCol() { return grabCol; }, set grabCol(v) { grabCol = v; }, get grabIdx() { return grabIdx; }, set grabIdx(v) { grabIdx = v; }, get grabbedId() { return grabbedId; }, set grabbedId(v) { grabbedId = v; }}, card);
  }

  function grabArrow(dir: RingDir) {
    return grabArrowBody({COLUMNS, get boardCols() { return boardCols; }, set boardCols(v) { boardCols = v; }, get grabCol() { return grabCol; }, set grabCol(v) { grabCol = v; }, get grabIdx() { return grabIdx; }, set grabIdx(v) { grabIdx = v; }, get grabbedId() { return grabbedId; }, set grabbedId(v) { grabbedId = v; }, menuRing}, dir);
  }

  function dropGrab() {
    return dropGrabBody({get grabCol() { return grabCol; }, set grabCol(v) { grabCol = v; }, get grabGhost() { return grabGhost; }, set grabGhost(v) { grabGhost = v; }, get grabIdx() { return grabIdx; }, set grabIdx(v) { grabIdx = v; }, get grabbedId() { return grabbedId; }, set grabbedId(v) { grabbedId = v; }, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, moveCard, persistColumnOrder});
  }

  function persistColumnOrder(card: KanbanCard, col: Status, toIdx: number) {
    return persistColumnOrderBody({inScope, mutate}, card, col, toIdx);
  }

  function ungrab() {
    return ungrabBody({get grabCol() { return grabCol; }, set grabCol(v) { grabCol = v; }, get grabGhost() { return grabGhost; }, set grabGhost(v) { grabGhost = v; }, get grabIdx() { return grabIdx; }, set grabIdx(v) { grabIdx = v; }, get grabbedId() { return grabbedId; }, set grabbedId(v) { grabbedId = v; }});
  }

  let detailCard = $derived(
    detailId ? kanbanCards.find((c) => c.id === detailId) : undefined,
  );

  function sideCount(list: string) {
    return sideCountBody({get doneCards() { return doneCards; }, set doneCards(v) { doneCards = v; }, isDueToday, listOf, get openCards() { return openCards; }, set openCards(v) { openCards = v; }}, list);
  }

  let listTitle = $derived(boardWindowTitle(selected, projects));
  let titleDot = $derived(boardTitleDot(selected, projects));

  function projectName(id: string) {
    return projectNameBody({get projects() { return projects; }, set projects(v) { projects = v; }}, id);
  }

  function winFor(card: KanbanCard) {
    return winForBody({get windows() { return windows; }, set windows(v) { windows = v; }}, card);
  }

  function doneChip(card: KanbanCard) {
    return doneChipBody({startOfToday}, card);
  }

  // row meta (notes snippet, difficulty pip, due chip, sub count)

  function notesSnippet(card: KanbanCard) {
    return notesSnippetBody({}, card);
  }

  function subProgress(card: KanbanCard) {
    return subProgressBody({}, card);
  }

  function agentName(card: KanbanCard) {
    return agentNameBody({get agentShortcuts() { return agentShortcuts; }, set agentShortcuts(v) { agentShortcuts = v; }}, card);
  }

  function runState(card: KanbanCard) {
    return runStateBody({get agentRuns() { return agentRuns; }, set agentRuns(v) { agentRuns = v; }}, card);
  }

  function runIsLive(r: AgentRun) {
    return runIsLiveBody({}, r);
  }

  function isWriting(card: KanbanCard) {
    return isWritingBody({get agentRuns() { return agentRuns; }, set agentRuns(v) { agentRuns = v; }, runIsLive}, card);
  }

  function planetIsLive(projectId: string | null | undefined) {
    return planetIsLiveBody({get agentRuns() { return agentRuns; }, set agentRuns(v) { agentRuns = v; }, get hackProjectId() { return hackProjectId; }, set hackProjectId(v) { hackProjectId = v; }, runIsLive}, projectId);
  }

  function hasAgent(card: KanbanCard) {
    return hasAgentBody({get agentRuns() { return agentRuns; }, set agentRuns(v) { agentRuns = v; }, runIsLive}, card);
  }

  function goToken(title: string) {
    return goTokenBody({}, title);
  }

  function neededForTodo(card: KanbanCard) {
    return neededForTodoBody({goToken, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }}, card);
  }

  function isDoingRow(c: KanbanCard) {
    return isDoingRowBody({}, c);
  }

  // Needed-doings sit at the top of Doing. A one-off drag may park a
  // doing above them; a newly attached Needed re-sorts them to the top.
  let seenNeededDoing = new Set<string>();
  let neededDoingSortReady = false;
  $effect(() => {
    if (grabbedId) return;
    const doing = kanbanCards.filter(isDoingRow);
    const withNeed = doing.filter((c) => neededForTodo(c));
    const ids = withNeed.map((c) => c.id);
    const grew = ids.some((id) => !seenNeededDoing.has(id));
    const first = !neededDoingSortReady;
    neededDoingSortReady = true;
    seenNeededDoing = new Set(ids);
    if (!first && !grew) return;
    const rest = doing.filter((c) => !neededForTodo(c));
    const desired = [...withNeed, ...rest];
    if (
      desired.length === 0 ||
      doing.every((c, i) => c.id === desired[i]?.id)
    )
      return;
    mutate((s) => {
      const group = s.kanbanCards.filter(isDoingRow);
      const needSet = new Set(ids);
      const next = [
        ...group.filter((c) => needSet.has(c.id)),
        ...group.filter((c) => !needSet.has(c.id)),
      ];
      let gi = 0;
      s.kanbanCards = s.kanbanCards.map((c) =>
        isDoingRow(c) ? next[gi++] : c,
      );
    }, "background");
  });

  function ageLabel(card: KanbanCard) {
    return ageLabelBody({}, card);
  }

  function dueInfo(card: KanbanCard) {
    return dueInfoBody({startOfToday}, card);
  }

  // Project chip: LP-SPLIT — on whenever the planet is live, even on that
  // planet's own tab, so a screenshot of any board reads the planet.
  function showProj(card: KanbanCard) {
    return showProjBody({livePlanetId}, card);
  }

  // The seven palette skins are retired; a
  // dead saved id falls back to the OS pair. Terminal stays valid.
  const KNOWN_THEMES = new Set(["systemLight", "systemDark", "terminal"]);

  function osThemeId() {
    return osThemeIdBody({});
  }

  async function loadState() {
    return loadStateBody({KNOWN_THEMES, get agentRuns() { return agentRuns; }, set agentRuns(v) { agentRuns = v; }, get agentShortcuts() { return agentShortcuts; }, set agentShortcuts(v) { agentShortcuts = v; }, applyPendingFocus, clampSharedSplit, clampWindowOpacity, get collapsedSectionIds() { return collapsedSectionIds; }, set collapsedSectionIds(v) { collapsedSectionIds = v; }, get hackProjectId() { return hackProjectId; }, set hackProjectId(v) { hackProjectId = v; }, invoke, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, get migratingCardTrackers() { return migratingCardTrackers; }, set migratingCardTrackers(v) { migratingCardTrackers = v; }, mutate, get neededSplit() { return neededSplit; }, set neededSplit(v) { neededSplit = v; }, get openSubtaskIds() { return openSubtaskIds; }, set openSubtaskIds(v) { openSubtaskIds = v; }, osThemeId, get pointsLedger() { return pointsLedger; }, set pointsLedger(v) { pointsLedger = v; }, get previewWidth() { return previewWidth; }, set previewWidth(v) { previewWidth = v; }, get projects() { return projects; }, set projects(v) { projects = v; }, get tags() { return tags; }, set tags(v) { tags = v; }, get themeId() { return themeId; }, set themeId(v) { themeId = v; }, get titleOverrides() { return titleOverrides; }, set titleOverrides(v) { titleOverrides = v; }, get windowOpacity() { return windowOpacity; }, set windowOpacity(v) { windowOpacity = v; }});
  }

  // A window-linked card shows the rail's rename when one exists; the card's
  // own title (editable in the detail drawer) is the fallback.
  function displayTitle(card: KanbanCard): string {
    if (card.windowId) {
      const o = titleOverrides.find((x) => x.windowId === card.windowId);
      if (o?.title) return o.title;
    }
    return card.title;
  }

  // Append-only local event log (kind 31200). Fire-
  // and-forget in the same tick as the save — a log failure never blocks a
  // board mutation, and claim/unclaim events come from the native
  // save_state diff instead of a second guess here.
  function logEvent(eType: string, content: Record<string, unknown>, projectId?: string) {
    return logEventBody({invoke}, eType, content, projectId);
  }

  // Every write goes through here: fresh load, mutate, save. The rail owns
  // the other slices; this window only ever touches kanbanCards / pointsLedger
  // / claims (unclaim on window-card complete or Doing→Todo drag).
  async function mutate(fn: (s: PersistedState) => void, origin?: string) {
    return mutateBody({errText, invoke, loadState, showLinkToast}, fn, origin);
  }

  function errText(e: unknown) {
    return errTextBody({}, e);
  }

  function showLinkToast(msg: string) {
    return showLinkToastBody({get linkToast() { return linkToast; }, set linkToast(v) { linkToast = v; }, get linkToastTimer() { return linkToastTimer; }, set linkToastTimer(v) { linkToastTimer = v; }}, msg);
  }

  async function openCardLink(card: KanbanCard, index = 0) {
    return openCardLinkBody({linkErrText, requestLink, showLinkToast}, card, index);
  }

  async function openUrl(url: string) {
    return openUrlBody({linkErrText, requestLink, showLinkToast}, url);
  }

  function addCardLink(card: KanbanCard, url: string, label: string) {
    return addCardLinkBody({mutate}, card, url, label);
  }

  function removeCardLink(card: KanbanCard, url: string) {
    return removeCardLinkBody({mutate}, card, url);
  }

  // Complete — the only door that retires a sprite and can mint points.
  // Award once per card, 1 pt (2 clinical/master), daily cap honored.
  // A window card KEEPS its claim here so the walker
  // can walk off the north rim — the rail's habitat:walked-off listener
  // performs the unclaim (the existing eviction) once the walker is off,
  // or immediately when no walker can narrate the walk (marble/open dot).
  function completeCard(card: KanbanCard) {
    return completeCardBody({DAILY_CAP, logEvent, mutate, todayKey}, card);
  }

  // Checkbox: complete awards points once per card (daily cap honored); the
  // window keeps its claim until the walk-off lands and the rail unclaims —
  // the same rules as the rail ✓. Unchecking returns the card as a loose todo.
  function toggleCard(card: KanbanCard) {
    return toggleCardBody({completeCard, logEvent, mutate}, card);
  }

  // Board drag: drop on a column = status move. Dropping on Done is the
  // same complete door as the checkbox. Todo/Doing keep the window claim
  // (town: todo = board, doing = field). Unclaim is Unclaim, not a column.
  function moveCard(card: KanbanCard, target: Status) {
    return moveCardBody({completeCard, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, logEvent, mutate}, card, target);
  }

  // detail drawer mutations (all additive on the card)

  function openDetail(card: KanbanCard) {
    return openDetailBody({armNeededMouth, get detailId() { return detailId; }, set detailId(v) { detailId = v; }, get neededSelectedId() { return neededSelectedId; }, set neededSelectedId(v) { neededSelectedId = v; }, get newTagName() { return newTagName; }, set newTagName(v) { newTagName = v; }, get planetDdOpen() { return planetDdOpen; }, set planetDdOpen(v) { planetDdOpen = v; }, get previewEdit() { return previewEdit; }, set previewEdit(v) { previewEdit = v; }}, card);
  }

  async function toggleDoing(card: KanbanCard, e: MouseEvent) {
    return toggleDoingBody({invoke, loadState, moveCard}, card, e);
  }

  function skipNeededCard(card: KanbanCard) {
    return skipNeededCardBody({logEvent, mutate}, card);
  }

  function laterNeededCard(card: KanbanCard) {
    return laterNeededCardBody({logEvent, mutate}, card);
  }

  function onSplitDown(e: PointerEvent) {
    return onSplitDownBody({clampSharedSplit, get mailEl() { return mailEl; }, set mailEl(v) { mailEl = v; }, mutate, get neededSplit() { return neededSplit; }, set neededSplit(v) { neededSplit = v; }}, e);
  }

  function columnWidthNow() {
    return columnWidthNowBody({get boardPx() { return boardPx; }, set boardPx(v) { boardPx = v; }, columnMode, columnPx, get planetColPref() { return planetColPref; }, set planetColPref(v) { planetColPref = v; }});
  }

  // Mail-stack floor plus the planet column. clampPreview subtracts this
  // from the body so the list and the preview both keep a minimum when
  // the column is open (168) or collapsed to chips (72).
  function mailMinPx() {
    return mailMinPxBody({columnWidthNow});
  }

  function togglePlanetCol() {
    return togglePlanetColBody({clampPreview, get planetColMode() { return planetColMode; }, get planetColPref() { return planetColPref; }, set planetColPref(v) { planetColPref = v; }, get previewWidth() { return previewWidth; }, set previewWidth(v) { previewWidth = v; }});
  }

  function previewMinPx() {
    return previewMinPxBody({});
  }

  function clampPreview(px: number) {
    return clampPreviewBody({get bodyEl() { return bodyEl; }, set bodyEl(v) { bodyEl = v; }, mailMinPx, previewMinPx}, px);
  }

  function onEastSplitDown(e: PointerEvent) {
    return onEastSplitDownBody({get bodyEl() { return bodyEl; }, set bodyEl(v) { bodyEl = v; }, clampPreview, mutate, get previewWidth() { return previewWidth; }, set previewWidth(v) { previewWidth = v; }}, e);
  }

  function resetPreviewWidth() {
    return resetPreviewWidthBody({clampPreview, mutate, get previewWidth() { return previewWidth; }, set previewWidth(v) { previewWidth = v; }});
  }

  function clampSharedSplit(frac: number): number {
    return Math.min(0.45, Math.max(0.18, frac));
  }

  $effect(() => {
    const el = bodyEl;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      previewWidth = clampPreview(previewWidth);
    });
    ro.observe(el);
    return () => ro.disconnect();
  });

  $effect(() => {
    const el = kanbanEl;
    if (!el || typeof ResizeObserver === "undefined") return;
    const measure = () => {
      boardPx = el.clientWidth;
      untrack(() => {
        previewWidth = clampPreview(previewWidth);
      });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  });

  // GRAB leftover (arrows in charge): the drawer's ring stops — every
  // selectable on that surface, in paint order. A field confirms by
  // FOCUSING it (the caret stays the field's own — pad grammar never
  // steals it, Esc blurs); a button confirms by doing what its click
  // does. Closing the drawer pops the surface back to the board.
  function drawerStops(card: KanbanCard) {
    return drawerStopsBody({deleteSubtask, deleteTodo, focusCard, setDue, setRating, get tags() { return tags; }, set tags(v) { tags = v; }, toggleCardTag, toggleSubtask, winFor}, card);
  }

  function saveTitle(card: KanbanCard, raw: string) {
    return saveTitleBody({mutate}, card, raw);
  }

  async function undoThisTask(card: KanbanCard) {
    return undoThisTaskBody({invoke, loadState}, card);
  }

  function saveNotes(card: KanbanCard, raw: string) {
    return saveNotesBody({mutate}, card, raw);
  }

  function setPlanet(card: KanbanCard, projectId: string) {
    return setPlanetBody({logEvent, mutate}, card, projectId);
  }

  function setAgent(card: KanbanCard, agentId: string) {
    return setAgentBody({logEvent, mutate}, card, agentId);
  }

  function newCardId() {
    return newCardIdBody({});
  }

  function childrenOf(parentId: string) {
    return childrenOfBody({get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }}, parentId);
  }

  function remainingKids(parentId: string) {
    return remainingKidsBody({childrenOf}, parentId);
  }

  function toggleOpenSubtasks(parentId: string, e: MouseEvent) {
    return toggleOpenSubtasksBody({mutate, get openSubtaskIds() { return openSubtaskIds; }, set openSubtaskIds(v) { openSubtaskIds = v; }}, parentId, e);
  }

  function wouldCycle(parentId: string, childId: string) {
    return wouldCycleBody({get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }}, parentId, childId);
  }

  function linkableTodos(parent: KanbanCard) {
    return linkableTodosBody({get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, wouldCycle}, parent);
  }

  function mintTodo(title: string, parentId?: string) {
    return mintTodoBody({get addingTodo() { return addingTodo; }, set addingTodo(v) { addingTodo = v; }, get collapsedSectionIds() { return collapsedSectionIds; }, set collapsedSectionIds(v) { collapsedSectionIds = v; }, get detailId() { return detailId; }, set detailId(v) { detailId = v; }, emitTo, logEvent, mutate, newCardId, get newTodoTitle() { return newTodoTitle; }, set newTodoTitle(v) { newTodoTitle = v; }, get projects() { return projects; }, set projects(v) { projects = v; }, get selected() { return selected; }, set selected(v) { selected = v; }}, title, parentId);
  }

  function attachChild(parent: KanbanCard, childId: string) {
    return attachChildBody({mutate, wouldCycle}, parent, childId);
  }

  function detachChild(childId: string) {
    return detachChildBody({mutate}, childId);
  }

  function setDue(card: KanbanCard, dateValue: string) {
    return setDueBody({mutate}, card, dateValue);
  }

  function dateInputValue(card: KanbanCard) {
    return dateInputValueBody({}, card);
  }

  function cardHasTag(card: KanbanCard, tagId: string) {
    return cardHasTagBody({}, card, tagId);
  }

  function addTagToCard(card: KanbanCard, raw: string) {
    return addTagToCardBody({mutate, get newTagName() { return newTagName; }, set newTagName(v) { newTagName = v; }}, card, raw);
  }

  function toggleCardTag(card: KanbanCard, tagId: string) {
    return toggleCardTagBody({mutate}, card, tagId);
  }

  function toggleSubtask(card: KanbanCard, subId: string) {
    return toggleSubtaskBody({mutate}, card, subId);
  }

  function deleteTodo(card: KanbanCard) {
    return deleteTodoBody({get detailId() { return detailId; }, set detailId(v) { detailId = v; }, logEvent, mutate, get neededSelectedId() { return neededSelectedId; }, set neededSelectedId(v) { neededSelectedId = v; }, get previewEdit() { return previewEdit; }, set previewEdit(v) { previewEdit = v; }}, card);
  }

  function setRating(card: KanbanCard, n: number | null) {
    return setRatingBody({mutate}, card, n);
  }

  function deleteSubtask(card: KanbanCard, subId: string) {
    return deleteSubtaskBody({mutate}, card, subId);
  }

  // board drag handlers

  function onCardDragStart(e: DragEvent, _card: KanbanCard) {
    return onCardDragStartBody({}, e, _card);
  }

  function onCardDragEnd() {
    return onCardDragEndBody({get dragCardId() { return dragCardId; }, set dragCardId(v) { dragCardId = v; }, get dropCol() { return dropCol; }, set dropCol(v) { dropCol = v; }});
  }

  function isCardDrag(e: DragEvent) {
    return isCardDragBody({get dragCardId() { return dragCardId; }, set dragCardId(v) { dragCardId = v; }}, e);
  }

  function onColDragOver(e: DragEvent, col: Status) {
    return onColDragOverBody({get dropCol() { return dropCol; }, set dropCol(v) { dropCol = v; }, isCardDrag}, e, col);
  }

  function onColDragLeave(e: DragEvent, col: Status) {
    return onColDragLeaveBody({get dropCol() { return dropCol; }, set dropCol(v) { dropCol = v; }}, e, col);
  }

  function onColDrop(e: DragEvent, col: Status) {
    return onColDropBody({get dragCardId() { return dragCardId; }, set dragCardId(v) { dragCardId = v; }, get dropCol() { return dropCol; }, set dropCol(v) { dropCol = v; }, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, moveCard}, e, col);
  }

  // Pointer drag is the grab path. The card floats under the cursor.
  // Shift+Enter keyboard grab is unchanged.
  let suppressClick = false;

  let ptr: {
    id: string;
    x: number;
    y: number;
    held: boolean;
    ox: number;
    oy: number;
    w: number;
    h: number;
  } | null = null;

  function onCardPointerDown(e: PointerEvent, card: KanbanCard) {
    return onCardPointerDownBody({get grabbedId() { return grabbedId; }, set grabbedId(v) { grabbedId = v; }, onPtrCancel, onPtrKey, onPtrMove, onPtrUp, get ptr() { return ptr; }, set ptr(v) { ptr = v; }, get suppressClick() { return suppressClick; }, set suppressClick(v) { suppressClick = v; }}, e, card);
  }

  function onPtrMove(e: PointerEvent) {
    return onPtrMoveBody({colAtPoint, get grabCol() { return grabCol; }, set grabCol(v) { grabCol = v; }, get grabGhost() { return grabGhost; }, set grabGhost(v) { grabGhost = v; }, get grabIdx() { return grabIdx; }, set grabIdx(v) { grabIdx = v; }, get grabbedId() { return grabbedId; }, set grabbedId(v) { grabbedId = v; }, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, overStartStrip, get ptr() { return ptr; }, set ptr(v) { ptr = v; }, ptrEnd, rowAtPoint, startGrab}, e);
  }

  function onPtrUp(e: PointerEvent) {
    return onPtrUpBody({dropGrab, get grabCol() { return grabCol; }, set grabCol(v) { grabCol = v; }, get grabbedId() { return grabbedId; }, set grabbedId(v) { grabbedId = v; }, overStartStrip, get ptr() { return ptr; }, set ptr(v) { ptr = v; }, ptrEnd, get suppressClick() { return suppressClick; }, set suppressClick(v) { suppressClick = v; }}, e);
  }

  function onPtrCancel() {
    return onPtrCancelBody({get grabbedId() { return grabbedId; }, set grabbedId(v) { grabbedId = v; }, get ptr() { return ptr; }, set ptr(v) { ptr = v; }, ptrEnd, ungrab});
  }

  function ptrEnd() {
    return ptrEndBody({onPtrCancel, onPtrKey, onPtrMove, onPtrUp, get ptr() { return ptr; }, set ptr(v) { ptr = v; }});
  }

  function onPtrKey(e: KeyboardEvent) {
    return onPtrKeyBody({get grabbedId() { return grabbedId; }, set grabbedId(v) { grabbedId = v; }, get ptr() { return ptr; }, set ptr(v) { ptr = v; }, ungrab}, e);
  }

  function overStartStrip(x: number, y: number) {
    return overStartStripBody({}, x, y);
  }

  function colAtPoint(x: number, y: number) {
    return colAtPointBody({COLUMNS}, x, y);
  }

  function rowAtPoint(col: Status, y: number) {
    return rowAtPointBody({}, col, y);
  }

  function onCardClick(card: KanbanCard) {
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    if (embedded) {
      dockCard = card;
      return;
    }
    return onCardClickBody({openDetail, get suppressClick() { return suppressClick; }, set suppressClick(v) { suppressClick = v; }}, card);
  }

  async function refreshWindows() {
    return refreshWindowsBody({invoke, get windows() { return windows; }, set windows(v) { windows = v; }});
  }

  async function focusCard(card: KanbanCard) {
    return focusCardBody({invoke, winFor}, card);
  }

  // S5 Hermes: launch/focus the EXISTING harness with this list's open
  // titles (window cards already carry "App — Title"). Never file contents.
  async function launchHermes() {
    return launchHermesBody({get hermesNote() { return hermesNote; }, set hermesNote(v) { hermesNote = v; }, invoke, get kanbanCards() { return kanbanCards; }, set kanbanCards(v) { kanbanCards = v; }, listOf, get selectedProject() { return selectedProject; }, set selectedProject(v) { selectedProject = v; }});
  }

  $effect(() => {
    if (!themeId) return;
    document.documentElement.dataset.theme = themeId;
    applyHostTokens(themeId);
    // S4.4: re-assert the native transparency surgery post-load.
    // The stage rail owns main's glass; embedding must not call ensure_glass.
    if (railPlanet != null) return;
    invoke("ensure_glass").catch(() => {});
  });

  // Reads themeId too so a theme flip re-derives the rgba fills (the theme
  // effect above runs first and sets the tokens).
  $effect(() => {
    if (!themeId) return;
    applyWindowOpacity(windowOpacity);
  });

  // The window is created hidden at launch and hidden again on close — skip
  // the AX poll while invisible, refresh on reveal.
  const onVisibility = () => {
    if (!document.hidden) {
      loadState();
      refreshWindows();
    }
  };

  // Rail → Lists deep link: select the card's project and open its detail.
  // Reload first so a card created milliseconds ago is actually present.
  async function onFocusCard(cardId: string) {
    await loadState();
    const card = kanbanCards.find((c) => c.id === cardId);
    if (!card) return;
    selected = listOf(card);
    detailId = card.id;
    newTagName = "";
    previewEdit = false;
  }

  let timer: ReturnType<typeof setInterval> | undefined;
  let unlisten: (() => void) | undefined;
  let unlistenFocus: (() => void) | undefined;
  let unlistenFocusProject: (() => void) | undefined;
  let unlistenVis: (() => void) | undefined;

  function startRefreshTimer() {
    stopRefreshTimer();
    timer = setInterval(() => {
      if (!document.hidden) refreshWindows();
    }, 1500);
  }

  function stopRefreshTimer() {
    if (timer) {
      clearInterval(timer);
      timer = undefined;
    }
  }

  onMount(async () => {
    if (!embedded) invoke("ensure_glass").catch(() => {});
    unlisten = await listen("state:changed", loadState);
    unlistenFocus = await listen<string>("orbitty:focus-card", (e) => {
      onFocusCard(e.payload);
    });
    // BOARD-MARK: full-page kanban → this planet (do not require projects[] yet).
    unlistenFocusProject = await listen<string>("orbitty:focus-project", (e) => {
      if (railPlanet != null) return;
      applyFocusProject(e.payload);
      // Delivered event consumes the take-once slot so a remount cannot
      // re-apply it. The embedded Well does not own the slot.
      if (!embedded) {
        void invoke("take_board_focus").catch(() => {});
      }
    });
    // Click that beat this listener: read-and-clear once. Embedded
    // instances skip it so a Well opened within 5s cannot steal it.
    if (!embedded) {
      try {
        const caught = await invoke<string | null>("take_board_focus");
        if (caught) applyFocusProject(caught);
      } catch {
        // Command missing: later clicks still arrive on the listener.
      }
    }
    // KANBAN-GRAB: Esc (pad B) lets go of a held card first. Then the
    // hosted well falls through so CompanionPage's bubble Esc dismisses
    // it. The overlay has no bubble hide, so that last Esc hides the
    // window. Tab never reaches this callback.
    if (railPlanet == null) {
      detachMenuKeys = menuRing.attach(() => {
        if (grabbedId) {
          ungrab();
          return true;
        }
        if (!embedded) {
          invoke("hide_window", { label: "kanban" }).catch(() => {});
          return true;
        }
        return false;
      });
    }
    // HOST-SPLIT embedded: do not await load_state / AX on first well
    // mount — that IPC return was painting cards mid-rise and lagging
    // the sprite aside. Overlay Launchpad still hydrates in-line.
    if (embedded) {
      void loadState();
      return;
    }
    await loadState();
    await refreshWindows();
    startRefreshTimer();
    // HITCH-DRAW: the window keeps warm on hide, so document.hidden never
    // flips — gate the 1500ms AX poll on the native event instead. Show
    // catches up once, then resumes the poll.
    unlistenVis = await listenPaneVis("kanban", (visible) => {
      if (visible) {
        void loadState().then(() => {
          applyPendingFocus();
        });
        refreshWindows();
        startRefreshTimer();
      } else {
        stopRefreshTimer();
        neededSelectedId = null;
        armNeededMouth(null, null);
      }
    });
    document.addEventListener("visibilitychange", onVisibility);
  });
  onDestroy(() => {
    stopRefreshTimer();
    ptrEnd();
    detachMenuKeys?.();
    unlisten?.();
    unlistenFocus?.();
    unlistenFocusProject?.();
    unlistenVis?.();
    document.removeEventListener("visibilitychange", onVisibility);
    armNeededMouth(null, null);
  });

  function onBannerMouseDown(e: MouseEvent) {
    startPaneDrag(e);
  }
</script>


{#snippet doneActorChips()}
  <!-- Happened split — All default, session-only. -->
  <div class="actor-chips" role="group" aria-label="Filter Done by who acted">
    <button
      class:active={doneFilter === "all"}
      aria-pressed={doneFilter === "all"}
      onclick={() => (doneFilter = "all")}>All</button
    >
    <button
      class:active={doneFilter === "mine"}
      aria-pressed={doneFilter === "mine"}
      onclick={() => (doneFilter = "mine")}>I did</button
    >
    <button
      class:active={doneFilter === "behalf"}
      aria-pressed={doneFilter === "behalf"}
      onclick={() => (doneFilter = "behalf")}>On my behalf</button
    >
  </div>
{/snippet}

<main class="kanban" class:embedded bind:this={kanbanEl}>
  {#if embedded && dockCard}
    <DockCardPop card={dockCard} onClose={() => (dockCard = null)} />
  {/if}
  {#if linkToast}
    <div class="k-toast" role="status">{linkToast}</div>
  {/if}
  {#if railPlanet == null && planetColMode !== "hidden"}
  <BoardChrome
    {selected}
    {listTitle}
    {titleDot}
    bind:hermesNote
    {embedded}
    {invoke}
    {onBannerMouseDown}
    {onClose}
    {onPopout}
    {doneActorChips}
  />
  {/if}

  <div class="k-body" bind:this={bodyEl}>
    {#if railPlanet == null && planetColMode !== "hidden"}
    <PlanetColumn
      bind:selected
      mode={planetColMode}
      onToggle={togglePlanetCol}
      {projects}
      {sideCount}
      {menuRing}
      {planetCss}
      {planetIsLive}
      {planetDragId}
      {planetDropId}
      {dropCol}
      {grabCol}
      {onPlanetPointerDown}
      {onPlanetTabClick}
      {onPlanetTabDblClick}
      {toggleDoneChip}
      {onColDragOver}
      {onColDragLeave}
      {onColDrop}
    />
    {/if}
    <BoardColumns
      bind:addingTodo
      bind:detailId
      bind:doneFilter
      bind:mailEl
      bind:newTodoTitle
      bind:openSubtaskIds
      bind:selected
      bind:addingNeeded
      bind:neededSelectedId
      bind:neededSplit
      bind:neededEl
      {ageLabel}
      {agentName}
      {childrenOf}
      {displayTitle}
      {dragCardId}
      {dropCol}
      {dueInfo}
      {grabCol}
      {grabGhost}
      {grabbedId}
      {hasAgent}
      {isWriting}
      {listTitle}
      {livePlanetId}
      {menuRing}
      {mintTodo}
      {moveCard}
      {neededForTodo}
      {onCardClick}
      {onCardDragEnd}
      {onCardDragStart}
      {onCardPointerDown}
      {onColDragLeave}
      {onColDragOver}
      {onColDrop}
      {openDetail}
      {planetCss}
      {projectName}
      {remainingKids}
      {showProj}
      {subProgress}
      {toggleDoing}
      {toggleOpenSubtasks}
      {openCardLink}
      {viewCols}
      {armNeededMouth}
      {kanbanCards}
      {logEvent}
      {mutate}
      {neededProjectId}
      {neededRows}
      {onNeededAnswer}
      {onSplitDown}
      {projects}
      chipsInTitle={railPlanet == null && planetColMode !== "hidden"}
    />

    <BoardPreview
      bind:newTagName
      bind:planetDdOpen
      bind:previewEdit
      bind:previewWidth
      bind:selected
      {addTagToCard}
      {agentName}
      {agentShortcuts}
      {attachChild}
      {cardHasTag}
      {childrenOf}
      {dateInputValue}
      {deleteSubtask}
      {deleteTodo}
      {detachChild}
      {detailCard}
      {displayTitle}
      {dueInfo}
      {focusCard}
      {linkableTodos}
      {livePlanetId}
      {mdLite}
      {menuRing}
      {moveCard}
      {onEastSplitDown}
      {openDetail}
      {planetCss}
      {projectName}
      {projects}
      {resetPreviewWidth}
      {saveNotes}
      {saveTitle}
      {addCardLink}
      {removeCardLink}
      {openCardLink}
      {openUrl}
      {setAgent}
      {setDue}
      {setPlanet}
      {setRating}
      {tags}
      {toggleCardTag}
      {toggleSubtask}
      {undoThisTask}
      {winFor}
    />
  </div>
</main>


