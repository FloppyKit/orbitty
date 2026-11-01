// Pure launcher: catalogue, rank, keys, clear-after-run.
// No DOM, no invoke. The dock paints rows and CompanionPage runs actions.
// Planet dots are passed in (planetCss lives with the caller).

export const VISIBLE_ROWS = 5;
export const MAX_ROWS = 50;
export const ROW_PX = 30;
export const LIST_PX = VISIBLE_ROWS * ROW_PX;

export type Action =
  | { t: "send" }
  | { t: "answer"; id: string }
  | { t: "invoke"; cmd: string; args?: Record<string, unknown> }
  | { t: "host"; mode: "launchpad" | "stars" | "inspector" }
  | { t: "page"; fn: string; args?: Record<string, unknown> };

export type LaunchRow = {
  id: string;
  title: string;
  subtitle?: string;
  dot?: string;
  glyph: string;
  kind: "ask" | "answer" | "command";
  run: Action;
};

export type PlanetRef = { id: string; name: string; dot?: string };
export type AgentRef = { id: string; label: string };
export type TodoRef = { cardId: string; text: string };

export type LaunchCtx = {
  query: string;
  armed: { id: string; title: string } | null;
  planets: PlanetRef[];
  hosted: "off" | "launchpad" | "stars" | "inspector";
  hostBusy: boolean;
  agent: string;
  agents: AgentRef[];
  mode: string;
  speak: boolean;
  sprite: boolean;
  captions: boolean;
  head: boolean;
  prompts: boolean;
  nextCount: number;
  smallestTodo: TodoRef | null;
  planetsHidden: boolean;
};

export type KeyInput = {
  key: string;
  shift?: boolean;
  meta?: boolean;
  alt?: boolean;
  ctrl?: boolean;
  composing?: boolean;
  /** Field has a non-empty selection. Esc collapses that and stops. */
  hasSelection?: boolean;
};

export type KeyState = {
  query: string;
  selected: number;
  count: number;
  visibleStart: number;
  stash: string | null;
};

export type KeyEffect =
  | "run"
  | "newline"
  | "complete"
  | "blur"
  | "ignore"
  | "collapse-selection"
  | "none";

export type KeyResult = {
  state: KeyState;
  effect: KeyEffect;
  runIndex?: number;
  complete?: string;
};

const SUGGESTION_IDS = [
  "open-smallest",
  "open-board",
  "open-stars",
  "planets",
  "claim-front",
] as const;

const MODES = ["spark", "chat", "work", "hold"] as const;
const MODE_LABEL: Record<(typeof MODES)[number], string> = {
  spark: "Spark",
  chat: "Chat",
  work: "Work",
  hold: "Hold",
};

function glyph(id: string): string {
  if (id.startsWith("open-board") || id.startsWith("board-")) return "B";
  if (id.includes("star")) return "S";
  if (
    id.startsWith("planet") ||
    id.startsWith("goto-") ||
    id.startsWith("locate-") ||
    id.startsWith("bring-")
  ) {
    return "P";
  }
  if (id.startsWith("claim")) return "C";
  if (id.startsWith("settings")) return "G";
  if (id.startsWith("agent-")) return "A";
  if (id.startsWith("mode-")) return "M";
  if (id.startsWith("speak")) return "V";
  if (id.startsWith("caption")) return "C";
  if (id.startsWith("sprite") || id.startsWith("figure")) return "F";
  if (id.startsWith("talk")) return "T";
  if (id.startsWith("next-")) return "N";
  if (id.startsWith("cheat")) return "?";
  if (id.startsWith("expand")) return "E";
  if (id.startsWith("side-")) return "W";
  if (id === "ask") return "?";
  if (id.startsWith("answer:")) return "A";
  return "·";
}

function command(partial: {
  id: string;
  title: string;
  subtitle?: string;
  dot?: string;
  run: Action;
}): LaunchRow {
  return {
    kind: "command",
    glyph: glyph(partial.id),
    ...partial,
  };
}

function askRow(query: string): LaunchRow {
  const text = query.trim();
  return {
    id: "ask",
    title: `Ask Orbitty: ${text}`,
    glyph: "?",
    kind: "ask",
    run: { t: "send" },
  };
}

function answerRow(armed: { id: string; title: string }): LaunchRow {
  return {
    id: `answer:${armed.id}`,
    title: `Answer: ${armed.title}`,
    subtitle: "Needed",
    glyph: "A",
    kind: "answer",
    run: { t: "answer", id: armed.id },
  };
}

function hostRow(
  id: string,
  title: string,
  mode: "launchpad" | "stars" | "inspector",
): LaunchRow {
  return command({ id, title, run: { t: "host", mode } });
}

/** Every command the mouth can offer, in catalogue order. No Ask row. */
export function catalogue(ctx: LaunchCtx): LaunchRow[] {
  const rows: LaunchRow[] = [];
  const push = (row: LaunchRow) => rows.push(row);
  const invoke = (id: string, title: string, cmd: string, args?: Record<string, unknown>, extra?: { subtitle?: string; dot?: string }) =>
    push(command({ id, title, subtitle: extra?.subtitle, dot: extra?.dot, run: { t: "invoke", cmd, args } }));

  invoke("open-board", "Open Board", "dest_show", { dest: "launchpad" });
  if (!ctx.hostBusy) push(hostRow("board-here", "Board in this window", "launchpad"));
  if (ctx.smallestTodo) {
    push(
      command({
        id: "open-smallest",
        title: "Open smallest todo",
        subtitle: ctx.smallestTodo.text,
        run: { t: "page", fn: "openSmallestTodo" },
      }),
    );
  }

  invoke("open-stars", "Open Stars", "dest_show", { dest: "stars" });
  invoke("stars-beside", "Stars beside planets", "dest_show", { dest: "stars-beside" });
  if (!ctx.hostBusy) push(hostRow("stars-here", "Stars in this window", "stars"));
  push(
    command({
      id: "stars-prompts",
      title: "Stars prompts",
      subtitle: ctx.prompts ? "On" : "Off",
      run: { t: "page", fn: "togglePrompts" },
    }),
  );

  invoke("planets", "Planets", "dest_show", { dest: "inspector" });
  if (!ctx.hostBusy) push(hostRow("planets-here", "Planets in this window", "inspector"));
  push(
    command({
      id: "planets-visibility",
      title: ctx.planetsHidden ? "Show planets" : "Hide planets",
      run: { t: "page", fn: "togglePlanetsHidden" },
    }),
  );
  for (const planet of ctx.planets) {
    invoke(
      `goto-${planet.id}`,
      `Go to ${planet.name}`,
      "bring_planet_front",
      { id: planet.id },
      { dot: planet.dot },
    );
    invoke(
      `locate-${planet.id}`,
      `Locate ${planet.name}`,
      "locate_planet",
      { id: planet.id },
      { dot: planet.dot },
    );
  }
  invoke("bring-all", "Bring all planets front", "bring_all_front");

  invoke("claim-front", "Claim front window", "claim_front_window");
  for (const planet of ctx.planets) {
    invoke(
      `claimto-${planet.id}`,
      `Claim front window to ${planet.name}`,
      "claim_front_to",
      { projectId: planet.id },
      { dot: planet.dot },
    );
  }
  invoke("claim-undo", "Undo claim", "claim_front_undo");

  invoke("settings", "Settings", "dest_show", { dest: "settings" });
  invoke("settings-beside", "Settings beside planets", "dest_show", { dest: "settings-beside" });
  invoke("side-window", "Open side window", "show_rail");
  invoke("next-card", "Next card", "toggle_next_card_window", undefined, {
    subtitle: ctx.nextCount > 0 ? String(ctx.nextCount) : undefined,
  });
  invoke("cheat-sheet", "Cheat sheet", "toggle_cheatsheet");
  push(
    command({
      id: "expand-chat",
      title: "Expand chat",
      run: { t: "page", fn: "toggleExpanded" },
    }),
  );

  for (const agent of ctx.agents) {
    push(
      command({
        id: `agent-${agent.id}`,
        title: `Agent: ${agent.label}`,
        subtitle: agent.id === ctx.agent ? "On" : undefined,
        run: { t: "page", fn: "pickAgent", args: { id: agent.id } },
      }),
    );
  }
  for (const mode of MODES) {
    invoke(`mode-${mode}`, `Mode: ${MODE_LABEL[mode]}`, "set_companion_mode", { mode }, {
      subtitle: ctx.mode === mode ? "On" : undefined,
    });
  }
  push(
    command({
      id: "speak",
      title: "Speak",
      subtitle: ctx.speak ? "On" : "Off",
      run: { t: "page", fn: "toggleSpeak" },
    }),
  );
  push(
    command({
      id: "captions",
      title: "Captions",
      subtitle: ctx.captions ? "On" : "Off",
      run: { t: "page", fn: "toggleCaptions" },
    }),
  );
  push(
    command({
      id: "sprite",
      title: "Sprite",
      subtitle: ctx.sprite ? "On" : "Off",
      run: { t: "page", fn: "toggleSprite" },
    }),
  );
  push(
    command({
      id: "figure",
      title: "Head or full figure",
      subtitle: ctx.head ? "Head" : "Full",
      run: { t: "page", fn: "toggleView" },
    }),
  );
  push(
    command({
      id: "talk",
      title: "Talk",
      run: { t: "page", fn: "talkStart" },
    }),
  );

  return rows;
}

function wordsOf(title: string): string[] {
  return title.toLowerCase().split(/[^a-z0-9]+/).filter(Boolean);
}

function isSubsequence(query: string, title: string): boolean {
  let i = 0;
  for (const ch of title) {
    if (ch === query[i]) i += 1;
    if (i >= query.length) return true;
  }
  return false;
}

/** Higher score = tighter word-start. Exact word is its own tier. */
function scoreTitle(query: string, title: string): { tier: number; score: number } | null {
  const words = wordsOf(title);
  let best = 0;
  let exact = false;
  for (const word of words) {
    if (!word.startsWith(query)) continue;
    if (word === query) exact = true;
    const score = Math.round((query.length * 1000) / word.length);
    if (score > best) best = score;
  }
  if (exact) return { tier: 0, score: 1000 };
  if (best > 0) return { tier: 1, score: best };
  if (isSubsequence(query, title.toLowerCase())) return { tier: 2, score: 0 };
  return null;
}

function suggestionRows(rows: LaunchRow[], recents: readonly string[]): LaunchRow[] {
  const byId = new Map(rows.map((row) => [row.id, row]));
  const fixed = SUGGESTION_IDS.filter((id) => byId.has(id));
  const top = recents.find((id) => byId.has(id));
  const ordered: string[] = [];
  if (top) ordered.push(top);
  for (const id of fixed) {
    if (!ordered.includes(id)) ordered.push(id);
  }
  const out: LaunchRow[] = [];
  for (const row of rows) {
    if (row.kind === "answer") out.push(row);
  }
  for (const id of ordered) {
    const row = byId.get(id);
    if (row && !out.includes(row)) out.push(row);
  }
  return out;
}

/**
 * Empty query → Suggestions in fixed order (a recent id may lead).
 * A leading "/" hides nothing here (Ask is added by launcherItems) and
 * keeps only the best match tier, so "/st" is Stars and "/cl" is Claim.
 * Otherwise every match, best tier and tightest word first, catalogue order on ties.
 */
export function rankItems(
  query: string,
  rows: LaunchRow[],
  recents: readonly string[] = [],
): LaunchRow[] {
  const raw = query.trim();
  const slash = raw.startsWith("/");
  const q = (slash ? raw.slice(1) : raw).trim().toLowerCase();
  if (!q) return suggestionRows(rows, recents);

  const matched: { row: LaunchRow; index: number; tier: number; score: number }[] = [];
  rows.forEach((row, index) => {
    if (row.kind === "ask") return;
    const scored = scoreTitle(q, row.title);
    if (!scored) return;
    matched.push({ row, index, tier: scored.tier, score: scored.score });
  });
  matched.sort((a, b) => a.tier - b.tier || b.score - a.score || a.index - b.index);
  if (matched.length === 0) return [];
  if (slash) {
    const best = matched[0];
    return matched
      .filter((item) => item.tier === best.tier && item.score === best.score)
      .map((item) => item.row);
  }
  return matched.map((item) => item.row);
}

/**
 * Which row Enter / Send should run for a fresh query.
 * Ask stays row 0. One word with no spaces preselects the first command
 * that matches by exact word or word-start (tier 0–1), so "boa" is Open
 * Board and Ask is one ArrowUp away. Two or more words, a leading "?",
 * a slash, an armed answer row, or a subsequence-only match stay on row 0.
 */
export function pickIndex(query: string, items: readonly LaunchRow[]): number {
  const raw = query.trim();
  if (!raw || raw.startsWith("?") || raw.startsWith("/") || /\s/.test(raw)) return 0;
  if (items[0]?.kind === "answer") return 0;
  const q = raw.toLowerCase();
  for (let i = 0; i < items.length; i++) {
    const row = items[i];
    if (!row || row.kind !== "command") continue;
    const scored = scoreTitle(q, row.title);
    if (scored && scored.tier <= 1) return i;
  }
  return 0;
}

export function launcherItems(ctx: LaunchCtx, recents: readonly string[] = []): LaunchRow[] {
  const rows = catalogue(ctx);
  const raw = ctx.query.trim();
  const slash = raw.startsWith("/");
  const ranked = rankItems(ctx.query, rows, recents).filter((row) => row.kind !== "answer");
  const head: LaunchRow[] = [];
  if (!slash && ctx.armed) head.push(answerRow(ctx.armed));
  else if (!slash && raw) head.push(askRow(ctx.query));
  return [...head, ...ranked].slice(0, MAX_ROWS);
}

export function noteRecent(ids: readonly string[], id: string): string[] {
  if (!id || id === "ask" || id.startsWith("answer:")) return [...ids];
  return [id, ...ids.filter((item) => item !== id)].slice(0, 8);
}

/**
 * Clear the field after a command when the text was the query.
 * A chat draft kept by arrowing onto a weak match stays.
 */
export function clearAfterRun(input: {
  kind: Action["t"];
  query: string;
  title: string;
  byArrow: boolean;
}): boolean {
  if (input.kind === "send" || input.kind === "answer") return true;
  const raw = input.query.trim();
  if (!raw) return true;
  if (raw.startsWith("/")) return true;
  const q = raw.toLowerCase();
  const scored = scoreTitle(q, input.title);
  if (input.byArrow && (!scored || scored.tier >= 2)) return false;
  return true;
}

function clampStart(selected: number, count: number, start: number): number {
  if (count <= 0) return 0;
  const maxStart = Math.max(0, count - VISIBLE_ROWS);
  let next = Math.min(Math.max(0, start), maxStart);
  if (selected < next) next = selected;
  if (selected >= next + VISIBLE_ROWS) next = selected - VISIBLE_ROWS + 1;
  return Math.min(Math.max(0, next), maxStart);
}

export function launcherKey(state: KeyState, input: KeyInput, titles: readonly string[]): KeyResult {
  const key = input.key;
  const meta = !!(input.meta || input.ctrl);
  if (input.composing && key === "Enter") return { state, effect: "ignore" };
  if (key === "Enter" && input.shift) return { state, effect: "newline" };
  if (key === "Enter") {
    if (state.count <= 0) return { state, effect: "ignore" };
    const index = Math.min(Math.max(0, state.selected), state.count - 1);
    return { state, effect: "run", runIndex: index };
  }
  if ((key === "ArrowDown" || key === "ArrowUp") && state.count > 0) {
    const dir = key === "ArrowDown" ? 1 : -1;
    const selected = (state.selected + dir + state.count) % state.count;
    const visibleStart = clampStart(selected, state.count, state.visibleStart);
    return { state: { ...state, selected, visibleStart }, effect: "none" };
  }
  if (meta && !input.alt && /^[1-5]$/.test(key)) {
    const index = state.visibleStart + Number(key) - 1;
    if (index < 0 || index >= state.count) return { state, effect: "ignore" };
    return { state, effect: "run", runIndex: index };
  }
  if (key === "Tab" && state.count > 0 && !input.shift) {
    const title = titles[state.selected] ?? "";
    if (!title) return { state, effect: "ignore" };
    return {
      state: { ...state, query: title, selected: 0, visibleStart: 0, stash: null },
      effect: "complete",
      complete: title,
    };
  }
  if (meta && !input.shift && (key === "z" || key === "Z") && state.stash !== null) {
    return {
      state: { ...state, query: state.stash, stash: null, selected: 0, visibleStart: 0 },
      effect: "none",
    };
  }
  if (key === "Escape") {
    if (input.hasSelection) return { state, effect: "collapse-selection" };
    if (state.selected !== 0) {
      return { state: { ...state, selected: 0, visibleStart: 0 }, effect: "none" };
    }
    if (state.query) {
      return {
        state: { ...state, query: "", stash: state.query, selected: 0, visibleStart: 0 },
        effect: "none",
      };
    }
    return { state, effect: "blur" };
  }
  return { state, effect: "none" };
}

/** Border-box height and the sprite-on grow extra for one field measure.
 *  Collapsed (docked, not hosted) never caps, sprite on or off: capping
 *  is what hid the draft after the window left the grow path. Expanded
 *  and hosted keep the 6-line cap. `sprite` is part of the key so a flip
 *  recomputes; it does not shrink the box. The extra stays the grown
 *  lines so a sprite-off hug still re-fires when the draft gets taller. */
export function refitComposer(input: {
  needed: number;
  oneLineH: number;
  lineH: number;
  expanded: boolean;
  hostedOff: boolean;
  sprite: boolean;
  maxExtraLines?: number;
}): { height: number; composerExtra: number; scroll: boolean } {
  const maxExtra = input.maxExtraLines ?? 5;
  const collapsed = !input.expanded && input.hostedOff;
  const max = input.oneLineH > 0 ? input.oneLineH + maxExtra * input.lineH : input.needed;
  const scroll = !collapsed && input.needed > max + 0.5;
  return {
    height: scroll ? max : input.needed,
    composerExtra:
      collapsed && input.oneLineH > 0 ? Math.max(0, Math.round(input.needed - input.oneLineH)) : 0,
    scroll,
  };
}
