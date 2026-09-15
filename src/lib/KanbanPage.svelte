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
  import { MenuRing, type RingDir, type RingStop } from "./menuKeys.svelte";
  import { mdLite } from "./mdLite";
  import "./chrome.css";
  import "./boardStack.css";
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
  // AGENT-PLANET leftover: writer on a planet glows that chip in the
  // Launchpad strip (same flag the inspector sheet tabs already read).
  let hackProjectId = $state<string | null>(null);
  let selectedBeforeDone = $state("inbox");
  let neededSplit = $state(0.28);
  let mailEl: HTMLDivElement | null = $state(null);
  let addingTodo = $state(false);
  let newTodoTitle = $state("");
  let openSubtaskIds = $state<string[]>([]);
  let newTagName = $state("");
  let previewEdit = $state(false);
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
  // PLANETS-STRIP: rail-owned marble flag, read-only here — tab double-click
  // emits to main; the rail persist is the writer.
  let collapsedSectionIds = $state<string[]>([]);

  let detailId = $state<string | null>(null);

  // The Launchpad ring — Needed pile, the planet
  // strip (a row: left/right walk it), then the board piles
  // column-major. Up/down walk the pile, left/right hop the highlight
  // to the same row in the next non-empty column (wraps); Enter selects
  // a Needed row (the composer answers it), selects a list tab, or
  // descends into a card's detail drawer. Esc backs one surface, then
  // the well (CompanionPage's bubble Esc). Embedded only — the overlay
  // Lists window keeps its mouse habits.
  // Shift+Enter (pad X) grabs the focused card —
  // it paints held at the grab cursor while arrows move it (left/right =
  // column, clamped at the ends, empty columns legal; up/down = row in
  // the column). Enter (pad A) drops through moveCard — the ONE save
  // door a mouse drag takes, so doing-fire still owns a GO-* todo →
  // doing. Esc (pad B) lets go: nothing ever mutated, the card is still
  // in its pre-grab column.
  // GRAB leftover (arrows in charge): the ring never parks. The drawer
  // is a pushed SURFACE on this one ring — not an empty stops list — so
  // after any mouse click (a card, a column, well chrome) arrows keep
  // walking the current surface's selectables instead of falling back
  // to page scroll, and Esc pops back to the board with its highlight.
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

  function raisePlanet(id: string) {
    invoke("bring_planet_front", { id }).catch(() => {});
    invoke("locate_planet", { id }).catch(() => {});
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
    const x0 = e.clientX;
    const y0 = e.clientY;
    let dragging = false;
    const onMove = (ev: PointerEvent) => {
      if (!dragging) {
        if (Math.hypot(ev.clientX - x0, ev.clientY - y0) < 6) return;
        dragging = true;
        planetDidDrag = true;
        planetDragId = id;
      }
      const before = planetBeforeAt(ev.clientX);
      planetDropId = before;
      reorderPlanetsLive(id, before);
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      if (dragging) persistPlanetOrder();
      planetDragId = null;
      planetDropId = null;
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
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
    const scoped = kanbanCards.filter((c) => inScope(c) && !c.parentId);
    const byAge = (a: KanbanCard, b: KanbanCard) => a.createdAt - b.createdAt;
    return {
      todo: scoped.filter((c) => c.status === "todo").sort(byAge),
      doing: scoped.filter((c) => c.status === "doing").sort(byAge),
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
  let viewCols = $derived.by(() => {
    if (!grabbedId || !grabCol) return boardCols;
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
    const c = kanbanCards.find((x) => x.id === detailId);
    if (c?.kind === "needed") onNeededFocus?.(c.id, c.title);
    else onNeededFocus?.(null, null);
  });

  $effect(() => {
    const pile = [...neededRows, ...viewCols.doing, ...viewCols.todo];
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
    grabbedId = null;
    grabCol = null;
    grabIdx = 0;
    const card = id ? kanbanCards.find((c) => c.id === id) : undefined;
    // The ONE save door — the same moveCard a mouse drop takes (Done is
    // the complete door; doing-fire diffs this save for a GO-* todo).
    if (card && target) moveCard(card, target);
  }

  function ungrab() {
    // Esc (pad B): no mutation ever happened — the card is still in its
    // pre-grab column; the held paint just stops.
    grabbedId = null;
    grabCol = null;
    grabIdx = 0;
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

  // ---- row meta (notes snippet, difficulty pip, due chip, sub count) ----

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
      tags = s.tags ?? [];
      kanbanCards = s.kanbanCards ?? [];
      pointsLedger = s.pointsLedger ?? [];
      titleOverrides = s.titleOverrides ?? [];
      collapsedSectionIds = s.collapsedSectionIds ?? [];
      agentShortcuts = s.agentShortcuts ?? [];
      agentRuns = s.agentRuns ?? [];
      neededSplit = Math.min(0.45, Math.max(0.18, s.neededSplit ?? 0.28));
      openSubtaskIds = s.openSubtaskIds ?? [];
      hackProjectId = s.companion?.hackLive
        ? (s.companion?.hackProjectId ?? null)
        : null;
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

  // ---- detail drawer mutations (all additive on the card) ----

  function openDetail(card: KanbanCard) {
    if (card.kind !== "needed") neededSelectedId = null;
    detailId = card.id;
    newTagName = "";
    previewEdit = false;
  }

  function toggleDoing(card: KanbanCard, e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    moveCard(card, card.status === "doing" ? "todo" : "doing");
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

  function onSplitDown(e: PointerEvent) {
    const el = mailEl;
    if (!el) return;
    e.preventDefault();
    const startY = e.clientY;
    const start = neededSplit;
    const h = Math.max(el.clientHeight, 1);
    const move = (ev: PointerEvent) => {
      neededSplit = Math.min(0.45, Math.max(0.18, start + (ev.clientY - startY) / h));
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      const v = neededSplit;
      mutate((s) => {
        s.neededSplit = v;
      });
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

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

  function deleteSubtask(card: KanbanCard, subId: string) {
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? { ...c, subtasks: (c.subtasks ?? []).filter((sub) => sub.id !== subId) }
          : c,
      );
    });
  }

  // ---- board drag handlers ----

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
    if (card) moveCard(card, col);
    dragCardId = null;
    dropCol = null;
  }

  // Pointer drag is the grab path (restored to c2997ce). Shift+Enter unchanged.
  let ptr: { id: string; x: number; y: number; held: boolean } | null = null;
  let suppressClick = false;

  function onCardPointerDown(e: PointerEvent, card: KanbanCard) {
    if (e.button !== 0 || grabbedId) return;
    suppressClick = false;
    ptr = { id: card.id, x: e.clientX, y: e.clientY, held: false };
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
      if (Math.hypot(e.clientX - p.x, e.clientY - p.y) < 4) return;
      const card = kanbanCards.find((c) => c.id === p.id);
      if (!card) return ptrEnd();
      startGrab(card);
      p.held = true;
    }
    if (!grabbedId) return;
    const col = colAtPoint(e.clientX, e.clientY);
    if (!col) return;
    grabCol = col;
    grabIdx = rowAtPoint(col, e.clientY);
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
      `[data-col="${col}"] .b-card:not(.held)`,
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
        loadState();
        refreshWindows();
        startRefreshTimer();
      } else {
        stopRefreshTimer();
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
    unlistenVis?.();
    document.removeEventListener("visibilitychange", onVisibility);
  });

  function onBannerMouseDown(e: MouseEvent) {
    startPaneDrag(e);
  }
</script>

{#snippet metaChips(card: KanbanCard)}
  {@const due = dueInfo(card)}
  {@const sub = subProgress(card)}
  {@const who = agentName(card)}
  {@const run = runState(card)}
  {@const age = ageLabel(card)}
  {#if run}
    <span class="k-run" title="Runner state">{run}</span>
  {/if}
  {#if who}
    <span class="k-agent" title="Assigned agent">{who}</span>
  {/if}
  {#if card.actor && card.actor !== "user"}
    <span class="k-actor" title="Actor">{card.actor}</span>
  {/if}
  {#if card.neededState}
    <span class="k-needst" title="Needed state">{card.neededState}</span>
  {/if}
  {#if due}
    <span class="k-due" class:overdue={due.overdue} title="Due date">{due.label}</span>
  {/if}
  {#if sub}
    <span class="k-sub" title="Sub-checklist done/total">{sub}</span>
  {/if}
  {#if showProj(card)}
    <span class="k-proj">{projectName(card.projectId)}</span>
  {/if}
  {#if age}
    <span class="k-age" title="Last touched">{age}</span>
  {/if}
{/snippet}

{#snippet stackCard(card: KanbanCard)}
  {@const pid = livePlanetId(card)}
  <article
    class="b-card"
    class:dragging={dragCardId === card.id}
    class:held={grabbedId === card.id}
    class:done={card.status === "done"}
    class:doing={card.status === "doing"}
    class:child={!!card.parentId}
    class:sel={detailId === card.id}
    class:mkey-sel={menuRing.activeId === `card-${card.id}`}
    data-mkey={`card-${card.id}`}
    draggable="true"
    ondragstart={(e) => onCardDragStart(e, card)}
    ondragend={onCardDragEnd}
    onpointerdown={(e) => onCardPointerDown(e, card)}
    onclick={() => onCardClick(card)}
    onkeydown={(e) => e.key === "Enter" && !e.shiftKey && openDetail(card)}
    tabindex="0"
    title="{displayTitle(card)} — click for details, drag to move"
  >
    <div class="b-card-top">
      {#if pid}
        <span
          class="planet-chip"
          style="background: {planetCss(pid)}"
          title={projectName(pid)}
          aria-hidden="true"
        ></span>
      {/if}
      <span class="b-title" class:strike={card.status === "done"}>{displayTitle(card)}</span>
      {#if card.status !== "done" && !card.parentId}
        <button
          type="button"
          class="k-doing"
          class:on={card.status === "doing"}
          onclick={(e) => toggleDoing(card, e)}
          onpointerdown={(e) => e.stopPropagation()}
          title={card.status === "doing" ? "Doing — click to park" : "Start — pin to the top"}
        >
          {#if card.status === "doing"}
            <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="12" cy="12" r="4"/><circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" stroke-width="1.6"/></svg>
            doing
          {:else}
            <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"/></svg>
            start
          {/if}
        </button>
      {/if}
      {#if !card.parentId && childrenOf(card.id).length > 0}
        <button
          type="button"
          class="k-subs"
          class:open={openSubtaskIds.includes(card.id)}
          onclick={(e) => toggleOpenSubtasks(card.id, e)}
          onpointerdown={(e) => e.stopPropagation()}
          title={openSubtaskIds.includes(card.id) ? "Hide subtasks" : "Show subtasks"}
        >
          <span class="chev" class:open={openSubtaskIds.includes(card.id)}>▸</span>
          {remainingKids(card.id)}
        </button>
      {/if}
    </div>
    <div class="b-meta">
      {@render metaChips(card)}
    </div>
  </article>
{/snippet}

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
  <!-- One drag source: JS startDragging on this banner. Drawers are
       borderless — no native Overlay strip competing with the click.
       HOST-SPLIT: embedded, this drags the host companion window. -->
  <header class="k-banner" onmousedown={onBannerMouseDown}>
    {#if embedded}
      <h1>Launchpad</h1>
      <!-- HOST-SPLIT: pop-out lives on the embedded Launchpad banner only
           (never STARS, never companion chrome) — the real kanban window
           becomes the popped Launchpad. -->
      <button
        class="popout"
        onmousedown={(e) => e.stopPropagation()}
        onclick={() => onPopout?.()}
        title="Pop out to the real Lists window — the well closes, the companion stays"
        aria-label="Pop out Launchpad"
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
    {:else}
      <h1><span class="mark"><span class="o">O</span><span class="rest">rb</span><span class="idot">i</span><span class="pop">tty</span></span> Lists</h1>
    {/if}
    <button
      class="winx"
      onmousedown={(e) => e.stopPropagation()}
      onclick={() =>
        embedded
          ? onClose?.()
          : invoke("hide_window", { label: "kanban" }).catch(() => {})}
      title={embedded
        ? "Dismiss the Launchpad well — the companion stays"
        : "Hide Lists — reopen from the rail or tray"}
      aria-label={embedded ? "Dismiss Launchpad" : "Hide Lists"}>×</button
    >
  </header>

  <div class="planet-strip" role="tablist" aria-label="Lists and planets">
    <div class="strip-pins">
      <button
        type="button"
        class="planet-tab"
        class:sel={selected === "inbox"}
        class:mkey-sel={menuRing.activeId === "listtab-inbox"}
        data-mkey="listtab-inbox"
        role="tab"
        aria-selected={selected === "inbox"}
        onclick={() => (selected = "inbox")}
      >
        <span class="planet-tab-name">Inbox</span>
        <span class="side-count">{sideCount("inbox")}</span>
      </button>
      <button
        type="button"
        class="planet-tab"
        class:sel={selected === "today"}
        class:mkey-sel={menuRing.activeId === "listtab-today"}
        data-mkey="listtab-today"
        role="tab"
        aria-selected={selected === "today"}
        onclick={() => (selected = "today")}
      >
        <span class="planet-tab-name">Today</span>
        <span class="side-count">{sideCount("today")}</span>
      </button>
      <button
        type="button"
        class="planet-tab"
        class:sel={selected === "done"}
        class:mkey-sel={menuRing.activeId === "listtab-done"}
        data-mkey="listtab-done"
        role="tab"
        aria-selected={selected === "done"}
        data-col="done"
        class:drop-ok={dropCol === "done" || grabCol === "done"}
        title="Done — click to show completed, click again to return"
        onclick={() => toggleDoneChip()}
        ondragover={(e) => onColDragOver(e, "done")}
        ondragleave={(e) => onColDragLeave(e, "done")}
        ondrop={(e) => onColDrop(e, "done")}
      >
        <span class="planet-tab-name">Done</span>
        <span class="side-count">{sideCount("done")}</span>
      </button>
    </div>
    <span class="strip-gap" aria-hidden="true"></span>
    <div class="strip-planets">
    {#each projects as p (p.id)}
      <button
        type="button"
        class="planet-tab"
        class:sel={selected === p.id}
        class:drop-ok={planetDropId === p.id}
        class:dragging={planetDragId === p.id}
        class:mkey-sel={menuRing.activeId === `listtab-${p.id}`}
        data-mkey={`listtab-${p.id}`}
        data-pid={p.id}
        role="tab"
        aria-selected={selected === p.id}
        title={p.name}
        onpointerdown={(e) => onPlanetPointerDown(e, p.id)}
        onclick={() => onPlanetTabClick(p.id)}
        ondblclick={() => onPlanetTabDblClick(p.id)}
      >
        <span
          class="planet-chip"
          class:live={hackProjectId === p.id}
          style="background: {planetCss(p.id)}; --pglow: {planetCss(p.id)}"
          aria-hidden="true"
        ></span>
        <span class="planet-tab-name">{p.name}</span>
        <span class="side-count">{sideCount(p.id)}</span>
      </button>
    {/each}
    </div>
  </div>

  <div class="k-heads" onmousedown={onBannerMouseDown}>
    <div class="k-titlerow">
      {#if selectedProject}
        <span
          class="planet-chip"
          class:live={hackProjectId === selectedProject.id}
          style="background: {planetCss(selectedProject.id)}; --pglow: {planetCss(selectedProject.id)}"
          title={selectedProject.name}
          aria-hidden="true"
        ></span>
      {/if}
      <h2 class="k-title">{listTitle}</h2>
      {#if selected === "done"}
        {@render doneActorChips()}
      {/if}
    </div>
    <div class="k-preview-head">Preview:</div>
  </div>
  {#if hermesNote}
    <div class="k-hermesnote" class:err={hermesNote.kind === "err"}>
      <span class="k-hermestext">{hermesNote.text}</span>
      <button class="mini ghost" onclick={() => (hermesNote = null)} aria-label="Dismiss">×</button>
    </div>
  {/if}

  <div class="k-body">
    <section class="k-main" aria-label={listTitle}>

      <div class="mail" bind:this={mailEl}>
        {#if selected === "done"}
          <section class="mail-todo mail-done" aria-label="Done">
            <div
              class="b-zone todo"
              class:drop-ok={dropCol === "done" || grabCol === "done"}
              data-col="done"
              ondragover={(e) => onColDragOver(e, "done")}
              ondragleave={(e) => onColDragLeave(e, "done")}
              ondrop={(e) => onColDrop(e, "done")}
            >
              <header class="b-head">
                <span class="b-name">Done</span>
                <span class="b-count">{viewCols.done.length}</span>
                {@render doneActorChips()}
              </header>
              {#each viewCols.done as card (card.id)}
                {@render stackCard(card)}
              {:else}
                <p class="b-empty">Nothing done yet.</p>
              {/each}
            </div>
          </section>
        {:else}
        <div
          class="mail-needed"
          class:empty={neededRows.length === 0 && !addingNeeded}
          style="flex-basis: {neededRows.length === 0 && !addingNeeded
            ? 'auto'
            : `${Math.round(neededSplit * 100)}%`}"
        >
          <NeededInbox
            variant="strip"
            cards={kanbanCards}
            {projects}
            projectId={neededProjectId}
            {mutate}
            {logEvent}
            bind:addingNeeded
            bind:selectedId={neededSelectedId}
            ringActiveId={menuRing.activeId}
            {onNeededAnswer}
          />
        </div>
        {#if neededRows.length > 0 || addingNeeded}
          <div
            class="mail-split"
            role="separator"
            aria-orientation="horizontal"
            aria-label="Resize Needed"
            title="Drag to resize Needed · double-click resets"
            onpointerdown={onSplitDown}
            ondblclick={() => {
              neededSplit = 0.28;
              mutate((s) => {
                s.neededSplit = 0.28;
              });
            }}
          ></div>
        {/if}
        <section class="mail-todo" aria-label="Todos">
          <div
            class="b-zone todo"
            class:drop-ok={dropCol === "todo" || grabCol === "todo"}
            data-col="todo"
            ondragover={(e) => onColDragOver(e, "todo")}
            ondragleave={(e) => onColDragLeave(e, "todo")}
            ondrop={(e) => onColDrop(e, "todo")}
          >
            <header class="b-head">
              <span class="mail-ico" aria-hidden="true">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
                  <rect x="4.5" y="4.5" width="15" height="15" rx="3.5"/>
                  <path d="M8 12.2l2.6 2.6L16.5 9" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              </span>
              <span class="b-name">Todo</span>
              <span class="b-count">{viewCols.doing.length + viewCols.todo.length}</span>
              <button
                type="button"
                class="ka-plus"
                onclick={() => (addingTodo = !addingTodo)}
                title={addingTodo ? "Cancel" : "Add a to-do"}
                aria-label={addingTodo ? "Cancel add to-do" : "Add a to-do"}
              >{addingTodo ? "×" : "+"}</button>
            </header>
            {#if addingTodo}
              <div class="todo-add">
                <input
                  class="d-input"
                  bind:value={newTodoTitle}
                  placeholder="New to-do…"
                  aria-label="New to-do title"
                  onkeydown={(e) => {
                    if (e.key === "Enter") mintTodo(newTodoTitle);
                    if (e.key === "Escape") addingTodo = false;
                  }}
                />
                <button class="mini" onclick={() => mintTodo(newTodoTitle)}>Add</button>
              </div>
            {/if}
            {#each viewCols.doing as card (card.id)}
              {@render stackCard(card)}
              {#if openSubtaskIds.includes(card.id)}
                {#each childrenOf(card.id) as kid (kid.id)}
                  {@render stackCard(kid)}
                {/each}
              {/if}
            {/each}
            {#each viewCols.todo as card (card.id)}
              {@render stackCard(card)}
              {#if openSubtaskIds.includes(card.id)}
                {#each childrenOf(card.id) as kid (kid.id)}
                  {@render stackCard(kid)}
                {/each}
              {/if}
            {:else}
              {#if viewCols.doing.length === 0}
                <p class="b-empty">No to-dos — drag a card here.</p>
              {/if}
            {/each}
          </div>
        </section>
        {/if}
      </div>
    </section>

    <aside class="k-detail" aria-label="Item details">
      {#if detailCard}
      {@const card = detailCard}
        {#if card.kind === "needed"}
          <p class="d-need-title">{card.title}</p>
          <label class="d-label" for="d-need-planet">Planet</label>
          <select
            id="d-need-planet"
            class="d-input"
            value={livePlanetId(card) ?? ""}
            onchange={(e) => setPlanet(card, e.currentTarget.value)}
          >
            <option value="">Inbox</option>
            {#each projects as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
          {#if card.neededWhy}
            <div class="kn-why">{@html mdLite(card.neededWhy)}</div>
          {/if}
          <div class="d-duerow">
            {#if onNeededAnswer}
              <button
                class="mini"
                onclick={() => onNeededAnswer?.()}
                title="Answer in the composer — Rule: … remembers"
                >Answer</button
              >
            {/if}
            <button class="mini ghost" onclick={() => laterNeededCard(card)}>Later</button>
            <button class="mini ghost" onclick={() => skipNeededCard(card)}>Skip</button>
          </div>
        {:else}
        {#if !previewEdit}
          {@const pid = livePlanetId(card)}
          {@const due = dueInfo(card)}
          <div class="d-read-top">
            <h3 class="d-need-title">{displayTitle(card)}</h3>
            <button type="button" class="mini" onclick={() => (previewEdit = true)}>Edit</button>
          </div>
          {#if pid}
            <p class="d-read-line">
              <span class="planet-chip" style="background: {planetCss(pid)}"></span>
              {projectName(pid)}
            </p>
          {:else}
            <p class="d-read-line dim">Inbox</p>
          {/if}
          {#if agentName(card)}
            <p class="d-read-line">{agentName(card)}</p>
          {/if}
          {#if card.notes}
            <div class="d-read-notes">{card.notes}</div>
          {/if}
          {#if due}
            <p class="d-read-line" class:overdue={due.overdue}>{due.label}</p>
          {/if}
          {#if childrenOf(card.id).length}
            <ul class="d-read-subs">
              {#each childrenOf(card.id) as kid (kid.id)}
                <li class:strike={kid.status === "done"}>{kid.title}</li>
              {/each}
            </ul>
          {/if}
          {#if (card.tagIds ?? []).length}
            <div class="d-tags">
              {#each tags.filter((t) => cardHasTag(card, t.id)) as t (t.id)}
                <span class="mini active">{t.name}</span>
              {/each}
            </div>
          {/if}
        {:else}
        <div class="d-read-top">
          <span class="d-label" style="margin:0">Edit</span>
          <button type="button" class="mini ghost" onclick={() => (previewEdit = false)}>Done</button>
        </div>
        <label class="d-label" for="d-name">Title</label>
        <input
          id="d-name"
          class="d-input"
          class:mkey-sel={menuRing.activeId === "d-title"}
          data-mkey="d-title"
          value={card.title}
          onchange={(e) => saveTitle(card, e.currentTarget.value)}
          required
        />

        <label class="d-label" for="d-planet">Planet</label>
        <select
          id="d-planet"
          class="d-input"
          class:mkey-sel={menuRing.activeId === "d-planet"}
          data-mkey="d-planet"
          value={livePlanetId(card) ?? ""}
          onchange={(e) => setPlanet(card, e.currentTarget.value)}
        >
          <option value="">Inbox</option>
          {#each projects as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>

        <label class="d-label" for="d-agent">Agent</label>
        <select
          id="d-agent"
          class="d-input"
          class:mkey-sel={menuRing.activeId === "d-agent"}
          data-mkey="d-agent"
          value={card.assignedAgentId ?? ""}
          onchange={(e) => setAgent(card, e.currentTarget.value)}
        >
          <option value="">None</option>
          {#each agentShortcuts as a (a.id)}
            <option value={a.id}>{a.name}</option>
          {/each}
        </select>

        <label class="d-label" for="d-notes">Notes</label>
        <textarea
          id="d-notes"
          class="d-input d-textarea d-notes-lg"
          class:mkey-sel={menuRing.activeId === "d-notes"}
          data-mkey="d-notes"
          rows="10"
          placeholder="Add notes…"
          value={card.notes ?? ""}
          onchange={(e) => saveNotes(card, e.currentTarget.value)}
        ></textarea>

        <label class="d-label" for="d-due">Due date</label>
        <div class="d-duerow">
          <input
            id="d-due"
            class="d-input"
            class:mkey-sel={menuRing.activeId === "d-due"}
            data-mkey="d-due"
            type="date"
            value={dateInputValue(card)}
            onchange={(e) => setDue(card, e.currentTarget.value)}
          />
          {#if card.dueAt}
            <button
              class="mini ghost"
              class:mkey-sel={menuRing.activeId === "d-due-clear"}
              data-mkey="d-due-clear"
              onclick={() => setDue(card, "")}>Clear</button
            >
          {/if}
        </div>

        <span class="d-label">Subtasks</span>
        <ul class="d-subs">
          {#each childrenOf(card.id) as kid (kid.id)}
            <li class="d-sub">
              <button
                class="k-check"
                class:checked={kid.status === "done"}
                onclick={() => moveCard(kid, kid.status === "done" ? "todo" : "done")}
                aria-label={kid.status === "done" ? "Reopen subtask" : "Complete subtask"}
              >{kid.status === "done" ? "✓" : ""}</button>
              <button
                type="button"
                class="d-subtitle"
                class:strike={kid.status === "done"}
                onclick={() => openDetail(kid)}
              >{kid.title}</button>
              <button
                class="d-del"
                onclick={() => detachChild(kid.id)}
                aria-label="Unlink subtask"
                title="Unlink — stays a to-do">×</button
              >
            </li>
          {/each}
          {#each card.subtasks ?? [] as sub (sub.id)}
            <li class="d-sub">
              <button
                class="k-check"
                class:checked={sub.done}
                class:mkey-sel={menuRing.activeId === `d-sub-check-${sub.id}`}
                data-mkey={`d-sub-check-${sub.id}`}
                onclick={() => toggleSubtask(card, sub.id)}
                aria-label={sub.done ? "Reopen subtask" : "Complete subtask"}
                title={sub.done ? "Reopen" : "Complete"}>{sub.done ? "✓" : ""}</button
              >
              <span class="d-subtitle" class:strike={sub.done}>{sub.title}</span>
              <button
                class="d-del"
                class:mkey-sel={menuRing.activeId === `d-sub-del-${sub.id}`}
                data-mkey={`d-sub-del-${sub.id}`}
                onclick={() => deleteSubtask(card, sub.id)}
                aria-label="Delete subtask"
                title="Delete">×</button
              >
            </li>
          {/each}
          {#if childrenOf(card.id).length === 0 && !(card.subtasks ?? []).length}
            <li class="d-hint">No subtasks yet — tag a to-do below.</li>
          {/if}
        </ul>
        {#if linkableTodos(card).length > 0}
          <select
            class="d-input"
            aria-label="Tag an existing to-do as a subtask"
            onchange={(e) => {
              attachChild(card, e.currentTarget.value);
              e.currentTarget.value = "";
            }}
          >
            <option value="">Tag a to-do as subtask…</option>
            {#each linkableTodos(card) as t (t.id)}
              <option value={t.id}>{t.title}</option>
            {/each}
          </select>
        {/if}

        <span class="d-label">Tags</span>
        {#if tags.length > 0}
          <div class="d-tags">
            {#each tags as t (t.id)}
              <button
                class="mini"
                class:active={cardHasTag(card, t.id)}
                class:mkey-sel={menuRing.activeId === `d-tag-${t.id}`}
                data-mkey={`d-tag-${t.id}`}
                aria-pressed={cardHasTag(card, t.id)}
                onclick={() => toggleCardTag(card, t.id)}>{t.name}</button
              >
            {/each}
          </div>
        {/if}
        <div class="d-subadd">
          <input
            class="d-input"
            class:mkey-sel={menuRing.activeId === "d-tag-new"}
            data-mkey="d-tag-new"
            bind:value={newTagName}
            placeholder="New tag"
            onkeydown={(e) => e.key === "Enter" && addTagToCard(card, newTagName)}
          />
          <button
            class="mini"
            onclick={() => addTagToCard(card, newTagName)}>Add</button
          >
        </div>

        {#if card.kind === "window" || card.windowId}
          {@const w = winFor(card)}
          <span class="d-label">Linked window</span>
          <div class="d-win">
            {#if w?.iconBase64}
              <img class="k-icon" src="data:image/png;base64,{w.iconBase64}" alt="" />
            {/if}
            <span class="d-winname" title={displayTitle(card)}>
              {card.windowId ? displayTitle(card) : "Not linked"}
            </span>
            {#if w}
              <button
                class="mini"
                class:mkey-sel={menuRing.activeId === "d-win-focus"}
                data-mkey="d-win-focus"
                onclick={() => focusCard(card)}>Focus</button
              >
            {/if}
          </div>
          <p class="d-hint">Window links are made by claiming in the rail — read-only here.</p>
        {/if}
        {/if}
        {/if}
      {:else}
        <p class="d-hint">Highlight a row to preview it here.</p>
      {/if}
    </aside>
  </div>
</main>

<style>
  /* Airy Wunderlist shapes on top of the same theme tokens. Every painted
     fill is the JS-computed rgba var (S4.3) so the slider fades the whole
     window chrome; text/icons/checkboxes stay full strength. */
  .kanban {
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

  :global(.low-fill) .kanban {
    text-shadow: 0 1px 2px var(--bg-solid, transparent);
  }

  /* Embedded in the companion's Launchpad well —
     fill the well pane (the host window owns the outer size), square
     corners inside the companion's rounded clip. */
  .kanban.embedded {
    height: 100%;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    border-radius: 0;
  }

  /* HOST-SPLIT: the Launchpad pop-out control (embedded banner only). */
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
    display: block;
  }

  .k-banner {
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
  .k-banner:active {
    cursor: grabbing;
  }

  .k-banner h1 {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  .winx {
    margin-left: auto;
  }

  .k-body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .k-body > .k-detail {
    display: flex;
  }

  /* Planet orbs across the top of Lists. */
  .planet-strip {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    overflow: hidden;
    padding: 4px 12px 8px;
  }

  .strip-pins {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .strip-planets {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: thin;
  }

  .planet-tab.drop-ok {
    box-shadow: inset 0 0 0 1px var(--accent);
  }

  .planet-tab.dragging {
    opacity: 0.45;
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
    background: var(--bg-fill, var(--bg));
    color: var(--accent);
    font-weight: 700;
    box-shadow: inset 0 0 0 1px var(--focus, #6e4db0);
  }

  .planet-tab-name {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .strip-gap {
    flex: none;
    width: 1px;
    align-self: stretch;
    margin: 2px 6px;
    background: var(--edge);
  }

  /* Overlay scrollbars: zero-width until the scroll area is hovered; never
     take layout space (`overlay` with `auto` fallback on the containers). */
  .k-side,
  .k-main,
  .b-cards,
  .k-detail {
    overflow-y: overlay;
    scrollbar-width: none;
  }

  :is(.k-side, .k-main, .b-cards, .k-detail)::-webkit-scrollbar {
    width: 0;
    background: transparent;
  }

  :is(.k-side, .k-main, .b-cards, .k-detail):hover::-webkit-scrollbar,
  :is(.k-side, .k-main, .b-cards, .k-detail):focus-within::-webkit-scrollbar {
    width: 8px;
  }

  :is(.k-side, .k-main, .b-cards, .k-detail)::-webkit-scrollbar-thumb {
    background: transparent;
    border-radius: 4px;
  }

  :is(.k-side, .k-main, .b-cards, .k-detail):hover::-webkit-scrollbar-thumb {
    background: color-mix(in srgb, var(--dim) 55%, transparent);
  }

  .k-side {
    flex-shrink: 0;
    width: 200px;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 8px;
    background: var(--panel-fill, var(--panel));
    border-right: 1px solid var(--edge);
  }

  .side-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 32px;
    padding: 5px 10px;
    font: inherit;
    font-size: 0.82rem;
    text-align: left;
    color: var(--ink);
    background: transparent;
    border: none;
    border-radius: 7px;
    cursor: pointer;
  }

  .side-row:hover {
    background: var(--bg-fill, var(--bg));
  }

  .side-row.sel {
    background: var(--bg-fill, var(--bg));
    color: var(--accent);
    font-weight: 700;
    box-shadow: inset 0 0 0 1px var(--focus, #6e4db0);
  }

  .side-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The list wears the disc's marble color. */
  .planet-chip {
    flex-shrink: 0;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
  }

  .planet-chip.live {
    box-shadow:
      inset 0 0 0 1px rgba(0, 0, 0, 0.25),
      0 0 8px 2px var(--pglow, var(--accent));
  }

  .side-count {
    flex-shrink: 0;
    font-size: 0.66rem;
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }

  .side-row.sel .side-count {
    color: var(--accent);
  }

  .side-sep {
    height: 1px;
    margin: 6px 10px;
    background: var(--edge);
  }

  .k-main {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 0;
    overflow: hidden;
  }

  .k-title {
    margin: 0;
    font-size: 1.15rem;
    font-weight: 700;
    letter-spacing: 0.01em;
  }

  /* S5: title row with the Hermes launcher (project lists only). */
  .k-titlerow {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .k-titlerow .k-title {
    flex: 1;
    min-width: 0;
  }

  .k-hermesnote {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 2px 10px;
    padding: 6px 10px;
    font-size: 0.72rem;
    line-height: 1.45;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 8px;
    word-break: break-all;
  }

  .k-hermesnote.err {
    border-color: var(--accent);
  }

  .k-hermestext {
    flex: 1;
    min-width: 0;
  }

  .k-rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .k-row {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 38px;
    padding: 4px 10px;
    border-radius: 8px;
  }

  .k-row:hover {
    background: var(--panel-fill, var(--panel));
  }

  .k-check {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    font: inherit;
    font-size: 0.72rem;
    font-weight: 700;
    line-height: 1;
    color: var(--bg);
    background: transparent;
    border: 1.5px solid var(--dim);
    border-radius: 999px;
    cursor: pointer;
  }

  .k-check:hover {
    border-color: var(--focus, #6e4db0);
  }

  .k-check.checked {
    color: var(--bg);
    background: var(--accent);
    border-color: var(--accent);
  }

  .k-icon {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    border-radius: 4px;
    object-fit: contain;
  }

  .k-icon.fallback {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--dim);
    font-size: 0.8rem;
  }

  .k-name {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 8px;
    overflow: hidden;
    white-space: nowrap;
    font: inherit;
    font-size: 0.88rem;
    text-align: left;
    color: var(--ink);
    background: transparent;
    border: none;
    border-radius: 6px;
    padding: 2px 2px;
    cursor: pointer;
  }

  .k-name:hover .k-title-text {
    color: var(--accent);
  }

  .k-title-text {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .k-notes {
    flex-shrink: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 0.72rem;
    color: var(--dim);
  }

  .k-name.strike {
    color: var(--dim);
    text-decoration: line-through;
  }

  /* Difficulty pip — metadata only, never points. */
  .pip {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 999px;
  }

  .pip.trivial { background: var(--dim); }
  .pip.easy { background: #34c759; }
  .pip.medium { background: #ff9f0a; }
  .pip.hard { background: #ff453a; }

  .k-due {
    flex-shrink: 0;
    font-size: 0.66rem;
    color: var(--dim);
    border: 1px solid var(--edge);
    border-radius: 999px;
    padding: 0 0.4rem;
  }

  .k-due.overdue {
    color: #ff453a;
    border-color: #ff453a;
  }

  .k-sub {
    flex-shrink: 0;
    font-size: 0.66rem;
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }

  .k-proj {
    flex-shrink: 0;
    max-width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.68rem;
    color: var(--dim);
  }

  .k-chip {
    flex-shrink: 0;
    font-size: 0.66rem;
    font-weight: 700;
    color: var(--accent);
  }

  .k-chip.cap {
    color: var(--dim);
    font-weight: 400;
  }

  /* Happened chips — All / I did / On my behalf. */
  .actor-chips {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }

  .actor-chips button {
    font: inherit;
    font-size: 0.62rem;
    line-height: 1.6;
    color: var(--dim);
    background: transparent;
    border: 1px solid var(--edge);
    border-radius: 999px;
    padding: 0 8px;
    cursor: pointer;
    white-space: nowrap;
  }

  .actor-chips button:hover {
    color: var(--ink);
    border-color: var(--focus, #6e4db0);
  }

  .actor-chips button.active {
    color: var(--accent);
    font-weight: 700;
    border-color: var(--focus, #6e4db0);
    background: var(--panel-fill, var(--panel));
  }

  .k-empty {
    margin: 6px 2px;
    padding: 12px 14px;
    font-size: 0.78rem;
    line-height: 1.5;
    color: var(--dim);
    border: 1px dashed var(--edge);
    border-radius: 8px;
  }

  .k-donehead {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 12px 2px 4px;
    padding: 4px 6px;
    font: inherit;
    font-size: 0.72rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 6px;
    cursor: pointer;
  }

  .k-donehead:hover {
    color: var(--accent);
  }

  .chev {
    font-size: 0.6rem;
    transition: transform 0.12s;
  }

  .chev.open {
    transform: rotate(90deg);
  }

  /* ---- S4.4 Board. Column counts live in boardStack.css. ---- */
  .b-cols {
    flex: 1;
    min-height: 0;
    display: grid;
    gap: 10px;
  }

  .b-col {
    min-height: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--edge);
    border-radius: 10px;
    background: var(--panel-fill, var(--panel));
    transition:
      border-color 0.12s,
      box-shadow 0.12s;
  }

  .b-col.drop-ok {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }

  .b-head {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    padding: 8px 10px 6px;
  }

  .b-name {
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--ink);
  }

  .b-col.drop-ok .b-name {
    color: var(--accent);
  }

  .b-count {
    font-size: 0.62rem;
    color: var(--dim);
    border: 1px solid var(--edge);
    border-radius: 999px;
    padding: 0 0.35rem;
  }

  .b-cards {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 4px 8px 8px;
  }

  .b-card {
    padding: 8px 10px;
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 8px;
    cursor: pointer;
    touch-action: none;
    -webkit-user-select: none;
    user-select: none;
  }

  .b-card:hover {
    border-color: var(--focus, #6e4db0);
  }

  .b-card.dragging {
    opacity: 0.45;
  }

  /* The held card — accent-lifted, painted at
     the grab cursor until Enter drops / Esc lets go. The column under
     the cursor reuses the drag's drop-ok paint. */
  .b-card.held {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
    opacity: 0.85;
    cursor: grabbing;
  }

  .b-card.done {
    opacity: 0.75;
  }

  .b-card-top {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .b-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.8rem;
    color: var(--ink);
  }

  .b-title.strike {
    color: var(--dim);
    text-decoration: line-through;
  }

  .b-notes {
    margin: 4px 0 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.68rem;
    color: var(--dim);
  }

  .b-meta {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 6px;
  }

  .b-meta:empty {
    display: none;
  }

  .b-empty {
    margin: 2px;
    padding: 10px;
    font-size: 0.68rem;
    line-height: 1.5;
    color: var(--dim);
    border: 1px dashed var(--edge);
    border-radius: 8px;
  }

  /* Preview tile — same chrome as the mail stack, not a flush wall. */
  .k-detail {
    flex-shrink: 0;
    width: 280px;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 10px;
  }

  .d-head {
    display: flex;
    align-items: center;
    margin-bottom: 2px;
  }

  .d-title {
    font-size: 0.72rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--dim);
  }

  .d-close {
    margin-left: auto;
    font: inherit;
    font-size: 0.9rem;
    line-height: 1;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 6px;
    padding: 2px 6px;
    cursor: pointer;
  }

  .d-close:hover {
    color: var(--accent);
  }

  .d-label {
    margin-top: 6px;
    font-size: 0.6rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--dim);
  }

  .d-input {
    width: 100%;
    box-sizing: border-box;
    font: inherit;
    font-size: 0.8rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 6px 8px;
  }

  .d-input:focus {
    outline: none;
    border-color: var(--focus, #6e4db0);
  }

  .d-textarea {
    resize: vertical;
    min-height: 64px;
    line-height: 1.45;
  }

  .d-diff,
  .d-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .d-duerow {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .d-subs {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .d-sub {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .d-sub .k-check {
    width: 16px;
    height: 16px;
    font-size: 0.6rem;
  }

  .d-subtitle {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.78rem;
    color: var(--ink);
  }

  .d-subtitle.strike {
    color: var(--dim);
    text-decoration: line-through;
  }

  .d-del {
    flex-shrink: 0;
    font: inherit;
    font-size: 0.78rem;
    line-height: 1;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 4px;
    padding: 1px 5px;
    cursor: pointer;
  }

  .d-del:hover {
    color: #ff453a;
  }

  .d-subadd {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .d-hint {
    margin: 0;
    font-size: 0.66rem;
    line-height: 1.5;
    color: var(--dim);
  }

  .d-win {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .d-winname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.74rem;
    color: var(--ink);
  }

  .mini {
    font: inherit;
    font-size: 0.64rem;
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
</style>
