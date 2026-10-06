//! DisplayLink vsync clock + 16ms habitat tick (drag / anim / cursor).
//! Motion posts to the main queue and returns — never waits on UI.
//! drag_tick / cursor_tick live in sibling modules (DIET-HABITAT).

#![allow(unexpected_cfgs)]

use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::AppHandle;

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};

use super::audio;
use super::{app_handle_cell, lock, panic_msg, save_worlds};


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
    glow_repaint(n);
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

/// While a plot should pulse, dirty expanded planets at ~20fps (every third
/// 16ms tick). Reduce motion repaints only when the glow starts or stops.
fn glow_repaint(n: u64) {
    let reduce = lock().reduce_motion;
    let glow = crate::starter::glow_now();
    static LAST: AtomicU8 = AtomicU8::new(0);
    let code = match glow {
        Some(crate::starter::Outpost::Comms) => 1,
        Some(crate::starter::Outpost::Stars) => 2,
        None => 0,
    };
    let prev = LAST.swap(code, Ordering::Relaxed);
    let edge = prev != code;
    let pulse = glow.is_some() && !reduce && n % 3 == 0;
    if !pulse && !edge {
        return;
    }
    let h = lock();
    for planet in h.planets.values() {
        if planet.is_marble || planet.view == 0 {
            continue;
        }
        unsafe {
            let view = planet.view as *mut Object;
            let _: () = msg_send![view, setNeedsDisplay: true];
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
    super::drag_tick::drag_tick();
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
    super::cursor_tick::cursor_tick();
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

