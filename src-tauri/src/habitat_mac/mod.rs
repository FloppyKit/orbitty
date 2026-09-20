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
mod walker_menu;
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
mod spawn;
mod panels;
mod sync;

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
pub(crate) use spawn::{default_state, ring_claimed, spawn_ring};
pub(crate) use panels::{effective_radius, make_panel, panel_frame, panel_set_frame, primary_screen_id, screens, visible_frames};


pub use tick::{persist_now, start};

// pixel sprite art
// walkers are generated people (chars.rs 12×12 glyph
// grids, composited body/clothes/eyes/mouth/hat + palette) drawn per frame
// below. The old blob/cat/robot templates are retired — an empty pack id
// means "generated being", never a blank.

pub(crate) const DOT_SIZE: f64 = 12.0;
pub(crate) const VIEW_PADDING: f64 = 30.0;
pub(crate) const EXPANDED_RADIUS: f64 = 72.0;
const MARBLE_RADIUS: f64 = 18.0;
/// BUDGET desk budget: at most 3 expanded planets (a 4th
/// expand collapses the least-recently focused of the rest) and at most 12
/// collapsed marble panels (13th+ projects stay on the rail list, no panel).
pub(crate) const MAX_EXPANDED: usize = 3;
pub(crate) const MAX_MARBLE_PANELS: usize = 12;
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

pub(crate) fn app_handle() -> Option<&'static AppHandle> {
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





// spawn ring (GO-ORBITTY-PLANET-SPAWN)
// SpawnRing / spawn_ring / default_state → spawn.rs
// screens / panel_frame / make_panel / panel_set_frame → panels.rs

pub(crate) fn now_epoch() -> f64 {
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

pub(crate) fn load_worlds(h: &mut Habitat) {
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


// the world view class

/// None means the class couldn't be registered; callers skip planet
/// creation so a habitat failure leaves the rail usable.
pub(crate) fn world_view_class() -> Option<&'static Class> {
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
        walker_menu::walker_use_image as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerUseFolder:),
        walker_menu::walker_use_folder as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerSetPack:),
        walker_menu::walker_set_pack as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerOpenBeing:),
        walker_menu::walker_open_being as extern "C" fn(&Object, Sel, *mut Object),
    );
    decl.add_method(
        sel!(walkerAssign:),
        walker_menu::walker_assign as extern "C" fn(&Object, Sel, *mut Object),
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
        return match catch_unwind(AssertUnwindSafe(|| sync::do_sync(&snapshot))) {
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
        let result = catch_unwind(AssertUnwindSafe(|| sync::do_sync(&snapshot)));
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

// do_sync / flush_pose_events / being_fields_eq → sync.rs (DIET-HABITAT)

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
