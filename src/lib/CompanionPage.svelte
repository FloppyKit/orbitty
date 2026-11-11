<script lang="ts">
  // Spark mouth + Spirit. No model in this page. Native resize only
  // (never frontend set_size). Persist the whole companion object.
  // Spirit is default; 3D mounts only while a 3D body is picked.
  import { invoke } from "@tauri-apps/api/core";
  import { emitTo, listen } from "@tauri-apps/api/event";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount, onDestroy, tick } from "svelte";
  import {
    applyHostTokens,
    applyWindowOpacity,
    clampWindowOpacity,
  } from "../themes/apply";
  import { listenPaneVis } from "./paneVis";
  import { maybeGraduatePeerChrome, peerChromeOn } from "./peerChrome";
  import CaptionPane from "./CaptionPane.svelte";
  import CompanionDock from "./CompanionDock.svelte";
  import CompToast from "./companion/Toast.svelte";
  import CompNotes from "./companion/Notes.svelte";
  import CompSpirit from "./companion/Spirit.svelte";
  import CompWell from "./companion/Well.svelte";
  import { eventElement } from "./dragPane";
  import WindowTitleRow from "./WindowTitleRow.svelte";
  import { chromeMode, navPlatform, navUa } from "./windowChrome";
  import { expandKey } from "./companion/expandKey";
  import { planetCss } from "./planetColor";
  import type { Action } from "./companion/launcher";

  import type {
    CompanionSlice,
    CompanionTurn,
    KanbanCard,
    PersistedState,
    StarsSlice,
  } from "./persist";
  import "./companion/page.css";
  import { measureHugHeight as measureHugHeightBody, reportHug as reportHugBody, reportHostHug as reportHostHugBody, actionRowExtra as actionRowExtraBody, reportComposerGrow as reportComposerGrowBody, toggleSprite as toggleSpriteBody, toggleCaptions as toggleCaptionsBody, togglePlanetsHidden as togglePlanetsHiddenBody, setSpeakVolume as setSpeakVolumeBody, toggleSpeak as toggleSpeakBody } from "./companion/pageHug.svelte";
  import { todayKey as todayKeyBody, toggleNextCard as toggleNextCardBody, togglePrompts as togglePromptsBody, ackStrip as ackStripBody, smallestTodo as smallestTodoBody, openSmallestTodo as openSmallestTodoBody, talkStop as talkStopBody, talkStart as talkStartBody, talkStopOnly as talkStopOnlyBody, talkStopSend as talkStopSendBody, applyTalkText as applyTalkTextBody, onNeededFocus as onNeededFocusBody, composerField as composerFieldBody, focusComposer as focusComposerBody, landCaret as landCaretBody, onNeededAnswer as onNeededAnswerBody, sendLine as sendLineBody, loadAgents as loadAgentsBody, stampThin as stampThinBody, onAgentChange as onAgentChangeBody } from "./companion/pageNeeded.svelte";
  import { toggleExpanded as toggleExpandedBody, enterHost as enterHostBody, dismissHost as dismissHostBody, refreshDestOpen as refreshDestOpenBody, toggleDest as toggleDestBody, popoutLaunchpad as popoutLaunchpadBody, popoutInspector as popoutInspectorBody, restartTimer as restartTimerBody, sliceSheet as sliceSheetBody, loadArt as loadArtBody, onGlbFail as onGlbFailBody, loadState as loadStateBody } from "./companion/pageExpand.svelte";
  import { startCompanionDrag as startCompanionDragBody, onDragMouseDown as onDragMouseDownBody, onDockMouseDown as onDockMouseDownBody } from "./companion/pageDrag.svelte";

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
  const is3D = $derived(bodyId === "female");

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
  let wellpaneEl: HTMLElement | null = $state(null);
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
    if (expandKey(e, { composing: e.isComposing }) === "toggle") {
      e.preventDefault();
      toggleExpanded();
      return;
    }
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
  // all three surfaces. Full/Head button: L-click Full ↔ Head; R-click
  // toggles sprite (preventDefault — no browser menu). Per-surface view
  // prefs are never written on sprite-off, so they survive Off.
  // Off draws no stage / exphead / wellhead — chat stays.
  let sprite = $state(true);

  // SPRITE-HUG: the bar height the native hug resizes to — CC rows (+
  // note / starstrip when present) down through the dock, bottom margin
  // included. Diffed from element edges, so it measures the same whether
  // the content is bottom-pinned by the live stage or already hugged.
  function measureHugHeight() {
    return measureHugHeightBody({get mainEl() { return mainEl; }, set mainEl(v) { mainEl = v; }, get overlayWinH() { return overlayWinH; }, set overlayWinH(v) { overlayWinH = v; }});
  }

  // SPRITE-HUG: idle collapsed + sprite off asks the native side to hug
  // (or re-fit) the bar. Flag-unchanged calls skip the persist/emit
  // native-side, so this is cheap on every collapsed layout change; the
  // native side no-ops while expanded/hosted/hidden or already fit.
  function reportHug() {
    return reportHugBody({get expanded() { return expanded; }, set expanded(v) { expanded = v; }, get hosted() { return hosted; }, set hosted(v) { hosted = v; }, invoke, measureHugHeight, get sprite() { return sprite; }, set sprite(v) { sprite = v; }});
  }

  // Hosted well gets a comfortable height (dock stays). Launchpad /
  // Planets scroll inside; STARS fills the well. Not a content-hug.
  const HOST_COMFORT_H = 820;
  let lastHostHug = 0;
  let hugTimer: ReturnType<typeof setTimeout> | undefined;
  function reportHostHug() {
    return reportHostHugBody({HOST_COMFORT_H, get expanded() { return expanded; }, set expanded(v) { expanded = v; }, get hosted() { return hosted; }, set hosted(v) { hosted = v; }, get hugTimer() { return hugTimer; }, set hugTimer(v) { hugTimer = v; }, invoke, get lastHostHug() { return lastHostHug; }, set lastHostHug(v) { lastHostHug = v; }, get swapPrev() { return swapPrev; }, set swapPrev(v) { swapPrev = v; }, get tileEnter() { return tileEnter; }, set tileEnter(v) { tileEnter = v; }, get tilePre() { return tilePre; }, set tilePre(v) { tilePre = v; }});
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
  function actionRowExtra() {
    return actionRowExtraBody({get cheatSheetHint() { return cheatSheetHint; }, set cheatSheetHint(v) { cheatSheetHint = v; }, get mainEl() { return mainEl; }, set mainEl(v) { mainEl = v; }});
  }

  function reportComposerGrow() {
    return reportComposerGrowBody({actionRowExtra, get composerExtra() { return composerExtra; }, set composerExtra(v) { composerExtra = v; }, invoke, get overlayExtra() { return overlayExtra; }, set overlayExtra(v) { overlayExtra = v; }});
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
    return toggleSpriteBody({get expanded() { return expanded; }, set expanded(v) { expanded = v; }, get hosted() { return hosted; }, set hosted(v) { hosted = v; }, invoke, measureHugHeight, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }, get sprite() { return sprite; }, set sprite(v) { sprite = v; }}, e);
  }

  const viewTitle = $derived(
    !sprite
      ? "Sprite hidden. Click: Head or full body · Right-click: show avatar"
      : bustOk
        ? "Click: Head or full body · Right-click: mute avatar"
        : "Click: full body (no bust art) · Right-click: mute avatar",
  );

  let captions = $state(true);

  async function toggleCaptions() {
    return toggleCaptionsBody({get captions() { return captions; }, set captions(v) { captions = v; }, get expanded() { return expanded; }, set expanded(v) { expanded = v; }, invoke, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }});
  }

  let planetsHidden = $state(false);
  let cheatSheetHint = $state(true);
  let peerChrome = $state(false);
  async function togglePlanetsHidden() {
    return togglePlanetsHiddenBody({invoke, get planetsHidden() { return planetsHidden; }, set planetsHidden(v) { planetsHidden = v; }, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }});
  }

  let speakVolume = $state(1);
  let volOpen = $state(false);
  async function setSpeakVolume(v: number) {
    return setSpeakVolumeBody({invoke, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }, get speak() { return speak; }, set speak(v) { speak = v; }, get speakVolume() { return speakVolume; }, set speakVolume(v) { speakVolume = v; }, toggleSpeak}, v);
  }

  // S2.7: Speak toggle (persisted companion.speakReplies, default off).
  // When on, each landed bot reply is read aloud by the native
  // `speak_reply` (macOS `say`, default system voice); the native side
  // kills any in-flight `say` before the next and when the toggle goes
  // off. Never the user's line, never an error line.
  let speak = $state(false);

  async function toggleSpeak() {
    return toggleSpeakBody({invoke, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }, get speak() { return speak; }, set speak(v) { speak = v; }});
  }

  // Prompts strip + hold-to-talk

  const STARS_SPOKES = [
    { key: "sleep", label: "Sleep" },
    { key: "terroir", label: "Terroir" },
    { key: "athleticism", label: "Athleticism" },
    { key: "relationships", label: "Relationships" },
    { key: "self", label: "Self" },
  ] as const;

  function todayKey() {
    return todayKeyBody({});
  }

  let stars = $state<StarsSlice>({ promptsOptIn: false, checkins: {} });
  let cards = $state<KanbanCard[]>([]);
  let planets = $state<{ id: string; name: string; dot?: string }[]>([]);
  // Quiet dock pip. Count = open/later-due
  // Needed only (not todos). Zero hides the pip; ⌘⇧N still toggles.
  let nextCount = $derived(
    cards.filter((c) => {
      if (c.kind !== "needed") return false;
      return c.neededState === "open" || c.neededState === "later";
    }).length,
  );
  function toggleNextCard() {
    return toggleNextCardBody({invoke});
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
    return togglePromptsBody({invoke, get prompts() { return prompts; }, set prompts(v) { prompts = v; }, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }});
  }

  // Rest / Water / × — an ack, not a lecture: hide until the day rolls or
  // a fresh check-in lands.
  async function ackStrip() {
    return ackStripBody({invoke, get promptAckKey() { return promptAckKey; }, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }, get stars() { return stars; }, set stars(v) { stars = v; }});
  }

  // Smallest todo: first undone subtask on the first Doing card, else that
  // card's title, else an honest "nothing open". Opens Lists on the card.
  function smallestTodo() {
    return smallestTodoBody({get cards() { return cards; }, set cards(v) { cards = v; }});
  }

  async function openSmallestTodo() {
    return openSmallestTodoBody({ackStrip, emitTo, enterHost, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }, smallestTodo});
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
    return talkStopBody({get fieldEl() { return fieldEl; }, set fieldEl(v) { fieldEl = v; }, invoke, get talking() { return talking; }, set talking(v) { talking = v; }});
  }

  async function talkStart(e: Event) {
    return talkStartBody({get draft() { return draft; }, set draft(v) { draft = v; }, get fieldEl() { return fieldEl; }, set fieldEl(v) { fieldEl = v; }, invoke, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }, get talkBase() { return talkBase; }, set talkBase(v) { talkBase = v; }, get talking() { return talking; }, set talking(v) { talking = v; }}, e);
  }

  // Check: stop only — the text stays in the field for review + Send.
  async function talkStopOnly() {
    return talkStopOnlyBody({get sendOnTalkEnd() { return sendOnTalkEnd; }, set sendOnTalkEnd(v) { sendOnTalkEnd = v; }, talkStop});
  }

  // Up-arrow: stop, then send immediately — no second confirm.
  async function talkStopSend() {
    return talkStopSendBody({sendLine, get sendOnTalkEnd() { return sendOnTalkEnd; }, set sendOnTalkEnd(v) { sendOnTalkEnd = v; }, talkStop, get talking() { return talking; }, set talking(v) { talking = v; }});
  }

  function applyTalkText(text: string) {
    return applyTalkTextBody({get draft() { return draft; }, set draft(v) { draft = v; }, get fieldEl() { return fieldEl; }, set fieldEl(v) { fieldEl = v; }, get talkBase() { return talkBase; }, set talkBase(v) { talkBase = v; }}, text);
  }

  let neededFocusId = $state<string | null>(null);
  let neededFocusTitle = $state<string | null>(null);
  // Plain let so onFocusChanged (registered once) sees the live hook.
  let mouthHooked = false;
  function onNeededFocus(id: string | null, title: string | null) {
    return onNeededFocusBody({landCaret, get mouthHooked() { return mouthHooked; }, set mouthHooked(v) { mouthHooked = v; }, get neededFocusId() { return neededFocusId; }, set neededFocusId(v) { neededFocusId = v; }, get neededFocusTitle() { return neededFocusTitle; }, set neededFocusTitle(v) { neededFocusTitle = v; }}, id, title);
  }

  function composerField() {
    return composerFieldBody({get fieldEl() { return fieldEl; }, set fieldEl(v) { fieldEl = v; }});
  }

  function focusComposer() {
    return focusComposerBody({composerField});
  }

  // Native raise (arm_companion_mouth / ⌘⇧O show_companion) makes this
  // the key window; the caret still has to land in .field after that.
  // One tick is too early — Launchpad just had key, and fieldEl can be
  // stale until the dock paints the placeholder. Retry until it sticks.
  async function landCaret() {
    return landCaretBody({focusComposer, tick});
  }

  // Enter on a Needed row arms the composer
  // answer (onNeededFocus already set the hint) — land the caret so the
  // next typed line answers it. Esc blurs back out (onPaneKey), arrows
  // resume the ring.
  function onNeededAnswer() {
    return onNeededAnswerBody({landCaret});
  }


  async function sendLine() {
    return sendLineBody({ACK_SPIN_MS, get ackLine() { return ackLine; }, set ackLine(v) { ackLine = v; }, get ackReveal() { return ackReveal; }, set ackReveal(v) { ackReveal = v; }, get attach() { return attach; }, set attach(v) { attach = v; }, get botLine() { return botLine; }, set botLine(v) { botLine = v; }, get draft() { return draft; }, set draft(v) { draft = v; }, invoke, get neededFocusId() { return neededFocusId; }, set neededFocusId(v) { neededFocusId = v; }, get pendingAck() { return pendingAck; }, set pendingAck(v) { pendingAck = v; }, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }, get sending() { return sending; }, set sending(v) { sending = v; }, get turns() { return turns; }, set turns(v) { turns = v; }, get userLine() { return userLine; }, set userLine(v) { userLine = v; }});
  }

  async function loadAgents() {
    return loadAgentsBody({get agent() { return agent; }, set agent(v) { agent = v; }, get agents() { return agents; }, set agents(v) { agents = v; }, invoke});
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
  const companionMode = $derived(AGENT_MODE[agent] ?? "");
  const smallest = $derived(smallestTodo());
  const hostBusy = $derived(tilePre || tileEnter || swapPrev !== null);

  async function stampThin() {
    return stampThinBody({invoke, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }});
  }

  async function onAgentChange() {
    return onAgentChangeBody({AGENT_MODE, get agent() { return agent; }, set agent(v) { agent = v; }, invoke, loadAgents, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }});
  }



  function toggleExpanded() {
    return toggleExpandedBody({get captions() { return captions; }, set captions(v) { captions = v; }, get expanded() { return expanded; }, set expanded(v) { expanded = v; }, invoke});
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
    return enterHostBody({DEST_RING, TILE_ENTER_MS, TILE_YAW_MS, get expanded() { return expanded; }, set expanded(v) { expanded = v; }, get hosted() { return hosted; }, set hosted(v) { hosted = v; }, invoke, get mainEl() { return mainEl; }, set mainEl(v) { mainEl = v; }, get prismR() { return prismR; }, set prismR(v) { prismR = v; }, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }, get spiritHold() { return spiritHold; }, set spiritHold(v) { spiritHold = v; }, get sprite() { return sprite; }, set sprite(v) { sprite = v; }, get swapDir() { return swapDir; }, set swapDir(v) { swapDir = v; }, get swapPrev() { return swapPrev; }, set swapPrev(v) { swapPrev = v; }, tick, get tileEnter() { return tileEnter; }, set tileEnter(v) { tileEnter = v; }, get tileGen() { return tileGen; }, set tileGen(v) { tileGen = v; }, get tilePre() { return tilePre; }, set tilePre(v) { tilePre = v; }, tileReset, get wellpaneEl() { return wellpaneEl; }, set wellpaneEl(v) { wellpaneEl = v; }}, mode);
  }

  async function dismissHost() {
    return dismissHostBody({TILE_LEAVE_MS, get hosted() { return hosted; }, set hosted(v) { hosted = v; }, invoke, get lastDest() { return lastDest; }, set lastDest(v) { lastDest = v; }, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }, get spiritHold() { return spiritHold; }, set spiritHold(v) { spiritHold = v; }, get swapPrev() { return swapPrev; }, set swapPrev(v) { swapPrev = v; }, get tileEnter() { return tileEnter; }, set tileEnter(v) { tileEnter = v; }, get tileGen() { return tileGen; }, set tileGen(v) { tileGen = v; }, get tileLeave() { return tileLeave; }, set tileLeave(v) { tileLeave = v; }, get tilePre() { return tilePre; }, set tilePre(v) { tilePre = v; }});
  }

  async function refreshDestOpen() {
    return refreshDestOpenBody({get destOpen() { return destOpen; }, set destOpen(v) { destOpen = v; }, invoke, get stars() { return stars; }, set stars(v) { stars = v; }});
  }

  // Dest chips are lightswitches for real windows. The companion
  // stays idle — never enterHost. Click while open hides; click
  // again shows and orders front.
  async function toggleDest(mode: "launchpad" | "stars" | "inspector") {
    return toggleDestBody({dismissHost, get hosted() { return hosted; }, set hosted(v) { hosted = v; }, invoke, refreshDestOpen, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }}, mode);
  }

  async function runPage(fn: string, args?: Record<string, unknown>) {
    if (fn === "openSmallestTodo") return openSmallestTodo();
    if (fn === "togglePrompts") return togglePrompts();
    if (fn === "togglePlanetsHidden") return togglePlanetsHidden();
    if (fn === "toggleExpanded") return toggleExpanded();
    if (fn === "toggleSpeak") return toggleSpeak();
    if (fn === "toggleCaptions") return toggleCaptions();
    if (fn === "toggleView") return toggleView();
    if (fn === "toggleSprite") {
      return toggleSprite({ preventDefault() {} } as MouseEvent);
    }
    if (fn === "talkStart") return talkStart(new Event("click"));
    if (fn === "pickAgent") {
      const id = typeof args?.id === "string" ? args.id : "";
      if (!id) return;
      agent = id;
      return onAgentChange();
    }
    throw new Error(`Unknown action ${fn}`);
  }

  async function runAction(action: Action): Promise<boolean> {
    try {
      if (action.t === "send" || action.t === "answer") {
        await sendLine();
        return true;
      }
      if (action.t === "invoke") {
        await invoke(action.cmd, action.args ?? {});
        return true;
      }
      if (action.t === "host") {
        await enterHost(action.mode);
        return true;
      }
      if (action.t === "page") {
        await runPage(action.fn, action.args);
        return true;
      }
      return false;
    } catch (e) {
      sendNote = `${e}`;
      return false;
    }
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
    return popoutLaunchpadBody({get hosted() { return hosted; }, set hosted(v) { hosted = v; }, invoke, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }, get tileGen() { return tileGen; }, set tileGen(v) { tileGen = v; }, tileReset});
  }

  // Same pop-out pattern — dismiss the well,
  // then show the real rail window. Tray Inspector still show_rail too.
  async function popoutInspector() {
    return popoutInspectorBody({get hosted() { return hosted; }, set hosted(v) { hosted = v; }, invoke, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }, get tileGen() { return tileGen; }, set tileGen(v) { tileGen = v; }, tileReset});
  }

  function restartTimer() {
    return restartTimerBody({get drawFrames() { return drawFrames; }, get frameIdx() { return frameIdx; }, set frameIdx(v) { frameIdx = v; }, get frameMs() { return frameMs; }, set frameMs(v) { frameMs = v; }, get timer() { return timer; }, set timer(v) { timer = v; }});
  }

  // Sheet → frames: 1:1 canvas blits (no resample, no redraw).
  async function sliceSheet(sheet: CompanionSheet) {
    return sliceSheetBody({}, sheet);
  }

  // Spirit when imagePackId is empty; else a role:companion pack.
  // Missing pack fails soft to the blessed spirit (never a blank window).
  // COMPANION-3D: reserved 3D ids load the SPIRIT art — it is the warm
  // fallback under Companion3D (and what a 3D failure lands on).
  async function loadArt() {
    return loadArtBody({get artLoaded() { return artLoaded; }, set artLoaded(v) { artLoaded = v; }, get artSmooth() { return artSmooth; }, set artSmooth(v) { artSmooth = v; }, get bodyId() { return bodyId; }, set bodyId(v) { bodyId = v; }, get frameIdx() { return frameIdx; }, set frameIdx(v) { frameIdx = v; }, get frameMs() { return frameMs; }, set frameMs(v) { frameMs = v; }, invoke, get missing() { return missing; }, set missing(v) { missing = v; }, get restBust() { return restBust; }, set restBust(v) { restBust = v; }, get restFrames() { return restFrames; }, set restFrames(v) { restFrames = v; }, restartTimer, sliceSheet, get thinkBust() { return thinkBust; }, set thinkBust(v) { thinkBust = v; }, get thinkFrames() { return thinkFrames; }, set thinkFrames(v) { thinkFrames = v; }});
  }

  // COMPANION-3D fail-soft: WebGL2 missing / loader error / missing GLB →
  // revert the pick to the blessed spirit (persisted, so Settings follows)
  // with one quiet line. Never a black stage, never a crash.
  async function onGlbFail() {
    return onGlbFailBody({get bodyId() { return bodyId; }, set bodyId(v) { bodyId = v; }, get glbFailed() { return glbFailed; }, set glbFailed(v) { glbFailed = v; }, invoke, get sendNote() { return sendNote; }, set sendNote(v) { sendNote = v; }});
  }

  async function loadState() {
    return loadStateBody({KNOWN_THEMES, get artLoaded() { return artLoaded; }, set artLoaded(v) { artLoaded = v; }, get bodyId() { return bodyId; }, set bodyId(v) { bodyId = v; }, get botLine() { return botLine; }, set botLine(v) { botLine = v; }, get captionReady() { return captionReady; }, set captionReady(v) { captionReady = v; }, get captions() { return captions; }, set captions(v) { captions = v; }, get cards() { return cards; }, set cards(v) { cards = v; }, get cheatSheetHint() { return cheatSheetHint; }, set cheatSheetHint(v) { cheatSheetHint = v; }, clampWindowOpacity, get expanded() { return expanded; }, set expanded(v) { expanded = v; }, get figureView() { return figureView; }, set figureView(v) { figureView = v; }, get glbFailed() { return glbFailed; }, set glbFailed(v) { glbFailed = v; }, get hackLive() { return hackLive; }, set hackLive(v) { hackLive = v; }, get hackWho() { return hackWho; }, set hackWho(v) { hackWho = v; }, invoke: invokeKeepingPlanets, loadArt, maybeGraduatePeerChrome, osThemeId, get peerChrome() { return peerChrome; }, set peerChrome(v) { peerChrome = v; }, peerChromeOn, get planetsHidden() { return planetsHidden; }, set planetsHidden(v) { planetsHidden = v; }, get prompts() { return prompts; }, set prompts(v) { prompts = v; }, refreshDestOpen, get sending() { return sending; }, set sending(v) { sending = v; }, get speak() { return speak; }, set speak(v) { speak = v; }, get speakVolume() { return speakVolume; }, set speakVolume(v) { speakVolume = v; }, get sprite() { return sprite; }, set sprite(v) { sprite = v; }, get stars() { return stars; }, set stars(v) { stars = v; }, get themeId() { return themeId; }, set themeId(v) { themeId = v; }, get turns() { return turns; }, set turns(v) { turns = v; }, get userLine() { return userLine; }, set userLine(v) { userLine = v; }, get windowOpacity() { return windowOpacity; }, set windowOpacity(v) { windowOpacity = v; }});
  }

  function invokeKeepingPlanets<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    return invoke<T>(cmd, args).then((result) => {
      if (
        cmd === "load_state" &&
        result &&
        typeof result === "object" &&
        "projects" in result
      ) {
        const projects =
          (result as { projects?: { id?: string; name?: string }[] }).projects ?? [];
        planets = projects.flatMap((p) =>
          p && typeof p.id === "string" && p.id
            ? [{ id: p.id, name: (p.name ?? "").trim() || "Planet", dot: planetCss(p.id) }]
            : [],
        );
      }
      return result;
    });
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
    return startCompanionDragBody({invoke}, e);
  }

  function onDragMouseDown(e: MouseEvent) {
    return onDragMouseDownBody({startCompanionDrag}, e);
  }

  function onDockMouseDown(e: MouseEvent | PointerEvent) {
    return onDockMouseDownBody({eventElement, startCompanionDrag}, e);
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
    windowTitle?: string;
    seconds: number;
  } | null>(null);
  let claimToastTimer: ReturnType<typeof setTimeout> | undefined;
  let unlistenClaimToast: (() => void) | undefined;

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
      // Path A GATE Hang-first: arm CHATTER/STOP only while companion focused.
      void invoke("interrupt_gate_set_armed", { armed: !!e.payload });
      if (e.payload && mouthHooked) focusComposer();
    });
    unlistenKanbanVis = await listenPaneVis("kanban", (visible) => {
      if (!visible) onNeededFocus(null, null);
    });
    // Tray Launchpad/STARS raise this pane
    // native-side and ask the well to open — raise-only via dest_show,
    // never toggleDest (launcher rows open; ⌘⌥ chords still toggle).
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
    unlistenClaimToast = await listen<{
      projectId: string;
      projectName: string;
      windowId: string;
      windowTitle?: string;
      seconds: number;
    }>("orbitty:claim-toast", (e) => {
      const p = e.payload;
      if (!p?.projectName) return;
      claimToast = {
        projectId: p.projectId,
        projectName: p.projectName,
        windowId: p.windowId,
        windowTitle: p.windowTitle || "",
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

<CompToast bind:claimToast bind:claimToastTimer />

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
    onHandoffReply={(reply) => {
      botLine = reply;
    }}
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
  class:chrome={expanded}
  bind:this={mainEl}
>
  {#snippet collapseToDock()}
    <button
      type="button"
      class="collapse-dock"
      data-no-drag
      aria-label="Collapse to dock"
      title="Collapse to dock"
      onmousedown={(e) => e.stopPropagation()}
      onclick={toggleExpanded}
    >
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
    </button>
  {/snippet}
  {#if expanded}
    <WindowTitleRow
      title="Orbitty"
      mode={chromeMode(navUa(), navPlatform())}
      label="companion"
      closeLabel="Hide Orbitty"
      {...{ chips: collapseToDock }}
      onClose={() => invoke("hide_window", { label: "companion" }).catch(() => {})}
    />
  {/if}
  {#if expanded && sprite}
    <CompSpirit
      corner
      {headShowing}
      {bodyId}
      {sending}
      {is3D}
      {glbFailed}
      {missing}
      {drawFrames}
      {frameIdx}
      {artSmooth}
      {onDragMouseDown}
      {onGlbFail}
    />
  {/if}

  <CompWell
    {hosted}
    {sprite}
    {swapPrev}
    {swapDir}
    {prismR}
    bind:wellpaneEl
    {expanded}
    {captions}
    {caption}
    {dismissHost}
    {popoutLaunchpad}
    {popoutInspector}
    {onNeededFocus}
    {onNeededAnswer}
    {createPlanetTick}
    {headShowing}
    {bodyId}
    {sending}
    {is3D}
    {glbFailed}
    {missing}
    {drawFrames}
    {frameIdx}
    {artSmooth}
    {onDragMouseDown}
    {onGlbFail}
  />

  {#if !expanded}
    <div class="dockchat">
      {#if sprite && (hosted === "off" || spiritHold)}
        <CompSpirit
          {headShowing}
          {bodyId}
          {sending}
          {is3D}
          {glbFailed}
          {missing}
          {drawFrames}
          {frameIdx}
          {artSmooth}
          {onDragMouseDown}
          {onGlbFail}
        />
      {/if}
      <div class="dockchat-copy">
        {#if captions}
          {@render caption(false, hosted !== "off")}
        {/if}
        <CompNotes
          bind:sendNote
          {showPromptStrip}
          {lowestSpoke}
          {ackStrip}
          {openSmallestTodo}
        />
      </div>
    </div>
  {:else}
    <CompNotes
      bind:sendNote
      {showPromptStrip}
      {lowestSpoke}
      {ackStrip}
      {openSmallestTodo}
    />
  {/if}

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
    {planets}
    neededId={neededFocusId}
    {hostBusy}
    {smallest}
    {captions}
    mode={companionMode}
    {runAction}
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
    {nextCount}
    {toggleNextCard}
    {toggleSpeak}
    {toggleCaptions}
    {setSpeakVolume}
    {toggleSprite}
    {toggleView}
    {toggleExpanded}
    {sendLine}
    {talkStart}
    {talkStopOnly}
    {talkStopSend}
    {onDockMouseDown}
  />

</main>

