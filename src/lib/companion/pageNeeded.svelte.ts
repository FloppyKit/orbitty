// Moved function bodies. scope fields are the lets each function already closed over.

import type { CompanionTurn, KanbanCard, StarsSlice } from "../persist";

interface CompanionAgents {
  agents: string[];
  selected: string;
}

interface AttachChip {
  path: string;
  name: string;
  size: number;
}

type Invoke = <T = unknown>(cmd: string, args?: any) => Promise<T>;

type Scope_todayKey = {};

type Scope_toggleNextCard = {
  invoke: Invoke;
};

type Scope_togglePrompts = {
  prompts: boolean;
  invoke: Invoke;
  sendNote: string | null;
};

type Scope_ackStrip = {
  promptAckKey: any;
  stars: StarsSlice;
  invoke: Invoke;
  sendNote: string | null;
};

type Scope_smallestTodo = {
  cards: KanbanCard[];
};

type Scope_openSmallestTodo = {
  smallestTodo: (...args: any[]) => { cardId: string; text: string } | null;
  ackStrip: (...args: any[]) => Promise<any>;
  sendNote: string | null;
  enterHost: (...args: any[]) => Promise<any>;
  emitTo: (...args: any[]) => any;
};

type Scope_talkStop = {
  talking: boolean;
  fieldEl: HTMLTextAreaElement | null;
  invoke: Invoke;
};

type Scope_talkStart = {
  talking: boolean;
  fieldEl: HTMLTextAreaElement | null;
  talkBase: string;
  draft: string;
  invoke: Invoke;
  sendNote: string | null;
};

type Scope_talkStopOnly = {
  sendOnTalkEnd: boolean;
  talkStop: (...args: any[]) => Promise<any>;
};

type Scope_talkStopSend = {
  talking: boolean;
  sendOnTalkEnd: boolean;
  talkStop: (...args: any[]) => Promise<any>;
  sendLine: (...args: any[]) => Promise<any>;
};

type Scope_applyTalkText = {
  draft: string;
  talkBase: string;
  fieldEl: HTMLTextAreaElement | null;
};

type Scope_onNeededFocus = {
  neededFocusId: string | null;
  neededFocusTitle: string | null;
  mouthHooked: boolean;
  landCaret: (...args: any[]) => Promise<any>;
};

type Scope_composerField = {
  fieldEl: HTMLTextAreaElement | null;
};

type Scope_focusComposer = {
  composerField: (...args: any[]) => HTMLTextAreaElement | null;
};

type Scope_landCaret = {
  tick: (...args: any[]) => Promise<any>;
  focusComposer: (...args: any[]) => boolean;
};

type Scope_onNeededAnswer = {
  landCaret: (...args: any[]) => Promise<any>;
};

type Scope_sendLine = {
  draft: string;
  attach: AttachChip | null;
  neededFocusId: string | null;
  invoke: Invoke;
  sendNote: string | null;
  sending: boolean;
  ackLine: string;
  pendingAck: string;
  ackReveal: ReturnType<typeof setTimeout> | undefined;
  ACK_SPIN_MS: number;
  turns: CompanionTurn[];
  userLine: string;
  botLine: string;
};

type Scope_loadAgents = {
  invoke: Invoke;
  agents: string[];
  agent: string;
};

type Scope_stampThin = {
  invoke: Invoke;
  sendNote: string | null;
};

type Scope_onAgentChange = {
  agent: string;
  invoke: Invoke;
  AGENT_MODE: any;
  sendNote: string | null;
  loadAgents: (...args: any[]) => Promise<any>;
};

export function todayKey(scope: Scope_todayKey): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function toggleNextCard(scope: Scope_toggleNextCard) {
  scope.invoke("toggle_next_card_window").catch(() => {});
}

export async function togglePrompts(scope: Scope_togglePrompts) {
  const next = !scope.prompts;
  scope.prompts = next;
  try {
    await scope.invoke("set_stars_prompts", { enabled: next });
  } catch (e) {
    scope.prompts = !next;
    scope.sendNote = `${e}`;
  }
}

export async function ackStrip(scope: Scope_ackStrip) {
  if (!scope.promptAckKey) return;
  const ack = scope.promptAckKey;
  scope.stars = { ...scope.stars, promptsAck: ack };
  try {
    await scope.invoke("ack_stars_prompt", { ack });
  } catch (e) {
    scope.sendNote = `${e}`;
  }
}

export function smallestTodo(scope: Scope_smallestTodo): { cardId: string; text: string } | null {
  const card = scope.cards.find((c) => c.status === "doing");
  if (!card) return null;
  const sub = card.subtasks?.find((s) => !s.done);
  return { cardId: card.id, text: sub?.title || card.title };
}

export async function openSmallestTodo(scope: Scope_openSmallestTodo) {
  const t = scope.smallestTodo();
  await scope.ackStrip();
  if (!t) {
    scope.sendNote = "Nothing open on the Board.";
    return;
  }
  try {
    // HOST-SPLIT: the card opens in the embedded Launchpad well — never
    // the overlay on the spirit. The focus event targets THIS window
    // (the embedded KanbanPage listens here); the re-emit covers a well
    // still mid-mount when the first event lands.
    await scope.enterHost("launchpad");
    scope.emitTo("companion", "orbitty:focus-card", t.cardId).catch(() => {});
    setTimeout(
      () =>
        scope.emitTo("companion", "orbitty:focus-card", t.cardId).catch(() => {}),
      400,
    );
  } catch (e) {
    scope.sendNote = `${e}`;
  }
}

export async function talkStop(scope: Scope_talkStop) {
  scope.talking = false;
  scope.fieldEl?.focus();
  try {
    await scope.invoke("talk_stop");
  } catch {
    /* release is best-effort — never leave the chord stuck */
  }
  scope.fieldEl?.focus();
}

export async function talkStart(scope: Scope_talkStart, e: Event) {
  e.preventDefault();
  if (scope.talking) return;
  scope.fieldEl?.focus();
  try {
    scope.talkBase = scope.draft.trimEnd();
    await scope.invoke("talk_start");
    scope.talking = true;
    scope.sendNote = null;
    scope.fieldEl?.focus();
  } catch (err) {
    scope.talking = false;
    scope.sendNote = `${err}`;
  }
}

export async function talkStopOnly(scope: Scope_talkStopOnly) {
  scope.sendOnTalkEnd = false;
  await scope.talkStop();
}

export async function talkStopSend(scope: Scope_talkStopSend) {
  if (!scope.talking) return;
  scope.sendOnTalkEnd = true;
  await scope.talkStop();
  await new Promise((r) => setTimeout(r, 80));
  if (scope.sendOnTalkEnd) {
    scope.sendOnTalkEnd = false;
    await scope.sendLine();
  }
}

export function applyTalkText(scope: Scope_applyTalkText, text: string) {
  const spoken = (text ?? "").trim();
  if (!spoken) {
    scope.draft = scope.talkBase;
  } else if (!scope.talkBase) {
    scope.draft = spoken;
  } else {
    scope.draft = `${scope.talkBase} ${spoken}`;
  }
  scope.fieldEl?.focus();
}

export function onNeededFocus(scope: Scope_onNeededFocus, id: string | null, title: string | null) {
  scope.neededFocusId = id;
  scope.neededFocusTitle = title;
  scope.mouthHooked = !!id;
  if (id) void scope.landCaret();
}

export function composerField(scope: Scope_composerField): HTMLTextAreaElement | null {
  return (
    scope.fieldEl ??
    document.querySelector<HTMLTextAreaElement>("textarea.field")
  );
}

export function focusComposer(scope: Scope_focusComposer): boolean {
  const el = scope.composerField();
  if (!el) return false;
  el.focus();
  try {
    const n = el.value.length;
    el.setSelectionRange(n, n);
  } catch {
    /* setSelectionRange can throw on a detached node */
  }
  return document.activeElement === el;
}

export async function landCaret(scope: Scope_landCaret) {
  await scope.tick();
  await new Promise<void>((r) =>
    requestAnimationFrame(() => requestAnimationFrame(() => r())),
  );
  if (scope.focusComposer()) return;
  for (const ms of [40, 100, 220, 400]) {
    await new Promise((r) => setTimeout(r, ms));
    if (scope.focusComposer()) return;
  }
}

export function onNeededAnswer(scope: Scope_onNeededAnswer) {
  void scope.landCaret();
}

export async function sendLine(scope: Scope_sendLine) {
  const line = scope.draft.trim();
  const file = scope.attach;
  if (!line && !file) return;
  // Highlighted Needed in Launchpad owns the mouth — even with
  // Launchpad in its own window. A file chip stands the hijack down.
  if (scope.neededFocusId && !file) {
    scope.draft = "";
    try {
      await scope.invoke("needed_answer", { id: scope.neededFocusId, answer: line });
    } catch (e) {
      scope.draft = line;
      scope.sendNote = `${e}`;
    }
    return;
  }
  if (scope.sending) return;
  scope.sending = true;
  scope.sendNote = null;
  scope.ackLine = "";
  scope.pendingAck = "";
  if (scope.ackReveal) clearTimeout(scope.ackReveal);
  scope.ackReveal = setTimeout(() => {
    scope.ackReveal = undefined;
    if (scope.sending && scope.pendingAck) scope.ackLine = scope.pendingAck;
  }, scope.ACK_SPIN_MS);
  // S2.5 optimistic you: the bubble lands NOW, the field clears, and
  // the scroll effect (watching turns.length + sending) pins the
  // expanded transcript to the bottom; `sending` swaps the spirit to
  // the thinking v9 loop until the reply lands — the wait mark sits
  // bare in the agent-text slot (no bubble, no border) where the
  // reply will land; spirit still plays thinking v9.
  // A file-only send bubbles "[file] name" (the
  // same stand-in companion_send persists); the chip vanishes on send
  // and returns only on an honest error.
  const youText = line || `[file] ${file?.name ?? ""}`;
  scope.turns = [...scope.turns, { role: "you", text: youText }].slice(-40);
  scope.userLine = youText;
  scope.draft = "";
  scope.attach = null;
  try {
    const reply = await scope.invoke<string>("companion_send", {
      line,
      path: file?.path ?? null,
    });
    scope.botLine = reply;
    // CAPTION-CHUNK: speaking is the arrival effect's job — fill the
    // live CC box, then read page one.
    // companion_send persists the exchange and emits state:changed;
    // if that loadState won the race, this exact exchange is already
    // the tail — append the reply only when it isn't.
    const n = scope.turns.length;
    const landed =
      n >= 2 &&
      scope.turns[n - 1].role === "them" &&
      scope.turns[n - 1].text === reply &&
      scope.turns[n - 2].role === "you" &&
      scope.turns[n - 2].text === youText;
    if (!landed) {
      scope.turns = [...scope.turns, { role: "them", text: reply }].slice(-40);
    }
    scope.ackLine = "";
    scope.pendingAck = "";
    if (scope.ackReveal) {
      clearTimeout(scope.ackReveal);
      scope.ackReveal = undefined;
    }
  } catch (e) {
    // The you bubble stays (it was appended up front); the draft is
    // already gone — one honest error line is all that remains. The
    // chip comes back too: the file never left, so don't fake it.
    scope.sendNote = `${e}`;
    scope.ackLine = "";
    scope.pendingAck = "";
    if (scope.ackReveal) {
      clearTimeout(scope.ackReveal);
      scope.ackReveal = undefined;
    }
    scope.attach = file;
  } finally {
    scope.sending = false;
  }
}

export async function loadAgents(scope: Scope_loadAgents) {
  try {
    const a = await scope.invoke<CompanionAgents>("companion_agents");
    scope.agents = a.agents.length > 0 ? a.agents : ["default"];
    scope.agent = a.selected;
  } catch {
    scope.agents = ["default"];
    scope.agent = "default";
  }
}

export async function stampThin(scope: Scope_stampThin) {
  try {
    const msg = await scope.invoke<string>("companion_thin");
    scope.sendNote = msg.split("\n")[0] ?? "THIN stamped";
  } catch (e) {
    scope.sendNote = `${e}`;
  }
}

export async function onAgentChange(scope: Scope_onAgentChange) {
  try {
    scope.agent = await scope.invoke<string>("set_companion_agent", { profile: scope.agent });
    // Same flip switches the JOB (mode law). Transcript stays.
    const nextMode = scope.AGENT_MODE[scope.agent];
    if (nextMode) {
      await scope.invoke("set_companion_mode", { mode: nextMode });
    }
  } catch (e) {
    scope.sendNote = `${e}`;
    await scope.loadAgents(); // snap back to the persisted pick
  }
}
