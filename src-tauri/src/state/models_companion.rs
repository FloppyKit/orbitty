#![allow(unused_imports)]
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[allow(unused_imports)]
use super::{CardTombstone, PersistedState};
#[allow(unused_imports)]
use super::rail_geom::*;
#[allow(unused_imports)]
use super::models::*;
#[allow(unused_imports)]
use super::models_board::*;
#[allow(unused_imports)]
use super::io::*;
#[allow(unused_imports)]
use super::merge::*;

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

pub(crate) fn companion_visible_default() -> bool {
    true
}

pub(crate) fn talk_source_default() -> String {
    // Shipping default is system speech.
    // A saved "hotkey" still wins so Matt's desk Handy path stays until
    // he picks System. Missing key (old saves) lands here.
    "system".into()
}

pub(crate) fn companion_view_collapsed_default() -> String {
    "full".into()
}

pub(crate) fn companion_view_expanded_default() -> String {
    "head".into()
}

pub(crate) fn companion_sprite_default() -> bool {
    true
}

pub(crate) fn companion_speak_volume_default() -> f32 {
    1.0
}

pub(crate) fn companion_captions_default() -> bool {
    true
}

pub(crate) fn companion_cheat_hint_default() -> bool {
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

pub(crate) fn permissions_default() -> String {
    "read-only".into()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TitleOverride {
    pub window_id: String,
    pub title: String,
}
