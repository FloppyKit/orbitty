<script lang="ts">
  // GO-LS-TAURI-AVATAR-LARGE — the large companion window. One silent
  // spirit: the blessed ghost.png sheet / idle_0…7 slices, 8 frames @
  // 140 ms, nearest-neighbor, hard alpha. Default body is the blessed
  // spirit. A Body picker can swap a `role: companion` pack (Jacob
  // temp bake) via imagePackId — no CSS bounce, no import UI. No points,
  // no editor tabs. GO-LS-STARS-STATUS adds the opt-in prompts strip (one
  // menu, never a loop) and hold-to-talk — nothing is ever drawn on the
  // spirit itself.
  // S2.4 / GO-COMPANION-BERD: REAL send via native companion_send.
  // Default backend = Berd (bound Orbitty Companion session + berdctl).
  // Escape: ORBITTY_COMPANION_BACKEND=hermes → legacy hermes path.
  // Async — Send disables while in flight. Never hosts a model here.
  // Agent picker lists ~/.agents/agents stems (Berd) by default.
  // S2.5: OPTIMISTIC you — the bubble lands on Send (before the agent
  // answers), the field clears, the transcript pins to the bottom, and a
  // pending them bubble holds until the reply; a failed send keeps
  // the you bubble. Expanded transcript = SMS bubbles (you right on
  // accent, them left muted, no you:/them: prefixes). Collapsed = true
  // CC (latest you / latest them). Expanded parks the surface view as a
  // 96×96 portrait absolute upper-right (CORNER); drag moves to that
  // portrait so the transcript can scroll. Missing art fails obvious
  // ("spirit missing").
  // S2.6: collapsed CC gets the same SMS energy — you right on accent,
  // them left on panel+edge, prefixes gone everywhere.
  // S2.7: the in-flight wait replaced the cycling Sending/Thinking/…
  // words (they felt fake). S2.8/S2.9 drew the wait as the Orbitty mark
  // in a pending bubble; AVATAR-THINK-V9 took the wait mark out of chat
  // chrome entirely — while a send is in flight the spirit itself (and
  // the expanded head crop) plays the thinking v9 loop
  // (companion/spirit/thinking, 8 frames @ 140 ms, fail soft to rest),
  // then snaps back to the blessed v5 rest idle when the reply lands.
  // V9 wait-in-slot: the bare Orbitty mark (no bubble, no fill, no
  // border) lives IN the agent-text slot — the last left-aligned
  // transcript row expanded, the bot CC slot collapsed — right where
  // the reply will land. Never floating under the stage / head.
  // Speak toggle (off by default, persisted
  // companion.speakReplies) reads bot replies aloud via macOS `say`
  // with the default system voice; a new reply or toggling off kills
  // the in-flight `say`. No wake word, no second TTS stack.
  // GO-ORBITTY-COMPANION-HEADROOM: the 128px .head crop is dead. Each
  // surface has its own persisted view — companion.viewCollapsed
  // (default "full") and companion.viewExpanded (default "head") —
  // written by the one Full/Head control near Expand, current surface
  // only. Head draws the body's bust_* set (head+neck+collar, hard
  // alpha, nearest-neighbor) on the same tick as idle/think, with a
  // tiny float and a small yaw/roll on send that settles; Reduce
  // Motion stills both. A body with no bust art sits on Full with
  // Head disabled — never a crop fallback, never a neck shear.
  // GO-ORBITTY-CAPTION-BUBBLE: collapsed CC. Speak off = small scroll
  // (no paging). Speak on = fitPages to that small box so say can flip.
  // Click the bot text = transient in-window full overlay. Enter/Space
  // page leftover only while Speak on. User bubbles never page.
  // GO-ORBITTY-GHOST-DOCK: ghost (idle collapsed, not hosted) gets a
  // tight spirit + a bottom dock. .ghost .stage .floatwrap shrinks to a
  // centered square (shorter of stage w/h) so the reported hit box hugs
  // the figure and empty air clicks through; agentrow + composer ride in
  // one .ghostdock slab (ghost-only paint: --bg-fill, radius 12, 1px
  // --edge). Expanded / hosted keep the full-window slab — no
  // click-through there, no dock paint.
  // GO-ORBITTY-DOCK-GRIP: the ghost dock's HIT is flush — dock top to
  // window bottom, full window width; the paint stays inset — so the
  // borderless frame's edge resize arrows land on the companion, and a
  // mousedown on dock chrome (not a control) drags like the spirit.
  // GO-ORBITTY-SPRITE-OFF: the Full/Head button hides the sprite
  // (persisted companion.sprite, default true — one flag for collapsed,
  // expanded, hosted) while chat stays. Off draws no stage / exphead /
  // wellhead: the ghost reports no floatwrap hit so empty air above the
  // dock clicks through, and the hosted well pane takes the full width.
  // GO-ORBITTY-DOCK-TWEAK: L-click hides/shows the sprite; R-click is
  // Full ↔ Head. Per-surface view prefs survive Off.
  // GO-ORBITTY-SPRITE-HUG: while !sprite the Full/Head glyph wears the
  // Speak-pattern slash, and idle collapsed + off hugs the window to the
  // measured CC+dock bar — the NATIVE side resizes (never a frontend
  // set_size), keeping the bottom edge so the dock doesn't jump. The
  // tall idle rect is stashed apart from HOST_STASH and is the only
  // rect ever persisted; sprite on / expand / host restore it first.
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { emitTo, listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy, tick } from "svelte";
  import {
    applyHostTokens,
    applyWindowOpacity,
    clampWindowOpacity,
  } from "../themes/apply";
  import { listenPaneVis } from "./paneVis";
  import KanbanPage from "./KanbanPage.svelte";
  import StarsPage from "./StarsPage.svelte";
  import RailPage from "./RailPage.svelte";
  import SpiritStage from "./SpiritStage.svelte";
  import CaptionPane from "./CaptionPane.svelte";

  import type {
    CompanionSlice,
    CompanionTurn,
    KanbanCard,
    PersistedState,
    StarsSlice,
  } from "./persist";
  interface CompanionSheet {
    dataUrl: string;
    frameW: number;
    frameH: number;
    count: number;
  }
  interface CompanionArt {
    frames: string[];
    thinkingFrames: string[];
    bustFrames?: string[];
    bustThinkingFrames?: string[];
    sheet: CompanionSheet | null;
    frameMs: number;
    smooth?: boolean;
  }
  interface CompanionAgents {
    agents: string[];
    selected: string;
  }

  // GO-ORBITTY-FOURCAST-RECAST: chrome labels only. The option value stays
  // the stem id so set_companion_agent and the on-disk allowlist are
  // untouched; an unknown stem fails open to the raw id.
  const AGENT_LABELS: Record<string, string> = {
    spark: "Spark",
    grok: "Work",
    "chaos-gremlin": "Chat",
    "fkt-homie": "Hold",
  };
  const agentLabel = (id: string) => AGENT_LABELS[id] ?? id;

  // GO-ORBITTY-COMPANION-BRAND: the seven palette skins are retired; a
  // dead saved id falls back to the OS pair. Terminal stays valid.
  const KNOWN_THEMES = new Set(["systemLight", "systemDark", "terminal"]);

  function osThemeId(): string {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "systemDark"
      : "systemLight";
  }

  // GO-ORBITTY-PANE-PAINT: seed the OS theme synchronously so --bg exists
  // before load_state lands — an empty themeId on an opaque drawer is a
  // white-on-white first paint. loadState overrides with the saved id.
  let themeId = $state(osThemeId());
  let windowOpacity = $state(1);
  // Rest = blessed v5 idle; think = v9 thinking swap. Both 8 @ 140 ms.
  // The active list is thinking only while a send is in flight.
  let restFrames = $state<string[]>([]);
  let thinkFrames = $state<string[]>([]);
  // HEADROOM: the body's bust set (head+neck+collar) — rest + thinking
  // swap, same cadence. Empty = this body has no Head art.
  let restBust = $state<string[]>([]);
  let thinkBust = $state<string[]>([]);
  let frameIdx = $state(0);
  let missing = $state(false);
  let frameMs = 140;
  let artSmooth = $state(false);
  // false = art never loaded (so onMount doesn't double-load).
  let artLoaded = false;
  let timer: ReturnType<typeof setInterval> | undefined;

  // S2.4 CC + composer + agent + expand. True CC: one latest user line,
  // one latest bot line — the transcript lives behind Expand. sendNote is
  // errors only; a good send IS the bot line, no banner.
  let userLine = $state("");
  let botLine = $state("");
  let draft = $state("");
  let sending = $state(false);
  let sendNote = $state<string | null>(null);
  // Thinking v9 plays only while a reply is in flight; missing thinking
  // art fails soft to rest. The full figure and the Head bust each draw
  // their own set on this same tick.
  const activeFullFrames = $derived(
    sending && thinkFrames.length > 0 ? thinkFrames : restFrames,
  );
  const activeBustFrames = $derived(
    sending && thinkBust.length > 0 ? thinkBust : restBust,
  );
  let agents = $state<string[]>(["default"]);
  let agent = $state("default");
  // The Body picker lives in Settings → Appearance (Companion body); this
  // window only follows the persisted pack (state:changed → loadState).
  let bodyId = $state("spirit");

  let expanded = $state(false);
  let turns = $state<CompanionTurn[]>([]);

  // GO-ORBITTY-HOST-SPLIT: the companion window is the host. Session-only
  // — boot is always the idle spirit and nothing persists `hosted`.
  // "launchpad" = KanbanPage embedded in the well (pop-out allowed);
  // "stars" = StarsPage embedded (no pop-out, ever);
  // "inspector" = RailPage embedded (pop-out to the rail window).
  type HostMode = "off" | "launchpad" | "stars" | "inspector";
  let hosted = $state<HostMode>("off");

  let mainEl: HTMLElement | null = null;
  let captionReady = $state(false);
  function onCaptionSpeakError(msg: string) {
    sendNote = msg;
  }

  // HOST-CC: hosted + expanded — Escape collapses the overlay only.
  // CaptionPane owns ccOpen Escape and Enter/Space paging.
  function onPaneKey(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    if (hosted !== "off" && expanded) {
      expanded = false;
      invoke("set_companion_expanded", { expanded: false }).catch(() => {});
      e.preventDefault();
    }
  }

  // HEADROOM: per-surface view prefs. Collapsed defaults Full, expanded
  // defaults Head; the Full/Head control writes the current surface only.
  let viewCollapsed = $state("full");
  let viewExpanded = $state("head");
  let viewBusy = $state(false);
  const viewPref = $derived(expanded ? viewExpanded : viewCollapsed);
  // Head shows only when the body actually has bust art — a missing bust
  // is a quiet miss on Full (Head disabled), NEVER the 128px crop.
  const bustOk = $derived(restBust.length > 0);
  // HOST-SPLIT: the hosted right column follows Full/Head like idle —
  // collapsed reads viewCollapsed, expanded reads viewExpanded (viewPref
  // already splits the surfaces). No hosted force, never a pref write.
  const headShowing = $derived(
    viewPref === "head" && bustOk && !missing,
  );
  const drawFrames = $derived(headShowing ? activeBustFrames : activeFullFrames);

  async function toggleView() {
    if (viewBusy || !bustOk) return;
    const surface = expanded ? "expanded" : "collapsed";
    const prev = viewPref;
    const next = prev === "head" ? "full" : "head";
    if (expanded) viewExpanded = next;
    else viewCollapsed = next;
    viewBusy = true;
    try {
      const written = await invoke<string>("set_companion_view", {
        surface,
        view: next,
      });
      if (expanded) viewExpanded = written;
      else viewCollapsed = written;
    } catch (e) {
      if (expanded) viewExpanded = prev;
      else viewCollapsed = prev;
      sendNote = `${e}`;
    } finally {
      viewBusy = false;
    }
  }

  // SPRITE-OFF: one persisted flag (companion.sprite, default true) for
  // all three surfaces. DOCK-TWEAK: L-click the Full/Head button toggles
  // it (R-click is Full ↔ Head, preventDefault — no browser menu). The
  // per-surface view prefs are never written here, so they survive Off.
  // Off draws no stage / exphead / wellhead — chat stays.
  let sprite = $state(true);

  // SPRITE-HUG: the bar height the native hug resizes to — CC rows (+
  // note / starstrip when present) down through the dock, bottom margin
  // included. Diffed from element edges, so it measures the same whether
  // the content is bottom-pinned by the live stage or already hugged.
  function measureHugHeight(): number | null {
    if (!mainEl) return null;
    const dock = mainEl.querySelector(".ghostdock");
    if (!dock) return null;
    const dockR = dock.getBoundingClientRect();
    let top = dockR.top;
    mainEl.querySelectorAll(".ccrow, .note, .starstrip").forEach((el) => {
      const r = el.getBoundingClientRect();
      if (r.height >= 2 && r.top < top) top = r.top;
    });
    const h = Math.round(dockR.bottom - top + 8);
    return h < 120 ? 120 : h;
  }

  // SPRITE-HUG: idle collapsed + sprite off asks the native side to hug
  // (or re-fit) the bar. Flag-unchanged calls skip the persist/emit
  // native-side, so this is cheap on every collapsed layout change; the
  // native side no-ops while expanded/hosted/hidden or already fit.
  function reportHug() {
    if (expanded || hosted !== "off" || sprite) return;
    const h = measureHugHeight();
    if (h) invoke("set_companion_sprite", { enabled: false, hugHeight: h }).catch(() => {});
  }

  async function toggleSprite(e: MouseEvent) {
    e.preventDefault();
    const next = !sprite;
    sprite = next;
    try {
      // SPRITE-HUG: pass the measured bar height on the off flip so the
      // native hug lands with the flag write; on never needs a height.
      const hugHeight =
        next || expanded || hosted !== "off" ? null : measureHugHeight();
      await invoke("set_companion_sprite", { enabled: next, hugHeight });
    } catch (err) {
      sprite = !next;
      sendNote = `${err}`;
    }
  }

  // DOCK-TWEAK: L-click hide/show sprite; R-click Full ↔ Head.
  const viewTitle = $derived(
    !sprite
      ? "Sprite hidden — chat stays. Click: show sprite · Right-click: Full or Head"
      : (bustOk
          ? "Click: hide sprite · Right-click: Full figure or Head bust — remembered per surface"
          : "Click: hide sprite · This body has no bust art — Head needs bust_* frames"),
  );

  // S2.7: Speak toggle (persisted companion.speakReplies, default off).
  // When on, each landed bot reply is read aloud by the native
  // `speak_reply` (macOS `say`, default system voice); the native side
  // kills any in-flight `say` before the next and when the toggle goes
  // off. Never the user's line, never an error line.
  let speak = $state(false);

  async function toggleSpeak() {
    const next = !speak;
    speak = next;
    try {
      await invoke("set_companion_speak", { enabled: next });
    } catch (e) {
      speak = !next;
      sendNote = `${e}`;
    }
  }

  // ---------- GO-LS-STARS-STATUS: prompts strip + hold-to-talk ----------

  const STARS_SPOKES = [
    { key: "sleep", label: "Sleep" },
    { key: "terroir", label: "Terroir" },
    { key: "athleticism", label: "Athleticism" },
    { key: "relationships", label: "Relationships" },
    { key: "self", label: "Self" },
  ] as const;

  function todayKey(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }

  let stars = $state<StarsSlice>({ promptsOptIn: false, checkins: {} });
  let cards = $state<KanbanCard[]>([]);
  // Prompts opt-in (default Off): mirrored with the rail's STARS pane —
  // both write the same key, this window via set_stars_prompts.
  let prompts = $state(false);

  const todayCheckin = $derived(stars.checkins[todayKey()]);
  // Thinnest spoke in words; ties break toward Sleep (STARS order).
  const lowestSpoke = $derived.by(() => {
    const c = todayCheckin;
    if (!c) return null;
    let best: (typeof STARS_SPOKES)[number] = STARS_SPOKES[0];
    for (const s of STARS_SPOKES) {
      if ((c[s.key] ?? 0) < (c[best.key] ?? 0)) best = s;
    }
    return best;
  });
  const promptSig = $derived(
    todayCheckin
      ? STARS_SPOKES.map((s) => todayCheckin[s.key] ?? 0).join(".")
      : "",
  );
  const promptAckKey = $derived(promptSig ? `${todayKey()}:${promptSig}` : "");
  // One strip per day unless they check in again (a new sig re-arms it).
  const showPromptStrip = $derived(
    prompts &&
      !!todayCheckin &&
      promptAckKey !== "" &&
      (stars.promptsAck ?? "") !== promptAckKey,
  );

  async function togglePrompts() {
    const next = !prompts;
    prompts = next;
    try {
      await invoke("set_stars_prompts", { enabled: next });
    } catch (e) {
      prompts = !next;
      sendNote = `${e}`;
    }
  }

  // Rest / Water / × — an ack, not a lecture: hide until the day rolls or
  // a fresh check-in lands.
  async function ackStrip() {
    if (!promptAckKey) return;
    const ack = promptAckKey;
    stars = { ...stars, promptsAck: ack };
    try {
      await invoke("ack_stars_prompt", { ack });
    } catch (e) {
      sendNote = `${e}`;
    }
  }

  // Smallest todo: first undone subtask on the first Doing card, else that
  // card's title, else an honest "nothing open". Opens Lists on the card.
  function smallestTodo(): { cardId: string; text: string } | null {
    const card = cards.find((c) => c.status === "doing");
    if (!card) return null;
    const sub = card.subtasks?.find((s) => !s.done);
    return { cardId: card.id, text: sub?.title || card.title };
  }

  async function openSmallestTodo() {
    const t = smallestTodo();
    await ackStrip();
    if (!t) {
      sendNote = "Nothing open on the Board.";
      return;
    }
    try {
      // HOST-SPLIT: the card opens in the embedded Launchpad well — never
      // the overlay on the spirit. The focus event targets THIS window
      // (the embedded KanbanPage listens here); the re-emit covers a well
      // still mid-mount when the first event lands.
      await enterHost("launchpad");
      emitTo("companion", "orbitty:focus-card", t.cardId).catch(() => {});
      setTimeout(
        () =>
          emitTo("companion", "orbitty:focus-card", t.cardId).catch(() => {}),
        400,
      );
    } catch (e) {
      sendNote = `${e}`;
    }
  }

  // Talk: Grok-style press to start / press to stop.
  // Default = Speech.framework (talk:partial / talk:ended fill the field).
  // Hotkey = Handy Ctrl+Space chord. Never webkitSpeechRecognition.
  // Never auto-send, never always-on, never a wake word.
  let talking = $state(false);
  let talkBase = "";
  let fieldEl: HTMLInputElement | null = null;
  // Up-arrow path: send as soon as the final text lands. System speech
  // emits talk:ended synchronously inside talk_stop; Handy never emits
  // it, so a short fallback sends whatever is already in the field.
  let sendOnTalkEnd = false;

  async function talkStop() {
    talking = false;
    fieldEl?.focus();
    try {
      await invoke("talk_stop");
    } catch {
      /* release is best-effort — never leave the chord stuck */
    }
    fieldEl?.focus();
  }

  async function talkStart(e: Event) {
    e.preventDefault();
    if (talking) return;
    fieldEl?.focus();
    try {
      talkBase = draft.trimEnd();
      await invoke("talk_start");
      talking = true;
      sendNote = null;
      fieldEl?.focus();
    } catch (err) {
      talking = false;
      sendNote = `${err}`;
    }
  }

  // Check: stop only — the text stays in the field for review + Send.
  async function talkStopOnly() {
    sendOnTalkEnd = false;
    await talkStop();
  }

  // Up-arrow: stop, then send immediately — no second confirm.
  async function talkStopSend() {
    if (!talking) return;
    sendOnTalkEnd = true;
    await talkStop();
    await new Promise((r) => setTimeout(r, 80));
    if (sendOnTalkEnd) {
      sendOnTalkEnd = false;
      await sendLine();
    }
  }

  function applyTalkText(text: string) {
    const spoken = (text ?? "").trim();
    if (!spoken) {
      draft = talkBase;
    } else if (!talkBase) {
      draft = spoken;
    } else {
      draft = `${talkBase} ${spoken}`;
    }
    fieldEl?.focus();
  }

  async function sendLine() {
    const line = draft.trim();
    if (!line || sending) return;
    sending = true;
    sendNote = null;
    // S2.5 optimistic you: the bubble lands NOW, the field clears, and
    // the scroll effect (watching turns.length + sending) pins the
    // expanded transcript to the bottom; `sending` swaps the spirit to
    // the thinking v9 loop until the reply lands — the wait mark sits
    // bare in the agent-text slot (no bubble, no border) where the
    // reply will land; spirit still plays thinking v9.
    turns = [...turns, { role: "you", text: line }].slice(-40);
    userLine = line;
    draft = "";
    try {
      const reply = await invoke<string>("companion_send", { line });
      botLine = reply;
      // CAPTION-CHUNK: speaking is the arrival effect's job — fill the
      // live CC box, then read page one.
      // companion_send persists the exchange and emits state:changed;
      // if that loadState won the race, this exact exchange is already
      // the tail — append the reply only when it isn't.
      const n = turns.length;
      const landed =
        n >= 2 &&
        turns[n - 1].role === "them" &&
        turns[n - 1].text === reply &&
        turns[n - 2].role === "you" &&
        turns[n - 2].text === line;
      if (!landed) {
        turns = [...turns, { role: "them", text: reply }].slice(-40);
      }
    } catch (e) {
      // The you bubble stays (it was appended up front); the draft is
      // already gone — one honest error line is all that remains.
      sendNote = `${e}`;
    } finally {
      sending = false;
    }
  }

  async function loadAgents() {
    try {
      const a = await invoke<CompanionAgents>("companion_agents");
      agents = a.agents.length > 0 ? a.agents : ["default"];
      agent = a.selected;
    } catch {
      agents = ["default"];
      agent = "default";
    }
  }

  async function onAgentChange() {
    try {
      agent = await invoke<string>("set_companion_agent", { profile: agent });
    } catch (e) {
      sendNote = `${e}`;
      await loadAgents(); // snap back to the persisted pick
    }
  }



  function toggleExpanded() {
    expanded = !expanded;
    invoke("set_companion_expanded", { expanded }).catch(() => {});
    // HOST-CC (overrides HOST-SPLIT): expanding while hosted KEEPS the
    // well — the transcript overlays it dimmed. Never dismissHost here;
    // the native host frame (1080×620) stays up.
  }

  // ---------- GO-ORBITTY-HOST-SPLIT: the hosted well ----------
  // One host window = this companion. The native set_companion_host owns
  // the frame (stash the idle rect → 1080×620 on enter, restore on off);
  // this JS state only owns WHICH component sits in the well.
  // GO-ORBITTY-HOST-CC: the expanded transcript OVERLAYS the well
  // (absolute over .wellwrap, dimmed scrim, opaque bubbles) — the well
  // stays mounted, the host frame stays up; collapse = second Expand or
  // Escape.

  async function enterHost(mode: "launchpad" | "stars" | "inspector") {
    if (hosted !== "off") {
      // Launchpad ↔ STARS swap: flip the well AND still invoke
      // set_companion_host — rust re-asserts HOSTED / the slab
      // (host_changed only: no second stash, no re-place, no re-grow).
      const prev = hosted;
      hosted = mode;
      try {
        await invoke("set_companion_host", { mode });
      } catch (e) {
        hosted = prev;
        sendNote = `${e}`;
      }
      return;
    }
    hosted = mode;
    try {
      // COMPANION-RAISE: native host-on lands BEFORE the collapse —
      // HOSTED=true (slab, poll idle) must be set before
      // expanded_changed(false) can arm the ghost poll.
      await invoke("set_companion_host", { mode });
    } catch (e) {
      hosted = "off";
      sendNote = `${e}`;
      return;
    }
    // Force collapsed on enter (the existing persisted expanded flag).
    if (expanded) {
      expanded = false;
      invoke("set_companion_expanded", { expanded: false }).catch(() => {});
    }
  }

  async function dismissHost() {
    if (hosted === "off") return;
    hosted = "off";
    try {
      await invoke("set_companion_host", { mode: "off" });
    } catch (e) {
      sendNote = `${e}`;
    }
  }

  // Dest strip: Launchpad/STARS/Planets toggle the well (click again
  // dismisses — the spirit gets the full stage back). A visible kanban
  // overlay is absorbed: the native enter hides it. Never invoke
  // open_kanban / open_stars / show_rail here — those are the overlay /
  // rail window, not the well.
  async function toggleDest(mode: "launchpad" | "stars" | "inspector") {
    if (hosted === mode) await dismissHost();
    else await enterHost(mode);
  }

  // Launchpad-only pop-out (the embedded banner button): the real kanban
  // window becomes the popped Launchpad beside the restored idle
  // companion. Never parks — the two sit side by side.
  async function popoutLaunchpad() {
    if (hosted !== "launchpad") return;
    hosted = "off";
    try {
      await invoke("set_companion_host", { mode: "off" });
      await invoke("open_kanban");
    } catch (e) {
      sendNote = `${e}`;
    }
  }

  // GO-ORBITTY-INSPECTOR-HOST: same pop-out pattern — dismiss the well,
  // then show the real rail window. Tray Inspector still show_rail too.
  async function popoutInspector() {
    if (hosted !== "inspector") return;
    hosted = "off";
    try {
      await invoke("set_companion_host", { mode: "off" });
      await invoke("show_rail");
    } catch (e) {
      sendNote = `${e}`;
    }
  }

  function restartTimer() {
    if (timer) clearInterval(timer);
    // Reduced motion: hold frame 0 of the active set (rest or thinking).
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    timer = setInterval(() => {
      const n = drawFrames.length;
      if (n > 1) frameIdx = (frameIdx + 1) % n;
    }, frameMs);
  }

  // Sheet → frames: 1:1 canvas blits (no resample, no redraw).
  async function sliceSheet(sheet: CompanionSheet): Promise<string[]> {
    const img = new Image();
    await new Promise<void>((resolve, reject) => {
      img.onload = () => resolve();
      img.onerror = () => reject(new Error("sheet decode failed"));
      img.src = sheet.dataUrl;
    });
    const c = document.createElement("canvas");
    c.width = sheet.frameW;
    c.height = sheet.frameH;
    const ctx = c.getContext("2d");
    if (!ctx) return [];
    ctx.imageSmoothingEnabled = false;
    const out: string[] = [];
    for (let i = 0; i < sheet.count; i++) {
      ctx.clearRect(0, 0, c.width, c.height);
      ctx.drawImage(
        img,
        i * sheet.frameW,
        0,
        sheet.frameW,
        sheet.frameH,
        0,
        0,
        sheet.frameW,
        sheet.frameH,
      );
      out.push(c.toDataURL("image/png"));
    }
    return out;
  }

  // Spirit when imagePackId is empty; else a role:companion pack.
  // Missing pack fails soft to the blessed spirit (never a blank window).
  async function loadArt() {
    const packId = bodyId === "spirit" ? null : bodyId;
    try {
      const art = await invoke<CompanionArt>("companion_frames", {
        packId,
      });
      let f = art.frames ?? [];
      if (f.length === 0 && art.sheet) f = await sliceSheet(art.sheet);
      if (f.length === 0) throw new Error("no frames");
      frameMs = art.frameMs > 0 ? art.frameMs : 140;
      artSmooth = !!art.smooth;
      frameIdx = 0;
      restFrames = f;
      thinkFrames = art.thinkingFrames ?? [];
      restBust = art.bustFrames ?? [];
      thinkBust = art.bustThinkingFrames ?? [];
      missing = false;
      restartTimer();
    } catch {
      if (packId) {
        bodyId = "spirit";
        try {
          const art = await invoke<CompanionArt>("companion_frames", {
            packId: null,
          });
          let f = art.frames ?? [];
          if (f.length === 0 && art.sheet) f = await sliceSheet(art.sheet);
          if (f.length === 0) throw new Error("no frames");
          frameMs = art.frameMs > 0 ? art.frameMs : 140;
          artSmooth = false;
          frameIdx = 0;
          restFrames = f;
          thinkFrames = art.thinkingFrames ?? [];
          restBust = art.bustFrames ?? [];
          thinkBust = art.bustThinkingFrames ?? [];
          missing = false;
          restartTimer();
          artLoaded = true;
          return;
        } catch {
          /* fall through to missing */
        }
      }
      restFrames = [];
      thinkFrames = [];
      restBust = [];
      thinkBust = [];
      missing = true;
    }
    artLoaded = true;
  }

  async function loadState() {
    try {
      const s = await invoke<PersistedState>("load_state");
      themeId = KNOWN_THEMES.has(s.themeId) ? s.themeId : osThemeId();
      windowOpacity = clampWindowOpacity(s.windowOpacity ?? 1);
      userLine = s.companion?.lastUserLine ?? "";
      botLine = s.companion?.lastBotLine ?? "";
      turns = s.companion?.turns ?? [];
      expanded = s.companion?.expanded ?? false;
      speak = s.companion?.speakReplies ?? false;
      // HEADROOM: missing keys land on the defaults (collapsed Full,
      // expanded Head); unknown values normalize the same way.
      viewCollapsed = s.companion?.viewCollapsed === "head" ? "head" : "full";
      viewExpanded = s.companion?.viewExpanded === "full" ? "full" : "head";
      // SPRITE-OFF: missing key (old saves) = true.
      sprite = s.companion?.sprite ?? true;
      const pack = s.companion?.imagePackId ?? null;
      const nextBody = pack && pack.length > 0 ? pack : "spirit";
      if (nextBody !== bodyId) {
        bodyId = nextBody;
        artLoaded = false;
        void loadArt();
      }
      stars = s.stars ?? { promptsOptIn: false, checkins: {} };
      prompts = stars.promptsOptIn ?? false;
      cards = s.kanbanCards ?? [];
    } catch {
      // keep last good state; next state:changed retries
    } finally {
      captionReady = true;
    }
  }

  // GRAB-LAW: collapsed, the spirit RECTANGLE is the drag handle — the
  // tight ghost .floatwrap (full body or Head bust, still a box, never
  // PNG alpha), never the whole .stage. Expanded, the corner portrait
  // still grips itself. The × is outside both and stops its own
  // propagation. Drawers are borderless — no native Overlay strip — so
  // this JS drag is the one source, including the top of the pane.
  function onDragMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    getCurrentWindow()
      .startDragging()
      .catch(() => {});
  }

  // DOCK-GRIP + GRAB-LAW: dock chrome (padding around dests / agent
  // pick / composer, never a control) drags on EVERY surface — idle,
  // hugged, expanded, hosted. Controls keep their own presses.
  function onDockMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    const t = e.target as HTMLElement | null;
    if (t?.closest("input, textarea, select, button, .field")) return;
    e.preventDefault();
    getCurrentWindow()
      .startDragging()
      .catch(() => {});
  }

  // FRONT-HOLD: a mousedown on any ghost hit (dock / spirit / bubble /
  // × ⚙ / strips / popups) heavy-raises the companion natively, so no
  // planet restack can bury it mid-click. Capture phase on <main>: the
  // × ⚙ buttons stopPropagation on bubble, capture still sees them.
  // Ghost only — the slab is a complete window with no bury risk.
  function onHitMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    if (expanded || hosted !== "off") return;
    const t = e.target as HTMLElement | null;
    if (
      !t?.closest(
        ".floatwrap, .ghostdock, .cc, .wait, .starstrip, .winx, .gear, .note"
      )
    )
      return;
    invoke("companion_front").catch(() => {});
  }

  // GRAB-LAW: the slab (expanded / hosted) is a complete window — the
  // pane drags from the well, transcript padding, chrome, spirit, empty
  // air. Never from a control (own presses), never from .bubble / .cc
  // (selection wins), never from a draggable kanban card. The dock,
  // floatwrap, and embedded banners grip themselves — the pane defers.
  function onPaneMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    if (!expanded && hosted === "off") return;
    const t = e.target as HTMLElement | null;
    if (t?.closest("input, textarea, select, button, a, .field")) return;
    if (t?.closest(".bubble, .cc")) return;
    if (t?.closest('[draggable="true"]')) return;
    if (t?.closest(".ghostdock, .floatwrap, .k-banner, .s-banner, .banner")) return;
    e.preventDefault();
    getCurrentWindow()
      .startDragging()
      .catch(() => {});
  }

  // ---------- GO-ORBITTY-COMPANION-CLEAR: ghost hit reporting ----------
  // Ghost = shown, collapsed, not hosted: empty pixels click through to
  // the desk, so the native 10Hz poll needs the coarse boxes that still
  // hit — window-local CSS points via getBoundingClientRect.
  // GO-ORBITTY-GHOST-DOCK: the spirit box is the tight ghost floatwrap —
  // a centered square (shorter of stage w/h), never the 100%×100% wrap
  // and never the empty .stage — and agentrow + composer report as ONE
  // .ghostdock slab. Slab (expanded/hosted) never consults the list
  // native-side; an empty report is a no-op there (kept last hits =
  // no hole on collapse), so leaving ghost just sends [].
  // GO-ORBITTY-DOCK-GRIP: the dock slab reports FLUSH, not the painted
  // inset — x=0, full window width, dock top → window bottom — so the
  // borderless frame's bottom / side-at-dock / corner resize arrows land
  // on a hit. Empty air above the dock still click-throughs.
  function reportHits() {
    if (!mainEl) return;
    const rects: { x: number; y: number; w: number; h: number }[] = [];
    const push = (el: Element | null) => {
      if (!el) return;
      const r = el.getBoundingClientRect();
      if (r.width < 2 || r.height < 2) return;
      rects.push({ x: r.x, y: r.y, w: r.width, h: r.height });
    };
    push(mainEl.querySelector(".stage .floatwrap"));
    mainEl.querySelectorAll(".cc").forEach((el) => push(el));
    push(mainEl.querySelector(".ccrow .wait"));
    push(mainEl.querySelector(".starstrip"));
    // DOCK-GRIP: flush dock hit — paint stays inset, hits are not paint.
    const dock = mainEl.querySelector(".ghostdock");
    if (dock) {
      const dr = dock.getBoundingClientRect();
      if (dr.width >= 2 && dr.height >= 2) {
        rects.push({
          x: 0,
          y: dr.y,
          w: window.innerWidth,
          h: window.innerHeight - dr.y,
        });
      }
    }
    push(mainEl.querySelector(".winx"));
    push(mainEl.querySelector(".gear"));
    push(mainEl.querySelector(".note"));
    invoke("set_companion_hits", { rects }).catch(() => {});
  }

  // Report on every collapsed-layout change (CC rows, wait mark, note,
  // starstrip, planets popup, art/view swap, sprite off). tick() first
  // so the DOM has the new layout before the measure.
  $effect(() => {
    const watching = [
      expanded,
      hosted,
      sprite,
      userLine,
      botLine,
      sending,
      sendNote,
      showPromptStrip,
      missing,
      drawFrames,
    ];
    void watching;
    if (expanded || hosted !== "off") {
      invoke("set_companion_hits", { rects: [] }).catch(() => {});
      return;
    }
    // SPRITE-HUG: reportHug rides the same post-tick measure — the bar
    // re-fits as CC rows / wait mark / strips come and go.
    tick().then(() => {
      reportHits();
      reportHug();
    });
  });

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

  // Rest ↔ thinking swaps restart the cycle: frame 0 of the thinking set
  // when a send leaves, frame 0 of rest when the reply lands.
  $effect(() => {
    void sending;
    frameIdx = 0;
  });

  let unlistenState: (() => void) | undefined;
  let unlistenVis: (() => void) | undefined;
  let unlistenSummon: (() => void) | undefined;
  let unlistenEnterHost: (() => void) | undefined;
  let unlistenTalk: (() => void)[] = [];
  let hitsObserver: ResizeObserver | undefined;
  onMount(async () => {
    invoke("ensure_glass").catch(() => {});
    // COMPANION-CLEAR: a window resize moves every hit box — re-report.
    if (mainEl && typeof ResizeObserver !== "undefined") {
      hitsObserver = new ResizeObserver(() => {
        if (!expanded && hosted === "off") reportHits();
      });
      hitsObserver.observe(mainEl);
    }
    await loadState();
    await loadAgents();
    if (!artLoaded) await loadArt();
    window.addEventListener("keydown", onPaneKey);
    unlistenState = await listen("state:changed", loadState);
    // HITCH-DRAW: hide stops the 140ms tick (the window keeps warm, so
    // document.hidden never flips); show resumes it. frameIdx is untouched
    // on hide — the blink resumes from the last frame, and the sheets are
    // never reloaded on a hide/show cycle. Reduce Motion still holds
    // frame 0 (restartTimer is a no-op there).
    unlistenVis = await listenPaneVis("companion", (visible) => {
      if (visible) {
        restartTimer();
        // HOST-SPLIT: a hide while hosted restored the idle rect
        // native-side; the session flag still owns the well, so re-assert
        // the host frame on show.
        if (hosted !== "off") {
          invoke("set_companion_host", { mode: hosted }).catch(() => {});
        }
        // SPRITE-HUG: a show while sprite-off (the hide persisted the
        // tall idle) re-hugs the bar — no state change fires the effect.
        tick().then(reportHug);
      } else if (timer) {
        clearInterval(timer);
        timer = undefined;
      }
    });
    // GO-ORBITTY-SUMMON: ⌘⇧O raises this pane native-side, but webview
    // focus alone never lands the caret in .field — the summon event does.
    unlistenSummon = await listen("orbitty:companion-summon", () => {
      fieldEl?.focus();
    });
    // GO-ORBITTY-TRAY-REMOTE: tray Launchpad/STARS raise this pane
    // native-side and ask the well to open — raise-only via enterHost,
    // never toggleDest (the dest buttons still own toggling).
    unlistenEnterHost = await listen<string>("orbitty:enter-host", (e) => {
      const m = e.payload;
      if (m === "launchpad" || m === "stars" || m === "inspector") void enterHost(m);
    });
    unlistenTalk.push(
      await listen<string>("talk:partial", (e) => {
        applyTalkText(e.payload ?? "");
      }),
    );
    unlistenTalk.push(
      await listen<string>("talk:ended", (e) => {
        applyTalkText(e.payload ?? "");
        talking = false;
        fieldEl?.focus();
        if (sendOnTalkEnd) {
          sendOnTalkEnd = false;
          void sendLine();
        }
      }),
    );
    unlistenTalk.push(
      await listen<string>("talk:error", (e) => {
        talking = false;
        sendOnTalkEnd = false;
        sendNote = e.payload || "System speech failed — type instead.";
      }),
    );
  });

  onDestroy(() => {
    if (timer) clearInterval(timer);
    hitsObserver?.disconnect();
    window.removeEventListener("keydown", onPaneKey);
    unlistenState?.();
    unlistenVis?.();
    unlistenSummon?.();
    unlistenEnterHost?.();
    for (const u of unlistenTalk) u();
    invoke("talk_stop").catch(() => {});
  });
</script>

{#snippet caption(expanded: boolean, hosted: boolean)}
  <CaptionPane
    {expanded}
    {hosted}
    {userLine}
    {botLine}
    {sending}
    {turns}
    {speak}
    {draft}
    ready={captionReady}
    onSpeakError={onCaptionSpeakError}
  />
{/snippet}

<main
  class="companion"
  class:hosting={hosted !== "off"}
  class:ghost={!expanded && hosted === "off"}
  class:spriteoff={!sprite}
  onmousedown={onPaneMouseDown}
  onmousedowncapture={onHitMouseDown}
  bind:this={mainEl}
>
  {#if expanded && sprite}
    <SpiritStage
      corner
      {headShowing}
      {missing}
      frames={drawFrames}
      {frameIdx}
      thinking={sending}
      smooth={artSmooth}
      onDrag={onDragMouseDown}
    />
  {/if}

  {#if hosted !== "off"}
    <!-- HOST-SPLIT + HOST-CC: the well — left ~2/3 is the embedded
         Launchpad / STARS / Inspector, right 1/3 is the Head spirit
         stage. The well stays MOUNTED while expanded — the transcript
         overlays it inside .wellwrap. The two capped CC lines +
         agentrow + composer stay full-width below the well. -->
    <div class="wellwrap">
      <div class="hostwell">
        <!-- SPRITE-OFF: no .wellhead when the sprite is hidden — the
             pane takes the full well (no right border). -->
        <div class="wellpane" class:fullwell={!sprite}>
          {#if hosted === "launchpad"}
            <KanbanPage embedded onClose={dismissHost} onPopout={popoutLaunchpad} />
          {:else if hosted === "stars"}
            <StarsPage embedded onClose={dismissHost} />
          {:else if hosted === "inspector"}
            <RailPage embedded onClose={dismissHost} onPopout={popoutInspector} />
          {/if}
        </div>
        {#if sprite}
          <div class="wellhead">
            {#if !expanded}
              <SpiritStage
                {headShowing}
                {missing}
                frames={drawFrames}
                {frameIdx}
                thinking={sending}
                smooth={artSmooth}
                onDrag={onDragMouseDown}
              />
            {/if}
          </div>
        {/if}
      </div>
      {#if expanded}
        {@render caption(true, true)}
      {/if}
    </div>
  {:else if expanded}
    {@render caption(true, false)}
  {:else if sprite}
    <SpiritStage
      {headShowing}
      {missing}
      frames={drawFrames}
      {frameIdx}
      thinking={sending}
      smooth={artSmooth}
      onDrag={onDragMouseDown}
    />
  {/if}

  {#if !expanded}
    {@render caption(false, hosted !== "off")}
  {/if}

  {#if sendNote}
    <div class="note err">
      <span class="note-text">{sendNote}</span>
      <button
        class="note-x"
        onclick={() => (sendNote = null)}
        aria-label="Dismiss">×</button
      >
    </div>
  {/if}

  {#if showPromptStrip && lowestSpoke}
    <div class="starstrip">
      <span class="starstrip-text">{lowestSpoke.label} is the thin spoke today.</span>
      <span class="starstrip-chips">
        <button class="chip" onclick={ackStrip}>Rest</button>
        <button class="chip" onclick={ackStrip}>Water</button>
        <button class="chip" onclick={openSmallestTodo}>Smallest todo</button>
      </span>
      <button class="starstrip-x" onclick={ackStrip} aria-label="Dismiss">×</button>
    </div>
  {/if}

  <!-- GO-ORBITTY-GHOST-DOCK: agentrow + composer ride in one wrap — the
       ghost's outlined bottom slab. × stays a floating corner; ⚙ is on
       the dock after Expand (GEAR-DOCK). -->
  <div class="ghostdock" onmousedown={onDockMouseDown}>
    <!-- Agent row: one strip — agent pick + icon toggles (Prompts, Speak,
         View, Expand/Collapse). The Body picker lives in Settings →
         Appearance. Icons are inline SVG, currentColor, 16px. -->
    <div class="agentrow">
      <select
        id="companion-agent"
        class="agentpick"
        bind:value={agent}
        onchange={onAgentChange}
        disabled={sending}
        aria-label="Which agent soul is the companion"
      >
        {#each agents as a}
          <option value={a}>{agentLabel(a)}</option>
        {/each}
      </select>
      <button
        class="expandbtn iconbtn promptsbtn"
        class:on={prompts}
        onclick={togglePrompts}
        aria-pressed={prompts}
        aria-label="STARS prompts"
        title="STARS prompts — one quiet strip when today's check-in has a thin spoke. Off by default."
      >
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M9 3l1.4 4.6L15 9l-4.6 1.4L9 15l-1.4-4.6L3 9l4.6-1.4z" />
          <path d="M17 11l.9 2.9 2.9.9-2.9.9-.9 2.9-.9-2.9-2.9-.9 2.9-.9z" />
          <path d="M16.5 3l.7 2.3 2.3.7-2.3.7-.7 2.3-.7-2.3-2.3-.7 2.3-.7z" />
        </svg>
      </button>
      <span class="dests" role="group" aria-label="Destinations">
        <button
          class="dest iconbtn"
          class:on={hosted === "launchpad"}
          onclick={() => toggleDest("launchpad")}
          aria-label="Launchpad"
          data-chip="Launchpad"
          title="Launchpad in the well — click again to dismiss"
        >
          <!-- DEST-CHROME: the same ship flying ↗ — the blessed rocket
               rotated 45° so the nose rides upper-right, never the
               upright capsule. -->
          <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
            <g transform="rotate(45 12 12)">
              <path
                fill-rule="evenodd"
                d="M12 2.5 C14.6 4.6 16 7.9 16 11.2 C16 13.1 15.7 14.9 15.2 16.2 L8.8 16.2 C8.3 14.9 8 13.1 8 11.2 C8 7.9 9.4 4.6 12 2.5 Z M10.5 9.3 a1.5 1.5 0 1 0 3 0 a1.5 1.5 0 1 0 -3 0 Z"
              />
              <path d="M8.7 12.3 C7.2 13.4 6.2 15.4 5.8 17.8 L8.9 16.6 C8.6 15.2 8.6 13.7 8.7 12.3 Z" />
              <path d="M15.3 12.3 C16.8 13.4 17.8 15.4 18.2 17.8 L15.1 16.6 C15.4 15.2 15.4 13.7 15.3 12.3 Z" />
              <path d="M12 17.6 C12.8 18.7 13.2 19.8 13.2 21 C12.5 20.6 11.5 20.6 10.8 21 C10.8 19.8 11.2 18.7 12 17.6 Z" />
            </g>
          </svg>
        </button>
        <button
          class="dest iconbtn"
          class:on={hosted === "stars"}
          onclick={() => toggleDest("stars")}
          aria-label="STARS"
          data-chip="STARS"
          title="STARS in the well — click again to dismiss"
        >
          <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
            <path d="M12 3 L14.1 9.1 L20.6 9.2 L15.4 13.1 L17.3 19.3 L12 15.6 L6.7 19.3 L8.6 13.1 L3.4 9.2 L9.9 9.1 Z" />
          </svg>
        </button>
        <button
          class="dest iconbtn planetsdest"
          class:on={hosted === "inspector"}
          onclick={() => toggleDest("inspector")}
          aria-label="Planets"
          data-chip="Planets"
          title="Planets in the well — click again to dismiss"
        >
          <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
            <circle cx="12" cy="12" r="5" />
            <ellipse
              cx="12"
              cy="12"
              rx="9.4"
              ry="3.3"
              transform="rotate(-20 12 12)"
              fill="none"
              stroke="currentColor"
              stroke-width="1.7"
            />
          </svg>
        </button>
      </span>
      <button
        class="expandbtn iconbtn speakbtn"
        class:on={speak}
        onclick={toggleSpeak}
        aria-pressed={speak}
        aria-label="Speak replies"
        title="Read bot replies aloud with the macOS system voice"
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <polygon
            points="11 5 6 9 2.5 9 2.5 15 6 15 11 19 11 5"
            fill="currentColor"
            stroke="none"
          />
          <path d="M15 9.2a4.2 4.2 0 0 1 0 5.6" />
          {#if !speak}
            <line x1="5.6" y1="18.4" x2="18.4" y2="5.6" />
          {/if}
        </svg>
      </button>
      <!-- DOCK-TWEAK: L-click hides/shows the sprite; R-click Full ↔ Head.
           The button stays ENABLED while the sprite is off — even with
           no bust art — so L-click can always bring it back. -->
      <button
        class="expandbtn iconbtn viewbtn"
        class:on={headShowing}
        onclick={toggleSprite}
        oncontextmenu={(e) => {
          e.preventDefault();
          toggleView();
        }}
        disabled={viewBusy || (sprite && !bustOk)}
        aria-pressed={headShowing}
        aria-label="Hide sprite, or right-click for Head or full body"
        title={viewTitle}
      >
        {#if headShowing}
          <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
            <circle cx="12" cy="8" r="4.4" />
            <path d="M4.6 21c.6-4 3.6-6.4 7.4-6.4s6.8 2.4 7.4 6.4z" />
            {#if !sprite}
              <line
                x1="5.6"
                y1="18.4"
                x2="18.4"
                y2="5.6"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
              />
            {/if}
          </svg>
        {:else}
          <!-- Full body: restroom-man (head + shouldered torso + two legs),
               never the circle + rounded-rect that read as a fat i. -->
          <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
            <circle cx="12" cy="4.6" r="2.6" />
            <path
              d="M9.2 8.4h5.6c1.3 0 2.2 1 2.2 2.2v4.4h-1.6v-3.6h-.6V21h-1.9v-6h-1.8v6H9.4v-9.6h-.6V15H7.2v-4.4c0-1.2.9-2.2 2-2.2z"
            />
            {#if !sprite}
              <line
                x1="5.6"
                y1="18.4"
                x2="18.4"
                y2="5.6"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
              />
            {/if}
          </svg>
        {/if}
      </button>
      <button
        class="expandbtn iconbtn"
        onclick={toggleExpanded}
        aria-label={expanded ? "Collapse transcript" : "Expand transcript"}
        title={expanded ? "Collapse transcript" : "Expand transcript"}
      >
        {#if expanded}
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <line x1="18" y1="6" x2="7.5" y2="16.5" />
            <polyline points="15.5 16.5 7.5 16.5 7.5 8.5" />
          </svg>
        {:else}
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <line x1="6" y1="18" x2="16.5" y2="7.5" />
            <polyline points="8.5 7.5 16.5 7.5 16.5 15.5" />
          </svg>
        {/if}
      </button>
      <button
        type="button"
        class="gear"
        onclick={() => invoke("open_settings_window").catch(() => {})}
        title="Open Settings"
        aria-label="Open Settings"
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
          <circle cx="12" cy="12" r="3" />
          <path
            d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 0 1-4 0v-.2a1.7 1.7 0 0 0-1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 0 1 0-4h.2a1.7 1.7 0 0 0 1.5-1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 0 1 4 0v.2a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9c.3.6.9 1 1.5 1H21a2 2 0 0 1 0 4h-.2a1.7 1.7 0 0 0-1.5 1z"
          />
        </svg>
      </button>
    </div>

    <form
      class="composer"
      onsubmit={(e) => {
        e.preventDefault();
        sendLine();
      }}
    >
      <div class="fieldwrap" class:talking>
        <input
          class="field"
          type="text"
          bind:this={fieldEl}
          bind:value={draft}
          placeholder="type…"
          aria-label="Message your companion"
          autocomplete="off"
          spellcheck="false"
        />
        <span class="talkslot">
          {#if talking}
            <button
              type="button"
              class="talkic"
              onclick={talkStopOnly}
              aria-label="Stop talking — text stays for review"
              title="Stop talking — the text stays in the field; you hit Send"
            >
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.6"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <polyline points="4.5 12.5 9.5 17.5 19.5 6.5" />
              </svg>
            </button>
            <button
              type="button"
              class="talkic sendnow"
              onclick={talkStopSend}
              aria-label="Stop and send now"
              title="Stop and send now"
            >
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.6"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <line x1="12" y1="19" x2="12" y2="5" />
                <polyline points="5.5 11.5 12 5 18.5 11.5" />
              </svg>
            </button>
          {:else}
            <button
              type="button"
              class="talkic"
              onclick={talkStart}
              aria-label="Talk"
              title="Click to talk. Source is Settings → Talk. Text lands in the field; you hit Send"
            >
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.9"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <rect x="9" y="2.5" width="6" height="11.5" rx="3" />
                <path d="M5.5 10.5a6.5 6.5 0 0 0 13 0" />
                <line x1="12" y1="17.5" x2="12" y2="21" />
              </svg>
            </button>
          {/if}
        </span>
      </div>
      <button
        class="send"
        type="submit"
        disabled={sending || !draft.trim()}
        aria-label="Send"
        title="Send"
      >
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M12 4l8 11h-4.5L12 10.4 8.5 15H4z" />
        </svg>
      </button>
    </form>
  </div>

  <button
    class="winx"
    onmousedown={(e) => e.stopPropagation()}
    onclick={async () => {
      // HOST-SPLIT: restore the idle size before the hide so the
      // persisted place is never the host frame.
      if (hosted !== "off") await dismissHost();
      invoke("hide_window", { label: "companion" }).catch(() => {});
    }}
    title="Hide Companion — reopen from the rail or tray"
    aria-label="Hide Companion">×</button
  >
</main>

<style>
  /* Same theme tokens as the rail/Lists: every painted fill is the
     JS-computed rgba var so the opacity slider fades the whole stage;
     the spirit pixels are an <img> and stay full strength. S2.4: column
     layout — ghost (collapsed) or head+transcript (expanded), CC rows,
     agent row, composer. */
  .companion {
    position: relative;
    height: 100vh;
    overflow: hidden;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border-radius: 12px;
    font-family:
      "SF Pro Text", "SF Pro Display", system-ui, -apple-system,
      BlinkMacSystemFont, "Helvetica Neue", sans-serif;
  }

  /* GO-ORBITTY-COMPANION-CLEAR: ghost (idle collapsed, not hosted) —
     the slab fill goes transparent so empty pixels show and click
     through to the desk; the chrome keeps its own fills and still hits.
     Slab (expanded / .hosting) keeps today's fill. */
  .companion.ghost {
    background: transparent;
  }

  :global(.low-fill) .companion {
    text-shadow: 0 1px 2px var(--bg-solid, transparent);
  }

  /* The only window chrome: one ×, top-LEFT (traffic lights are hidden
     native-side, so the left corner is free). Hides the window, never
     quits (tray → Quit). */
  .winx {
    position: absolute;
    top: 8px;
    left: 10px;
    z-index: 2;
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

  /* GO-ORBITTY-HOST-CC: the wrap is the overlay's positioning context —
     the expanded transcript covers the well ONLY (agentrow + composer
     stay below it, usable). The well keeps its own flex so nothing
     resizes on Expand. */
  .wellwrap {
    position: relative;
    flex: 1;
    min-height: 120px;
    min-width: 0;
    display: flex;
    flex-direction: row;
  }

  /* GO-ORBITTY-HOST-SPLIT: the hosted well — left ~2/3 embedded
     Launchpad/STARS, right 1/3 the Head spirit stage. The window grew to
     1080×620 native-side; the bottom chrome (CC + agentrow + composer)
     stays full-width under the well. */
  .hostwell {
    flex: 1;
    min-height: 120px;
    min-width: 0;
    display: flex;
    flex-direction: row;
  }

  .wellpane {
    flex: 1 1 66%;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-right: 1px solid var(--edge);
  }

  .wellhead {
    flex: 0 1 34%;
    min-width: 0;
    min-height: 0;
    display: flex;
  }

  /* SPRITE-OFF hosted: no .wellhead — the pane takes the full well and
     drops the divider. */
  .wellpane.fullwell {
    border-right: none;
  }

  /* SPRITE-HUG: the old spriteoff ::before flex spacer is dead — the
     native hug shrinks the idle collapsed frame to the CC+dock bar
     (keep-bottom), so the frame IS the chrome and there is no tall
     empty pane left to pin against. The bar stays ghost: empty air in
     it still clicks through via the reported hit list. */

  /* GO-ORBITTY-CLEAN-COMPANION-CAPTION: transcript / CC / wait mark live
     in CaptionPane.svelte. */

  /* GO-LS-STARS-STATUS prompt strip: one quiet menu when Prompts is On and
     today's check-in has a thin spoke. Rest/Water/× ack and hide it until
     the day rolls or a fresh check-in lands — never a loop. */
  .starstrip {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 4px 10px 0;
    padding: 5px 6px 5px 10px;
    font-size: 0.68rem;
    line-height: 1.3;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 8px;
  }

  .starstrip-text {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .starstrip-chips {
    flex: none;
    display: inline-flex;
    gap: 4px;
  }

  .chip {
    font: inherit;
    font-size: 0.66rem;
    font-weight: 600;
    padding: 2px 8px;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 999px;
    cursor: pointer;
  }

  .chip:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .starstrip-x {
    flex: none;
    padding: 0 2px;
    font: inherit;
    color: var(--dim);
    background: transparent;
    border: none;
    cursor: pointer;
  }

  .starstrip-x:hover {
    color: var(--ink);
  }

  /* Send error — one honest line, dismissible (rail honesty). */
  .note {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 4px 10px 0;
    padding: 5px 10px;
    font-size: 0.68rem;
    line-height: 1.3;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border-radius: 8px;
  }

  .note-text {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .note-x {
    flex: none;
    padding: 0 2px;
    font: inherit;
    color: var(--dim);
    background: transparent;
    border: none;
    cursor: pointer;
  }

  .note-x:hover {
    color: var(--ink);
  }

  /* GO-ORBITTY-GHOST-DOCK: one wrap around agentrow + composer. Ghost
     paints it as the small outlined bottom slab — same paint as the
     expanded window (--bg-fill, radius 12, 1px --edge) — floating just
     off the window edges. Expanded / hosted: no paint, no extra chrome,
     same stack as the unwrapped rows. DOCK-GRIP: ghost dock chrome (the
     padding around the rows, never a control) is a drag region; the
     slab (expanded / hosted) never drags from the dock.
     HOST-DOCK leftover: bottom inset (8px margin + 1px border) stays in
     every mode so dest toggle does not walk the dests. Ghost still adds
     the side/top float and the visible ring. */
  .ghostdock {
    position: relative;
    display: flex;
    flex-direction: column;
    margin-bottom: 8px;
    border: 1px solid transparent;
  }

  .ghost .ghostdock {
    margin: 4px 8px 8px;
    background: var(--bg-fill, var(--bg));
    border-color: var(--edge);
    border-radius: 12px;
  }

  /* Agent row: the agent soul pick + a strip of icon toggles (Prompts,
     Speak, View, Expand/Collapse). Ghost chrome gaps drag (DOCK-GRIP);
     the controls never do. */
  .agentrow {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px 0;
  }

  .agentpick {
    flex: none;
    max-width: 46%;
    padding: 3px 6px;
    font: inherit;
    font-size: 0.78rem;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 8px;
    outline: none;
  }

  .agentpick:focus {
    border-color: var(--focus, #6e4db0);
  }

  /* DEST-STRIP: the old spacer becomes the destination strip — quiet
     text (rail .stack-btn energy, scaled down), one line, flex:1 so the
     cluster holds the spark→speaker gap. At the 280px min the dests give
     first (type / letter-spacing / gap) — never the icon hit targets,
     never the agentpick. */
  .dests {
    flex: 1;
    min-width: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 3px;
    white-space: nowrap;
    position: relative;
  }

  /* PLANETS-POP: the name list opens UP from the dest strip, so it
     overlays the well / stage / CC / transcript and can never slide down
     over the composer. z-index 6 sits above the well + transcript (the
     chrome ×/gear are 2). Height-capped so a long world list scrolls
     inside itself instead of reaching the window top. */
  .planetspop {
    position: absolute;
    bottom: calc(100% + 8px);
    left: 50%;
    transform: translateX(-50%);
    z-index: 6;
    display: flex;
    flex-direction: column;
    min-width: 140px;
    max-width: 220px;
    max-height: 40vh;
    overflow-y: auto;
    padding: 4px;
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 10px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.28);
  }

  .planetsrow {
    padding: 5px 8px;
    font: inherit;
    font-size: 0.72rem;
    text-align: left;
    color: var(--ink);
    background: transparent;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .planetsrow:hover {
    background: var(--bg-fill, var(--bg));
  }

  .planetsrow.empty {
    color: var(--dim);
    cursor: default;
  }

  .planetsrow.empty:hover {
    background: transparent;
  }

  /* GO-ORBITTY-DEST-CHROME + DOCK-TWEAK: dests wear the same outlined
     pill as the other dock icon buttons — --panel-fill, 1px --edge,
     8px radius like .expandbtn — so Expand does not eat the rings. */
  .dest {
    flex: none;
    position: relative;
    padding: 2px 3px;
    font: inherit;
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    white-space: nowrap;
    color: var(--dim);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 8px;
    cursor: pointer;
  }

  .dest:hover {
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
  }

  /* DEST-CHROME: CSS hover chip naming the dest (title= never paints on
     the transparent panel). Opens UP from the button — over the stage /
     CC / well like .planetspop, never down over the composer. Same
     1px --edge ring + 8px radius as the pill; pointer-events none so it
     never steals the dest's own hover. */
  .dest::after {
    content: attr(data-chip);
    position: absolute;
    bottom: calc(100% + 6px);
    left: 50%;
    transform: translateX(-50%);
    z-index: 6;
    padding: 2px 7px;
    font-size: 0.62rem;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 8px;
    opacity: 0;
    pointer-events: none;
  }

  .dest:hover::after {
    opacity: 1;
  }

  /* HOST-SPLIT: the dest whose well is live. */
  .dest.on {
    color: var(--accent);
  }

  .expandbtn {
    flex: none;
    padding: 4px 12px;
    font: inherit;
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 8px;
    cursor: pointer;
  }

  .expandbtn:hover,
  .iconbtn:hover {
    border-color: var(--focus, #6e4db0);
  }

  /* Icon toggles: same pill as the old text buttons, squared up for a
     16px glyph. GO-ORBITTY-COMPANION-BRAND: the on state is per-toggle
     brand — Prompts sparkle = ochre --mark, Speak = violet --focus,
     Head silhouette = --ink (never teal). Idle icons stay --ink.
     GO-ORBITTY-HABITAT-BRAND: every on/hover RING is the deeper violet
     --focus #6e4db0 — the sparkle FILL stays ochre, pinky #c793e6 is
     --bloom (words/tags), never the ring. */
  .iconbtn {
    padding: 4px 7px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .iconbtn svg {
    width: 16px;
    height: 16px;
    display: block;
  }

  .promptsbtn.on {
    color: var(--mark, #e59e3d);
    border-color: var(--focus, #6e4db0);
  }

  .speakbtn.on {
    color: var(--focus, #6e4db0);
    border-color: var(--focus, #6e4db0);
  }

  .viewbtn.on {
    color: var(--ink);
    border-color: var(--focus, #6e4db0);
  }

  .iconbtn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  /* GEAR-DOCK: tiny corner glyph, not a dock-pill icon. */
  .gear {
    flex: none;
    align-self: center;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    margin-left: 2px;
    padding: 0;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 3px;
    cursor: pointer;
  }

  .gear svg {
    width: 11px;
    height: 11px;
    display: block;
  }

  .gear:hover {
    color: var(--ink);
  }

  /* Composer: field with the mic parked inside its right edge + Send
     outside. Ghost chrome gaps drag (DOCK-GRIP); field / mic / Send
     never do. */
  .composer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px 10px;
  }

  .field {
    flex: 1;
    min-width: 0;
    padding: 7px 10px;
    font: inherit;
    font-size: 0.82rem;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 9px;
    outline: none;
  }

  .field:focus {
    border-color: var(--focus, #6e4db0);
  }

  .field::placeholder {
    color: var(--dim);
  }

  .send {
    flex: none;
    padding: 7px 12px;
    font: inherit;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--on-accent, #ffffff);
    background: var(--accent);
    border: none;
    border-radius: 9px;
    cursor: pointer;
  }

  .send svg {
    width: 14px;
    height: 14px;
    display: block;
  }

  .send:disabled {
    opacity: 0.45;
    cursor: default;
  }

  /* Mic in the field: the wrap is relative and the slot parks the mic
     (idle) or check + up-arrow (recording) inside the right edge;
     padding-right keeps typed text from running under the icons.
     Check = stop (text stays for review); up-arrow = stop + send now. */
  .fieldwrap {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
  }

  .fieldwrap .field {
    width: 100%;
    padding-right: 32px;
  }

  .fieldwrap.talking .field {
    padding-right: 56px;
  }

  .talkslot {
    position: absolute;
    right: 5px;
    top: 50%;
    transform: translateY(-50%);
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }

  .talkic {
    width: 24px;
    height: 24px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
  }

  .talkic svg {
    width: 16px;
    height: 16px;
    display: block;
  }

  .talkic:hover {
    color: var(--accent);
  }

  .fieldwrap.talking .talkic {
    color: var(--ink);
  }

  .talkic.sendnow {
    color: var(--accent);
  }
</style>
