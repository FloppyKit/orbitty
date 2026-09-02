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
  // GO-ORBITTY-LP-MINE: a mixed filtered set splits into two quiet
  // sections — "On this planet" (kind "window"; heading "Inbox" when the
  // Inbox is selected) then "My list" (kind "todo" / loose). One kind
  // only → no headings. Board keeps three columns but parks window cards
  // above todos inside each. No new column, no shame counts; add-a-todo
  // still mints kind "todo" and the rail claim door is untouched.
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount, onDestroy } from "svelte";
  import { applyHostTokens, applyWindowOpacity, clampWindowOpacity } from "../themes/apply";
  import { listenPaneVis } from "./paneVis";

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

  interface WindowInfo {
    id: string;
    pid: number;
    appName: string;
    title: string;
    bundleId: string | null;
    stableKey: string;
    iconBase64: string | null;
    isFrontmost: boolean;
    isMinimized: boolean;
  }

  interface Project {
    id: string;
    name: string;
  }

  interface Claim {
    windowId: string;
    projectId: string;
    lastTouchedAt: number;
    // GO-LS-TAURI-USER-PACKS reboot-rematch fingerprint (rail-owned; the
    // rail rewrites windowId on rematch and Lists adopts it from disk).
    bundleId?: string | null;
    appName?: string;
    titleAtClaim?: string;
    titleIndex?: number;
    walkerHome?: string | null;
  }

  interface Subtask {
    id: string;
    title: string;
    done: boolean;
  }

  type Difficulty = "trivial" | "easy" | "medium" | "hard";
  type Status = "todo" | "doing" | "done";
  // GO-ORBITTY-LP-NEEDED: Needed inbox states. `later` carries dueAt as
  // the snooze stamp; a past dueAt counts as open for the strip.
  type NeededState = "open" | "later" | "answered" | "skipped";
  // GO-ORBITTY-LP-ACTOR: who completed a quest. Missing on pre-actor
  // cards/rows — the HUD reads missing as "user" (history was human).
  type Actor = "user" | "agent" | "action" | "vps";
  // Happened chip filter: All default; mine = user|missing; behalf =
  // agent|action|vps. Session-only, like the other view state.
  type DoneFilter = "all" | "mine" | "behalf";

  interface KanbanCard {
    id: string;
    title: string;
    projectId: string;
    windowId?: string | null;
    kind?: "window" | "todo" | "needed";
    // Dummy "todo" on Needed cards so kind-blind code never crashes —
    // every quest view filters kind first.
    status: Status;
    createdAt: number;
    lastTouchedAt: number;
    completedAt?: number | null;
    goalAligned: boolean;
    clinicalOrMaster: boolean;
    pointsAwarded: number;
    starred?: boolean;
    // S4.4 detail fields (additive; difficulty never touches points).
    notes?: string;
    difficulty?: Difficulty;
    subtasks?: Subtask[];
    dueAt?: number | null;
    tagIds?: string[];
    // GO-ORBITTY-LP-NEEDED (additive): the super-quick user-only inbox.
    neededState?: NeededState;
    neededWhy?: string;
    neededAnswer?: string;
    // GO-ORBITTY-LP-ACTOR (additive): stamped by completeCard only.
    actor?: Actor;
  }

  interface PointsEntry {
    id: string;
    cardId: string;
    amount: number;
    day: string;
    at: number;
    // GO-ORBITTY-LP-ACTOR (additive): who acted on the awarding complete.
    actor?: Actor;
  }

  interface Tag {
    id: string;
    name: string;
    color?: string | null;
  }

  interface WindowTags {
    windowId: string;
    tagIds: string[];
  }

  // Rail-owned display renames — read-only here, round-tripped untouched.
  interface TitleOverride {
    windowId: string;
    title: string;
  }

  // GO-ORBITTY-BOARD-ACTIONS: rail-owned named macros (state.rs
  // ActionShortcut, camelCase). The well below is a second writer — same
  // shape, same id format, same run_action invoke as the rail.
  interface ActionShortcut {
    id: string;
    name: string;
    command: string;
    cwd?: string | null;
  }

  // Full shape — this window saves state.json, so rail-owned fields must
  // round-trip untouched.
  interface PersistedState {
    themeId: string;
    windowOpacity: number;
    projects: Project[];
    claims: Claim[];
    tags: Tag[];
    windowTags: WindowTags[];
    collapsedSectionIds: string[];
    nudgeSuppressedWindowIds: string[];
    kanbanCards: KanbanCard[];
    pointsLedger: PointsEntry[];
    titleOverrides: TitleOverride[];
    // GO-ORBITTY-BOARD-ACTIONS: declared so mutate() never serializes a
    // state without the key and wipes the rail's macro array.
    actionShortcuts?: ActionShortcut[];
    // GO-LS-TAURI-AVATAR-LARGE: companion-window slice — owned by the
    // companion window / native side; round-tripped untouched by mutate().
    companion?: {
      visible: boolean;
      bounds: { x: number; y: number; w: number; h: number } | null;
      imagePackId: string | null;
      state?: string;
    };
    // GO-LS-STARS-STATUS: rail-owned check-in slice — round-tripped
    // untouched by mutate() like the companion slice.
    stars?: {
      promptsOptIn: boolean;
      checkins: Record<
        string,
        {
          sleep: number;
          terroir: number;
          athleticism: number;
          relationships: number;
          self: number;
        }
      >;
      promptsAck?: string;
    };
  }

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

  // GO-ORBITTY-BOARD-ACTIONS: macros well state (mirrors the rail's).
  let actionShortcuts = $state<ActionShortcut[]>([]);
  let actionNote = $state<{ kind: "ok" | "err"; text: string } | null>(null);
  let addingAction = $state(false);
  let newActionName = $state("");
  let newActionCommand = $state("");
  let newActionCwd = $state("");
  let runningActionId = $state<string | null>(null);

  // Sidebar selection: a smart filter, Inbox, or a project id. Session-only.
  let selected = $state("inbox");
  let newTodo = $state("");
  let showCompleted = $state(true);

  // S4.4: List | Board toggle + detail drawer. Both session-only.
  let view = $state<"list" | "board">("list");
  let detailId = $state<string | null>(null);
  let newSubtask = $state("");

  // Board drag: card id in flight + column under the cursor.
  let dragCardId = $state<string | null>(null);
  let dropCol = $state<Status | null>(null);

  // S5: Hermes launch/focus for the selected project list (titles only).
  let hermesNote = $state<{ kind: "ok" | "err"; text: string } | null>(null);
  let selectedProject = $derived(projects.find((p) => p.id === selected));

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
  let openCards = $derived(
    kanbanCards.filter((c) => c.kind !== "needed" && c.status !== "done"),
  );
  let doneCards = $derived(
    kanbanCards
      .filter((c) => c.kind !== "needed" && c.status === "done" && c.completedAt)
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

  // Open rows: window-linked (doing) first, then todos, oldest first.
  let visibleOpen = $derived.by(() => {
    if (selected === "done") return [];
    const rows =
      selected === "today"
        ? openCards.filter((c) => c.createdAt >= startOfToday())
        : openCards.filter((c) => listOf(c) === selected);
    return [...rows].sort(
      (a, b) =>
        (a.windowId ? 0 : 1) - (b.windowId ? 0 : 1) || a.createdAt - b.createdAt,
    );
  });

  let visibleDone = $derived.by(() => {
    if (selected === "today") return [];
    if (selected === "done") return doneCards.filter(inDoneFilter);
    return doneCards.filter((c) => listOf(c) === selected);
  });

  // GO-ORBITTY-LP-MINE: kind split for the quiet sections. A missing kind
  // reads as mine — pre-kind cards were all user todos. Headings appear
  // only when BOTH kinds are present; a lone kind renders flat.
  let openWindows = $derived(visibleOpen.filter((c) => c.kind === "window"));
  let openTodos = $derived(visibleOpen.filter((c) => c.kind !== "window"));
  let openSplit = $derived(openWindows.length > 0 && openTodos.length > 0);
  let doneWindows = $derived(visibleDone.filter((c) => c.kind === "window"));
  let doneTodos = $derived(visibleDone.filter((c) => c.kind !== "window"));
  let doneSplit = $derived(doneWindows.length > 0 && doneTodos.length > 0);
  let windowHeading = $derived(
    selected === "inbox" ? "Inbox" : "On this planet",
  );

  // Board scope mirrors the sidebar selection; the Done smart filter boards
  // every card so all three columns stay meaningful.
  function inScope(c: KanbanCard): boolean {
    if (c.kind === "needed") return false;
    if (selected === "done") return true;
    if (selected === "today") return c.createdAt >= startOfToday();
    return listOf(c) === selected;
  }

  let boardCols = $derived.by(() => {
    const scoped = kanbanCards.filter(inScope);
    const byAge = (a: KanbanCard, b: KanbanCard) => a.createdAt - b.createdAt;
    // GO-ORBITTY-LP-MINE: ongoings above mine inside every column — same
    // three columns, no new chrome.
    const kindFirst = (a: KanbanCard, b: KanbanCard) =>
      (a.kind === "window" ? 0 : 1) - (b.kind === "window" ? 0 : 1);
    return {
      todo: scoped
        .filter((c) => c.status === "todo")
        .sort((a, b) => kindFirst(a, b) || byAge(a, b)),
      doing: scoped
        .filter((c) => c.status === "doing")
        .sort((a, b) => kindFirst(a, b) || byAge(a, b)),
      done: scoped
        .filter((c) => c.status === "done")
        .filter(inDoneFilter)
        .sort(
          (a, b) => kindFirst(a, b) || (b.completedAt ?? 0) - (a.completedAt ?? 0),
        ),
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
      actionShortcuts = s.actionShortcuts ?? [];
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
      s.actionShortcuts ??= [];
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
  // same complete door as the checkbox. Dragging a window-claimed card out
  // of Doing unclaims its window — the board never lies about linkage.
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
              windowId: target === "doing" ? (c.windowId ?? null) : null,
            }
          : c,
      );
      if (card.status === "doing" && card.windowId) {
        s.claims = s.claims.filter((c) => c.windowId !== card.windowId);
      }
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

  // ---- GO-ORBITTY-LP-NEEDED: the Needed inbox (pile, not a lens) ----
  // Super-quick questions the user must answer. Never a fourth column,
  // never points, never completeCard. Open + later show in the strip;
  // a later whose dueAt has passed counts as open again.
  let addingNeeded = $state(false);
  let newNeeded = $state("");
  let answerFor = $state<string | null>(null);
  let answerDraft = $state("");

  let neededNow = $derived(
    kanbanCards
      .filter(
        (c) =>
          c.kind === "needed" &&
          (c.neededState === "open" || c.neededState === "later"),
      )
      .sort((a, b) => a.createdAt - b.createdAt),
  );

  function addNeeded() {
    const title = newNeeded.trim();
    if (!title) return;
    const projectId = selectedProject?.id ?? "";
    const now = Date.now() / 1000;
    const id = `n-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    mutate((s) => {
      s.kanbanCards = [
        ...s.kanbanCards,
        {
          id,
          title,
          projectId,
          windowId: null,
          kind: "needed" as const,
          neededState: "open" as const,
          status: "todo" as const, // dummy — views filter kind first
          createdAt: now,
          lastTouchedAt: now,
          completedAt: null,
          goalAligned: false,
          clinicalOrMaster: false,
          pointsAwarded: 0,
          starred: false,
        },
      ];
    });
    logEvent("needed-open", { cardId: id, title }, projectId);
    newNeeded = "";
    addingNeeded = false;
  }

  function answerNeeded(card: KanbanCard) {
    const answer = answerDraft.trim();
    if (!answer) return;
    const now = Date.now() / 1000;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? {
              ...c,
              neededAnswer: answer,
              neededState: "answered" as const,
              lastTouchedAt: now,
            }
          : c,
      );
    });
    // The answer text itself never enters the log — ids only.
    logEvent("needed-answer", { cardId: card.id }, card.projectId);
    answerFor = null;
    answerDraft = "";
  }

  function skipNeeded(card: KanbanCard) {
    const now = Date.now() / 1000;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? { ...c, neededState: "skipped" as const, lastTouchedAt: now }
          : c,
      );
    });
    logEvent("needed-skip", { cardId: card.id }, card.projectId);
    if (answerFor === card.id) {
      answerFor = null;
      answerDraft = "";
    }
  }

  // Snooze one local day; the strip treats a past dueAt as open again.
  function laterNeeded(card: KanbanCard) {
    const now = Date.now() / 1000;
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? {
              ...c,
              neededState: "later" as const,
              dueAt: now + 86400,
              lastTouchedAt: now,
            }
          : c,
      );
    });
    logEvent("needed-later", { cardId: card.id }, card.projectId);
    if (answerFor === card.id) {
      answerFor = null;
      answerDraft = "";
    }
  }

  // ---- GO-ORBITTY-BOARD-ACTIONS: macros well (same array as the rail) ----

  interface ActionResult {
    ok: boolean;
    code: number | null;
    stdout: string;
    stderr: string;
  }

  function addAction() {
    const name = newActionName.trim();
    const command = newActionCommand.trim();
    if (!name || !command) return;
    const cwd = newActionCwd.trim() || null;
    mutate((s) => {
      s.actionShortcuts = [
        ...(s.actionShortcuts ?? []),
        {
          id: `a-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
          name,
          command,
          cwd,
        },
      ];
    });
    newActionName = "";
    newActionCommand = "";
    newActionCwd = "";
    addingAction = false;
  }

  function deleteAction(id: string) {
    mutate((s) => {
      s.actionShortcuts = (s.actionShortcuts ?? []).filter((a) => a.id !== id);
    });
  }

  // Click runs the command via /bin/zsh -lc in cwd or $HOME; stdout/stderr
  // surface in a small note and never crash the page. Same copy as the rail.
  async function runAction(a: ActionShortcut) {
    if (runningActionId) return;
    runningActionId = a.id;
    actionNote = null;
    try {
      const r = await invoke<ActionResult>("run_action", {
        command: a.command,
        cwd: a.cwd ?? null,
      });
      const out = [r.stdout, r.stderr].filter(Boolean).join("\n");
      const head = r.ok ? `${a.name}: done` : `${a.name}: exit ${r.code ?? "?"}`;
      actionNote = {
        kind: r.ok ? "ok" : "err",
        text: out ? `${head}\n${out}` : head,
      };
    } catch (e) {
      actionNote = { kind: "err", text: `${a.name}: ${e}` };
    }
    runningActionId = null;
  }

  // Next chip: same smallest rule as the companion — first Doing card,
  // first undone subtask else the card title. Local focus only.
  let nextTodo = $derived.by(() => {
    const card = kanbanCards.find(
      (c) => c.kind !== "needed" && c.status === "doing",
    );
    if (!card) return null;
    const sub = card.subtasks?.find((s) => !s.done);
    return { cardId: card.id, text: sub?.title || card.title };
  });

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
        (c) => c.kind !== "needed" && c.status !== "done" && listOf(c) === p.id,
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
    invoke("ensure_glass").catch(() => {});
    await loadState();
    unlisten = await listen("state:changed", loadState);
    unlistenFocus = await listen<string>("orbitty:focus-card", (e) => {
      onFocusCard(e.payload);
    });
    await refreshWindows();
    // HOST-SPLIT embedded: the host companion window stays visible — no
    // AX poll timer and no kanban pane-vis gating in the well
    // (state:changed keeps the cards fresh).
    if (embedded) return;
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

<!-- GO-ORBITTY-LP-MINE: row snippets so the split sections and the flat
     list share one row shape. -->
{#snippet openRow(card: KanbanCard)}
  {@const w = winFor(card)}
  <li class="k-row" class:min={w?.isMinimized}>
    <button
      class="k-check"
      onclick={() => toggleCard(card)}
      title="Mark done"
      aria-label="Mark done"
    ></button>
    {#if card.windowId}
      <button
        class="k-iconbtn"
        onclick={() => focusCard(card)}
        title={w ? `Focus ${w.appName}` : "Window not open"}
        aria-label="Focus linked window"
      >
        {#if w?.iconBase64}
          <img class="k-icon" src="data:image/png;base64,{w.iconBase64}" alt="" />
        {:else}
          <span class="k-icon fallback" aria-hidden="true">▦</span>
        {/if}
      </button>
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
  <li class="k-row done">
    <button
      class="k-check checked"
      onclick={() => toggleCard(card)}
      title="Reopen as a to-do"
      aria-label="Reopen as a to-do">✓</button
    >
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
        onclick={() => (view = "list")}>List</button
      >
      <button
        role="tab"
        aria-selected={view === "board"}
        class:active={view === "board"}
        onclick={() => (view = "board")}>Board</button
      >
    </div>
    <span class="k-pts" title="Points today / daily cap">{pointsToday()}/{DAILY_CAP}</span>
  </header>

  <!-- GO-ORBITTY-LP-NEEDED: the Needed inbox strip — full width under the
       banner, above .k-body. Rendered only while an open/later Needed
       exists; empty means no chrome. Quiet pile, never a fourth column. -->
  {#if neededNow.length > 0}
    <section class="k-needed" aria-label="Needed from you">
      <div class="kn-head">
        <span class="kn-title">Needed from you</span>
        <button
          class="ka-plus"
          onclick={() => (addingNeeded = true)}
          title="Add a Needed question"
          aria-label="Add a Needed question">+</button
        >
      </div>
      <ul class="kn-rows">
        {#each neededNow as card (card.id)}
          <li class="kn-row">
            <div class="kn-main">
              <span class="kn-q">{card.title}</span>
              {#if card.neededWhy}
                <span class="kn-why">{card.neededWhy}</span>
              {/if}
              {#if answerFor === card.id}
                <div class="kn-answer">
                  <input
                    bind:value={answerDraft}
                    placeholder="Your answer…"
                    aria-label="Your answer"
                    onkeydown={(e) => {
                      if (e.key === "Enter") answerNeeded(card);
                      if (e.key === "Escape") {
                        answerFor = null;
                        answerDraft = "";
                      }
                    }}
                  />
                  <button class="mini" onclick={() => answerNeeded(card)}>Save</button>
                </div>
              {/if}
            </div>
            <div class="kn-acts">
              <button
                class="mini"
                class:active={answerFor === card.id}
                onclick={() => {
                  answerFor = answerFor === card.id ? null : card.id;
                  answerDraft = "";
                }}>Answer</button
              >
              <button class="mini ghost" onclick={() => skipNeeded(card)}>Skip</button>
              <button
                class="mini ghost"
                onclick={() => laterNeeded(card)}
                title="Ask again tomorrow">Later</button
              >
            </div>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  <div class="k-body">
    <aside class="k-side" aria-label="Lists">
      <!-- GO-ORBITTY-BOARD-ACTIONS: named macros on the Launchpad sidebar —
           the same actionShortcuts array as the rail, written via mutate(). -->
      <section class="k-actions" aria-label="Actions">
        <header class="ka-head">
          <span class="ka-title">Actions</span>
          <button
            class="ka-plus"
            onclick={() => (addingAction = !addingAction)}
            title={addingAction ? "Cancel" : "Add an action"}
            aria-label={addingAction ? "Cancel add action" : "Add action"}
            aria-expanded={addingAction}>{addingAction ? "×" : "+"}</button
          >
        </header>

        {#if nextTodo}
          {@const t = nextTodo}
          <button
            class="ka-next"
            onclick={() => onFocusCard(t.cardId)}
            title="Focus the smallest open to-do — {t.text}"
          >
            <span class="ka-next-label">Next</span>
            <span class="ka-next-text">{t.text}</span>
          </button>
        {/if}

        {#each actionShortcuts as a (a.id)}
          <div class="ka-row">
            <button
              class="ka-run"
              class:running={runningActionId === a.id}
              onclick={() => runAction(a)}
              title="Run: {a.command}">{a.name}</button
            >
            <button
              class="ka-del"
              onclick={() => deleteAction(a.id)}
              title="Delete {a.name}"
              aria-label="Delete {a.name}">×</button
            >
          </div>
        {:else}
          {#if !addingAction}
            <p class="ka-hint">No actions yet.</p>
          {/if}
        {/each}

        {#if addingAction}
          <div class="ka-form">
            <input
              bind:value={newActionName}
              placeholder="Name"
              aria-label="Action name"
            />
            <input
              bind:value={newActionCommand}
              placeholder="Command"
              aria-label="Shell command"
              onkeydown={(e) => e.key === "Enter" && addAction()}
            />
            <input
              bind:value={newActionCwd}
              placeholder="Working directory (optional)"
              aria-label="Working directory (optional)"
              onkeydown={(e) => e.key === "Enter" && addAction()}
            />
            <button class="mini" onclick={addAction}>Save</button>
          </div>
        {/if}

        {#if actionNote}
          <div class="ka-note" class:err={actionNote.kind === "err"}>
            <span class="ka-notetext">{actionNote.text}</span>
            <button class="mini ghost" onclick={() => (actionNote = null)} aria-label="Dismiss">×</button>
          </div>
        {/if}
      </section>

      <!-- GO-ORBITTY-LP-NEEDED: + Needed under the Actions well — always
           available so the first card can be born while the strip is
           hidden. One-line title; Enter/Save mints kind "needed". -->
      <section class="k-needed-side" aria-label="Needed">
        <header class="ka-head">
          <span class="ka-title">Needed</span>
          <button
            class="ka-plus"
            onclick={() => (addingNeeded = !addingNeeded)}
            title={addingNeeded ? "Cancel" : "Add a Needed question"}
            aria-label={addingNeeded ? "Cancel add needed" : "Add a Needed question"}
            aria-expanded={addingNeeded}>{addingNeeded ? "×" : "+"}</button
          >
        </header>
        {#if addingNeeded}
          <div class="ka-form">
            <input
              bind:value={newNeeded}
              placeholder="Needed from you…"
              aria-label="Needed question"
              onkeydown={(e) => e.key === "Enter" && addNeeded()}
            />
            <button class="mini" onclick={addNeeded}>Save</button>
          </div>
        {/if}
      </section>

      <div class="side-sep" aria-hidden="true"></div>

      <button
        class="side-row"
        class:sel={selected === "today"}
        onclick={() => (selected = "today")}
      >
        <span class="side-name">Today</span>
        <span class="side-count">{sideCount("today")}</span>
      </button>
      <button
        class="side-row"
        class:sel={selected === "done"}
        onclick={() => (selected = "done")}
      >
        <span class="side-name">Done</span>
        <span class="side-count">{sideCount("done")}</span>
      </button>

      <div class="side-sep" aria-hidden="true"></div>

      <button
        class="side-row"
        class:sel={selected === "inbox"}
        onclick={() => (selected = "inbox")}
      >
        <span class="side-name">Inbox</span>
        <span class="side-count">{sideCount("inbox")}</span>
      </button>
      {#each projects as p (p.id)}
        <button
          class="side-row"
          class:sel={selected === p.id}
          onclick={() => (selected = p.id)}
        >
          <span class="side-name">{p.name}</span>
          <span class="side-count">{sideCount(p.id)}</span>
        </button>
      {/each}
    </aside>

    <section class="k-main" aria-label={listTitle}>
      <div class="k-titlerow">
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
                  {@const w = winFor(card)}
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
                      {#if card.windowId}
                        {#if w?.iconBase64}
                          <img class="k-icon" src="data:image/png;base64,{w.iconBase64}" alt="" />
                        {:else}
                          <span class="k-icon fallback" aria-hidden="true">▦</span>
                        {/if}
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
        <!-- GO-ORBITTY-LP-MINE: a mixed set renders two quiet sections —
             ongoings first, then mine. One kind only stays a flat list. -->
        {#if openSplit}
          <h3 class="k-sect">{windowHeading}</h3>
          <ul class="k-rows">
            {#each openWindows as card (card.id)}
              {@render openRow(card)}
            {/each}
          </ul>
          <h3 class="k-sect">My list</h3>
          <ul class="k-rows">
            {#each openTodos as card (card.id)}
              {@render openRow(card)}
            {/each}
          </ul>
        {:else}
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
        {/if}

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
            {#if doneSplit}
              <h3 class="k-sect">{windowHeading}</h3>
              <ul class="k-rows done">
                {#each doneWindows as card (card.id)}
                  {@render doneRow(card)}
                {/each}
              </ul>
              <h3 class="k-sect">My list</h3>
              <ul class="k-rows done">
                {#each doneTodos as card (card.id)}
                  {@render doneRow(card)}
                {/each}
              </ul>
            {:else}
              <ul class="k-rows done">
                {#each visibleDone as card (card.id)}
                  {@render doneRow(card)}
                {/each}
              </ul>
            {/if}
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
    height: auto;
    flex: 1;
    min-height: 0;
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

  /* ---- GO-ORBITTY-LP-NEEDED: the Needed inbox strip (full width under
     the banner). Quiet ochre label, theme-token rows — no red, no count. ---- */
  .k-needed {
    flex-shrink: 0;
    padding: 8px 14px 10px;
    border-bottom: 1px solid var(--edge);
    background: var(--panel-fill, var(--panel));
  }

  .kn-head {
    display: flex;
    align-items: center;
    padding: 0 0 4px;
  }

  .kn-title {
    flex: 1;
    font-size: 0.66rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: #e59e3d;
  }

  .kn-rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .kn-row {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 5px 8px;
    border: 1px solid var(--edge);
    border-radius: 8px;
    background: var(--bg-fill, var(--bg));
  }

  .kn-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .kn-q {
    font-size: 0.8rem;
    line-height: 1.35;
    color: var(--ink);
    overflow-wrap: anywhere;
  }

  .kn-why {
    font-size: 0.68rem;
    line-height: 1.35;
    color: var(--dim);
    overflow-wrap: anywhere;
  }

  .kn-answer {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
  }

  .kn-answer input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 0.76rem;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 4px 8px;
  }

  .kn-answer input:focus {
    outline: none;
    border-color: var(--focus, #6e4db0);
  }

  .kn-acts {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .k-body {
    flex: 1;
    min-height: 0;
    display: flex;
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

  /* ---- GO-ORBITTY-BOARD-ACTIONS: macros well at the top of the sidebar ---- */
  .k-actions {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .ka-head {
    display: flex;
    align-items: center;
    padding: 0 4px 0 10px;
  }

  .ka-title {
    flex: 1;
    font-size: 0.66rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--dim);
  }

  .ka-plus {
    font: inherit;
    font-size: 0.82rem;
    line-height: 1;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 5px;
    padding: 1px 6px;
    cursor: pointer;
  }

  .ka-plus:hover {
    color: var(--accent);
    background: var(--bg-fill, var(--bg));
  }

  .ka-next {
    display: flex;
    align-items: baseline;
    gap: 6px;
    width: 100%;
    font: inherit;
    font-size: 0.74rem;
    text-align: left;
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 4px 10px;
    cursor: pointer;
  }

  .ka-next:hover {
    border-color: var(--focus, #6e4db0);
  }

  .ka-next-label {
    flex-shrink: 0;
    font-size: 0.62rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent);
  }

  .ka-next-text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink);
  }

  .ka-row {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .ka-run {
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
    border-radius: 7px;
    padding: 4px 10px;
    cursor: pointer;
  }

  .ka-run:hover {
    background: var(--bg-fill, var(--bg));
    color: var(--accent);
  }

  .ka-run.running {
    opacity: 0.55;
  }

  .ka-del {
    flex-shrink: 0;
    font: inherit;
    font-size: 0.74rem;
    line-height: 1;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 4px;
    padding: 1px 5px;
    cursor: pointer;
  }

  .ka-del:hover {
    color: #ff453a;
  }

  .ka-hint {
    margin: 0;
    padding: 0 10px;
    font-size: 0.66rem;
    color: var(--dim);
  }

  .ka-form {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 2px 0;
  }

  .ka-form input {
    width: 100%;
    box-sizing: border-box;
    font: inherit;
    font-size: 0.76rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 5px 8px;
  }

  .ka-form input:focus {
    outline: none;
    border-color: var(--focus, #6e4db0);
  }

  .ka-note {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 5px 8px;
    font-size: 0.68rem;
    line-height: 1.4;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    word-break: break-all;
  }

  .ka-note.err {
    border-color: var(--accent);
  }

  .ka-notetext {
    flex: 1;
    min-width: 0;
    white-space: pre-wrap;
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

  .k-row.min .k-name {
    opacity: 0.55;
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

  .k-iconbtn {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: 4px;
    cursor: pointer;
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

  /* GO-ORBITTY-LP-MINE: quiet section headings — no counts, no shame. */
  .k-sect {
    margin: 10px 2px 4px;
    font-size: 0.66rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--dim);
  }

  .k-main .k-sect:first-of-type {
    margin-top: 2px;
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
