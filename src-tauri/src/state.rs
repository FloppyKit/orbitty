//! S2 persistence — explicit claim state, JSON file under the app data dir.
//! S4 adds: `claims[].lastTouchedAt` (zombie aging), `kanbanCards`,
//! `pointsLedger` (local Kanban SoT + capped points, same file, additive).
//! S4.2 adds `windowOpacity` (0.35–1.0 background-fill alpha, default 1.0).
//! GO-ORBITTY-CHARS adds `claims[].charSeed` / `charLook` / `spriteScale`
//! (generated person + per-walker size; all optional, all additive).
//! GO-ORBITTY-SLEEPERS adds `claims[].filePath` / `hermesProfile` /
//! `hermesSession` / `actionId` / `command` / `cwd` — the wake trail for a
//! closed claimed window (all optional, all additive).
//! Shape: `{ themeId, windowOpacity, projects: [{id,name}],
//! claims: [{windowId,projectId,lastTouchedAt}],
//! tags: [{id,name,color?}], windowTags: [{windowId,tagIds}],
//! collapsedSectionIds: [sectionId], nudgeSuppressedWindowIds: [windowId],
//! kanbanCards: [{id,title,projectId,windowId?,kind?,status,createdAt,
//! lastTouchedAt,completedAt?,goalAligned,clinicalOrMaster,pointsAwarded,
//! starred, notes?, difficulty?, subtasks?, dueAt?, tagIds?,
//! neededState?, neededWhy?, neededAnswer?, actor?}],
//! pointsLedger: [{id,cardId,amount,day,at,actor?}],
//! titleOverrides: [{windowId,title}] }`.
//! Settings slice adds `autohideRowChrome` (default true) / `hideTitles`
//! (default false; legacy `showFullTitles` is ignored — old saves load with
//! titles visible, one line) and the user-authored shortcut lanes:
//! `actionShortcuts: [{id,name,command,cwd?,combo?}]`,
//! `agentShortcuts: [{id,name,harness,profile,model?,mdPaths[],projectId?}]`.
//! S4.4 adds the Habitica-thin detail fields on cards (all optional, all
//! additive — old saves load, and the rail's saves must not strip them).
//! Polish slice adds `titleOverrides: [{windowId,title}]` — display-only
//! renames written by the rail alone; an absent/empty entry means the live
//! AX title shows. AX/OS titles are never touched.
//! GO-LS-TAURI-AVATAR-LARGE adds `companion: {visible, bounds, imagePackId,
//! state}` — the large companion window (S2), written native-side (bounds/
//! visible) only. S2.1: `visible` defaults true (first run opens with the
//! app); `imagePackId` is legacy — the window always draws the blessed
//! spirit, the field remains so old saves load.
//! GO-LS-STARS-STATUS adds `stars: {promptsOptIn, checkins, promptsAck}` —
//! the explicit daily check-in radar (Sleep · Terroir · Athleticism ·
//! Relationships · Self, 0–4 each, keyed by local YYYY-MM-DD). No PHI:
//! five small self-reported integers per day, written only by the rail's
//! Save today. All fields default so old saves load.
//! GO-ORBITTY-STARS-AVERAGE adds `stars.averageWindow` ("week" | "14" |
//! "month", default "14") — which rolling window the mean line under the
//! chip strip shows. Missing on old saves → "14" via the slice Default.
//! GO-ORBITTY-MARBLE-DRIFT adds `marbleDrift` (default false) — the
//! Settings → Appearance toggle that lets nearby collapsed marbles slowly
//! drift. Off means marbles sit exactly where they were left.
//! GO-ORBITTY-WORD-SKY adds `sky: {muted}` — the ONLY persisted piece of
//! the word sky. Tokens are recomputed from the bag (done titles/notes,
//! tag names, weak planet names, user companion lines) on every open and
//! are never stored.
//! GO-ORBITTY-BOARD-STARS adds `sharedPaneMode` ("board" | "stars",
//! default "board") — the last page mounted in the one shared 780×520
//! `kanban` overlay, so a reopen restores it.
//! GO-ORBITTY-RAIL-DRAWER adds `drawers` — the invisible-leash state for
//! the three chrome drawers (`companion` / `kanban` / `settings`) only.
//! Never `sky`, never `stars` (Board+STARS share `kanban`). A missing key
//! (old saves) means: companion starts plucked iff legacy
//! `companion.bounds` exists; every other pane starts docked.
//! GO-ORBITTY-LP-ACTOR adds `actor` ("user" | "agent" | "action" | "vps")
//! on `kanbanCards` + `pointsLedger` — who completed the quest. Optional,
//! additive; a missing actor is pre-actor history, which the HUD reads as
//! "user". Only the KanbanPage completeCard door stamps it today.
//! GO-ORBITTY-SPRITE-OFF adds `companion.sprite` (default true) — R-click
//! on the Full/Head button hides the companion sprite while chat stays.
//! One key for collapsed, expanded, and hosted; a missing key (old saves)
//! loads true.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct PersistedState {
    pub theme_id: String,
    /// Window background-fill alpha (S4.2). 1.0 = fully opaque; the slider
    /// floor is 0.35. Never fades text/icons.
    pub window_opacity: f64,
    pub projects: Vec<Project>,
    pub claims: Vec<Claim>,
    pub tags: Vec<Tag>,
    pub window_tags: Vec<WindowTags>,
    pub collapsed_section_ids: Vec<String>,
    /// Claim-first nudge is shown once per window id, then silent forever.
    pub nudge_suppressed_window_ids: Vec<String>,
    pub kanban_cards: Vec<KanbanCard>,
    pub points_ledger: Vec<PointsEntry>,
    /// Display-only window title renames (rail is the only writer).
    pub title_overrides: Vec<TitleOverride>,
    /// Settings: hide circle/stow/folder chrome until the row is hovered.
    pub autohide_row_chrome: bool,
    /// Settings: hide window titles on rows (icon + chrome only). Legacy
    /// `showFullTitles` is ignored — both old values mean titles visible.
    pub hide_titles: bool,
    /// Settings: hide app icons on window rows (layout collapses).
    pub hide_app_icons: bool,
    /// Actions lane: user-authored shell shortcuts, run via /bin/zsh -lc.
    pub action_shortcuts: Vec<ActionShortcut>,
    /// Agents lane: user-authored harness shortcuts; Orbitty never runs a
    /// model itself — launch goes to the external harness CLI.
    pub agent_shortcuts: Vec<AgentShortcut>,
    /// Gear walker-size slider (0.4–2.5). 0 / missing = 1.0.
    pub sprite_scale: f64,
    /// GO-ORBITTY-MARBLE-DRIFT: Settings → Appearance toggle. Default OFF —
    /// collapsed marbles sit where they were left until opted in.
    pub marble_drift: bool,
    /// GO-ORBITTY-YARD-WARN leftover: the Herding Cats confirm's source
    /// checkboxes — independent on/off, at least one must be on before
    /// Drop can arm. Tabs stay disabled in the pane until Tabbity lands.
    /// The legacy `yardSource` radio key is no longer written; old saves
    /// heal to windows on / tabs off via the Default impl below. No
    /// yardCats array: cats never persist.
    pub yard_windows: bool,
    pub yard_tabs: bool,
    /// GO-LS-TAURI-AVATAR-LARGE: the large companion window (S2). All fields
    /// default — old saves load, and other windows round-trip the slice.
    pub companion: CompanionState,
    /// GO-LS-STARS-STATUS: the operator-green radar. Explicit check-in only
    /// — no wearables, no dwell score, no inferred anything.
    pub stars: StarsState,
    /// GO-ORBITTY-WORD-SKY: mute list only. The bag is derived, never
    /// persisted; a missing key loads as an empty mute list.
    pub sky: SkyState,
    /// GO-ORBITTY-BOARD-STARS: which page the one shared 780×520 `kanban`
    /// overlay last mounted — "board" (KanbanPage) | "stars" (StarsPage).
    /// Written native-side before every show; a cold webview reconciles it
    /// via load_state on mount (PANE-404: the URL never carries the mode);
    /// a bare re-show restores it.
    pub shared_pane_mode: String,
    /// GO-ORBITTY-RAIL-DRAWER: leash state per drawer label. Missing labels
    /// default per `drawer_docked` — never strip old keys on save.
    pub drawers: std::collections::BTreeMap<String, DrawerState>,
}

/// GO-ORBITTY-RAIL-DRAWER: the only labels that can be drawers. The rail
/// is not a drawer; `sky` stays hide-not-destroy; Board+STARS are the one
/// `kanban` overlay; planets are never docked.
pub const DRAWER_LABELS: [&str; 3] = ["companion", "kanban", "settings"];

/// GO-ORBITTY-RAIL-DRAWER slice. `docked` = follow-translate on the rail
/// from the inboard slot plus (dx, dy); `bounds` is the plucked resting
/// rect (physical px). For `companion`, the legacy top-level
/// `companion.bounds` wins when both exist.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct DrawerState {
    #[serde(default = "drawer_docked_default")]
    pub docked: bool,
    pub dx: f64,
    pub dy: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<CompanionBounds>,
}

fn drawer_docked_default() -> bool {
    true
}

impl Default for DrawerState {
    fn default() -> Self {
        Self {
            docked: true,
            dx: 0.0,
            dy: 0.0,
            bounds: None,
        }
    }
}

/// The old-save law, shared by `PersistedState::drawer_docked` and the
/// native runtime cache: an explicit entry wins; a missing companion entry
/// means plucked iff legacy `companion.bounds` exists; every other missing
/// label means docked.
pub fn drawer_docked_from(
    drawers: &std::collections::BTreeMap<String, DrawerState>,
    companion_has_legacy_bounds: bool,
    label: &str,
) -> bool {
    if let Some(d) = drawers.get(label) {
        return d.docked;
    }
    if label == "companion" {
        return !companion_has_legacy_bounds;
    }
    true
}

impl PersistedState {
    /// Effective docked flag for a drawer label under the old-save law.
    pub fn drawer_docked(&self, label: &str) -> bool {
        drawer_docked_from(&self.drawers, self.companion.bounds.is_some(), label)
    }
}

// Hand-rolled so a missing field (or missing file) defaults window_opacity
// to fully opaque instead of f64's 0.0 (an invisible window).
impl Default for PersistedState {
    fn default() -> Self {
        Self {
            theme_id: String::new(),
            window_opacity: 1.0,
            projects: Vec::new(),
            claims: Vec::new(),
            tags: Vec::new(),
            window_tags: Vec::new(),
            collapsed_section_ids: Vec::new(),
            nudge_suppressed_window_ids: Vec::new(),
            kanban_cards: Vec::new(),
            points_ledger: Vec::new(),
            title_overrides: Vec::new(),
            autohide_row_chrome: true,
            hide_titles: false,
            hide_app_icons: false,
            action_shortcuts: Vec::new(),
            agent_shortcuts: Vec::new(),
            sprite_scale: 1.0,
            marble_drift: false,
            yard_windows: true,
            yard_tabs: false,
            companion: CompanionState::default(),
            stars: StarsState::default(),
            sky: SkyState::default(),
            shared_pane_mode: "board".to_string(),
            drawers: std::collections::BTreeMap::new(),
        }
    }
}

/// GO-ORBITTY-WORD-SKY slice. `muted` holds smashed words (lowercase
/// tokens); everything else about the sky is recomputed on open.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SkyState {
    pub muted: Vec<String>,
}

/// GO-LS-STARS-STATUS slice. `prompts_opt_in` defaults FALSE — the
/// companion's one-strip menu (Rest / Water / Smallest todo) stays silent
/// until the human opts in, in the STARS pane or on the companion.
/// `prompts_ack` is the strip dismissal stamp `"{day}:{sig}"` — one show
/// per day unless they check in again (a new sig re-arms the strip).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct StarsState {
    pub prompts_opt_in: bool,
    /// Local calendar day `YYYY-MM-DD` → that day's five spokes.
    pub checkins: std::collections::BTreeMap<String, StarsCheckin>,
    pub prompts_ack: String,
    /// GO-ORBITTY-STARS-AVERAGE: "week" | "14" | "month" — rolling window
    /// for the per-spoke mean line. Default "14"; read-only native side.
    pub average_window: String,
}

impl Default for StarsState {
    fn default() -> Self {
        Self {
            prompts_opt_in: false,
            checkins: std::collections::BTreeMap::new(),
            prompts_ack: String::new(),
            average_window: "14".to_string(),
        }
    }
}

/// One day's check-in: five spokes, 0–4 each. `self` is the fifth spoke
/// (Spiritual in the pane heading); renamed past the Rust keyword.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct StarsCheckin {
    pub sleep: u8,
    pub terroir: u8,
    pub athleticism: u8,
    pub relationships: u8,
    #[serde(rename = "self")]
    pub self_: u8,
}

/// GO-LS-TAURI-AVATAR-LARGE companion slice. Bounds are physical pixels
/// (written by the native move/resize/hide handlers, restored on show).
/// `image_pack_id`: None = blessed spirit (default). Some ids (e.g.
/// `companion-jacob`) are temp companion bodies in packs/ with
/// `role: companion`. Never a planet walker.
/// `visible` defaults TRUE: first run (no prior key) opens the companion
/// with the app; after that the native hide/show handlers persist it.
/// S2.2: `last_user_line` is the composer's latest sent line (true CC —
/// one caption row, not a transcript), written by `companion_send`.
/// S2.4 / GO-COMPANION-BERD: `last_bot_line` is the companion backend reply;
/// `agent_profile` is the companion soul label — Berd agent stem under
/// `~/.agents/agents` (default backend) or Hermes profile when
/// `ORBITTY_COMPANION_BACKEND=hermes` ("" = unset → grok when present);
/// `expanded` persists the transcript view; `turns` is the capped
/// transcript (last 40) the expanded view scrolls.
/// S2.7: `speak_replies` (default false) — read bot replies aloud via
/// macOS `say`.
/// GO-ORBITTY-COMPANION-HEADROOM: `view_collapsed` / `view_expanded` are
/// the per-surface Full/Head prefs ("full" | "head") — collapsed defaults
/// "full", expanded defaults "head". Two keys, never one global bool;
/// writing one surface never touches the other. Head draws the pack's
/// bust_* set; a missing bust sits on full (no 128px crop fallback).
/// GO-ORBITTY-SPRITE-OFF: `sprite` (default true) hides the sprite art on
/// every surface while chat stays; the Full/Head prefs survive Off
/// untouched, so turning the sprite back on restores each surface's view.
/// GO-ORBITTY-MODE-SPLIT adds `mode` ("spark" default | "chat" | "work" |
/// "hold") + `threads` (mode → capped transcript). `turns` remains the
/// ACTIVE thread; serde defaults load old saves, `heal_modes` parks a
/// pre-split transcript under spark.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct CompanionState {
    #[serde(default = "companion_visible_default")]
    pub visible: bool,
    /// None = never placed — first show lands at the trailing-edge default.
    pub bounds: Option<CompanionBounds>,
    pub image_pack_id: Option<String>,
    /// "silent" | "idle" | "needs_human". S2 ships idle-only (no pulse
    /// wiring); the field is forward-compat for the urgency slice.
    pub state: String,
    /// S2.2: latest line sent from the companion composer. "" = none yet.
    pub last_user_line: String,
    /// S2.4: latest reply from the companion backend. "" = none.
    pub last_bot_line: String,
    /// S2.4 / GO-COMPANION-BERD: Berd agent stem, Hermes profile name, or ""
    /// (unset — resolves at send/display time, never stored resolved).
    pub agent_profile: String,
    /// S2.4: expanded transcript view; restored on show.
    pub expanded: bool,
    /// S2.4: capped transcript (last 40 turns) for the expanded view.
    pub turns: Vec<CompanionTurn>,
    /// S2.7: read bot replies aloud via macOS `say` (default system
    /// voice). Default OFF.
    pub speak_replies: bool,
    /// GO-ORBITTY-SETTINGS-WIN: companion Talk source. `"system"` (default,
    /// macOS speech — engine is GO-ORBITTY-PTT-DICTATION) or `"hotkey"`
    /// (Handy PTT chord). Unknown values load as system.
    #[serde(default = "talk_source_default")]
    pub talk_source: String,
    /// GO-ORBITTY-COMPANION-HEADROOM: collapsed-surface view —
    /// "full" (default) | "head".
    #[serde(default = "companion_view_collapsed_default")]
    pub view_collapsed: String,
    /// GO-ORBITTY-COMPANION-HEADROOM: expanded-surface view —
    /// "full" | "head" (default).
    #[serde(default = "companion_view_expanded_default")]
    pub view_expanded: String,
    /// GO-ORBITTY-SPRITE-OFF: draw the companion sprite. One flag for
    /// collapsed, expanded, and hosted; missing key (old saves) = true.
    #[serde(default = "companion_sprite_default")]
    pub sprite: bool,
    /// GO-ORBITTY-MODE-SPLIT: the active companion mode — "spark"
    /// (default) | "chat" | "work" | "hold". A SECOND axis next to the
    /// four-cast agent pick (spark/grok/chaos-gremlin/fkt-homie stems
    /// stay): mode owns the transcript + the bus target, never a stem
    /// recast. Chat/Hold never get the GO bus.
    #[serde(default = "companion_mode_default")]
    pub mode: String,
    /// GO-ORBITTY-MODE-SPLIT: mode → parked transcript (cap 40 each).
    /// `turns` stays the ACTIVE thread (CaptionPane reads it); a mode
    /// switch parks turns under the old mode and loads the new mode's
    /// thread. Healed on load: missing map + a pre-split transcript →
    /// copied under "spark" (first run lands the old chat on Spark).
    #[serde(default)]
    pub threads: std::collections::BTreeMap<String, Vec<CompanionTurn>>,
}

/// S2.4: one saved companion turn. role: "you" | "them".
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CompanionTurn {
    pub role: String,
    pub text: String,
}

fn companion_visible_default() -> bool {
    true
}

fn talk_source_default() -> String {
    // GO-ORBITTY-PTT-DICTATION: shipping default is system speech.
    // A saved "hotkey" still wins so Matt's desk Handy path stays until
    // he picks System. Missing key (old saves) lands here.
    "system".into()
}

fn companion_view_collapsed_default() -> String {
    "full".into()
}

fn companion_view_expanded_default() -> String {
    "head".into()
}

fn companion_sprite_default() -> bool {
    true
}

pub fn companion_mode_default() -> String {
    "spark".into()
}

/// GO-ORBITTY-MODE-SPLIT: the four modes are the whole allowlist —
/// anything else is None so callers fail closed (never a silent spark).
pub fn normalize_companion_mode(raw: &str) -> Option<String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "spark" => Some("spark".into()),
        "chat" => Some("chat".into()),
        "work" => Some("work".into()),
        "hold" => Some("hold".into()),
        _ => None,
    }
}

/// One transcript cap (last 40 turns) for the active thread and every
/// parked mode thread.
pub const COMPANION_THREAD_CAP: usize = 40;

pub fn cap_companion_turns(turns: &mut Vec<CompanionTurn>) {
    if turns.len() > COMPANION_THREAD_CAP {
        let drop = turns.len() - COMPANION_THREAD_CAP;
        turns.drain(0..drop);
    }
}

impl CompanionState {
    /// GO-ORBITTY-MODE-SPLIT heal, run on every native load: an unknown /
    /// missing mode snaps to spark, and a pre-split save (no threads map,
    /// existing turns) parks its transcript under spark so the first run
    /// shows the old chat on the Spark chip.
    pub fn heal_modes(&mut self) {
        if normalize_companion_mode(&self.mode).is_none() {
            self.mode = companion_mode_default();
        }
        if self.threads.is_empty() && !self.turns.is_empty() {
            self.threads.insert(companion_mode_default(), self.turns.clone());
        }
    }

    /// Append one turn to the ACTIVE thread and its parked copy under the
    /// current mode — the two never drift, so a mode switch mid-thread
    /// loses nothing. Both capped at 40.
    pub fn push_turn(&mut self, role: &str, text: &str) {
        let t = CompanionTurn {
            role: role.into(),
            text: text.into(),
        };
        self.turns.push(t.clone());
        cap_companion_turns(&mut self.turns);
        let thread = self.threads.entry(self.mode.clone()).or_default();
        thread.push(t);
        cap_companion_turns(thread);
    }

    /// Switch the active mode: park the current turns under the old mode,
    /// load the new mode's thread (empty when never visited). Chat lines
    /// never leak into Spark — each mode's transcript is its own.
    pub fn switch_mode(&mut self, new_mode: &str) {
        let parked = std::mem::take(&mut self.turns);
        self.threads.insert(self.mode.clone(), parked);
        self.turns = self.threads.get(new_mode).cloned().unwrap_or_default();
        self.mode = new_mode.to_string();
    }
}

/// GO-ORBITTY-COMPANION-HEADROOM: only "full" / "head" are real; anything
/// else (old saves, typos) lands on that surface's default.
pub fn normalize_companion_view(raw: &str, default: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "full" => "full".into(),
        "head" => "head".into(),
        _ => default.into(),
    }
}

pub fn normalize_talk_source(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "hotkey" | "handy" => "hotkey".into(),
        _ => "system".into(),
    }
}

impl Default for CompanionState {
    fn default() -> Self {
        Self {
            visible: true,
            bounds: None,
            image_pack_id: None,
            state: "idle".into(),
            last_user_line: String::new(),
            last_bot_line: String::new(),
            agent_profile: String::new(),
            expanded: false,
            turns: Vec::new(),
            speak_replies: false,
            talk_source: talk_source_default(),
            view_collapsed: companion_view_collapsed_default(),
            view_expanded: companion_view_expanded_default(),
            sprite: true,
            mode: companion_mode_default(),
            threads: std::collections::BTreeMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CompanionBounds {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ActionShortcut {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// GO-ORBITTY-ACTION-FILE: optional global chord, e.g. "cmd+alt+m".
    /// Absent/empty = Spirit-only. Registered native-side on load and on
    /// every persist; reserved/taken chords are never registered (the
    /// Settings list shows `conflict`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combo: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AgentShortcut {
    pub id: String,
    pub name: String,
    /// "hermes" now; "cursor" when cursor-agent is on PATH.
    pub harness: String,
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub md_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TitleOverride {
    pub window_id: String,
    pub title: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Claim {
    pub window_id: String,
    pub project_id: String,
    /// Epoch seconds of last focus/restore/complete/frontmost tick; 0 = pre-S4
    /// save (the rail migrates it to first-seen time on load).
    #[serde(default)]
    pub last_touched_at: f64,
    // GO-LS-TAURI-USER-PACKS reboot-rematch fingerprint, stamped from the live
    // window at claim time (live AX title, never the display override). Live
    // ids die on reboot; these fields rebuild the stableKey so the claim can
    // adopt the same window's new id. All optional — pre-GO saves load clean.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_at_claim: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_index: Option<u32>,
    /// Sprite picker (claim menu): packs/<id> folder this window's walker
    /// draws from; absent/empty = the generated being (GO-ORBITTY-CHARS).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_pack_id: Option<String>,
    /// GO-ORBITTY-CHARS: the generated person's roll seed. Stamped at claim
    /// time (old saves are seeded on load), so a claim keeps the same being
    /// across relaunches instead of rerolling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char_seed: Option<u64>,
    /// Explicit layer picks once the Being editor touches them; absent =
    /// derived from char_seed. A picked image pack (sprite_pack_id) still
    /// wins the draw.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char_look: Option<crate::chars::CharLook>,
    /// Per-walker size (0.4–2.5); absent/0 = the gear's global spriteScale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_scale: Option<f64>,
    /// GO-ORBITTY-CLAIM-ONE: legacy occupancy-teleport leftover. Ignored —
    /// occupancy is `project_id` only (one windowId, one planet). The
    /// field stays additive so pre-CLAIM-ONE saves still load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walker_home: Option<String>,
    // GO-ORBITTY-SLEEPERS wake stamps (all additive — old saves load). A
    // closed claimed window stays on its planet as a sleeper; these fields
    // are what wake_sleeper walks through, in order, after a live rematch.
    /// AX document path (file:// only, existed at stamp time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// Hermes-class apps only: profile when known (never guessed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hermes_profile: Option<String>,
    /// Hermes-class apps only: the live AX title as the session hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hermes_session: Option<String>,
    /// Optional link to an actionShortcuts entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    /// Terminal-class recipe (only when AX honestly gives a cwd).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// Local Kanban SoT — S4.3: cards are born at claim time (`kind: "window"`,
/// status `doing`) or as freeform `kind: "todo"` from the Lists window.
/// Unclaim keeps the card as a project todo with `window_id` cleared.
/// `starred` is shape-only (no star UI this slice).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KanbanCard {
    pub id: String,
    pub title: String,
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>, // "window" | "todo" | "needed"
    pub status: String,       // "todo" | "doing" | "done" ("todo" dummy on Needed)
    pub created_at: f64,
    pub last_touched_at: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<f64>,
    pub goal_aligned: bool,
    pub clinical_or_master: bool,
    pub points_awarded: u32,
    #[serde(default)]
    pub starred: bool,
    // S4.4 detail-panel fields. `difficulty` is metadata only — it never
    // touches the points formula (1 / 2 clinical|master / cap 12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<String>, // "trivial" | "easy" | "medium" | "hard"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtasks: Option<Vec<Subtask>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_ids: Option<Vec<String>>,
    // GO-ORBITTY-LP-NEEDED: the Needed inbox (all additive — old saves
    // load). `due_at` doubles as the Later snooze stamp on Needed cards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needed_state: Option<String>, // "open" | "later" | "answered" | "skipped"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needed_why: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needed_answer: Option<String>,
    // GO-ORBITTY-LP-ACTOR: who completed the card ("user" | "agent" |
    // "action" | "vps"). Additive — missing on pre-actor saves, and the HUD
    // reads missing as "user" (pre-actor history was human).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Subtask {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub done: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PointsEntry {
    pub id: String,
    pub card_id: String,
    pub amount: u32,
    /// Local calendar day `YYYY-MM-DD` — the daily cap (12) sums on this.
    pub day: String,
    pub at: f64,
    /// GO-ORBITTY-LP-ACTOR: who acted on the complete that minted this row
    /// ("user" | "agent" | "action" | "vps"). Additive; missing = pre-actor
    /// human history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WindowTags {
    pub window_id: String,
    pub tag_ids: Vec<String>,
}

fn state_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir.join("state.json"))
}

pub fn load_state(app: &tauri::AppHandle) -> Result<PersistedState, String> {
    let path = state_path(app)?;
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let mut st: PersistedState = serde_json::from_str(&text)
                .map_err(|e| format!("parse {}: {e}", path.display()))?;
            // GO-ORBITTY-MODE-SPLIT: old saves heal here — mode defaults
            // spark, a pre-split transcript parks under spark.
            st.companion.heal_modes();
            Ok(st)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PersistedState::default()),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

pub fn save_state(app: &tauri::AppHandle, state: &PersistedState) -> Result<(), String> {
    let path = state_path(app)?;
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(state).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("rename to {}: {e}", path.display()))?;
    Ok(())
}

/// GO-LS-TAURI-CUTOVER one-time migration: if the new `space.orbitty.mac`
/// app-data dir has no state.json yet and the old `app.livesprite.mac` dir
/// does, copy state + souls/context/packs over once. Never overwrites an
/// existing Orbitty install; the old dir is left in place untouched.
pub fn migrate_app_support(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Ok(new_dir) = app.path().app_data_dir() else {
        return;
    };
    if new_dir.join("state.json").exists() {
        return;
    }
    let Some(parent) = new_dir.parent().map(|p| p.to_path_buf()) else {
        return;
    };
    let old_dir = parent.join("app.livesprite.mac");
    if !old_dir.join("state.json").exists() {
        return;
    }
    if std::fs::create_dir_all(&new_dir).is_err() {
        return;
    }
    for name in ["state.json", "souls", "context", "packs"] {
        let src = old_dir.join(name);
        let dst = new_dir.join(name);
        if src.exists() && !dst.exists() {
            let _ = copy_tree(&src, &dst);
        }
    }
    eprintln!(
        "orbitty: migrated app support {} → {}",
        old_dir.display(),
        new_dir.display()
    );
}

fn copy_tree(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            copy_tree(&entry.path(), &dst.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(src, dst).map(|_| ())
    }
}
