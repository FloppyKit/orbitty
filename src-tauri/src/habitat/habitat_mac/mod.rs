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
use objc::runtime::Object;
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
mod drag_tick;
mod cursor_tick;
mod input;
mod mouse;
mod mouse_up;
mod walker_menu;
mod avatar_popup;
mod planet_menu;
mod board_geom;
mod decor_art;
mod draw;
mod boombox_hud;
mod boombox_monitors;
mod droid;
mod droid_pack;
mod todo_tree;
mod planet_idle;
mod geom;
mod world;
mod pose;
mod ball;
mod ball_tick;
mod view_class;
mod drift;
mod roam;
mod decor;
mod colors;
mod art;
mod marble_motion;
mod marble_tracks;
mod anim;
mod anim_planet;
mod anim_step;
mod anim_walkoff;
mod layout;
mod spawn;
mod panels;
mod sync;
mod sync_budget;
mod sync_pose;
mod sync_still;
mod sync_roster;
mod sync_spawn;
mod types;
mod worlds_io;
mod api;

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
    SLEEPER_NORTH, SLEEPER_SOUTH, SLEEPER_WEST, STATION_BOARD, STATION_COMSTACK, STATION_FIELD,
    STATION_HOUSE, STATION_RIM, STATION_SUNDIAL, YARD_ARRIVE, YARD_DWELL_MAX_MS, YARD_DWELL_MIN_MS,
    YARD_EAST, YARD_INSET,
    YARD_NORTH, YARD_SAIL, YARD_SOUTH, YARD_WEST,
};
pub(crate) use ball::{
    ball_arc_heads, ball_hop_secs, ball_pos, pack_still_aspect, walker_head_view, BallState,
    BALL_ARC_HEADS_MAX, BALL_ARC_HEADS_MIN,
};
pub(crate) use view_class::{menu_target, world_view_class};

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
                .spawn(move || {
                    crate::agent_runs::evict_walked_off(&app, &card_id);
                    crate::shift_continue::on_agent_walked_off(&app, &card_id);
                });
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


// DecorKind / DecorRef → decor.rs

// Per-walker RoamState → roam.rs


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



/// xorshift64 for roam pauses (no rand crate on the tree); global so the
/// anim loop can draw numbers while planets are mutably borrowed.
pub(crate) use types::{Being, Habitat, PackFrames, Planet};

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




// world view + menu target class → view_class.rs


fn panic_msg(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
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


pub(crate) use worlds_io::{load_worlds, save_worlds};

pub use api::{
    boombox_playing, bring_all_front, bring_planet_front, locate_planet, locate_walker,
    restore_all, set_planets_hidden, sync,
};
