use crate::state;
use tauri::{Emitter, Manager};

use super::rt::DRAWER_GAP;
use super::rt::VisibleFrame;
use super::place::blossom_beside_rail;
use super::place::centered_on_frame;
use super::rt::drawer_cache_refresh;
use super::rt::drawer_expect;
use super::place::drawer_live_off;
use super::rt::drawer_order_front;
use super::rt::drawer_rt;
use super::place::live_points_rect;
use super::place::occupied_drawer_rects;
use super::rt::pane_appkit_visible;
use super::place::place_chrome;
use super::rt::points_frames;
use super::place::rail_points_rect;
use super::stage::rail_stage_apply;
use super::stage::rail_stage_is_revealed;
use super::place::rect_fully_off_all;
use super::place::rect_mostly_on;
use super::place::rect_overlap;
use super::rt::schedule_drawer_mount_check;
use super::rt::screen_visible_frames;
use super::place::tile_past_occupied;

/// Show Orbitty / dock: if the rail is not mostly on any display,
/// re-home it on the primary (left inset, vertically centered). Never
/// a leash — chrome is not moved. Size is the live frame.
/// A parked stage strip is intentionally mostly off-screen; re-home would
/// pull that sliver back to the left inset, so a hidden strip stays parked.
pub(crate) fn heal_rail(app: &tauri::AppHandle) {
    if !rail_stage_is_revealed() {
        rail_stage_apply(app, false);
        return;
    }
    let Some(w) = app.get_webview_window("main") else {
        return;
    };
    let pvfs = points_frames(&screen_visible_frames());
    if pvfs.is_empty() {
        return;
    }
    let live = live_points_rect(&w);
    let ok = live
        .as_ref()
        .map(|r| pvfs.iter().any(|vf| rect_mostly_on(r, vf)))
        .unwrap_or(false);
    if ok {
        return;
    }
    let vf = pvfs[0];
    let (ww, hh) = live
        .map(|r| (r.w.max(160.0).min(vf.w), r.h.max(200.0).min(vf.h)))
        .unwrap_or((360.0_f64.min(vf.w), 420.0_f64.min(vf.h)));
    let r = state::CompanionBounds {
        x: vf.x + 24.0,
        y: vf.y + ((vf.h - hh) / 2.0).max(0.0),
        w: ww,
        h: hh,
    };
    appkit_place_points(&w, &r);
}

/// No usable last-place (Law B): blossom beside the rail this show only,
/// else center on the primary display's visibleFrame. Primary is
/// NSScreen.screens[0] — first in screen_visible_frames. If the pane
/// does not fit there, the next largest display that fits, still
/// centered. Degenerate (nothing fits): still the primary center, never
/// vis-origin / upper-left. None only when no display geometry is known
/// (caller: companion → companion_default_frame).
pub(crate) fn drawer_slot_fallback(
    app: &tauri::AppHandle,
    w: &tauri::WebviewWindow,
    label: &str,
) -> Option<state::CompanionBounds> {
    let screens = screen_visible_frames();
    let pvfs = points_frames(&screens);
    let spec = crate::panes::pane_spec(label);
    let mut size = live_points_rect(w)
        .map(|r| (r.w, r.h))
        .or_else(|| spec.map(|(_, sw, sh, _)| (sw, sh)))
        .unwrap_or((780.0, 520.0));
    if let Some((_, sw, sh, min)) = spec {
        let doubled = (1.8..=2.2).contains(&(size.0 / sw)) && (1.8..=2.2).contains(&(size.1 / sh));
        let ghosted = min
            .filter(|(mw, mh)| sw > *mw || sh > *mh)
            .map(|(mw, mh)| size.0 <= mw + 1.0 && size.1 <= mh + 1.0)
            .unwrap_or(false);
        if doubled || ghosted {
            size = (sw, sh);
        }
    }
    if let Some(r) = pane_center_fallback(app, label, size, &pvfs) {
        return Some(r);
    }
    pvfs.first().copied().and_then(|vf| {
        let cascade = state::DRAWER_LABELS
            .iter()
            .position(|l| *l == label)
            .unwrap_or(0) as f64
            * 36.0;
        let r = centered_on_frame(vf, size, cascade);
        let occupied = occupied_drawer_rects(app, Some(label));
        tile_past_occupied(app, r, &vf, &occupied)
    })
}

/// Primary first (NSScreen[0]), then remaining displays by area. Blossom
/// beside the rail (then beside already-shown drawers). Center only if
/// blossom cannot sit, and even then tile past occupied so chrome does
/// not land on one origin.
pub(crate) fn pane_center_fallback(
    app: &tauri::AppHandle,
    label: &str,
    size: (f64, f64),
    pvfs: &[VisibleFrame],
) -> Option<state::CompanionBounds> {
    if pvfs.is_empty() {
        return None;
    }
    let (fw, fh) = crate::panes::pane_spec(label)
        .map(|(_, sw, sh, _)| (sw.max(size.0), sh.max(size.1)))
        .unwrap_or(size);
    if let Some(r) = blossom_beside_rail(app, size, pvfs, label) {
        return Some(r);
    }
    let rail = rail_points_rect(app);
    let occupied = occupied_drawer_rects(app, Some(label));
    let mut rest: Vec<VisibleFrame> = pvfs.iter().skip(1).copied().collect();
    rest.sort_by(|a, b| {
        (b.w * b.h)
            .partial_cmp(&(a.w * a.h))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let cands: Vec<VisibleFrame> = std::iter::once(pvfs[0]).chain(rest).collect();
    for vf in cands {
        if vf.w < fw || vf.h < fh {
            continue;
        }
        let mut r = centered_on_frame(vf, size, 0.0);
        if let Some(rr) = &rail {
            let (ox, oy) = rect_overlap(
                &r,
                &VisibleFrame {
                    x: rr.x,
                    y: rr.y,
                    w: rr.w,
                    h: rr.h,
                },
            );
            if ox > 1.0 && oy > 1.0 {
                // Prefer the inboard side of the rail, then the other
                // side. Never skip this display — that dumped first-open
                // chrome onto a portrait hanging below the primary.
                let inboard_x = rr.x + rr.w + DRAWER_GAP;
                let other_x = rr.x - DRAWER_GAP - r.w;
                if inboard_x + r.w <= vf.x + vf.w {
                    r.x = inboard_x;
                } else if other_x >= vf.x {
                    r.x = other_x;
                }
                r.y = (vf.y + (vf.h - r.h) / 2.0).max(vf.y);
                if r.y + r.h > vf.y + vf.h {
                    r.y = vf.y + vf.h - r.h;
                }
                r.y = r.y.max(vf.y);
            }
        }
        if let Some(t) = tile_past_occupied(app, r, &vf, &occupied) {
            return Some(t);
        }
        // Do not return the covering center — that stacked chrome on the
        // rail in the middle of the main display.
    }
    None
}

/// The one drawer placement primitive — NSWindow
/// setFrame: in AppKit global points, on the main thread and WAITED (the
/// same sync hop as the front-truth raise). Tauri's LogicalPosition /
/// LogicalSize convert through the window's CURRENT monitor scale, so a
/// pane parked on a foreign-scale display (WARM's hidden create parks on
/// s2 @2x) landed far from the points target — the click-hop. AppKit
/// points are the scale-free space screen_visible_frames already speaks;
/// top-left points flip to AppKit's bottom-left against the primary
/// screen's height, the same flip screen_visible_frames does.
#[cfg(target_os = "macos")]
pub(crate) fn appkit_place_points(w: &tauri::WebviewWindow, r: &state::CompanionBounds) {
    let w = w.clone();
    let r = *r;
    crate::main_thread_sync(move || {
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct NSPoint {
            x: f64,
            y: f64,
        }
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct NSSize {
            width: f64,
            height: f64,
        }
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct NSRect {
            origin: NSPoint,
            size: NSSize,
        }
        let Ok(raw) = w.ns_window() else {
            return;
        };
        if raw.is_null() {
            return;
        }
        unsafe {
            let screens: *mut Object = msg_send![class!(NSScreen), screens];
            if screens.is_null() {
                return;
            }
            let count: usize = msg_send![screens, count];
            if count == 0 {
                return;
            }
            let s0: *mut Object = msg_send![screens, objectAtIndex: 0usize];
            let primary: NSRect = msg_send![s0, frame];
            let frame = NSRect {
                origin: NSPoint {
                    x: r.x,
                    y: primary.size.height - (r.y + r.h),
                },
                size: NSSize {
                    width: r.w,
                    height: r.h,
                },
            };
            let nsw = raw as *mut Object;
            let _: () = msg_send![nsw, setFrame: frame display: true];
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn appkit_place_points(w: &tauri::WebviewWindow, r: &state::CompanionBounds) {
    let _ = w.set_size(tauri::LogicalSize::new(r.w, r.h));
    let _ = w.set_position(tauri::LogicalPosition::new(r.x, r.y));
}

/// Points-space placement: the rect
/// speaks CG points and placement is AppKit-points native — never Tauri
/// Logical/Physical set_position/set_size. The Moved ledger is stamped
/// from the post-place outer_position read-back ONLY (no scale-guessed
/// prediction), so our own move never reads as a drag.
pub(crate) fn place_logical_rect(w: &tauri::WebviewWindow, label: &str, r: &state::CompanionBounds) {
    let r = state::CompanionBounds {
        x: r.x,
        y: r.y,
        w: r.w.max(280.0),
        h: r.h.max(360.0),
    };
    appkit_place_points(w, &r);
    if let Ok(p) = w.outer_position() {
        drawer_expect(label, (p.x, p.y));
    }
}

/// Bar placement — the same AppKit-points path as
/// place_logical_rect but with the hugged floor (120) instead of the 360
/// idle min, so the CC+dock bar can exist at all. The caller drops the
/// window's own min-size before this runs and never persists the bar.
pub(crate) fn place_hug_rect(w: &tauri::WebviewWindow, label: &str, r: &state::CompanionBounds) {
    let r = state::CompanionBounds {
        x: r.x,
        y: r.y,
        w: r.w.max(280.0),
        h: r.h.max(120.0),
    };
    appkit_place_points(w, &r);
    if let Ok(p) = w.outer_position() {
        drawer_expect(label, (p.x, p.y));
    }
}

/// Persist the live frame unless it is fully off every display.
/// Covering the rail, sitting at center, overlapping another pane:
/// those are the user's place. Companion also writes legacy bounds.
fn persist_drawer_rect(app: &tauri::AppHandle, label: &str, rect: state::CompanionBounds) {
    let pvfs = points_frames(&screen_visible_frames());
    if rect_fully_off_all(&rect, &pvfs) {
        return;
    }
    {
        let Ok(mut rt) = drawer_rt().lock() else {
            return;
        };
        if let Some(d) = rt.drawers.as_mut() {
            let entry = d.entry(label.to_string()).or_default();
            entry.docked = false;
            entry.dx = 0.0;
            entry.dy = 0.0;
            entry.bounds = Some(rect);
        }
        rt.dirty = true;
        if label == "companion" {
            rt.companion_legacy_bounds = true;
        }
    }
    if label == "companion" {
        let _ = state::update(app, |st| {
            st.companion.bounds = Some(rect);
            Ok(())
        });
    }
}

/// Persist the warm drawers cache into state.json (other slices are
/// reload-preserved). Clears the dirty flag; emits state:changed.
pub(crate) fn drawer_persist(app: &tauri::AppHandle) {
    let drawers = drawer_rt()
        .lock()
        .ok()
        .and_then(|rt| rt.drawers.clone());
    let Some(drawers) = drawers else {
        return;
    };
    if state::update(app, |st| {
        st.drawers = drawers;
        Ok(())
    })
    .is_ok()
    {
        if let Ok(mut rt) = drawer_rt().lock() {
            rt.dirty = false;
        }
        let _ = app.emit("state:changed", ());
    }
}

/// 1s-throttled persist — unused while Law B writes place only on hide.
#[allow(dead_code)]
fn drawer_persist_throttled(app: &tauri::AppHandle, label: &str) {
    let due = {
        let Ok(mut rt) = drawer_rt().lock() else {
            return;
        };
        let due = rt
            .last_write
            .get(label)
            .map(|t| t.elapsed() >= std::time::Duration::from_secs(1))
            .unwrap_or(true);
        if due {
            rt.last_write
                .insert(label.to_string(), std::time::Instant::now());
        }
        due
    };
    if due {
        drawer_persist(app);
    }
}

/// Law B: persist the live points rect if it is mostly on a display.
pub(crate) fn persist_shown_place(app: &tauri::AppHandle, label: &str) {
    let Some(w) = app.get_webview_window(label) else {
        return;
    };
    if w.is_maximized().unwrap_or(false) {
        return;
    }
    let Some(rect) = live_points_rect(&w) else {
        return;
    };
    persist_drawer_rect(app, label, rect);
    drawer_persist(app);
}

/// Law B: do not persist during drag. A Moved at 60Hz used to write
/// Tauri-scale-poisoned rects (and emit state:changed) while the mouse
/// still owned the window — that is the hop. Place is written once on
/// hide/close via persist_shown_place.
fn on_drawer_moved(_app: &tauri::AppHandle, _label: &str, _pos: tauri::PhysicalPosition<i32>) {}

fn on_drawer_resized(_app: &tauri::AppHandle, _label: &str) {}

/// Leash wiring for the three drawer labels (alongside wire_pane_events at
/// create/setup). No-op for any other label.
pub(crate) fn wire_drawer_events(app: &tauri::AppHandle, window: &tauri::WebviewWindow, label: &str) {
    if !state::DRAWER_LABELS.contains(&label) {
        return;
    }
    let handle = app.clone();
    let label_owned = label.to_string();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Moved(pos) => on_drawer_moved(&handle, &label_owned, *pos),
        tauri::WindowEvent::Resized(_) => on_drawer_resized(&handle, &label_owned),
        _ => {}
    });
}

/// Click shows, drag moves, hide hides. Next show is the last saved
/// frame (fully-off → beside the rail once). Already-visible is
/// focus-only — never re-place a pane the user can already see.
/// Show means stay shown. Chrome is not pinned.
pub(crate) fn show_drawer(app: &tauri::AppHandle, label: &str) -> Result<(), String> {
    let w = crate::panes::ensure_window(app, label)?;
    let was_minimized = crate::panes::restore_if_minimized(&w);
    let st = state::load_state(app).unwrap_or_default();
    drawer_cache_refresh(&st);
    let visible = w.is_visible().unwrap_or(false);
    // A hugged bar sits below the 360 min on
    // purpose — it is not a ghosted min-size window; never re-place it
    // to the saved tall rect on a summon.
    let hugged = label == "companion" && crate::panes::companion_hugged();
    let ghosted = !hugged
        && live_points_rect(&w)
            .map(|r| {
                crate::panes::pane_spec(label)
                    .and_then(|(_, sw, sh, min)| {
                        min.filter(|(mw, mh)| sw > *mw || sh > *mh)
                            .map(|(mw, mh)| r.w <= mw + 1.0 && r.h <= mh + 1.0)
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    // a visible pane with no
    // 24px handle on ANY display is lost, not "already visible" — fall
    // through to the place path so the summon actually recovers it (for
    // the companion, raise_now then clamps the dock back on-screen).
    if was_minimized || (visible && !ghosted && pane_appkit_visible(&w) && !drawer_live_off(&w)) {
        let _ = w.unminimize();
        let _ = w.show();
        w.set_focus().map_err(|e| e.to_string())?;
        let _ = w.set_ignore_cursor_events(false);
        crate::panes::emit_pane_vis(app, label, true);
        schedule_drawer_mount_check(app, label);
        // The forced mouse-on above must not
        // stick on a ghost — re-apply slab/ghost from the live mode.
        if label == "companion" {
            crate::companion_clear::apply_mode(app);
        }
        // FRONT-HOLD: the raise lands AFTER apply_mode so the
        // ghost re-apply can never undo the front — Show Companion on an
        // already-visible buried ghost recovers without Quit.
        drawer_order_front(app, &w, label);
        return Ok(());
    }
    place_chrome(app, &w, label, &st);
    w.show().map_err(|e| e.to_string())?;
    w.set_focus().map_err(|e| e.to_string())?;
    let _ = w.set_ignore_cursor_events(false);
    crate::panes::emit_pane_vis(app, label, true);
    schedule_drawer_mount_check(app, label);
    // Re-apply slab/ghost first, then raise
    // so apply_mode cannot undo the front (same order as already-visible).
    if label == "companion" {
        crate::companion_clear::apply_mode(app);
    }
    drawer_order_front(app, &w, label);
    if !pane_appkit_visible(&w) {
        let _ = w.show();
        let _ = w.set_focus();
        if label == "companion" {
            crate::companion_clear::apply_mode(app);
        }
        drawer_order_front(app, &w, label);
    }
    Ok(())
}

