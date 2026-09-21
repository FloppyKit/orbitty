//! Habitat core types: Being / Planet / Habitat / PackFrames (DIET-HABITAT).
//!
//! Extracted from habitat_mac/mod.rs. Drag / Placement / PendingClick stay
//! in mod.rs (enum variant fields are module-private).

#![allow(unexpected_cfgs)]

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::audio;
use super::ball::BallState;
use super::decor::DecorRef;
use super::drift::{ClusterState, MarbleOrbit};
use super::droid;
use super::droid_pack;
use super::pose::Pose;
use super::roam::RoamState;
use super::todo_tree;
use super::world::{DecorItem, DecorPoint, WorldState};
use super::{Drag, PendingClick, Placement, NSPoint, NSRect, Projection};
use crate::habitat::HabitatEntry;

pub(crate) struct Being {
    pub(crate) entry_id: String,
    /// "App — Title" for the click caption (titles only, truncated).
    pub(crate) label: String,
    /// View-local, flipped coords; recomputed on layout.
    pub(crate) frame: NSRect,
    pub(crate) bob: f64,
    /// Retained NSImage, or 0. Owned by the session icon cache.
    pub(crate) icon: usize,
    /// the generated person's layer picks (resolved at
    /// sync from the claim's charLook, else derived from charSeed).
    pub(crate) look: crate::chars::CharLook,
    /// Per-walker size multiplier from the claim (1.0 = follow the gear's
    /// global spriteScale).
    pub(crate) user_scale: f64,
    /// Index into Habitat.pack_frames when user packs supply frames; None =
    /// draw the generated person.
    pub(crate) pack: Option<usize>,
    /// Zombie fade (TS-7): 1.0 fresh, dimmed for 7+ days untouched. Parked
    /// entries never become walkers — they occupy like dots (see do_sync).
    pub(crate) dim: f64,
    /// the claim's window is closed. The walker sleeps
    /// (idle frame 0, dimmed, no roam/bounce) and double-click wakes it via
    /// coordinator::wake_sleeper_with instead of a dead restore_window.
    pub(crate) asleep: bool,
    /// The claim's raw spritePackId ("" = generated being) — the walker
    /// right-click menu checks the current pick against this.
    pub(crate) sprite_pack_id: String,
    /// the board-driven pose, resolved from the
    /// snapshot's card fields at sync. Rebuilt every sync; the roam state
    /// (position) lives outside so a re-sync never teleports anyone.
    pub(crate) pose: Pose,
    /// Some(seed) = an agent walker — the draw
    /// blits the baked droid atlas (droid.rs) instead of the pack/person
    /// art. The seed hashes the run's `planetId:cardId`.
    pub(crate) droid: Option<u64>,
    /// 0 working, 1 blocked, 2 needs-you.
    pub(crate) droid_glance: u8,
}

pub(crate) struct Planet {
    pub(crate) panel: usize, // NSPanel
    pub(crate) view: usize,  // LiveSpriteWorldView
    /// Display name from the rail snapshot — walker Assign-to-planet menu.
    pub(crate) name: String,
    pub(crate) radius: f64,
    pub(crate) sprite_scale: f64,
    pub(crate) is_marble: bool,
    pub(crate) color: (f64, f64, f64),
    pub(crate) rotation: f64,
    pub(crate) projection: Projection,
    pub(crate) walkers: Vec<Being>,
    pub(crate) dots: Vec<Being>,
    /// Transient native caption pill (single-click feedback), ~2s.
    pub(crate) caption: Option<(String, Instant)>,
    /// the cursor is over a marble's disc — draw
    /// the planet name as the caption pill whenever no click caption is
    /// live. Set by the cursor watcher (panels ignore mouse events
    /// off-disc, so AppKit tracking areas would never see the enter).
    pub(crate) hover_name: bool,
    /// sun membership + join ease; cleared by
    /// isolation and when motion is stilled. The track itself (radius,
    /// phase, period) is hashed from the marble's id on the fly — these
    /// fields only carry the sun key and the ease-in window.
    pub(crate) orbit: Option<MarbleOrbit>,
    /// Drawn radius multiplier. GO-ORBITTY-MARBLE-SOLAR killed the fake-Z
    /// scale-throb — this stays 1.0; the field remains so the draw and
    /// hit-test paths keep one expression. In-memory only.
    pub(crate) marble_scale: f64,
    /// the dropped marble sits until this
    /// instant, then may rejoin its ring. Drag wins.
    pub(crate) pin_until: Option<Instant>,
    /// Decor placements, mirrored from the world state at sync/mutation.
    pub(crate) plants: Vec<DecorItem>,
    pub(crate) mushrooms: Vec<DecorItem>,
    pub(crate) buildings: Vec<DecorItem>,
    pub(crate) boombox_pos: Option<DecorPoint>,
    /// Boombox playing flag, mirrored from the world state.
    pub(crate) boombox: bool,
    /// beach-ball keep-up toggle,
    /// mirrored from the world state (planet right-click menu).
    pub(crate) beach_ball: bool,
    /// Live beach-ball flight; Some only while the toggle is on AND ≥ 2
    /// awake Doing walkers work the grass. In-memory only.
    pub(crate) ball: Option<BallState>,
    /// Live AVAudioPlayer while the boombox is on; stopped + released on
    /// toggle-off, planet teardown, or re-toggle. usize = retained
    /// ObjC pointer; only touched on the main thread.
    pub(crate) audio: Option<usize>,
    /// Last live volume applied (remembered step × zoom gain). Zoom
    /// fades this in place — never restart the file.
    pub(crate) audio_volume: f32,
    /// Last laid-out decor frames (view-local) for hit-testing.
    pub(crate) decor_frames: Vec<(DecorRef, NSRect)>,
    /// Roam state per walker entry id.
    pub(crate) roam: HashMap<String, RoamState>,
    /// The snapshot entries the current walkers/dots were built from
    /// (CADENCE-FIX pose-only fast path): a sync whose being-defining
    /// fields are unchanged updates poses in place instead of rebuilding
    /// every Being — no icon re-decode, roam state untouched.
    pub(crate) walker_entries: Vec<HabitatEntry>,
    pub(crate) dot_entries: Vec<HabitatEntry>,
    /// the back-board's ticks (id + column, no
    /// titles), mirrored from the snapshot. Drawn as the three-column
    /// board blit; marbles never draw it.
    pub(crate) board_cards: Vec<crate::habitat::BoardCardTick>,
    /// Unbound-todo trees (not WORLD-YARD Place-plant).
    pub(crate) todo_trees: Vec<crate::habitat::TodoTreeTick>,
    pub(crate) todo_tree_frames: Vec<(String, NSRect)>,
    pub(crate) tree_grow_days: f64,
    pub(crate) tree_chores: Vec<todo_tree::TreeChore>,
}

#[derive(Default)]
pub(crate) struct Habitat {
    pub(crate) planets: HashMap<String, Planet>,
    pub(crate) worlds: HashMap<String, WorldState>,
    pub(crate) worlds_loaded: bool,
    /// Set by scroll-zoom / decor / boombox mutations; cursor_tick persists
    /// at 10Hz so a scroll gesture can't thrash the disk.
    pub(crate) worlds_dirty: bool,
    pub(crate) drag: Option<Drag>,
    pub(crate) pending_click: Option<PendingClick>,
    /// WORLD-TOUCH leftover: mouse-down on the planet board's Done column.
    pub(crate) pending_board_done: Option<String>,
    /// BOARD-MARK: mouse-down on Todo/Doing columns → open this planet's Launchpad.
    pub(crate) pending_board_open: Option<String>,
    /// the locate stroke — (entry id, armed at).
    /// Drawn as a ~500ms ring around the walker's frame; cursor_tick
    /// expires it (10Hz, so Reduce Motion can't pin it on).
    pub(crate) locate: Option<(String, Instant)>,
    /// Dest-menu find: (project id, armed at). Marble = diagonal saturn
    /// ring; expanded = rim glow + center name fade.
    pub(crate) locate_planet: Option<(String, Instant)>,
    /// Armed by the Place menu items; the next empty-disc click drops it.
    pub(crate) placement: Option<Placement>,
    /// the planet whose boombox HUD
    /// is open (slider + speaker + `new track` pill). In-memory; the HUD
    /// dies with the box (cursor_tick self-heal).
    pub(crate) boombox_hud: Option<String>,
    pub(crate) bob_phase: f64,
    pub(crate) last_anim: Option<Instant>,
    pub(crate) reduce_motion: bool,
    /// Settings → Appearance toggle, mirrored
    /// from the rail's snapshot (default false = marbles sit).
    pub(crate) marble_drift: bool,
    /// frozen barycenter homes, keyed by the
    /// bary sun_key (BARY_PREFIX + sorted member ids). In-memory only;
    /// entries are dropped the tick after their cluster dissolves.
    pub(crate) clusters: HashMap<String, ClusterState>,
    /// Shared drift clock (seconds), advanced EVERY habitat tick (~16ms)
    /// while motion is on so drifting marbles don't step at the 3-tick
    /// walker cadence. bob_phase stays the walker clock.
    pub(crate) drift_phase: f64,
    pub(crate) last_drift: Option<Instant>,
    /// Gear walker-size slider (0.5–1.25, WORLD-PROPS cap). 0 = treat as 1.0.
    pub(crate) sprite_scale: f64,
    /// R-click Planets dest: hide every disc. In-memory; persist is companion.planetsHidden.
    pub(crate) planets_hidden: bool,
    /// named destinations for Assign to planet.
    pub(crate) planet_names: Vec<(String, String)>,
    pub(crate) icon_cache: HashMap<String, usize>, // bundleId/appName → NSImage
    /// User-pack art (GO-LS-TAURI-USER-PACKS): (pack folder id, frames) per
    /// pack — walk cycle plus optional idle cycle for paused walkers.
    /// Walkers reference a pack by id via the claim's spritePackId;
    /// empty/unknown id = the generated person. The roster is re-enumerated
    /// on first sync and after any import/remove, but frames stay UNDECODED
    /// until an expanded planet wears the pack (GO-ORBITTY-PLANET-BUDGET).
    pub(crate) pack_frames: Vec<(String, PackFrames)>,
    pub(crate) packs_loaded: bool,
    /// bundled town art (Resources/town/*),
    /// loaded once into retained NSImages on the first world draw.
    /// 0 = missing → the cheap glyph fallback draws instead.
    pub(crate) town_igloo: usize,
    pub(crate) town_sign: usize,
    pub(crate) town_boombox: usize,
    pub(crate) town_ball: usize,
    pub(crate) town_sundial: usize,
    pub(crate) town_comstack: usize,
    pub(crate) town_art_loaded: bool,
    /// Global walk-cycle counter, +1 per anim tick (20Hz); walkers advance
    /// pack frames at half that rate.
    pub(crate) anim_frame: u64,
    /// Done entries with no walker to walk off
    /// (marble planet, parked dot) already fast-forward-emitted
    /// `habitat:walked-off` — once per Done spell, never per sync. Pruned
    /// when the entry leaves Done so a re-Done emits again.
    pub(crate) walkoff_fast: std::collections::HashSet<String>,
    /// baked droid atlases, keyed by the run's
    /// seed hash. Baked ONCE on first draw (droid.rs), session-cached like
    /// the icon cache — never a per-frame procgen.
    pub(crate) droids: HashMap<u64, droid::DroidAtlas>,
    /// 4-dir painted kits (droid_pack.rs). None until first draw loads
    /// the PNGs; dummy 12×14 still draws if load fails.
    pub(crate) droid_kits: Option<droid_pack::DroidKits>,
}

pub(crate) struct PackFrames {
    pub(crate) walk: Vec<usize>,
    pub(crate) idle: Vec<usize>,
    /// BUDGET lazy packs: the on-disk art source (paths
    /// only). No bitmap is decoded until an expanded planet wears the pack
    /// (`loaded` flips on first wear); the bitmap drops when the last
    /// expanded wearer collapses. Marbles never load a pack.
    pub(crate) art: crate::packs::PackArt,
    pub(crate) loaded: bool,
}

