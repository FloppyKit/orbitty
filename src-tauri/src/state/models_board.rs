use serde::{Deserialize, Serialize};

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

/// Live browser tab bound to a walker claim. Session tab ids stay out of
/// state.json; this is the durable url/title the extension rematches on.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabRef {
    pub browser: String,
    pub url: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fav_icon_url: Option<String>,
    pub last_seen_at: f64,
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
    /// Set only on `tab:` claims. Absent on window claims and old saves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabRef>,
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

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrackerHistory {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<std::collections::BTreeMap<String, i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sessions: Option<TrackerSessions>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrackerSessions {
    pub day: String,
    pub spans: Vec<(i64, i64)>,
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
    /// Optional day totals and today's timer spans. Old saves omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<TrackerHistory>,
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
