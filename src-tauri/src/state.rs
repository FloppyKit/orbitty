//! Persistence — one JSON file under the app data dir.
//! Additive fields: old saves load. Whole-object save must not strip
//! unknown keys. No PHI. AX titles are never rewritten.

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
    /// Settings → Appearance toggle. Default OFF —
    /// collapsed marbles sit where they were left until opted in.
    pub marble_drift: bool,
    /// the Herding Cats confirm's source
    /// checkboxes — independent on/off, at least one must be on before
    /// Drop can arm. Tabs stay disabled in the pane until Tabbity lands.
    /// The legacy `yardSource` radio key is no longer written; old saves
    /// heal to windows on / tabs off via the Default impl below. No
    /// yardCats array: cats never persist.
    pub yard_windows: bool,
    pub yard_tabs: bool,
    /// The large companion window. All fields
    /// default — old saves load, and other windows round-trip the slice.
    pub companion: CompanionState,
    /// The operator-green radar. Explicit check-in only
    /// — no wearables, no dwell score, no inferred anything.
    pub stars: StarsState,
    /// Mute list only. The bag is derived, never
    /// persisted; a missing key loads as an empty mute list.
    pub sky: SkyState,
    /// Which page the one shared 780×520 `kanban`
    /// overlay last mounted — "board" (KanbanPage) | "stars" (StarsPage).
    /// Written native-side before every show; a cold webview reconciles it
    /// via load_state on mount (PANE-404: the URL never carries the mode);
    /// a bare re-show restores it.
    pub shared_pane_mode: String,
    /// Leash state per drawer label. Missing labels
    /// default per `drawer_docked` — never strip old keys on save.
    pub drawers: std::collections::BTreeMap<String, DrawerState>,
    /// One row per doing-run — the agent walker
    /// on that planet. Additive; old saves load with an empty list.
    pub agent_runs: Vec<AgentRun>,
    /// Daily spend ceiling USD. Default 20.
    pub spend_cap_usd: f64,
    /// false = Never for money.
    pub spend_cap_enabled: bool,
    /// Concurrent runners, default 2, clamp 1–4.
    pub runner_cap: u32,
    /// Local calendar day for spendUsd roll.
    pub spend_day: String,
    /// Parsed usage USD today. 0 if nothing parsed.
    pub spend_usd: f64,
    /// Launchpad mail split: Needed fraction of column 1 (0.18–0.45).
    /// Default 0.28. Additive — old saves load the default.
    #[serde(default = "needed_split_default")]
    pub needed_split: f64,
    /// Launchpad preview column width (px). Default 280. Additive.
    #[serde(default = "preview_width_default")]
    pub preview_width: f64,
    /// Launchpad east: Trackers fraction of the right column (0.14–0.55).
    /// Default 0.20. Additive.
    #[serde(default = "tracker_split_default")]
    pub tracker_split: f64,
    /// Parent todo ids with nested subtasks expanded on Launchpad.
    #[serde(default)]
    pub open_subtask_ids: Vec<String>,
    /// Unbound-todo trees on planets. Additive.
    #[serde(default)]
    pub todo_trees: Vec<TodoTree>,
    /// Days to full canopy when the todo has no due. Default 14.
    #[serde(default = "tree_grow_days_default")]
    pub tree_grow_days: f64,
    /// Settings: new events.jsonl lines get a NIP-01 id + device sig.
    /// Default on. Old lines stay unsigned. Missing key loads as on.
    #[serde(default = "signed_locally_default")]
    pub signed_locally: bool,
    /// Settings: best-effort EVENT of signed 420/421/422 to the loopback
    /// sidecar. Default off. Missing key loads as off.
    #[serde(default)]
    pub private_relay: bool,
    /// ctl `shift on` — 15s supervisor ticker. Default off.
    /// Spark/Chat never auto-pickup; Work/Hold does.
    #[serde(default)]
    pub shift_on: bool,
    /// Unused expanded planets marble after this idle. Default 4h.
    /// "1h" | "4h" | "12h" | "never". Missing/unknown → 4h.
    #[serde(default = "planet_idle_default")]
    pub planet_idle: String,
}

fn needed_split_default() -> f64 {
    0.28
}

fn preview_width_default() -> f64 {
    280.0
}

fn tracker_split_default() -> f64 {
    0.20
}

fn tree_grow_days_default() -> f64 {
    14.0
}

fn signed_locally_default() -> bool {
    true
}

fn planet_idle_default() -> String {
    "4h".into()
}

/// One unbound-todo tree on a planet disc.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct TodoTree {
    pub card_id: String,
    pub project_id: String,
    pub x: f64,
    pub y: f64,
    pub planted_at: f64,
}

impl Default for TodoTree {
    fn default() -> Self {
        Self {
            card_id: String::new(),
            project_id: String::new(),
            x: 0.0,
            y: 0.1,
            planted_at: 0.0,
        }
    }
}

/// The only labels that can be drawers. The rail
/// is not a drawer; `sky` stays hide-not-destroy; Board+STARS are the one
/// `kanban` overlay; planets are never docked.
pub const DRAWER_LABELS: [&str; 4] = ["companion", "kanban", "settings", "stars"];

/// `docked` = follow-translate on the rail
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
            agent_runs: Vec::new(),
            spend_cap_usd: 20.0,
            spend_cap_enabled: true,
            runner_cap: 2,
            spend_day: String::new(),
            spend_usd: 0.0,
            needed_split: 0.28,
            preview_width: 280.0,
            tracker_split: 0.20,
            open_subtask_ids: Vec::new(),
            todo_trees: Vec::new(),
            tree_grow_days: 14.0,
            signed_locally: true,
            private_relay: false,
            shift_on: false,
            planet_idle: planet_idle_default(),
        }
    }
}

/// One doing-run — the row the habitat mirrors
/// as the seeded droid walker on `planet_id`. Minted/refreshed by the
/// attach door (`agent_runs::attach_on_moves`) when a gated `kind=todo`
/// card lands in Doing; the launchers (doing-fire / pickup /
/// `launch_agent`) own the process — this row is never a spawn.
/// `state`: "starting" | "working" | "blocked" | "done". The doing-run
/// doors write working/done; the Herdr adapter (`herdr.rs`) writes blocked
/// and carries `herdr`; starting stays read-supported only (NeededInbox
/// open or blocked → the walker sits the near rim; done → the far-rim
/// walk-off, then the rail evicts the row).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentRun {
    pub card_id: String,
    pub planet_id: String,
    /// The agentShortcuts row running the card; "" = none resolved
    /// (a GO-* fire before the standing answer assigns one).
    pub agent_shortcut_id: String,
    pub state: String,
    /// The herdr pen for this run — present
    /// only when the herdr branch of `launch_agent` launched it. The few
    /// adapter fields ride the one row; never a second planet DB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub herdr: Option<AgentRunHerdr>,
}

/// The herdr pen of one doing-run, persisted
/// on the run row (`agentRun.herdr`). Additive; old saves load with None.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentRunHerdr {
    pub planet_id: String,
    pub workspace: String,
    pub tab: String,
    pub pane: String,
    pub agent_name: String,
    /// "grok" | "opencode" — opencode is detect-only (on PATH = legal),
    /// never prompt-installed.
    pub kind: String,
}

impl Default for AgentRun {
    fn default() -> Self {
        Self {
            card_id: String::new(),
            planet_id: String::new(),
            agent_shortcut_id: String::new(),
            state: "working".to_string(),
            herdr: None,
        }
    }
}

/// `muted` holds smashed words (lowercase
/// tokens); everything else about the sky is recomputed on open.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SkyState {
    pub muted: Vec<String>,
}

/// `prompts_opt_in` defaults FALSE — the
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
    /// "week" | "14" | "month" | "year" | "custom".
    /// Default "14" on old saves. Custom uses average_from / average_to.
    pub average_window: String,
    pub average_from: String,
    pub average_to: String,
    /// spoke scale version. 0/absent = legacy 0–4
    /// (migrate n→n+1 once at setup), 2 = 1–5. The flag is what stops a
    /// double migration — a migrated save has no zeros left to sniff for.
    pub scale_v: u8,
}

impl Default for StarsState {
    fn default() -> Self {
        Self {
            prompts_opt_in: false,
            checkins: std::collections::BTreeMap::new(),
            prompts_ack: String::new(),
            average_window: "14".to_string(),
            average_from: String::new(),
            average_to: String::new(),
            scale_v: 0,
        }
    }
}

/// One day's check-in: five spokes, 1–5 each (legacy 0–4 migrates n→n+1
/// once via `migrate_stars_scale`). `self` is the fifth spoke
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

/// Companion persist slice. Additive keys; old saves load.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct CompanionState {
    #[serde(default = "companion_visible_default")]
    pub visible: bool,
    /// None = never placed — first show lands at the trailing-edge default.
    pub bounds: Option<CompanionBounds>,
    pub image_pack_id: Option<String>,
    /// "silent" | "idle" | "needs_human". Ships idle-only (no pulse
    /// wiring); the field is forward-compat for the urgency slice.
    pub state: String,
    /// Latest line sent from the companion composer. "" = none yet.
    pub last_user_line: String,
    /// Latest reply from the companion backend. "" = none.
    pub last_bot_line: String,
    /// Berd agent stem, Hermes profile name, or ""
    /// (unset — resolves at send/display time, never stored resolved).
    pub agent_profile: String,
    /// Expanded transcript view; restored on show.
    pub expanded: bool,
    /// Capped transcript (last 40 turns) for the expanded view.
    pub turns: Vec<CompanionTurn>,
    /// Read bot replies aloud via macOS `say` (default system
    /// voice). Default OFF. R-click the speaker mutes this.
    pub speak_replies: bool,
    /// Companion `say` volume 0.0–1.0. Missing key (old saves) = 1.
    #[serde(default = "companion_speak_volume_default")]
    pub speak_volume: f32,
    /// Draw the CC / transcript. R-click the expand control. Default on.
    #[serde(default = "companion_captions_default")]
    pub captions: bool,
    /// Hide desktop planet discs. R-click the Planets dest. Default off.
    #[serde(default)]
    pub planets_hidden: bool,
    /// Print ⌘? under the composer. Default on.
    #[serde(default = "companion_cheat_hint_default")]
    pub cheat_sheet_hint: bool,
    /// Companion Talk source. `"system"` (default,
    /// macOS speech) or `"hotkey"`
    /// (Handy PTT chord). Unknown values load as system.
    #[serde(default = "talk_source_default")]
    pub talk_source: String,
    /// Collapsed-surface view —
    /// "full" (default) | "head".
    #[serde(default = "companion_view_collapsed_default")]
    pub view_collapsed: String,
    /// Expanded-surface view —
    /// "full" | "head" (default).
    #[serde(default = "companion_view_expanded_default")]
    pub view_expanded: String,
    /// Draw the companion sprite. One flag for
    /// collapsed, expanded, and hosted; missing key (old saves) = true.
    #[serde(default = "companion_sprite_default")]
    pub sprite: bool,
    /// The active companion mode — "spark"
    /// (default) | "chat" | "work" | "hold". Job law only (desk / join /
    /// build / parked). ONE transcript in `turns`. Mode does not park a
    /// second brain. Four-cast stem pick is still a second axis.
    #[serde(default = "companion_mode_default")]
    pub mode: String,
    /// Legacy parked-per-mode map. No longer the live transcript —
    /// `turns` is the one companion thread. Kept on disk so old saves
    /// load; heal copies a non-empty parked thread into `turns` only
    /// when `turns` is empty. Mode switch does not swap this.
    #[serde(default)]
    pub threads: std::collections::BTreeMap<String, Vec<CompanionTurn>>,
    /// Ctl-lit "agent working" flag for the dock
    /// LED (`orbittyctl companion hack -- on [--who cursor]`). Any
    /// harness opts in; Spark's own `sending` lights the same LED
    /// without ctl. A flag, never a transcript. Default false.
    pub hack_live: bool,
    /// Names the live source for the LED title
    /// ("cursor", "goose", …). "" = unknown; cleared on off.
    pub hack_who: String,
    /// the live agent's pinned planet
    /// (pickup writes it) — the dest/planet menus glow that one planet
    /// while the run is live. None = no pin / not a pickup; cleared on
    /// off. Never a transcript.
    pub hack_project_id: Option<String>,
    /// Free Mode — the grok-lane router runs the
    /// free lanes only (local Ollama → OpenRouter free) and paid Grok
    /// stays off, so no surprise bill. Default off. The OpenRouter key
    /// is NOT here — it lives 0600 in companions/orbitty-primary, never
    /// in this whole-object save.
    #[serde(default)]
    pub free_mode: bool,
    /// The conductor — a slim local fast-ack layer
    /// in front of the mouth. On = every Spark send gets a one-line ack
    /// + a verb in hundreds of ms (local Ollama first, non-LLM
    /// classifier when it's down); `chat` falls through to the hop
    /// mouth for the real reply in the same bubble. Off = today's mouth
    /// only. Default off. The set_companion_conductor* commands are the
    /// only doors — save_state carries the disk values forward.
    #[serde(default)]
    pub conductor_on: bool,
    /// Conductor model id (an Ollama tag). "" =
    /// built-in classifier (this GO). Never Grok / OpenRouter / 12B.
    #[serde(default)]
    pub conductor_model: String,
    /// Reply lane: "grok" | "local" | "openrouter" | "free".
    /// Empty (old saves) heals from free_mode. Dedicated command.
    #[serde(default)]
    pub reply_lane: String,
    /// Chosen local Reply id from the scan (LM Studio or Ollama).
    /// Empty = prefer E4B on LM Studio.
    #[serde(default)]
    pub reply_model: String,
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
    // Shipping default is system speech.
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

fn companion_speak_volume_default() -> f32 {
    1.0
}

fn companion_captions_default() -> bool {
    true
}

fn companion_cheat_hint_default() -> bool {
    true
}

pub fn companion_mode_default() -> String {
    "spark".into()
}

/// The four modes are the whole allowlist —
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
    /// Heal, run on every native load: unknown mode → spark.
    /// One transcript: if `turns` is empty and a parked thread has
    /// lines, lift the longest parked thread so MODE-SPLIT history is
    /// not dropped. Do not swap on later mode changes.
    pub fn heal_modes(&mut self) {
        if normalize_companion_mode(&self.mode).is_none() {
            self.mode = companion_mode_default();
        }
        if self.turns.is_empty() {
            if let Some((_, t)) = self
                .threads
                .iter()
                .max_by_key(|(_, v)| v.len())
                .filter(|(_, v)| !v.is_empty())
            {
                self.turns = t.clone();
            }
        }
        if self.threads.is_empty() && !self.turns.is_empty() {
            self.threads
                .insert(self.mode.clone(), self.turns.clone());
        }
    }

    /// Append one turn to the single companion thread. Shadow-copy under
    /// the current mode so old readers of `threads[mode]` still see it.
    pub fn push_turn(&mut self, role: &str, text: &str) {
        let t = CompanionTurn {
            role: role.into(),
            text: text.into(),
        };
        self.turns.push(t.clone());
        cap_companion_turns(&mut self.turns);
        let thread = self.threads.entry(self.mode.clone()).or_default();
        *thread = self.turns.clone();
    }

    /// Job switch only. Same `turns`. Never parks a second transcript.
    pub fn switch_mode(&mut self, new_mode: &str) {
        self.mode = new_mode.to_string();
        self.threads
            .insert(self.mode.clone(), self.turns.clone());
    }
}

/// Only "full" / "head" are real; anything
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
            speak_volume: companion_speak_volume_default(),
            captions: true,
            planets_hidden: false,
            cheat_sheet_hint: true,
            talk_source: talk_source_default(),
            view_collapsed: companion_view_collapsed_default(),
            view_expanded: companion_view_expanded_default(),
            sprite: true,
            mode: companion_mode_default(),
            threads: std::collections::BTreeMap::new(),
            hack_live: false,
            hack_who: String::new(),
            hack_project_id: None,
            free_mode: false,
            conductor_on: false,
            conductor_model: String::new(),
            reply_lane: String::new(),
            reply_model: String::new(),
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
    /// Optional global chord, e.g. "cmd+alt+m".
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
    /// "cursor" | "goose" | "grok" | "hermes" — per row, picked on
    /// `agent add` (never a product-wide default).
    pub harness: String,
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub md_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Free-text extra instructions — pickup
    /// concatenates them into the query file after the md paths.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
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
    /// Named counters for this planet. Additive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trackers: Option<Vec<Tracker>>,
    /// Absolute planet workspace root. Absent = no task snapshot. Void never has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
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
    // Reboot-rematch fingerprint, stamped from the live
    // window at claim time (live AX title, never the display override). Live
    // ids die on reboot; these fields rebuild the stableKey so the claim can
    // adopt the same window's new id. All optional — old saves load clean.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_at_claim: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_index: Option<u32>,
    /// Sprite picker (claim menu): packs/<id> folder this window's walker
    /// draws from; absent/empty = the generated being.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_pack_id: Option<String>,
    /// The generated person's roll seed. Stamped at claim
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
    /// Legacy occupancy-teleport leftover. Ignored —
    /// occupancy is `project_id` only (one windowId, one planet). The
    /// field stays additive so old saves still load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walker_home: Option<String>,
    // Wake stamps (all additive — old saves load). A
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

/// Local Kanban SoT — cards are born at claim time (`kind: "window"`,
/// status `doing`) or as freeform `kind: "todo"` from the Lists window.
/// Unclaim keeps the card as a project todo with `window_id` cleared.
/// `starred` is shape-only (no star UI).
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
    // Detail-panel fields. `difficulty` is metadata only — it never
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
    // The Needed inbox (all additive — old saves
    // load). `due_at` doubles as the Later snooze stamp on Needed cards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needed_state: Option<String>, // "open" | "later" | "answered" | "skipped"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needed_why: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needed_answer: Option<String>,
    // Who completed the card ("user" | "agent" |
    // "action" | "vps"). Additive — missing on pre-actor saves, and the HUD
    // reads missing as "user" (pre-actor history was human).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    // The agentShortcuts row this Todo is assigned
    // to. Additive — missing on pre-assign saves; `kanban assign` writes it
    // (empty agent unassigns); `agent rm` leaves the id on the card, same
    // as project rm keeping cards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_agent_id: Option<String>,
    /// Another todo this card is nested under. Additive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Optional 1–7 rating. Metadata only — never points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<u8>,
    /// Leftover card-level trackers. Launchpad migrates these onto
    /// the planet; the key stays so old saves do not strip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trackers: Option<Vec<Tracker>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tracker {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub value: i32,
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
    /// Who acted on the complete that minted this row
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
            // Old saves heal here — mode defaults
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

/// FIVE one-time migration: legacy checkins are 0–4, the
/// scale is now 1–5. Every saved spoke shifts n → n+1 exactly once — the
/// `scale_v` flag guards it, never zero-sniffing (a migrated save has no
/// zeros) — then the migrated shape is written back so no window ever
/// re-reads the old scale. Best-effort: a load/save failure just retries
/// next boot. An empty day stays empty: only EXISTING checkins shift, no
/// day is ever invented here.
pub fn migrate_stars_scale(app: &tauri::AppHandle) {
    let Ok(mut st) = load_state(app) else {
        return;
    };
    if st.stars.scale_v >= 2 {
        return;
    }
    for c in st.stars.checkins.values_mut() {
        c.sleep = c.sleep.saturating_add(1).min(5);
        c.terroir = c.terroir.saturating_add(1).min(5);
        c.athleticism = c.athleticism.saturating_add(1).min(5);
        c.relationships = c.relationships.saturating_add(1).min(5);
        c.self_ = c.self_.saturating_add(1).min(5);
    }
    st.stars.scale_v = 2;
    let _ = save_state(app, &st);
}

/// One-time migration: if the new `space.orbitty.mac`
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
