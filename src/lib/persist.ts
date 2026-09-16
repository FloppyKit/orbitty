// TS mirror of src-tauri/src/state.rs (camelCase).
// Additive only — every key the native save writes. Pages import this so
// load → mutate → save cannot type-narrow a subset and strip slices.
// Live AX rows (WindowInfo) are not in state.json; they share this file
// so rail + lists use one window shape.

export interface WindowInfo {
  id: string;
  pid: number;
  appName: string;
  title: string;
  bundleId: string | null;
  stableKey: string;
  iconBase64: string | null;
  isFrontmost: boolean;
  isMinimized: boolean;
  documentPath?: string | null;
  sleeping?: boolean;
}

export interface Project {
  id: string;
  name: string;
  // Named counters for this planet. Additive.
  trackers?: Tracker[];
  workspaceRoot?: string | null;
}

export interface TodoTree {
  cardId: string;
  projectId: string;
  x: number;
  y: number;
  plantedAt: number;
}

export interface CharLook {
  body: number;
  clothes: number;
  eyes: number;
  mouth: number;
  hat: number;
  colors: number;
  species?: number;
  kind?: number | null;
  palette?: number | null;
}

export interface Claim {
  windowId: string;
  projectId: string;
  lastTouchedAt: number;
  bundleId?: string | null;
  appName?: string;
  titleAtClaim?: string;
  titleIndex?: number;
  spritePackId?: string | null;
  charSeed?: number | null;
  charLook?: CharLook | null;
  spriteScale?: number | null;
  walkerHome?: string | null;
  filePath?: string | null;
  hermesProfile?: string | null;
  hermesSession?: string | null;
  actionId?: string | null;
  command?: string | null;
  cwd?: string | null;
}

export type CardKind = "window" | "todo" | "needed";
export type CardStatus = "todo" | "doing" | "done";
export type NeededState = "open" | "later" | "answered" | "skipped";
export type Actor = "user" | "agent" | "action" | "vps";
export type Difficulty = "trivial" | "easy" | "medium" | "hard";

export interface Subtask {
  id: string;
  title: string;
  done: boolean;
}

export interface Tracker {
  id: string;
  label: string;
  value: number;
}

export interface KanbanCard {
  id: string;
  title: string;
  projectId: string;
  windowId?: string | null;
  kind?: CardKind;
  status: CardStatus;
  createdAt: number;
  lastTouchedAt: number;
  completedAt?: number | null;
  goalAligned: boolean;
  clinicalOrMaster: boolean;
  pointsAwarded: number;
  starred?: boolean;
  notes?: string;
  difficulty?: Difficulty;
  subtasks?: Subtask[];
  dueAt?: number | null;
  tagIds?: string[];
  neededState?: NeededState;
  neededWhy?: string;
  neededAnswer?: string;
  actor?: Actor;
  // Agent row this Todo is assigned to
  // (agentShortcuts id). Additive — never strip the key; a removed agent
  // leaves the id on the card (same law as project rm).
  assignedAgentId?: string | null;
  // Another todo this card is nested under. Hidden from the
  // stack when "Hide subtasks" is on; still a real card.
  parentId?: string | null;
  // Optional 1–7 rating (not difficulty, not points).
  rating?: number | null;
  // trackers used to live on the card. Launchpad
  // migrates any leftover onto the planet; the key stays so saves
  // do not strip old rows.
  trackers?: Tracker[];
  taskId?: string | null;
  taskReason?: string | null;
}

export interface PointsEntry {
  id: string;
  cardId: string;
  amount: number;
  day: string;
  at: number;
  actor?: Actor;
}

export interface Tag {
  id: string;
  name: string;
  color?: string | null;
}

export interface WindowTags {
  windowId: string;
  tagIds: string[];
}

export interface TitleOverride {
  windowId: string;
  title: string;
}

export interface ActionShortcut {
  id: string;
  name: string;
  command: string;
  cwd?: string | null;
  // Optional global chord, e.g. "cmd+alt+m".
  // Empty/absent = Spirit-only (chat intercept or Settings click).
  combo?: string | null;
}

export interface AgentShortcut {
  id: string;
  name: string;
  harness: string;
  profile: string;
  model?: string | null;
  mdPaths: string[];
  projectId?: string | null;
  // Free-text extra instructions — pickup
  // concatenates them into the query file after the md paths.
  extra?: string | null;
}

// One row per doing-run — the agent walker on
// that planet. Minted native-side when a gated kind=todo card lands in
// Doing (doing-fire / pickup own the process; this row is never a
// spawn). state: starting | working | blocked | done. Additive — never
// strip the key; the rail's walked-off door evicts the done row.
export interface AgentRun {
  cardId: string;
  planetId: string;
  agentShortcutId?: string;
  state: "starting" | "working" | "blocked" | "done";
  // Present only when the herdr branch of
  // launch_agent launched the run. Round-tripped opaquely — the native
  // side owns it; never a second planet DB.
  herdr?: {
    planetId: string;
    workspace: string;
    tab: string;
    pane: string;
    agentName: string;
    kind: string;
  };
}

export interface CompanionBounds {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface CompanionTurn {
  role: string;
  text: string;
}

// The four companion modes — a second axis next
// to the four-cast agent pick (stems stay). Mode owns the transcript +
// the bus target; Chat/Hold never get the GO bus.
export type CompanionMode = "spark" | "chat" | "work" | "hold";

export interface CompanionSlice {
  visible: boolean;
  bounds: CompanionBounds | null;
  imagePackId: string | null;
  state?: string;
  lastUserLine?: string;
  lastBotLine?: string;
  agentProfile?: string;
  expanded?: boolean;
  turns?: CompanionTurn[];
  speakReplies?: boolean;
  speakVolume?: number;
  talkSource?: string;
  viewCollapsed?: string;
  viewExpanded?: string;
  sprite?: boolean;
  captions?: boolean;
  planetsHidden?: boolean;
  cheatSheetHint?: boolean;
  // Active mode (default spark) + per-mode parked
  // transcripts (cap 40 each). `turns` stays the ACTIVE thread; the
  // native heal parks a pre-split transcript under spark on load.
  mode?: CompanionMode;
  threads?: Partial<Record<CompanionMode, CompanionTurn[]>>;
  // Ctl-lit "agent working" flag for the dock LED
  // (`orbittyctl companion hack -- on [--who cursor]`). hackWho names the
  // live source for the LED title. A flag, never a transcript.
  hackLive?: boolean;
  hackWho?: string;
  // the live agent's pinned planet
  // (pickup writes it) — the dest/planet menus glow that one planet
  // while hackLive is on. null/undefined = no pin; cleared on off.
  hackProjectId?: string | null;
  // Free Mode — free lanes only (local Ollama →
  // OpenRouter free); paid Grok stays off. The BYOK key never lives in
  // state — native keeps it 0600 under companions/orbitty-primary.
  // REQUIRED — an omitted key heals to false on the native
  // save and silently strips the mode (the agentRuns strip-trap). Page
  // state loaded from disk defaults it false; never leave it undefined.
  freeMode: boolean;
  // Conductor — the slim local fast-ack layer in
  // front of the mouth. Optional in TS (agentRuns precedent): the
  // native save_state carries the disk values forward (same strip-trap
  // as freeMode), so a stale whole-state writer can never strip them;
  // the set_companion_conductor* commands are the only doors.
  conductorOn?: boolean;
  conductorModel?: string;
}

export interface StarsCheckin {
  sleep: number;
  terroir: number;
  athleticism: number;
  relationships: number;
  self: number;
}

export type AverageWindow = "week" | "14" | "month" | "year" | "custom";

export interface StarsSlice {
  promptsOptIn: boolean;
  checkins: Record<string, StarsCheckin>;
  promptsAck?: string;
  averageWindow?: AverageWindow;
  averageFrom?: string;
  averageTo?: string;
  // 2 = spokes are 1–5; absent = legacy 0–4, which
  // native setup migrates n→n+1 once before any window reads state.
  scaleV?: number;
}

export interface SkySlice {
  muted: string[];
}

export type SharedPaneMode = "board" | "stars";

export interface DrawerSlice {
  docked?: boolean;
  dx?: number;
  dy?: number;
  bounds?: CompanionBounds | null;
}

export interface PersistedState {
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
  autohideRowChrome: boolean;
  hideTitles: boolean;
  hideAppIcons: boolean;
  spriteScale: number;
  marbleDrift: boolean;
  // Herding Cats confirm source checkboxes
  // — independent bools, at least one on before Drop can arm. Tabs stay
  // disabled until Tabbity lands. The old `yardSource` radio key is no
  // longer written; a save that predates the bools heals to windows on /
  // tabs off via the native serde default. No yardCats: cats never
  // persist. Optional in TS so whole-state writers that predate the
  // fields (the rail's persist) keep type-checking.
  yardWindows?: boolean;
  yardTabs?: boolean;
  actionShortcuts: ActionShortcut[];
  agentShortcuts: AgentShortcut[];
  // Optional in TS so whole-state writers that
  // predate the field keep type-checking; the native save always writes it.
  agentRuns?: AgentRun[];
  // Daily spend ceiling + runner cap. Optional so
  // older whole-state writers keep type-checking; native defaults 20 / 2.
  spendCapUsd?: number;
  spendCapEnabled?: boolean;
  runnerCap?: number;
  spendDay?: string;
  spendUsd?: number;
  // Launchpad mail: Needed fraction of column 1 (0.18–0.45). Default 0.28.
  neededSplit?: number;
  // Parent todo ids whose nested subtasks are expanded.
  openSubtaskIds?: string[];
  // Unbound-todo trees on planets. Additive.
  todoTrees?: TodoTree[];
  // Days to full canopy when the todo has no due. Default 14.
  treeGrowDays?: number;
  companion: CompanionSlice;
  stars: StarsSlice;
  sky: SkySlice;
  sharedPaneMode: SharedPaneMode;
  drawers: Record<string, DrawerSlice>;
}
