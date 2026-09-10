//! Habitat — one borderless planet panel per project that has a home claim.
//! The frontend owns claims/projects/collapse state and pushes a snapshot;
//! the platform side owns panels, sprites, drag, and planet positions.
//! Tags never reach this module: only home-claimed windows become sprites.

use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HabitatEntry {
    pub id: String,
    pub app_name: String,
    pub title: String,
    pub bundle_id: Option<String>,
    pub minimized: bool,
    pub icon_base64: Option<String>,
    /// Zombie age class from the rail's lastTouchedAt: fresh | fade | park.
    /// `gone` entries are omitted by the rail and never reach the habitat.
    #[serde(default = "default_age")]
    pub age: String,
    /// Sprite picker (claim menu): packs/<id> folder; empty = the generated
    /// being (GO-ORBITTY-CHARS). Unknown ids (pack removed) also fall back
    /// to the generated being.
    #[serde(default)]
    pub sprite_pack_id: String,
    /// GO-ORBITTY-CHARS: generated-person roll seed (0 = pre-GO snapshot;
    /// still derives a stable look from seed 0).
    #[serde(default)]
    pub char_seed: u64,
    /// Explicit layer picks from the Being editor; None = derive from seed.
    #[serde(default)]
    pub char_look: Option<crate::chars::CharLook>,
    /// Per-walker size (0.4–2.5); 0 = inherit the gear's global spriteScale.
    #[serde(default)]
    pub sprite_scale: f64,
    /// GO-ORBITTY-SLEEPERS: the claim's window is closed — the walker
    /// sleeps (idle frame 0, dim, no bounce) until Done/Unclaim, and a
    /// double-click wakes it (file → session → recipe → app), never a bare
    /// restore_window on a dead id.
    #[serde(default)]
    pub asleep: bool,
    // GO-ORBITTY-CLAIM-ONE: walker_home (the occupancy teleport) is gone —
    // occupancy is the entry's planet, i.e. the claim's projectId. One
    // windowId, one projectId.
    /// GO-ORBITTY-CADENCE: the window-linked card's column
    /// ("todo" | "doing" | "done"; "" = no linked card). The rail resolves
    /// which card wins; the native side holds the status → station/pose
    /// table. Needed cards never land here (their status is a dummy).
    #[serde(default)]
    pub card_status: String,
    /// The linked card's kind ("window" | "todo"; "" = no card).
    /// Informational — the pose table keys off card_status + needed_state.
    #[serde(default)]
    pub card_kind: String,
    /// GO-ORBITTY-CADENCE: "open" | "later" when an open/later Needed card
    /// names this window or its project (Needed sits the walker, and wins
    /// over Doing); "" = nothing needed from the user here.
    #[serde(default)]
    pub needed_state: String,
}

fn default_age() -> String {
    "fresh".to_string()
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HabitatPlanet {
    pub project_id: String,
    pub name: String,
    pub collapsed: bool,
    pub entries: Vec<HabitatEntry>,
    /// GO-ORBITTY-WORLD-TOWN: the project board's ticks for the back-board
    /// blit — id + column only, no titles. The rail fills from kanbanCards
    /// on this projectId (kind !== "needed"), capped ~12: oldest open
    /// first, then recent done.
    #[serde(default)]
    pub board_cards: Vec<BoardCardTick>,
}

/// GO-ORBITTY-WORLD-TOWN: one board tick — a card's id (hashed for the
/// tick color) and its column ("todo" | "doing" | "done").
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BoardCardTick {
    pub id: String,
    pub status: String,
    /// Window this tick is linked to, if any. Native holds the blit on
    /// "doing" until that walker touches the board (WalkOff + via_board).
    #[serde(default)]
    pub window_id: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HabitatSnapshot {
    pub planets: Vec<HabitatPlanet>,
    /// Gear walker-size slider (0.4–2.5, default 1.0).
    #[serde(default = "default_sprite_scale")]
    pub sprite_scale: f64,
    /// GO-ORBITTY-MARBLE-DRIFT: the Settings toggle rides the snapshot —
    /// one path, so the native side never reads state.json for this.
    #[serde(default)]
    pub marble_drift: bool,
    /// All named planets (including empty ones) so the walker Assign-to
    /// submenu can list destinations that currently have no walkers.
    #[serde(default)]
    pub all_planets: Vec<HabitatPlanetName>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HabitatPlanetName {
    pub project_id: String,
    pub name: String,
}

fn default_sprite_scale() -> f64 {
    1.0
}

#[cfg(target_os = "macos")]
#[path = "habitat_mac/mod.rs"]
mod habitat_mac;

#[cfg(not(target_os = "macos"))]
#[path = "habitat_stub.rs"]
mod habitat_mac;

pub use habitat_mac::{
    boombox_playing, bring_all_front, bring_planet_front, char_preview, locate_planet,
    locate_walker, persist_now, reload_pack_frames, restore_all, start, sync,
};
