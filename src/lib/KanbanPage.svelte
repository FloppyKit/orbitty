<script lang="ts">
  // S4.4 Lists window — two views of the same cards. List: Wunderlist-ish
  // sidebar + airy rows showing notes/difficulty/due/sub-count. Board:
  // Hive-like Todo · Doing · Done columns with drag-between-columns status
  // moves. Clicking a row/card (not the checkbox) opens a Habitica-thin
  // detail drawer: title, notes, difficulty (metadata only — the points
  // formula is untouched), sub-checklist, due date, tags, read-only linked
  // window. Same state.json discipline as before: every mutation is
  // load → mutate → save so rail-owned slices are never clobbered, and the
  // rail adopts card/ledger writes via the backend "state:changed" event.
  // GO-ORBITTY-LP-ACTOR: `actor` on cards + ledger rows records who
  // completed a quest. completeCard stamps "user"; Happened splits via the
  // All / I did / On my behalf chips (missing actor = pre-actor human
  // history = I did). Needed never enters the complete door.
  // GO-ORBITTY-LP-INBOX: Launchpad first paint is YOUR work — rows are
  // kind "todo" (and the Needed strip) only. Kind "window" never paints
  // here: no "On this planet" split, no window-first sort, sidebar planet
  // rows filter your todos and never load the window map. The Planets
  // sheet owns windows later. Every todo row wears a planet color dot
  // when projectId is a live project, the List|Board toggle sticks
  // across sessions (default list), and add-a-todo still mints kind
  // "todo". The rail claim door is untouched.
  import { invoke } from "@tauri-apps/api/core";
  import { emitTo, listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount, onDestroy } from "svelte";
  import { applyHostTokens, applyWindowOpacity, clampWindowOpacity } from "../themes/apply";
  import { listenPaneVis } from "./paneVis";
  import { planetCss } from "./planetColor";
  import NeededInbox from "./NeededInbox.svelte";
  import type {
    Actor,
    Claim,
    Difficulty,
    KanbanCard,
    NeededState,
    PersistedState,
    PointsEntry,
    Project,
    Tag,
    TitleOverride,
    WindowInfo,
  } from "./persist";

  // GO-ORBITTY-HOST-SPLIT: embedded mounts this page in the companion's
  // Launchpad well — × calls onClose (never hide_window), the banner
  // wordmark is Launchpad, and a pop-out control (onPopout) hands the
  // board back to the real kanban window. The overlay passes nothing.
  let {
    embedded = false,
    onClose,
    onPopout,
  }: {
    embedded?: boolean;
    onClose?: () => void;
    onPopout?: () => void;
  } = $props();

  type Status = "todo" | "doing" | "done";
  // Happened chip filter: All default; mine = user|missing; behalf =
  // agent|action|vps. Session-only, like the other view state.
  type DoneFilter = "all" | "mine" | "behalf";

  const DAILY_CAP = 12;
  const DIFFICULTIES: Difficulty[] = ["trivial", "easy", "medium", "hard"];
  const COLUMNS: { id: Status; name: string; hint: string }[] = [
    { id: "todo", name: "Todo", hint: "No to-dos — drag a card here." },
    { id: "doing", name: "Doing", hint: "Nothing in progress — drag a card here." },
    { id: "done", name: "Done", hint: "Nothing done yet — check a to-do." },
  ];

  // GO-ORBITTY-PANE-PAINT: seed the OS theme synchronously so --bg exists
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

  // GO-ORBITTY-ACTION-FILE: no Actions well here — actions are a Settings
  // list + `orbittyctl action` verbs; the slice still round-trips via
  // load → mutate → save.
  // GO-ORBITTY-LP-NEEDED: shared so strip + and sidebar form stay one toggle.
  let addingNeeded = $state(false);

  // Sidebar selection: a smart filter, Inbox, or a project id. Session-only.
  let selected = $state("inbox");
  let newTodo = $state("");
  let showCompleted = $state(true);
  // PLANETS-STRIP: rail-owned marble flag, read-only here — tab double-click
  // emits to main; the rail persist is the writer.
  let collapsedSectionIds = $state<string[]>([]);

  // S4.4: List | Board toggle + detail drawer. LP-INBOX: the toggle is
  // last-used across sessions (default list); the rest stays session-only.
  const VIEW_KEY = "orbitty.launchpad.view";
  let view = $state<"list" | "board">(
    localStorage.getItem(VIEW_KEY) === "board" ? "board" : "list",
  );
  function setView(v: "list" | "board") {
    view = v;
    localStorage.setItem(VIEW_KEY, v);
  }
  let detailId = $state<string | null>(null);
  let newSubtask = $state("");

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
    selected = id;
    raisePlanet(id);
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

  // GO-ORBITTY-LP-ACTOR: Happened chips (sidebar Done + Board Done header).
  let doneFilter = $state<DoneFilter>("all");

  function actorSide(card: KanbanCard): "mine" | "behalf" {
    return card.actor && card.actor !== "user" ? "behalf" : "mine";
  }

  function inDoneFilter(c: KanbanCard): boolean {
    return doneFilter === "all" || actorSide(c) === doneFilter;
  }

  // Open rows: todos only, oldest first.
  let visibleOpen = $derived.by(() => {
    if (selected === "done") return [];
    const rows =
      selected === "today"
        ? openCards.filter((c) => c.createdAt >= startOfToday())
        : openCards.filter((c) => listOf(c) === selected);
    return [...rows].sort((a, b) => a.createdAt - b.createdAt);
  });

  let visibleDone = $derived.by(() => {
    if (selected === "today") return [];
    if (selected === "done") return doneCards.filter(inDoneFilter);
    return doneCards.filter((c) => listOf(c) === selected);
  });

  // LP-INBOX: planet color dot — only when projectId is a LIVE project
  // (a vanished project falls back to Inbox and gets no dot).
  function livePlanetId(card: KanbanCard): string | null {
    return projects.some((p) => p.id === card.projectId) ? card.projectId : null;
  }

  // Board scope mirrors the sidebar selection; the Done smart filter boards
  // every card so all three columns stay meaningful.
  function inScope(c: KanbanCard): boolean {
    if (c.kind === "needed" || c.kind === "window") return false;
    if (selected === "done") return true;
    if (selected === "today") return c.createdAt >= startOfToday();
    return listOf(c) === selected;
  }

  let boardCols = $derived.by(() => {
    const scoped = kanbanCards.filter(inScope);
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

  let detailCard = $derived(
    detailId ? kanbanCards.find((c) => c.id === detailId) : undefined,
  );

  function sideCount(list: string): number {
    if (list === "today")
      return openCards.filter((c) => c.createdAt >= startOfToday()).length;
    if (list === "done") return doneCards.length;
    return openCards.filter((c) => listOf(c) === list).length;
  }

  let canAdd = $derived(selected !== "today" && selected !== "done");
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

  // Project chip: shown whenever it adds information the sidebar doesn't.
  function showProj(card: KanbanCard): boolean {
    return (
      projectName(card.projectId) !== "" &&
      (selected === "today" || selected === "done" || selected !== card.projectId)
    );
  }

  // GO-ORBITTY-COMPANION-BRAND: the seven palette skins are retired; a
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

  // GO-ORBITTY-EVENT-LOG: append-only local event log (kind 31200). Fire-
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
      // next state:changed / visibility refresh retries
    }
  }

  // Complete — the only door that retires a sprite and can mint points.
  // Award once per card, 1 pt (2 clinical/master), daily cap honored.
  // GO-ORBITTY-CADENCE: a window card KEEPS its claim here so the walker
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

  function addTodo() {
    const title = newTodo.trim();
    if (!title || !canAdd) return;
    const projectId = selected === "inbox" ? "" : selected;
    const now = Date.now() / 1000;
    mutate((s) => {
      s.kanbanCards = [
        ...s.kanbanCards,
        {
          id: `c-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
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
    });
    newTodo = "";
  }

  // ---- detail drawer mutations (all additive on the card) ----

  function openDetail(card: KanbanCard) {
    detailId = card.id;
    newSubtask = "";
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

  // Difficulty is metadata only — it never enters the points formula.
  function setDifficulty(card: KanbanCard, d: Difficulty | null) {
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id ? { ...c, difficulty: d ?? undefined } : c,
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

  function addSubtask(card: KanbanCard) {
    const title = newSubtask.trim();
    if (!title) return;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? {
              ...c,
              subtasks: [
                ...(c.subtasks ?? []),
                {
                  id: `s-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
                  title,
                  done: false,
                },
              ],
            }
          : c,
      );
    });
    newSubtask = "";
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

  function onCardDragStart(e: DragEvent, card: KanbanCard) {
    dragCardId = card.id;
    e.dataTransfer?.setData("text/plain", card.id);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
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
    newSubtask = "";
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
    unlisten?.();
    unlistenFocus?.();
    unlistenVis?.();
    document.removeEventListener("visibilitychange", onVisibility);
  });

  // One drag source: the in-SPA banner. Native Overlay title strip is
  // off (drawers are borderless). Buttons stop the press so × / tabs
  // still click.
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

{#snippet metaChips(card: KanbanCard)}
  {@const due = dueInfo(card)}
  {@const sub = subProgress(card)}
  {#if card.difficulty}
    <span
      class="pip {card.difficulty}"
      title="Difficulty: {card.difficulty} (metadata only — points unchanged)"
      aria-label="Difficulty: {card.difficulty}"
    ></span>
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
{/snippet}

{#snippet doneActorChips()}
  <!-- GO-ORBITTY-LP-ACTOR: Happened split — All default, session-only. -->
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

<!-- GO-ORBITTY-LP-INBOX: one row shape, todos only. Every row wears a
     planet color dot when projectId is a live project. -->
{#snippet openRow(card: KanbanCard)}
  {@const pid = livePlanetId(card)}
  <li class="k-row">
    <button
      class="k-check"
      onclick={() => toggleCard(card)}
      title="Mark done"
      aria-label="Mark done"
    ></button>
    {#if pid}
      <span
        class="planet-chip"
        style="background: {planetCss(pid)}"
        title={projectName(pid)}
        aria-hidden="true"
      ></span>
    {/if}
    <button
      class="k-name"
      onclick={() => openDetail(card)}
      title="{displayTitle(card)} — click for details"
    >
      <span class="k-title-text">{displayTitle(card)}</span>
      {#if card.notes}
        <span class="k-notes">{notesSnippet(card)}</span>
      {/if}
    </button>
    {@render metaChips(card)}
  </li>
{/snippet}

{#snippet doneRow(card: KanbanCard)}
  {@const chip = doneChip(card)}
  {@const pid = livePlanetId(card)}
  <li class="k-row done">
    <button
      class="k-check checked"
      onclick={() => toggleCard(card)}
      title="Reopen as a to-do"
      aria-label="Reopen as a to-do">✓</button
    >
    {#if pid}
      <span
        class="planet-chip"
        style="background: {planetCss(pid)}"
        title={projectName(pid)}
        aria-hidden="true"
      ></span>
    {/if}
    <button
      class="k-name strike"
      onclick={() => openDetail(card)}
      title="{displayTitle(card)} — click for details">{displayTitle(card)}</button
    >
    {@render metaChips(card)}
    {#if chip}
      <span class="k-chip" class:cap={chip === "cap"}>{chip}</span>
    {/if}
  </li>
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
      <h1><span class="mark"><span class="o">O</span><span class="rest">rb</span><span class="idot">i</span><span class="pop">ty</span></span> Lists</h1>
    {/if}
    <div class="view-toggle" role="tablist" aria-label="View">
      <button
        role="tab"
        aria-selected={view === "list"}
        class:active={view === "list"}
        onclick={() => setView("list")}>List</button
      >
      <button
        role="tab"
        aria-selected={view === "board"}
        class:active={view === "board"}
        onclick={() => setView("board")}>Board</button
      >
    </div>
    <span class="k-pts" title="Points today / daily cap">{pointsToday()}/{DAILY_CAP}</span>
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

  <NeededInbox
    variant="strip"
    cards={kanbanCards}
    projectId={selectedProject?.id ?? ""}
    {mutate}
    {logEvent}
    bind:addingNeeded
  />

  <div class="planet-strip" role="tablist" aria-label="Lists and planets">
    <button
      type="button"
      class="planet-tab"
      class:sel={selected === "inbox"}
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
      role="tab"
      aria-selected={selected === "done"}
      onclick={() => (selected = "done")}
    >
      <span class="planet-tab-name">Done</span>
      <span class="side-count">{sideCount("done")}</span>
    </button>
    <span class="strip-gap" aria-hidden="true"></span>
    {#each projects as p (p.id)}
      <button
        type="button"
        class="planet-tab"
        class:sel={selected === p.id}
        role="tab"
        aria-selected={selected === p.id}
        title={p.name}
        onclick={() => onPlanetTabClick(p.id)}
        ondblclick={() => onPlanetTabDblClick(p.id)}
      >
        <span
          class="planet-chip"
          style="background: {planetCss(p.id)}"
          aria-hidden="true"
        ></span>
        <span class="planet-tab-name">{p.name}</span>
        <span class="side-count">{sideCount(p.id)}</span>
      </button>
    {/each}
  </div>

  <div class="k-body">
    <section class="k-main" aria-label={listTitle}>
      <div class="k-titlerow">
        {#if selectedProject}
          <span
            class="planet-chip"
            style="background: {planetCss(selectedProject.id)}"
            title={selectedProject.name}
            aria-hidden="true"
          ></span>
        {/if}
        <h2 class="k-title">{listTitle}</h2>
        {#if selected === "done"}
          {@render doneActorChips()}
        {/if}
        {#if selectedProject}
          <button
            class="mini"
            onclick={launchHermes}
            title="Hermes — focus or launch the existing harness with this list's titles-only context"
          >Hermes</button>
        {/if}
      </div>
      {#if hermesNote}
        <div class="k-hermesnote" class:err={hermesNote.kind === "err"}>
          <span class="k-hermestext">{hermesNote.text}</span>
          <button class="mini ghost" onclick={() => (hermesNote = null)} aria-label="Dismiss">×</button>
        </div>
      {/if}

      {#if view === "board"}
        <div class="b-cols">
          {#each COLUMNS as col (col.id)}
            <section
              class="b-col"
              class:drop-ok={dropCol === col.id}
              aria-label={col.name}
              ondragover={(e) => onColDragOver(e, col.id)}
              ondragleave={(e) => onColDragLeave(e, col.id)}
              ondrop={(e) => onColDrop(e, col.id)}
            >
              <header class="b-head">
                <span class="b-name">{col.name}</span>
                <span class="b-count">{boardCols[col.id].length}</span>
                {#if col.id === "done"}
                  {@render doneActorChips()}
                {/if}
              </header>
              <div class="b-cards">
                {#each boardCols[col.id] as card (card.id)}
                  {@const pid = livePlanetId(card)}
                  <article
                    class="b-card"
                    class:dragging={dragCardId === card.id}
                    class:done={card.status === "done"}
                    draggable="true"
                    ondragstart={(e) => onCardDragStart(e, card)}
                    ondragend={onCardDragEnd}
                    onclick={() => openDetail(card)}
                    onkeydown={(e) => e.key === "Enter" && openDetail(card)}
                    tabindex="0"
                    role="button"
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
                    </div>
                    {#if card.notes}
                      <p class="b-notes">{notesSnippet(card)}</p>
                    {/if}
                    <div class="b-meta">
                      {@render metaChips(card)}
                    </div>
                  </article>
                {:else}
                  <p class="b-empty">{col.hint}</p>
                {/each}
              </div>
            </section>
          {/each}
        </div>
      {:else}
        <!-- GO-ORBITTY-LP-INBOX: one flat list of your todos — no
             "On this planet" / window-first split. -->
        <ul class="k-rows">
          {#each visibleOpen as card (card.id)}
            {@render openRow(card)}
          {:else}
            <li class="k-empty">
              {selected === "done"
                ? "Nothing done yet — check a to-do."
                : selected === "today"
                  ? "Nothing new today."
                  : "All clear. Add a to-do below."}
            </li>
          {/each}
        </ul>

        {#if visibleDone.length > 0}
          <button
            class="k-donehead"
            onclick={() => (showCompleted = !showCompleted)}
            aria-expanded={showCompleted}
          >
            <span class="chev" class:open={showCompleted} aria-hidden="true">▸</span>
            Completed ({visibleDone.length})
          </button>
          {#if showCompleted}
            <ul class="k-rows done">
              {#each visibleDone as card (card.id)}
                {@render doneRow(card)}
              {/each}
            </ul>
          {/if}
        {/if}
      {/if}

      <div class="k-add">
        <input
          bind:value={newTodo}
          placeholder={canAdd ? "Add a to-do…" : "Open a list to add to-dos"}
          disabled={!canAdd}
          onkeydown={(e) => e.key === "Enter" && addTodo()}
        />
      </div>
    </section>

    {#if detailCard}
      {@const card = detailCard}
      <aside class="k-detail" aria-label="Item details">
        <header class="d-head">
          <span class="d-title">Details</span>
          <button class="d-close" onclick={() => (detailId = null)} aria-label="Close details">×</button>
        </header>

        <label class="d-label" for="d-name">Title</label>
        <input
          id="d-name"
          class="d-input"
          value={card.title}
          onchange={(e) => saveTitle(card, e.currentTarget.value)}
          required
        />

        <label class="d-label" for="d-notes">Notes</label>
        <textarea
          id="d-notes"
          class="d-input d-textarea"
          rows="4"
          placeholder="Add notes…"
          value={card.notes ?? ""}
          onchange={(e) => saveNotes(card, e.currentTarget.value)}
        ></textarea>

        <span class="d-label">Difficulty</span>
        <div class="d-diff" title="Metadata only — the points formula never reads difficulty">
          {#each DIFFICULTIES as d (d)}
            <button
              class="mini"
              class:active={card.difficulty === d}
              onclick={() => setDifficulty(card, card.difficulty === d ? null : d)}
            >
              {d}
            </button>
          {/each}
        </div>

        <label class="d-label" for="d-due">Due date</label>
        <div class="d-duerow">
          <input
            id="d-due"
            class="d-input"
            type="date"
            value={dateInputValue(card)}
            onchange={(e) => setDue(card, e.currentTarget.value)}
          />
          {#if card.dueAt}
            <button class="mini ghost" onclick={() => setDue(card, "")}>Clear</button>
          {/if}
        </div>

        <span class="d-label">Checklist</span>
        <ul class="d-subs">
          {#each card.subtasks ?? [] as sub (sub.id)}
            <li class="d-sub">
              <button
                class="k-check"
                class:checked={sub.done}
                onclick={() => toggleSubtask(card, sub.id)}
                aria-label={sub.done ? "Reopen subtask" : "Complete subtask"}
                title={sub.done ? "Reopen" : "Complete"}>{sub.done ? "✓" : ""}</button
              >
              <span class="d-subtitle" class:strike={sub.done}>{sub.title}</span>
              <button
                class="d-del"
                onclick={() => deleteSubtask(card, sub.id)}
                aria-label="Delete subtask"
                title="Delete">×</button
              >
            </li>
          {:else}
            <li class="d-hint">No subtasks yet.</li>
          {/each}
        </ul>
        <div class="d-subadd">
          <input
            class="d-input"
            bind:value={newSubtask}
            placeholder="Add a subtask…"
            onkeydown={(e) => e.key === "Enter" && addSubtask(card)}
          />
          <button class="mini" onclick={() => addSubtask(card)}>Add</button>
        </div>

        <span class="d-label">Tags</span>
        {#if tags.length > 0}
          <div class="d-tags">
            {#each tags as t (t.id)}
              <button
                class="mini"
                class:active={cardHasTag(card, t.id)}
                aria-pressed={cardHasTag(card, t.id)}
                onclick={() => toggleCardTag(card, t.id)}>{t.name}</button
              >
            {/each}
          </div>
        {:else}
          <p class="d-hint">No tags yet — make them in the rail attach menu.</p>
        {/if}

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
              <button class="mini" onclick={() => focusCard(card)}>Focus</button>
            {/if}
          </div>
          <p class="d-hint">Window links are made by claiming in the rail — read-only here.</p>
        {/if}
      </aside>
    {/if}
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

  /* GO-ORBITTY-HOST-SPLIT: embedded in the companion's Launchpad well —
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

  /* The only close affordance: hides Lists, never quits (tray → Quit). */
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

  .k-banner h1 {
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

  /* S4.4 List | Board segmented toggle. */
  .view-toggle {
    display: inline-flex;
    margin-left: 10px;
    border: 1px solid var(--edge);
    border-radius: 7px;
    overflow: hidden;
  }

  .view-toggle button {
    font: inherit;
    font-size: 0.68rem;
    color: var(--dim);
    background: transparent;
    border: none;
    padding: 3px 10px;
    cursor: pointer;
  }

  .view-toggle button + button {
    border-left: 1px solid var(--edge);
  }

  .view-toggle button.active {
    color: var(--accent);
    font-weight: 700;
    background: var(--panel-fill, var(--panel));
  }

  .k-pts {
    margin-left: auto;
    font-size: 0.66rem;
    font-weight: 700;
    color: var(--accent);
    border: 1px solid var(--edge);
    border-radius: 999px;
    padding: 1px 0.5rem;
  }

  .k-body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  /* GO-ORBITTY-PLANETS-STRIP: planet orbs across the top of Lists. */
  .planet-strip {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    overflow-x: auto;
    padding: 4px 12px 8px;
    scrollbar-width: thin;
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

  /* GO-ORBITTY-PLANET-MARK: the list wears the disc's marble color. */
  .planet-chip {
    flex-shrink: 0;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
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
    padding: 12px 16px 12px;
    overflow-y: auto;
  }

  .k-title {
    margin: 2px 2px 10px;
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

  /* GO-ORBITTY-LP-ACTOR: Happened chips — All / I did / On my behalf. */
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

  .k-add {
    margin-top: auto;
    padding-top: 12px;
  }

  .k-add input {
    width: 100%;
    box-sizing: border-box;
    font: inherit;
    font-size: 0.88rem;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 9px;
    padding: 10px 12px;
  }

  .k-add input:focus {
    outline: none;
    border-color: var(--focus, #6e4db0);
  }

  .k-add input:disabled {
    opacity: 0.6;
  }

  /* ---- S4.4 Board (Hive-like Todo · Doing · Done) ---- */
  .b-cols {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
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
    gap: 6px;
    padding: 8px 10px 6px;
  }

  .b-name {
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--dim);
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
    cursor: grab;
  }

  .b-card:hover {
    border-color: var(--focus, #6e4db0);
  }

  .b-card.dragging {
    opacity: 0.45;
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

  /* ---- S4.4 detail drawer (Habitica-thin) ---- */
  .k-detail {
    flex-shrink: 0;
    width: 272px;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    background: var(--panel-fill, var(--panel));
    border-left: 1px solid var(--edge);
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
