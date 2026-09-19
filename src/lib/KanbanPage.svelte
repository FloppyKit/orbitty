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
  import { onMount, onDestroy } from "svelte";
  import { applyHostTokens, applyWindowOpacity, clampWindowOpacity } from "../themes/apply";
  import { listenPaneVis } from "./paneVis";
  import { planetCss } from "./planetColor";
  import { startPaneDrag } from "./dragPane";
  import NeededInbox, { neededFor } from "./NeededInbox.svelte";
  import BoardChrome from "./board/Chrome.svelte";
  import BoardColumns from "./board/Columns.svelte";
  import BoardPreview from "./board/Preview.svelte";
  import "./board/board.css";
  import { MenuRing, type RingDir, type RingStop } from "./menuKeys.svelte";
  import { mdLite } from "./mdLite";
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
    Tracker,
    PersistedState,
    PointsEntry,
    Project,
    Tag,
    TitleOverride,
    WindowInfo,
  } from "./persist";

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
  }: {
    embedded?: boolean;
    onClose?: () => void;
    onPopout?: () => void;
    onNeededFocus?: (id: string | null, title: string | null) => void;
    // MENU-KEYS: Enter on a Needed row — the host lands the caret in the
    // composer so the next typed line answers it.
    onNeededAnswer?: () => void;
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
  // Launchpad strip (same flag the inspector sheet tabs already read).
  let hackProjectId = $state<string | null>(null);
  let selectedBeforeDone = $state("inbox");
  let neededSplit = $state(0.28);
  let previewWidth = $state(280);
  let trackerSplit = $state(0.20);
  let mailEl: HTMLDivElement | null = $state(null);
  let bodyEl: HTMLDivElement | null = $state(null);
  let eastEl: HTMLDivElement | null = $state(null);
  let addingTodo = $state(false);
  let newTodoTitle = $state("");
  let openSubtaskIds = $state<string[]>([]);
  let newTagName = $state("");
  let newTrackerLabel = $state("");
  let newTrackerPlanetId = $state("");
  let addingTracker = $state(false);
  let trackerArmed = $state(false);
  let trackPaneEl: HTMLElement | null = $state(null);
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
  // Shared so strip + and sidebar form stay one toggle.
  let addingNeeded = $state(false);

  // Sidebar selection: a smart filter, Inbox, or a project id. Session-only.
  let selected = $state("inbox");
  /** BOARD-MARK: apply after loadState / pane show (projects may be empty on first emit). */
  let pendingFocusProject = $state<string | null>(null);
  // PLANETS-STRIP: rail-owned marble flag, read-only here — tab double-click
  // emits to main; the rail persist is the writer.
  let collapsedSectionIds = $state<string[]>([]);

  let detailId = $state<string | null>(null);

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
    if (!embedded) return;
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
    const stops: RingStop[] = [];
    // The Needed column is always open — the pile rows ride the ring
    // directly (no toggle stop). Enter on a row arms the composer
    // answer (the companion text box is the mouth) and lands the caret.
    for (const c of neededRows) {
      stops.push({
        id: `needed-${c.id}`,
        confirm: () => {
          neededSelectedId = c.id;
          armNeededMouth(c.id, c.title);
          onNeededAnswer?.();
        },
      });
    }
    // The planet strip is a ROW on this surface: left/right walk it,
    // down drops into the first card of the first non-empty column, up
    // climbs back to the Needed pile.
    const tabIds = ["inbox", "today", "done", ...projects.map((p) => p.id)];
    tabIds.forEach((id, i) => {
      stops.push({
        id: `listtab-${id}`,
        confirm: () => {
          if (projects.some((p) => p.id === id)) onPlanetTabClick(id);
          else selected = id;
        },
        arrow: (dir) => {
          if (dir === "left" || dir === "right") {
            const n = tabIds.length;
            const next = tabIds[(i + (dir === "left" ? -1 : 1) + n) % n];
            menuRing.select(`listtab-${next}`);
            return true;
          }
          if (dir === "down") {
            const first = COLUMNS.map((c) => boardCols[c.id]).find(
              (l) => l.length > 0,
            )?.[0];
            if (first) {
              menuRing.select(`card-${first.id}`);
              return true;
            }
            return false;
          }
          if (neededRows.length > 0) {
            menuRing.select(`needed-${neededRows[neededRows.length - 1].id}`);
            return true;
          }
          return false;
        },
      });
    });
    const cols = COLUMNS.map((c) => boardCols[c.id]);
    cols.forEach((colCards, ci) => {
      colCards.forEach((card, ri) => {
        stops.push({
          id: `card-${card.id}`,
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
  const showTrackers = $derived(selected === "inbox" || !!selectedProject);
  const visibleTrackers = $derived.by(() => {
    if (selectedProject) {
      return (selectedProject.trackers ?? []).map((tr) => ({
        projectId: selectedProject.id,
        chip: false,
        tr,
      }));
    }
    if (selected !== "inbox") return [];
    return projects.flatMap((p) =>
      (p.trackers ?? []).map((tr) => ({
        projectId: p.id,
        chip: true,
        tr,
      })),
    );
  });


  function raisePlanet(id: string) {
    invoke("bring_planet_front", { id }).catch(() => {});
    invoke("locate_planet", { id }).catch(() => {});
  }


  function applyFocusProject(raw: string | null | undefined) {
    const id = (raw ?? "").trim();
    if (!id) return;
    pendingFocusProject = id;
    selected = id;
    if (projects.some((p) => p.id === id)) raisePlanet(id);
  }

  function onPlanetTabClick(id: string) {
    if (planetDidDrag) {
      planetDidDrag = false;
      return;
    }
    selected = id;
    raisePlanet(id);
  }

  function reorderPlanetsLive(fromId: string, beforeId: string | null) {
    if (!fromId || fromId === beforeId) return;
    const list = [...projects];
    const from = list.findIndex((p) => p.id === fromId);
    if (from < 0) return;
    const [moved] = list.splice(from, 1);
    const to = beforeId ? list.findIndex((p) => p.id === beforeId) : list.length;
    list.splice(to < 0 ? list.length : to, 0, moved);
    projects = list;
  }

  function persistPlanetOrder() {
    const order = projects.map((p) => p.id);
    mutate((s) => {
      const byId = new Map((s.projects ?? []).map((p) => [p.id, p]));
      s.projects = order.map((id) => byId.get(id)).filter((p): p is Project => !!p);
    });
  }

  function planetBeforeAt(clientX: number): string | null {
    const els = document.querySelectorAll(".strip-planets .planet-tab");
    for (const el of els) {
      const r = el.getBoundingClientRect();
      if (clientX < r.left + r.width / 2) {
        return el.getAttribute("data-pid");
      }
    }
    return null;
  }

  function onPlanetPointerDown(e: PointerEvent, id: string) {
    if (e.button !== 0) return;
    if (!(e.currentTarget instanceof HTMLElement)) return;
    e.stopPropagation();
    const tab = e.currentTarget;
    const strip = tab.closest(".strip-planets");
    const x0 = e.clientX;
    const y0 = e.clientY;
    const scroll0 = strip instanceof HTMLElement ? strip.scrollLeft : 0;
    let dragging = false;
    let armed = true;
    const SLOP = 16;
    const onMove = (ev: PointerEvent) => {
      if (!armed) return;
      const dx = ev.clientX - x0;
      const dy = ev.clientY - y0;
      if (!dragging) {
        if (Math.hypot(dx, dy) < SLOP) return;
        if (Math.abs(dy) > Math.abs(dx)) {
          armed = false;
          cleanup();
          return;
        }
        if (
          strip instanceof HTMLElement &&
          Math.abs(strip.scrollLeft - scroll0) > 2
        ) {
          armed = false;
          cleanup();
          return;
        }
        dragging = true;
        planetDidDrag = true;
        planetDragId = id;
      }
      const before = planetBeforeAt(ev.clientX);
      planetDropId = before;
      reorderPlanetsLive(id, before);
    };
    const onUp = () => {
      armed = false;
      if (dragging) persistPlanetOrder();
      planetDragId = null;
      planetDropId = null;
      cleanup();
    };
    const cleanup = () => {
      try {
        tab.releasePointerCapture(e.pointerId);
      } catch {
        /* capture is best-effort */
      }
      tab.removeEventListener("pointermove", onMove);
      tab.removeEventListener("pointerup", onUp);
      tab.removeEventListener("pointercancel", onUp);
    };
    try {
      tab.setPointerCapture(e.pointerId);
    } catch {
      /* capture is best-effort */
    }
    tab.addEventListener("pointermove", onMove);
    tab.addEventListener("pointerup", onUp);
    tab.addEventListener("pointercancel", onUp);
  }

  function toggleDoneChip() {
    if (selected === "done") {
      selected = selectedBeforeDone || "inbox";
    } else {
      selectedBeforeDone = selected;
      selected = "done";
    }
  }

  function onPlanetTabDblClick(id: string) {
    const collapsed = collapsedSectionIds.includes(id);
    emitTo("main", collapsed ? "habitat:expand" : "habitat:collapse", id).catch(
      () => {},
    );
  }

  function todayKey(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }

  function startOfToday(): number {
    const d = new Date();
    d.setHours(0, 0, 0, 0);
    return d.getTime() / 1000;
  }

  function isDueToday(c: KanbanCard): boolean {
    if (!c.dueAt) return false;
    const t = startOfToday();
    return c.dueAt >= t && c.dueAt < t + 86400;
  }

  function pointsToday(): number {
    const key = todayKey();
    return pointsLedger
      .filter((e) => e.day === key)
      .reduce((a, e) => a + e.amount, 0);
  }

  // A card whose project vanished falls back to Inbox — never invisible.
  function listOf(c: KanbanCard): string {
    return projects.some((p) => p.id === c.projectId) ? c.projectId : "inbox";
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

  function actorSide(card: KanbanCard): "mine" | "behalf" {
    return card.actor && card.actor !== "user" ? "behalf" : "mine";
  }

  function inDoneFilter(c: KanbanCard): boolean {
    return doneFilter === "all" || actorSide(c) === doneFilter;
  }

  // LP-INBOX: planet color dot — only when projectId is a LIVE project
  // (a vanished project falls back to Inbox and gets no dot).
  function livePlanetId(card: KanbanCard): string | null {
    return projects.some((p) => p.id === card.projectId) ? card.projectId : null;
  }

  // Board scope mirrors the sidebar selection; the Done smart filter boards
  // every card so all three columns stay meaningful.
  function inScope(c: KanbanCard): boolean {
    // LP-SPLIT: the kind filter is also what keeps ANSWERED Neededs out
    // of the consolidated Inbox — a Needed never becomes a board todo.
    if (c.kind === "needed" || c.kind === "window") return false;
    if (selected === "done") return true;
    if (selected === "today") return isDueToday(c);
    // LP-SPLIT: Inbox is the consolidated pile — every planet's todos,
    // each card marked by its planet chip. Planet tabs still filter.
    if (selected === "inbox") return true;
    return listOf(c) === selected;
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
    if (grabGhost) return boardCols;
    const card = kanbanCards.find((c) => c.id === grabbedId);
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

  $effect(() => {
    void selected;
    addingTracker = false;
    newTrackerLabel = "";
    newTrackerPlanetId = selectedProject?.id ?? "";
    trackerArmed = false;
  });

  $effect(() => {
    if (!trackerArmed) return;
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node | null;
      if (trackPaneEl && t && !trackPaneEl.contains(t)) trackerArmed = false;
    };
    window.addEventListener("pointerdown", onDown, true);
    return () => window.removeEventListener("pointerdown", onDown, true);
  });

  // Overlay Launchpad is its own window — the embedded onNeededFocus
  // prop is not this path. Native raise makes companion key; the
  // companion webview then lands the caret. Do not fire on boot.
  function armNeededMouth(id: string | null, title: string | null) {
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
    if (grabbedId) return; // a repeated Shift+Enter is not a re-grab
    grabbedId = card.id;
    grabCol = card.status;
    // Start where the card really sits so the grab never jumps it.
    grabIdx = Math.max(
      0,
      boardCols[card.status].findIndex((c) => c.id === card.id),
    );
  }

  function grabArrow(dir: RingDir) {
    if (!grabbedId || !grabCol) return;
    if (dir === "left" || dir === "right") {
      // Column hop, clamped at the ends. Empty columns are legal targets
      // here — you are placing the card, not selecting one.
      const order = COLUMNS.map((c) => c.id);
      const next = order.indexOf(grabCol) + (dir === "left" ? -1 : 1);
      if (next < 0 || next >= order.length) return;
      grabCol = order[next];
    } else {
      grabIdx += dir === "up" ? -1 : 1;
    }
    // Same row, clamped into the column (its length excludes the held card).
    const len = boardCols[grabCol].filter((c) => c.id !== grabbedId).length;
    grabIdx = Math.min(Math.max(grabIdx, 0), len);
    // The card moved under a pinned highlight — keep it scrolled in view.
    menuRing.select(`card-${grabbedId}`);
  }

  function dropGrab() {
    const id = grabbedId;
    const target = grabCol;
    const idx = grabIdx;
    grabbedId = null;
    grabCol = null;
    grabIdx = 0;
    grabGhost = null;
    const card = id ? kanbanCards.find((c) => c.id === id) : undefined;
    if (!card || !target) return;
    if (target !== card.status) {
      moveCard(card, target);
      return;
    }
    persistColumnOrder(card, target, idx);
  }

  function persistColumnOrder(card: KanbanCard, col: Status, toIdx: number) {
    mutate((s) => {
      const cards = s.kanbanCards;
      const match = (c: KanbanCard) =>
        c.status === col &&
        c.kind !== "needed" &&
        c.kind !== "window" &&
        !c.parentId &&
        inScope(c);
      const group = cards.filter(match);
      const from = group.findIndex((c) => c.id === card.id);
      if (from < 0) return;
      const [held] = group.splice(from, 1);
      group.splice(Math.min(Math.max(toIdx, 0), group.length), 0, held);
      let gi = 0;
      s.kanbanCards = cards.map((c) => (match(c) ? group[gi++] : c));
    });
  }

  function ungrab() {
    // Esc (pad B): no mutation ever happened — the card is still in its
    // pre-grab column; the held paint just stops.
    grabbedId = null;
    grabCol = null;
    grabIdx = 0;
    grabGhost = null;
  }

  let detailCard = $derived(
    detailId ? kanbanCards.find((c) => c.id === detailId) : undefined,
  );

  function sideCount(list: string): number {
    if (list === "today") return openCards.filter((c) => isDueToday(c)).length;
    if (list === "done") return doneCards.length;
    // LP-SPLIT: Inbox counts the whole consolidated all-planets pile.
    if (list === "inbox") return openCards.length;
    return openCards.filter((c) => listOf(c) === list).length;
  }

  let listTitle = $derived(
    selected === "today"
      ? "Today"
      : selected === "done"
        ? "Done"
        : selected === "inbox"
          ? "Inbox"
          : (projects.find((p) => p.id === selected)?.name ?? "Inbox"),
  );

  function projectName(id: string): string {
    return projects.find((p) => p.id === id)?.name ?? "";
  }

  function winFor(card: KanbanCard): WindowInfo | undefined {
    return card.windowId
      ? windows.find((w) => w.id === card.windowId)
      : undefined;
  }

  function doneChip(card: KanbanCard): string {
    if ((card.completedAt ?? 0) < startOfToday()) return "";
    return card.pointsAwarded > 0 ? `+${card.pointsAwarded}` : "cap";
  }

  // row meta (notes snippet, difficulty pip, due chip, sub count)

  function notesSnippet(card: KanbanCard): string {
    const first = (card.notes ?? "").split("\n")[0].trim();
    return first.length > 60 ? `${first.slice(0, 60)}…` : first;
  }

  function subProgress(card: KanbanCard): string {
    const subs = card.subtasks ?? [];
    if (subs.length === 0) return "";
    return `${subs.filter((s) => s.done).length}/${subs.length}`;
  }

  function agentName(card: KanbanCard): string {
    const id = card.assignedAgentId;
    if (!id) return "";
    return agentShortcuts.find((a) => a.id === id)?.name ?? "";
  }

  function runState(card: KanbanCard): string {
    const r = agentRuns.find((x) => x.cardId === card.id);
    if (!r || r.state === "done") return "";
    return r.state;
  }

  function runIsLive(r: AgentRun): boolean {
    if (r.state !== "working" && r.state !== "starting" && r.state !== "blocked")
      return false;
    // GO→doing mints a working row with an empty seat. That is not a
    // live writer — companion LED (hackLive) is the other honest signal.
    return !!(r.agentShortcutId || "").trim() || !!r.herdr?.agentName;
  }

  function isWriting(card: KanbanCard): boolean {
    const r = agentRuns.find((x) => x.cardId === card.id);
    return !!r && runIsLive(r);
  }

  function planetIsLive(projectId: string | null | undefined): boolean {
    if (!projectId) return false;
    if (hackProjectId === projectId) return true;
    return agentRuns.some((r) => r.planetId === projectId && runIsLive(r));
  }

  function hasAgent(card: KanbanCard): boolean {
    if (card.assignedAgentId) return true;
    const r = agentRuns.find((x) => x.cardId === card.id);
    return !!r && runIsLive(r);
  }

  function goToken(title: string): string | null {
    const m = title.match(/GO-[A-Z0-9]+(?:-[A-Z0-9]+)*/i);
    return m ? m[0].toUpperCase() : null;
  }

  function neededForTodo(card: KanbanCard): KanbanCard | undefined {
    const go = goToken(card.title);
    if (!go) return undefined;
    const planet = card.projectId || "";
    return kanbanCards.find(
      (c) =>
        c.kind === "needed" &&
        (c.neededState === "open" || c.neededState === "later") &&
        (c.projectId || "") === planet &&
        ((c.title || "").toUpperCase().includes(go) ||
          (c.neededWhy || "").toUpperCase().includes(go)),
    );
  }

  function isDoingRow(c: KanbanCard): boolean {
    return (
      c.status === "doing" &&
      c.kind !== "needed" &&
      c.kind !== "window" &&
      !c.parentId
    );
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
    });
  });

  function ageLabel(card: KanbanCard): string {
    const t = card.lastTouchedAt || card.createdAt;
    if (!t) return "";
    const s = Date.now() / 1000 - t;
    if (s < 90) return "now";
    if (s < 3600) return `${Math.max(1, Math.round(s / 60))}m`;
    if (s < 86400) return `${Math.max(1, Math.round(s / 3600))}h`;
    return `${Math.max(1, Math.round(s / 86400))}d`;
  }

  function dueInfo(card: KanbanCard): { label: string; overdue: boolean } | null {
    if (!card.dueAt) return null;
    const d = new Date(card.dueAt * 1000);
    const day = new Date(d);
    day.setHours(0, 0, 0, 0);
    const diffDays = Math.round(day.getTime() / 1000 - startOfToday()) / 86400;
    let label: string;
    if (diffDays === 0) label = "Today";
    else if (diffDays === 1) label = "Tomorrow";
    else if (diffDays === -1) label = "Yesterday";
    else label = d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
    return {
      label,
      overdue: card.status !== "done" && day.getTime() / 1000 < startOfToday(),
    };
  }

  // Project chip: LP-SPLIT — on whenever the planet is live, even on that
  // planet's own tab, so a screenshot of any board reads the planet.
  function showProj(card: KanbanCard): boolean {
    return livePlanetId(card) !== null;
  }

  // The seven palette skins are retired; a
  // dead saved id falls back to the OS pair. Terminal stays valid.
  const KNOWN_THEMES = new Set(["systemLight", "systemDark", "terminal"]);

  function osThemeId(): string {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "systemDark"
      : "systemLight";
  }

  async function loadState() {
    try {
      const s = await invoke<PersistedState>("load_state");
      // "" means the rail follows the OS; retired ids (clear/liquidGlass)
      // resolve the same way.
      themeId = KNOWN_THEMES.has(s.themeId) ? s.themeId : osThemeId();
      windowOpacity = clampWindowOpacity(s.windowOpacity ?? 1);
      projects = s.projects ?? [];
      if (pendingFocusProject) {
        selected = pendingFocusProject;
        if (projects.some((p) => p.id === pendingFocusProject)) raisePlanet(pendingFocusProject);
      }
      tags = s.tags ?? [];
      kanbanCards = s.kanbanCards ?? [];
      pointsLedger = s.pointsLedger ?? [];
      titleOverrides = s.titleOverrides ?? [];
      collapsedSectionIds = s.collapsedSectionIds ?? [];
      agentShortcuts = s.agentShortcuts ?? [];
      agentRuns = s.agentRuns ?? [];
      // BOARD-SPLIT-Y: one shared band — prefer neededSplit, mirror to trackers.
      const band = clampSharedSplit(s.neededSplit ?? s.trackerSplit ?? 0.28);
      neededSplit = band;
      previewWidth = Math.max(160, s.previewWidth ?? 280);
      trackerSplit = band;
      openSubtaskIds = s.openSubtaskIds ?? [];
      hackProjectId = s.companion?.hackLive
        ? (s.companion?.hackProjectId ?? null)
        : null;
      if (
        !migratingCardTrackers &&
        (s.kanbanCards ?? []).some((c) => (c.trackers ?? []).length > 0)
      ) {
        migratingCardTrackers = true;
        await mutate((st) => {
          for (const c of st.kanbanCards) {
            const dump = c.trackers ?? [];
            if (!dump.length || !c.projectId) continue;
            st.projects = st.projects.map((p) =>
              p.id === c.projectId
                ? { ...p, trackers: [...(p.trackers ?? []), ...dump] }
                : p,
            );
          }
          st.kanbanCards = st.kanbanCards.map((c) =>
            c.trackers?.length ? { ...c, trackers: undefined } : c,
          );
        });
        migratingCardTrackers = false;
      }
    } catch {
      // keep last good state; next state:changed retries
    }
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
    invoke("log_event", {
      eType,
      content,
      projectId: projectId || null,
      actor: "user",
    }).catch(() => {});
  }

  // Every write goes through here: fresh load, mutate, save. The rail owns
  // the other slices; this window only ever touches kanbanCards / pointsLedger
  // / claims (unclaim on window-card complete or Doing→Todo drag).
  async function mutate(fn: (s: PersistedState) => void) {
    try {
      const s = await invoke<PersistedState>("load_state");
      s.projects ??= [];
      s.claims ??= [];
      s.tags ??= [];
      s.windowTags ??= [];
      s.collapsedSectionIds ??= [];
      s.nudgeSuppressedWindowIds ??= [];
      s.kanbanCards ??= [];
      s.pointsLedger ??= [];
      s.titleOverrides ??= [];
      fn(s);
      await invoke("save_state", { state: s });
      await loadState();
    } catch {
      // Optimistic column paint already moved the card; reload disk
      // so a failed save does not leave a lying column.
      await loadState();
    }
  }

  // Complete — the only door that retires a sprite and can mint points.
  // Award once per card, 1 pt (2 clinical/master), daily cap honored.
  // A window card KEEPS its claim here so the walker
  // can walk off the north rim — the rail's habitat:walked-off listener
  // performs the unclaim (the existing eviction) once the walker is off,
  // or immediately when no walker can narrate the walk (marble/open dot).
  function completeCard(card: KanbanCard) {
    const now = Date.now() / 1000;
    mutate((s) => {
      const key = todayKey();
      const todayPts = s.pointsLedger
        .filter((e) => e.day === key)
        .reduce((a, e) => a + e.amount, 0);
      const alreadyAwarded = s.pointsLedger.some((e) => e.cardId === card.id);
      const base = card.clinicalOrMaster ? 2 : 1;
      const award = !alreadyAwarded && todayPts + base <= DAILY_CAP ? base : 0;
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? {
              ...c,
              status: "done",
              lastTouchedAt: now,
              completedAt: now,
              pointsAwarded: c.pointsAwarded + award,
              actor: "user" as const,
            }
          : c,
      );
      s.agentRuns = (s.agentRuns ?? []).map((r) =>
        r.cardId === card.id && r.state !== "done" ? { ...r, state: "done" as const } : r,
      );
      if (award > 0) {
        s.pointsLedger = [
          ...s.pointsLedger,
          {
            id: `pt-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
            cardId: card.id,
            amount: award,
            day: key,
            at: now,
            actor: "user" as const,
          },
        ];
      }
      // No unclaim here: the walk-off narrates first, then the rail's
      // habitat:walked-off handler unclaims (the existing Done eviction).
    });
    logEvent("board-move", { from: card.status, to: "done", cardId: card.id }, card.projectId);
  }

  // Checkbox: complete awards points once per card (daily cap honored); the
  // window keeps its claim until the walk-off lands and the rail unclaims —
  // the same rules as the rail ✓. Unchecking returns the card as a loose todo.
  function toggleCard(card: KanbanCard) {
    const now = Date.now() / 1000;
    if (card.status === "done") {
      mutate((s) => {
        s.kanbanCards = s.kanbanCards.map((c) =>
          c.id === card.id
            ? {
                ...c,
                status: "todo",
                windowId: null,
                completedAt: null,
                lastTouchedAt: now,
              }
            : c,
        );
      });
      logEvent("board-move", { from: "done", to: "todo", cardId: card.id }, card.projectId);
      return;
    }
    completeCard(card);
  }

  // Board drag: drop on a column = status move. Dropping on Done is the
  // same complete door as the checkbox. Todo/Doing keep the window claim
  // (town: todo = board, doing = field). Unclaim is Unclaim, not a column.
  function moveCard(card: KanbanCard, target: Status) {
    if (card.status === target) return;
    if (target === "done") {
      completeCard(card);
      return;
    }
    const now = Date.now() / 1000;
    // Paint the column this frame. Doing-fire / walker attach run after
    // the save returns — awaiting them here is the todo→doing hitch.
    kanbanCards = kanbanCards.map((c) =>
      c.id === card.id
        ? {
            ...c,
            status: target,
            completedAt: null,
            lastTouchedAt: now,
          }
        : c,
    );
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? {
              ...c,
              status: target,
              completedAt: null,
              lastTouchedAt: now,
            }
          : c,
      );
    });
    logEvent("board-move", { from: card.status, to: target, cardId: card.id }, card.projectId);
  }

  // detail drawer mutations (all additive on the card)

  function openDetail(card: KanbanCard) {
    if (card.kind !== "needed") {
      if (neededSelectedId) armNeededMouth(null, null);
      neededSelectedId = null;
    }
    detailId = card.id;
    newTagName = "";
    previewEdit = false;
    planetDdOpen = false;
  }

  async function toggleDoing(card: KanbanCard, e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (card.status === "doing") {
      moveCard(card, "todo");
      return;
    }
    const agentId = (card.assignedAgentId || "").trim();
    if (agentId) {
      try {
        await invoke("agent_pickup", { id: agentId, cardId: card.id });
        await loadState();
        return;
      } catch {
        // Harness miss still parks the card so the walker can mint.
      }
    }
    moveCard(card, "doing");
  }

  function skipNeededCard(card: KanbanCard) {
    const now = Date.now() / 1000;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? { ...c, neededState: "skipped" as const, lastTouchedAt: now }
          : c,
      );
    });
    logEvent("needed-skip", { cardId: card.id }, card.projectId);
  }

  function laterNeededCard(card: KanbanCard) {
    const now = Date.now() / 1000;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? { ...c, neededState: "later" as const, lastTouchedAt: now }
          : c,
      );
    });
    logEvent("needed-later", { cardId: card.id }, card.projectId);
  }

  // BOARD-SPLIT-Y: Needed + Trackers share one Y band — dragging either
  // splitter updates both ratios so column 1 and column 2 stay aligned.
  function onSplitDown(e: PointerEvent) {
    const el = mailEl;
    if (!el) return;
    e.preventDefault();
    const startY = e.clientY;
    const start = neededSplit;
    const h = Math.max(el.clientHeight, 1);
    const move = (ev: PointerEvent) => {
      const v = clampSharedSplit(start + (ev.clientY - startY) / h);
      neededSplit = v;
      trackerSplit = v;
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      const v = neededSplit;
      mutate((s) => {
        s.neededSplit = v;
        s.trackerSplit = v;
      });
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  // Left column min = selected planet tab width (the chip+name above
  // the mail stack). Preview min = one title row like "LED watch dogfood".
  function mailMinPx(): number {
    const tab = document.querySelector(
      ".kanban .planet-strip .planet-tab.sel",
    ) as HTMLElement | null;
    if (tab) return Math.max(96, Math.ceil(tab.getBoundingClientRect().width));
    return 120;
  }

  function previewMinPx(): number {
    const probe = document.createElement("span");
    probe.style.cssText =
      "position:absolute;left:-9999px;font:400 0.8rem 'SF Pro Text',system-ui,sans-serif;white-space:nowrap";
    probe.textContent = "LED watch dogfood";
    document.body.appendChild(probe);
    const text = probe.getBoundingClientRect().width;
    probe.remove();
    return Math.max(160, Math.ceil(text + 11 + 7 + 16));
  }

  function clampPreview(px: number): number {
    const body = bodyEl;
    const prevMin = previewMinPx();
    if (!body) return Math.max(prevMin, px);
    const cs = getComputedStyle(body);
    const pad =
      (parseFloat(cs.paddingLeft) || 0) + (parseFloat(cs.paddingRight) || 0);
    const avail = Math.max(body.clientWidth - pad - 6, 1);
    const maxPreview = Math.max(prevMin, avail - mailMinPx());
    return Math.min(maxPreview, Math.max(prevMin, px));
  }

  function onEastSplitDown(e: PointerEvent) {
    if (!bodyEl) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    const startX = e.clientX;
    const start = previewWidth;
    const move = (ev: PointerEvent) => {
      previewWidth = clampPreview(start - (ev.clientX - startX));
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      const v = previewWidth;
      mutate((s) => {
        s.previewWidth = v;
      });
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  function resetPreviewWidth() {
    previewWidth = clampPreview(280);
    mutate((s) => {
      s.previewWidth = previewWidth;
    });
  }

  // BOARD-SPLIT-Y: Needed + Trackers share one clamp band so mirrored
  // ratios stay equal (was clampTracker 0.14–0.55 vs needed 0.18–0.45).
  function clampSharedSplit(frac: number): number {
    return Math.min(0.45, Math.max(0.18, frac));
  }

  function onTrackSplitDown(e: PointerEvent) {
    const el = eastEl;
    if (!el) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    const startY = e.clientY;
    const start = trackerSplit;
    const h = Math.max(el.clientHeight, 1);
    const move = (ev: PointerEvent) => {
      const v = clampSharedSplit(start + (ev.clientY - startY) / h);
      trackerSplit = v;
      neededSplit = v;
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      const v = trackerSplit;
      mutate((s) => {
        s.trackerSplit = v;
        s.neededSplit = v;
      });
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  function resetTrackerSplit() {
    trackerSplit = 0.28;
    neededSplit = 0.28;
    mutate((s) => {
      s.trackerSplit = 0.28;
      s.neededSplit = 0.28;
    });
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

  // GRAB leftover (arrows in charge): the drawer's ring stops — every
  // selectable on that surface, in paint order. A field confirms by
  // FOCUSING it (the caret stays the field's own — pad grammar never
  // steals it, Esc blurs); a button confirms by doing what its click
  // does. Closing the drawer pops the surface back to the board.
  function drawerStops(card: KanbanCard): RingStop[] {
    const focusStop = (id: string) => () => {
      document
        .querySelector<HTMLElement>(`[data-mkey="${CSS.escape(id)}"]`)
        ?.focus();
    };
    const stops: RingStop[] = [
      { id: "d-title", confirm: focusStop("d-title") },
      { id: "d-planet", confirm: focusStop("d-planet") },
      { id: "d-agent", confirm: focusStop("d-agent") },
      { id: "d-notes", confirm: focusStop("d-notes") },
    ];
    stops.push({ id: "d-due", confirm: focusStop("d-due") });
    if (card.dueAt) {
      stops.push({ id: "d-due-clear", confirm: () => setDue(card, "") });
    }
    for (const sub of card.subtasks ?? []) {
      stops.push({
        id: `d-sub-check-${sub.id}`,
        confirm: () => toggleSubtask(card, sub.id),
      });
      stops.push({
        id: `d-sub-del-${sub.id}`,
        confirm: () => deleteSubtask(card, sub.id),
      });
    }
    stops.push({ id: "d-tag-new", confirm: focusStop("d-tag-new") });
    for (const t of tags) {
      stops.push({
        id: `d-tag-${t.id}`,
        confirm: () => toggleCardTag(card, t.id),
      });
    }
    if ((card.kind === "window" || card.windowId) && winFor(card)) {
      stops.push({ id: "d-win-focus", confirm: () => focusCard(card) });
    }
    for (let n = 1; n <= 7; n++) {
      stops.push({
        id: `d-rate-${n}`,
        confirm: () => setRating(card, card.rating === n ? null : n),
      });
    }
    if (card.kind !== "window" && card.kind !== "needed") {
      stops.push({ id: "d-trash", confirm: () => deleteTodo(card) });
    }
    return stops;
  }

  function saveTitle(card: KanbanCard, raw: string) {
    const title = raw.trim();
    if (!title || title === card.title) return;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id ? { ...c, title, lastTouchedAt: Date.now() / 1000 } : c,
      );
    });
  }

  async function undoThisTask(card: KanbanCard) {
    try {
      await invoke("undo_task", { cardId: card.id });
      await loadState();
    } catch {
      /* no before snapshot */
    }
  }

  function saveNotes(card: KanbanCard, raw: string) {
    const notes = raw.trim();
    if (notes === (card.notes ?? "")) return;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id ? { ...c, notes: notes || undefined } : c,
      );
    });
  }

  function setPlanet(card: KanbanCard, projectId: string) {
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? { ...c, projectId, lastTouchedAt: Date.now() / 1000 }
          : c,
      );
    });
    logEvent("board-planet", { cardId: card.id, projectId }, projectId || undefined);
  }

  function setAgent(card: KanbanCard, agentId: string) {
    const assignedAgentId = agentId || null;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? {
              ...c,
              assignedAgentId,
              actor: assignedAgentId ? "agent" : c.actor,
              lastTouchedAt: Date.now() / 1000,
            }
          : c,
      );
    });
    logEvent(
      "board-assign",
      { cardId: card.id, agentId: assignedAgentId },
      card.projectId,
    );
  }

  function newCardId(): string {
    return `c-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
  }

  function childrenOf(parentId: string): KanbanCard[] {
    return kanbanCards.filter(
      (c) => c.parentId === parentId && c.kind !== "needed" && c.kind !== "window",
    );
  }

  function remainingKids(parentId: string): number {
    return childrenOf(parentId).filter((c) => c.status !== "done").length;
  }

  function toggleOpenSubtasks(parentId: string, e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    const on = openSubtaskIds.includes(parentId);
    const next = on
      ? openSubtaskIds.filter((id) => id !== parentId)
      : [...openSubtaskIds, parentId];
    openSubtaskIds = next;
    mutate((s) => {
      s.openSubtaskIds = next;
    });
  }

  function wouldCycle(parentId: string, childId: string): boolean {
    let id: string | null | undefined = parentId;
    const seen = new Set<string>();
    while (id) {
      if (id === childId) return true;
      if (seen.has(id)) return true;
      seen.add(id);
      id = kanbanCards.find((c) => c.id === id)?.parentId;
    }
    return false;
  }

  function linkableTodos(parent: KanbanCard): KanbanCard[] {
    return kanbanCards.filter(
      (c) =>
        c.id !== parent.id &&
        c.kind !== "needed" &&
        c.kind !== "window" &&
        c.parentId !== parent.id &&
        !wouldCycle(parent.id, c.id),
    );
  }

  function mintTodo(title: string, parentId?: string) {
    const t = title.trim();
    if (!t) return;
    const now = Date.now() / 1000;
    const projectId =
      projects.some((p) => p.id === selected) ? selected : "";
    const card: KanbanCard = {
      id: newCardId(),
      title: t,
      projectId,
      kind: "todo",
      status: "todo",
      createdAt: now,
      lastTouchedAt: now,
      goalAligned: true,
      clinicalOrMaster: false,
      pointsAwarded: 0,
      parentId: parentId || undefined,
    };
    mutate((s) => {
      s.kanbanCards = [...s.kanbanCards, card];
    });
    logEvent("board-add", { cardId: card.id, title: t, parentId }, projectId || undefined);
    if (!parentId) {
      addingTodo = false;
      newTodoTitle = "";
      detailId = card.id;
    }
  }

  function attachChild(parent: KanbanCard, childId: string) {
    if (!childId || wouldCycle(parent.id, childId)) return;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === childId
          ? { ...c, parentId: parent.id, lastTouchedAt: Date.now() / 1000 }
          : c,
      );
    });
  }

  function detachChild(childId: string) {
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === childId
          ? { ...c, parentId: undefined, lastTouchedAt: Date.now() / 1000 }
          : c,
      );
    });
  }

  function setDue(card: KanbanCard, dateValue: string) {
    // Local noon sidesteps DST/timezone edge flips on the stored epoch.
    const dueAt = dateValue
      ? new Date(`${dateValue}T12:00:00`).getTime() / 1000
      : null;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id ? { ...c, dueAt } : c,
      );
    });
  }

  function dateInputValue(card: KanbanCard): string {
    if (!card.dueAt) return "";
    const d = new Date(card.dueAt * 1000);
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }

  function cardHasTag(card: KanbanCard, tagId: string): boolean {
    return (card.tagIds ?? []).includes(tagId);
  }

  function addTagToCard(card: KanbanCard, raw: string) {
    const name = raw.trim();
    if (!name) return;
    mutate((s) => {
      s.tags ??= [];
      let tag = s.tags.find((t) => t.name.toLowerCase() === name.toLowerCase());
      if (!tag) {
        tag = {
          id: `t-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
          name,
        };
        s.tags = [...s.tags, tag];
      }
      const tagId = tag.id;
      s.kanbanCards = s.kanbanCards.map((c) => {
        if (c.id !== card.id) return c;
        const cur = c.tagIds ?? [];
        if (cur.includes(tagId)) return c;
        return { ...c, tagIds: [...cur, tagId] };
      });
    });
    newTagName = "";
  }

  function toggleCardTag(card: KanbanCard, tagId: string) {
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) => {
        if (c.id !== card.id) return c;
        const cur = c.tagIds ?? [];
        const next = cur.includes(tagId)
          ? cur.filter((id) => id !== tagId)
          : [...cur, tagId];
        return { ...c, tagIds: next.length ? next : undefined };
      });
    });
  }

  function toggleSubtask(card: KanbanCard, subId: string) {
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? {
              ...c,
              subtasks: (c.subtasks ?? []).map((sub) =>
                sub.id === subId ? { ...sub, done: !sub.done } : sub,
              ),
            }
          : c,
      );
    });
  }

  function deleteTodo(card: KanbanCard) {
    if (card.kind === "window" || card.kind === "needed") return;
    const id = card.id;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards
        .filter((c) => c.id !== id)
        .map((c) =>
          c.parentId === id
            ? { ...c, parentId: undefined, lastTouchedAt: Date.now() / 1000 }
            : c,
        );
      s.agentRuns = (s.agentRuns ?? []).filter((r) => r.cardId !== id);
    });
    logEvent("board-delete", { cardId: id, title: card.title }, card.projectId);
    previewEdit = false;
    if (detailId === id) detailId = null;
    if (neededSelectedId === id) neededSelectedId = null;
  }

  function setRating(card: KanbanCard, n: number | null) {
    const rating = n != null && n >= 1 && n <= 7 ? n : null;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? { ...c, rating, lastTouchedAt: Date.now() / 1000 }
          : c,
      );
    });
  }

  function clipTrackLabel(s: string): string {
    return s.trim().slice(0, 7);
  }

  function addTracker(projectId: string, raw: string) {
    const label = clipTrackLabel(raw);
    if (!label || !projectId) return;
    const tr: Tracker = {
      id: `tr-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
      label,
      value: 0,
    };
    mutate((s) => {
      s.projects = s.projects.map((p) =>
        p.id === projectId
          ? { ...p, trackers: [...(p.trackers ?? []), tr] }
          : p,
      );
    });
    newTrackerLabel = "";
    addingTracker = false;
  }

  function patchTracker(
    projectId: string,
    trackerId: string,
    fn: (t: Tracker) => Tracker,
  ) {
    mutate((s) => {
      s.projects = s.projects.map((p) => {
        if (p.id !== projectId) return p;
        return {
          ...p,
          trackers: (p.trackers ?? []).map((t) =>
            t.id === trackerId ? fn(t) : t,
          ),
        };
      });
    });
  }

  function bumpTracker(projectId: string, trackerId: string, delta: number) {
    patchTracker(projectId, trackerId, (t) => ({ ...t, value: t.value + delta }));
  }

  function setTrackerValue(projectId: string, trackerId: string, raw: string) {
    const n = parseInt(raw.trim(), 10);
    if (!Number.isFinite(n)) return;
    const cur = (projects.find((p) => p.id === projectId)?.trackers ?? []).find(
      (t) => t.id === trackerId,
    );
    if (!cur || cur.value === n) return;
    patchTracker(projectId, trackerId, (t) => ({ ...t, value: n }));
  }

  function removeTracker(projectId: string, trackerId: string) {
    mutate((s) => {
      s.projects = s.projects.map((p) => {
        if (p.id !== projectId) return p;
        const next = (p.trackers ?? []).filter((t) => t.id !== trackerId);
        return { ...p, trackers: next.length ? next : undefined };
      });
    });
  }

  function deleteSubtask(card: KanbanCard, subId: string) {
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? { ...c, subtasks: (c.subtasks ?? []).filter((sub) => sub.id !== subId) }
          : c,
      );
    });
  }

  // board drag handlers

  function onCardDragStart(e: DragEvent, _card: KanbanCard) {
    // Pointer path owns the mouse. Prevent native WK drag so it cannot
    // pointercancel the stream at ~4px. DnD handlers stay as a belt.
    e.preventDefault();
  }

  function onCardDragEnd() {
    dragCardId = null;
    dropCol = null;
  }

  function isCardDrag(e: DragEvent): boolean {
    return !!dragCardId || !!e.dataTransfer?.types?.includes("text/plain");
  }

  function onColDragOver(e: DragEvent, col: Status) {
    if (!isCardDrag(e)) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    if (dropCol !== col) dropCol = col;
  }

  function onColDragLeave(e: DragEvent, col: Status) {
    const next = e.relatedTarget as Node | null;
    if (
      next &&
      e.currentTarget instanceof HTMLElement &&
      e.currentTarget.contains(next)
    )
      return;
    if (dropCol === col) dropCol = null;
  }

  function onColDrop(e: DragEvent, col: Status) {
    e.preventDefault();
    const id = e.dataTransfer?.getData("text/plain") || dragCardId;
    const card = kanbanCards.find((c) => c.id === id);
    if (card && col === "done") moveCard(card, col);
    dragCardId = null;
    dropCol = null;
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
    if (e.button !== 0 || grabbedId) return;
    if (!(e.currentTarget instanceof HTMLElement)) return;
    suppressClick = false;
    const row = e.currentTarget.closest(".b-row");
    const r = (row instanceof HTMLElement
      ? row
      : e.currentTarget
    ).getBoundingClientRect();
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      /* capture is best-effort */
    }
    ptr = {
      id: card.id,
      x: e.clientX,
      y: e.clientY,
      held: false,
      ox: e.clientX - r.left,
      oy: e.clientY - r.top,
      w: r.width,
      h: r.height,
    };
    window.addEventListener("pointermove", onPtrMove);
    window.addEventListener("pointerup", onPtrUp);
    window.addEventListener("pointercancel", onPtrCancel);
    window.addEventListener("keydown", onPtrKey, true);
    window.addEventListener("blur", onPtrCancel);
  }

  function onPtrMove(e: PointerEvent) {
    const p = ptr;
    if (!p) return;
    if (!p.held) {
      if (Math.hypot(e.clientX - p.x, e.clientY - p.y) < 2) return;
      const card = kanbanCards.find((c) => c.id === p.id);
      if (!card) return ptrEnd();
      startGrab(card);
      p.held = true;
      grabGhost = {
        x: e.clientX - p.ox,
        y: e.clientY - p.oy,
        w: p.w,
        h: p.h,
      };
    }
    if (!grabbedId) return;
    const card = kanbanCards.find((c) => c.id === grabbedId);
    if (!card) return;
    grabCol = card.status;
    grabIdx = rowAtPoint(card.status, e.clientY);
    if (grabGhost) {
      grabGhost = {
        ...grabGhost,
        x: e.clientX - p.ox,
        y: e.clientY - p.oy,
      };
    }
  }

  function onPtrUp() {
    const p = ptr;
    if (!p) return;
    const wasHeld = p.held;
    const stillHeld = !!grabbedId;
    ptrEnd();
    if (!wasHeld) return;
    suppressClick = true;
    if (stillHeld) dropGrab();
  }

  function onPtrCancel() {
    if (ptr?.held && grabbedId) ungrab();
    ptrEnd();
  }

  function ptrEnd() {
    ptr = null;
    window.removeEventListener("pointermove", onPtrMove);
    window.removeEventListener("pointerup", onPtrUp);
    window.removeEventListener("pointercancel", onPtrCancel);
    window.removeEventListener("keydown", onPtrKey, true);
    window.removeEventListener("blur", onPtrCancel);
  }

  function onPtrKey(e: KeyboardEvent) {
    if (e.key !== "Escape" || !ptr?.held || !grabbedId) return;
    ungrab();
    e.preventDefault();
    e.stopPropagation();
  }

  function colAtPoint(x: number, y: number): Status | null {
    const id = document
      .elementFromPoint(x, y)
      ?.closest("[data-col]")
      ?.getAttribute("data-col");
    return COLUMNS.some((c) => c.id === id) ? (id as Status) : null;
  }

  function rowAtPoint(col: Status, y: number): number {
    const els = document.querySelectorAll(
      `.${col}-lane .b-card:not(.held)`,
    );
    let i = 0;
    for (const el of els) {
      const r = el.getBoundingClientRect();
      if (y > r.top + r.height / 2) i++;
    }
    return i;
  }

  function onCardClick(card: KanbanCard) {
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    openDetail(card);
  }

  async function refreshWindows() {
    try {
      windows = await invoke<WindowInfo[]>("list_windows");
    } catch {
      // AX not granted here yet; window icons just stay generic
    }
  }

  async function focusCard(card: KanbanCard) {
    const w = winFor(card);
    if (!w) return;
    try {
      await invoke("focus_window", { id: w.id });
    } catch {
      // app may have exited between polls; next refresh reflects reality
    }
  }

  // S5 Hermes: launch/focus the EXISTING harness with this list's open
  // titles (window cards already carry "App — Title"). Never file contents.
  async function launchHermes() {
    const p = selectedProject;
    if (!p) return;
    const titles = kanbanCards
      .filter(
        (c) =>
          c.kind !== "needed" &&
          c.kind !== "window" &&
          c.status !== "done" &&
          listOf(c) === p.id,
      )
      .map((c) => c.title);
    try {
      const route = await invoke<string>("launch_coordinator", {
        projectId: p.id,
        projectName: p.name,
        titles,
      });
      hermesNote = { kind: "ok", text: route };
    } catch (e) {
      hermesNote = { kind: "err", text: `${e}` };
    }
  }

  $effect(() => {
    if (!themeId) return;
    document.documentElement.dataset.theme = themeId;
    applyHostTokens(themeId);
    // S4.4: re-assert the native transparency surgery post-load.
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
      applyFocusProject(e.payload);
    });
    // HOST-SPLIT embedded: do not await load_state / AX on first well
    // mount — that IPC return was painting cards mid-rise and lagging
    // the sprite aside. Overlay Launchpad still hydrates in-line.
    if (embedded) {
      detachMenuKeys = menuRing.attach(() => {
        // KANBAN-GRAB: Esc (pad B) lets go of a held card first. An open
        // drawer is its own ring surface and eats its Esc ahead of this
        // root handler — the detailId branch only covers the sub-frame
        // before the surface push lands. Then (CompanionPage's bubble
        // Esc) the well.
        if (grabbedId) {
          ungrab();
          return true;
        }
        return false;
      });
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
          if (pendingFocusProject) {
            selected = pendingFocusProject;
            if (projects.some((p) => p.id === pendingFocusProject)) {
              raisePlanet(pendingFocusProject);
            }
          }
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


  // BOARD-SPLIT-Y (locked rule):
  // - empty shelf → one title row (never stretch)
  // - both have content → match bottoms to the taller natural height
  $effect(() => {
    const n = neededEl;
    const t = trackPaneEl;
    const mail = mailEl;
    void neededRows.length;
    void visibleTrackers.length;
    void addingNeeded;
    void addingTracker;
    void neededSplit;
    void trackerSplit;
    if (!n || !t || !mail) return;

    const neededIsEmpty = neededRows.length === 0 && !addingNeeded;
    const trackersIsEmpty = visibleTrackers.length === 0 && !addingTracker;

    const clear = () => {
      n.style.height = "";
      n.style.minHeight = "";
      n.style.maxHeight = "";
      t.style.height = "";
      t.style.minHeight = "";
      t.style.maxHeight = "";
    };

    if (neededIsEmpty || trackersIsEmpty) {
      clear();
      return;
    }

    const sync = () => {
      // Measure natural %-basis heights, then equalize to the taller.
      clear();
      const h = Math.max(n.offsetHeight, t.offsetHeight);
      if (h <= 0) return;
      n.style.height = `${h}px`;
      n.style.minHeight = `${h}px`;
      n.style.maxHeight = `${h}px`;
      t.style.height = `${h}px`;
      t.style.minHeight = `${h}px`;
      t.style.maxHeight = `${h}px`;
    };

    // After layout so % flex-basis has resolved.
    requestAnimationFrame(sync);
    const ro = new ResizeObserver(() => requestAnimationFrame(sync));
    ro.observe(mail);
    return () => {
      ro.disconnect();
      clear();
    };
  });


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

<main class="kanban" class:embedded>
  <BoardChrome
    bind:selected
    bind:hermesNote
    {embedded}
    {dropCol}
    {grabCol}
    {invoke}
    {listTitle}
    {menuRing}
    {onBannerMouseDown}
    {onClose}
    {onColDragLeave}
    {onColDragOver}
    {onColDrop}
    {onPlanetPointerDown}
    {onPlanetTabClick}
    {onPlanetTabDblClick}
    {onPopout}
    {planetCss}
    {planetDragId}
    {planetDropId}
    {planetIsLive}
    {projects}
    {selectedProject}
    {sideCount}
    {toggleDoneChip}
    {doneActorChips}
  />

  <div class="k-body" bind:this={bodyEl}>
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
    />

    <BoardPreview
      neededEmpty={neededRows.length === 0 && !addingNeeded}
      bind:addingTracker
      bind:eastEl
      bind:newTagName
      bind:newTrackerLabel
      bind:newTrackerPlanetId
      bind:planetDdOpen
      bind:previewEdit
      bind:previewWidth
      bind:selected
      bind:trackPaneEl
      bind:trackerSplit
      {addTagToCard}
      {addTracker}
      {agentName}
      {agentShortcuts}
      {attachChild}
      {bumpTracker}
      {cardHasTag}
      {childrenOf}
      {clipTrackLabel}
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
      {onTrackSplitDown}
      {openDetail}
      {planetCss}
      {projectName}
      {projects}
      {removeTracker}
      {resetPreviewWidth}
      {resetTrackerSplit}
      {saveNotes}
      {saveTitle}
      {selectedProject}
      {setAgent}
      {setDue}
      {setPlanet}
      {setRating}
      {setTrackerValue}
      {showTrackers}
      {tags}
      {toggleCardTag}
      {toggleSubtask}
      {trackerArmed}
      {undoThisTask}
      {visibleTrackers}
      {winFor}
    />
  </div>
</main>


