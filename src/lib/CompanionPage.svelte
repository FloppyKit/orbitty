<script lang="ts">
  // Spark mouth + Spirit. No model in this page. Native resize only
  // (never frontend set_size). Persist the whole companion object.
  // Spirit is default; 3D mounts only while a 3D body is picked.
  import { invoke } from "@tauri-apps/api/core";
  import { emit, emitTo, listen } from "@tauri-apps/api/event";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
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
  import Companion3D from "./Companion3D.svelte";
  import CaptionPane from "./CaptionPane.svelte";
  import CompanionDock from "./CompanionDock.svelte";
  import { eventElement } from "./dragPane";

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

  // The seven palette skins are retired; a
  // dead saved id falls back to the OS pair. Terminal stays valid.
  const KNOWN_THEMES = new Set(["systemLight", "systemDark", "terminal"]);

  function osThemeId(): string {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "systemDark"
      : "systemLight";
  }

  // Seed the OS theme synchronously so --bg exists
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
  // Px the collapsed composer grew past one line
  // (bound up from the dock). Sprite-off rides reportHug; sprite-on grows
  // the window down via set_companion_composer_grow.
  let composerExtra = $state(0);
  // Caption overlay wanted window height. Latch overlayBaseH so the
  // grow does not feed back into the next report.
  let overlayWinH = $state(0);
  let overlayExtra = $state(0);
  let overlayBaseH = 0;
  let ccSnap = $state(false);
  let sending = $state(false);
  let sendNote = $state<string | null>(null);
  // The conductor's fast ack line, splashed over
  // the wait mark while the mouth answers (companion:ack). Ephemeral —
  // never a turn; the reply replaces it. "" = no ack this send.
  let ackLine = $state("");
  let pendingAck = "";
  let ackReveal: ReturnType<typeof setTimeout> | undefined;
  const ACK_SPIN_MS = 2100;
  // The one-file chip. Original path only (never a
  // copy); it rides companion_send as an optional path, vanishes after
  // the send, and comes back on an honest error. One chip, × to drop.
  interface AttachChip {
    path: string;
    name: string;
    size: number;
  }
  let attach = $state<AttachChip | null>(null);
  async function attachPath(path: string) {
    try {
      attach = await invoke<AttachChip>("companion_attach_stat", { path });
      sendNote = null;
    } catch (e) {
      sendNote = `${e}`;
    }
  }
  async function pickAttach() {
    try {
      const info = await invoke<AttachChip | null>("companion_attach_pick");
      if (info) {
        attach = info;
        sendNote = null;
      }
    } catch (e) {
      sendNote = `${e}`;
    }
  }
  function dropAttach() {
    attach = null;
  }
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
  // The ctl-lit half of the dock LED — a harness
  // sets companion.hackLive (+ optional hackWho) via
  // `orbittyctl companion hack -- on [--who cursor]`; the dock lights the
  // LED when this OR `sending` is true. Persisted flag, not a transcript.
  let hackLive = $state(false);
  let hackWho = $state("");
  // The Body picker lives in Settings → Appearance (Companion body); this
  // window only follows the persisted pack (state:changed → loadState).
  let bodyId = $state("spirit");
  // COMPANION-3D: reserved 3D body ids (bundled GLBs, never pack folders).
  // glbFailed is the session fail-soft latch — a 3D miss reverts to the
  // spirit (persisted) instead of leaving a black stage.
  let glbFailed = $state(false);
  const is3D = $derived(bodyId === "tang" || bodyId === "female");

  let expanded = $state(false);
  let turns = $state<CompanionTurn[]>([]);

  // The companion window is the host. Session-only
  // — boot is always the idle spirit and nothing persists `hosted`.
  // "launchpad" = KanbanPage embedded in the well (pop-out allowed);
  // "stars" = StarsPage embedded (no pop-out, ever);
  // "inspector" = RailPage embedded (pop-out to the rail window).
  type HostMode = "off" | "launchpad" | "stars" | "inspector";
  let hosted = $state<HostMode>("off");
  let destOpen = $state({ launchpad: false, stars: false, inspector: false });
  // TILE-CHORDS leftover (⌘⌥↑): the last dest lowered through dismissHost
  // (dest-chip toggle, well ×, Escape, ⌘⌥↓ all land there). Session memory
  // only — never state.json; null = none lowered yet → ⌘⌥↑ opens Launchpad.
  let lastDest = $state<"launchpad" | "stars" | "inspector" | null>(null);

  // Host grow is native keep-bottom. Hide idle chrome (tilePre) before
  // the well mounts so ghost/CC never layout into the grown frame.
  const TILE_ENTER_MS = 280;
  const TILE_LEAVE_MS = 200;
  let tilePre = $state(false);
  let tileEnter = $state(false);
  let tileLeave = $state(false);
  let tileGen = 0;
  // Pin the idle figure through native grow so it never vanishes, then
  // FLIP (translate + scale) into the column — full body the whole way.
  let spiritHold = $state(false);

  // Dest swap is a ±120° triangular prism. Same dest does not turn.
  // Reduced motion: crossfade. Spark/dock never transform.
  const DEST_RING: readonly HostMode[] = ["launchpad", "stars", "inspector"];
  const TILE_YAW_MS = 400;
  let swapPrev = $state<HostMode | null>(null);
  let swapDir = $state<1 | -1>(1);
  // Prism apothem in px, measured from .wellpane at swap start.
  let prismR = $state(0);

  function tileReset() {
    spiritHold = false;
    tilePre = false;
    tileEnter = false;
    tileLeave = false;
    swapPrev = null;
    // Drop any stale spirit-aside translate vars (they only apply under
    // tile-pre/tile-enter; a fresh enter re-measures anyway).
    mainEl?.style.removeProperty("--spir-dx");
    mainEl?.style.removeProperty("--spir-dy");
    mainEl?.style.removeProperty("--spir-s");
  }

  let mainEl: HTMLElement | null = null;
  let wellpaneEl: HTMLElement | null = null;
  let captionReady = $state(false);
  function onCaptionSpeakError(msg: string) {
    sendNote = msg;
  }

  // COLLAPSE-CC: the fixed reply overlay opened/closed — re-report ghost
  // hits so it catches (open) / releases (close) its pixels. Lives out
  // here because the caption snippet's `hosted` param shadows HostMode.
  // wantWin is the overlay's wanted
  // WINDOW height — the hug (sprite-off) / composer grow (sprite-on)
  // grows the companion window so the reply fits whole, and the close
  // snaps it back. overlayBaseH latches on the FIRST report of an open,
  // so a re-latch (reply swap while open) never double-counts the grow
  // already applied.
  function onCaptionOverlayChange(open: boolean, wantWin = 0) {
    if (open) {
      if (overlayWinH <= 0) overlayBaseH = window.innerHeight;
      overlayWinH = wantWin;
      overlayExtra = Math.max(0, Math.round(wantWin - overlayBaseH));
    } else {
      if (overlayWinH > 0) ccSnap = true;
      overlayWinH = 0;
      overlayExtra = 0;
      overlayBaseH = 0;
    }
    if (!expanded && hosted === "off") {
      tick().then(() => {
        reportHug();
        reportComposerGrow();
      });
    }
  }

  // Hosted+expanded Escape collapses the overlay only. CaptionPane
  // owns ccOpen Escape. Hosted with no overlay: dismissHost.
  function onPaneKey(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    // Lock law: a focused field owns Esc first —
    // its own handler (if any) already ran at target; blur it here and
    // never close the well out from under an edit. e.target still names
    // the field even when the field's own Esc unmounted it mid-dispatch.
    const t = e.target as HTMLElement | null;
    if (t?.closest?.("input, textarea, select")) {
      if (document.activeElement === t) t.blur();
      e.preventDefault();
      return;
    }
    if (hosted !== "off" && expanded) {
      expanded = false;
      invoke("set_companion_expanded", { expanded: false }).catch(() => {});
      e.preventDefault();
    } else if (hosted !== "off") {
      e.preventDefault();
      void dismissHost();
    }
  }

  // One Full/Head pref for idle, expanded, Launchpad, STARS, Planets.
  let figureView = $state("full");
  let viewBusy = $state(false);
  const bustOk = $derived(is3D || restBust.length > 0);
  const headShowing = $derived(
    figureView === "head" && bustOk && !missing,
  );
  const drawFrames = $derived(headShowing ? activeBustFrames : activeFullFrames);

  async function toggleView() {
    if (viewBusy || !bustOk) return;
    const prev = figureView;
    const next = prev === "head" ? "full" : "head";
    figureView = next;
    viewBusy = true;
    try {
      const written = await invoke<string>("set_companion_view", {
        surface: "all",
        view: next,
      });
      figureView = written === "head" ? "head" : "full";
    } catch (e) {
      figureView = prev;
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
    let h = Math.round(dockR.bottom - top + 8);
    // the open reply overlay rides the
    // hug — grow the bar to its wanted window height so the fixed popup
    // fits whole (it covers the bar), never clipped to the idle bar.
    // The native side still clamps to the tall idle rect — that clamp is
    // the absurd line where the overlay keeps its inner scroll.
    if (overlayWinH > 0) h = Math.max(h, overlayWinH);
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

  // Hosted well gets a comfortable height (dock stays). Launchpad /
  // Planets scroll inside; STARS fills the well. Not a content-hug.
  const HOST_COMFORT_H = 820;
  let lastHostHug = 0;
  let hugTimer: ReturnType<typeof setTimeout> | undefined;
  function reportHostHug() {
    if (hosted === "off" || expanded || tilePre || tileEnter || swapPrev) return;
    if (hugTimer) clearTimeout(hugTimer);
    hugTimer = setTimeout(() => {
      hugTimer = undefined;
      if (hosted === "off" || expanded || tilePre || tileEnter || swapPrev) return;
      if (Math.abs(HOST_COMFORT_H - lastHostHug) < 4) return;
      lastHostHug = HOST_COMFORT_H;
      invoke("set_companion_host_hug", { height: HOST_COMFORT_H }).catch(() => {});
    }, 80);
  }

  // Sprite-ON collapsed grows the WINDOW down
  // with the composer (keep top — the spirit is never squashed). The
  // native side owns the gate (collapsed + sprite on + not hosted +
  // shown) and the ungrow on expand / host / sprite-off / hide, so this
  // is a plain report of the dock-measured extra.
  // the open reply overlay's extra
  // rides the SAME channel — the window grows down until the popup fits
  // whole, and snaps back when it closes. (Sprite-off rides reportHug;
  // the native gate turns this report into a no-op there.)
  function actionRowExtra(): number {
    if (!cheatSheetHint || !mainEl) return 0;
    const row = mainEl.querySelector(".actionrow");
    if (!row) return 0;
    const h = row.getBoundingClientRect().height;
    return h > 2 ? Math.round(h) : 0;
  }

  function reportComposerGrow() {
    invoke("set_companion_composer_grow", {
      extra: composerExtra + overlayExtra + actionRowExtra(),
    }).catch(() => {});
  }

  $effect(() => {
    const watching = [
      composerExtra,
      overlayExtra,
      expanded,
      hosted,
      sprite,
      cheatSheetHint,
    ];
    void watching;
    tick().then(() => reportComposerGrow());
  });

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

  const viewTitle = $derived(
    !sprite
      ? "Sprite hidden. Click: Head or full body · Right-click: show sprite"
      : bustOk
        ? "Click: Head or full body · Right-click: hide sprite"
        : "Click: full body (no bust art) · Right-click: hide sprite",
  );

  const captions = true;

  let planetsHidden = $state(false);
  let cheatSheetHint = $state(true);
  async function togglePlanetsHidden() {
    const next = !planetsHidden;
    planetsHidden = next;
    try {
      await invoke("set_planets_hidden", { hidden: next });
    } catch (e) {
      planetsHidden = !next;
      sendNote = `${e}`;
    }
  }

  let speakVolume = $state(1);
  let volOpen = $state(false);
  async function setSpeakVolume(v: number) {
    const vol = Math.max(0, Math.min(1, v));
    speakVolume = vol;
    try {
      await invoke("set_companion_speak_volume", { volume: vol });
      if (vol > 0 && !speak) await toggleSpeak();
    } catch (e) {
      sendNote = `${e}`;
    }
  }

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

  // Prompts strip + hold-to-talk

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
  // Quiet dock pip. Count = open/later-due
  // Needed only (not todos). Zero hides the pip; ⌘⇧N still toggles.
  let nextCount = $derived(
    cards.filter((c) => {
      if (c.kind !== "needed") return false;
      return c.neededState === "open" || c.neededState === "later";
    }).length,
  );
  function toggleNextCard() {
    invoke("toggle_next_card_window").catch(() => {});
  }
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
  let fieldEl: HTMLTextAreaElement | null = null;
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

  let neededFocusId = $state<string | null>(null);
  let neededFocusTitle = $state<string | null>(null);
  // Plain let so onFocusChanged (registered once) sees the live hook.
  let mouthHooked = false;
  function onNeededFocus(id: string | null, title: string | null) {
    neededFocusId = id;
    neededFocusTitle = title;
    mouthHooked = !!id;
    if (id) void landCaret();
  }

  function composerField(): HTMLTextAreaElement | null {
    return (
      fieldEl ??
      document.querySelector<HTMLTextAreaElement>("textarea.field")
    );
  }

  function focusComposer(): boolean {
    const el = composerField();
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

  // Native raise (arm_companion_mouth / ⌘⇧O show_companion) makes this
  // the key window; the caret still has to land in .field after that.
  // One tick is too early — Launchpad just had key, and fieldEl can be
  // stale until the dock paints the placeholder. Retry until it sticks.
  async function landCaret() {
    await tick();
    await new Promise<void>((r) =>
      requestAnimationFrame(() => requestAnimationFrame(() => r())),
    );
    if (focusComposer()) return;
    for (const ms of [40, 100, 220, 400]) {
      await new Promise((r) => setTimeout(r, ms));
      if (focusComposer()) return;
    }
  }

  // Enter on a Needed row arms the composer
  // answer (onNeededFocus already set the hint) — land the caret so the
  // next typed line answers it. Esc blurs back out (onPaneKey), arrows
  // resume the ring.
  function onNeededAnswer() {
    void landCaret();
  }


  async function sendLine() {
    const line = draft.trim();
    const file = attach;
    if (!line && !file) return;
    // Highlighted Needed in Launchpad owns the mouth — even with
    // Launchpad in its own window. A file chip stands the hijack down.
    if (neededFocusId && !file) {
      draft = "";
      try {
        await invoke("needed_answer", { id: neededFocusId, answer: line });
      } catch (e) {
        draft = line;
        sendNote = `${e}`;
      }
      return;
    }
    if (sending) return;
    sending = true;
    sendNote = null;
    ackLine = "";
    pendingAck = "";
    if (ackReveal) clearTimeout(ackReveal);
    ackReveal = setTimeout(() => {
      ackReveal = undefined;
      if (sending && pendingAck) ackLine = pendingAck;
    }, ACK_SPIN_MS);
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
    turns = [...turns, { role: "you", text: youText }].slice(-40);
    userLine = youText;
    draft = "";
    attach = null;
    try {
      const reply = await invoke<string>("companion_send", {
        line,
        path: file?.path ?? null,
      });
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
        turns[n - 2].text === youText;
      if (!landed) {
        turns = [...turns, { role: "them", text: reply }].slice(-40);
      }
      ackLine = "";
      pendingAck = "";
      if (ackReveal) {
        clearTimeout(ackReveal);
        ackReveal = undefined;
      }
    } catch (e) {
      // The you bubble stays (it was appended up front); the draft is
      // already gone — one honest error line is all that remains. The
      // chip comes back too: the file never left, so don't fake it.
      sendNote = `${e}`;
      ackLine = "";
      pendingAck = "";
      if (ackReveal) {
        clearTimeout(ackReveal);
        ackReveal = undefined;
      }
      attach = file;
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

  // Four-cast stem → mode JOB (spark/work/chat/hold). One transcript —
  // set_companion_mode does not park a second brain. Labels stay Spark /
  // Work / Chat / Hold.
  const AGENT_MODE: Record<string, string> = {
    spark: "spark",
    grok: "work",
    "chaos-gremlin": "chat",
    "fkt-homie": "hold",
  };

  async function stampThin() {
    try {
      const msg = await invoke<string>("companion_thin");
      sendNote = msg.split("\n")[0] ?? "THIN stamped";
    } catch (e) {
      sendNote = `${e}`;
    }
  }

  async function onAgentChange() {
    try {
      agent = await invoke<string>("set_companion_agent", { profile: agent });
      // Same flip switches the JOB (mode law). Transcript stays.
      const nextMode = AGENT_MODE[agent];
      if (nextMode) {
        await invoke("set_companion_mode", { mode: nextMode });
      }
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

  // The hosted well
  // One host window = this companion. The native set_companion_host owns
  // the frame (stash the idle rect → 1080×620 on enter, restore on off);
  // this JS state only owns WHICH component sits in the well.
  // The expanded transcript OVERLAYS the well
  // (absolute over .wellwrap, dimmed scrim, opaque bubbles) — the well
  // stays mounted, the host frame stays up; collapse = second Expand or
  // Escape.

  async function enterHost(mode: "launchpad" | "stars" | "inspector") {
    const gen = ++tileGen; // a fresh enter/swap supersedes any in-flight leave
    if (hosted !== "off") {
      // Swap dests: re-assert host (no second grow). Same dest does not turn.
      tileReset();
      const prev = hosted;
      // Different dest turns the prism; same dest (tray raise) does not.
      if (prev !== mode) {
        const from = DEST_RING.indexOf(prev);
        swapDir = DEST_RING.indexOf(mode) === (from + 1) % DEST_RING.length ? 1 : -1;
        // Prism apothem from the pane width: r = w / (2·tan(π/3)). The
        // pane is already mounted and keeps its width across a swap.
        const w = wellpaneEl?.clientWidth ?? 0;
        prismR = w > 16 ? Math.round(w / (2 * Math.tan(Math.PI / 3))) : 0;
        // r=0 + rotateY(120) + backface-hidden is a black card. Instant
        // swap instead of a 3D turn that cannot land.
        swapPrev = prismR > 0 ? prev : null;
      }
      hosted = mode;
      try {
        await invoke("set_companion_host", { mode });
      } catch (e) {
        hosted = prev;
        swapPrev = null;
        sendNote = `${e}`;
        return;
      }
      // Both faces are mounted now (outgoing under swapPrev, incoming
      // under hosted) — drop the outgoing face once the CSS turn has
      // finished (timeout matches the 400ms keyframes; 140 reduced). A
      // fresh enter/swap/leave bumps tileGen and owns it.
      if (swapPrev !== null) {
        const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
        setTimeout(
          () => {
            if (gen === tileGen) swapPrev = null;
          },
          (reduce ? 140 : TILE_YAW_MS) + 40,
        );
      }
      return;
    }
    // Measure the idle spirit box before native grow; hosted spirit
    // translates onto that box. Never scale about the feet.
    const idleSpirit =
      !expanded && sprite
        ? (mainEl
            ?.querySelector(".stage .floatwrap")
            ?.getBoundingClientRect() ?? null)
        : null;
    if (idleSpirit && mainEl && idleSpirit.width >= 2) {
      const win = mainEl.getBoundingClientRect();
      mainEl.style.setProperty(
        "--hold-x",
        `${(idleSpirit.left - win.left).toFixed(1)}px`,
      );
      mainEl.style.setProperty(
        "--hold-b",
        `${(win.bottom - idleSpirit.bottom).toFixed(1)}px`,
      );
      mainEl.style.setProperty("--hold-w", `${idleSpirit.width.toFixed(1)}px`);
      mainEl.style.setProperty("--hold-h", `${idleSpirit.height.toFixed(1)}px`);
      spiritHold = true;
    }
    tilePre = true;
    // Hold class + native grow in parallel — sequential tick-then-invoke
    // was the empty pause before the sprite moved.
    const grow = invoke("set_companion_host", { mode });
    try {
      await Promise.all([tick(), grow]);
    } catch (e) {
      tileReset();
      spiritHold = false;
      sendNote = `${e}`;
      return;
    }
    hosted = mode;
    await tick();
    let shifted = false;
    if (idleSpirit && mainEl && idleSpirit.width >= 2) {
      const art =
        mainEl.querySelector(".wellhead .floatwrap") ??
        mainEl.querySelector(".wellhead .spirshift");
      const rest = art?.getBoundingClientRect();
      if (rest && rest.width >= 2 && rest.height >= 2) {
        const dx =
          idleSpirit.left + idleSpirit.width / 2 - (rest.left + rest.width / 2);
        const dy = idleSpirit.bottom - rest.bottom;
        const s = idleSpirit.width / rest.width;
        mainEl.style.setProperty("--spir-dx", `${dx.toFixed(1)}px`);
        mainEl.style.setProperty("--spir-dy", `${dy.toFixed(1)}px`);
        mainEl.style.setProperty("--spir-s", Number.isFinite(s) ? s.toFixed(3) : "1");
        shifted = true;
      }
    }
    if (!shifted && mainEl) {
      mainEl.style.removeProperty("--spir-dx");
      mainEl.style.removeProperty("--spir-dy");
      mainEl.style.removeProperty("--spir-s");
    }
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    tilePre = false;
    tileEnter = true;
    spiritHold = false;
    setTimeout(
      () => {
        if (gen !== tileGen) return;
        tileEnter = false;
      },
      (reduce ? 140 : TILE_ENTER_MS) + 80,
    );
    // Force collapsed on enter (the existing persisted expanded flag).
    if (expanded) {
      expanded = false;
      invoke("set_companion_expanded", { expanded: false }).catch(() => {});
    }
  }

  async function dismissHost() {
    if (hosted === "off" || tileLeave) return;
    // ⌘⌥↑ leftover: remember what is being lowered (written just before
    // `hosted = "off"` below; a superseded leave never completes the
    // lower, so it never writes).
    const lowered = hosted;
    const gen = ++tileGen;
    tilePre = false;
    tileEnter = false;
    spiritHold = false;
    // A mid-drop dismiss supersedes the enter timeout; the spirit is
    // already in the hosted column (no hold overlay to release).
    swapPrev = null; // a mid-turn dismiss drops the outgoing face
    // Reverse the drop in the grown frame; native idle restore waits
    // for CSS. Never restore mid-animation.
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (!reduce) {
      tileLeave = true;
      await new Promise((r) => setTimeout(r, TILE_LEAVE_MS));
      if (gen !== tileGen) return; // a fresh enter/swap superseded us
      tileLeave = false;
    }
    lastDest = lowered;
    hosted = "off";
    try {
      await invoke("set_companion_host", { mode: "off" });
    } catch (e) {
      sendNote = `${e}`;
    }
  }

  async function refreshDestOpen() {
    try {
      destOpen = await invoke<{
        launchpad: boolean;
        stars: boolean;
        inspector: boolean;
      }>("dest_open_state");
    } catch {
      /* chip paint retries on the next pane-vis */
    }
  }

  // Dest chips are lightswitches for real windows. The companion
  // stays idle — never enterHost. Click while open hides; click
  // again shows and orders front.
  async function toggleDest(mode: "launchpad" | "stars" | "inspector") {
    if (hosted !== "off") await dismissHost();
    try {
      await invoke("dest_toggle", { dest: mode });
    } catch (e) {
      sendNote = `${e}`;
    }
    await refreshDestOpen();
  }

  // PLANETS-STRIP: dest L-click toggles the well. Planet tabs in the well
  // (and Launchpad) raise/locate — the dest right-click menu is gone.
  // createPlanetTick still opens the sheet + name field if something
  // bumps it; the strip + is the mint.
  let createPlanetTick = $state(0);

  // Launchpad-only pop-out (the embedded banner button): the real kanban
  // window becomes the popped Launchpad beside the restored idle
  // companion. Never parks — the two sit side by side.
  async function popoutLaunchpad() {
    if (hosted !== "launchpad") return;
    tileGen++; // cancel any in-flight leave; pop-out stays instant
    tileReset();
    hosted = "off";
    try {
      await invoke("set_companion_host", { mode: "off" });
      await invoke("open_kanban");
    } catch (e) {
      sendNote = `${e}`;
    }
  }

  // Same pop-out pattern — dismiss the well,
  // then show the real rail window. Tray Inspector still show_rail too.
  async function popoutInspector() {
    if (hosted !== "inspector") return;
    tileGen++; // cancel any in-flight leave; pop-out stays instant
    tileReset();
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
  // COMPANION-3D: reserved 3D ids load the SPIRIT art — it is the warm
  // fallback under Companion3D (and what a 3D failure lands on).
  async function loadArt() {
    const packId =
      bodyId === "spirit" || bodyId === "tang" || bodyId === "female"
        ? null
        : bodyId;
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

  // COMPANION-3D fail-soft: WebGL2 missing / loader error / missing GLB →
  // revert the pick to the blessed spirit (persisted, so Settings follows)
  // with one quiet line. Never a black stage, never a crash.
  async function onGlbFail() {
    if (glbFailed) return;
    glbFailed = true;
    sendNote = "3D body failed; back on Spirit.";
    bodyId = "spirit";
    try {
      await invoke<string | null>("set_companion_pack", { packId: "" });
    } catch {
      /* the local revert stands; the next state:changed re-syncs */
    }
  }

  async function loadState() {
    try {
      const s = await invoke<PersistedState>("load_state");
      themeId = KNOWN_THEMES.has(s.themeId) ? s.themeId : osThemeId();
      windowOpacity = clampWindowOpacity(s.windowOpacity ?? 1);
      // sendLine already swapped the you-row to
      // the line just sent; while sending, a state:changed reload must
      // not clobber it with the STALE persisted lastUserLine (still the
      // previous prompt — the native side writes the new one only when
      // the reply lands). Persisted truth reapplies on the next load
      // once sending is false.
      if (!sending) userLine = s.companion?.lastUserLine ?? "";
      botLine = s.companion?.lastBotLine ?? "";
      const incoming = s.companion?.turns ?? [];
      if (sending) {
        const optimistic = turns.at(-1);
        turns = incoming;
        if (
          optimistic?.role === "you" &&
          turns.at(-1)?.text !== optimistic.text
        ) {
          turns = [...turns, optimistic].slice(-40);
        }
      } else {
        turns = incoming;
      }
      expanded = s.companion?.expanded ?? false;
      speak = s.companion?.speakReplies ?? false;
      speakVolume = typeof s.companion?.speakVolume === "number"
        ? Math.max(0, Math.min(1, s.companion.speakVolume))
        : 1;
      figureView = s.companion?.viewCollapsed === "head" ? "head" : "full";
      sprite = s.companion?.sprite ?? true;
      planetsHidden = s.companion?.planetsHidden ?? false;
      cheatSheetHint = s.companion?.cheatSheetHint ?? true;
      // DOCK-HACK: missing keys (old saves) = dark LED, unknown source.
      hackLive = s.companion?.hackLive ?? false;
      hackWho = s.companion?.hackWho ?? "";
      const pack = s.companion?.imagePackId ?? null;
      const nextBody = pack && pack.length > 0 ? pack : "spirit";
      if (nextBody !== bodyId) {
        bodyId = nextBody;
        glbFailed = false; // a fresh pick re-tries the 3D load
        artLoaded = false;
        void loadArt();
      }
      stars = s.stars ?? { promptsOptIn: false, checkins: {} };
      prompts = stars.promptsOptIn ?? false;
      cards = s.kanbanCards ?? [];
      void refreshDestOpen();
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
  // AppKit setFrame follows the cursor (grab offset). startDragging
  // jumps the pane to the click; Tauri setPosition is the wrong space.
  // Click ≠ drag: wait for a real move, then native follow until mouse up.
  function startCompanionDrag(e: MouseEvent) {
    if (e.button !== 0) return;
    const x0 = e.screenX;
    const y0 = e.screenY;
    const move = (ev: MouseEvent) => {
      if (Math.hypot(ev.screenX - x0, ev.screenY - y0) < 12) return;
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", onUp);
      invoke("companion_drag_begin").catch(() => {});
    };
    const onUp = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", onUp);
  }

  function onDragMouseDown(e: MouseEvent) {
    startCompanionDrag(e);
  }

  function onDockMouseDown(e: MouseEvent | PointerEvent) {
    if (e.button !== 0) return;
    const t = eventElement(e);
    if (
      t?.closest(
        "button, input, a, select, textarea, option, .field, .fieldwrap, .volpop, .agentbubble, .agentopt",
      )
    )
      return;
    startCompanionDrag(e);
  }

  // SPRITE-HUG: re-fit the idle bar when CC / wait / strips change.
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
      composerExtra,
      cheatSheetHint,
    ];
    void watching;
    if (expanded || hosted !== "off") return;
    tick().then(() => reportHug());
  });

  // hug to the STARS ruler on dest change. Do
  // not watch Launchpad lists — those scroll inside the tab.
  $effect(() => {
    if (hosted === "off" || expanded) {
      lastHostHug = 0;
      if (hugTimer) {
        clearTimeout(hugTimer);
        hugTimer = undefined;
      }
      return;
    }
    void tilePre;
    void tileEnter;
    void swapPrev;
    tick().then(() => reportHostHug());
    return () => {
      if (hugTimer) {
        clearTimeout(hugTimer);
        hugTimer = undefined;
      }
    };
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
  let unlistenDestVis: (() => void) | undefined;
  let unlistenSummon: (() => void) | undefined;
  let unlistenNeededFocus: (() => void) | undefined;
  let unlistenWinFocus: (() => void) | undefined;
  let unlistenKanbanVis: (() => void) | undefined;
  let unlistenEnterHost: (() => void) | undefined;
  let unlistenToggleHost: (() => void) | undefined;
  let unlistenYawHost: (() => void) | undefined;
  let unlistenDismissHost: (() => void) | undefined;
  let unlistenRaiseLastHost: (() => void) | undefined;
  let unlistenDrop: (() => void) | undefined;
  let unlistenAck: (() => void) | undefined;
  let unlistenTalk: (() => void)[] = [];
  let hugObserver: ResizeObserver | undefined;
  let claimToast = $state<{
    projectId: string;
    projectName: string;
    windowId: string;
    seconds: number;
  } | null>(null);
  let claimToastTimer: ReturnType<typeof setTimeout> | undefined;
  let unlistenClaimToast: (() => void) | undefined;
  let unlistenBoardOpen: (() => void) | undefined;

  onMount(async () => {
    invoke("ensure_glass").catch(() => {});
    if (mainEl && typeof ResizeObserver !== "undefined") {
      hugObserver = new ResizeObserver(() => {
        // a resize after the overlay
        // closed is the snap-back landing — release the bottom-pack so
        // the bar owns the column again.
        if (ccSnap) ccSnap = false;
      });
      hugObserver.observe(mainEl);
    }
    await loadState();
    await refreshDestOpen();
    await loadAgents();
    if (!artLoaded) await loadArt();
    window.addEventListener("keydown", onPaneKey);
    unlistenState = await listen("state:changed", loadState);
    // HITCH-DRAW: hide stops the 140ms tick (the window keeps warm, so
    // document.hidden never flips); show resumes it. frameIdx is untouched
    // on hide — the blink resumes from the last frame, and the sheets are
    // never reloaded on a hide/show cycle. Reduce Motion still holds
    // frame 0 (restartTimer is a no-op there).
    unlistenDestVis = await listen<import("./paneVis").PaneVis>("orbitty:pane-vis", () => {
      void refreshDestOpen();
    });
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
        // COLLAPSE-CC: same for a sprite-on composer grow — the hide
        // ungrew native-side; re-report the dock's extra.
        tick().then(() => {
          reportHug();
          reportComposerGrow();
        });
      } else if (timer) {
        clearInterval(timer);
        timer = undefined;
      }
    });
    // ⌘⇧O raises this pane native-side, but webview
    // focus alone never lands the caret in .field — the summon event does.
    unlistenSummon = await listen("orbitty:companion-summon", () => {
      void landCaret();
    });
    unlistenNeededFocus = await listen<{
      id: string | null;
      title: string | null;
    }>("orbitty:needed-focus", (e) => {
      onNeededFocus(e.payload?.id ?? null, e.payload?.title ?? null);
    });
    unlistenWinFocus = await getCurrentWebviewWindow().onFocusChanged((e) => {
      if (e.payload && mouthHooked) focusComposer();
    });
    unlistenKanbanVis = await listenPaneVis("kanban", (visible) => {
      if (!visible) onNeededFocus(null, null);
    });
    // Tray Launchpad/STARS raise this pane
    // native-side and ask the well to open — raise-only via enterHost,
    // never toggleDest (the dest buttons still own toggling).
    unlistenEnterHost = await listen<string>("orbitty:enter-host", (e) => {
      const m = e.payload;
      if (m === "launchpad" || m === "stars" || m === "inspector") {
        void invoke("dest_show", { dest: m }).then(() => refreshDestOpen());
      }
    });
    // ⌘⌥1/2/3 are the dest chips — toggleDest,
    // so the same chord while that dest is hosted dismisses (the page
    // owns `hosted`; the native side only raises the pane). ⌘⌥←/→ yaw
    // the DEST_RING while hosted; idle is a no-op, never a summon (the
    // native side doesn't even raise the pane on the arrows). ⌘⌥↓ is
    // the same no-summon posture: dismiss whatever is hosted, no-op idle.
    // ⌘⌥↑ is the raise posture of ⌘⌥1/2/3 (the native side shows the
    // pane first, so idle works): re-host the last lowered dest.
    unlistenToggleHost = await listen<string>("orbitty:toggle-host", (e) => {
      const m = e.payload;
      if (m === "launchpad" || m === "stars" || m === "inspector") void toggleDest(m);
    });
    unlistenYawHost = await listen<string>("orbitty:yaw-host", (e) => {
      if (hosted === "off") return;
      // A second yaw mid-turn (key repeat) skips a dest — on a 3-ring
      // that is the other neighbor, i.e. the opposite arrow.
      if (swapPrev !== null || tileLeave) return;
      const from = DEST_RING.indexOf(hosted);
      const step = e.payload === "prev" ? DEST_RING.length - 1 : 1;
      const next = DEST_RING[(from + step) % DEST_RING.length];
      if (next !== "off") void enterHost(next);
    });
    unlistenDismissHost = await listen("orbitty:dismiss-host", () => {
      if (hosted === "off") return;
      void dismissHost();
    });
    // ⌘⌥↑: raise the last lowered dest (Launchpad when none this
    // session). Already hosted = a menu is up — no-op.
    unlistenRaiseLastHost = await listen("orbitty:raise-last-host", () => {
      void invoke("dest_show", { dest: lastDest ?? "launchpad" }).then(() =>
        refreshDestOpen(),
      );
    });
    unlistenBoardOpen = await listen<string>("habitat:board-open", (e) => {
      const id = (e.payload ?? "").trim();
      if (!id) return;
      void (async () => {
        try {
          await enterHost("launchpad");
          // Embedded Kanban shares this webview; pop-out uses label "kanban".
          await emit("orbitty:focus-project", id);
          emitTo("kanban", "orbitty:focus-project", id).catch(() => {});
        } catch (err) {
          sendNote = `${err}`;
        }
      })();
    });
    unlistenClaimToast = await listen<{
      projectId: string;
      projectName: string;
      windowId: string;
      seconds: number;
    }>("orbitty:claim-toast", (e) => {
      const p = e.payload;
      if (!p?.projectName) return;
      claimToast = {
        projectId: p.projectId,
        projectName: p.projectName,
        windowId: p.windowId,
        seconds: p.seconds ?? 3,
      };
      if (claimToastTimer) clearTimeout(claimToastTimer);
      claimToastTimer = setTimeout(() => {
        claimToast = null;
      }, (claimToast.seconds || 3) * 1000);
    });

    // The conductor's ack lands mid-send and rides
    // the wait-mark slot until the reply replaces it. Only while sending
    // — a late event after the reply must not resurrect a splash.
    unlistenAck = await listen<{ line?: string }>("companion:ack", (e) => {
      if (!sending) return;
      const line = (e.payload?.line ?? "").trim();
      pendingAck = line;
      if (!ackReveal) ackLine = line;
    });
    // A file dropped anywhere on this window becomes
    // the one chip (first path only — multi-file is forbidden in v0).
    // The path is the original; stat validates and sizes the chip.
    unlistenDrop = await getCurrentWebviewWindow().onDragDropEvent((e) => {
      if (e.payload.type !== "drop") return;
      const paths = e.payload.paths;
      if (paths.length > 1) sendNote = "one file at a time";
      if (paths.length >= 1) void attachPath(paths[0]);
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
    unlistenClaimToast?.();
    unlistenBoardOpen?.();
    if (claimToastTimer) clearTimeout(claimToastTimer);
    if (timer) clearInterval(timer);
    hugObserver?.disconnect();
    window.removeEventListener("keydown", onPaneKey);
    unlistenState?.();
    unlistenVis?.();
    unlistenDestVis?.();
    unlistenSummon?.();
    unlistenNeededFocus?.();
    unlistenWinFocus?.();
    unlistenKanbanVis?.();
    unlistenEnterHost?.();
    unlistenToggleHost?.();
    unlistenYawHost?.();
    unlistenDismissHost?.();
    unlistenRaiseLastHost?.();

    unlistenDrop?.();
    unlistenAck?.();
    for (const u of unlistenTalk) u();
    invoke("talk_stop").catch(() => {});
  });
</script>

{#if claimToast}
  <div class="claim-toast" role="status">
    <span>Claimed to {claimToast.projectName}</span>
    <button
      type="button"
      onclick={() => {
        if (!claimToast) return;
        void (async () => {
          try {
            const s = await invoke<PersistedState>("load_state");
            const list = s.projects ?? [];
            if (!list.length) return;
            const i = list.findIndex((p) => p.id === claimToast!.projectId);
            const next = list[(i + 1) % list.length];
            if (!next) return;
            await invoke("claim_front_change", { projectId: next.id });
            claimToast = {
              ...claimToast!,
              projectId: next.id,
              projectName: next.name,
            };
          } catch {
            /* ignore */
          }
        })();
      }}>Change</button
    >
    <button
      type="button"
      onclick={() => {
        invoke("claim_front_undo").catch(() => {});
        claimToast = null;
        if (claimToastTimer) clearTimeout(claimToastTimer);
      }}>Undo</button
    >
  </div>
{/if}


{#snippet caption(expanded: boolean, hosted: boolean)}
  <CaptionPane
    {expanded}
    {hosted}
    {userLine}
    {botLine}
    {sending}
    {ackLine}
    {turns}
    {speak}
    {draft}
    ready={captionReady}
    onSpeakError={onCaptionSpeakError}
    onOverlayChange={onCaptionOverlayChange}
  />
{/snippet}

<main
  class="companion"
  class:hosting={hosted !== "off"}
  class:ghost={!expanded && hosted === "off"}
  class:spriteoff={!sprite}
  class:ccopen={overlayWinH > 0 || ccSnap}
  class:tile-pre={tilePre}
  class:tile-enter={tileEnter}
  class:tile-leave={tileLeave}
  class:spirit-hold={spiritHold}
  class:cc-off={!captions}
  bind:this={mainEl}
>
  {#if expanded && sprite}
    {#if is3D && !glbFailed}
      {#key bodyId}
        <Companion3D
          corner
          {headShowing}
          {bodyId}
          thinking={sending}
          onDrag={onDragMouseDown}
          onFail={onGlbFail}
        />
      {/key}
    {:else}
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
             pane takes the full well (the card keeps its ring). -->
        <div
          class="wellpane"
          class:fullwell={!sprite}
          class:turning={swapPrev !== null}
          style:--prism-r="{prismR}px"
          bind:this={wellpaneEl}
        >
          <!-- on a hosted dest swap
               the outgoing page mounts as a second face (swapPrev) and
               both faces ride a ±120deg rotateY around the card's
               vertical axis — the crease between them is the read. Each
               face IS the tile for the turn (fill / 1px edge / 12px
               radius ride WITH it); the pane's own chrome goes
               transparent so no static frame rings the moving cards.
               Both mounted the whole 400ms, so the well never flashes
               blank. Not turning: one static layer, no 3D, same flex
               sizing as before. -->
          {#if swapPrev !== null}
            <div class="prismface face-out" class:face-back={swapDir === -1}>
              {#if swapPrev === "launchpad"}
                <KanbanPage
                  embedded
                  onClose={dismissHost}
                  onPopout={popoutLaunchpad}
                  {onNeededFocus}
                  {onNeededAnswer}
                />
              {:else if swapPrev === "stars"}
                <StarsPage embedded onClose={dismissHost} />
              {:else}
                <RailPage
                  embedded
                  onClose={dismissHost}
                  onPopout={popoutInspector}
                  {createPlanetTick}
                />
              {/if}
            </div>
          {/if}
          <div
            class="wellpage"
            class:face-in={swapPrev !== null}
            class:face-back={swapPrev !== null && swapDir === -1}
          >
            {#if hosted === "launchpad"}
              <KanbanPage
                embedded
                onClose={dismissHost}
                onPopout={popoutLaunchpad}
                {onNeededFocus}
                {onNeededAnswer}
              />
            {:else if hosted === "stars"}
              <StarsPage embedded onClose={dismissHost} />
            {:else if hosted === "inspector"}
              <RailPage
                embedded
                onClose={dismissHost}
                onPopout={popoutInspector}
                {createPlanetTick}
              />
            {/if}
          </div>
        </div>
        {#if sprite}
          <!-- Idle figure is held through native grow, then this column
               FLIPs (translate + scale, same Head/full as idle). -->
          <div class="wellhead">
            {#if !expanded}
              <div class="spirshift">
                {#if is3D && !glbFailed}
                  {#key bodyId}
                    <Companion3D
                      headShowing={headShowing}
                      {bodyId}
                      thinking={sending}
                      onDrag={onDragMouseDown}
                      onFail={onGlbFail}
                    />
                  {/key}
                {:else}
                  <SpiritStage
                    headShowing={headShowing}
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
        {/if}
      </div>
      {#if expanded && captions}
        {@render caption(true, true)}
      {/if}
    </div>
  {:else if expanded && captions}
    {@render caption(true, false)}
  {/if}
  {#if sprite && !expanded && (hosted === "off" || spiritHold)}
    {#if is3D && !glbFailed}
      {#key bodyId}
        <Companion3D
          {headShowing}
          {bodyId}
          thinking={sending}
          onDrag={onDragMouseDown}
          onFail={onGlbFail}
        />
      {/key}
    {:else}
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
  {/if}

  {#if !expanded && captions}
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

  <!-- Agentrow + dests + Speak +
       sprite/view + Expand + ⚙ + composer live in CompanionDock.svelte.
       × stays a floating corner here. -->
  <CompanionDock
    {agents}
    bind:agent
    {sending}
    {hackLive}
    {hackWho}
    {prompts}
    {hosted}
    {speak}
    {speakVolume}
    {headShowing}
    {viewBusy}
    {sprite}
    {bustOk}
    {viewTitle}
    {expanded}
    {planetsHidden}
    {talking}
    neededHint={neededFocusTitle}
    {cheatSheetHint}
    onCheatSheet={() => {
      invoke("toggle_cheatsheet").catch(() => {});
    }}
    bind:draft
    bind:fieldEl
    bind:composerExtra
    {attach}
    onPickAttach={pickAttach}
    onDropAttach={dropAttach}
    {onAgentChange}
    onThin={stampThin}
    {togglePrompts}
    destOpen={destOpen}
    {toggleDest}
    {nextCount}
    {toggleNextCard}
    {toggleSpeak}
    {setSpeakVolume}
    {toggleSprite}
    {toggleView}
    {toggleExpanded}
    {togglePlanetsHidden}
    {sendLine}
    {talkStart}
    {talkStopOnly}
    {talkStopSend}
    {onDockMouseDown}
  />

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

  /* Ghost (idle collapsed, not hosted) —
     the slab fill goes transparent so empty pixels show and click
     through to the desk; the chrome keeps its own fills and still hits.
     Slab (expanded / .hosting) keeps today's fill. */
  .companion.ghost {
    background: transparent;
  }

  /* Hosted, the slab fill goes transparent so the
     desk shows in the gap around the glass card — same family as the
     ghost dock. The window still eats clicks (no click-through). */
  .companion.hosting {
    background: transparent;
  }

  /* while the reply overlay holds the
     sprite-off window taller than the bar (the hug grew keep-bottom),
     bottom-pack the CC + dock so the grow — and the snap-back — never
     hops the dock. A no-op once the bar refills the window (flex-end ==
     flex-start when there is no extra space). Sprite-on never needs
     this: the stage flexes to absorb the grow-down. */
  .companion.ghost.spriteoff.ccopen {
    justify-content: flex-end;
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

  .winx.hosted-hide {
    display: none;
  }

  /* The wrap is the overlay's positioning context —
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

  /* The hosted well — left ~2/3 embedded
     Launchpad/STARS, right 1/3 the Head spirit stage. The window grew to
     1080×620 native-side; the bottom chrome (CC + agentrow + composer)
     stays full-width under the well. */
  .hostwell {
    position: relative;
    flex: 1;
    min-height: 120px;
    min-width: 0;
    display: flex;
    flex-direction: row;
    gap: 10px;
    padding: 10px;
  }

  /* The .wellpane wrap IS the glass card — same
     family as the ghost dock. The 10px ring inside .hostwell lets the
     desk show around it; radius 12 (same as .ghostdock), --bg-fill,
     1px --edge all around. The hard border-right divider is dead: the
     gap to the Spark column is air. Embedded Kanban / Stars / Rail keep
     their square inner chrome — the wrap is the card, not each page's
     banner. One tile at a time; no motion in this phrase. */
  .wellpane {
    position: relative; /* TILE-YAW: the outgoing overlay anchors here */
    flex: 1 1 66%;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 12px;
  }

  /* Hosted spirit sits in the same 96×96 corner as expanded chat so
     dest screens share one layout and expand does not jump the figure. */
  .wellhead {
    position: absolute;
    top: 8px;
    left: 10px;
    width: 96px;
    height: 96px;
    flex: none;
    z-index: 3;
    min-width: 96px;
    min-height: 96px;
    display: flex;
  }

  .companion:not(.spriteoff) .wellpane:not(.fullwell) {
    margin-left: 116px;
  }

  /* TILE-DROP: card rises from the dock. Face stays painted. No 3D. */
  .companion.tile-pre:not(.hosting) {
    justify-content: flex-end;
  }

  .companion.tile-pre:not(.hosting) > *:not(.ghostdock):not(.winx):not(.stage) {
    display: none;
  }

  /* Idle figure pinned through the grow — keep-bottom, so it does not
     vanish or stretch. Hands off to the column FLIP on the next frame. */
  .companion.spirit-hold > :global(.stage) {
    position: absolute;
    left: var(--hold-x);
    bottom: var(--hold-b);
    width: var(--hold-w);
    height: var(--hold-h);
    z-index: 5;
    flex: none;
    min-height: 0;
  }

  .companion.tile-pre .wellwrap,
  .companion.tile-enter .wellwrap,
  .companion.tile-leave .wellwrap {
    overflow: hidden;
  }

  .companion.tile-pre .wellpane {
    opacity: 1;
    transform: translateY(28%);
  }

  .companion.tile-enter .wellpane {
    animation: orbitty-tile-drop-in 280ms cubic-bezier(0.25, 0.8, 0.25, 1) both;
  }

  .companion.tile-leave .wellpane {
    animation: orbitty-tile-drop-out 200ms cubic-bezier(0.4, 0, 1, 1) both;
  }

  @keyframes orbitty-tile-drop-in {
    0% {
      transform: translateY(28%);
    }
    100% {
      transform: translateY(0);
    }
  }

  @keyframes orbitty-tile-drop-out {
    0% {
      transform: translateY(0);
    }
    100% {
      transform: translateY(24%);
    }
  }
  .spirshift {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
  }

  .companion.tile-pre :global(.wellhead .floatwrap),
  .companion.tile-enter :global(.wellhead .floatwrap) {
    transform-origin: bottom center;
    will-change: transform;
  }

  .companion.tile-pre :global(.wellhead .floatwrap) {
    transform: translate(var(--spir-dx, 0px), var(--spir-dy, 0px))
      scale(var(--spir-s, 1));
  }

  .companion.tile-enter :global(.wellhead .floatwrap) {
    animation: orbitty-spirit-aside 280ms cubic-bezier(0.25, 0.8, 0.25, 1) both;
  }

  @keyframes orbitty-spirit-aside {
    0% {
      transform: translate(var(--spir-dx, 0px), var(--spir-dy, 0px))
        scale(var(--spir-s, 1));
    }
    100% {
      transform: translate(0px, 0px) scale(1);
    }
  }

  /* the swap verb is a TRIANGULAR PRISM
     turn (Matt: not a recipe-card deal, not a book, not an X, not a 90°
     cube). The three dests are the three faces of a triangular prism
     rotating around a vertical axis through the card — the step is
     ±120deg. The WHOLE TILE yaws: .wellpane.turning sheds its own
     chrome (transparent fill/border, overflow visible) and only
     supplies the perspective — each FACE is the glass card (fill, 1px
     --edge, 12px radius, content clipped to the radius) turning over
     the desk. No static frame rings the moving pages. Each face
     transform is
     translateZ(-r) rotateY(θ) translateZ(r) with r = --prism-r (the
     apothem paneWidth / (2·tan(π/3)), measured JS-side at swap start):
     the face orbits the prism axis sitting r behind the card plane, so
     the front face lands exactly 1:1 on the card — no magnify pop at
     either end of the turn. backface-visibility hides each face once
     it passes 90°, so the outgoing face exits edge-on and the incoming
     face grows in from the crease — the user SEES the edge turn. A
     slight brightness dip on the receding face sells the depth. Both
     faces stay mounted the whole 400ms (ease-in-out; the JS cleanup
     timeout matches), so the well never flashes blank. Forward = next
     in the launchpad → stars → inspector ring: the front face exits
     left and the next face comes around from the right (.face-back
     mirrors). The Spark column / dock / CC never transform. */
  .wellpage {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    transform: none;
    animation: none;
  }

  .wellpane.turning {
    perspective: 1400px;
    /* Whole-tile turn: the pane sheds its own chrome for the 400ms so
       no static frame rings the moving tiles — the desk shows around
       them. border-color (not border-width) keeps the box identical;
       the faces below carry the card. */
    background: transparent;
    border-color: transparent;
    overflow: visible;
  }

  /* During the turn both faces are absolute copies of the card box and
     each IS the tile — the card fill / 1px --edge / 12px radius ride
     WITH the face as it yaws (content clipped to the radius), opaque,
     input-blackouted for the 400ms. inset:-1px + border-box puts the
     face's border exactly where the pane's border sat, so the chrome
     handoff at cleanup lands on the same box with no 1px pop. No
     transform here at base — the keyframes own every rotate/translate
     so reduced motion can simply swap the animation. */
  .wellpane.turning .prismface,
  .wellpane.turning .wellpage.face-in {
    position: absolute;
    inset: -1px;
    box-sizing: border-box;
    pointer-events: none; /* 400ms input blackout on the card — no misclick */
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 12px;
    overflow: hidden;
    backface-visibility: hidden;
  }

  /* The outgoing face: starts at rest on the card plane (identity-net)
     and orbits away — forward exits left (0 → -120deg). */
  .wellpane.turning .prismface.face-out {
    animation: orbitty-prism-out-fwd 400ms ease-in-out both;
  }

  .wellpane.turning .prismface.face-out.face-back {
    animation-name: orbitty-prism-out-back;
  }

  /* The incoming face: starts one face around the prism (±120deg,
     backface-hidden until it comes around) and lands exactly at rest. */
  .wellpane.turning .wellpage.face-in {
    animation: orbitty-prism-in-fwd 400ms ease-in-out both;
  }

  .wellpane.turning .wellpage.face-in.face-back {
    animation-name: orbitty-prism-in-back;
  }

  @keyframes orbitty-prism-out-fwd {
    0% {
      filter: brightness(1);
      transform: translateZ(calc(-1 * var(--prism-r))) rotateY(0deg)
        translateZ(var(--prism-r));
    }
    100% {
      filter: brightness(0.84);
      transform: translateZ(calc(-1 * var(--prism-r))) rotateY(-120deg)
        translateZ(var(--prism-r));
    }
  }

  @keyframes orbitty-prism-out-back {
    0% {
      filter: brightness(1);
      transform: translateZ(calc(-1 * var(--prism-r))) rotateY(0deg)
        translateZ(var(--prism-r));
    }
    100% {
      filter: brightness(0.84);
      transform: translateZ(calc(-1 * var(--prism-r))) rotateY(120deg)
        translateZ(var(--prism-r));
    }
  }

  @keyframes orbitty-prism-in-fwd {
    0% {
      filter: brightness(0.84);
      transform: translateZ(calc(-1 * var(--prism-r))) rotateY(120deg)
        translateZ(var(--prism-r));
    }
    100% {
      filter: brightness(1);
      transform: translateZ(calc(-1 * var(--prism-r))) rotateY(0deg)
        translateZ(var(--prism-r));
    }
  }

  @keyframes orbitty-prism-in-back {
    0% {
      filter: brightness(0.84);
      transform: translateZ(calc(-1 * var(--prism-r))) rotateY(-120deg)
        translateZ(var(--prism-r));
    }
    100% {
      filter: brightness(1);
      transform: translateZ(calc(-1 * var(--prism-r))) rotateY(0deg)
        translateZ(var(--prism-r));
    }
  }

  /* Reduced motion: fade or snap — never a translate, never a rotate.
     Enter fades; the leave wait is 0 JS-side, so dismiss is a snap. The
     spirit does not glide aside — it simply rests in the column. The
     prism turn degrades to a short crossfade (the JS cleanup shortens
     to match): the outgoing face simply fades over the incoming one,
     which sits at rest — no 3D anywhere. */
  @media (prefers-reduced-motion: reduce) {
    .companion.tile-pre .wellpane {
      transform: none;
    }
    .companion.tile-enter .wellpane {
      animation: orbitty-tile-fade 140ms ease-out both;
    }
    .companion.tile-pre :global(.wellhead .floatwrap),
    .companion.tile-enter :global(.wellhead .floatwrap) {
      transform: none;
      animation: none;
    }
    .wellpane.turning .prismface.face-out {
      animation: orbitty-tile-fade-out 140ms ease-out both;
    }
    /* face-in stays at rest here — the outgoing fade over it IS the
       crossfade. */
    .wellpane.turning .wellpage.face-in {
      animation: none;
    }
  }

  @keyframes orbitty-tile-fade-out {
    from {
      opacity: 1;
    }
    to {
      opacity: 0;
    }
  }

  @keyframes orbitty-tile-fade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  /* SPRITE-OFF hosted: no .wellhead — the pane takes the full well. The
     card keeps its full ring (the old divider drop died with the
     divider). */

  /* SPRITE-HUG: the old spriteoff ::before flex spacer is dead — the
     native hug shrinks the idle collapsed frame to the CC+dock bar
     (keep-bottom), so the frame IS the chrome and there is no tall
     empty pane left to pin against. The bar stays ghost: empty air in
     it still clicks through via the reported hit list. */

  /* Transcript / CC / wait mark live
     in CaptionPane.svelte. */

  /* Prompt strip: one quiet menu when Prompts is On and
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

  /* Dock styles (.ghostdock, agentrow,
     dests, icon toggles, gear, composer, talk slot) moved to
     CompanionDock.svelte with the markup. */

  .claim-toast {
    position: fixed;
    left: 50%;
    bottom: 18px;
    transform: translateX(-50%);
    z-index: 80;
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 8px 12px;
    border-radius: 10px;
    background: rgba(20, 22, 28, 0.92);
    color: #f2f4f8;
    font-size: 12px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
  }
  .claim-toast button {
    appearance: none;
    border: 0;
    background: rgba(255, 255, 255, 0.12);
    color: inherit;
    border-radius: 6px;
    padding: 4px 8px;
    font-size: 12px;
    cursor: pointer;
  }
  .claim-toast button:hover {
    background: rgba(255, 255, 255, 0.2);
  }
</style>
