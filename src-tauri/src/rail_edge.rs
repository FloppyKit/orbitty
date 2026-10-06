//! Native cursor edge watcher. The parked DOM strip cannot reveal the rail
//! when another app covers it or the window is inactive, so a background
//! poller reads the global cursor and calls `rail_stage_apply(true)`.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Instant;

pub const EDGE_REVEAL_PX: f64 = 6.0;
pub const EDGE_DWELL_MS: u64 = 120;
pub const EDGE_REARM_PX: f64 = 48.0;
const POLL_FAST_MS: u64 = 33;
const POLL_IDLE_MS: u64 = 250;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gate {
    Open,
    Rearm,
    LeaveBand,
}

#[derive(Clone, Copy, Debug)]
pub struct EdgeWatch {
    armed: bool,
    dwell_since: Option<u64>,
    gate: Gate,
}

impl EdgeWatch {
    pub fn new() -> Self {
        Self {
            armed: false,
            dwell_since: None,
            gate: Gate::LeaveBand,
        }
    }

    pub fn armed(&self) -> bool {
        self.armed
    }
}

impl Default for EdgeWatch {
    fn default() -> Self {
        Self::new()
    }
}

/// AppKit bottom-left Y → top-left Y against the primary screen height.
/// Same flip as `drawers::screen_visible_frames` / `appkit_place_points`.
pub fn flip_appkit_y(y_bottom: f64, primary_h: f64) -> f64 {
    primary_h - y_bottom
}

/// A bottom-left AppKit rect → top-left points. `y_bottom` is the rect origin.
pub fn flip_appkit_rect(x: f64, y_bottom: f64, w: f64, h: f64, primary_h: f64) -> ScreenRect {
    ScreenRect {
        x,
        y: flip_appkit_y(y_bottom + h, primary_h),
        w,
        h,
    }
}

fn norm_edge(edge: &str) -> &'static str {
    match edge {
        "right" | "top" | "bottom" => {
            if edge == "right" {
                "right"
            } else if edge == "top" {
                "top"
            } else {
                "bottom"
            }
        }
        _ => "left",
    }
}

fn on_screen(cursor: (f64, f64), screen: ScreenRect) -> bool {
    cursor.0 >= screen.x
        && cursor.0 <= screen.x + screen.w
        && cursor.1 >= screen.y
        && cursor.1 <= screen.y + screen.h
}

/// Distance inward from the configured screen-frame edge. `None` when the
/// cursor is not on that display.
pub fn edge_distance(cursor: (f64, f64), screen: ScreenRect, edge: &str) -> Option<f64> {
    if !on_screen(cursor, screen) {
        return None;
    }
    Some(match norm_edge(edge) {
        "right" => (screen.x + screen.w) - cursor.0,
        "top" => cursor.1 - screen.y,
        "bottom" => (screen.y + screen.h) - cursor.1,
        _ => cursor.0 - screen.x,
    })
}

fn in_corner(cursor: (f64, f64), screen: ScreenRect, edge: &str) -> bool {
    let gap = crate::drawers::EDGE_CORNER_GAP;
    match norm_edge(edge) {
        "top" | "bottom" => {
            let along = cursor.0 - screen.x;
            along < gap || screen.w - along < gap
        }
        _ => {
            let along = cursor.1 - screen.y;
            along < gap || screen.h - along < gap
        }
    }
}

fn in_band(cursor: (f64, f64), screen: ScreenRect, edge: &str) -> bool {
    match edge_distance(cursor, screen, edge) {
        Some(d) if d >= 0.0 && d <= EDGE_REVEAL_PX => !in_corner(cursor, screen, edge),
        _ => false,
    }
}

pub fn edge_watch_parked(s: &mut EdgeWatch) {
    s.armed = false;
    s.dwell_since = None;
    s.gate = Gate::LeaveBand;
}

pub fn edge_watch_revealed(s: &mut EdgeWatch) {
    s.armed = false;
    s.dwell_since = None;
    s.gate = Gate::Rearm;
}

/// True once, when the cursor has dwelled in the reveal band.
pub fn edge_watch_step(
    s: &mut EdgeWatch,
    cursor: (f64, f64),
    screen: ScreenRect,
    edge: &str,
    button_down: bool,
    now_ms: u64,
) -> bool {
    if button_down {
        s.dwell_since = None;
        return false;
    }
    let band = in_band(cursor, screen, edge);
    let dist = edge_distance(cursor, screen, edge);
    if s.gate == Gate::LeaveBand {
        if !band {
            s.gate = Gate::Open;
            s.armed = true;
            s.dwell_since = None;
        }
        return false;
    }
    if s.gate == Gate::Rearm {
        let far = match dist {
            Some(d) => d >= EDGE_REARM_PX,
            None => true,
        };
        if !far {
            s.dwell_since = None;
            return false;
        }
        s.gate = Gate::Open;
        s.armed = true;
        s.dwell_since = None;
    }
    if !s.armed || !band {
        if !band {
            s.dwell_since = None;
        }
        return false;
    }
    match s.dwell_since {
        None => {
            s.dwell_since = Some(now_ms);
            false
        }
        Some(t) if now_ms.saturating_sub(t) >= EDGE_DWELL_MS => {
            s.armed = false;
            s.dwell_since = None;
            s.gate = Gate::Rearm;
            true
        }
        Some(_) => false,
    }
}

#[derive(Clone, Copy)]
struct DisplaySnap {
    frame: ScreenRect,
    visible: ScreenRect,
}

static RUNNING: AtomicBool = AtomicBool::new(false);
static POLLING: AtomicBool = AtomicBool::new(false);
static ARMED: AtomicBool = AtomicBool::new(false);
static STAGE: AtomicBool = AtomicBool::new(false);
static VISIBLE: AtomicBool = AtomicBool::new(true);
static EDGE_CODE: AtomicU8 = AtomicU8::new(0);
/// 0 idle, 1 park, 2 reveal. The poller applies this to its `EdgeWatch`.
static SYNC: AtomicU8 = AtomicU8::new(0);
static DISPLAY: Mutex<Option<DisplaySnap>> = Mutex::new(None);

fn edge_code(edge: &str) -> u8 {
    match norm_edge(edge) {
        "right" => 1,
        "top" => 2,
        "bottom" => 3,
        _ => 0,
    }
}

fn edge_name(code: u8) -> &'static str {
    match code {
        1 => "right",
        2 => "top",
        3 => "bottom",
        _ => "left",
    }
}

pub fn note_display(edge: &str, fx: f64, fy: f64, fw: f64, fh: f64, vx: f64, vy: f64, vw: f64, vh: f64) {
    EDGE_CODE.store(edge_code(edge), Ordering::Relaxed);
    STAGE.store(true, Ordering::Relaxed);
    if let Ok(mut slot) = DISPLAY.lock() {
        *slot = Some(DisplaySnap {
            frame: ScreenRect {
                x: fx,
                y: fy,
                w: fw,
                h: fh,
            },
            visible: ScreenRect {
                x: vx,
                y: vy,
                w: vw,
                h: vh,
            },
        });
    }
}

pub fn sync_watch(revealed: bool) {
    SYNC.store(if revealed { 2 } else { 1 }, Ordering::Relaxed);
}

fn apply_sync(watch: &mut EdgeWatch) {
    match SYNC.swap(0, Ordering::Relaxed) {
        1 => edge_watch_parked(watch),
        2 => edge_watch_revealed(watch),
        _ => {}
    }
}

pub fn status_json(app: &tauri::AppHandle) -> Value {
    let loaded = crate::state::load_state(app).ok();
    let edge = loaded
        .as_ref()
        .map(|s| s.rail_edge.clone())
        .unwrap_or_else(|| edge_name(EDGE_CODE.load(Ordering::Relaxed)).to_string());
    let auto = loaded
        .as_ref()
        .map(|s| s.rail_auto_hide_on_blur)
        .unwrap_or(true);
    let display = DISPLAY.lock().ok().and_then(|g| *g).map(|d| {
        json!({
            "x": d.frame.x,
            "y": d.frame.y,
            "w": d.frame.w,
            "h": d.frame.h,
            "visible": {
                "x": d.visible.x,
                "y": d.visible.y,
                "w": d.visible.w,
                "h": d.visible.h,
            }
        })
    });
    json!({
        "revealed": crate::drawers::rail_stage_is_revealed(),
        "edge": edge,
        "autoHideOnBlur": auto,
        "watcher": {
            "running": RUNNING.load(Ordering::Relaxed),
            "polling": POLLING.load(Ordering::Relaxed),
            "armed": ARMED.load(Ordering::Relaxed),
        },
        "display": display,
    })
}

#[cfg(target_os = "macos")]
mod poll {
    #![allow(unexpected_cfgs)]

    use super::{
        apply_sync, edge_name, edge_watch_step, now_ms, ARMED, DISPLAY, EDGE_CODE, POLL_FAST_MS,
        POLL_IDLE_MS, POLLING, RUNNING, STAGE, VISIBLE,
    };
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use tauri::Manager;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreate(source: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn CGEventGetLocation(event: *mut std::ffi::c_void) -> CGPoint;
        fn CGEventSourceButtonState(state_id: i32, button: u32) -> bool;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const std::ffi::c_void);
    }

    fn cursor_xy() -> Option<(f64, f64)> {
        unsafe {
            let ev = CGEventCreate(std::ptr::null_mut());
            if ev.is_null() {
                return None;
            }
            let p = CGEventGetLocation(ev);
            CFRelease(ev);
            Some((p.x, p.y))
        }
    }

    fn button_down() -> bool {
        unsafe {
            // kCGEventSourceStateHIDSystemState = 1. Left, right, other.
            CGEventSourceButtonState(1, 0)
                || CGEventSourceButtonState(1, 1)
                || CGEventSourceButtonState(1, 2)
        }
    }

    pub fn capture_primary() -> Option<(super::ScreenRect, super::ScreenRect)> {
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};
        #[repr(C)]
        struct NSPoint {
            x: f64,
            y: f64,
        }
        #[repr(C)]
        struct NSSize {
            width: f64,
            height: f64,
        }
        #[repr(C)]
        struct NSRect {
            origin: NSPoint,
            size: NSSize,
        }
        unsafe {
            let screens: *mut Object = msg_send![class!(NSScreen), screens];
            if screens.is_null() {
                return None;
            }
            let count: usize = msg_send![screens, count];
            if count == 0 {
                return None;
            }
            let s0: *mut Object = msg_send![screens, objectAtIndex: 0usize];
            let primary: NSRect = msg_send![s0, frame];
            let vis: NSRect = msg_send![s0, visibleFrame];
            let primary_h = primary.size.height;
            let full = super::flip_appkit_rect(
                primary.origin.x,
                primary.origin.y,
                primary.size.width,
                primary.size.height,
                primary_h,
            );
            let visible = super::flip_appkit_rect(
                vis.origin.x,
                vis.origin.y,
                vis.size.width,
                vis.size.height,
                primary_h,
            );
            Some((full, visible))
        }
    }

    fn refresh_visible(app: &tauri::AppHandle) {
        let app = app.clone();
        let probe = app.clone();
        let _ = app.run_on_main_thread(move || {
            let vis = probe
                .get_webview_window("main")
                .and_then(|w| w.is_visible().ok())
                .unwrap_or(false);
            VISIBLE.store(vis, Ordering::Relaxed);
        });
    }

    pub fn spawn(app: tauri::AppHandle) {
        if RUNNING.swap(true, Ordering::Relaxed) {
            return;
        }
        let _ = std::thread::Builder::new()
            .name("orbitty-edge".into())
            .spawn(move || {
                let mut watch = super::EdgeWatch::new();
                let mut tick: u32 = 0;
                refresh_visible(&app);
                loop {
                    tick = tick.wrapping_add(1);
                    if tick % 8 == 0 {
                        refresh_visible(&app);
                    }
                    apply_sync(&mut watch);
                    let snap = DISPLAY.lock().ok().and_then(|g| *g);
                    let fast = !crate::drawers::rail_stage_is_revealed()
                        && STAGE.load(Ordering::Relaxed)
                        && VISIBLE.load(Ordering::Relaxed)
                        && snap.is_some();
                    POLLING.store(fast, Ordering::Relaxed);
                    ARMED.store(watch.armed(), Ordering::Relaxed);
                    if fast {
                        if let (Some(cursor), Some(screen)) = (cursor_xy(), snap) {
                            let edge = edge_name(EDGE_CODE.load(Ordering::Relaxed));
                            if edge_watch_step(
                                &mut watch,
                                cursor,
                                screen.frame,
                                edge,
                                button_down(),
                                now_ms(),
                            ) {
                                let hop = app.clone();
                                let _ = app.run_on_main_thread(move || {
                                    crate::drawers::rail_stage_apply_edge(&hop);
                                });
                            }
                            ARMED.store(watch.armed(), Ordering::Relaxed);
                        }
                        std::thread::sleep(Duration::from_millis(POLL_FAST_MS));
                    } else {
                        std::thread::sleep(Duration::from_millis(POLL_IDLE_MS));
                    }
                }
            });
    }
}

fn now_ms() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis() as u64
}

pub fn capture_primary() -> Option<(ScreenRect, ScreenRect)> {
    #[cfg(target_os = "macos")]
    {
        poll::capture_primary()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

pub fn start(app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    poll::spawn(app);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        RUNNING.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> ScreenRect {
        ScreenRect {
            x: 0.0,
            y: 0.0,
            w: 900.0,
            h: 1600.0,
        }
    }

    fn arm(s: &mut EdgeWatch, scr: ScreenRect, edge: &str) {
        let outside = match edge {
            "right" => (scr.x + scr.w - 80.0, scr.y + scr.h / 2.0),
            "top" => (scr.x + scr.w / 2.0, scr.y + 80.0),
            "bottom" => (scr.x + scr.w / 2.0, scr.y + scr.h - 80.0),
            _ => (scr.x + 80.0, scr.y + scr.h / 2.0),
        };
        assert!(!edge_watch_step(s, outside, scr, edge, false, 0));
        assert!(s.armed());
    }

    fn band_point(scr: ScreenRect, edge: &str) -> (f64, f64) {
        let mid_x = scr.x + scr.w / 2.0;
        let mid_y = scr.y + scr.h / 2.0;
        match edge {
            "right" => (scr.x + scr.w - 3.0, mid_y),
            "top" => (mid_x, scr.y + 3.0),
            "bottom" => (mid_x, scr.y + scr.h - 3.0),
            _ => (scr.x + 3.0, mid_y),
        }
    }

    #[test]
    fn edge_band_dwell_reveals() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        arm(&mut s, scr, "left");
        let p = (3.0, 800.0);
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, 1_000));
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, 1_119));
        assert!(edge_watch_step(&mut s, p, scr, "left", false, 1_120));
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, 2_000));
    }

    #[test]
    fn edge_corners_excluded() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        arm(&mut s, scr, "left");
        for y in [scr.y + 10.0, scr.y + 47.0, scr.y + scr.h - 47.0, scr.y + scr.h - 1.0] {
            let mut w = s;
            for t in 0..20 {
                assert!(
                    !edge_watch_step(&mut w, (2.0, y), scr, "left", false, 1_000 + t * 50),
                    "y {y}"
                );
            }
        }
    }

    #[test]
    fn edge_button_down_blocks() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        arm(&mut s, scr, "left");
        let p = (2.0, 800.0);
        for t in 0..40 {
            assert!(!edge_watch_step(&mut s, p, scr, "left", true, t * 50));
        }
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, 3_000));
        assert!(edge_watch_step(&mut s, p, scr, "left", false, 3_120));
    }

    #[test]
    fn edge_rearm_hysteresis() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        arm(&mut s, scr, "left");
        let p = (4.0, 800.0);
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, 0));
        assert!(edge_watch_step(&mut s, p, scr, "left", false, 120));
        let mut t = 200u64;
        let end = t + 2_000;
        let mut near = true;
        while t < end {
            let x = if near { 3.0 } else { 7.0 };
            near = !near;
            assert!(!edge_watch_step(&mut s, (x, 800.0), scr, "left", false, t));
            t += 33;
        }
        // Stay inside the 6px band longer than a dwell. Rearm must hold.
        assert!(!edge_watch_step(&mut s, (3.0, 800.0), scr, "left", false, t));
        assert!(!edge_watch_step(&mut s, (4.0, 800.0), scr, "left", false, t + 200));
        t += 200;
        assert!(!edge_watch_step(&mut s, (80.0, 800.0), scr, "left", false, t));
        t += 30;
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, t));
        assert!(edge_watch_step(&mut s, p, scr, "left", false, t + 120));
    }

    #[test]
    fn edge_after_park_needs_leave() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        arm(&mut s, scr, "left");
        edge_watch_parked(&mut s);
        let p = (2.0, 800.0);
        for t in 0..30 {
            assert!(!edge_watch_step(&mut s, p, scr, "left", false, t * 40));
        }
        assert!(!edge_watch_step(&mut s, (30.0, 800.0), scr, "left", false, 2_000));
        assert!(!edge_watch_step(&mut s, p, scr, "left", false, 2_100));
        assert!(edge_watch_step(&mut s, p, scr, "left", false, 2_220));
    }

    #[test]
    fn edge_all_four_edges() {
        let scr = ScreenRect {
            x: 2820.0,
            y: 40.0,
            w: 1920.0,
            h: 1080.0,
        };
        for edge in ["left", "right", "top", "bottom"] {
            let mut s = EdgeWatch::new();
            arm(&mut s, scr, edge);
            let p = band_point(scr, edge);
            assert!(
                !edge_watch_step(&mut s, p, scr, edge, false, 10),
                "{edge} enter"
            );
            assert!(
                edge_watch_step(&mut s, p, scr, edge, false, 130),
                "{edge} dwell"
            );
        }
    }

    #[test]
    fn edge_other_display_ignored() {
        let rail = ScreenRect {
            x: 2820.0,
            y: 0.0,
            w: 900.0,
            h: 1600.0,
        };
        let mut s = EdgeWatch::new();
        arm(&mut s, rail, "left");
        let other_left = (3.0, 800.0);
        for t in 0..40 {
            assert!(!edge_watch_step(
                &mut s,
                other_left,
                rail,
                "left",
                false,
                t * 50
            ));
        }
    }

    #[test]
    fn edge_flap_sim() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        arm(&mut s, scr, "left");
        let mut reveals = 0;
        let mut t = 100u64;
        for _ in 0..20 {
            if edge_watch_step(&mut s, (2.0, 800.0), scr, "left", false, t) {
                reveals += 1;
            }
            t += 50;
            if edge_watch_step(&mut s, (40.0, 800.0), scr, "left", false, t) {
                reveals += 1;
            }
            t += 50;
        }
        assert_eq!(reveals, 0);
    }

    #[test]
    fn appkit_y_flips_to_top_left() {
        assert_eq!(flip_appkit_y(1600.0, 1600.0), 0.0);
        assert_eq!(flip_appkit_y(0.0, 1600.0), 1600.0);
        let r = flip_appkit_rect(0.0, 79.0, 900.0, 1521.0, 1600.0);
        assert_eq!(r.y, 1600.0 - (79.0 + 1521.0));
        assert_eq!(r.x, 0.0);
        assert_eq!(r.w, 900.0);
        assert_eq!(r.h, 1521.0);
    }
}
