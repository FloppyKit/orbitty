// GO-ORBITTY-CLEAN-TYPES: TS mirror of src-tauri/src/state.rs (camelCase).
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
  // GO-ORBITTY-ACTION-FILE: optional global chord, e.g. "cmd+alt+m".
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
  talkSource?: string;
  viewCollapsed?: string;
  viewExpanded?: string;
  sprite?: boolean;
}

export interface StarsCheckin {
  sleep: number;
  terroir: number;
  athleticism: number;
  relationships: number;
  self: number;
}

export type AverageWindow = "week" | "14" | "month";

export interface StarsSlice {
  promptsOptIn: boolean;
  checkins: Record<string, StarsCheckin>;
  promptsAck?: string;
  averageWindow?: AverageWindow;
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
  // GO-ORBITTY-YARD-WARN: Herding Cats confirm source radio — only
  // "windows" exists until Tabbity lands. No yardCats: cats never persist.
  // Native always writes this (serde default heals a missing key back to
  // "windows" on every save); optional in TS so whole-state writers that
  // predate the field (the rail's persist) keep type-checking. Becomes
  // required when the yard lands.
  yardSource?: string;
  actionShortcuts: ActionShortcut[];
  agentShortcuts: AgentShortcut[];
  companion: CompanionSlice;
  stars: StarsSlice;
  sky: SkySlice;
  sharedPaneMode: SharedPaneMode;
  drawers: Record<string, DrawerSlice>;
}
