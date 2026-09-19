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
    app_handle, app_handle_cell, default_state, emit_board_checkin, layout_beings,
    layout_decor, lock, now_epoch, panel_frame, panel_set_frame, panic_msg,
    save_worlds, screens, set_caption, view_to_disc, DecorKind, DecorPoint, Drag,
    Habitat, NSPoint, NSRect, CAPTION_TTL, DRIFT_PIN_SECS, LOCATE_TTL,
    PLANET_LOCATE_TTL, VIEW_PADDING
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
        super::marble_motion::drift_pass(&mut h);
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

// Walker anim → anim.rs
fn anim_tick() {
    super::anim::anim_tick();
}

// Marble drift_pass / orbit_restore → marble_motion.rs

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

