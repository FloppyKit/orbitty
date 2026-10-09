use crate::state;
use tauri::{Emitter, Manager};

use super::place::EDGE_CORNER_GAP;
use super::show::appkit_place_points;
use super::place::live_points_rect;
use super::rt::points_frames;
use super::place::remember_anchor;
use super::rt::screen_visible_frames;

/// Parked strip along the edge, with the hot corners left clear.
/// Unknown edges use the left size.
pub(crate) fn stage_parked_size(edge: &str, vw: f64, vh: f64) -> (f64, f64) {
    match state::normalize_rail_edge(edge) {
        "top" | "bottom" => ((vw - 2.0 * EDGE_CORNER_GAP).max(360.0), 420.0),
        _ => (360.0, (vh - 2.0 * EDGE_CORNER_GAP).max(420.0)),
    }
}

static RAIL_STAGE_REVEALED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
pub(crate) static REVEALED_ANCHOR: std::sync::Mutex<Option<state::CompanionBounds>> =
    std::sync::Mutex::new(None);

pub(crate) fn rail_stage_is_revealed() -> bool {
    RAIL_STAGE_REVEALED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Park or reveal. `revealed` true is Expand (raise, never park). False is a park request.
pub(crate) fn rail_stage_apply(app: &tauri::AppHandle, revealed: bool) {
    let ev = if revealed {
        crate::rail_stage::RailEvent::Expand
    } else {
        crate::rail_stage::RailEvent::ParkRequest
    };
    rail_stage_dispatch(app, ev);
}

/// Edge watcher reveal. Re-raises when already revealed; never parks.
pub(crate) fn rail_stage_apply_edge(app: &tauri::AppHandle) {
    rail_stage_dispatch(app, crate::rail_stage::RailEvent::EdgeReveal);
}

pub(crate) fn rail_stage_set_reason(app: &tauri::AppHandle, revealed: bool, reason: Option<&str>) {
    let ev = match (revealed, reason) {
        (false, Some("raised")) => crate::rail_stage::RailEvent::RaisedOther,
        (false, _) => crate::rail_stage::RailEvent::ParkRequest,
        (true, _) => crate::rail_stage::RailEvent::Expand,
    };
    rail_stage_dispatch(app, ev);
}

pub(crate) fn rail_stage_toggle(app: &tauri::AppHandle) {
    rail_stage_dispatch(app, crate::rail_stage::RailEvent::Hotkey);
}

fn rail_stage_dispatch(app: &tauri::AppHandle, event: crate::rail_stage::RailEvent) {
    let facts = live_rail_facts(app);
    let state = if rail_stage_is_revealed() {
        crate::rail_stage::RailVis::Revealed
    } else {
        crate::rail_stage::RailVis::Parked
    };
    let (next, acts) = crate::rail_stage::rail_vis_step(state, event, &facts);
    rail_stage_perform(app, next, &acts);
}

pub(crate) fn rail_stage_perform(
    app: &tauri::AppHandle,
    next: crate::rail_stage::RailVis,
    acts: &[crate::rail_stage::RailAct],
) {
    let raise = crate::rail_stage::plan_has_raise(acts);
    let park = crate::rail_stage::plan_has_park(acts);
    if !raise && !park {
        return;
    }
    let revealed = next == crate::rail_stage::RailVis::Revealed;
    RAIL_STAGE_REVEALED.store(revealed, std::sync::atomic::Ordering::Relaxed);
    crate::rail_edge::sync_watch(revealed);
    crate::rail_edge::note_visible(true);
    let edge = ensure_edge(app);
    let rect = place_for(app, &edge, revealed);
    let source = acts.iter().find_map(|a| match a {
        crate::rail_stage::RailAct::Emit(s) => Some(*s),
        _ => None,
    });
    if let Some(w) = app.get_webview_window("main") {
        if raise {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_ignore_cursor_events(false);
            stage_alpha_level(&w, 1.0, crate::rail_stage::RAIL_LEVEL_FLOATING);
            if let Some(r) = rect {
                appkit_place_points(&w, &r);
            }
            stage_order_front(&w);
            crate::glass::stage_borderless(&w);
            crate::glass::no_shadow(&w);
            let _ = w.set_focus();
        } else {
            stage_alpha_level(&w, 0.0, crate::rail_stage::RAIL_LEVEL_NORMAL);
            let _ = w.set_ignore_cursor_events(true);
            if let Some(r) = rect {
                appkit_place_points(&w, &r);
            }
            crate::glass::no_shadow(&w);
        }
        sample_rail_chrome(app, if raise { source } else { None });
    }
    let mut payload = serde_json::json!({ "revealed": revealed });
    if let Some(source) = source {
        payload["source"] = serde_json::json!(source);
    }
    let _ = app.emit_to("main", "orbitty:rail-stage", payload);
    if raise {
        let later = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(150));
            let hop = later.clone();
            let _ = later.run_on_main_thread(move || {
                let bits = read_window_bits(&hop);
                crate::rail_edge::note_active_after(bits.app_active);
            });
        });
    }
    // Listen-only diag. No-op unless RAIL_MOUSE_DIAG=1.
    #[cfg(target_os = "macos")]
    crate::windows::rail_mouse_diag::sync_revealed(revealed);
    // Local Cmd+key Back/Forward. Installed while revealed, removed on park.
    #[cfg(target_os = "macos")]
    crate::windows::sync_rail_mouse_keys(revealed);
}

#[derive(Clone, Copy)]
struct WindowBits {
    app_active: bool,
    is_key: bool,
    occluded: bool,
    level: i64,
    alpha: f64,
}

fn live_rail_facts(app: &tauri::AppHandle) -> crate::rail_stage::RailFacts {
    let bits = read_window_bits(app);
    crate::rail_stage::RailFacts {
        app_active: bits.app_active,
        key: bits.is_key,
        occluded: bits.occluded,
        cursor_in_frame: false,
        away_ms: 0,
        occluded_ms: if bits.occluded {
            crate::rail_stage::RAIL_AWAY_MS
        } else {
            0
        },
        button_down: false,
        auto_hide: crate::rail_edge::cached_auto_hide(),
    }
}

pub(crate) fn sample_rail_chrome(app: &tauri::AppHandle, source: Option<&str>) {
    let bits = read_window_bits(app);
    let frame = app
        .get_webview_window("main")
        .and_then(|w| live_points_rect(&w))
        .map(|r| crate::rail_edge::ScreenRect {
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
        })
        .unwrap_or(crate::rail_edge::ScreenRect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        });
    crate::rail_edge::note_chrome(
        bits.level,
        bits.alpha,
        frame,
        bits.app_active,
        bits.is_key,
        bits.occluded,
        source,
    );
}

fn ensure_edge(app: &tauri::AppHandle) -> String {
    if !crate::rail_edge::stage_known() {
        if let Ok(st) = state::load_state(app) {
            crate::rail_edge::note_edge_prefs(&st.rail_edge, st.rail_auto_hide_on_blur);
        }
    }
    let edge = crate::rail_edge::cached_edge().to_string();
    if let Some((full, vis)) = crate::rail_edge::capture_primary() {
        crate::rail_edge::note_display(
            &edge, full.x, full.y, full.w, full.h, vis.x, vis.y, vis.w, vis.h,
        );
    }
    edge
}

fn place_for(app: &tauri::AppHandle, edge: &str, revealed: bool) -> Option<state::CompanionBounds> {
    let _ = app;
    let vis = points_frames(&screen_visible_frames()).into_iter().next()?;
    let fulls = crate::rail_edge::all_full_frames();
    let full = fulls.first().copied().unwrap_or(crate::rail_edge::ScreenRect {
        x: vis.x,
        y: vis.y,
        w: vis.w,
        h: vis.h,
    });
    let (rw, rh) = state::rail_wheel_size(edge, true, vis.w, vis.h);
    let (ax, ay, aw, ah) =
        state::rail_stage_frame(edge, true, vis.x, vis.y, vis.w, vis.h, rw, rh);
    remember_anchor(state::CompanionBounds {
        x: ax,
        y: ay,
        w: aw,
        h: ah,
    });
    if revealed {
        return Some(state::CompanionBounds {
            x: ax,
            y: ay,
            w: aw,
            h: ah,
        });
    }
    let (pw, ph) = stage_parked_size(edge, full.w, full.h);
    let parked = crate::rail_edge::parked_place(edge, full, &fulls, pw, ph);
    Some(state::CompanionBounds {
        x: parked.x,
        y: parked.y,
        w: parked.w,
        h: parked.h,
    })
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
fn read_window_bits(app: &tauri::AppHandle) -> WindowBits {
    let slot = std::sync::Arc::new(std::sync::Mutex::new(WindowBits {
        app_active: false,
        is_key: false,
        occluded: false,
        level: 0,
        alpha: 0.0,
    }));
    let write = slot.clone();
    let app = app.clone();
    crate::main_thread_sync(move || {
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};
        let mut bits = write.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            if !ns_app.is_null() {
                bits.app_active = msg_send![ns_app, isActive];
            }
            if let Some(w) = app.get_webview_window("main") {
                if let Ok(raw) = w.ns_window() {
                    let nsw = raw as *mut Object;
                    if !nsw.is_null() {
                        bits.is_key = msg_send![nsw, isKeyWindow];
                        let occ: usize = msg_send![nsw, occlusionState];
                        bits.occluded = occ & 2 == 0;
                        bits.level = msg_send![nsw, level];
                        bits.alpha = msg_send![nsw, alphaValue];
                    }
                }
            }
        }
    });
    let guard = slot.lock().unwrap_or_else(|e| e.into_inner());
    *guard
}

#[cfg(not(target_os = "macos"))]
fn read_window_bits(_app: &tauri::AppHandle) -> WindowBits {
    WindowBits {
        app_active: false,
        is_key: false,
        occluded: false,
        level: 0,
        alpha: 0.0,
    }
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
fn stage_alpha_level(w: &tauri::WebviewWindow, alpha: f64, level: i64) {
    let w = w.clone();
    crate::main_thread_sync(move || {
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};
        let Ok(raw) = w.ns_window() else {
            return;
        };
        let nsw = raw as *mut Object;
        if nsw.is_null() {
            return;
        }
        unsafe {
            let _: () = msg_send![nsw, setAlphaValue: alpha];
            let _: () = msg_send![nsw, setLevel: level];
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn stage_alpha_level(_w: &tauri::WebviewWindow, _alpha: f64, _level: i64) {}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
fn stage_order_front(w: &tauri::WebviewWindow) {
    let w = w.clone();
    crate::main_thread_sync(move || {
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};
        let Ok(raw) = w.ns_window() else {
            return;
        };
        let nsw = raw as *mut Object;
        if nsw.is_null() {
            return;
        }
        unsafe {
            let _: () = msg_send![nsw, orderFrontRegardless];
            let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            if !ns_app.is_null() {
                let responds: bool = msg_send![ns_app, respondsToSelector: sel!(activate)];
                if responds {
                    let _: () = msg_send![ns_app, activate];
                } else {
                    let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
                }
            }
            let _: () = msg_send![nsw, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn stage_order_front(_w: &tauri::WebviewWindow) {}

