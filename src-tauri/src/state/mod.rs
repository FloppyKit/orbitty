//! Persistence — one JSON file under the app data dir.
//! Additive fields: old saves load. Whole-object save must not strip
//! unknown keys. No PHI. AX titles are never rewritten.

use serde::{Deserialize, Serialize};


mod rail_geom;
mod models;
mod models_companion;
mod models_board;
mod io;
mod merge;
pub(crate) mod instance;

pub(crate) use rail_geom::*;
pub(crate) use models::*;
pub(crate) use models_companion::*;
pub(crate) use models_board::*;
pub(crate) use io::*;
pub(crate) use merge::*;

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

#[cfg(test)]
mod tests;
