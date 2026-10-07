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

    pub fn dwelling(&self) -> bool {
        self.dwell_since.is_some()
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

/// Parked poll interval. In the reveal band, or while a dwell is running, stay fast.
/// Far from the edge, slow down. `None` distance (cursor on another display) is idle.
pub fn next_poll_ms(dist_to_edge_px: Option<f64>, in_band: bool, dwell_active: bool) -> u64 {
    if in_band || dwell_active {
        return POLL_FAST_MS;
    }
    match dist_to_edge_px {
        Some(d) if d <= 160.0 => 50,
        Some(d) if d <= 480.0 => 120,
        _ => POLL_IDLE_MS,
    }
}

fn axis_overlap(a0: f64, a1: f64, b0: f64, b1: f64) -> bool {
    a0 < b1 && b0 < a1
}

pub fn overlap_area(a: ScreenRect, b: ScreenRect) -> f64 {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.w).min(b.x + b.w);
    let y2 = (a.y + a.h).min(b.y + b.h);
    (x2 - x1).max(0.0) * (y2 - y1).max(0.0)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn on_screen_px(frame: ScreenRect, displays: &[ScreenRect]) -> f64 {
    displays.iter().map(|d| overlap_area(frame, *d)).sum()
}

/// True when another display shares this edge (a parked window must not
/// jump onto that neighbour).
pub fn edge_abuts(edge: &str, display: ScreenRect, others: &[ScreenRect]) -> bool {
    const EPS: f64 = 2.0;
    others.iter().any(|o| {
        let same = (o.x - display.x).abs() < 0.5
            && (o.y - display.y).abs() < 0.5
            && (o.w - display.w).abs() < 0.5
            && (o.h - display.h).abs() < 0.5;
        if same {
            return false;
        }
        match norm_edge(edge) {
            "right" => {
                (o.x - (display.x + display.w)).abs() <= EPS
                    && axis_overlap(display.y, display.y + display.h, o.y, o.y + o.h)
            }
            "top" => {
                ((o.y + o.h) - display.y).abs() <= EPS
                    && axis_overlap(display.x, display.x + display.w, o.x, o.x + o.w)
            }
            "bottom" => {
                (o.y - (display.y + display.h)).abs() <= EPS
                    && axis_overlap(display.x, display.x + display.w, o.x, o.x + o.w)
            }
            _ => {
                ((o.x + o.w) - display.x).abs() <= EPS
                    && axis_overlap(display.y, display.y + display.h, o.y, o.y + o.h)
            }
        }
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParkedPlace {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub alpha: f64,
    pub offscreen: bool,
}

/// Outer edge: the parked window is fully off every display. An abutting
/// edge keeps the 12px sliver frame at alpha 0 so it does not land on the
/// neighbour.
pub fn parked_place(
    edge: &str,
    display: ScreenRect,
    others: &[ScreenRect],
    ww: f64,
    hh: f64,
) -> ParkedPlace {
    if edge_abuts(edge, display, others) {
        let (x, y, w, h) = crate::state::rail_stage_frame(
            edge, false, display.x, display.y, display.w, display.h, ww, hh,
        );
        return ParkedPlace {
            x,
            y,
            w,
            h,
            alpha: 0.0,
            offscreen: false,
        };
    }
    let y_mid = display.y + ((display.h - hh) / 2.0).max(0.0);
    let x_mid = display.x + ((display.w - ww) / 2.0).max(0.0);
    let (x, y) = match norm_edge(edge) {
        "right" => (display.x + display.w, y_mid),
        "top" => (x_mid, display.y - hh),
        "bottom" => (x_mid, display.y + display.h),
        _ => (display.x - ww, y_mid),
    };
    ParkedPlace {
        x,
        y,
        w: ww,
        h: hh,
        alpha: 0.0,
        offscreen: true,
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
static AUTO_HIDE: AtomicBool = AtomicBool::new(true);
static EDGE_CODE: AtomicU8 = AtomicU8::new(0);
/// 0 idle, 1 park, 2 reveal. The poller applies this to its `EdgeWatch`.
static SYNC: AtomicU8 = AtomicU8::new(0);
static DISPLAY: Mutex<Option<DisplaySnap>> = Mutex::new(None);

#[derive(Clone, Debug)]
struct ChromeSnap {
    level: i64,
    alpha: f64,
    frame: ScreenRect,
    app_active: bool,
    is_key: bool,
    occluded: bool,
    last_source: String,
    last_at_ms: u64,
    app_active_after: Option<bool>,
}

fn empty_chrome() -> ChromeSnap {
    ChromeSnap {
        level: 0,
        alpha: 0.0,
        frame: ScreenRect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        },
        app_active: false,
        is_key: false,
        occluded: false,
        last_source: String::new(),
        last_at_ms: 0,
        app_active_after: None,
    }
}

static CHROME: Mutex<Option<ChromeSnap>> = Mutex::new(None);

pub fn note_visible(on: bool) {
    VISIBLE.store(on, Ordering::Relaxed);
}

pub fn note_edge_prefs(edge: &str, auto_hide: bool) {
    EDGE_CODE.store(edge_code(edge), Ordering::Relaxed);
    AUTO_HIDE.store(auto_hide, Ordering::Relaxed);
    STAGE.store(true, Ordering::Relaxed);
}

pub fn cached_edge() -> &'static str {
    edge_name(EDGE_CODE.load(Ordering::Relaxed))
}

pub fn cached_auto_hide() -> bool {
    AUTO_HIDE.load(Ordering::Relaxed)
}

pub fn stage_known() -> bool {
    STAGE.load(Ordering::Relaxed)
}

pub fn note_chrome(
    level: i64,
    alpha: f64,
    frame: ScreenRect,
    app_active: bool,
    is_key: bool,
    occluded: bool,
    source: Option<&str>,
) {
    if let Ok(mut g) = CHROME.lock() {
        let mut snap = g.clone().unwrap_or_else(empty_chrome);
        snap.level = level;
        snap.alpha = alpha;
        snap.frame = frame;
        snap.app_active = app_active;
        snap.is_key = is_key;
        snap.occluded = occluded;
        if let Some(source) = source {
            snap.last_source = source.to_string();
            snap.last_at_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            snap.app_active_after = None;
        }
        *g = Some(snap);
    }
}

pub fn note_active_after(active: bool) {
    if let Ok(mut g) = CHROME.lock() {
        let mut snap = g.clone().unwrap_or_else(empty_chrome);
        snap.app_active_after = Some(active);
        snap.app_active = active;
        *g = Some(snap);
    }
}

pub fn chrome_frame() -> Option<ScreenRect> {
    CHROME.lock().ok().and_then(|g| g.as_ref().map(|s| s.frame)).filter(|f| f.w > 0.0 && f.h > 0.0)
}

pub fn chrome_occluded() -> bool {
    CHROME.lock().ok().and_then(|g| g.as_ref().map(|s| s.occluded)).unwrap_or(false)
}

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
    let chrome = CHROME.lock().ok().and_then(|g| g.clone());
    let frame = chrome.as_ref().map(|c| {
        json!({ "x": c.frame.x, "y": c.frame.y, "w": c.frame.w, "h": c.frame.h })
    });
    let last = chrome.as_ref().map(|c| {
        json!({
            "source": c.last_source,
            "atMs": c.last_at_ms,
            "appActiveAfter": c.app_active_after,
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
        "level": chrome.as_ref().map(|c| c.level).unwrap_or(0),
        "alpha": chrome.as_ref().map(|c| c.alpha).unwrap_or(0.0),
        "frame": frame,
        "appActive": chrome.as_ref().map(|c| c.app_active).unwrap_or(false),
        "isKey": chrome.as_ref().map(|c| c.is_key).unwrap_or(false),
        "occluded": chrome.as_ref().map(|c| c.occluded).unwrap_or(false),
        "occludedBy": occluded_by_json(),
        "lastReveal": last,
    })
}

fn occluded_by_json() -> Value {
    #[cfg(target_os = "macos")]
    {
        poll::occluded_by_json()
    }
    #[cfg(not(target_os = "macos"))]
    {
        json!([])
    }
}

#[cfg(target_os = "macos")]
mod poll {
    #![allow(unexpected_cfgs)]

    use super::{
        apply_sync, chrome_frame, chrome_occluded, edge_distance, edge_name, edge_watch_step,
        in_band, next_poll_ms, now_ms, ARMED, DISPLAY, EDGE_CODE, POLL_IDLE_MS, POLLING, RUNNING,
        STAGE, VISIBLE,
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

    fn point_in(p: (f64, f64), r: super::ScreenRect) -> bool {
        p.0 >= r.x && p.0 <= r.x + r.w && p.1 >= r.y && p.1 <= r.y + r.h
    }

    fn refresh_facts(app: &tauri::AppHandle) {
        let hop = app.clone();
        let _ = app.run_on_main_thread(move || {
            crate::drawers::sample_rail_chrome(&hop, None);
        });
    }

    fn tick_revealed(app: &tauri::AppHandle, away_since: &mut Option<u64>, occ_since: &mut Option<u64>) {
        let now = now_ms();
        let inside = match (cursor_xy(), chrome_frame()) {
            (Some(c), Some(f)) => point_in(c, f),
            _ => false,
        };
        if inside {
            *away_since = None;
        } else if away_since.is_none() {
            *away_since = Some(now);
        }
        let occluded = chrome_occluded();
        if occluded {
            if occ_since.is_none() {
                *occ_since = Some(now);
            }
        } else {
            *occ_since = None;
        }
        let facts = crate::rail_stage::RailFacts {
            app_active: super::CHROME
                .lock()
                .ok()
                .and_then(|g| g.as_ref().map(|s| s.app_active))
                .unwrap_or(false),
            key: super::CHROME
                .lock()
                .ok()
                .and_then(|g| g.as_ref().map(|s| s.is_key))
                .unwrap_or(false),
            occluded,
            cursor_in_frame: inside,
            away_ms: away_since.map(|t| now.saturating_sub(t)).unwrap_or(0),
            occluded_ms: occ_since.map(|t| now.saturating_sub(t)).unwrap_or(0),
            button_down: button_down(),
            auto_hide: super::cached_auto_hide(),
        };
        let (vis, acts) = crate::rail_stage::rail_vis_step(
            crate::rail_stage::RailVis::Revealed,
            crate::rail_stage::RailEvent::Tick,
            &facts,
        );
        if vis == crate::rail_stage::RailVis::Parked
            && crate::rail_stage::plan_has_park(&acts)
        {
            let hop = app.clone();
            let _ = app.run_on_main_thread(move || {
                crate::drawers::rail_stage_perform(
                    &hop,
                    crate::rail_stage::RailVis::Parked,
                    &[
                        crate::rail_stage::RailAct::Park,
                        crate::rail_stage::RailAct::Emit("native"),
                    ],
                );
            });
        }
    }

    pub fn spawn(app: tauri::AppHandle) {
        if RUNNING.swap(true, Ordering::Relaxed) {
            return;
        }
        let _ = std::thread::Builder::new()
            .name("orbitty-edge".into())
            .spawn(move || {
                let mut watch = super::EdgeWatch::new();
                let mut last_vis = 0u64;
                let mut last_facts = 0u64;
                let mut away_since: Option<u64> = None;
                let mut occ_since: Option<u64> = None;
                loop {
                    let now = now_ms();
                    if now.saturating_sub(last_vis) >= 1000 {
                        last_vis = now;
                        refresh_visible(&app);
                    }
                    apply_sync(&mut watch);
                    let snap = DISPLAY.lock().ok().and_then(|g| *g);
                    let revealed = crate::drawers::rail_stage_is_revealed();
                    ARMED.store(watch.armed(), Ordering::Relaxed);
                    if revealed {
                        POLLING.store(false, Ordering::Relaxed);
                        if now.saturating_sub(last_facts) >= 500 {
                            last_facts = now;
                            refresh_facts(&app);
                        }
                        tick_revealed(&app, &mut away_since, &mut occ_since);
                        std::thread::sleep(Duration::from_millis(POLL_IDLE_MS));
                        continue;
                    }
                    away_since = None;
                    occ_since = None;
                    let fast = STAGE.load(Ordering::Relaxed)
                        && VISIBLE.load(Ordering::Relaxed)
                        && snap.is_some();
                    POLLING.store(fast, Ordering::Relaxed);
                    if fast {
                        if let (Some(cursor), Some(screen)) = (cursor_xy(), snap) {
                            let edge = edge_name(EDGE_CODE.load(Ordering::Relaxed));
                            let dist = edge_distance(cursor, screen.frame, edge);
                            let band = in_band(cursor, screen.frame, edge);
                            let buttons = if band { button_down() } else { false };
                            if edge_watch_step(
                                &mut watch,
                                cursor,
                                screen.frame,
                                edge,
                                buttons,
                                now_ms(),
                            ) {
                                let hop = app.clone();
                                let _ = app.run_on_main_thread(move || {
                                    crate::drawers::rail_stage_apply_edge(&hop);
                                });
                            }
                            ARMED.store(watch.armed(), Ordering::Relaxed);
                            let ms = next_poll_ms(dist, band, watch.dwelling());
                            std::thread::sleep(Duration::from_millis(ms));
                        } else {
                            std::thread::sleep(Duration::from_millis(POLL_IDLE_MS));
                        }
                    } else {
                        std::thread::sleep(Duration::from_millis(POLL_IDLE_MS));
                    }
                }
            });
    }

    pub fn all_full_frames() -> Vec<super::ScreenRect> {
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
        let mut out = Vec::new();
        unsafe {
            let screens: *mut Object = msg_send![class!(NSScreen), screens];
            if screens.is_null() {
                return out;
            }
            let count: usize = msg_send![screens, count];
            if count == 0 {
                return out;
            }
            let s0: *mut Object = msg_send![screens, objectAtIndex: 0usize];
            let primary: NSRect = msg_send![s0, frame];
            let primary_h = primary.size.height;
            for i in 0..count {
                let s: *mut Object = msg_send![screens, objectAtIndex: i];
                let fr: NSRect = msg_send![s, frame];
                out.push(super::flip_appkit_rect(
                    fr.origin.x,
                    fr.origin.y,
                    fr.size.width,
                    fr.size.height,
                    primary_h,
                ));
            }
        }
        out
    }

    pub fn occluded_by_json() -> serde_json::Value {
        let Some(frame) = chrome_frame() else {
            return serde_json::json!([]);
        };
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGWindowListCopyWindowInfo(option: u32, relative_to: u32) -> *const std::ffi::c_void;
        }
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFArrayGetCount(array: *const std::ffi::c_void) -> isize;
            fn CFArrayGetValueAtIndex(
                array: *const std::ffi::c_void,
                idx: isize,
            ) -> *const std::ffi::c_void;
            fn CFDictionaryGetValue(
                dict: *const std::ffi::c_void,
                key: *const std::ffi::c_void,
            ) -> *const std::ffi::c_void;
            fn CFNumberGetValue(
                number: *const std::ffi::c_void,
                the_type: i32,
                value: *mut std::ffi::c_void,
            ) -> bool;
            fn CFStringGetCString(
                the_string: *const std::ffi::c_void,
                buffer: *mut std::os::raw::c_char,
                buffer_size: isize,
                encoding: u32,
            ) -> bool;
            fn CFRelease(cf: *const std::ffi::c_void);
        }
        use core_foundation::base::TCFType;
        use core_foundation::string::CFString;
        const OPT: u32 = (1 << 0) | (1 << 4);
        const SINT64: i32 = 4;
        const UTF8: u32 = 0x08000100;
        let our = std::process::id() as i64;
        unsafe {
            let arr = CGWindowListCopyWindowInfo(OPT, 0);
            if arr.is_null() {
                return serde_json::json!([]);
            }
            let count = CFArrayGetCount(arr);
            let key_pid = CFString::new("kCGWindowOwnerPID");
            let key_num = CFString::new("kCGWindowNumber");
            let key_layer = CFString::new("kCGWindowLayer");
            let key_owner = CFString::new("kCGWindowOwnerName");
            let key_bounds = CFString::new("kCGWindowBounds");
            let mut above = true;
            let mut out = Vec::new();
            for i in 0..count {
                let dict = CFArrayGetValueAtIndex(arr, i);
                if dict.is_null() {
                    continue;
                }
                let mut layer: i64 = -1;
                let mut pid: i64 = 0;
                let mut num: i64 = 0;
                let layer_ref = CFDictionaryGetValue(dict, key_layer.as_concrete_TypeRef() as *const _);
                let pid_ref = CFDictionaryGetValue(dict, key_pid.as_concrete_TypeRef() as *const _);
                let num_ref = CFDictionaryGetValue(dict, key_num.as_concrete_TypeRef() as *const _);
                if !layer_ref.is_null() {
                    let _ = CFNumberGetValue(layer_ref, SINT64, &mut layer as *mut i64 as *mut _);
                }
                if !pid_ref.is_null() {
                    let _ = CFNumberGetValue(pid_ref, SINT64, &mut pid as *mut i64 as *mut _);
                }
                if !num_ref.is_null() {
                    let _ = CFNumberGetValue(num_ref, SINT64, &mut num as *mut i64 as *mut _);
                }
                if pid == our {
                    above = false;
                    continue;
                }
                if !above || layer != 0 {
                    continue;
                }
                let bounds = CFDictionaryGetValue(dict, key_bounds.as_concrete_TypeRef() as *const _);
                if bounds.is_null() {
                    continue;
                }
                let kx = CFString::new("X");
                let ky = CFString::new("Y");
                let kw = CFString::new("Width");
                let kh = CFString::new("Height");
                let mut x = 0f64;
                let mut y = 0f64;
                let mut w = 0f64;
                let mut h = 0f64;
                let xr = CFDictionaryGetValue(bounds, kx.as_concrete_TypeRef() as *const _);
                let yr = CFDictionaryGetValue(bounds, ky.as_concrete_TypeRef() as *const _);
                let wr = CFDictionaryGetValue(bounds, kw.as_concrete_TypeRef() as *const _);
                let hr = CFDictionaryGetValue(bounds, kh.as_concrete_TypeRef() as *const _);
                if xr.is_null() || yr.is_null() || wr.is_null() || hr.is_null() {
                    continue;
                }
                // CG window bounds are top-left already.
                const DOUBLE: i32 = 13;
                let _ = CFNumberGetValue(xr, DOUBLE, &mut x as *mut f64 as *mut _);
                let _ = CFNumberGetValue(yr, DOUBLE, &mut y as *mut f64 as *mut _);
                let _ = CFNumberGetValue(wr, DOUBLE, &mut w as *mut f64 as *mut _);
                let _ = CFNumberGetValue(hr, DOUBLE, &mut h as *mut f64 as *mut _);
                let rect = super::ScreenRect { x, y, w, h };
                if super::overlap_area(rect, frame) <= 1.0 {
                    continue;
                }
                let owner_ref = CFDictionaryGetValue(dict, key_owner.as_concrete_TypeRef() as *const _);
                let mut buf = [0u8; 256];
                let mut owner = String::new();
                if !owner_ref.is_null()
                    && CFStringGetCString(
                        owner_ref,
                        buf.as_mut_ptr() as *mut _,
                        buf.len() as isize,
                        UTF8,
                    )
                {
                    owner = std::ffi::CStr::from_ptr(buf.as_ptr() as *const _)
                        .to_string_lossy()
                        .into_owned();
                }
                if owner.to_lowercase().contains("orbitty") {
                    continue;
                }
                out.push(serde_json::json!({ "owner": owner, "windowId": num }));
                if out.len() >= 8 {
                    break;
                }
            }
            CFRelease(arr);
            serde_json::json!(out)
        }
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

pub fn all_full_frames() -> Vec<ScreenRect> {
    #[cfg(target_os = "macos")]
    {
        poll::all_full_frames()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

pub fn start(app: tauri::AppHandle) {
    static WATCH_OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let off = *WATCH_OFF.get_or_init(|| std::env::var("ORBITTY_EDGE_WATCH").ok().as_deref() == Some("0"));
    if off {
        RUNNING.store(false, Ordering::Relaxed);
        return;
    }
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

    #[test]
    fn next_poll_far_is_slow() {
        assert_eq!(next_poll_ms(Some(900.0), false, false), 250);
        assert_eq!(next_poll_ms(None, false, false), 250);
        assert_eq!(next_poll_ms(Some(200.0), false, false), 120);
    }

    #[test]
    fn next_poll_band_is_fast() {
        assert_eq!(next_poll_ms(Some(2.0), true, false), 33);
        assert_eq!(next_poll_ms(Some(40.0), false, true), 33);
        assert_eq!(next_poll_ms(Some(80.0), false, false), 50);
    }

    #[test]
    fn fast_flick_latency_bounded() {
        let scr = screen();
        let mut s = EdgeWatch::new();
        assert!(!edge_watch_step(&mut s, (400.0, 800.0), scr, "left", false, 0));
        assert!(s.armed());
        let mut x = 450.0_f64;
        let mut t = 0u64;
        let mut arrived: Option<u64> = None;
        let mut revealed: Option<u64> = None;
        while t < 8_000 && revealed.is_none() {
            let sample = if x <= EDGE_REVEAL_PX { 3.0 } else { x };
            if sample <= EDGE_REVEAL_PX && arrived.is_none() {
                arrived = Some(t);
            }
            let dist = edge_distance((sample, 800.0), scr, "left");
            let band = in_band((sample, 800.0), scr, "left");
            if edge_watch_step(&mut s, (sample, 800.0), scr, "left", false, t) {
                revealed = Some(t);
                break;
            }
            let ms = next_poll_ms(dist, band, s.dwelling());
            if x > EDGE_REVEAL_PX {
                let next = x - 3000.0 * (ms as f64) / 1000.0;
                if x > EDGE_REVEAL_PX && next <= EDGE_REVEAL_PX {
                    let frac = (x - EDGE_REVEAL_PX) / (x - next);
                    arrived = Some(t + ((ms as f64) * frac) as u64);
                }
                x = next;
            }
            t += ms;
        }
        let arrived = arrived.expect("cursor reached the band");
        let revealed = revealed.expect("dwell revealed");
        assert!(
            revealed.saturating_sub(arrived) <= EDGE_DWELL_MS + 150,
            "latency {} arrived {arrived} revealed {revealed}",
            revealed.saturating_sub(arrived)
        );
    }

    #[test]
    fn parked_offscreen_frame_outer_edges() {
        let primary = ScreenRect {
            x: 0.0,
            y: 0.0,
            w: 900.0,
            h: 1600.0,
        };
        let alone = [primary];
        for edge in ["left", "right", "top", "bottom"] {
            let p = parked_place(edge, primary, &alone, 360.0, 420.0);
            assert_eq!(p.alpha, 0.0, "{edge}");
            assert!(p.offscreen, "{edge}");
            let frame = ScreenRect {
                x: p.x,
                y: p.y,
                w: p.w,
                h: p.h,
            };
            assert_eq!(on_screen_px(frame, &alone), 0.0, "{edge}");
        }
        let neighbor = ScreenRect {
            x: 900.0,
            y: 0.0,
            w: 1920.0,
            h: 1080.0,
        };
        let both = [primary, neighbor];
        let right = parked_place("right", primary, &both, 360.0, 420.0);
        assert_eq!(right.alpha, 0.0);
        assert!(!right.offscreen);
        let (sx, sy, sw, sh) =
            crate::state::rail_stage_frame("right", false, 0.0, 0.0, 900.0, 1600.0, 360.0, 420.0);
        assert_eq!(right.x, sx);
        assert_eq!(right.y, sy);
        assert_eq!(right.w, sw);
        assert_eq!(right.h, sh);
        let left = parked_place("left", primary, &both, 360.0, 420.0);
        assert!(left.offscreen);
        assert_eq!(
            on_screen_px(
                ScreenRect {
                    x: left.x,
                    y: left.y,
                    w: left.w,
                    h: left.h,
                },
                &both
            ),
            0.0
        );
    }
}
