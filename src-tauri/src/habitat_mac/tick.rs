//! DisplayLink vsync clock + 16ms habitat tick (drag / anim / cursor).
//! Motion posts to the main queue and returns — never waits on UI.
//! GAIN slop in drag_tick is moved as-is. boombox_click lives in input.rs.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use super::audio;
use super::boombox_hud;
use super::input::{boombox_click, contains_interactive, over_disc, BOOMBOX_CLICK_SLOP};
use super::{
    app_handle, app_handle_cell, ball_arc_heads, ball_hop_secs, default_state, emit_board_checkin, emit_walked_off, house_spawn, layout_beings, walkoff_target,
    layout_decor, lock, now_epoch, pack_still_aspect, panel_frame, panel_set_frame, panic_msg, save_worlds, screens,
    set_caption, sleeper_target, station_target, view_to_disc, visible_frames, walker_head_view, yard_clamp, yard_contains,
    droid_pack,
    yard_dwell_ms, yard_pick, yard_pick_sailing, yard_seed, BallState, ClusterState,
    DecorKind, DecorPoint, Drag, Habitat, MarbleOrbit, NSPoint, NSRect, Pose, RoamState,
    BARY_PREFIX, BARY_R_MAX, BARY_R_MIN, CAPTION_TTL, DRIFT_JOIN_DIST, HOME_HIDE_X,
    DRIFT_LEAVE_DIST, DRIFT_PIN_SECS, FRONT_DEADBAND, HOST_JOIN_DIST, HOST_LEAVE_DIST,
    HOST_PREFIX, HOST_SWITCH_CLOSER, JOIN_EASE_SECS, LOCATE_TTL, MARBLE_RADIUS, OVAL_RATIO,
    PLANET_LOCATE_TTL,
    TRACK_T_MAX, TRACK_T_MIN, VIEW_PADDING, YARD_ARRIVE, YARD_SAIL,
};

// CVDisplayLink posts drift_pass to main and returns. Coalesce to one hop.
// If the link fails, the 16ms thread drives drift_pass every tick.

type CVDisplayLinkRef = *mut c_void;

#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
    fn CVDisplayLinkCreateWithActiveCGDisplays(link_out: *mut CVDisplayLinkRef) -> i32;
    fn CVDisplayLinkSetOutputCallback(
        link: CVDisplayLinkRef,
        callback: CVDisplayLinkOutputCallback,
        user_info: *mut c_void,
    ) -> i32;
    fn CVDisplayLinkStart(link: CVDisplayLinkRef) -> i32;
    fn CVDisplayLinkStop(link: CVDisplayLinkRef) -> i32;
    fn CVDisplayLinkRelease(link: CVDisplayLinkRef);
}

type CVDisplayLinkOutputCallback = unsafe extern "C" fn(
    CVDisplayLinkRef,
    *const c_void, // inNow (CVTimeStamp — unused)
    *const c_void, // inOutputTime (unused)
    u64,           // CVOptionFlags in
    *mut u64,      // CVOptionFlags out
    *mut c_void,   // context
) -> i32;

/// Raw pointer wrapper so the link can live in a static Mutex.
struct DisplayLink(CVDisplayLinkRef);
unsafe impl Send for DisplayLink {}

fn display_link_cell() -> &'static Mutex<Option<DisplayLink>> {
    static DL: OnceLock<Mutex<Option<DisplayLink>>> = OnceLock::new();
    DL.get_or_init(|| Mutex::new(None))
}

/// True while the vsync clock owns motion — the 16ms thread must then
/// never also call drift_pass (two motion clocks at once = jitter).
static DISPLAY_LINK_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Per-callback counter so the new-cluster discovery cadence (n % 3)
/// works exactly like the 16ms path's.
static DISPLAY_LINK_TICK: AtomicU64 = AtomicU64::new(0);
/// Coalesce (GO-ORBITTY-MARBLE-HITCH): at most one motion hop
/// queued-or-running on main. A vsync callback that fires while a hop is
/// still waiting drops the refresh instead of piling up a burst of late
/// setFrameOrigins after a stall.
static MOTION_HOP_PENDING: AtomicBool = AtomicBool::new(false);
/// Coalesce for the 16ms habitat tick hop (GO-ORBITTY-MARBLE-YIELD),
/// INDEPENDENT of MOTION_HOP_PENDING: a drag/anim/cursor turn in flight
/// never gates a vsync motion hop, and a queued motion hop never gates
/// the next tick. If a turn must drop it is a tick turn (cursor/anim),
/// never drift_pass / drift_motion.
static TICK_HOP_PENDING: AtomicBool = AtomicBool::new(false);

fn display_link_active() -> bool {
    DISPLAY_LINK_ACTIVE.load(Ordering::SeqCst)
}

// GCD (libdispatch, exported through libSystem): the non-waiting hop.
// dispatch_async_f queues work on the main queue and returns at once —
// the DisplayLink thread never sits waiting for setFrameOrigin. The
// SDK's dispatch_get_main_queue() is a header inline (no symbol) that
// returns &_dispatch_main_q, so bind the global directly.
extern "C" {
    #[link_name = "_dispatch_main_q"]
    static DISPATCH_MAIN_Q: c_void;
    fn dispatch_async_f(
        queue: *mut c_void,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
}

/// Runs ON the main queue, posted by display_link_output. The coalesce
/// flag clears only after the pass, so callbacks landing while main is
/// busy drop instead of bursting. Once the link is stopped (quit) a hop
/// still sitting in the queue skips — it must not touch panels during
/// teardown.
extern "C" fn motion_hop_main(context: *mut c_void) {
    let n = context as usize as u64;
    if display_link_active() {
        if let Err(p) = catch_unwind(AssertUnwindSafe(|| drift_motion(n))) {
            eprintln!(
                "habitat: displaylink motion panicked (caught): {}",
                panic_msg(&*p)
            );
        }
    }
    MOTION_HOP_PENDING.store(false, Ordering::SeqCst);
}

/// Runs ON the main queue, posted by the 16ms habitat thread
/// (GO-ORBITTY-MARBLE-YIELD post-and-return: the worker never waits on
/// main, so a busy cursor/anim turn can't stall the clock that feeds
/// drag). The coalesce flag clears only after the turn; turns landing
/// while main is busy drop — cursor/anim may drop, marble motion rides
/// the separate MOTION_HOP_PENDING hop and is never dropped by this.
extern "C" fn tick_hop_main(context: *mut c_void) {
    let n = context as usize as u64;
    if let Err(p) = catch_unwind(AssertUnwindSafe(|| tick(n))) {
        eprintln!("habitat: tick panicked (caught): {}", panic_msg(&*p));
    }
    TICK_HOP_PENDING.store(false, Ordering::SeqCst);
}

unsafe extern "C" fn display_link_output(
    _link: CVDisplayLinkRef,
    _in_now: *const c_void,
    _in_output_time: *const c_void,
    _flags_in: u64,
    _flags_out: *mut u64,
    _user_info: *mut c_void,
) -> i32 {
    // Choose-music panel or a planet/walker NSMenu: motion paused —
    // setFrameOrigin during a nested tracking loop beach-balls the menu
    // (FRONT-HOLD leftover; same guard as MUSIC_PICKER_OPEN).
    if audio::MUSIC_PICKER_OPEN.load(Ordering::SeqCst)
        || crate::companion_clear::menu_quiet()
        || lock().planets_hidden
    {
        return 0;
    }
    let n = DISPLAY_LINK_TICK.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
    // Post-and-return: never block the vsync thread on the UI thread.
    // Coalesce: a hop already queued or running → drop this refresh.
    if MOTION_HOP_PENDING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return 0;
    }
    dispatch_async_f(
        &DISPATCH_MAIN_Q as *const c_void as *mut c_void,
        n as usize as *mut c_void,
        motion_hop_main,
    );
    0
}

/// Create + start the vsync clock once at habitat start. Any failure
/// leaves DISPLAY_LINK_ACTIVE false, so drift_tick falls back to driving
/// drift_pass on every 16ms tick — never skipping a move.
fn display_link_start() {
    unsafe {
        let mut link: CVDisplayLinkRef = std::ptr::null_mut();
        if CVDisplayLinkCreateWithActiveCGDisplays(&mut link) != 0 || link.is_null() {
            eprintln!("habitat: CVDisplayLink create failed — 16ms thread drives motion, never skips");
            return;
        }
        if CVDisplayLinkSetOutputCallback(link, display_link_output, std::ptr::null_mut()) != 0 {
            eprintln!("habitat: CVDisplayLink callback failed — 16ms thread drives motion, never skips");
            CVDisplayLinkRelease(link);
            return;
        }
        if CVDisplayLinkStart(link) != 0 {
            eprintln!("habitat: CVDisplayLink start failed — 16ms thread drives motion, never skips");
            CVDisplayLinkRelease(link);
            return;
        }
        *display_link_cell().lock().unwrap_or_else(|e| e.into_inner()) = Some(DisplayLink(link));
        DISPLAY_LINK_ACTIVE.store(true, Ordering::SeqCst);
    }
}

/// Stop + release on habitat stop (app quit). The callback never blocks
/// on the main thread, so Stop can't deadlock against the quit path; a
/// motion hop still queued at this point skips via display_link_active().
fn display_link_stop() {
    let link = display_link_cell()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    DISPLAY_LINK_ACTIVE.store(false, Ordering::SeqCst);
    if let Some(DisplayLink(link)) = link {
        unsafe {
            CVDisplayLinkStop(link);
            CVDisplayLinkRelease(link);
        }
    }
}

// tick thread: drag 60Hz · animation 20Hz · cursor 10Hz
// (Motion rides CVDisplayLink when it's up — see drift_tick.)
//
// the worker POSTS each tick to the main queue
// (dispatch_async_f — the same hop the DisplayLink clock uses) and
// returns at once; the blocking run_on_main_thread here WAS the
// remaining 1-2-3-4-pause (a busy main stalled the 16ms clock, then the
// coalesce dropped queued vsync hops). TICK_HOP_PENDING coalesces tick
// turns independently of MOTION_HOP_PENDING, so a tick in flight never
// drops a vsync motion hop — at worst the motion hop runs right after a
// (short) tick turn, and cursor_tick yields to it (see tick).

pub fn start(app: AppHandle) {
    let cell = app_handle_cell();
    if let Ok(st) = crate::state::load_state(&app) {
        lock().planets_hidden = st.companion.planets_hidden;
    }
    let _ = cell.set(app);
    display_link_start();
    std::thread::spawn(|| {
        let mut n = 0u64;
        loop {
            let active = {
                let h = lock();
                !h.planets.is_empty() || h.drag.is_some()
            };
            if active {
                // Post-and-return. Coalesce: a tick turn still queued or
                // running → drop THIS turn (drag/anim/cursor skip a beat)
                // instead of piling up late turns after a stall.
                if TICK_HOP_PENDING
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    // SAFETY: same hop as display_link_output — main queue
                    // global, plain-old-data context, `extern "C"` work fn.
                    unsafe {
                        dispatch_async_f(
                            &DISPATCH_MAIN_Q as *const c_void as *mut c_void,
                            n as usize as *mut c_void,
                            tick_hop_main,
                        );
                    }
                }
                n = n.wrapping_add(1);
                std::thread::sleep(Duration::from_millis(16));
            } else {
                std::thread::sleep(Duration::from_millis(300));
            }
        }
    });
}

fn tick(n: u64) {
    // Hidden discs: do not AX-hit-test or dirty views on main while
    // WKWebView is loading custom schemes — that combo beach-balls.
    if lock().planets_hidden {
        return;
    }
    drag_tick();
    if n % 3 == 0 {
        anim_tick();
    }
    drift_tick(n);
    super::planet_idle::maybe_pass();
    if n % 6 == 0 {
        // a queued marble motion hop outranks
        // cursor work. Skip this cursor turn (hit-test/save ride the next
        // one) so the vsync hop runs now — drop cursor, never drift_pass
        // / drift_motion.
        if !MOTION_HOP_PENDING.load(Ordering::SeqCst) {
            cursor_tick();
        }
    }
}

/// RIM hitch law: a move is NEVER skipped (the old
/// n % 6 == 0 hole was the 1-2-3-pause Matt saw on glass). CVDisplayLink
/// is the motion clock when it's up — the 16ms thread then keeps
/// drag/anim/cursor only and must not also call drift_pass (two motion
/// clocks at once = jitter). Fallback: this thread drives drift_pass
/// every tick, still never skipping.
fn drift_tick(n: u64) {
    if display_link_active() {
        return; // CVDisplayLink owns motion.
    }
    drift_motion(n);
}

/// One motion turn: advance the shared drift clock and run the
/// setFrameOrigin sweep. Called on the main queue from the CVDisplayLink
/// callback's posted hop or, when the link is down, from every 16ms tick.
/// The clock
/// advances EVERY turn while motion is on, and drift_pass runs every
/// turn once any live cluster exists — new-cluster discovery still rides
/// the 3-turn cadence (n % 3 == 0) so isolates cost nothing. Walker bob
/// stays on the anim_tick cadence. No integrator.
fn drift_motion(n: u64) {
    let mut h = lock();
    if !h.marble_drift || h.reduce_motion {
        h.last_drift = None; // re-seed the clock on resume
        return;
    }
    let now = Instant::now();
    let dt = h
        .last_drift
        .map(|t| now.duration_since(t).as_secs_f64())
        .unwrap_or(1.0 / 60.0)
        .min(0.25);
    h.last_drift = Some(now);
    // The clock keeps time through a drag so a drop eases back onto the
    // advanced θ (drag_tick re-arms every ease) instead of time jumping.
    h.drift_phase += dt;
    if h.drag.is_some() {
        return;
    }
    let clustered = h
        .planets
        .values()
        .any(|p| p.is_marble && p.orbit.is_some());
    if clustered || n % 3 == 0 {
        drift_pass(&mut h);
    }
}

fn drag_tick() {
    let mut h = lock();
    let Some(d) = h.drag.as_mut() else {
        return;
    };
    unsafe {
        let buttons: usize = msg_send![class!(NSEvent), pressedMouseButtons];
        if buttons & 1 == 0 {
            let (project_id, moved, kind, entry_id) = match d {
                Drag::Panel { project_id, moved, .. } => {
                    (project_id.clone(), *moved, 0u8, String::new())
                }
                // Boombox decor gets its own code: a press-release that
                // never moved is a click (HUD toggle), not a placement.
                Drag::Decor { project_id, decor, moved, .. } => (
                    project_id.clone(),
                    *moved,
                    if decor.kind == DecorKind::Boombox { 3u8 } else { 1u8 },
                    String::new(),
                ),
                Drag::Walker {
                    project_id,
                    entry_id,
                    moved,
                    ..
                } => (project_id.clone(), *moved, 2u8, entry_id.clone()),
                // HUD slider: no click/drag question on release — the
                // stroke already wrote the remembered volume live.
                Drag::BoomboxSlider { project_id } => {
                    (project_id.clone(), 0.0, 4u8, String::new())
                }
                Drag::TodoTree {
                    project_id,
                    card_id,
                    moved,
                    ..
                } => (project_id.clone(), *moved, 5u8, card_id.clone()),
            };
            h.drag = None;
            // drift paused while this drag held
            // but the shared clock kept ticking — re-arm every orbit lerp
            // at the live frame so marbles ease back onto their rings
            // instead of snapping to the advanced θ.
            for planet in h.planets.values_mut() {
                if !planet.is_marble {
                    continue;
                }
                if let Some(o) = planet.orbit.as_mut() {
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    o.lerp_start = Instant::now();
                    o.lerp_from = NSPoint {
                        x: frame.mid_x(),
                        y: frame.mid_y(),
                    };
                }
            }
            if kind == 0 {
                let panel_and_marble = h.planets.get(&project_id).map(|p| (p.panel, p.is_marble));
                if let Some((panel, is_marble)) = panel_and_marble {
                    persist_position(&mut h, &project_id, panel);
                    // drag wins — the dropped
                    // marble sits ~2s, then eases in polar onto whatever
                    // ring its cluster describes at the drop spot. Orbit
                    // state is KEPT so the 220/280 hysteresis still
                    // decides membership; the pin branch re-arms the lerp.
                    if let Some(planet) = h.planets.get_mut(&project_id) {
                        planet.pin_until =
                            Some(Instant::now() + Duration::from_secs_f64(DRIFT_PIN_SECS));
                    }
                    if moved < 4.0 && is_marble {
                        if let Some(app) = app_handle() {
                            let _ = app.emit_to("main", "habitat:expand", project_id);
                        }
                    }
                }
            } else if kind == 1 {
                h.worlds_dirty = true;
            } else if kind == 3 {
                if moved < BOOMBOX_CLICK_SLOP {
                    boombox_click(&mut h, &project_id);
                } else {
                    h.worlds_dirty = true;
                    // A drag is not a click — and an open HUD never
                    // rides the box to its new spot.
                    boombox_hud::close(&mut h);
                }
            } else if kind == 4 {
                // HUD slider: the value was applied live during the
                // stroke; set_volume's worlds_dirty persists at 10Hz.
            } else if kind == 5 {
                if let Some(planet) = h.planets.get(&project_id) {
                    if let Some(t) = planet.todo_trees.iter().find(|t| t.card_id == entry_id)
                    {
                        super::todo_tree::emit_moved(&entry_id, t.x, t.y);
                    }
                }
            } else if moved >= 4.0 {
                // kind == 2 (walker): a real drag, not a click — the
                // pending click dies. PROPS leftover (town usable): the
                // drop point hit-tests the board columns — a drop on the
                // billboard is a STATUS MOVE, never a failed park. A
                // drop on grass/disc with no column just resumes the
                // pose walk next anim tick.
                h.pending_click = None;
                let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
                let col = h.planets.get(&project_id).and_then(|planet| {
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    let local = NSPoint {
                        x: mouse.x - frame.origin.x,
                        y: frame.origin.y + frame.size.height - mouse.y,
                    };
                    super::draw::hit_board_column(planet, local)
                });
                match col {
                    // Todo column: the card stays/returns todo. Spending
                    // this roam's check-in is what makes "stays todo"
                    // hold — the walker parks at the board lip instead
                    // of re-checking-in a beat later. The rail flips
                    // doing→todo (a todo card no-ops there).
                    Some(0) => {
                        if let Some(planet) = h.planets.get_mut(&project_id) {
                            if let Some(r) = planet.roam.get_mut(&entry_id) {
                                r.via_checkin = true;
                            }
                        }
                        if let Some(app) = app_handle() {
                            let _ = app.emit("habitat:board-todo", entry_id);
                        }
                    }
                    // Doing column: the board touch by hand — emit the
                    // check-in (rail flips todo→doing) and mark it spent
                    // on this roam entry, so a drag back to Todo STAYS
                    // Todo (same stickiness as the walked check-in).
                    Some(1) => {
                        if let Some(planet) = h.planets.get_mut(&project_id) {
                            if let Some(r) = planet.roam.get_mut(&entry_id) {
                                r.via_checkin = true;
                            }
                        }
                        emit_board_checkin(&entry_id);
                    }
                    // Done column: no longer mass-evicts walkers via
                    // habitat:board-done (BOARD-MARK). Drop is a no-op;
                    // complete stays on the board UI / walker Done path.
                    Some(2) => {}
                    _ => {}
                }
            }
            return;
        }
        let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        // HUD slider strokes compute the value under the drag borrow,
        // then apply after the match (set_volume wants the whole h).
        let mut slider: Option<(String, f32)> = None;
        match h.drag.as_mut() {
            Some(Drag::Panel {
                project_id,
                grab_dx,
                grab_dy,
                start,
                moved,
            }) => {
                *moved = moved.max((mouse.x - start.x).hypot(mouse.y - start.y));
                let (dx, dy, pid) = (*grab_dx, *grab_dy, project_id.clone());
                if let Some(planet) = h.planets.get_mut(&pid) {
                    // A dragged planet is user-placed. Orbit state is
                    // kept through the drag so the hysteresis — not the
                    // drag — decides whether it leaves its ring;
                    // drift_pass re-arms the lerp under the pin.
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    let cx = mouse.x - dx;
                    let cy = mouse.y - dy;
                    let _: () = msg_send![panel,
                        setFrameOrigin: NSPoint {
                            x: cx - frame.size.width / 2.0,
                            y: cy - frame.size.height / 2.0,
                        }];
                }
            }
            Some(Drag::Decor {
                project_id,
                decor,
                start,
                moved,
            }) => {
                *moved = moved.max((mouse.x - start.x).hypot(mouse.y - start.y));
                let dref = *decor;
                let pid = project_id.clone();
                // Boombox: do not slide the sprite until past click slop.
                // Under that, mouse-up is the HUD toggle and boombox_pos stays.
                if dref.kind == DecorKind::Boombox && *moved < BOOMBOX_CLICK_SLOP {
                    return;
                }
                if let Some(planet) = h.planets.get_mut(&pid) {
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    let local = NSPoint {
                        x: mouse.x - frame.origin.x,
                        y: frame.origin.y + frame.size.height - mouse.y,
                    };
                    let side = (planet.radius + VIEW_PADDING) * 2.0;
                    let center = NSPoint { x: side / 2.0, y: side / 2.0 };
                    let (x, y) = view_to_disc(center, planet.radius, planet.projection, local);
                    match dref.kind {
                        DecorKind::Plant => {
                            if let Some(d) = planet.plants.get_mut(dref.index) {
                                d.x = x;
                                d.y = y;
                            }
                        }
                        DecorKind::Mushroom => {
                            if let Some(d) = planet.mushrooms.get_mut(dref.index) {
                                d.x = x;
                                d.y = y;
                            }
                        }
                        DecorKind::Building => {
                            if let Some(d) = planet.buildings.get_mut(dref.index) {
                                d.x = x;
                                d.y = y;
                            }
                        }
                        DecorKind::Boombox => {
                            planet.boombox_pos = Some(DecorPoint { x, y });
                        }
                    }
                    let snap = (
                        planet.plants.clone(),
                        planet.mushrooms.clone(),
                        planet.buildings.clone(),
                        planet.boombox_pos,
                    );
                    layout_decor(planet);
                    let view = planet.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                    drop(planet);
                    if let Some(w) = h.worlds.get_mut(&pid) {
                        w.plants = snap.0;
                        w.mushrooms = snap.1;
                        w.buildings = snap.2;
                        w.boombox_pos = snap.3;
                    }
                }
            }
            Some(Drag::Walker {
                project_id,
                entry_id,
                start,
                moved,
            }) => {
                *moved = moved.max((mouse.x - start.x).hypot(mouse.y - start.y));
                let pid = project_id.clone();
                let eid = entry_id.clone();
                if let Some(planet) = h.planets.get_mut(&pid) {
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    let local = NSPoint {
                        x: mouse.x - frame.origin.x,
                        y: frame.origin.y + frame.size.height - mouse.y,
                    };
                    let side = (planet.radius + VIEW_PADDING) * 2.0;
                    let center = NSPoint { x: side / 2.0, y: side / 2.0 };
                    let (x, y) = view_to_disc(center, planet.radius, planet.projection, local);
                    if let Some(r) = planet.roam.get_mut(&eid) {
                        r.x = x;
                        r.y = y;
                        r.tx = x;
                        r.ty = y;
                    }
                    layout_beings(planet);
                    let view = planet.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
            Some(Drag::TodoTree {
                project_id,
                card_id,
                start,
                moved,
            }) => {
                *moved = moved.max((mouse.x - start.x).hypot(mouse.y - start.y));
                let pid = project_id.clone();
                let cid = card_id.clone();
                if let Some(planet) = h.planets.get_mut(&pid) {
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    let local = NSPoint {
                        x: mouse.x - frame.origin.x,
                        y: frame.origin.y + frame.size.height - mouse.y,
                    };
                    let side = (planet.radius + VIEW_PADDING) * 2.0;
                    let center = NSPoint { x: side / 2.0, y: side / 2.0 };
                    let (x, y) = view_to_disc(center, planet.radius, planet.projection, local);
                    if let Some(t) = planet.todo_trees.iter_mut().find(|t| t.card_id == cid) {
                        let (cx, cy) = crate::todo_trees::clamp_to_grove(x, y);
                        t.x = cx;
                        t.y = cy;
                    }
                    super::todo_tree::layout(planet);
                    let view = planet.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
            Some(Drag::BoomboxSlider { project_id }) => {
                let pid = project_id.clone();
                if let Some(planet) = h.planets.get(&pid) {
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    let local = NSPoint {
                        x: mouse.x - frame.origin.x,
                        y: frame.origin.y + frame.size.height - mouse.y,
                    };
                    if let Some(r) = boombox_hud::rects(planet) {
                        slider = Some((pid, boombox_hud::slider_value(&r, local)));
                    }
                }
            }
            None => {}
        }
        if let Some((pid, v)) = slider {
            boombox_hud::set_volume(&mut h, &pid, v);
        }
    }
}

fn persist_position(h: &mut Habitat, project_id: &str, panel: usize) {
    unsafe {
        let panel = panel as *mut Object;
        let frame: NSRect = msg_send![panel, frame];
        let center = NSPoint {
            x: frame.mid_x(),
            y: frame.mid_y(),
        };
        let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        let all = screens();
        let chosen = all
            .iter()
            .find(|(_, f)| f.contains(center))
            .or_else(|| all.iter().find(|(_, f)| f.contains(mouse)))
            .or_else(|| all.first());
        let Some((screen_id, sframe)) = chosen else { return };
        let entry = h
            .worlds
            .entry(project_id.to_string())
            .or_insert_with(|| default_state(screen_id, 0));
        entry.screen_id = screen_id.clone();
        entry.x = center.x - sframe.origin.x;
        entry.y = sframe.origin.y + sframe.size.height - center.y;
        entry.last_used = now_epoch();
        save_worlds(h);
    }
}

fn anim_tick() {
    let mut h = lock();
    let now = Instant::now();
    let dt = h
        .last_anim
        .map(|t| now.duration_since(t).as_secs_f64())
        .unwrap_or(0.05)
        .min(0.25);
    h.last_anim = Some(now);
    // motion off (toggle or Reduce Motion) →
    // marbles sit at default size and orbit state clears, so a re-enable
    // re-seeds from the live frame. Runs before the Reduce Motion early
    // return so stilling also restores size.
    if h.reduce_motion || !h.marble_drift {
        orbit_restore(&mut h);
    }
    if h.reduce_motion {
        return; // static sprites under Reduce Motion
    }
    h.bob_phase += dt;
    h.anim_frame = h.anim_frame.wrapping_add(1);
    let bob_phase = h.bob_phase;
    // BUDGET still-planet law: an expanded planet dirties
    // its view ONLY on real motion this tick — a roam step or pause flip,
    // a bob change, a live locate stroke, or a visible caption. An
    // all-sleeper / empty planet paints nothing per tick; sleepers never
    // keep the view dirty. Marbles already `continue`.
    let locate = h.locate.clone();
    let locate_planet = h.locate_planet.clone();
    // PROPS leftover (town usable): a walker mid-drag is owned by
    // drag_tick — the pose branches below must NOT retarget it back to
    // the station on the same press (that fight was "drag moves roam
    // for a frame; next tick retargets"). The drop itself is handled in
    // drag_tick's release (a column drop is a status move).
    let drag_walker = match &h.drag {
        Some(Drag::Walker {
            project_id,
            entry_id,
            ..
        }) => Some((project_id.clone(), entry_id.clone())),
        _ => None,
    };
    // per-pack still-frame
    // aspects for the keep-up head math, snapshotted before the planet
    // loop (0.0 = no decoded frames → the generated person's height).
    let pack_aspects: Vec<f64> = h
        .pack_frames
        .iter()
        .map(|(_, pf)| pack_still_aspect(pf))
        .collect();
    for (pid, planet) in h.planets.iter_mut() {
        if planet.is_marble {
            continue; // marbles are bare and still
        }
        let mut dirty = false;
        // WORLD-PLACES leftover / WORLD-YARD: layout_beings reruns ONLY
        // when a roam x/y actually changed this tick — re-laying out
        // every tick held the habitat lock on the AppKit thread (menu
        // beach-ball). YARD undid the in-place forbid for the Doing
        // wander ONLY: a walking Work body moves every tick (layout
        // reruns, law 4). YARD leftover: ball OFF the amble never parks
        // (a Doing body re-lays out every tick); ball ON the dart parks
        // for each dwell and re-lays out on the darts. Bob alone still
        // dirties the display below.
        let mut moved = false;
        let now_i = Instant::now();
        let ids: Vec<String> = planet.walkers.iter().map(|b| b.entry_id.clone()).collect();
        // bob only on a tick the walker actually
        // steps (pause_end None and dist >= 0.025). Paused and asleep
        // walkers stand at bob 0; sleepers never enter the step branch.
        // a Doing walker bobs while
        // WALKING the grass. Ball OFF the smooth amble never parks, so
        // the bob never stands on the green; ball ON the dart dwell
        // (600–1400 ms) stands at bob 0 between darts — the keep-up
        // ready stance. Stand/Sit still park idle.
        let mut stepping = vec![false; ids.len()];
        // Ambient random roam dies the moment any walker on this planet
        // works or sits — the board, not the clock, drives motion. Sleepers
        // don't count (a closed window is not visible work) and are never
        // retargeted themselves.
        let any_work_sit = planet
            .walkers
            .iter()
            .any(|b| !b.asleep && matches!(b.pose, Pose::Work | Pose::Sit));
        // the beach
        // ball flips the Doing footwork — OFF keeps the smooth amble,
        // ON restores the exact pre-amble dart (the dwell-and-dart IS
        // the keep-up footwork). Captured once per planet per tick.
        let keep_up = planet.beach_ball;
        // WORLD-STOP: Done walks to the south/near lip of the painted
        // rails, not STATION_BOARD (back of the blit). Same rects as the
        // Done-column hit-test.
        let lip = super::draw::board_near_lip(
            planet.radius,
            planet.projection,
            planet.marble_scale,
        );
        for (i, id) in ids.iter().enumerate() {
            let (pose, asleep) = planet
                .walkers
                .get(i)
                .map(|b| (b.pose, b.asleep))
                .unwrap_or((Pose::Roam, false));
            // create/get the roam BEFORE the
            // sleeper continue — a new/asleep claim must not miss its
            // house roam this tick. The seed is pose-aware (the old 0.2
            // cos/sin circle fallback is gone): Stand/Work come from the
            // igloo (WORLD-TOWN: STATION_HOUSE only — user buildings stay
            // décor), Sit seeds AT the rim station, WalkOff keeps the
            // current roam or starts at the igloo — never past
            // HOME_HIDE_X, so the walk off the left rim is visible.
            // Computed before the roam borrow.
            let house = house_spawn(planet, id);
            let roam = planet.roam.entry(id.clone()).or_insert_with(|| match pose {
                Pose::Stand | Pose::Work => {
                    let (tx, ty) = station_target(pose, id);
                    RoamState {
                        x: house.0,
                        y: house.1,
                        tx,
                        ty,
                        pause_end: None,
                        facing_left: false,
                        facing_dir: 0,
                        off_emitted: false,
                        via_board: false,
                        via_home: false,
                        via_checkin: false,
                        checkin_emit_at: None,
                        wander_seed: 0,
                    }
                }
                Pose::Sit => {
                    let (tx, ty) = station_target(Pose::Sit, id);
                    RoamState {
                        x: tx,
                        y: ty,
                        tx,
                        ty,
                        pause_end: None,
                        facing_left: false,
                        facing_dir: 0,
                        off_emitted: false,
                        via_board: false,
                        via_home: false,
                        via_checkin: false,
                        checkin_emit_at: None,
                        wander_seed: 0,
                    }
                }
                Pose::WalkOff => RoamState {
                    x: house.0,
                    y: house.1,
                    tx: lip.0,
                    ty: lip.1,
                    pause_end: None,
                    facing_left: false,
                        facing_dir: 0,
                    off_emitted: false,
                    via_board: false,
                    via_home: false,
                    via_checkin: false,
                    checkin_emit_at: None,
                    wander_seed: 0,
                },
                Pose::Roam => RoamState {
                    x: house.0,
                    y: house.1,
                    tx: house.0,
                    ty: house.1,
                    pause_end: None,
                    facing_left: false,
                        facing_dir: 0,
                    off_emitted: false,
                    via_board: false,
                    via_home: false,
                    via_checkin: false,
                    checkin_emit_at: None,
                    wander_seed: 0,
                },
            });
            // A walker that left WalkOff (re-claimed) re-arms its emit so
            // the NEXT Done walks off and evicts again — and re-arms the
            // board visit too (LEFTOVER via_board).
            if pose != Pose::WalkOff {
                roam.off_emitted = false;
                roam.via_board = false;
                roam.via_home = false;
            }
            // Mid-drag: drag_tick owns this walker's x/y/tx/ty for the
            // press — no station retarget, no step, no bob here.
            if drag_walker
                .as_ref()
                .is_some_and(|(dpid, deid)| dpid == pid && deid == id)
            {
                continue;
            }
            // Closed sleepers park in the camp (north of igloo, west of
            // grass) — dim + still, never the green. Minimized is NOT
            // asleep (full-color on the green). A Done card still walks
            // a sleeper off: the complete ritual closes the window, and
            // the walk-off narrates the eviction.
            if asleep && pose != Pose::WalkOff {
                let (tx, ty) = sleeper_target(id);
                if (roam.x - tx).abs() > 1e-9 || (roam.y - ty).abs() > 1e-9 {
                    roam.x = tx;
                    roam.y = ty;
                    moved = true;
                }
                roam.tx = tx;
                roam.ty = ty;
                roam.pause_end = None;
                continue;
            }
            if planet
                .tree_chores
                .iter()
                .any(|c| c.walker_id == *id)
            {
                continue;
            }
            match pose {
                // Needed: bad news skips the queue — snap to the gate, never
                // a scenic route. Sit = idle frame 0, no bob, full bright.
                Pose::Sit => {
                    let (tx, ty) = station_target(Pose::Sit, id);
                    if (roam.x - tx).abs() > 1e-9 || (roam.y - ty).abs() > 1e-9 {
                        roam.x = tx;
                        roam.y = ty;
                        moved = true;
                    }
                    roam.tx = tx;
                    roam.ty = ty;
                    roam.pause_end = None;
                }
                // Stand walks to the board lip (not the post foot).
                // Work ambles the grass AABB. Never moonwalk on one pixel.
                Pose::Stand | Pose::Work => {
                    // Back from off the left rim (re-claim after Done):
                    // the walker was invisible, so re-enter AT THE IGLOO
                    // and walk the return — never a snap onto the station.
                    if roam.x < HOME_HIDE_X {
                        roam.x = house.0;
                        roam.y = house.1;
                        // the return from hide
                        // is a fresh claim journey — re-arm the board
                        // check-in so this claim touches the board too.
                        roam.via_checkin = false;
                        moved = true;
                    }
                    if pose == Pose::Stand {
                        roam.tx = lip.0;
                        roam.ty = lip.1;
                        roam.pause_end = None;
                    } else {
                        if roam.wander_seed == 0 {
                            roam.wander_seed = yard_seed(id);
                            let (tx, ty) = yard_pick(&mut roam.wander_seed);
                            roam.tx = tx;
                            roam.ty = ty;
                            roam.pause_end = None;
                        } else if !yard_contains(roam.tx, roam.ty) {
                            let (tx, ty) = yard_pick(&mut roam.wander_seed);
                            roam.tx = tx;
                            roam.ty = ty;
                            roam.pause_end = None;
                        }
                        if !keep_up {
                            // Ball off: the amble never dwells — a pause
                            // left by a mid-dwell toggle-off dies now,
                            // never a standing park on the green.
                            roam.pause_end = None;
                        }
                    }
                    let mut dx = roam.tx - roam.x;
                    let mut dy = roam.ty - roam.y;
                    let mut dist = (dx * dx + dy * dy).sqrt();
                    // CHECKIN (+ PROPS leftover retry): a
                    // Stand (Todo) walker at the near lip of the board
                    // checks in — the rail flips that window's card
                    // todo→doing and the next sync walks it to the field
                    // as Work. The emit is fire-and-forget, so while the
                    // snapshot STILL says todo it re-fires on a ~0.5s
                    // throttle: a missed rail (or an AX-untrusted no-op
                    // sync) used to park the walker at the lip forever.
                    // via_checkin is NOT set on this auto path — it stays
                    // the "user dropped on Todo, don't auto-flip" flag
                    // (only the drag-drop column paths set it). The rail's
                    // listener is idempotent (todo→doing only), so the
                    // repeats are safe.
                    if pose == Pose::Stand && !roam.via_checkin && dist < 0.05 {
                        let still_todo = planet
                            .walker_entries
                            .iter()
                            .any(|e| &e.id == id && e.card_status == "todo");
                        let due = roam
                            .checkin_emit_at
                            .is_none_or(|t| t.elapsed() >= Duration::from_millis(500));
                        if still_todo && due {
                            roam.checkin_emit_at = Some(now_i);
                            emit_board_checkin(id);
                        }
                    }
                    if pose == Pose::Work {
                        if keep_up {
                            // Ball ON — the exact pre-amble dart: on the
                            // grass point (dist < YARD_ARRIVE, 0.04)
                            // dwell 600–1400 ms (hashed off the stream)
                            // STANDING — the dwell is the pause, no step,
                            // no bob — then pick a NEW random point in
                            // the box and dart on.
                            if dist < YARD_ARRIVE && roam.pause_end.is_none() {
                                // Arrival slop (<= YARD_ARRIVE) snaps onto
                                // the green — the one clamp that can move
                                // a body, sub-0.04, invisible.
                                let (cx, cy) = yard_clamp(roam.x, roam.y);
                                roam.x = cx;
                                roam.y = cy;
                                let dwell = yard_dwell_ms(&mut roam.wander_seed);
                                roam.pause_end =
                                    Some(now_i + Duration::from_millis(dwell));
                                dirty = true; // point reached — the walk cycle parks
                            }
                            if let Some(end) = roam.pause_end {
                                if now_i < end {
                                    continue; // dwelling on the point
                                }
                                roam.pause_end = None;
                                let (tx, ty) = yard_pick(&mut roam.wander_seed);
                                roam.tx = tx;
                                roam.ty = ty;
                                dx = roam.tx - roam.x;
                                dy = roam.ty - roam.y;
                                dist = (dx * dx + dy * dy).sqrt();
                                dirty = true; // new point — the walk cycle resumes
                            }
                        } else if dist < YARD_SAIL {
                            // Ball OFF — smooth wander: BEFORE arriving
                            // (dist < YARD_SAIL, 0.12) the NEXT point is
                            // already picked — no pause, no snap-turn.
                            // The pick prefers a heading that is not a
                            // reverse of the current dx,dy (an about-face
                            // is skipped up to 3 stream steps), so the
                            // body curves onto the next leg instead of
                            // darting back.
                            let (tx, ty) = yard_pick_sailing(
                                &mut roam.wander_seed,
                                roam.x,
                                roam.y,
                                dx,
                                dy,
                            );
                            roam.tx = tx;
                            roam.ty = ty;
                            dx = roam.tx - roam.x;
                            dy = roam.ty - roam.y;
                            dist = (dx * dx + dy * dy).sqrt();
                            dirty = true; // target handed off mid-stride
                        }
                    }
                    if dist >= 0.025 {
                        // Ball OFF: slow amble on the grass (0.045·dt —
                        // ball-in-the-air leftover: 0.075 still read
                        // brisk). Ball ON: the dart keeps the full
                        // 0.16·dt on the green — the keep-up footwork.
                        // Either way the off-grass walk-IN (from the
                        // igloo / board lip / a hand drop) keeps the old
                        // 0.16·dt — the visible approach is unchanged.
                        let speed = if pose == Pose::Work
                            && !keep_up
                            && yard_contains(roam.x, roam.y)
                        {
                            0.045
                        } else {
                            0.16
                        };
                        let step = (speed * dt).min(dist);
                        roam.x += dx / dist * step;
                        roam.y += dy / dist * step;
                        roam.facing_left = dx < -0.002;
                        roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
                        stepping[i] = true;
                        moved = true; // walking (to the lip / across the grass)
                    }
                    // WORLD-YARD law 6: a Doing walker ON the grass never
                    // leaves it — hard clamp to the inset AABB (a no-op on
                    // the convex walk between two in-box points; the wall
                    // against drift/bugs — never the igloo x <= -0.50,
                    // never the sign footprint). The walk-IN from the
                    // igloo / board lip / a hand drop stays a visible
                    // approach: its target is in the box and converges —
                    // clamping THAT leg would pop the body onto the edge.
                    if pose == Pose::Work && yard_contains(roam.x, roam.y) {
                        let (cx, cy) = yard_clamp(roam.x, roam.y);
                        if (cx - roam.x).abs() > 1e-12 || (cy - roam.y).abs() > 1e-12 {
                            roam.x = cx;
                            roam.y = cy;
                            moved = true;
                        }
                    }
                }
                // Done: walk to the near lip of the board, blit flips,
                // dwell so checkout reads, walk home to the igloo, dwell
                // at the door, then unclaim. No north walk-through. No
                // left-rim curtain; last walker lets the planet shrink.
                Pose::WalkOff => {
                    let is_droid = planet
                        .walkers
                        .get(i)
                        .and_then(|b| b.droid)
                        .is_some();
                    if is_droid {
                        let (tx, ty) = walkoff_target(id, roam.x, roam.y);
                        let dx = tx - roam.x;
                        let dy = ty - roam.y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        roam.tx = tx;
                        roam.ty = ty;
                        if dist < 0.05 || roam.x < HOME_HIDE_X {
                            if !roam.off_emitted {
                                roam.off_emitted = true;
                                emit_walked_off(id);
                                dirty = true;
                            }
                        } else {
                            let step = (0.16 * dt).min(dist);
                            roam.x += dx / dist * step;
                            roam.y += dy / dist * step;
                            roam.facing_left = dx < -0.002;
                        roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
                            stepping[i] = true;
                            moved = true;
                        }
                        continue;
                    }
                    if roam.x < HOME_HIDE_X {
                        roam.x = house.0;
                        roam.y = house.1;
                        roam.via_board = false;
                        roam.via_home = false;
                        roam.off_emitted = false;
                        roam.pause_end = None;
                        moved = true;
                    }
                    if !roam.via_board {
                        let dx = lip.0 - roam.x;
                        let dy = lip.1 - roam.y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist < 0.05 {
                            roam.via_board = true;
                            roam.x = lip.0;
                            roam.y = lip.1;
                            roam.tx = lip.0;
                            roam.ty = lip.1;
                            roam.pause_end =
                                Some(now_i + Duration::from_millis(400));
                            dirty = true; // lift blit hold this frame
                            moved = true;
                        } else {
                            roam.tx = lip.0;
                            roam.ty = lip.1;
                            let step = (0.16 * dt).min(dist);
                            roam.x += dx / dist * step;
                            roam.y += dy / dist * step;
                            roam.facing_left = dx < -0.002;
                        roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
                            stepping[i] = true;
                            moved = true;
                            continue;
                        }
                    }
                    if let Some(end) = roam.pause_end {
                        if now_i < end {
                            continue;
                        }
                        roam.pause_end = None;
                    }
                    if !roam.via_home {
                        let dx = house.0 - roam.x;
                        let dy = house.1 - roam.y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist < 0.05 {
                            roam.via_home = true;
                            roam.x = house.0;
                            roam.y = house.1;
                            roam.tx = house.0;
                            roam.ty = house.1;
                            roam.pause_end =
                                Some(now_i + Duration::from_millis(400));
                            dirty = true;
                            moved = true;
                            continue;
                        } else {
                            roam.tx = house.0;
                            roam.ty = house.1;
                            let step = (0.16 * dt).min(dist);
                            roam.x += dx / dist * step;
                            roam.y += dy / dist * step;
                            roam.facing_left = dx < -0.002;
                        roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
                            stepping[i] = true;
                            moved = true;
                            continue;
                        }
                    }
                    roam.tx = house.0;
                    roam.ty = house.1;
                    if !roam.off_emitted {
                        roam.off_emitted = true;
                        emit_walked_off(id);
                        dirty = true;
                    }
                }
                Pose::Roam => {
                    if any_work_sit {
                        // Amble off while anyone works/sits: stand in
                        // place — no re-park, no pause cycle.
                        roam.tx = roam.x;
                        roam.ty = roam.y;
                        roam.pause_end = None;
                        continue;
                    }
                    if let Some(end) = roam.pause_end {
                        if now_i < end {
                            continue;
                        }
                        // Pause ended — both branches below (re-park / step)
                        // mark the dirty walk-cycle change.
                        roam.pause_end = None;
                    }
                    let dx = roam.tx - roam.x;
                    let dy = roam.ty - roam.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < 0.025 {
                        let a = bob_phase * 2.7 + i as f64 * 4.1;
                        let rad = 0.22 + 0.42 * ((bob_phase * 1.3 + i as f64).sin() * 0.5 + 0.5);
                        roam.tx = a.cos() * rad;
                        roam.ty = a.sin() * rad;
                        roam.pause_end = Some(
                            now_i + Duration::from_millis(700 + ((i as u64 * 410) % 1400)),
                        );
                        dirty = true; // target reached — the walk cycle parks
                    } else {
                        let step = (0.16 * dt).min(dist);
                        roam.x += dx / dist * step;
                        roam.y += dy / dist * step;
                        roam.facing_left = dx < -0.002;
                        roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
                        stepping[i] = true;
                        moved = true; // roam stepped
                    }
                }
            }
        }
        // WORLD-PLACES leftover / WORLD-YARD: full layout ONLY on a real
        // roam position change — re-laying out every tick held the
        // habitat lock on the AppKit thread (menu beach-ball). The Doing
        // amble moves every tick (layout reruns there, law 4); the
        // ball-on dart moves only between dwells. A move implies a
        // display refresh.
        if super::todo_tree::tick(planet, dt, pid) {
            moved = true;
        }
        if moved {
            layout_beings(planet);
            dirty = true;
        }
        for (i, b) in planet.walkers.iter_mut().enumerate() {
            let bob = if stepping.get(i).copied().unwrap_or(false) {
                (bob_phase * 2.2 + (i as f64) * 1.1).sin() * 1.4
            } else {
                0.0
            };
            if bob != b.bob {
                b.bob = bob;
                dirty = true;
            }
        }
        // beach-ball
        // keep-up. The per-planet toggle (right-click menu, persisted
        // next to the boombox) plus ≥ 2 awake Doing walkers → the ball
        // hops head to head IN VIEW PIXELS — TOUCH walker A's head, pop
        // straight up the screen, arc, drop, TOUCH walker B's LIVE head
        // (a darting walker still gets tagged) — one hop per 1.0–1.45s,
        // then the next hop with NO hold on the head (2px-kiss leftover:
        // constant speed, touch → pop → next hop, never a linger, no
        // rest on the ground), round-robin through the Doing set (never the
        // same pair back-to-back when 3+). One or zero Doing → hidden. The ball is
        // NOT a third wanderer: it lands on bodies, never on a random
        // grass pixel, and the walkers' dart IS the keep-up footwork.
        // Heads come from walker_head_view — the same blit math draw.rs
        // uses — so a touch overlaps the drawn sprite exactly. In flight
        // the ball moves every tick → the view stays dirty.
        if planet.beach_ball {
            let doing: Vec<(String, (f64, f64, f64))> = planet
                .walkers
                .iter()
                .filter(|b| !b.asleep && b.pose == Pose::Work)
                .filter_map(|b| {
                    walker_head_view(&pack_aspects, planet, &b.entry_id)
                        .map(|head| (b.entry_id.clone(), head))
                })
                .collect();
            if doing.len() >= 2 {
                // 2px-kiss leftover: the hop is done at elapsed >= dur —
                // NO contact hold, no ease, no dwell, no slowdown in the
                // last stretch. Linear f; the ball touches the lander's
                // LIVE head at f = 1 and the next hop pops immediately.
                // Never a rest on the grass.
                let hop_done = match &planet.ball {
                    Some(ball) => {
                        !doing.iter().any(|(id, _)| id == &ball.to_id)
                            || now_i.duration_since(ball.at).as_secs_f64() >= ball.dur
                    }
                    None => true,
                };
                if hop_done {
                    // `from` = the VIEW-PIXEL head of the last lander,
                    // snapshotted NOW: the previous hop ended on that
                    // head (f = 1), so the new hop leaves from the touch
                    // point — never from the feet, never a grass pixel.
                    let (from, mut stream, last_to) = match &planet.ball {
                        Some(ball) => (
                            doing
                                .iter()
                                .find(|(id, _)| id == &ball.to_id)
                                .map(|(_, (hx, hy, _))| (*hx, *hy))
                                .unwrap_or(ball.from),
                            ball.stream,
                            Some(ball.to_id.clone()),
                        ),
                        None => (
                            (doing[0].1 .0, doing[0].1 .1),
                            yard_seed(&doing[0].0),
                            None,
                        ),
                    };
                    // Round-robin: land on the NEXT Doing body after the
                    // last target — cycles the set (with exactly 2 it
                    // alternates; 3+ never repeats a pair back-to-back).
                    let next = last_to
                        .as_ref()
                        .and_then(|lt| doing.iter().position(|(id, _)| id == lt))
                        .map(|i| (i + 1) % doing.len())
                        .unwrap_or(1);
                    let dur = ball_hop_secs(&mut stream);
                    // The lift is SCREEN-space: 1.2–1.8 × the target's
                    // drawn sprite height — clearly off the grass at
                    // mid-hop, never a bulge along the oval.
                    let arc = ball_arc_heads(&mut stream) * doing[next].1 .2;
                    planet.ball = Some(BallState {
                        from,
                        to_id: doing[next].0.clone(),
                        at: now_i,
                        dur,
                        arc,
                        stream,
                    });
                }
                dirty = true; // in flight — redraw every tick
            } else if planet.ball.take().is_some() {
                dirty = true; // dropped below 2 Doing — hidden
            }
        } else if planet.ball.take().is_some() {
            dirty = true; // toggled off — hidden
        }
        // Optional glance: the keep-up target faces the incoming ball in
        // the last stretch of the hop (cheap; the walk facing resumes the
        // tick after it lands). Both x's are VIEW px (the head helper) —
        // never a view/disc mix.
        if let Some(ball) = &planet.ball {
            let f = (now_i.duration_since(ball.at).as_secs_f64() / ball.dur).min(1.0);
            if f > 0.45 {
                let from_x = ball.from.0;
                let to_id = ball.to_id.clone();
                if let Some((hx, _, _)) = walker_head_view(&pack_aspects, planet, &to_id) {
                    let ball_x = from_x + (hx - from_x) * f;
                    if let Some(r) = planet.roam.get_mut(&to_id) {
                        r.facing_left = ball_x < hx;
                        r.facing_dir = if ball_x < hx {
                            droid_pack::DIR_W
                        } else {
                            droid_pack::DIR_E
                        };
                    }
                }
            }
        }
        if !dirty {
            // Transient overlays keep painting for their window only: the
            // locate stroke (~500ms) and a visible caption (~2s). Their
            // set/expire sites dirty the transitions; this covers the ticks
            // in between so the ring/pill never half-draws.
            dirty = locate.as_ref().is_some_and(|(id, at)| {
                at.elapsed() < LOCATE_TTL && planet.walkers.iter().any(|b| &b.entry_id == id)
            }) || locate_planet.as_ref().is_some_and(|(id, at)| {
                id == pid && at.elapsed() < PLANET_LOCATE_TTL
            }) || planet
                .caption
                .as_ref()
                .is_some_and(|(_, at)| at.elapsed() < CAPTION_TTL);
        }
        if dirty {
            unsafe {
                let view = planet.view as *mut Object;
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
        }
    }
    // The marble drift lives in drift_tick, which runs every habitat
    // tick (~16ms) so drifting marbles don't step at this 3-tick walker
    // cadence.
}

/// One marble drift turn. Track law is on the constants above
/// (HOST_JOIN_DIST, OVAL_RATIO, DRIFT_PIN_SECS). Never write worlds.json.
fn drift_pass(h: &mut Habitat) {
    let now = Instant::now();
    // (project_id, global center, screen_id) for every live marble, and
    // for every expanded planet — the candidate suns. Expanded planets
    // are never moved by us.
    let mut marbles: Vec<(String, NSPoint, String)> = Vec::new();
    // (project_id, global center, screen_id, LIVE radius) for expanded
    // planets — the radius is the scroll-zoomed drawn disc, so the ring
    // and the collider follow a growing chip.
    let mut expanded: Vec<(String, NSPoint, String, f64)> = Vec::new();
    for (pid, planet) in h.planets.iter() {
        let Some(w) = h.worlds.get(pid) else { continue };
        let panel = planet.panel as *mut Object;
        let frame: NSRect = unsafe { msg_send![panel, frame] };
        let center = NSPoint {
            x: frame.mid_x(),
            y: frame.mid_y(),
        };
        if planet.is_marble {
            marbles.push((pid.clone(), center, w.screen_id.clone()));
        } else {
            expanded.push((pid.clone(), center, w.screen_id.clone(), planet.radius));
        }
    }
    // Last tick's sun key per marble — the host / leave-edge hysteresis
    // memory.
    let prev_sun: HashMap<String, String> = marbles
        .iter()
        .filter_map(|(pid, _, _)| {
            let key = h.planets.get(pid)?.orbit.as_ref()?.sun_key.clone();
            Some((pid.clone(), key))
        })
        .collect();
    let all = unsafe { screens() };
    let visible = unsafe { visible_frames() };
    let t = h.drift_phase;
    let pad = MARBLE_RADIUS + 8.0;
    // Barycenter keys still alive this tick — stale homes drop at the end.
    let mut live_bary_keys: Vec<String> = Vec::new();
    // Sun 1: the nearest expanded planet on the same screen
    // (marble index, sun key, sun center) for hosted marbles; the rest
    // fall through to the barycenter pool.
    let mut hosted: Vec<(usize, String, NSPoint)> = Vec::new();
    let mut free: Vec<usize> = Vec::new();
    for (i, (pid, center, screen_id)) in marbles.iter().enumerate() {
        // Candidate suns on this screen: (planet id, center, distance off
        // the LIVE disc edge). Join/leave/switch all measure edge distance
        // (center dist − live radius, floored at 0) so the same 280/360/80
        // law holds at any zoom — a bigger chip keeps its ring.
        let cands: Vec<(&String, NSPoint, f64)> = expanded
            .iter()
            .filter(|(_, _, sid, _)| sid == screen_id)
            .map(|(id, c, _, live_r)| {
                let d = (c.x - center.x).hypot(c.y - center.y);
                (id, *c, (d - live_r).max(0.0))
            })
            .collect();
        let cur_host = prev_sun
            .get(pid)
            .and_then(|k| k.strip_prefix(HOST_PREFIX));
        let mut chosen: Option<(&str, NSPoint)> = None;
        if let Some(cur_id) = cur_host {
            if let Some((id, c, d)) = cands.iter().find(|(id, _, _)| id.as_str() == cur_id) {
                if *d <= HOST_LEAVE_DIST {
                    chosen = Some((id.as_str(), *c));
                    // Host hysteresis: switch only if a rival sun on this
                    // screen is ≥ HOST_SWITCH_CLOSER pt closer, so the sun
                    // never flickers between two near planets.
                    if let Some((nid, nc, nd)) = cands
                        .iter()
                        .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                    {
                        if nid.as_str() != cur_id && *nd <= d - HOST_SWITCH_CLOSER {
                            chosen = Some((nid.as_str(), *nc));
                        }
                    }
                }
            }
        }
        if chosen.is_none() {
            chosen = cands
                .iter()
                .filter(|(_, _, d)| *d <= HOST_JOIN_DIST)
                .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(id, c, _)| (id.as_str(), *c));
        }
        match chosen {
            Some((epid, c)) => hosted.push((i, format!("{HOST_PREFIX}{epid}"), c)),
            None => free.push(i),
        }
    }
    // Sun 2: the frozen barycenter of the unhosted cluster
    // Union-find over hysteresis edges among FREE marbles: ≤ join always
    // links; ≤ leave links only pairs that shared a sun last tick.
    let rn = free.len();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut parent: Vec<usize> = (0..rn).collect();
    for a in 0..rn {
        for b in (a + 1)..rn {
            let (ia, ib) = (free[a], free[b]);
            if marbles[ia].2 != marbles[ib].2 {
                continue; // same screen only — never hop displays
            }
            let d =
                (marbles[ia].1.x - marbles[ib].1.x).hypot(marbles[ia].1.y - marbles[ib].1.y);
            let linked = d <= DRIFT_JOIN_DIST
                || (d <= DRIFT_LEAVE_DIST
                    && matches!(
                        (prev_sun.get(&marbles[ia].0), prev_sun.get(&marbles[ib].0)),
                        (Some(x), Some(y)) if x == y
                    ));
            if linked {
                let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                if ra != rb {
                    parent[ra] = rb;
                }
            }
        }
    }
    let mut comps: HashMap<usize, Vec<usize>> = HashMap::new();
    for k in 0..rn {
        let root = find(&mut parent, k);
        comps.entry(root).or_default().push(k);
    }
    // (sun key, sun center, member marble indices, is_host) for every
    // live sun this tick.
    let mut groups: Vec<(String, NSPoint, Vec<usize>, bool)> = Vec::new();
    {
        let mut by_host: HashMap<String, (NSPoint, Vec<usize>)> = HashMap::new();
        for (i, key, c) in hosted {
            by_host.entry(key).or_insert_with(|| (c, Vec::new())).1.push(i);
        }
        for (key, (c, idxs)) in by_host {
            groups.push((key, c, idxs, true));
        }
    }
    for members in comps.into_values() {
        if members.len() < 2 {
            // Isolate with no sun: sit, orbit state cleared.
            let (pid, _, _) = &marbles[free[members[0]]];
            if let Some(planet) = h.planets.get_mut(pid) {
                planet.orbit = None;
            }
            continue;
        }
        let mut ids: Vec<&str> = members
            .iter()
            .map(|k| marbles[free[*k]].0.as_str())
            .collect();
        ids.sort_unstable();
        let key = format!("{BARY_PREFIX}{}", ids.join("\n"));
        if !h.clusters.contains_key(&key) {
            // Recapture: home = the live centroid ONCE. Never walked —
            // the figure cannot drift across the desk.
            let (mut sx, mut sy) = (0.0, 0.0);
            for k in &members {
                let c = marbles[free[*k]].1;
                sx += c.x;
                sy += c.y;
            }
            let n = members.len() as f64;
            h.clusters.insert(
                key.clone(),
                ClusterState {
                    home: NSPoint {
                        x: sx / n,
                        y: sy / n,
                    },
                },
            );
        }
        let home = h.clusters[&key].home;
        live_bary_keys.push(key.clone());
        groups.push((
            key,
            home,
            members.iter().map(|k| free[*k]).collect(),
            false,
        ));
    }
    // Tracks: every marble onto its own path around its sun
    for (sun_key, sun_raw, idxs, is_host) in groups {
        let rot = sun_rot(&sun_key);
        // Hosted tracks take the live-radius circle — only phase0 /
        // period / dir stay hashed per marble, so the hashed-radius
        // range is bary-only.
        let (r_min, r_max) = if is_host {
            (0.0, 0.0)
        } else {
            (BARY_R_MIN, BARY_R_MAX)
        };
        // Independent tracks: radius / phase0 / direction / period are
        // all hashed from the marble's OWN id — never opposite ends of
        // a stick, never one shared θ.
        let tracks: Vec<(usize, TrackParams)> = idxs
            .iter()
            .map(|m| (*m, track_params(&marbles[*m].0, r_min, r_max)))
            .collect();
        // Host lookup: the panel (z boolean) and the LIVE scroll-zoomed
        // radius (ring + collider). A host group whose planet vanished
        // mid-pass is skipped — next tick re-hosts or goes bary.
        let host_info: Option<(usize, f64)> = if is_host {
            sun_key
                .strip_prefix(HOST_PREFIX)
                .and_then(|epid| h.planets.get(epid))
                .map(|p| (p.panel, p.radius))
        } else {
            None
        };
        if is_host && host_info.is_none() {
            continue;
        }
        // The hosted ring: a CIRCLE at live radius + two marble-widths
        // + the marble radius. Bigger chip → bigger ring.
        let ring_r = host_info.map(|(_, live_r)| host_ring_r(live_r));
        // Screen keep-in: nudge the SUN inward first (per tick only —
        // the frozen home is never written back, and a host planet's
        // panel never moves) so the full-size figure + marble pad stays
        // on the group's screen; only a screen too small even at the
        // best sun spot shrinks r. Marbles are never clamped
        // independently, and neither (0,0) nor visibleFrame.origin is
        // ever a target.
        let mut sun = sun_raw;
        let mut rscale = 1.0;
        let group_screen = &marbles[idxs[0]].2;
        if let Some((_, f)) = visible
            .iter()
            .find(|(id, _)| id == group_screen)
            .or_else(|| visible.first())
        {
            let r_top = match ring_r {
                Some(r) => r,
                None => tracks.iter().map(|(_, p)| p.r).fold(0.0, f64::max),
            };
            let mut inset = f.inset(pad + r_top);
            if inset.origin.x > inset.max_x() || inset.origin.y > inset.max_y() {
                inset = f.inset(pad);
                if inset.origin.x > inset.max_x() || inset.origin.y > inset.max_y() {
                    inset = f.inset(0.0); // degenerate screen — keep discs on
                }
            }
            sun = NSPoint {
                x: sun.x.clamp(inset.origin.x, inset.max_x().max(inset.origin.x)),
                y: sun.y.clamp(inset.origin.y, inset.max_y().max(inset.origin.y)),
            };
            let fit = (sun.x - f.origin.x - pad)
                .min(f.max_x() - sun.x - pad)
                .min(sun.y - f.origin.y - pad)
                .min(f.max_y() - sun.y - pad);
            if r_top > fit {
                rscale = (fit / r_top).clamp(0.0, 1.0);
                if let Some((_, live_r)) = host_info {
                    // The standoff law dies first: a host ring never
                    // scales below one marble-width off the LIVE face.
                    let min_r = live_r + MARBLE_RADIUS + 2.0 * MARBLE_RADIUS;
                    rscale = rscale.max((min_r / r_top).min(1.0));
                }
            }
        }
        for (m, params) in &tracks {
            let (pid, live, screen_id) = &marbles[*m];
            let Some(planet) = h.planets.get_mut(pid) else {
                continue;
            };
            // A new sun (first join, host switch, or a barycenter
            // recapture) re-arms the ease from the live frame — joins
            // never teleport.
            let just_joined = planet
                .orbit
                .as_ref()
                .is_none_or(|o| o.sun_key != sun_key);
            if just_joined {
                planet.orbit = Some(MarbleOrbit {
                    sun_key: sun_key.clone(),
                    lerp_start: now,
                    lerp_from: *live,
                    front: true,
                });
            }
            if planet.pin_until.is_some_and(|until| now < until) {
                // Drag wins: sit, and keep the ease armed at the live
                // frame so the rejoin glides onto the track instead of
                // jumping. The pin never stills anyone else.
                if let Some(o) = planet.orbit.as_mut() {
                    o.lerp_start = now;
                    o.lerp_from = *live;
                }
                continue;
            }
            // Even speed along the path: constant dθ, no Kepler.
            let theta =
                params.phase0 + params.dir * std::f64::consts::TAU * (t / params.period);
            // This marble's own ring radius right now: the LIVE host
            // circle, or the hashed bary oval radius.
            let r_now = match ring_r {
                Some(r) => r * rscale,
                None => params.r * rscale,
            };
            let (dx, dy) = if ring_r.is_some() {
                circle_offset(r_now, theta, rot)
            } else {
                oval_offset(r_now, theta, rot)
            };
            let target = NSPoint {
                x: sun.x + dx,
                y: sun.y + dy,
            };
            // Join ease: ~6s ease-IN from the armed live frame onto THIS
            // marble's own ring, POLAR — θ (shortest arc) and r ease
            // along the ring, never a cartesian straight line through
            // the chip, never a lerp toward another marble.
            let (lerp_start, lerp_from) = planet
                .orbit
                .as_ref()
                .map(|o| (o.lerp_start, o.lerp_from))
                .unwrap_or((now, *live));
            let f = (now.duration_since(lerp_start).as_secs_f64() / JOIN_EASE_SECS).min(1.0);
            let mut pos = if f >= 1.0 {
                target
            } else {
                let e = f * f;
                // Live frame → the track's unrotated frame, then ease
                // angle + radius and re-apply the shape.
                let (rs, rc) = rot.sin_cos();
                let lx = lerp_from.x - sun.x;
                let ly = lerp_from.y - sun.y;
                let fx = lx * rc + ly * rs;
                let fy = ly * rc - lx * rs;
                let from_phi = fy.atan2(fx);
                let from_r = fx.hypot(fy);
                let dphi = (theta - from_phi + std::f64::consts::PI)
                    .rem_euclid(std::f64::consts::TAU)
                    - std::f64::consts::PI;
                let phi = from_phi + dphi * e;
                let r = from_r + (r_now - from_r) * e;
                let (ex, ey) = if ring_r.is_some() {
                    circle_offset(r, phi, rot)
                } else {
                    oval_offset(r, phi, rot)
                };
                NSPoint {
                    x: sun.x + ex,
                    y: sun.y + ey,
                }
            };
            // Collider LAW (not a window, never drawn): vs every expanded
            // planet on this screen the marble CENTER stays ≥ live radius
            // + MARBLE_RADIUS from the planet center. A point inside is
            // pushed out along the bearing from the planet — marbles go
            // AROUND the disc, never through. Catches join leftovers and
            // bary clips; the 3-width ring never triggers it.
            for (_, ec, esid, er) in &expanded {
                if esid != screen_id {
                    continue;
                }
                let min_d = *er + MARBLE_RADIUS;
                let dx = pos.x - ec.x;
                let dy = pos.y - ec.y;
                let d = dx.hypot(dy);
                if d < min_d {
                    let (ux, uy) = if d > 1e-6 { (dx / d, dy / d) } else { (1.0, 0.0) };
                    pos = NSPoint {
                        x: ec.x + ux * min_d,
                        y: ec.y + uy * min_d,
                    };
                }
            }
            let panel = planet.panel as *mut Object;
            unsafe {
                let frame: NSRect = msg_send![panel, frame];
                let _: () = msg_send![panel,
                    setFrameOrigin: NSPoint {
                        x: pos.x - frame.size.width / 2.0,
                        y: pos.y - frame.size.height / 2.0,
                    }];
            }
            // Z-law: boolean front/back vs THIS marble's own host only.
            // sin θ with an equator deadband so the bit never chatters.
            // orderWindow:relativeTo: fires only when the bit flips (or
            // on join, to establish stacking vs the new host) — never a
            // per-tick group restack.
            if let Some((host_panel, _)) = host_info {
                let s = theta.sin();
                let prev = planet.orbit.as_ref().map(|o| o.front).unwrap_or(true);
                let mut front = prev;
                if s > FRONT_DEADBAND {
                    front = true;
                } else if s < -FRONT_DEADBAND {
                    front = false;
                }
                if just_joined || front != prev {
                    let host = host_panel as *mut Object;
                    unsafe {
                        let win: i64 = msg_send![host, windowNumber];
                        // NSWindowAbove = 1, NSWindowBelow = -1.
                        let mode: i64 = if front { 1 } else { -1 };
                        let _: () = msg_send![panel, orderWindow: mode relativeTo: win];
                    }
                    if let Some(o) = planet.orbit.as_mut() {
                        o.front = front;
                    }
                }
            }
            // Live position in memory only — screen-local flipped, same
            // as persist_position. The orbit path NEVER dirties
            // worlds.json; pin/drop/toggle/quit persist directly.
            if let Some((_, sframe)) = all.iter().find(|(id, _)| id == screen_id) {
                if let Some(w) = h.worlds.get_mut(pid) {
                    w.x = pos.x - sframe.origin.x;
                    w.y = sframe.origin.y + sframe.size.height - pos.y;
                }
            }
        }
    }
    // Drop barycenter homes whose cluster dissolved (isolation /
    // recapture under a new key).
    h.clusters.retain(|k, _| live_bary_keys.iter().any(|lk| lk == k));
}

/// Screen-space offset on the mild oval at angle θ with semi-major r:
/// the minor axis is OVAL_RATIO · r and the whole oval is rotated in the
/// plane by the sun's hashed orientation. Constant θ speed on an
/// almost-round oval reads as even along the path. Bary tracks only —
/// hosted tracks ride the live-radius circle below (a 0.88 squash would
/// pull the ring onto the face).
fn oval_offset(r: f64, theta: f64, rot: f64) -> (f64, f64) {
    let (s, c) = theta.sin_cos();
    let ex = r * c;
    let ey = OVAL_RATIO * r * s;
    let (rs, rc) = rot.sin_cos();
    (ex * rc - ey * rs, ex * rs + ey * rc)
}

/// RIM, standoff cut to two widths in
/// the hosted ring radius — a CIRCLE around the
/// planet center at the LIVE (scroll-zoomed) radius + two
/// marble-widths + the marble radius. Bigger chip → bigger ring, always
/// ~2 widths off the live edge. Constant r: no accordion, no back tuck.
fn host_ring_r(live_radius: f64) -> f64 {
    live_radius + 2.0 * (2.0 * MARBLE_RADIUS) + MARBLE_RADIUS
}

/// screen-space offset on the hosted CIRCLE at
/// angle θ — constant r around the PLANET center, rotated in the plane
/// by the sun's hashed orientation (front/back is the plane's sin θ,
/// not screen-down; the same θ drives the front/back boolean). No oval
/// squash, drawn scale stays 1.0.
fn circle_offset(r: f64, theta: f64, rot: f64) -> (f64, f64) {
    let (s, c) = theta.sin_cos();
    let (rs, rc) = rot.sin_cos();
    (r * (c * rc - s * rs), r * (c * rs + s * rc))
}

/// FNV-1a over the key: stable hashes for track/orientation picks, no
/// rand, same input → same track every launch.
fn fnv1a(key: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// one marble's own track, FNV-1a over the
/// marble's id — radius ∈ r_min–r_max (bary tracks only, 70–140pt;
/// hosted tracks take the r(θ) halo law instead), period ∈ 90–140s,
/// independent phase0 and direction. Every marble on a sun gets a
/// DIFFERENT ring: never opposite ends of a stick, never one shared θ.
#[derive(Clone, Copy)]
struct TrackParams {
    r: f64,
    period: f64,
    phase0: f64,
    dir: f64,
}

fn track_params(id: &str, r_min: f64, r_max: f64) -> TrackParams {
    let hash = fnv1a(id);
    let pick = |shift: u32| ((hash >> shift) & 0xffff) as f64 / 65535.0;
    TrackParams {
        r: r_min + (r_max - r_min) * pick(0),
        period: TRACK_T_MIN + (TRACK_T_MAX - TRACK_T_MIN) * pick(16),
        phase0: std::f64::consts::TAU * pick(32),
        dir: if hash & 1 == 1 { 1.0 } else { -1.0 },
    }
}

/// One hashed in-plane rotation per SUN (host planet id or barycenter
/// key) — the whole solar system shares an orientation, marbles don't
/// pick their own planes.
fn sun_rot(sun_key: &str) -> f64 {
    std::f64::consts::TAU * ((fnv1a(sun_key) & 0xffff) as f64 / 65535.0)
}

/// motion off (toggle or Reduce Motion) →
/// marbles sit and drift state drops, so a re-enable re-seeds from the
/// live frame. Positions are untouched — worlds.json keeps the live
/// x/y either way.
fn orbit_restore(h: &mut Habitat) {
    if h.clusters.is_empty()
        && h.planets.values().all(|p| {
            !p.is_marble || (p.orbit.is_none() && (p.marble_scale - 1.0).abs() <= 0.001)
        })
    {
        return;
    }
    h.clusters.clear();
    for planet in h.planets.values_mut() {
        if !planet.is_marble {
            continue;
        }
        planet.orbit = None;
        if (planet.marble_scale - 1.0).abs() > 0.001 {
            planet.marble_scale = 1.0;
            unsafe {
                let view = planet.view as *mut Object;
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
        }
    }
}

/// Dynamic click-through: the window server routes events by frame, not pixel
/// alpha, so panels opt out of mouse events unless the cursor is over the
/// planet disc or a sprite/dot. Also folds worlds on missing displays onto
/// the primary and tracks the Reduce Motion setting.
fn cursor_tick() {
    let mut h = lock();
    // Boombox: AVAudioPlayer loops while on. Zoom is the fader — set
    // volume in place, never restart the file. Off / unplayable still
    // stop. Runs even mid-drag so music never gaps while moving a planet.
    let audio_state: HashMap<String, (String, f32, bool)> = h
        .worlds
        .iter()
        .map(|(id, w)| {
            (
                id.clone(),
                (w.music.clone(), w.boombox_volume, w.boombox_muted),
            )
        })
        .collect();
    let reduce = h.reduce_motion;
    // Choose-music panel up: no start until it closes (beachball guard).
    let picker_up = audio::MUSIC_PICKER_OPEN.load(Ordering::SeqCst);
    let mut failed: Vec<String> = Vec::new();
    if !picker_up {
        for (project_id, planet) in h.planets.iter_mut() {
            if !planet.boombox {
                continue;
            }
            let (music, remembered, muted) = audio_state
                .get(project_id)
                .cloned()
                .unwrap_or_else(|| (String::new(), 1.0, false));
            // HUD speaker hard mute: the kept player fades to 0 — never
            // the Off end's stop/release, so unmute continues mid-track.
            let live = if muted {
                0.0
            } else {
                audio::live_volume(remembered, planet.radius, reduce)
            };
            if planet.audio.is_some() {
                // Same player: fade even at 0 (marble mute, keep the
                // playhead so zooming back continues the track).
                audio::fade_audio(planet, live);
            } else if live > 0.0 {
                if !audio::start_audio(planet, &music, live) {
                    failed.push(project_id.clone());
                    set_caption(planet, "Choose music…".into());
                }
            }
        }
    }
    for id in failed {
        if let Some(world) = h.worlds.get_mut(&id) {
            world.music.clear();
            h.worlds_dirty = true;
        }
    }
    // Scroll-zoom / menu mutations persist through here (10Hz), never per
    // scroll event.
    if h.worlds_dirty {
        h.worlds_dirty = false;
        save_worlds(&h);
    }
    // HUD self-heal: the box left the world (Remove, a sync, collapse,
    // teardown) → the HUD dies with it, whichever door did it.
    if h.boombox_hud.is_some() {
        let gone = {
            let pid = h.boombox_hud.as_deref().unwrap_or_default();
            h.planets
                .get(pid)
                .map_or(true, |p| p.is_marble || boombox_hud::rects(p).is_none())
        };
        if gone {
            boombox_hud::close(&mut h);
        }
    }
    // Locate stroke expiry (GO-ORBITTY-RAIL-ROW-MENU): 10Hz sweep, so the
    // ring draws out even with Reduce Motion on (anim_tick is parked then).
    if h
        .locate
        .as_ref()
        .is_some_and(|(_, at)| at.elapsed() >= LOCATE_TTL)
    {
        let id = h.locate.take().map(|(id, _)| id).unwrap_or_default();
        for p in h.planets.values() {
            if p.walkers.iter().any(|b| b.entry_id == id) {
                unsafe {
                    let view = p.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
        }
    }
    if let Some((id, at)) = h.locate_planet.clone() {
        if at.elapsed() < PLANET_LOCATE_TTL {
            if let Some(p) = h.planets.get(&id) {
                unsafe {
                    let view = p.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
        } else {
            h.locate_planet = None;
            if let Some(p) = h.planets.get(&id) {
                unsafe {
                    let view = p.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
        }
    }
    if h.drag.is_some() {
        return; // a drag holds interactivity even if the cursor leaves the disc
    }
    unsafe {
        let ws: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let reduce: bool = msg_send![ws, accessibilityDisplayShouldReduceMotion];
        h.reduce_motion = reduce;

        let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        let all = screens();
        let primary = all.first().map(|(id, _)| id.clone());
        let hud_open = h.boombox_hud.clone();
        let Habitat { planets, worlds, .. } = &mut *h;
        for (project_id, planet) in planets.iter_mut() {
            let panel = planet.panel as *mut Object;
            // Display unplugged/rearranged: fold onto the primary screen.
            if let Some(state) = worlds.get_mut(project_id) {
                if !all.iter().any(|(id, _)| id == &state.screen_id) {
                    if let Some(p) = &primary {
                        state.screen_id = p.clone();
                        let side = (planet.radius + VIEW_PADDING) * 2.0;
                        let frame = panel_frame(state, side);
                        panel_set_frame(panel, frame, true);
                    }
                }
            }
            let frame: NSRect = msg_send![panel, frame];
            let local = NSPoint {
                x: mouse.x - frame.origin.x,
                y: frame.max_y() - mouse.y,
            };
            let over =
                contains_interactive(planet, local, hud_open.as_deref() == Some(project_id));
            let _: () = msg_send![panel, setIgnoresMouseEvents: !over];
            // marble hover names the planet. Done
            // here in the cursor watcher — the panel ignores mouse events
            // off-disc, so a tracking area would never see the enter. The
            // draw falls back to the name only when no click caption is
            // live, so placed/walker/boombox pills are never stomped.
            let hover = planet.is_marble && over_disc(planet, local);
            if hover != planet.hover_name {
                planet.hover_name = hover;
                let view = planet.view as *mut Object;
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
            // Caption expiry (10Hz here so Reduce Motion can't pin it on).
            if planet
                .caption
                .as_ref()
                .is_some_and(|(_, at)| at.elapsed() >= CAPTION_TTL)
            {
                planet.caption = None;
                let view = planet.view as *mut Object;
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
        }
    }
}

/// Persist live world x/y on quit — the orbit path never dirties, so
/// quitting without this would lose the drifted positions. Also the
/// habitat stop: the CVDisplayLink motion clock is stopped + released.
pub fn persist_now() {
    display_link_stop();
    let mut h = lock();
    // Tray Quit must end the song: stop + release every player so nothing
    // keeps ringing after the habitat is gone.
    for planet in h.planets.values_mut() {
        audio::stop_audio(planet);
    }
    save_worlds(&h);
}

