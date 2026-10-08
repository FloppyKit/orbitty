//! Persisted planet world positions (worlds.json).
//!
//! Extracted from habitat_mac/mod.rs (DIET-HABITAT).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One placed plant/mushroom: disc-local unit coords (-1..1) + birth time
/// for the 7-day growth curve.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct DecorItem {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) planted_at: f64,
}

/// Boombox placement (no growth — it just sits there and plays).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct DecorPoint {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct WorldState {
    pub(crate) screen_id: String,
    /// Screen-local, flipped (y down) center coords.
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) last_used: f64,
    /// Scroll-zoomed expanded radius; 0 = never zoomed (EXPANDED_RADIUS).
    pub(crate) radius: f64,
    /// Pre-GROW toggles, migrated into `plants`/`mushrooms` placements on
    /// load (true + empty list → one default placement, then cleared).
    pub(crate) plant: bool,
    pub(crate) mushroom: bool,
    /// Boombox playing flag (kept): on + `music` set → AVAudioPlayer loop
    /// while the planet lives. Placement lives in `boombox_pos`.
    pub(crate) boombox: bool,
    pub(crate) music: String,
    /// GO-ORBITTY-BOOMBOX remembered volume step (Quiet .25 / Low .5 /
    /// Med .75 / High 1.0). Field-level default keeps pre-fader saves at
    /// full volume — the container `default` would give 0.0.
    #[serde(default = "default_boombox_volume")]
    pub(crate) boombox_volume: f32,
    /// the HUD speaker's hard mute.
    /// The player is kept, faded to 0; unmute returns to the slider's
    /// remembered multiplier. Container default keeps old saves unmuted.
    pub(crate) boombox_muted: bool,
    /// Legacy field. Live disc ignores this — GO-ORBITTY-PROJ-LOCK derives
    /// projection from marble vs expanded. Rewritten on sync to heal.
    pub(crate) projection: String,
    /// First time this world was seen; 0 = pre-GROW save (set to now on
    /// load). The decor budget grows from this.
    pub(crate) first_seen: f64,
    pub(crate) plants: Vec<DecorItem>,
    pub(crate) mushrooms: Vec<DecorItem>,
    pub(crate) buildings: Vec<DecorItem>,
    pub(crate) boombox_pos: Option<DecorPoint>,
    /// per-planet beach-ball keep-up
    /// toggle (planet right-click menu, checkable). Default off — the
    /// container `default` gives pre-leftover saves false. Live flight
    /// state is in-memory only (`Planet::ball`).
    pub(crate) beach_ball: bool,
}

pub(crate) fn default_boombox_volume() -> f32 {
    1.0
}

/// Additive migration for pre-GROW saves; returns true when it changed
/// anything (caller persists). Idempotent.
pub(crate) fn migrate_world(w: &mut WorldState, now: f64) -> bool {
    let mut dirty = false;
    if w.first_seen <= 0.0 {
        w.first_seen = now;
        dirty = true;
    }
    if w.plant {
        if w.plants.is_empty() {
            w.plants.push(DecorItem {
                x: -0.42,
                y: 0.34,
                planted_at: now,
            });
        }
        w.plant = false;
        dirty = true;
    }
    if w.mushroom {
        if w.mushrooms.is_empty() {
            w.mushrooms.push(DecorItem {
                x: 0.42,
                y: 0.34,
                planted_at: now,
            });
        }
        w.mushroom = false;
        dirty = true;
    }
    // An "on" boombox from before placements becomes a placed boombox that
    // keeps playing.
    if w.boombox && w.boombox_pos.is_none() {
        w.boombox_pos = Some(DecorPoint { x: 0.0, y: 0.62 });
        dirty = true;
    }
    dirty
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct WorldsFile {
    pub(crate) worlds: HashMap<String, WorldState>,
}
