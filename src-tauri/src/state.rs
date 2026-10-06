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
    /// Last planet a Layer-0 claim landed on (tray / Dock / planet menu).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_claim_project_id: Option<String>,
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
    /// PEER-CHROME: show companion-dock dest strip + rail window stack
    /// (STARS·Sky·Board·Comp). Default OFF for demo/new; auto-graduates
    /// after 3 claims or first Done; Settings can force on/off.
    #[serde(default)]
    pub peer_chrome: bool,
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
    /// Old Launchpad trackers split. Trackers paint on STARS now.
    /// The key stays so a whole-state save does not drop it.
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
    /// Optional one-user replica URL (`ws://100.x:13120` or SSH tunnel
    /// `ws://127.0.0.1:port`). Empty = local sidecar only.
    #[serde(default)]
    pub private_relay_url: String,
    /// ctl `shift on` — 15s supervisor ticker. Default off.
    /// Spark/Chat never auto-pickup; Work/Hold does.
    #[serde(default)]
    pub shift_on: bool,
    /// Unused expanded planets marble after this idle. Default 4h.
    /// "1h" | "4h" | "12h" | "never". Missing/unknown → 4h.
    #[serde(default = "planet_idle_default")]
    pub planet_idle: String,
    /// No-herdr terminal. "ghostty" | "terminal" | "". Empty = Ghostty
    /// when `/Applications/Ghostty.app` exists, else Terminal. A window
    /// that sends ghostty or terminal wins; an omitted key stays "".
    #[serde(default)]
    pub agent_terminal: String,
    /// Inspector rail board columns (`todo` / `doing` / `done`). Needed
    /// always shows and is never stored. A save without the key loads
    /// as ["todo"].
    #[serde(default = "rail_board_cols_default")]
    pub rail_board_cols: Vec<String>,
    /// Stage Manager edge for this install. One state file is one machine,
    /// so this is not a per-planet pref and is not a sync key.
    /// "left" | "right" | "top" | "bottom". Missing or unknown → left.
    #[serde(default = "rail_edge_default")]
    pub rail_edge: String,
    /// Hide the stage strip when focus leaves the rail. Distinct from
    /// `autohide_row_chrome` (row-button chrome). Default on.
    #[serde(default = "rail_auto_hide_default")]
    pub rail_auto_hide_on_blur: bool,
    /// Alpha mini story. Rust-owned: only `starter.rs` and
    /// `set_starter_coach_done` write it, through `state::update`.
    /// A window save copies this from disk (`merge_window_save`).
    #[serde(default)]
    pub starter_story: StarterStory,
    /// Cards a window or a Rust delete removed. A later save brings a
    /// card back only when that card's `lastTouchedAt` is newer than the
    /// stamp. Old builds ignore the key. Empty stays off disk.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_tombstones: Vec<CardTombstone>,
}

/// One removed card. `last_touched_at` is the delete stamp (epoch seconds).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CardTombstone {
    pub id: String,
    pub last_touched_at: f64,
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

fn rail_board_cols_default() -> Vec<String> {
    vec!["todo".into()]
}

fn rail_edge_default() -> String {
    "left".into()
}

fn rail_auto_hide_default() -> bool {
    true
}

/// "left" | "right" | "top" | "bottom". Anything else is left.
pub fn normalize_rail_edge(raw: &str) -> &'static str {
    match raw.trim() {
        "right" => "right",
        "top" => "top",
        "bottom" => "bottom",
        "left" => "left",
        _ => "left",
    }
}

/// Window origin on a top-left visible frame. Hidden keeps a 12px sliver
/// on `edge`. Revealed sits 8px in from that same edge.
pub fn rail_stage_frame(
    edge: &str,
    revealed: bool,
    vx: f64,
    vy: f64,
    vw: f64,
    vh: f64,
    ww: f64,
    wh: f64,
) -> (f64, f64, f64, f64) {
    let sliver = 12.0;
    let inset = 8.0;
    let w = if ww > 0.0 { ww } else { 360.0 };
    let h = if wh > 0.0 { wh } else { 420.0 };
    let (x, y) = match normalize_rail_edge(edge) {
        "right" => {
            let x = if revealed {
                vx + vw - w - inset
            } else {
                vx + vw - sliver
            };
            let y = vy + ((vh - h) / 2.0).max(0.0);
            (x, y)
        }
        "top" => {
            let y = if revealed { vy + inset } else { vy - h + sliver };
            let x = vx + ((vw - w) / 2.0).max(0.0);
            (x, y)
        }
        "bottom" => {
            let y = if revealed {
                vy + vh - h - inset
            } else {
                vy + vh - sliver
            };
            let x = vx + ((vw - w) / 2.0).max(0.0);
            (x, y)
        }
        _ => {
            let x = if revealed { vx + inset } else { vx - w + sliver };
            let y = vy + ((vh - h) / 2.0).max(0.0);
            (x, y)
        }
    };
    (x, y, w, h)
}

/// Revealed cap: wheel column plus the board beside it.
/// `stageLook.ts` `WHEEL_COL` 320 + `PREVIEW_MARGIN` + ~690px board.
const RAIL_SPIN_W: f64 = 1040.0;
const RAIL_SPIN_H: f64 = 760.0;

/// Revealed window hugs the wheel and the board. LEFT/RIGHT inset the width
/// by the 8px `rail_stage_frame` uses on the parked edge; TOP/BOTTOM inset
/// the height. Parked stays the compact 360×420 sliver. Unknown edges are left.
pub fn rail_wheel_size(edge: &str, revealed: bool, vw: f64, vh: f64) -> (f64, f64) {
    if !revealed {
        return (360.0, 420.0);
    }
    match normalize_rail_edge(edge) {
        "top" | "bottom" => (vw.min(RAIL_SPIN_W), (vh - 8.0).max(0.0).min(RAIL_SPIN_H)),
        _ => ((vw - 8.0).max(0.0).min(RAIL_SPIN_W), vh.min(RAIL_SPIN_H)),
    }
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
            last_claim_project_id: None,
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
            peer_chrome: false,
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
            private_relay_url: String::new(),
            shift_on: false,
            planet_idle: planet_idle_default(),
            agent_terminal: String::new(),
            card_tombstones: Vec::new(),
            rail_board_cols: vec!["todo".into()],
            rail_edge: rail_edge_default(),
            rail_auto_hide_on_blur: true,
            starter_story: StarterStory::default(),
        }
    }
}

/// Alpha mini story flags. `v` 0 means never evaluated (missing on disk).
/// JSON key `starterStory`. Windows never write this slice.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct StarterStory {
    pub v: u8,
    pub skipped: bool,
    pub coach_done: bool,
    pub seeded_project_id: Option<String>,
    pub comms_built: bool,
    pub stars_built: bool,
    pub intro_done: bool,
}

impl Default for StarterStory {
    fn default() -> Self {
        Self {
            v: 0,
            skipped: false,
            coach_done: false,
            seeded_project_id: None,
            comms_built: false,
            stars_built: false,
            intro_done: false,
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
    /// Flipable journal pages. Empty until Reflect writes a stub.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ledger_pages: Vec<LedgerPage>,
    /// Check-in fraction of the STARS left column. Default 0.62.
    #[serde(default = "stars_act_split_default")]
    pub stars_act_split: f64,
    /// Left-column fraction of the STARS body. Default 0.52.
    #[serde(default = "stars_east_split_default")]
    pub stars_east_split: f64,
}

fn stars_act_split_default() -> f64 {
    0.62
}

fn stars_east_split_default() -> f64 {
    0.52
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
            ledger_pages: Vec::new(),
            stars_act_split: stars_act_split_default(),
            stars_east_split: stars_east_split_default(),
        }
    }
}

/// One day's check-in: five spokes, 1–5 each (legacy 0–4 migrates n→n+1
/// once via `migrate_stars_scale`). `self` is the fifth spoke
/// (Spiritual in the pane heading); renamed past the Rust keyword.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct StarsCheckin {
    pub sleep: u8,
    pub terroir: u8,
    pub athleticism: u8,
    pub relationships: u8,
    #[serde(rename = "self")]
    pub self_: u8,
    /// Short day notes. Empty or absent = not filled. Saved with the spokes.
    #[serde(default, skip_serializing_if = "opt_blank")]
    pub win1: Option<String>,
    #[serde(default, skip_serializing_if = "opt_blank")]
    pub win2: Option<String>,
    #[serde(default, skip_serializing_if = "opt_blank")]
    pub grateful: Option<String>,
}

fn opt_blank(v: &Option<String>) -> bool {
    v.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true)
}

/// Which metrics a ledger stub opted in. Reflect does not write the body.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MetricFlags {
    pub spokes: bool,
    pub wins: bool,
    pub grateful: bool,
    pub trackers: bool,
    pub cloud: bool,
}

/// One flipable journal card. Body stays empty until a later Reflect phrase.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LedgerPage {
    pub id: String,
    /// Epoch milliseconds.
    pub created_at: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day_key: Option<String>,
    /// "overall" or a planet id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric_flags: Option<MetricFlags>,
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
    /// Online agent behind "Ask online agent". Empty = ask each time.
    /// No personal default and no home-directory path.
    #[serde(default)]
    pub handoff_backend: String,
    /// Hand off without a click when the local model can't do the ask.
    /// Default off.
    #[serde(default)]
    pub auto_handoff: bool,
    /// Companion harness: "goose" | "grok" | "hermes" | "direct". "" = unset.
    /// ORBITTY_COMPANION_BACKEND overrides this when it is set. berd is
    /// env-only and is not stored here. direct is built in (no CLI).
    #[serde(default)]
    pub backend: String,
    /// OpenAI-compatible base URL for the Direct compat lane.
    /// Empty = not set. http only for loopback. The key is NOT here —
    /// it lives 0600 in companions/orbitty-primary/direct.key.
    #[serde(default)]
    pub direct_base_url: String,
    /// Model id for that compat base. Empty = not set.
    #[serde(default)]
    pub direct_model: String,
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
/// Companion body ids that were removed from the app. Kept only so old
/// saves heal to the Spirit (`CompanionState::heal_retired_body`).
pub const RETIRED_BODY_IDS: &[&str] = &["tang"];

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
    pub fn heal_retired_body(&mut self) {
        // A body id the app no longer ships (the retired 3D mesh) reads as
        // the Spirit, so an old save lands on a working companion instead
        // of a pack lookup that can only miss.
        if self
            .image_pack_id
            .as_deref()
            .is_some_and(|id| RETIRED_BODY_IDS.contains(&id))
        {
            self.image_pack_id = None;
        }
    }

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
            handoff_backend: String::new(),
            auto_handoff: false,
            backend: String::new(),
            direct_base_url: String::new(),
            direct_model: String::new(),
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
    /// "cursor" | "goose" | "grok" | "hermes" | "pi" — per row, picked on
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
    /// Cursor CLI preset: "read-only" | "workspace-write" | "full-auto".
    /// Missing on old rows loads as read-only. Only full-auto passes
    /// the CLI `--force` / `--trust` flags.
    #[serde(default = "permissions_default")]
    pub permissions: String,
}

fn permissions_default() -> String {
    "read-only".into()
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

/// Local Kanban SoT — freeform `kind: "todo"` from Lists, or a todo
/// assigned to a claimed window (`window_id` set, status `doing`).
/// Bare claim does not birth a card. Unclaim clears `window_id`.
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
    /// Clickable file:// or https:// links. Absent on old saves.
    /// Empty is stored as absent so a save never writes `links: null`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<CardLink>>,
}

/// One card link. `label` is display-only; `url` is the allowlisted target.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CardLink {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tracker {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub value: i32,
    /// Absent or "number" is a counter. "timer" stores elapsed seconds in `value`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Epoch ms while a timer is running. Stop folds the delta into `value` and clears this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running_since: Option<f64>,
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
    load_path(&path)
}

fn load_path(path: &std::path::Path) -> Result<PersistedState, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let mut st: PersistedState = serde_json::from_str(&text)
                .map_err(|e| format!("parse {}: {e}", path.display()))?;
            // Old saves heal here — mode defaults
            // spark, a pre-split transcript parks under spark.
            st.companion.heal_modes();
            st.companion.heal_retired_body();
            Ok(st)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PersistedState::default()),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

/// Disk write. Private: every caller goes through [`update`] so a stale
/// snapshot cannot land between another writer's load and save.
fn save_state(app: &tauri::AppHandle, state: &PersistedState) -> Result<(), String> {
    let path = state_path(app)?;
    save_path(&path, state)
}

fn save_path(path: &std::path::Path, state: &PersistedState) -> Result<(), String> {
    // Every writer hits this door. Canonicalize links here so a board
    // window's whole-state save keeps them and a hostile scheme never lands.
    let mut state = state.clone();
    crate::card_links::canonicalize_state_links(&mut state)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(&state).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename to {}: {e}", path.display()))?;
    Ok(())
}

fn write_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // A panicked writer must not freeze every later save.
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

std::thread_local! {
    static IN_UPDATE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

struct UpdateFlag;
impl Drop for UpdateFlag {
    fn drop(&mut self) {
        IN_UPDATE.with(|c| c.set(false));
    }
}

/// The only write helper. Holds the process-wide lock across load, `f`,
/// and save. `f` must not call `update` again (the lock is not reentrant).
/// An unchanged state skips the disk write.
pub fn update<T>(
    app: &tauri::AppHandle,
    f: impl FnOnce(&mut PersistedState) -> Result<T, String>,
) -> Result<T, String> {
    if IN_UPDATE.with(|c| c.get()) {
        return Err("state update re-entered".into());
    }
    let _guard = write_lock();
    IN_UPDATE.with(|c| c.set(true));
    let _flag = UpdateFlag;
    let mut st = load_state(app)?;
    let before = serde_json::to_string(&st).ok();
    let out = f(&mut st)?;
    let after = serde_json::to_string(&st).ok();
    if before != after {
        save_state(app, &st)?;
    }
    Ok(out)
}

#[cfg(test)]
fn update_at<T>(
    path: &std::path::Path,
    f: impl FnOnce(&mut PersistedState) -> Result<T, String>,
) -> Result<T, String> {
    let mut st = load_path(path)?;
    let before = serde_json::to_string(&st).ok();
    let out = f(&mut st)?;
    let after = serde_json::to_string(&st).ok();
    if before != after {
        save_path(path, &st)?;
    }
    Ok(out)
}

/// Counts from one window merge. Logged as
/// `state-save-merge {window, kept_disk, took_incoming}` — counts only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowMerge {
    pub kept_disk: usize,
    pub took_incoming: usize,
}

/// Window `save_state` merge. Per-card by `lastTouchedAt`: a strictly
/// newer disk card beats an older incoming one; equal touch takes the
/// incoming card (the save in hand). An omitted card stays unless the
/// incoming tombstone is at least as new as the disk card. Rust-owned
/// slices are copied from `disk` and never from the window payload.
pub fn merge_window_save(disk: &PersistedState, incoming: &PersistedState) -> (PersistedState, WindowMerge) {
    let mut out = incoming.clone();
    out.agent_runs = disk.agent_runs.clone();
    out.companion.hack_live = disk.companion.hack_live;
    out.companion.hack_who = disk.companion.hack_who.clone();
    out.companion.hack_project_id = disk.companion.hack_project_id.clone();
    out.companion.free_mode = disk.companion.free_mode;
    out.companion.conductor_on = disk.companion.conductor_on;
    out.companion.conductor_model = disk.companion.conductor_model.clone();
    out.companion.reply_lane = disk.companion.reply_lane.clone();
    out.companion.reply_model = disk.companion.reply_model.clone();
    out.companion.handoff_backend = disk.companion.handoff_backend.clone();
    out.companion.auto_handoff = disk.companion.auto_handoff;
    out.companion.backend = disk.companion.backend.clone();
    out.companion.direct_base_url = disk.companion.direct_base_url.clone();
    out.companion.direct_model = disk.companion.direct_model.clone();
    out.shift_on = disk.shift_on;
    out.peer_chrome = disk.peer_chrome;
    // Stage prefs are machine settings written by set_rail_stage_prefs.
    // A window save that omits them deserializes to the defaults and must
    // not wipe a saved edge or auto-hide choice.
    out.rail_edge = disk.rail_edge.clone();
    out.rail_auto_hide_on_blur = disk.rail_auto_hide_on_blur;
    out.starter_story = disk.starter_story.clone();
    if out.last_claim_project_id.is_none() {
        out.last_claim_project_id = disk.last_claim_project_id.clone();
    }
    // Carried when the window names a terminal. An omitted key
    // deserializes to "" and must not wipe a saved override.
    if !matches!(out.agent_terminal.trim(), "ghostty" | "terminal") {
        out.agent_terminal = disk.agent_terminal.clone();
    }
    let (cards, tombs, kept, took) = merge_cards(
        &disk.kanban_cards,
        &incoming.kanban_cards,
        &disk.card_tombstones,
        &incoming.card_tombstones,
    );
    out.kanban_cards = cards;
    out.card_tombstones = tombs;
    // taskId / taskReason are native-only (task::stamp_card / task::end);
    // no window ever edits them. A window copy's values are always a stale
    // echo, so disk wins regardless of lastTouchedAt — a real later window
    // edit (newer stamp) must not carry an old taskId over the new one.
    // A card that is not on disk (new, or a newer window edit that
    // resurrects a Rust delete) has no native stamp to keep — drop the
    // window's stale taskId / taskReason.
    for c in out.kanban_cards.iter_mut() {
        match disk.kanban_cards.iter().find(|d| d.id == c.id) {
            Some(d) => {
                c.task_id = d.task_id.clone();
                c.task_reason = d.task_reason.clone();
            }
            None => {
                c.task_id = None;
                c.task_reason = None;
            }
        }
    }
    (
        out,
        WindowMerge {
            kept_disk: kept,
            took_incoming: took,
        },
    )
}

/// Apply a Rust writer's before→after delta onto the freshly loaded
/// state. Slices the writer did not change stay as `live` has them, so a
/// stale whole-state snapshot cannot drop an `agentRuns` row or a hack
/// flag it never touched.
pub fn apply_writer_delta(live: &mut PersistedState, before: &PersistedState, after: &PersistedState) {
    apply_card_delta(
        &mut live.kanban_cards,
        &before.kanban_cards,
        &after.kanban_cards,
        &mut live.card_tombstones,
    );
    apply_run_delta(&mut live.agent_runs, &before.agent_runs, &after.agent_runs);
    apply_tombstone_delta(
        &mut live.card_tombstones,
        &before.card_tombstones,
        &after.card_tombstones,
    );
    take_changed(&mut live.theme_id, &before.theme_id, &after.theme_id);
    take_changed(&mut live.window_opacity, &before.window_opacity, &after.window_opacity);
    take_changed(&mut live.projects, &before.projects, &after.projects);
    take_changed(&mut live.claims, &before.claims, &after.claims);
    take_changed(
        &mut live.last_claim_project_id,
        &before.last_claim_project_id,
        &after.last_claim_project_id,
    );
    take_changed(&mut live.tags, &before.tags, &after.tags);
    take_changed(&mut live.window_tags, &before.window_tags, &after.window_tags);
    take_changed(
        &mut live.collapsed_section_ids,
        &before.collapsed_section_ids,
        &after.collapsed_section_ids,
    );
    take_changed(
        &mut live.nudge_suppressed_window_ids,
        &before.nudge_suppressed_window_ids,
        &after.nudge_suppressed_window_ids,
    );
    take_changed(&mut live.points_ledger, &before.points_ledger, &after.points_ledger);
    take_changed(&mut live.title_overrides, &before.title_overrides, &after.title_overrides);
    take_changed(
        &mut live.autohide_row_chrome,
        &before.autohide_row_chrome,
        &after.autohide_row_chrome,
    );
    take_changed(&mut live.hide_titles, &before.hide_titles, &after.hide_titles);
    take_changed(&mut live.hide_app_icons, &before.hide_app_icons, &after.hide_app_icons);
    take_changed(
        &mut live.action_shortcuts,
        &before.action_shortcuts,
        &after.action_shortcuts,
    );
    take_changed(
        &mut live.agent_shortcuts,
        &before.agent_shortcuts,
        &after.agent_shortcuts,
    );
    take_changed(&mut live.sprite_scale, &before.sprite_scale, &after.sprite_scale);
    take_changed(&mut live.marble_drift, &before.marble_drift, &after.marble_drift);
    take_changed(&mut live.peer_chrome, &before.peer_chrome, &after.peer_chrome);
    take_changed(&mut live.yard_windows, &before.yard_windows, &after.yard_windows);
    take_changed(&mut live.yard_tabs, &before.yard_tabs, &after.yard_tabs);
    apply_companion_delta(&mut live.companion, &before.companion, &after.companion);
    take_changed(&mut live.stars, &before.stars, &after.stars);
    take_changed(&mut live.sky, &before.sky, &after.sky);
    take_changed(&mut live.shared_pane_mode, &before.shared_pane_mode, &after.shared_pane_mode);
    take_changed(&mut live.drawers, &before.drawers, &after.drawers);
    take_changed(&mut live.spend_cap_usd, &before.spend_cap_usd, &after.spend_cap_usd);
    take_changed(
        &mut live.spend_cap_enabled,
        &before.spend_cap_enabled,
        &after.spend_cap_enabled,
    );
    take_changed(&mut live.runner_cap, &before.runner_cap, &after.runner_cap);
    take_changed(&mut live.spend_day, &before.spend_day, &after.spend_day);
    take_changed(&mut live.spend_usd, &before.spend_usd, &after.spend_usd);
    take_changed(&mut live.needed_split, &before.needed_split, &after.needed_split);
    take_changed(&mut live.preview_width, &before.preview_width, &after.preview_width);
    take_changed(&mut live.tracker_split, &before.tracker_split, &after.tracker_split);
    take_changed(
        &mut live.open_subtask_ids,
        &before.open_subtask_ids,
        &after.open_subtask_ids,
    );
    take_changed(&mut live.todo_trees, &before.todo_trees, &after.todo_trees);
    take_changed(&mut live.tree_grow_days, &before.tree_grow_days, &after.tree_grow_days);
    take_changed(&mut live.signed_locally, &before.signed_locally, &after.signed_locally);
    take_changed(&mut live.private_relay, &before.private_relay, &after.private_relay);
    take_changed(
        &mut live.private_relay_url,
        &before.private_relay_url,
        &after.private_relay_url,
    );
    take_changed(&mut live.shift_on, &before.shift_on, &after.shift_on);
    take_changed(&mut live.planet_idle, &before.planet_idle, &after.planet_idle);
    take_changed(
        &mut live.agent_terminal,
        &before.agent_terminal,
        &after.agent_terminal,
    );
    take_changed(
        &mut live.rail_board_cols,
        &before.rail_board_cols,
        &after.rail_board_cols,
    );
    take_changed(&mut live.rail_edge, &before.rail_edge, &after.rail_edge);
    take_changed(
        &mut live.rail_auto_hide_on_blur,
        &before.rail_auto_hide_on_blur,
        &after.rail_auto_hide_on_blur,
    );
    // Named so writer_delta_lists_cover_every_field stays green. A writer
    // that did not change the slice leaves `live` alone. Window saves do
    // not come through here; merge_window_save copies starter_story from disk.
    take_changed(
        &mut live.starter_story,
        &before.starter_story,
        &after.starter_story,
    );
}

fn wall_now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn json_eq<T: serde::Serialize + ?Sized>(a: &T, b: &T) -> bool {
    serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
}

fn take_changed<T: Clone + serde::Serialize>(live: &mut T, before: &T, after: &T) {
    if !json_eq(before, after) {
        *live = after.clone();
    }
}

fn apply_companion_delta(live: &mut CompanionState, before: &CompanionState, after: &CompanionState) {
    take_changed(&mut live.visible, &before.visible, &after.visible);
    take_changed(&mut live.bounds, &before.bounds, &after.bounds);
    take_changed(&mut live.image_pack_id, &before.image_pack_id, &after.image_pack_id);
    take_changed(&mut live.state, &before.state, &after.state);
    take_changed(&mut live.last_user_line, &before.last_user_line, &after.last_user_line);
    take_changed(&mut live.last_bot_line, &before.last_bot_line, &after.last_bot_line);
    take_changed(&mut live.agent_profile, &before.agent_profile, &after.agent_profile);
    take_changed(&mut live.expanded, &before.expanded, &after.expanded);
    take_changed(&mut live.turns, &before.turns, &after.turns);
    take_changed(&mut live.speak_replies, &before.speak_replies, &after.speak_replies);
    take_changed(&mut live.speak_volume, &before.speak_volume, &after.speak_volume);
    take_changed(&mut live.captions, &before.captions, &after.captions);
    take_changed(&mut live.planets_hidden, &before.planets_hidden, &after.planets_hidden);
    take_changed(
        &mut live.cheat_sheet_hint,
        &before.cheat_sheet_hint,
        &after.cheat_sheet_hint,
    );
    take_changed(&mut live.talk_source, &before.talk_source, &after.talk_source);
    take_changed(&mut live.view_collapsed, &before.view_collapsed, &after.view_collapsed);
    take_changed(&mut live.view_expanded, &before.view_expanded, &after.view_expanded);
    take_changed(&mut live.sprite, &before.sprite, &after.sprite);
    take_changed(&mut live.mode, &before.mode, &after.mode);
    take_changed(&mut live.threads, &before.threads, &after.threads);
    take_changed(&mut live.hack_live, &before.hack_live, &after.hack_live);
    take_changed(&mut live.hack_who, &before.hack_who, &after.hack_who);
    take_changed(
        &mut live.hack_project_id,
        &before.hack_project_id,
        &after.hack_project_id,
    );
    take_changed(&mut live.free_mode, &before.free_mode, &after.free_mode);
    take_changed(&mut live.conductor_on, &before.conductor_on, &after.conductor_on);
    take_changed(
        &mut live.conductor_model,
        &before.conductor_model,
        &after.conductor_model,
    );
    take_changed(&mut live.reply_lane, &before.reply_lane, &after.reply_lane);
    take_changed(&mut live.reply_model, &before.reply_model, &after.reply_model);
    take_changed(
        &mut live.handoff_backend,
        &before.handoff_backend,
        &after.handoff_backend,
    );
    take_changed(&mut live.auto_handoff, &before.auto_handoff, &after.auto_handoff);
    take_changed(&mut live.backend, &before.backend, &after.backend);
    take_changed(
        &mut live.direct_base_url,
        &before.direct_base_url,
        &after.direct_base_url,
    );
    take_changed(&mut live.direct_model, &before.direct_model, &after.direct_model);
}

fn upsert_tombstone(tombs: &mut Vec<CardTombstone>, id: &str, at: f64) {
    if let Some(t) = tombs.iter_mut().find(|t| t.id == id) {
        if at > t.last_touched_at {
            t.last_touched_at = at;
        }
    } else {
        tombs.push(CardTombstone {
            id: id.to_string(),
            last_touched_at: at,
        });
    }
}

fn tomb_at(tombs: &[CardTombstone], id: &str) -> Option<f64> {
    tombs.iter().find(|t| t.id == id).map(|t| t.last_touched_at)
}

fn merge_cards(
    disk: &[KanbanCard],
    incoming: &[KanbanCard],
    disk_tombs: &[CardTombstone],
    incoming_tombs: &[CardTombstone],
) -> (Vec<KanbanCard>, Vec<CardTombstone>, usize, usize) {
    let mut tombs = Vec::new();
    for t in disk_tombs.iter().chain(incoming_tombs.iter()) {
        upsert_tombstone(&mut tombs, &t.id, t.last_touched_at);
    }
    let disk_by: std::collections::HashMap<&str, &KanbanCard> =
        disk.iter().map(|c| (c.id.as_str(), c)).collect();
    let inc_by: std::collections::HashMap<&str, &KanbanCard> =
        incoming.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut ids: Vec<&str> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for c in disk.iter().chain(incoming.iter()) {
        if seen.insert(c.id.as_str()) {
            ids.push(c.id.as_str());
        }
    }
    let mut out = Vec::new();
    let mut kept = 0usize;
    let mut took = 0usize;
    for id in ids {
        match (disk_by.get(id), inc_by.get(id)) {
            (Some(d), Some(i)) => {
                if d.last_touched_at > i.last_touched_at {
                    out.push((*d).clone());
                    kept += 1;
                } else {
                    out.push((*i).clone());
                    took += 1;
                }
            }
            (Some(d), None) => {
                let newer_delete = tomb_at(incoming_tombs, id).is_some_and(|t| t >= d.last_touched_at);
                if newer_delete {
                    took += 1;
                } else {
                    out.push((*d).clone());
                    kept += 1;
                }
            }
            (None, Some(i)) => {
                let blocked = tomb_at(disk_tombs, id).is_some_and(|t| t >= i.last_touched_at);
                if blocked {
                    kept += 1;
                } else {
                    out.push((*i).clone());
                    took += 1;
                }
            }
            (None, None) => {}
        }
    }
    // Lane order is the window's array, not disk-first. A drag that only
    // reorders never changes lastTouchedAt, so disk-first ids froze the list.
    out = order_like_incoming(out, incoming);
    tombs.retain(|t| match out.iter().find(|c| c.id == t.id) {
        Some(c) => t.last_touched_at > c.last_touched_at,
        None => true,
    });
    (out, tombs, kept, took)
}

/// Incoming order for cards the window still has. Disk-only survivors
/// (another writer added them) stay, after that order.
fn order_like_incoming(merged: Vec<KanbanCard>, incoming: &[KanbanCard]) -> Vec<KanbanCard> {
    let mut rest = merged;
    let mut out = Vec::with_capacity(rest.len());
    for c in incoming {
        if let Some(i) = rest.iter().position(|x| x.id == c.id) {
            out.push(rest.remove(i));
        }
    }
    out.append(&mut rest);
    out
}

fn apply_card_delta(
    live: &mut Vec<KanbanCard>,
    before: &[KanbanCard],
    after: &[KanbanCard],
    tombs: &mut Vec<CardTombstone>,
) {
    if json_eq(before, after) {
        return;
    }
    let before_by: std::collections::HashMap<&str, &KanbanCard> =
        before.iter().map(|c| (c.id.as_str(), c)).collect();
    let after_by: std::collections::HashMap<&str, &KanbanCard> =
        after.iter().map(|c| (c.id.as_str(), c)).collect();
    for c in before {
        if after_by.contains_key(c.id.as_str()) {
            continue;
        }
        if let Some(idx) = live.iter().position(|x| x.id == c.id) {
            if live[idx].last_touched_at <= c.last_touched_at {
                let at = c.last_touched_at.max(live[idx].last_touched_at);
                live.remove(idx);
                upsert_tombstone(tombs, &c.id, at);
            }
        }
    }
    for c in after {
        let changed = match before_by.get(c.id.as_str()) {
            None => true,
            Some(prev) => !json_eq(*prev, c),
        };
        if !changed {
            continue;
        }
        if tomb_at(tombs, &c.id).is_some_and(|t| t >= c.last_touched_at) && before_by.get(c.id.as_str()).is_none()
        {
            continue;
        }
        if let Some(idx) = live.iter().position(|x| x.id == c.id) {
            let saved = live[idx].last_touched_at;
            // touch() clamps to now+60, which sits below a far-future
            // saved stamp and would drop the edit (taskId included).
            // A saved stamp past now+60s is bogus — take the writer.
            if c.last_touched_at >= saved || saved > wall_now_secs() + 60.0 {
                live[idx] = c.clone();
            }
        } else if !tomb_at(tombs, &c.id).is_some_and(|t| t >= c.last_touched_at) {
            live.push(c.clone());
        }
    }
}

fn apply_run_delta(live: &mut Vec<AgentRun>, before: &[AgentRun], after: &[AgentRun]) {
    if json_eq(before, after) {
        return;
    }
    let before_by: std::collections::HashMap<&str, &AgentRun> =
        before.iter().map(|r| (r.card_id.as_str(), r)).collect();
    let after_by: std::collections::HashMap<&str, &AgentRun> =
        after.iter().map(|r| (r.card_id.as_str(), r)).collect();
    for r in before {
        if after_by.contains_key(r.card_id.as_str()) {
            continue;
        }
        live.retain(|x| x.card_id != r.card_id);
    }
    for r in after {
        let changed = match before_by.get(r.card_id.as_str()) {
            None => true,
            Some(prev) => !json_eq(*prev, r),
        };
        if !changed {
            continue;
        }
        if let Some(idx) = live.iter().position(|x| x.card_id == r.card_id) {
            live[idx] = r.clone();
        } else {
            live.push(r.clone());
        }
    }
}

fn apply_tombstone_delta(
    live: &mut Vec<CardTombstone>,
    before: &[CardTombstone],
    after: &[CardTombstone],
) {
    if json_eq(before, after) {
        return;
    }
    for t in after {
        let same = before.iter().find(|b| b.id == t.id).is_some_and(|b| json_eq(b, t));
        if !same {
            upsert_tombstone(live, &t.id, t.last_touched_at);
        }
    }
}

/// FIVE one-time migration: legacy checkins are 0–4, the
/// scale is now 1–5. Every saved spoke shifts n → n+1 exactly once — the
/// `scale_v` flag guards it, never zero-sniffing (a migrated save has no
/// zeros) — then the migrated shape is written back so no window ever
/// re-reads the old scale. Best-effort: a load/save failure just retries
/// next boot. An empty day stays empty: only EXISTING checkins shift, no
/// day is ever invented here.
pub fn migrate_stars_scale(app: &tauri::AppHandle) {
    let _ = update(app, |st| {
        if st.stars.scale_v >= 2 {
            return Ok(());
        }
        for c in st.stars.checkins.values_mut() {
            c.sleep = c.sleep.saturating_add(1).min(5);
            c.terroir = c.terroir.saturating_add(1).min(5);
            c.athleticism = c.athleticism.saturating_add(1).min(5);
            c.relationships = c.relationships.saturating_add(1).min(5);
            c.self_ = c.self_.saturating_add(1).min(5);
        }
        st.stars.scale_v = 2;
        Ok(())
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn touch_card(id: &str, touch: f64, notes: Option<&str>, links: Option<Vec<CardLink>>) -> KanbanCard {
        KanbanCard {
            id: id.into(),
            title: id.into(),
            project_id: "p".into(),
            window_id: None,
            kind: Some("todo".into()),
            status: "todo".into(),
            created_at: 1.0,
            last_touched_at: touch,
            completed_at: None,
            goal_aligned: true,
            clinical_or_master: false,
            points_awarded: 0,
            starred: false,
            notes: notes.map(str::to_string),
            difficulty: None,
            subtasks: None,
            due_at: None,
            tag_ids: None,
            needed_state: None,
            needed_why: None,
            needed_answer: None,
            actor: None,
            assigned_agent_id: Some("agent-1".into()),
            parent_id: None,
            rating: None,
            trackers: None,
            task_id: None,
            task_reason: None,
            links,
        }
    }

    fn run_row(card_id: &str) -> AgentRun {
        AgentRun {
            card_id: card_id.into(),
            planet_id: "p".into(),
            agent_shortcut_id: "agent-1".into(),
            state: "working".into(),
            herdr: None,
        }
    }

    fn temp_state() -> std::path::PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("orbitty-state-lock-{ms}-{n}-{}.json", std::process::id()))
    }

    fn update_file<T>(
        path: &std::path::Path,
        f: impl FnOnce(&mut PersistedState) -> Result<T, String>,
    ) -> Result<T, String> {
        let _g = write_lock();
        update_at(path, f)
    }

    /// N4 coverage: every `PersistedState` / `CompanionState` field must be
    /// named in the writer-delta lists, or a native writer's change to it
    /// is silently dropped under the lock. Reads this file's own source.
    fn struct_fields(src: &str, header: &str) -> Vec<String> {
        let start = src.find(header).unwrap_or_else(|| panic!("no {header}"));
        let body = &src[start + header.len()..];
        let end = body.find("\n}").unwrap();
        body[..end]
            .lines()
            .filter_map(|l| {
                let t = l.trim();
                let rest = t.strip_prefix("pub ")?;
                let (name, _) = rest.split_once(':')?;
                (!name.contains(' ') && !name.contains('(')).then(|| name.to_string())
            })
            .collect()
    }

    fn fn_body<'a>(src: &'a str, header: &str) -> &'a str {
        let start = src.find(header).unwrap_or_else(|| panic!("no {header}"));
        let body = &src[start..];
        let end = body.find("\n}\n").unwrap();
        &body[..end]
    }

    /// `live.<field>` as a whole identifier — `live.private_relay_url` must
    /// not count as naming `private_relay`.
    fn names_field(body: &str, field: &str) -> bool {
        let needle = format!("live.{field}");
        body.match_indices(&needle).any(|(i, _)| {
            body[i + needle.len()..]
                .chars()
                .next()
                .map_or(true, |c| !(c.is_ascii_alphanumeric() || c == '_'))
        })
    }

    #[test]
    fn names_field_is_word_bounded() {
        let body = "take_changed(&mut live.private_relay_url, &b, &a);";
        assert!(names_field(body, "private_relay_url"));
        assert!(!names_field(body, "private_relay"));
    }

    /// R3: a window holding a stale copy makes a real later edit (newer
    /// lastTouchedAt). Its edit wins; disk's native taskId/taskReason stay.
    #[test]
    fn newer_window_edit_keeps_disk_task_fields() {
        let mut disk = PersistedState::default();
        let mut d = touch_card("c1", 10.0, Some("old notes"), None);
        d.task_id = Some("t-abc".into());
        d.task_reason = Some("done".into());
        disk.kanban_cards.push(d);
        let mut incoming = PersistedState::default();
        // Window copy predates the stamp: no task fields, but a newer edit.
        incoming.kanban_cards.push(touch_card("c1", 20.0, Some("new notes"), None));
        let (out, _) = merge_window_save(&disk, &incoming);
        let c = out.kanban_cards.iter().find(|c| c.id == "c1").unwrap();
        assert_eq!(c.notes.as_deref(), Some("new notes"));
        assert_eq!(c.last_touched_at, 20.0);
        assert_eq!(c.task_id.as_deref(), Some("t-abc"));
        assert_eq!(c.task_reason.as_deref(), Some("done"));

        // A window can't clear or forge them either.
        let mut forged = incoming.clone();
        forged.kanban_cards[0].task_id = Some("t-forged".into());
        let (out, _) = merge_window_save(&disk, &forged);
        assert_eq!(out.kanban_cards[0].task_id.as_deref(), Some("t-abc"));
    }

    /// F2: a window card that is not on disk must not keep a stale taskId.
    #[test]
    fn window_card_not_on_disk_clears_task_fields() {
        let disk = PersistedState::default();
        let mut incoming = PersistedState::default();
        let mut c = touch_card("c-new", 5.0, None, None);
        c.task_id = Some("t-stale".into());
        c.task_reason = Some("done".into());
        incoming.kanban_cards.push(c);
        let (out, _) = merge_window_save(&disk, &incoming);
        let c = out.kanban_cards.iter().find(|c| c.id == "c-new").unwrap();
        assert!(c.task_id.is_none());
        assert!(c.task_reason.is_none());

        // Newer window edit resurrects a card Rust deleted. The stale
        // task fields on that echo are dropped.
        let mut disk = PersistedState::default();
        disk.card_tombstones.push(CardTombstone {
            id: "c-old".into(),
            last_touched_at: 3.0,
        });
        let mut incoming = PersistedState::default();
        let mut c = touch_card("c-old", 9.0, Some("back"), None);
        c.task_id = Some("t-ghost".into());
        c.task_reason = Some("done".into());
        incoming.kanban_cards.push(c);
        let (out, _) = merge_window_save(&disk, &incoming);
        let c = out.kanban_cards.iter().find(|c| c.id == "c-old").unwrap();
        assert!(c.task_id.is_none());
        assert!(c.task_reason.is_none());
        assert_eq!(c.notes.as_deref(), Some("back"));
    }

    #[test]
    fn agent_terminal_explicit_window_wins_omitted_keeps_disk() {
        let mut disk = PersistedState::default();
        disk.agent_terminal = "terminal".into();
        let mut incoming = PersistedState::default();
        incoming.agent_terminal = "ghostty".into();
        let (out, _) = merge_window_save(&disk, &incoming);
        assert_eq!(out.agent_terminal, "ghostty");

        let mut omitted = PersistedState::default();
        omitted.agent_terminal.clear();
        let (out, _) = merge_window_save(&disk, &omitted);
        assert_eq!(out.agent_terminal, "terminal");

        let before = PersistedState::default();
        let mut after = before.clone();
        after.agent_terminal = "terminal".into();
        let mut live = PersistedState::default();
        apply_writer_delta(&mut live, &before, &after);
        assert_eq!(live.agent_terminal, "terminal");
    }

    #[test]
    fn writer_delta_lists_cover_every_field() {
        let src = include_str!("state.rs");
        let top = struct_fields(src, "pub struct PersistedState {");
        let comp = struct_fields(src, "pub struct CompanionState {");
        assert!(top.len() >= 40 && comp.len() >= 20, "parser found {} / {}", top.len(), comp.len());
        let writer = fn_body(src, "pub fn apply_writer_delta(");
        let companion = fn_body(src, "fn apply_companion_delta(");
        let miss_top: Vec<_> = top
            .iter()
            .filter(|f| !names_field(writer, f))
            .collect();
        let miss_comp: Vec<_> = comp
            .iter()
            .filter(|f| !names_field(companion, f))
            .collect();
        assert!(
            miss_top.is_empty() && miss_comp.is_empty(),
            "add to apply_writer_delta: {miss_top:?}; to apply_companion_delta: {miss_comp:?}"
        );
    }

    /// Old saves that picked the removed 3D body still load and land on
    /// the Spirit; other picks and unknown old fields are untouched/ignored.
    #[test]
    fn old_save_with_retired_body_loads_as_spirit() {
        let path = temp_state();
        std::fs::write(
            &path,
            r#"{"themeId":"x","companion":{"imagePackId":"tang","lookAtTarget":{"x":1},"lookAt":true},"kanbanCards":[]}"#,
        )
        .unwrap();
        let st = load_path(&path).expect("old save loads");
        assert_eq!(st.companion.image_pack_id, None);
        std::fs::write(&path, r#"{"companion":{"imagePackId":"female"}}"#).unwrap();
        assert_eq!(load_path(&path).unwrap().companion.image_pack_id.as_deref(), Some("female"));
        std::fs::write(&path, r#"{"companion":{"imagePackId":"my-pack"}}"#).unwrap();
        assert_eq!(load_path(&path).unwrap().companion.image_pack_id.as_deref(), Some("my-pack"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn eight_threads_keep_every_write() {
        let path = temp_state();
        let _ = std::fs::remove_file(&path);
        std::thread::scope(|s| {
            for i in 0..8 {
                let path = path.clone();
                s.spawn(move || {
                    update_file(&path, |st| {
                        st.kanban_cards.push(touch_card(&format!("c{i}"), i as f64, None, None));
                        Ok(())
                    })
                    .unwrap();
                });
            }
        });
        let st = load_path(&path).unwrap();
        let mut ids: Vec<_> = st.kanban_cards.iter().map(|c| c.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7"]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn pickup_race_keeps_run_row_200() {
        let path = temp_state();
        for i in 0..200 {
            save_path(
                &path,
                &PersistedState {
                    kanban_cards: vec![touch_card("c1", 1.0, None, None)],
                    ..PersistedState::default()
                },
            )
            .unwrap();
            std::thread::scope(|s| {
                let path_run = path.clone();
                s.spawn(move || {
                    update_file(&path_run, |st| {
                        st.agent_runs.push(run_row("c1"));
                        Ok(())
                    })
                    .unwrap();
                });
                let path_hack = path.clone();
                s.spawn(move || {
                    update_file(&path_hack, |st| {
                        st.companion.hack_live = true;
                        st.companion.hack_who = "cursor".into();
                        st.companion.hack_project_id = Some("p".into());
                        Ok(())
                    })
                    .unwrap();
                });
            });
            let st = load_path(&path).unwrap();
            assert!(
                st.agent_runs.iter().any(|r| r.card_id == "c1"),
                "run dropped on iter {i}"
            );
            assert!(st.companion.hack_live, "hack cleared on iter {i}");
            assert_eq!(st.companion.hack_who, "cursor");
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn stale_hack_snapshot_does_not_drop_run() {
        let mut live = PersistedState::default();
        live.agent_runs.push(run_row("c1"));
        live.kanban_cards.push(touch_card("c1", 5.0, Some("disk"), None));
        let before = PersistedState {
            kanban_cards: vec![touch_card("c1", 5.0, Some("disk"), None)],
            ..PersistedState::default()
        };
        let mut after = before.clone();
        after.companion.hack_live = true;
        after.companion.hack_who = "cursor".into();
        apply_writer_delta(&mut live, &before, &after);
        assert_eq!(live.agent_runs.len(), 1);
        assert!(live.companion.hack_live);
        assert_eq!(live.kanban_cards[0].notes.as_deref(), Some("disk"));
    }

    #[test]
    fn writer_run_delete_still_drops_the_row() {
        let mut live = PersistedState::default();
        live.agent_runs.push(run_row("c1"));
        let before = PersistedState {
            agent_runs: vec![run_row("c1")],
            ..PersistedState::default()
        };
        let after = PersistedState::default();
        apply_writer_delta(&mut live, &before, &after);
        assert!(live.agent_runs.is_empty());
    }

    #[test]
    fn fresh_handoff_is_empty_and_off() {
        let c = CompanionState::default();
        assert!(c.handoff_backend.is_empty());
        assert!(!c.auto_handoff);
        let partial: CompanionState =
            serde_json::from_str(r#"{"visible":true,"freeMode":true}"#).unwrap();
        assert!(partial.handoff_backend.is_empty());
        assert!(!partial.auto_handoff);
        assert!(partial.free_mode);
        let saved = serde_json::to_value(&c).unwrap();
        assert_eq!(saved["handoffBackend"], "");
        assert_eq!(saved["autoHandoff"], false);
    }

    #[test]
    fn stale_window_cannot_strip_handoff() {
        let mut disk = PersistedState::default();
        disk.companion.handoff_backend = "goose".into();
        disk.companion.auto_handoff = true;
        let mut incoming = disk.clone();
        incoming.companion.handoff_backend.clear();
        incoming.companion.auto_handoff = false;
        let (merged, _) = merge_window_save(&disk, &incoming);
        assert_eq!(merged.companion.handoff_backend, "goose");
        assert!(merged.companion.auto_handoff);

        let before = PersistedState::default();
        let mut after = before.clone();
        after.companion.reply_lane = "local".into();
        let mut live = PersistedState::default();
        live.companion.handoff_backend = "grok".into();
        live.companion.auto_handoff = true;
        apply_writer_delta(&mut live, &before, &after);
        assert_eq!(live.companion.handoff_backend, "grok");
        assert!(live.companion.auto_handoff);
        assert_eq!(live.companion.reply_lane, "local");
    }

    #[test]
    fn fresh_backend_is_empty_and_stale_window_cannot_strip_it() {
        let c = CompanionState::default();
        assert!(c.backend.is_empty());
        let partial: CompanionState = serde_json::from_str(r#"{"visible":true}"#).unwrap();
        assert!(partial.backend.is_empty());
        let saved = serde_json::to_value(&c).unwrap();
        assert_eq!(saved["backend"], "");

        let mut disk = PersistedState::default();
        disk.companion.backend = "hermes".into();
        let mut incoming = disk.clone();
        incoming.companion.backend.clear();
        let (merged, _) = merge_window_save(&disk, &incoming);
        assert_eq!(merged.companion.backend, "hermes");

        let before = PersistedState::default();
        let after = before.clone();
        let mut live = PersistedState::default();
        live.companion.backend = "hermes".into();
        apply_writer_delta(&mut live, &before, &after);
        assert_eq!(live.companion.backend, "hermes");

        let mut changed = before.clone();
        changed.companion.backend = "grok".into();
        apply_writer_delta(&mut live, &before, &changed);
        assert_eq!(live.companion.backend, "grok");
    }

    #[test]
    fn direct_endpoint_fields_are_covered_and_the_key_is_not() {
        let src = include_str!("state.rs");
        let comp = struct_fields(src, "pub struct CompanionState {");
        assert!(comp.iter().any(|f| f == "direct_base_url"));
        assert!(comp.iter().any(|f| f == "direct_model"));
        assert!(comp.iter().all(|f| !f.to_ascii_lowercase().contains("key")));
        let companion = fn_body(src, "fn apply_companion_delta(");
        assert!(names_field(companion, "direct_base_url"));
        assert!(names_field(companion, "direct_model"));
        assert!(!companion.contains("direct_key"));
        let c = CompanionState::default();
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["directBaseUrl"], "");
        assert_eq!(v["directModel"], "");
        assert!(v.get("directKey").is_none());
    }

    #[test]
    fn window_merge_stale_card_loses_and_rust_slices_stay() {
        let mut disk = PersistedState::default();
        disk.kanban_cards.push(touch_card(
            "c1",
            10.0,
            Some("newer notes"),
            Some(vec![CardLink {
                url: "https://example.com/a".into(),
                label: None,
            }]),
        ));
        disk.agent_runs.push(run_row("c1"));
        disk.companion.hack_live = true;
        disk.companion.hack_who = "cursor".into();
        disk.companion.hack_project_id = Some("p".into());
        disk.companion.free_mode = true;
        disk.companion.conductor_on = true;
        disk.companion.conductor_model = "built-in".into();
        disk.companion.reply_lane = "free".into();
        disk.companion.reply_model = "e4b".into();
        disk.companion.handoff_backend = "goose".into();
        disk.companion.auto_handoff = true;
        disk.companion.backend = "hermes".into();
        disk.companion.direct_base_url = "https://example.test/v1".into();
        disk.companion.direct_model = "m".into();
        disk.shift_on = true;
        disk.peer_chrome = true;
        disk.last_claim_project_id = Some("planet".into());

        let mut incoming = disk.clone();
        incoming.kanban_cards[0] = touch_card("c1", 4.0, Some("stale"), None);
        incoming.agent_runs.clear();
        incoming.companion.hack_live = false;
        incoming.companion.hack_who.clear();
        incoming.companion.hack_project_id = None;
        incoming.companion.free_mode = false;
        incoming.companion.conductor_on = false;
        incoming.companion.conductor_model.clear();
        incoming.companion.reply_lane.clear();
        incoming.companion.reply_model.clear();
        incoming.companion.handoff_backend.clear();
        incoming.companion.auto_handoff = false;
        incoming.companion.backend.clear();
        incoming.companion.direct_base_url.clear();
        incoming.companion.direct_model.clear();
        incoming.shift_on = false;
        incoming.peer_chrome = false;
        incoming.last_claim_project_id = None;

        let (merged, stats) = merge_window_save(&disk, &incoming);
        assert_eq!(merged.kanban_cards[0].notes.as_deref(), Some("newer notes"));
        assert_eq!(
            merged.kanban_cards[0].links.as_ref().map(|l| l.len()),
            Some(1)
        );
        assert_eq!(merged.agent_runs.len(), 1);
        assert!(merged.companion.hack_live);
        assert_eq!(merged.companion.hack_who, "cursor");
        assert_eq!(merged.companion.hack_project_id.as_deref(), Some("p"));
        assert!(merged.companion.free_mode);
        assert!(merged.companion.conductor_on);
        assert_eq!(merged.companion.conductor_model, "built-in");
        assert_eq!(merged.companion.reply_lane, "free");
        assert_eq!(merged.companion.reply_model, "e4b");
        assert_eq!(merged.companion.handoff_backend, "goose");
        assert!(merged.companion.auto_handoff);
        assert_eq!(merged.companion.backend, "hermes");
        assert_eq!(merged.companion.direct_base_url, "https://example.test/v1");
        assert_eq!(merged.companion.direct_model, "m");
        assert!(merged.shift_on);
        assert!(merged.peer_chrome);
        assert_eq!(merged.last_claim_project_id.as_deref(), Some("planet"));
        assert_eq!(stats.kept_disk, 1);
        assert_eq!(stats.took_incoming, 0);
    }

    #[test]
    fn window_merge_newer_delete_and_link_remove() {
        let mut disk = PersistedState::default();
        disk.kanban_cards.push(touch_card(
            "keep",
            3.0,
            None,
            Some(vec![CardLink {
                url: "https://example.com/old".into(),
                label: None,
            }]),
        ));
        disk.kanban_cards.push(touch_card("gone", 2.0, None, None));
        disk.kanban_cards.push(touch_card("stay", 8.0, Some("disk"), None));

        let mut incoming = PersistedState::default();
        // Link remove bumps lastTouchedAt, so the incoming card is newer.
        let mut edited = touch_card("keep", 9.0, None, None);
        edited.links = None;
        incoming.kanban_cards.push(edited);
        incoming.kanban_cards.push(touch_card("stay", 1.0, Some("stale rail"), None));
        incoming.card_tombstones.push(CardTombstone {
            id: "gone".into(),
            last_touched_at: 4.0,
        });
        // A card the window never saw must survive an omit with no tombstone.
        // `stay` is present but older — disk wins. Add `fresh` only on disk
        // by not listing it here; it is on disk below.
        disk.kanban_cards.push(touch_card("fresh", 6.0, Some("born on disk"), None));

        let (merged, stats) = merge_window_save(&disk, &incoming);
        let ids: Vec<_> = merged.kanban_cards.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"keep"), "{ids:?}");
        assert!(!ids.contains(&"gone"), "delete should win: {ids:?}");
        assert!(ids.contains(&"fresh"), "omit without tombstone resurrected by absence: {ids:?}");
        let keep = merged.kanban_cards.iter().find(|c| c.id == "keep").unwrap();
        assert!(keep.links.is_none(), "newer link remove should stick");
        let stay = merged.kanban_cards.iter().find(|c| c.id == "stay").unwrap();
        assert_eq!(stay.notes.as_deref(), Some("disk"));
        assert!(stats.took_incoming >= 1);
        assert!(stats.kept_disk >= 1);
        assert!(merged.card_tombstones.iter().any(|t| t.id == "gone"));
    }

    #[test]
    fn window_merge_old_tombstone_does_not_block_newer_card() {
        let mut disk = PersistedState::default();
        disk.card_tombstones.push(CardTombstone {
            id: "c1".into(),
            last_touched_at: 2.0,
        });
        let mut incoming = PersistedState::default();
        incoming.kanban_cards.push(touch_card("c1", 5.0, Some("back"), None));
        let (merged, _) = merge_window_save(&disk, &incoming);
        assert_eq!(merged.kanban_cards.len(), 1);
        assert!(merged.card_tombstones.iter().all(|t| t.id != "c1"));
    }

    #[test]
    fn rail_board_cols_default_and_delta() {
        let missing: PersistedState = serde_json::from_str("{}").unwrap();
        assert_eq!(missing.rail_board_cols, vec!["todo".to_string()]);

        let round: PersistedState =
            serde_json::from_str(r#"{"railBoardCols":["todo","done"]}"#).unwrap();
        assert_eq!(
            round.rail_board_cols,
            vec!["todo".to_string(), "done".to_string()]
        );
        let encoded = serde_json::to_value(&round).unwrap();
        assert_eq!(encoded["railBoardCols"], serde_json::json!(["todo", "done"]));

        let mut before = PersistedState::default();
        before.rail_board_cols = vec!["doing".into()];
        let mut after = before.clone();
        after.rail_board_cols = vec!["done".into()];
        let mut live = PersistedState::default();
        apply_writer_delta(&mut live, &before, &after);
        assert_eq!(live.rail_board_cols, vec!["done".to_string()]);

        let mut kept = PersistedState::default();
        kept.rail_board_cols = vec!["keep".into()];
        apply_writer_delta(&mut kept, &after, &after);
        assert_eq!(kept.rail_board_cols, vec!["keep".to_string()]);
    }

    #[test]
    fn rail_stage_prefs_default_left_and_survive_window_save() {
        let missing: PersistedState = serde_json::from_str("{}").unwrap();
        assert_eq!(missing.rail_edge, "left");
        assert!(missing.rail_auto_hide_on_blur);
        assert!(missing.autohide_row_chrome);
        assert_eq!(normalize_rail_edge("nope"), "left");
        assert_eq!(normalize_rail_edge("right"), "right");

        let mut disk = PersistedState::default();
        disk.rail_edge = "bottom".into();
        disk.rail_auto_hide_on_blur = false;
        disk.autohide_row_chrome = true;
        let mut incoming = PersistedState::default();
        incoming.rail_edge = "left".into();
        incoming.rail_auto_hide_on_blur = true;
        incoming.autohide_row_chrome = false;
        let (merged, _) = merge_window_save(&disk, &incoming);
        assert_eq!(merged.rail_edge, "bottom");
        assert!(!merged.rail_auto_hide_on_blur);
        assert!(!merged.autohide_row_chrome);

        let (x, y, w, h) = rail_stage_frame("left", false, 0.0, 0.0, 1400.0, 900.0, 360.0, 420.0);
        assert_eq!((x, y, w, h), (-348.0, 240.0, 360.0, 420.0));
        let (x, _, _, _) = rail_stage_frame("left", true, 0.0, 0.0, 1400.0, 900.0, 360.0, 420.0);
        assert_eq!(x, 8.0);
        let (x, _, _, _) = rail_stage_frame("right", false, 0.0, 0.0, 1400.0, 900.0, 360.0, 420.0);
        assert_eq!(x, 1388.0);
        let (_, y, _, _) = rail_stage_frame("top", false, 0.0, 20.0, 1400.0, 900.0, 360.0, 420.0);
        assert_eq!(y, 20.0 - 420.0 + 12.0);
        let (_, y, _, _) = rail_stage_frame("bottom", true, 0.0, 0.0, 1400.0, 900.0, 360.0, 420.0);
        assert_eq!(y, 900.0 - 420.0 - 8.0);

        let before = PersistedState::default();
        let mut after = before.clone();
        after.rail_edge = "top".into();
        after.rail_auto_hide_on_blur = false;
        let mut live = PersistedState::default();
        apply_writer_delta(&mut live, &before, &after);
        assert_eq!(live.rail_edge, "top");
        assert!(!live.rail_auto_hide_on_blur);
    }

    #[test]
    fn rail_wheel_size_hugs_wheel_and_board() {
        assert_eq!(rail_wheel_size("left", true, 1440.0, 875.0), (1040.0, 760.0));
        assert_eq!(rail_wheel_size("right", true, 1440.0, 875.0), (1040.0, 760.0));
        assert_eq!(rail_wheel_size("top", true, 1440.0, 875.0), (1040.0, 760.0));
        assert_eq!(rail_wheel_size("bottom", true, 1440.0, 875.0), (1040.0, 760.0));
        assert_eq!(rail_wheel_size("left", true, 1000.0, 700.0), (992.0, 700.0));
        assert_eq!(rail_wheel_size("top", true, 1000.0, 700.0), (1000.0, 692.0));
        assert_eq!(rail_wheel_size("left", false, 1440.0, 875.0), (360.0, 420.0));
        assert_eq!(rail_wheel_size("right", false, 100.0, 100.0), (360.0, 420.0));
        assert_eq!(rail_wheel_size("top", false, 1440.0, 875.0), (360.0, 420.0));
        assert_eq!(rail_wheel_size("bottom", false, 8.0, 8.0), (360.0, 420.0));
        assert_eq!(rail_wheel_size("nope", true, 1440.0, 875.0), (1040.0, 760.0));
        let (x, y, w, h) = rail_stage_frame("left", true, 0.0, 0.0, 1440.0, 875.0, 1040.0, 760.0);
        assert_eq!((x, y, w, h), (8.0, 57.5, 1040.0, 760.0));
    }

    #[test]
    fn starter_story_is_rust_owned_and_survives_window_save() {
        let mut disk = PersistedState::default();
        disk.starter_story.v = 1;
        disk.starter_story.seeded_project_id = Some("p1".into());
        disk.starter_story.comms_built = true;
        disk.starter_story.stars_built = true;
        disk.starter_story.coach_done = true;
        disk.starter_story.intro_done = false;
        let incoming = PersistedState::default();
        let (merged, _) = merge_window_save(&disk, &incoming);
        assert_eq!(merged.starter_story.v, 1);
        assert_eq!(merged.starter_story.seeded_project_id.as_deref(), Some("p1"));
        assert!(merged.starter_story.comms_built);
        assert!(merged.starter_story.stars_built);
        assert!(merged.starter_story.coach_done);
        assert!(!merged.starter_story.intro_done);
        assert!(!merged.starter_story.skipped);
    }

    #[test]
    fn starter_story_serializes_to_the_camel_case_key_contract() {
        let story = StarterStory {
            v: 1,
            skipped: true,
            coach_done: true,
            seeded_project_id: Some("p".into()),
            comms_built: true,
            stars_built: true,
            intro_done: true,
        };
        let value = serde_json::to_value(&story).unwrap();
        let obj = value.as_object().unwrap();
        let keys: std::collections::BTreeSet<_> = obj.keys().cloned().collect();
        let expect: std::collections::BTreeSet<_> = [
            "v",
            "skipped",
            "coachDone",
            "seededProjectId",
            "commsBuilt",
            "starsBuilt",
            "introDone",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        assert_eq!(keys, expect);
        assert_eq!(obj.get("v").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(obj.get("skipped").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(obj.get("coachDone").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            obj.get("seededProjectId").and_then(|v| v.as_str()),
            Some("p")
        );
        assert_eq!(obj.get("commsBuilt").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(obj.get("starsBuilt").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(obj.get("introDone").and_then(|v| v.as_bool()), Some(true));
    }
}
