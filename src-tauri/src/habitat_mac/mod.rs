//! macOS habitat — one NSPanel per claimed planet.
//!
//! Click-through off the disc. Close is a sleeper, not Done. AppKit on
//! the main thread only. Desk cap: ≤3 expanded, ≤12 marbles.

// objc 0.2 macros expand to a `cfg(feature = "cargo-clippy")` probe that newer
// rustc flags; harmless upstream noise.
#![allow(unexpected_cfgs)]

use super::{HabitatEntry, HabitatPlanet, HabitatSnapshot};
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object};
use objc::{class, msg_send, sel, sel_impl};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

mod audio;
mod tick;
mod input;
mod draw;
mod boombox_hud;
mod droid;
mod droid_pack;
mod todo_tree;
mod planet_idle;
mod geom;
mod world;
mod pose;
mod ball;
mod drift;
mod roam;
mod decor;
mod colors;
mod art;
mod marble_motion;
mod anim;
mod layout;

pub(crate) use geom::{
    clamp_disc, disc_to_view, locked_projection, locked_projection_str,
    radius_y, view_to_disc, walker_fit, CGSize, NSPoint, NSRect, Projection,
};
pub(crate) use world::{
    default_boombox_volume, migrate_world, DecorItem, DecorPoint, WorldState, WorldsFile,
};
pub(crate) use pose::{
    droid_glance, hash_str, pose_for, pose_name, sleeper_target, station_anchor, station_target,
    sticky_pose, walkoff_target, yard_clamp, yard_contains, yard_dwell_ms, yard_pick,
    yard_pick_sailing, yard_seed, yard_step, Pose, HOME_HIDE_X, HOME_OFF, SLEEPER_EAST,
    SLEEPER_NORTH, SLEEPER_SOUTH, SLEEPER_WEST, STATION_BOARD, STATION_FIELD, STATION_HOUSE,
    STATION_RIM, YARD_ARRIVE, YARD_DWELL_MAX_MS, YARD_DWELL_MIN_MS, YARD_EAST, YARD_INSET,
    YARD_NORTH, YARD_SAIL, YARD_SOUTH, YARD_WEST,
};
pub(crate) use ball::{
    ball_arc_heads, ball_hop_secs, ball_pos, pack_still_aspect, walker_head_view, BallState,
    BALL_ARC_HEADS_MAX, BALL_ARC_HEADS_MIN,
};

pub(crate) use drift::{
    ClusterState, MarbleOrbit, BARY_PREFIX, BARY_R_MAX, BARY_R_MIN, DRIFT_JOIN_DIST,
    DRIFT_LEAVE_DIST, DRIFT_PIN_SECS, FRONT_DEADBAND, HOST_JOIN_DIST, HOST_LEAVE_DIST,
    HOST_PREFIX, HOST_SWITCH_CLOSER, JOIN_EASE_SECS, OVAL_RATIO, TRACK_T_MAX, TRACK_T_MIN,
};

pub(crate) use roam::RoamState;
pub(crate) use decor::{DecorKind, DecorRef};
pub(crate) use colors::{card_color, color_for};
pub(crate) use art::{
    base64_decode, bitmap_rep_from_bytes, ensure_pack_loaded, ensure_town_art, fallback_pack_index,
    load_pack_frames, ns_image_from_bytes, slice_rep_frames, unload_dead_packs,
};
pub use art::{char_preview, reload_pack_frames};
pub(crate) use layout::{building_cap, layout_beings, layout_decor, make_being, plant_cap};


pub use tick::{persist_now, start};

// pixel sprite art
// walkers are generated people (chars.rs 12×12 glyph
// grids, composited body/clothes/eyes/mouth/hat + palette) drawn per frame
// below. The old blob/cat/robot templates are retired — an empty pack id
// means "generated being", never a blank.

pub(crate) const DOT_SIZE: f64 = 12.0;
const VIEW_PADDING: f64 = 30.0;
pub(crate) const EXPANDED_RADIUS: f64 = 72.0;
const MARBLE_RADIUS: f64 = 18.0;
/// BUDGET desk budget: at most 3 expanded planets (a 4th
/// expand collapses the least-recently focused of the rest) and at most 12
/// collapsed marble panels (13th+ projects stay on the rail list, no panel).
const MAX_EXPANDED: usize = 3;
const MAX_MARBLE_PANELS: usize = 12;
// Scroll-zoom clamp for an expanded planet's radius.
pub(crate) const MIN_RADIUS: f64 = 18.0;
const MAX_RADIUS_FLOOR: f64 = 220.0;
const BASE_SPIN: f64 = std::f64::consts::TAU / 45.0; // one turn per 45s

// Marble drift consts + MarbleOrbit/ClusterState → drift.rs

// Board → station / Pose / yard → pose.rs

// Beach-ball keep-up → ball.rs

/// where a board-bound walker comes FROM — the
/// igloo station, always. User-placed `planet.buildings` stay sandbox
/// décor and are never a spawn. Stand/Work roams (and returns from
/// hide) start here and walk to the station; Sit never spawns here
/// (bad news snaps to the front rim).
/// PROPS leftover (town usable): the doorstep is IN FRONT of the east
/// door, just outside the igloo blit (door x + 0.12; the blit's east
/// edge is ≈ -0.517) — a claimed walker appears on open disc,
/// clickable (right-click menu / locate) the moment it spawns, never
/// inside the PNG. Same y as the door.
pub(crate) fn house_spawn(_planet: &Planet, _entry_id: &str) -> (f64, f64) {
    (STATION_HOUSE.0 + 0.12, STATION_HOUSE.1)
}

/// A Done walker past the hide line: not drawn, not hit-tested — it waits
/// off-screen for the rail's eviction, never a vanish mid-disc.
pub(crate) fn walker_hidden(planet: &Planet, b: &Being) -> bool {
    b.pose == Pose::WalkOff
        && planet
            .roam
            .get(&b.entry_id)
            .is_some_and(|r| r.x < HOME_HIDE_X)
}

/// The walk-off finished: tell the rail so its existing Done eviction
/// (unclaim) lands. Fire-and-forget; the rail's handler is idempotent.
/// Emits are once per walk-off (anim_tick's off_emitted, do_sync's
/// walkoff_fast for walker-less Done entries); do_sync also re-delivers
/// while a walked-off WALKER lingers past the hide line, so a missed
/// emit there heals on the next sync.
pub(crate) fn emit_walked_off(entry_id: &str) {
    if let Some(app) = app_handle() {
        let _ = app.emit_to("main", "habitat:walked-off", entry_id.to_string());
        #[cfg(unix)]
        if let Some(card_id) = entry_id.strip_prefix("agent:") {
            let app = app.clone();
            let card_id = card_id.to_string();
            let _ = std::thread::Builder::new()
                .name("orbitty-shift-continue".into())
                .spawn(move || crate::shift_continue::on_agent_walked_off(&app, &card_id));
        }
    }
}

/// a Todo walker touched the near lip of the
/// board — the rail's listener flips that window's card todo→doing
/// (only if still todo; a human drag always wins). Fire-and-forget;
/// PROPS leftover: anim_tick re-fires on a ~0.5s throttle while the
/// snapshot still says todo (the listener is idempotent), so a missed
/// emit heals instead of parking the walker at the lip. Broadcast to
/// ALL webviews (app.emit), not only main. Not a WalkOff path —
/// via_board/via_home stay Done-only.
pub(crate) fn emit_board_checkin(entry_id: &str) {
    if let Some(app) = app_handle() {
        let _ = app.emit("habitat:board-checkin", entry_id.to_string());
    }
}


/// Decor budget: 3 plants + 3 mushrooms at first sight, +1 each every 2
/// days, capped at 5+5; the boombox is always a single placement.
const DECOR_BASE: usize = 3;
const DECOR_CAP: usize = 5;
const DECOR_STEP_SECS: f64 = 2.0 * 86400.0;


/// (plants, mushrooms) the world may hold right now.
fn decor_budget(first_seen: f64, now: f64) -> (usize, usize) {
    let extra = ((now - first_seen).max(0.0) / DECOR_STEP_SECS) as usize;
    (
        (DECOR_BASE + extra).min(DECOR_CAP),
        (DECOR_BASE + extra).min(DECOR_CAP),
    )
}

// projection geometry (disc helpers live in geom.rs)

pub(crate) fn max_radius() -> f64 {
    let all = unsafe { screens() };
    let (w, h) = all
        .first()
        .map(|(_, f)| (f.size.width, f.size.height))
        .unwrap_or((1440.0, 900.0));
    (w.min(h) * 1.35 / 2.0).max(MAX_RADIUS_FLOOR)
}

// persisted world positions → world.rs

// habitat state (main thread only)

pub(crate) struct Being {
    entry_id: String,
    /// "App — Title" for the click caption (titles only, truncated).
    label: String,
    /// View-local, flipped coords; recomputed on layout.
    frame: NSRect,
    bob: f64,
    /// Retained NSImage, or 0. Owned by the session icon cache.
    icon: usize,
    /// the generated person's layer picks (resolved at
    /// sync from the claim's charLook, else derived from charSeed).
    look: crate::chars::CharLook,
    /// Per-walker size multiplier from the claim (1.0 = follow the gear's
    /// global spriteScale).
    user_scale: f64,
    /// Index into Habitat.pack_frames when user packs supply frames; None =
    /// draw the generated person.
    pack: Option<usize>,
    /// Zombie fade (TS-7): 1.0 fresh, dimmed for 7+ days untouched. Parked
    /// entries never become walkers — they occupy like dots (see do_sync).
    dim: f64,
    /// the claim's window is closed. The walker sleeps
    /// (idle frame 0, dimmed, no roam/bounce) and double-click wakes it via
    /// coordinator::wake_sleeper_with instead of a dead restore_window.
    asleep: bool,
    /// The claim's raw spritePackId ("" = generated being) — the walker
    /// right-click menu checks the current pick against this.
    sprite_pack_id: String,
    /// the board-driven pose, resolved from the
    /// snapshot's card fields at sync. Rebuilt every sync; the roam state
    /// (position) lives outside so a re-sync never teleports anyone.
    pose: Pose,
    /// Some(seed) = an agent walker — the draw
    /// blits the baked droid atlas (droid.rs) instead of the pack/person
    /// art. The seed hashes the run's `planetId:cardId`.
    droid: Option<u64>,
    /// 0 working, 1 blocked, 2 needs-you.
    droid_glance: u8,
}

// DecorKind / DecorRef → decor.rs

// Per-walker RoamState → roam.rs

pub(crate) struct Planet {
    panel: usize, // NSPanel
    view: usize,  // LiveSpriteWorldView
    /// Display name from the rail snapshot — walker Assign-to-planet menu.
    name: String,
    radius: f64,
    sprite_scale: f64,
    is_marble: bool,
    color: (f64, f64, f64),
    rotation: f64,
    projection: Projection,
    walkers: Vec<Being>,
    dots: Vec<Being>,
    /// Transient native caption pill (single-click feedback), ~2s.
    caption: Option<(String, Instant)>,
    /// the cursor is over a marble's disc — draw
    /// the planet name as the caption pill whenever no click caption is
    /// live. Set by the cursor watcher (panels ignore mouse events
    /// off-disc, so AppKit tracking areas would never see the enter).
    hover_name: bool,
    /// sun membership + join ease; cleared by
    /// isolation and when motion is stilled. The track itself (radius,
    /// phase, period) is hashed from the marble's id on the fly — these
    /// fields only carry the sun key and the ease-in window.
    orbit: Option<MarbleOrbit>,
    /// Drawn radius multiplier. GO-ORBITTY-MARBLE-SOLAR killed the fake-Z
    /// scale-throb — this stays 1.0; the field remains so the draw and
    /// hit-test paths keep one expression. In-memory only.
    marble_scale: f64,
    /// the dropped marble sits until this
    /// instant, then may rejoin its ring. Drag wins.
    pin_until: Option<Instant>,
    /// Decor placements, mirrored from the world state at sync/mutation.
    plants: Vec<DecorItem>,
    mushrooms: Vec<DecorItem>,
    buildings: Vec<DecorItem>,
    boombox_pos: Option<DecorPoint>,
    /// Boombox playing flag, mirrored from the world state.
    boombox: bool,
    /// beach-ball keep-up toggle,
    /// mirrored from the world state (planet right-click menu).
    beach_ball: bool,
    /// Live beach-ball flight; Some only while the toggle is on AND ≥ 2
    /// awake Doing walkers work the grass. In-memory only.
    ball: Option<BallState>,
    /// Live AVAudioPlayer while the boombox is on; stopped + released on
    /// toggle-off, planet teardown, or re-toggle. usize = retained
    /// ObjC pointer; only touched on the main thread.
    audio: Option<usize>,
    /// Last live volume applied (remembered step × zoom gain). Zoom
    /// fades this in place — never restart the file.
    audio_volume: f32,
    /// Last laid-out decor frames (view-local) for hit-testing.
    decor_frames: Vec<(DecorRef, NSRect)>,
    /// Roam state per walker entry id.
    roam: HashMap<String, RoamState>,
    /// The snapshot entries the current walkers/dots were built from
    /// (CADENCE-FIX pose-only fast path): a sync whose being-defining
    /// fields are unchanged updates poses in place instead of rebuilding
    /// every Being — no icon re-decode, roam state untouched.
    walker_entries: Vec<HabitatEntry>,
    dot_entries: Vec<HabitatEntry>,
    /// the back-board's ticks (id + column, no
    /// titles), mirrored from the snapshot. Drawn as the three-column
    /// board blit; marbles never draw it.
    board_cards: Vec<crate::habitat::BoardCardTick>,
    /// Unbound-todo trees (not WORLD-YARD Place-plant).
    todo_trees: Vec<crate::habitat::TodoTreeTick>,
    todo_tree_frames: Vec<(String, NSRect)>,
    tree_grow_days: f64,
    tree_chores: Vec<todo_tree::TreeChore>,
}

/// Any Planet drop (stale teardown, marble-budget cap, map clear) ends its
/// song — stop + release, never an orphaned player.
impl Drop for Planet {
    fn drop(&mut self) {
        audio::stop_audio(self);
    }
}

/// What a mouse-down grabbed: the whole planet (move it) or one decor item
/// (drag it somewhere else on the disc).
enum Drag {
    Panel {
        project_id: String,
        grab_dx: f64,
        grab_dy: f64,
        start: NSPoint,
        moved: f64,
    },
    Decor {
        project_id: String,
        decor: DecorRef,
        start: NSPoint,
        moved: f64,
    },
    Walker {
        project_id: String,
        entry_id: String,
        start: NSPoint,
        moved: f64,
    },
    /// HUD slider stroke (BOOMBOX click leftover): the press landed on
    /// the HUD's vertical slider; drag_tick maps mouse y → the
    /// remembered volume. No click/drag question on release.
    BoomboxSlider {
        project_id: String,
    },
    TodoTree {
        project_id: String,
        card_id: String,
        start: NSPoint,
        moved: f64,
    },
}

/// After "Place plant/mushroom/boombox" the next disc click drops it.
#[derive(Clone, Debug)]
struct Placement {
    project_id: String,
    kind: DecorKind,
}

/// Sprite/dot mouse-down pending its mouse-up click check. `walker`
/// distinguishes stowed sprites from open-window dots — single vs double
/// click mean different things on each.
struct PendingClick {
    project_id: String,
    entry_id: String,
    walker: bool,
}

#[derive(Default)]
pub(crate) struct Habitat {
    planets: HashMap<String, Planet>,
    worlds: HashMap<String, WorldState>,
    worlds_loaded: bool,
    /// Set by scroll-zoom / decor / boombox mutations; cursor_tick persists
    /// at 10Hz so a scroll gesture can't thrash the disk.
    worlds_dirty: bool,
    drag: Option<Drag>,
    pending_click: Option<PendingClick>,
    /// WORLD-TOUCH leftover: mouse-down on the planet board's Done column.
    pending_board_done: Option<String>,
    /// BOARD-MARK: mouse-down on Todo/Doing columns → open this planet's Launchpad.
    pending_board_open: Option<String>,
    /// the locate stroke — (entry id, armed at).
    /// Drawn as a ~500ms ring around the walker's frame; cursor_tick
    /// expires it (10Hz, so Reduce Motion can't pin it on).
    locate: Option<(String, Instant)>,
    /// Dest-menu find: (project id, armed at). Marble = diagonal saturn
    /// ring; expanded = rim glow + center name fade.
    locate_planet: Option<(String, Instant)>,
    /// Armed by the Place menu items; the next empty-disc click drops it.
    placement: Option<Placement>,
    /// the planet whose boombox HUD
    /// is open (slider + speaker + `new track` pill). In-memory; the HUD
    /// dies with the box (cursor_tick self-heal).
    boombox_hud: Option<String>,
    bob_phase: f64,
    last_anim: Option<Instant>,
    reduce_motion: bool,
    /// Settings → Appearance toggle, mirrored
    /// from the rail's snapshot (default false = marbles sit).
    marble_drift: bool,
    /// frozen barycenter homes, keyed by the
    /// bary sun_key (BARY_PREFIX + sorted member ids). In-memory only;
    /// entries are dropped the tick after their cluster dissolves.
    clusters: HashMap<String, ClusterState>,
    /// Shared drift clock (seconds), advanced EVERY habitat tick (~16ms)
    /// while motion is on so drifting marbles don't step at the 3-tick
    /// walker cadence. bob_phase stays the walker clock.
    drift_phase: f64,
    last_drift: Option<Instant>,
    /// Gear walker-size slider (0.5–1.25, WORLD-PROPS cap). 0 = treat as 1.0.
    sprite_scale: f64,
    /// R-click Planets dest: hide every disc. In-memory; persist is companion.planetsHidden.
    planets_hidden: bool,
    /// named destinations for Assign to planet.
    planet_names: Vec<(String, String)>,
    icon_cache: HashMap<String, usize>, // bundleId/appName → NSImage
    /// User-pack art (GO-LS-TAURI-USER-PACKS): (pack folder id, frames) per
    /// pack — walk cycle plus optional idle cycle for paused walkers.
    /// Walkers reference a pack by id via the claim's spritePackId;
    /// empty/unknown id = the generated person. The roster is re-enumerated
    /// on first sync and after any import/remove, but frames stay UNDECODED
    /// until an expanded planet wears the pack (GO-ORBITTY-PLANET-BUDGET).
    pack_frames: Vec<(String, PackFrames)>,
    packs_loaded: bool,
    /// bundled town art (Resources/town/*),
    /// loaded once into retained NSImages on the first world draw.
    /// 0 = missing → the cheap glyph fallback draws instead.
    town_igloo: usize,
    town_sign: usize,
    town_boombox: usize,
    town_ball: usize,
    town_art_loaded: bool,
    /// Global walk-cycle counter, +1 per anim tick (20Hz); walkers advance
    /// pack frames at half that rate.
    anim_frame: u64,
    /// Done entries with no walker to walk off
    /// (marble planet, parked dot) already fast-forward-emitted
    /// `habitat:walked-off` — once per Done spell, never per sync. Pruned
    /// when the entry leaves Done so a re-Done emits again.
    walkoff_fast: std::collections::HashSet<String>,
    /// baked droid atlases, keyed by the run's
    /// seed hash. Baked ONCE on first draw (droid.rs), session-cached like
    /// the icon cache — never a per-frame procgen.
    droids: HashMap<u64, droid::DroidAtlas>,
    /// 4-dir painted kits (droid_pack.rs). None until first draw loads
    /// the PNGs; dummy 12×14 still draws if load fails.
    droid_kits: Option<droid_pack::DroidKits>,
}

pub(crate) struct PackFrames {
    walk: Vec<usize>,
    idle: Vec<usize>,
    /// BUDGET lazy packs: the on-disk art source (paths
    /// only). No bitmap is decoded until an expanded planet wears the pack
    /// (`loaded` flips on first wear); the bitmap drops when the last
    /// expanded wearer collapses. Marbles never load a pack.
    art: crate::packs::PackArt,
    loaded: bool,
}

/// xorshift64 for roam pauses (no rand crate on the tree); global so the
/// anim loop can draw numbers while planets are mutably borrowed.
fn rand_unit() -> f64 {
    static RNG: OnceLock<Mutex<u64>> = OnceLock::new();
    let mut g = RNG
        .get_or_init(|| Mutex::new(now_epoch().to_bits() ^ 0x9e37_79b9_7f4a_7c15))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut x = *g;
    if x == 0 {
        x = 0x2545_f491_4f6c_dd1d;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *g = x;
    ((x >> 11) as f64) / ((1u64 << 53) as f64)
}

fn habitat() -> &'static Mutex<Habitat> {
    static H: OnceLock<Mutex<Habitat>> = OnceLock::new();
    H.get_or_init(|| Mutex::new(Habitat::default()))
}

pub(crate) fn lock() -> std::sync::MutexGuard<'static, Habitat> {
    habitat().lock().unwrap_or_else(|e| e.into_inner())
}

fn app_handle_cell() -> &'static OnceLock<AppHandle> {
    static APP: OnceLock<AppHandle> = OnceLock::new();
    &APP
}

fn app_handle() -> Option<&'static AppHandle> {
    app_handle_cell().get()
}

// small objc helpers

/// Toll-free-bridged NSString, intentionally leaked (used as a constant).
pub(crate) unsafe fn ns_string(s: &str) -> *mut Object {
    let cf = CFString::new(s);
    let raw = cf.as_concrete_TypeRef() as *mut Object;
    std::mem::forget(cf);
    raw
}

unsafe fn ns_color(r: f64, g: f64, b: f64, a: f64) -> *mut Object {
    msg_send![class!(NSColor), colorWithCalibratedRed: r green: g blue: b alpha: a]
}

unsafe fn set_fill(color: *mut Object) {
    let _: () = msg_send![color, setFill];
}

unsafe fn fill_oval(rect: NSRect) {
    let path: *mut Object = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect];
    let _: () = msg_send![path, fill];
}


/// Stable per-project planet color: golden-angle hue scatter, pastel-leaning.
// color_for / card_color / hsv → colors.rs

// Town art / pack decode / being preview → art.rs


// screens

unsafe fn screens() -> Vec<(String, NSRect)> {
    let arr: *mut Object = msg_send![class!(NSScreen), screens];
    if arr.is_null() {
        return Vec::new();
    }
    let count: usize = msg_send![arr, count];
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let s: *mut Object = msg_send![arr, objectAtIndex: i];
        if s.is_null() {
            continue;
        }
        let frame: NSRect = msg_send![s, frame];
        let desc: *mut Object = msg_send![s, deviceDescription];
        let num: *mut Object = if desc.is_null() {
            std::ptr::null_mut()
        } else {
            msg_send![desc, objectForKey: ns_string("NSScreenNumber")]
        };
        let id = if num.is_null() {
            format!("screen-{:x}", s as usize)
        } else {
            let n: u32 = msg_send![num, unsignedIntValue];
            n.to_string()
        };
        out.push((id, frame));
    }
    out
}

/// visibleFrame excludes the menu bar / Dock — drift clamps to it so a
/// marble never wanders under OS chrome. (`screens()` keeps returning the
/// full frame; persisted coords round-trip through that.)
pub(crate) unsafe fn visible_frames() -> Vec<(String, NSRect)> {
    let arr: *mut Object = msg_send![class!(NSScreen), screens];
    if arr.is_null() {
        return Vec::new();
    }
    let count: usize = msg_send![arr, count];
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let s: *mut Object = msg_send![arr, objectAtIndex: i];
        if s.is_null() {
            continue;
        }
        let frame: NSRect = msg_send![s, visibleFrame];
        let desc: *mut Object = msg_send![s, deviceDescription];
        let num: *mut Object = if desc.is_null() {
            std::ptr::null_mut()
        } else {
            msg_send![desc, objectForKey: ns_string("NSScreenNumber")]
        };
        let id = if num.is_null() {
            format!("screen-{:x}", s as usize)
        } else {
            let n: u32 = msg_send![num, unsignedIntValue];
            n.to_string()
        };
        out.push((id, frame));
    }
    out
}

fn primary_screen_id() -> Option<String> {
    unsafe { screens().into_iter().next().map(|(id, _)| id) }
}

/// Stored coords are screen-local, flipped (y down) — clamp on-screen.
fn panel_frame(state: &WorldState, side: f64) -> NSRect {
    let all = unsafe { screens() };
    let fallback = NSRect::new(0.0, 0.0, 1440.0, 900.0);
    let (_, frame) = all
        .iter()
        .find(|(id, _)| id == &state.screen_id)
        .or_else(|| all.first())
        .map(|(id, f)| (id.clone(), *f))
        .unwrap_or_else(|| (String::new(), fallback));
    let lo = side / 2.0 + 8.0;
    let cx = state.x.clamp(lo, (frame.size.width - side / 2.0 - 8.0).max(lo));
    let cy = state.y.clamp(lo, (frame.size.height - side / 2.0 - 8.0).max(lo));
    NSRect::new(
        frame.origin.x + cx - side / 2.0,
        frame.origin.y + frame.size.height - cy - side / 2.0,
        side,
        side,
    )
}

// spawn ring (GO-ORBITTY-PLANET-SPAWN)
//
// New worlds appear on a ring around the rail, not mid-right of the
// display. Center = the `main` window's outer-frame center, converted into
// the same screen-local flipped (y-down) coords WorldState.x/y already
// use. Radius = half the rail's outer width + 80pt. First slot is
// 3 o'clock, then counterclockwise in 45° steps; if the rail sits on the
// right third of its screen (or 3 o'clock would clip the visibleFrame)
// the walk mirrors — 9 o'clock first, then clockwise — so the first
// visible slot is always inboard. A slot is skipped when the expanded
// disc (EXPANDED_RADIUS, not the marble) would clip that screen's
// visibleFrame. Existing worlds never move and do not occupy slots; only
// worlds already sitting on the ring (spawned here this session) count as
// claimed. Rail missing (headless test) → mid-right fallback, no panic.

const RING_SLOTS: usize = 8;
const RING_CLEARANCE: f64 = 80.0;

struct SpawnRing {
    screen_id: String,
    cx: f64,
    cy: f64,
    radius: f64,
    mirrored: bool,
    visible: [bool; RING_SLOTS],
}

fn ring_slot(ring: &SpawnRing, k: usize) -> (f64, f64) {
    let step = std::f64::consts::FRAC_PI_4 * (k as f64);
    // y is down, so visual up is negative y: both walks go up first.
    let (dx, dy) = if ring.mirrored {
        (-step.cos(), -step.sin()) // 9 o'clock, then clockwise
    } else {
        (step.cos(), -step.sin()) // 3 o'clock, then counterclockwise
    };
    (ring.cx + ring.radius * dx, ring.cy + ring.radius * dy)
}

fn spawn_ring() -> Option<SpawnRing> {
    let window = app_handle()?.get_webview_window("main")?;
    let scale = window.scale_factor().ok()?;
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    // Tauri reports physical pixels from the primary screen's top-left
    // (y down); AppKit frames are points from its bottom-left (y up).
    let w = size.width as f64 / scale;
    let h = size.height as f64 / scale;
    let cx_t = pos.x as f64 / scale + w / 2.0;
    let cy_t = pos.y as f64 / scale + h / 2.0;
    let all = unsafe { screens() };
    let primary_h = all.first().map(|(_, f)| f.size.height)?;
    let center = NSPoint {
        x: cx_t,
        y: primary_h - cy_t,
    };
    let (screen_id, sf) = all
        .iter()
        .find(|(_, f)| f.contains(center))
        .or_else(|| all.first())
        .map(|(id, f)| (id.clone(), *f))?;
    let cx = center.x - sf.origin.x;
    let cy = sf.size.height - (center.y - sf.origin.y);
    let visible = unsafe { visible_frames() };
    let vf = visible
        .iter()
        .find(|(id, _)| id == &screen_id)
        .map(|(_, f)| *f)
        .unwrap_or(sf);
    // visibleFrame as a y-down rect in screen-local coords.
    let vx0 = vf.origin.x - sf.origin.x;
    let vy0 = sf.size.height - (vf.origin.y + vf.size.height - sf.origin.y);
    let clips = |sx: f64, sy: f64| {
        sx - EXPANDED_RADIUS < vx0
            || sx + EXPANDED_RADIUS > vx0 + vf.size.width
            || sy - EXPANDED_RADIUS < vy0
            || sy + EXPANDED_RADIUS > vy0 + vf.size.height
    };
    let radius = w / 2.0 + RING_CLEARANCE;
    let mirrored = cx > sf.size.width * 2.0 / 3.0 || clips(cx + radius, cy);
    let mut ring = SpawnRing {
        screen_id,
        cx,
        cy,
        radius,
        mirrored,
        visible: [false; RING_SLOTS],
    };
    for k in 0..RING_SLOTS {
        let (sx, sy) = ring_slot(&ring, k);
        ring.visible[k] = !clips(sx, sy);
    }
    Some(ring)
}

/// Worlds already sitting on a visible ring slot — only those occupy
/// slots; pre-existing (mid-right or dragged) worlds are not on the ring.
fn ring_claimed(ring: &SpawnRing, worlds: &HashMap<String, WorldState>) -> usize {
    worlds
        .values()
        .filter(|w| {
            w.screen_id == ring.screen_id
                && (0..RING_SLOTS).any(|k| {
                    if !ring.visible[k] {
                        return false;
                    }
                    let (sx, sy) = ring_slot(ring, k);
                    (w.x - sx).abs() < 1.0 && (w.y - sy).abs() < 1.0
                })
        })
        .count()
}

/// New worlds land on the spawn ring around the rail; `existing_on_screen`
/// is the count of ring slots already claimed this session. Headless
/// fallback: mid-right, clearly on-screen, staggered per existing world.
fn default_state(screen_id: &str, existing_on_screen: usize) -> WorldState {
    let placed = spawn_ring().and_then(|ring| {
        let open: Vec<usize> = (0..RING_SLOTS).filter(|&k| ring.visible[k]).collect();
        if open.is_empty() {
            return None;
        }
        let lap = existing_on_screen / open.len();
        let (sx, sy) = ring_slot(&ring, open[existing_on_screen % open.len()]);
        // Past a full lap, ride the same slots a touch outward so a ninth
        // world never lands exactly on the first.
        let grow = lap as f64 * 96.0;
        let dx = (sx - ring.cx) / ring.radius;
        let dy = (sy - ring.cy) / ring.radius;
        Some((ring.screen_id.clone(), sx + dx * grow, sy + dy * grow))
    });
    let (screen_id, x, y) = match placed {
        Some(p) => p,
        None => {
            let all = unsafe { screens() };
            let size = all
                .iter()
                .find(|(id, _)| id == screen_id)
                .or_else(|| all.first())
                .map(|(_, f)| f.size)
                .unwrap_or(CGSize {
                    width: 1440.0,
                    height: 900.0,
                });
            (
                screen_id.to_string(),
                size.width * 0.72 + (existing_on_screen as f64) * 40.0,
                size.height * 0.45,
            )
        }
    };
    WorldState {
        screen_id,
        x,
        y,
        last_used: now_epoch(),
        radius: 0.0,
        plant: false,
        mushroom: false,
        boombox: false,
        music: String::new(),
        boombox_volume: 1.0,
        boombox_muted: false,
        projection: "oblique".into(),
        first_seen: now_epoch(),
        plants: Vec::new(),
        mushrooms: Vec::new(),
        buildings: Vec::new(),
        boombox_pos: None,
        beach_ball: false,
    }
}

fn now_epoch() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

// worlds.json

fn worlds_path() -> Option<std::path::PathBuf> {
    let app = app_handle()?;
    let dir = app.path().app_data_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("worlds.json"))
}

fn load_worlds(h: &mut Habitat) {
    if h.worlds_loaded {
        return;
    }
    h.worlds_loaded = true;
    if let Some(path) = worlds_path() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(file) = serde_json::from_str::<WorldsFile>(&text) {
                h.worlds = file.worlds;
            }
        }
    }
    // Pre-GROW saves: bool toggles → placements, first_seen stamped.
    let now = now_epoch();
    let mut dirty = false;
    for w in h.worlds.values_mut() {
        dirty |= migrate_world(w, now);
    }
    if dirty {
        save_worlds(h);
    }
}

fn save_worlds(h: &Habitat) {
    let Some(path) = worlds_path() else { return };
    let file = WorldsFile {
        worlds: h.worlds.clone(),
    };
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// Expanded radius: the world's scroll-zoomed value when set, else the
/// default. Marbles are always MARBLE_RADIUS regardless of zoom.
fn effective_radius(world: Option<&WorldState>, collapsed: bool) -> f64 {
    if collapsed {
        return MARBLE_RADIUS;
    }
    match world {
        Some(w) if w.radius > 0.0 => w.radius.clamp(MIN_RADIUS, max_radius()),
        _ => EXPANDED_RADIUS,
    }
}

// the world view class

/// None means the class couldn't be registered; callers skip planet
/// creation so a habitat failure leaves the rail usable.
fn world_view_class() -> Option<&'static Class> {
    static CLS: OnceLock<Option<&'static Class>> = OnceLock::new();
    CLS.get_or_init(|| {
        // add_method asserts on any mismatch — registration must never abort
        // the app, so it runs under catch_unwind.
        catch_unwind(AssertUnwindSafe(|| unsafe { register_world_view_class() }))
            .unwrap_or_else(|p| {
                eprintln!("habitat: world view class registration panicked: {}", panic_msg(&*p));
                None
            })
    })
    .as_ref()
    .copied()
}

unsafe fn register_world_view_class() -> Option<&'static Class> {
    // Already registered (unit tests, re-init) — reuse, never panic.
    if let Some(cls) = Class::get("LiveSpriteWorldView") {
        return Some(cls);
    }
    let mut decl = ClassDecl::new("LiveSpriteWorldView", class!(NSView))?;
    // drawRect:'s dirty-rect arg is part of the signature (objc's add_method
    // asserts selector/fn arg counts match); the body ignores it and always
    // redraws the whole view.
    decl.add_method(
        sel!(drawRect:),
        draw::draw_rect as extern "C" fn(&Object, Sel, NSRect),
    );
    decl.add_method(
        sel!(mouseDown:),
        input::mouse_down as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(mouseUp:),
        input::mouse_up as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(sel!(isFlipped), is_flipped as extern "C" fn(&Object, Sel) -> bool);
    decl.add_method(
        sel!(acceptsFirstMouse:),
        accepts_first_mouse as extern "C" fn(&Object, Sel, *mut Object) -> bool,
    );
    decl.add_method(
        sel!(scrollWheel:),
        input::scroll_wheel as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(rightMouseDown:),
        input::right_mouse_down as extern "C" fn(&Object, Sel, *mut Object),
    );
    Some(decl.register())
}

/// Target for the planet right-click menu. A single leaked instance lives
/// forever (NSMenuItem does not retain its target).
fn menu_target() -> Option<usize> {
    static TARGET: OnceLock<Option<usize>> = OnceLock::new();
    TARGET
        .get_or_init(|| {
            catch_unwind(AssertUnwindSafe(|| unsafe {
                let cls = register_menu_target_class()?;
                let obj: *mut Object = msg_send![cls, new];
                if obj.is_null() {
                    None
                } else {
                    Some(obj as usize)
                }
            }))
            .unwrap_or_else(|p| {
                eprintln!("habitat: menu target registration panicked: {}", panic_msg(&*p));
                None
            })
        })
        .as_ref()
        .copied()
}

unsafe fn register_menu_target_class() -> Option<&'static Class> {
    if let Some(cls) = Class::get("OrbittyPlanetMenuTarget") {
        return Some(cls);
    }
    let mut decl = ClassDecl::new("OrbittyPlanetMenuTarget", class!(NSObject))?;
    decl.add_method(
        sel!(planetMinimize:),
        input::menu_minimize as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetRestore:),
        input::menu_restore as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetClaimFront:),
        input::menu_claim_front as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetTogglePlant:),
        input::menu_toggle_plant as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetToggleMushroom:),
        input::menu_toggle_mushroom as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetToggleBoombox:),
        input::menu_toggle_boombox as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetChooseMusic:),
        audio::menu_choose_music as extern "C" fn(&Object, Sel, *mut Object),
    );
    // boombox sprite menu + volume steps.
    decl.add_method(
        sel!(planetBoomboxPlayPause:),
        input::menu_boombox_play_pause as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetRemoveBoombox:),
        input::menu_remove_boombox as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetBoomboxVolume:),
        input::menu_boombox_volume as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetBoomboxSlider:),
        input::menu_boombox_slider as extern "C" fn(&Object, Sel, *mut Object),
    );
    // per-planet beach-ball toggle.
    decl.add_method(
        sel!(planetToggleBeachBall:),
        input::menu_toggle_beach_ball as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(planetPlaceBuilding:),
        input::menu_place_building as extern "C" fn(&Object, Sel, *mut Object),
    );
    // walker right-click menu actions.
    decl.add_method(
        sel!(walkerUseImage:),
        input::walker_use_image as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerUseFolder:),
        input::walker_use_folder as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerSetPack:),
        input::walker_set_pack as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerOpenBeing:),
        input::walker_open_being as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerAssign:),
        input::walker_assign as extern "C" fn(&Object, Sel, *mut Object),
    );
    Some(decl.register())
}

fn panic_msg(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

use objc::runtime::Sel;

extern "C" fn is_flipped(_this: &Object, _cmd: Sel) -> bool {
    true
}

extern "C" fn accepts_first_mouse(_this: &Object, _cmd: Sel, _event: *mut Object) -> bool {
    true
}


// drawRect body lives in draw.rs (GO-ORBITTY-CLEAN-HABITAT-DRAW).
const CAPTION_TTL: Duration = Duration::from_secs(2);

/// Locate stroke lifetime (GO-ORBITTY-RAIL-ROW-MENU): rail row left-click
/// and walker left-click both glow ~500ms, then the ring draws out.
const LOCATE_TTL: Duration = Duration::from_millis(500);

/// Planet find glow (dest-menu click). Longer than walker locate so the
/// expanded name can flash and fade.
const PLANET_LOCATE_TTL: Duration = Duration::from_millis(1400);


// plant_cap / building_cap / layout_decor → layout.rs

/// Native caption pill on the planet for ~2s (titles only), drawn on the
/// next display pass.
pub(crate) fn set_caption(planet: &mut Planet, text: String) {
    planet.caption = Some((text, Instant::now()));
    unsafe {
        let view = planet.view as *mut Object;
        let _: () = msg_send![view, setNeedsDisplay: true];
    }
}

pub(crate) unsafe fn sender_project_id(sender: *mut Object) -> Option<String> {
    let obj: *mut Object = msg_send![sender, representedObject];
    if obj.is_null() {
        return None;
    }
    let utf8: *const std::os::raw::c_char = msg_send![obj, UTF8String];
    if utf8.is_null() {
        return None;
    }
    Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
}


/// World entry for menu/scroll mutations; created defensively if missing.
pub(crate) fn world_entry<'a>(h: &'a mut Habitat, project_id: &str) -> &'a mut WorldState {
    h.worlds
        .entry(project_id.to_string())
        .or_insert_with(|| default_state(&primary_screen_id().unwrap_or_default(), 0))
}


// panels

unsafe fn make_panel() -> *mut Object {
    let panel: *mut Object = msg_send![class!(NSPanel), alloc];
    // NSWindowStyleMaskBorderless (0) | NSWindowStyleMaskNonactivatingPanel (1<<7)
    let panel: *mut Object = msg_send![panel,
        initWithContentRect: NSRect::new(0.0, 0.0, 10.0, 10.0)
        styleMask: 128usize
        backing: 2usize // NSBackingStoreBuffered
        defer: false];
    // Peer surface at the normal window level: other apps can cover the
    // planet later; it neither floats forever nor hides under everything.
    let _: () = msg_send![panel, setLevel: 0i64];
    // canJoinAllSpaces (1) | stationary (1<<4) | fullScreenAuxiliary (1<<8)
    let _: () = msg_send![panel, setCollectionBehavior: 273usize];
    let _: () = msg_send![panel, setOpaque: false];
    let clear: *mut Object = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![panel, setBackgroundColor: clear];
    let _: () = msg_send![panel, setHasShadow: false]; // draw_world paints its own
    // Click-through by default; the cursor watcher flips this off only while
    // the pointer is over the planet disc or a sprite/dot.
    let _: () = msg_send![panel, setIgnoresMouseEvents: true];
    let _: () = msg_send![panel, setAcceptsMouseMovedEvents: true];
    let _: () = msg_send![panel, setReleasedWhenClosed: false];
    let _: () = msg_send![panel, setHidesOnDeactivate: false];
    // Show Desktop stay-visible: planets are not hidable app chrome.
    let _: () = msg_send![panel, setCanHide: false];
    let _: () = msg_send![panel, setBecomesKeyOnlyIfNeeded: true];
    panel
}

/// Walkers orbit with the planet's spin — this is the "walking the surface."
/// Dots sit on a static inner ring. View-local flipped coords.
// layout_beings / make_being → layout.rs

// sync (port of OverlayController.relayout)

/// True on the AppKit main thread. The Tauri command handler for
/// `sync_habitat` runs there, so this is the common path.
fn on_main_thread() -> bool {
    unsafe { msg_send![class!(NSThread), isMainThread] }
}

pub fn sync(app: &AppHandle, snapshot: HabitatSnapshot) -> Result<(), String> {
    // CADENCE-FIX: already on main → run inline. The old code queued the
    // closure with run_on_main_thread and then sat in recv_timeout(2s) —
    // on the main thread that is main waiting for a closure queued behind
    // itself, so every board drag beach-balled for the full 2s before the
    // sync landed.
    if on_main_thread() {
        return match catch_unwind(AssertUnwindSafe(|| do_sync(&snapshot))) {
            Ok(()) => Ok(()),
            Err(p) => {
                let msg = panic_msg(&*p);
                eprintln!("habitat: sync panicked (caught): {msg}");
                Err(format!("habitat sync failed: {msg}"))
            }
        };
    }
    let app = app.clone();
    // Off main (hypothetical IPC caller): keep the hop, but never block
    // main — recv_timeout on a worker thread is harmless.
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let result = catch_unwind(AssertUnwindSafe(|| do_sync(&snapshot)));
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(p)) => {
            let msg = panic_msg(&*p);
            eprintln!("habitat: sync panicked (caught): {msg}");
            Err(format!("habitat sync failed: {msg}"))
        }
        Err(_) => Err("habitat sync: no result from main thread".to_string()),
    }
}

fn do_sync(snapshot: &HabitatSnapshot) {
    // Pose transitions are collected under the lock and appended to the
    // event log AFTER the lock drops, on a background thread — jsonl IO
    // never runs on the AppKit thread inside the habitat lock (that was
    // the second half of the board-drag beach ball). A missed pose line
    // beats a stalled UI.
    let mut pose_events: Vec<(String, String, Pose, Pose)> = Vec::new();
    let mut h = lock();
    load_worlds(&mut h);
    // Re-arm the once-only Done fast-forward: the set tracks entries that
    // already emitted; an entry whose card leaves Done (uncheck, new open
    // card) drops out so its next Done emits again.
    {
        let done_ids: std::collections::HashSet<&str> = snapshot
            .planets
            .iter()
            .flat_map(|p| p.entries.iter())
            .filter(|e| pose_for(e) == Pose::WalkOff)
            .map(|e| e.id.as_str())
            .collect();
        h.walkoff_fast.retain(|id| done_ids.contains(id.as_str()));
    }
    let scale = if snapshot.sprite_scale <= 0.0 {
        1.0
    } else {
        snapshot.sprite_scale.clamp(0.5, 1.25)
    };
    h.sprite_scale = scale;
    // A drift toggle flip persists live marble x/y at once — the orbit
    // path never dirties, so a toggle would otherwise lose the drifted
    // positions until quit.
    if h.marble_drift != snapshot.marble_drift {
        h.worlds_dirty = true;
    }
    h.marble_drift = snapshot.marble_drift;
    h.planet_names = if snapshot.all_planets.is_empty() {
        snapshot
            .planets
            .iter()
            .map(|p| (p.project_id.clone(), p.name.clone()))
            .collect()
    } else {
        snapshot
            .all_planets
            .iter()
            .map(|p| (p.project_id.clone(), p.name.clone()))
            .collect()
    };
    for planet in h.planets.values_mut() {
        if (planet.sprite_scale - scale).abs() > 0.001 {
            planet.sprite_scale = scale;
            layout_beings(planet);
        }
    }
    // User packs: first sync and every import/remove reloads frame art.
    // A reload re-indexes pack_frames, so Being.pack indices built before
    // it are stale — the pose-only fast path below must not reuse them.
    let packs_reloaded = !h.packs_loaded;
    if !h.packs_loaded {
        h.packs_loaded = true;
        load_pack_frames(&mut h);
    }

    // Tear down panels for projects that left the habitat.
    let incoming: Vec<&str> = snapshot.planets.iter().map(|p| p.project_id.as_str()).collect();
    let stale: Vec<String> = h
        .planets
        .keys()
        .filter(|k| !incoming.contains(&k.as_str()))
        .cloned()
        .collect();
    for project_id in stale {
        if let Some(mut planet) = h.planets.remove(&project_id) {
            audio::stop_audio(&mut planet);
            unsafe {
                let panel = planet.panel as *mut Object;
                let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
                let _: () = msg_send![panel, release];
            }
        }
    }

    let Some(primary_id) = primary_screen_id() else {
        return;
    };

    // BUDGET cap 3 expanded: habitat:expand /
    // click-to-expand always succeeds — when a 4th world is expanded, the
    // least-recently focused of the rest takes the marble (stable snapshot
    // order breaks recency ties). The rail owns the collapsed flag: the
    // habitat:collapse echo makes it persist there, and this sync already
    // renders the victim as a marble so a 4th planet never shows. The
    // expand gesture itself is the focus — a marble→expanded transition
    // bumps the world's last_used ledger (drags and scroll-zooms already
    // stamp it), so recency survives restarts via worlds.json.
    let mut force_marble: std::collections::HashSet<String> = std::collections::HashSet::new();
    {
        let now = now_epoch();
        let mut expanded: Vec<(String, f64, usize)> = Vec::new();
        for (idx, p) in snapshot.planets.iter().enumerate() {
            if p.collapsed {
                continue;
            }
            let just_expanded = h
                .planets
                .get(&p.project_id)
                .is_some_and(|pl| pl.is_marble);
            let recency = if just_expanded || !h.worlds.contains_key(&p.project_id) {
                // A brand-new claim arrives focused; default_state stamps
                // it too when the world entry is created below.
                if let Some(w) = h.worlds.get_mut(&p.project_id) {
                    w.last_used = now;
                    h.worlds_dirty = true;
                }
                now
            } else {
                h.worlds
                    .get(&p.project_id)
                    .map(|w| w.last_used)
                    .unwrap_or(now)
            };
            expanded.push((p.project_id.clone(), recency, idx));
        }
        if expanded.len() > MAX_EXPANDED {
            expanded.sort_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.2.cmp(&b.2))
            });
            for (pid, _, _) in expanded.into_iter().skip(MAX_EXPANDED) {
                force_marble.insert(pid.clone());
                if let Some(app) = app_handle() {
                    let _ = app.emit_to("main", "habitat:collapse", pid);
                }
            }
        }
    }

    // BUDGET cap 12 marbles: at most 12 collapsed
    // NSPanels. Extra collapsed projects stay on the rail list with no
    // panel; a slot frees when one expands or is unclaimed (the stale
    // teardown above). The count uses the EFFECTIVE collapse state (post
    // cap-3), so a marble expanding this sync frees its slot for a waiting
    // project in the same pass.
    let eff_collapsed: HashMap<&str, bool> = snapshot
        .planets
        .iter()
        .map(|p| {
            (
                p.project_id.as_str(),
                p.collapsed || force_marble.contains(&p.project_id),
            )
        })
        .collect();
    let mut marble_panels = h
        .planets
        .keys()
        .filter(|pid| eff_collapsed.get(pid.as_str()).copied().unwrap_or(false))
        .count();
    if marble_panels > MAX_MARBLE_PANELS {
        // Pre-cap worlds overflow: tear down rail-last marble panels first;
        // a world cap-3 just collapsed keeps its panel while any other
        // marble exists (the user just watched it shrink — it must not
        // vanish). Torn-down projects stay on the rail list, no panel.
        let order: HashMap<&str, usize> = snapshot
            .planets
            .iter()
            .enumerate()
            .map(|(i, p)| (p.project_id.as_str(), i))
            .collect();
        let mut victims: Vec<String> = h
            .planets
            .keys()
            .filter(|pid| eff_collapsed.get(pid.as_str()).copied().unwrap_or(false))
            .cloned()
            .collect();
        victims.sort_by(|a, b| {
            force_marble
                .contains(a)
                .cmp(&force_marble.contains(b))
                .then_with(|| {
                    let ia = order.get(a.as_str()).copied().unwrap_or(usize::MAX);
                    let ib = order.get(b.as_str()).copied().unwrap_or(usize::MAX);
                    ib.cmp(&ia) // rail-last first
                })
        });
        for pid in victims.into_iter().take(marble_panels - MAX_MARBLE_PANELS) {
            if let Some(mut planet) = h.planets.remove(&pid) {
                audio::stop_audio(&mut planet);
                unsafe {
                    let panel = planet.panel as *mut Object;
                    let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
                    let _: () = msg_send![panel, release];
                }
            }
        }
        marble_panels = MAX_MARBLE_PANELS;
    }

    for HabitatPlanet {
        project_id,
        name,
        collapsed,
        entries,
        board_cards,
        todo_trees,
    } in &snapshot.planets
    {
        // Effective collapse: the snapshot's flag, or a cap-3 victim this
        // sync. Collapsed layout stays empty — marbles are bare.
        let collapsed_eff = *collapsed || force_marble.contains(project_id);
        let radius = effective_radius(h.worlds.get(project_id), collapsed_eff);
        let side = (radius + VIEW_PADDING) * 2.0;
        let color = color_for(project_id);

        // CADENCE-FIX v1.1: a CLAIMED window is a walker — open or
        // minimized no longer decides (both full-color on the green).
        // Only a CLOSED claim is asleep (dim + still, sleeper camp).
        // Dots are parked zombies only. Marbles stay bare until expanded.
        let mut sorted = entries.clone();
        sorted.sort_by(|a, b| {
            (a.app_name.to_lowercase() + &a.title.to_lowercase())
                .cmp(&(b.app_name.to_lowercase() + &b.title.to_lowercase()))
        });
        // Zombie aging (TS-7): 14+ days untouched parks the sprite down to
        // marble-scale occupancy; faded ones still walk but draw dimmed.
        // sleepers (closed claimed windows) always
        // walk — asleep and dim — and are never parked into dots or culled
        // for age `gone`; only Done/Unclaim evicts them.
        let (parked, walking): (Vec<&HabitatEntry>, Vec<&HabitatEntry>) = sorted
            .iter()
            .partition(|e| e.age == "park" && !e.asleep);
        let (walker_entries, dot_entries): (Vec<&HabitatEntry>, Vec<&HabitatEntry>) =
            if collapsed_eff {
                (Vec::new(), Vec::new())
            } else {
                (walking, parked)
            };

        // a Done entry that gets no walker to walk off
        // (marble planet, or a parked dot) evicts immediately — fast-
        // forward, never a phantom walk. Walkers holding a Done card walk
        // off visibly; anim_tick emits at the rim. CADENCE-FIX: the fast-
        // forward fires ONCE per Done spell (walkoff_fast), not on every
        // sync while the rail's unclaim round-trips.
        let walking_ids: std::collections::HashSet<&str> =
            walker_entries.iter().map(|e| e.id.as_str()).collect();
        for e in entries.iter() {
            if pose_for(e) == Pose::WalkOff
                && !walking_ids.contains(e.id.as_str())
                && h.walkoff_fast.insert(e.id.clone())
            {
                emit_walked_off(&e.id);
            }
        }

        // CADENCE-FIX pose-only fast path: same walker/dot set, same
        // being-defining fields — only the board card fields moved (a
        // board drag). Skip the Being rebuild entirely: no make_being, no
        // icon re-decode, no pack re-lookup; poses update in place below
        // and roam state (positions) stays.
        let pose_only = !packs_reloaded
            && h.planets.get(project_id).is_some_and(|planet| {
                planet.walker_entries.len() == walker_entries.len()
                    && planet.dot_entries.len() == dot_entries.len()
                    && planet
                        .walker_entries
                        .iter()
                        .zip(walker_entries.iter())
                        .all(|(a, b)| being_fields_eq(a, b))
                    && planet
                        .dot_entries
                        .iter()
                        .zip(dot_entries.iter())
                        .all(|(a, b)| being_fields_eq(a, b))
            });

        // Beings are built before borrowing the planet: make_being needs the
        // shared icon cache.
        let built = if pose_only {
            None
        } else {
            let walkers: Vec<Being> = walker_entries
                .iter()
                .map(|e| make_being(&mut h, e))
                .collect();
            let dots: Vec<Being> = dot_entries.iter().map(|e| make_being(&mut h, e)).collect();
            // Lazy packs (GO-ORBITTY-PLANET-BUDGET): decode a sprite pack only
            // when an EXPANDED planet wears it this sync. Marbles never load a
            // pack — their being lists are empty, so this never fires for them.
            if !collapsed_eff {
                for idx in walkers.iter().filter_map(|b| b.pack) {
                    ensure_pack_loaded(&mut h, idx);
                }
            }
            Some((walkers, dots))
        };

        let decor = h.worlds.get(project_id).cloned();
        if let Some(planet) = h.planets.get_mut(project_id) {
            planet.name = name.clone();
            // WORLD-TOWN: the board blit's ticks ride the snapshot; a
            // board drag changes them without touching any being field,
            // so this dirties the view on its own (pose-only path too).
            let board_changed = planet.board_cards != *board_cards;
            planet.board_cards = board_cards.clone();
            planet.tree_grow_days = snapshot.tree_grow_days;
            todo_tree::sync_chores(planet, project_id, todo_trees);
            planet.todo_trees = todo_trees.clone();
            todo_tree::layout(planet);
            let want_proj = locked_projection(collapsed_eff);
            let proj_changed = planet.projection != want_proj;
            planet.projection = want_proj;
            if let Some(w) = decor {
                // LEFTOVER hitch: layout_decor only when the décor itself
                // moved — a board drag / tick sync re-runs the frame math
                // for nothing otherwise.
                let decor_changed = planet.plants != w.plants
                    || planet.mushrooms != w.mushrooms
                    || planet.buildings != w.buildings
                    || planet.boombox_pos != w.boombox_pos
                    || planet.boombox != w.boombox
                    || planet.beach_ball != w.beach_ball;
                if decor_changed {
                    planet.plants = w.plants;
                    planet.mushrooms = w.mushrooms;
                    planet.buildings = w.buildings;
                    planet.boombox_pos = w.boombox_pos;
                    planet.boombox = w.boombox;
                    // the beach-ball toggle rides the
                    // same mirror; a sync that turns it off hides the live
                    // ball at once.
                    planet.beach_ball = w.beach_ball;
                    if !planet.beach_ball {
                        planet.ball = None;
                    }
                    layout_decor(planet);
                }
            }
            let geometry_changed = planet.radius != radius || planet.is_marble != collapsed_eff;
            if geometry_changed {
                // Marble ↔ planet swap: resize the panel around its center so
                // the world stays where the user put it (keeps stack order).
                unsafe {
                    let panel = planet.panel as *mut Object;
                    let view = planet.view as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    panel_set_frame(
                        panel,
                        NSRect::new(
                            frame.mid_x() - side / 2.0,
                            frame.mid_y() - side / 2.0,
                            side,
                            side,
                        ),
                        true,
                    );
                    let _: () = msg_send![view, setFrame: NSRect::new(0.0, 0.0, side, side)];
                }
                planet.radius = radius;
                planet.is_marble = collapsed_eff;
                // Marble ↔ planet swap drops any fake-Z orbit state; an
                // expanded planet never translates or scales.
                planet.orbit = None;
                planet.marble_scale = 1.0;
            }
            match built {
                // Pose-only fast path: update poses in place (sticky
                // WalkOff), keep every Being and all roam state.
                None => {
                    let mut any_pose = false;
                    for (b, e) in planet.walkers.iter_mut().zip(walker_entries.iter()) {
                        let next = sticky_pose(b.pose, pose_for(e));
                        let glance = droid_glance(e);
                        if next != b.pose {
                            pose_events.push((
                                project_id.clone(),
                                b.entry_id.clone(),
                                b.pose,
                                next,
                            ));
                            b.pose = next;
                            any_pose = true;
                        }
                        if glance != b.droid_glance {
                            b.droid_glance = glance;
                            any_pose = true;
                        }
                    }
                    planet.walker_entries =
                        walker_entries.iter().map(|e| (*e).clone()).collect();
                    planet.dot_entries = dot_entries.iter().map(|e| (*e).clone()).collect();
                    if geometry_changed || proj_changed {
                        layout_beings(planet);
                    }
                    if any_pose || geometry_changed || proj_changed || board_changed {
                        unsafe {
                            let view = planet.view as *mut Object;
                            let _: () = msg_send![view, setNeedsDisplay: true];
                        }
                    }
                }
                Some((mut walkers, dots)) => {
                    // one event per pose transition (first
                    // appearances stay silent — the claim event already exists, and
                    // a restart never replays a backlog into the log).
                    let prev_poses: HashMap<String, Pose> = planet
                        .walkers
                        .iter()
                        .map(|b| (b.entry_id.clone(), b.pose))
                        .collect();
                    // CADENCE-FIX: a walk-off in flight survives a blank
                    // card-status blip — never Roam mid-walk.
                    for b in walkers.iter_mut() {
                        if let Some(&prev) = prev_poses.get(&b.entry_id) {
                            b.pose = sticky_pose(prev, b.pose);
                        }
                    }
                    planet.color = color;
                    planet.walkers = walkers;
                    planet.dots = dots;
                    planet.walker_entries =
                        walker_entries.iter().map(|e| (*e).clone()).collect();
                    planet.dot_entries = dot_entries.iter().map(|e| (*e).clone()).collect();
                    layout_beings(planet);
                    for b in &planet.walkers {
                        if let Some(prev) = prev_poses.get(&b.entry_id) {
                            if *prev != b.pose {
                                pose_events.push((
                                    project_id.clone(),
                                    b.entry_id.clone(),
                                    *prev,
                                    b.pose,
                                ));
                            }
                        }
                    }
                    unsafe {
                        let view = planet.view as *mut Object;
                        let _: () = msg_send![view, setNeedsDisplay: true];
                    }
                }
            }
        } else {
            if collapsed_eff && marble_panels >= MAX_MARBLE_PANELS {
                // 13th+ marble (GO-ORBITTY-PLANET-BUDGET): the project
                // stays on the rail list with NO panel. A slot frees when
                // a marble expands or is unclaimed; that sync's sig change
                // materializes the waitlist in rail order.
                continue;
            }
            if !h.worlds.contains_key(project_id) {
                // Slot index = ring slots already claimed this session;
                // existing worlds are not on the ring and do not occupy.
                let on_screen = match spawn_ring() {
                    Some(ring) => ring_claimed(&ring, &h.worlds),
                    None => h
                        .worlds
                        .values()
                        .filter(|w| w.screen_id == primary_id)
                        .count(),
                };
                h.worlds
                    .insert(project_id.clone(), default_state(&primary_id, on_screen));
            }
            let state = h.worlds[project_id].clone();
            // A planet that didn't exist above can't have taken the
            // pose-only fast path (its gate requires the planet), so the
            // beings are always built here.
            let Some((walkers, dots)) = built else {
                continue;
            };
            let mut planet = Planet {
                panel: 0,
                view: 0,
                name: name.clone(),
                radius,
                sprite_scale: h.sprite_scale,
                is_marble: collapsed_eff,
                color,
                rotation: 0.0,
                projection: locked_projection(collapsed_eff),
                walkers,
                dots,
                caption: None,
                hover_name: false,
                orbit: None,
                marble_scale: 1.0,
                pin_until: None,
                plants: state.plants.clone(),
                mushrooms: state.mushrooms.clone(),
                buildings: state.buildings.clone(),
                boombox_pos: state.boombox_pos,
                boombox: state.boombox,
                beach_ball: state.beach_ball,
                ball: None,
                audio: None,
                audio_volume: 1.0,
                decor_frames: Vec::new(),
                roam: HashMap::new(),
                walker_entries: walker_entries.iter().map(|e| (*e).clone()).collect(),
                dot_entries: dot_entries.iter().map(|e| (*e).clone()).collect(),
                board_cards: board_cards.clone(),
                todo_trees: todo_trees.clone(),
                todo_tree_frames: Vec::new(),
                tree_grow_days: snapshot.tree_grow_days,
                tree_chores: Vec::new(),
            };
            layout_beings(&mut planet);
            todo_tree::layout(&mut planet);
            let Some(cls) = world_view_class() else {
                // Class registration failed — skip this planet and leave the
                // rail (list/focus/stow) fully usable.
                eprintln!("habitat: no world view class; skipping planet {project_id}");
                continue;
            };
            unsafe {
                let view: *mut Object = msg_send![cls, alloc];
                let view: *mut Object =
                    msg_send![view, initWithFrame: NSRect::new(0.0, 0.0, side, side)];
                let panel = make_panel();
                let _: () = msg_send![panel, setContentView: view];
                let _: () = msg_send![view, release]; // panel owns it now
                // Borderless panels never draw a title, but the name shows up
                // in window lists / screen-recording prompts.
                let _: () = msg_send![panel, setTitle: ns_string(&format!("Orbitty — {name}"))];
                panel_set_frame(panel, panel_frame(&state, side), false);
                if h.planets_hidden {
                    let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
                } else {
                    let _: () = msg_send![panel, orderFrontRegardless];
                }
                planet.panel = panel as usize;
                planet.view = view as usize;
            }
            // Boombox persisted on: start playing once the planet exists.
            // A hard-muted save starts nothing — the cursor_tick fader
            // door brings it up on unmute.
            if state.boombox && !state.music.is_empty() {
                let live = if state.boombox_muted {
                    0.0
                } else {
                    audio::live_volume(state.boombox_volume, planet.radius, h.reduce_motion)
                };
                audio::start_audio(&mut planet, &state.music, live);
            }
            h.planets.insert(project_id.clone(), planet);
            // a fresh panel must not
            // debut behind the user's windows — orderFrontRegardless ran
            // above; arm the locate glow so the first blits ring the disc
            // (project_add arms pre-sync too; this is the guarantee).
            h.locate_planet = Some((project_id.clone(), Instant::now()));
            if collapsed_eff {
                marble_panels += 1;
            }
        }
        // heal worlds.json so a leftover "flat"
        // expanded save does not linger. Live disc never reads this.
        if let Some(w) = h.worlds.get_mut(project_id) {
            let want = locked_projection_str(collapsed_eff);
            if w.projection != want {
                w.projection = want.to_string();
                h.worlds_dirty = true;
            }
        }
    }
    // walked-off re-delivery + Reduce Motion
    // fast-forward. anim_tick emits when a Done walker crosses the hide
    // line; this pass re-emits on every sync while one lingers hidden (the
    // rail's handler is idempotent, so a missed first emit heals here).
    // Under Reduce Motion there is no walk to narrate — Done walkers snap
    // past the hide line at once instead of freezing mid-disc.
    let reduce_motion = h.reduce_motion;
    for planet in h.planets.values_mut() {
        if planet.is_marble {
            continue;
        }
        // Under Reduce Motion anim_tick never steps, so a posed walker
        // would freeze wherever it was — snap every station-posed walker
        // to its station here instead. The world stays truthful, just
        // still: positions are the board, motion is what's reduced.
        if reduce_motion {
            let posed: Vec<(String, Pose)> = planet
                .walkers
                .iter()
                .filter(|b| !b.asleep || b.pose == Pose::WalkOff)
                .map(|b| (b.entry_id.clone(), b.pose))
                .collect();
            let mut snapped = false;
            for (id, pose) in posed {
                let Some((tx, ty)) = (match pose {
                    // PROPS leftover (town usable): Stand snaps to the
                    // board's south lip too — never the post foot
                    // (STATION_BOARD sits behind the sign PNG).
                    Pose::Stand => Some(draw::board_near_lip(
                        planet.radius,
                        planet.projection,
                        planet.marble_scale,
                    )),
                    Pose::WalkOff | Pose::Work | Pose::Sit => {
                        Some(station_target(pose, &id))
                    }
                    Pose::Roam => None,
                }) else {
                    continue;
                };
                if let Some(r) = planet.roam.get_mut(&id) {
                    if (r.x - tx).abs() > 1e-9 || (r.y - ty).abs() > 1e-9 {
                        r.x = tx;
                        r.y = ty;
                        r.tx = tx;
                        r.ty = ty;
                        r.pause_end = None;
                        snapped = true;
                    }
                }
            }
            if snapped {
                layout_beings(planet);
                unsafe {
                    let view = planet.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
        }
        let done_ids: Vec<String> = planet
            .walkers
            .iter()
            .filter(|b| b.pose == Pose::WalkOff)
            .map(|b| b.entry_id.clone())
            .collect();
        for id in done_ids {
            if planet.roam.get(&id).is_some_and(|r| r.x < HOME_HIDE_X) {
                emit_walked_off(&id);
            }
        }
    }
    // Lazy packs (GO-ORBITTY-PLANET-BUDGET): a pack whose last expanded
    // wearer collapsed (or left) drops its decoded bitmaps; the art paths
    // stay so a re-expand re-decodes.
    unload_dead_packs(&mut h);
    // The habitat lock drops BEFORE the event-log appends — jsonl IO runs
    // on a background thread, never on the AppKit thread under the lock.
    drop(h);
    flush_pose_events(pose_events);
}

/// CADENCE-FIX: one event per pose transition, appended off the AppKit
/// thread and off the habitat lock. Best-effort: a missed pose line beats
/// a stalled UI.
fn flush_pose_events(events: Vec<(String, String, Pose, Pose)>) {
    if events.is_empty() {
        return;
    }
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    std::thread::spawn(move || {
        for (project_id, entry_id, from, to) in events {
            crate::event_log::append_best(
                &app,
                "walker-pose",
                None,
                Some(&project_id),
                Some("habitat"),
                true,
                &serde_json::json!({
                    "entryId": entry_id,
                    "from": pose_name(from),
                    "to": pose_name(to),
                }),
            );
        }
    });
}

/// Being-defining fields: everything except the board card fields
/// (card_status / card_kind / needed_state — those only move the pose)
/// and `minimized`, which CADENCE-FIX made pose-irrelevant (a claimed
/// window is a walker either way). The pose-only fast path compares these
/// to skip a full Being rebuild on board drags.
fn being_fields_eq(a: &HabitatEntry, b: &HabitatEntry) -> bool {
    a.id == b.id
        && a.app_name == b.app_name
        && a.title == b.title
        && a.bundle_id == b.bundle_id
        && a.icon_base64 == b.icon_base64
        && a.age == b.age
        && a.sprite_pack_id == b.sprite_pack_id
        && a.char_seed == b.char_seed
        && a.char_look == b.char_look
        && a.sprite_scale == b.sprite_scale
        && a.asleep == b.asleep
        && a.agent_seed == b.agent_seed
}

unsafe fn panel_set_frame(panel: *mut Object, frame: NSRect, display: bool) {
    let _: () = msg_send![panel, setFrame: frame display: display];
}

/// the rail's `habitat:locate` — stroke the
/// walker's frame for ~500ms. Glow only: no focus, no raise, no wake.
pub fn locate_walker(app: AppHandle, entry_id: String) -> Result<(), String> {
    let app = app.clone();
    app.run_on_main_thread(move || {
        let mut h = lock();
        h.locate = Some((entry_id.clone(), Instant::now()));
        for p in h.planets.values() {
            if p.walkers.iter().any(|b| b.entry_id == entry_id) {
                unsafe {
                    let view = p.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Dest-menu find: glow that planet ~1.4s. Raises nothing — caller may
/// also `bring_planet_front`. Marble = diagonal ring; expanded = rim + name.
pub fn locate_planet(app: AppHandle, project_id: String) -> Result<(), String> {
    let app = app.clone();
    app.run_on_main_thread(move || {
        let mut h = lock();
        h.locate_planet = Some((project_id.clone(), Instant::now()));
        if let Some(p) = h.planets.get(&project_id) {
            unsafe {
                let view = p.view as *mut Object;
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

// rail Home button

/// Order every planet panel front. Peer surfaces keep their relative order;
/// this just lifts the whole set above whatever covered it.
pub fn bring_all_front() {
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    let _ = app.run_on_main_thread(|| {
        let h = lock();
        if h.planets_hidden {
            return;
        }
        for planet in h.planets.values() {
            unsafe {
                let panel = planet.panel as *mut Object;
                let _: () = msg_send![panel, orderFrontRegardless];
            }
        }
    });
}

/// Hide or show every planet disc. Hidden discs orderOut; shown lift front.
pub fn set_planets_hidden(hidden: bool) {
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    let _ = app.run_on_main_thread(move || {
        let mut h = lock();
        h.planets_hidden = hidden;
        for planet in h.planets.values() {
            unsafe {
                let panel = planet.panel as *mut Object;
                if hidden {
                    let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
                } else {
                    let _: () = msg_send![panel, orderFrontRegardless];
                }
            }
        }
    });
}

/// Order one planet's panel front by project id — that disc only. An
/// unknown or panel-less id is a no-op, never a bring_all_front fallback
/// (the user picked one name; lifting every disc would lie).
pub fn bring_planet_front(project_id: &str) {
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    let id = project_id.to_string();
    let _ = app.run_on_main_thread(move || {
        let h = lock();
        if h.planets_hidden {
            return;
        }
        if let Some(planet) = h.planets.get(&id) {
            unsafe {
                let panel = planet.panel as *mut Object;
                let _: () = msg_send![panel, orderFrontRegardless];
            }
        }
    });
}

// yard meow gate (GO-ORBITTY-HERDING-CATS)

/// Is any planet's boombox actually playing right now? The yard's meow
/// loop skips while this is true — voices never stack. Read-only; called
/// from the yard's main-thread hop (lock order yard → habitat, never the
/// reverse, so no deadlock).
pub fn boombox_playing() -> bool {
    let h = lock();
    for p in h.planets.values() {
        let Some(ptr) = p.audio else {
            continue;
        };
        unsafe {
            let player = ptr as *mut Object;
            if player.is_null() {
                continue;
            }
            let playing: bool = msg_send![player, isPlaying];
            if playing {
                return true;
            }
        }
    }
    false
}

// tray escape hatch

/// Bring back every minimized window that has a home (Swift restoreAll).
/// Parked zombies sit in `dots` (marble-scale occupancy) but are still real
/// minimized windows, so they come back too; deminiaturize is a no-op on
/// already-open dot windows. Sleepers (closed windows) are skipped — their
/// door is wake, not deminiaturize.
pub fn restore_all() {
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    let _ = app.run_on_main_thread(|| {
        let ids: Vec<String> = {
            let h = lock();
            h.planets
                .values()
                .flat_map(|p| {
                    p.walkers
                        .iter()
                        .chain(p.dots.iter())
                        // Sleepers are closed windows — deminiaturize on a
                        // dead id can only fail; wake is their door.
                        .filter(|b| !b.asleep)
                        .map(|b| b.entry_id.clone())
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        for id in ids {
            let _ = crate::windows::deminiaturize_window(&id);
        }
    });
}
