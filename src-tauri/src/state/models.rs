use serde::{Deserialize, Serialize};
use super::models_companion::{CompanionBounds};

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

pub(crate) fn drawer_docked_default() -> bool {
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

pub(crate) fn stars_act_split_default() -> f64 {
    0.62
}

pub(crate) fn stars_east_split_default() -> f64 {
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

pub(crate) fn opt_blank(v: &Option<String>) -> bool {
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
    /// Inclusive end of a span page. Absent on old saves = one day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day_key_end: Option<String>,
    /// "overall" or a planet id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric_flags: Option<MetricFlags>,
}

