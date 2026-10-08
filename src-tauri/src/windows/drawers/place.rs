use crate::state;
use tauri::Manager;

use super::rt::DRAWER_GAP;
use super::rt::DRAWER_HANDLE;
use super::rt::VisibleFrame;
use super::rt::drawer_expect;
use super::rt::drawer_order_front;
use super::rt::pane_appkit_visible;
use super::rt::points_frames;
use super::rt::schedule_drawer_mount_check;
use super::rt::screen_visible_frames;
use super::show::appkit_place_points;
use super::show::drawer_slot_fallback;
use super::show::place_hug_rect;
use super::show::place_logical_rect;
use super::stage::REVEALED_ANCHOR;
use super::stage::rail_stage_is_revealed;

/// No overlap with the visibleFrame at all.
fn rect_fully_off(b: &state::CompanionBounds, vf: &VisibleFrame) -> bool {
    b.x + b.w <= vf.x || b.x >= vf.x + vf.w || b.y + b.h <= vf.y || b.y >= vf.y + vf.h
}

/// Overlap lengths (w, h) of two rects; 0 on an axis when disjoint.
pub(crate) fn rect_overlap(b: &state::CompanionBounds, vf: &VisibleFrame) -> (f64, f64) {
    let ox = ((b.x + b.w).min(vf.x + vf.w) - b.x.max(vf.x)).max(0.0);
    let oy = ((b.y + b.h).min(vf.y + vf.h) - b.y.max(vf.y)).max(0.0);
    (ox, oy)
}

/// The grab-handle law: at least DRAWER_HANDLE × DRAWER_HANDLE of the
/// rect sits on this frame.
fn rect_handle_on(b: &state::CompanionBounds, vf: &VisibleFrame) -> bool {
    let (ox, oy) = rect_overlap(b, vf);
    ox >= DRAWER_HANDLE && oy >= DRAWER_HANDLE
}

/// Law B: a saved/live frame is "on a screen" only if half its width AND
/// half its height sit on this display. A 24px sliver is not a place.
pub(crate) fn rect_mostly_on(b: &state::CompanionBounds, vf: &VisibleFrame) -> bool {
    if b.w <= 0.0 || b.h <= 0.0 {
        return false;
    }
    let (ox, oy) = rect_overlap(b, vf);
    ox >= b.w * 0.5 && oy >= b.h * 0.5
}

/// Persist/restore gate leftover — place_chrome no longer uses this.
/// No display info → true (never treat a pane as lost when screens unknown).
#[allow(dead_code)]
fn rect_usable(app: &tauri::AppHandle, b: &state::CompanionBounds) -> bool {
    if points_cover_rail(app, b) {
        return false;
    }
    let pvfs = points_frames(&screen_visible_frames());
    pvfs.is_empty() || pvfs.iter().any(|vf| rect_mostly_on(b, vf))
}

fn heal_saved_size(label: &str, mut r: state::CompanionBounds) -> state::CompanionBounds {
    if let Some((_, sw, sh, min)) = crate::panes::pane_spec(label) {
        let doubled =
            (1.8..=2.2).contains(&(r.w / sw)) && (1.8..=2.2).contains(&(r.h / sh));
        let ghosted = min
            .filter(|(mw, mh)| sw > *mw || sh > *mh)
            .map(|(mw, mh)| r.w <= mw + 1.0 && r.h <= mh + 1.0)
            .unwrap_or(false);
        if doubled || ghosted {
            r.w = sw;
            r.h = sh;
        }
    }
    r
}

fn saved_bounds(st: &state::PersistedState, label: &str) -> Option<state::CompanionBounds> {
    if let Some(b) = st.drawers.get(label).and_then(|d| d.bounds) {
        return Some(b);
    }
    if label == "companion" {
        return st.companion.bounds;
    }
    None
}

/// One place law: last saved frame if it still sits on some display.
/// Fully off every display, or no save → once beside the rail.
/// Overlap, center, covering the rail: those are the user's place.
pub(crate) fn place_chrome(
    app: &tauri::AppHandle,
    w: &tauri::WebviewWindow,
    label: &str,
    st: &state::PersistedState,
) {
    if let Some(b) = saved_bounds(st, label) {
        let r = heal_saved_size(label, b);
        let pvfs = points_frames(&screen_visible_frames());
        if pvfs.is_empty() || !rect_fully_off_all(&r, &pvfs) {
            place_logical_rect(w, label, &r);
            return;
        }
    }
    if let Some(r) = drawer_slot_fallback(app, w, label) {
        place_logical_rect(w, label, &r);
    } else if label == "companion" {
        crate::panes::companion_default_frame(app, w);
    }
}

/// Fully off EVERY frame in the set. No display info → false (never
/// treat a pane as lost when the screens cannot be known).
pub(crate) fn rect_fully_off_all(b: &state::CompanionBounds, vfs: &[VisibleFrame]) -> bool {
    !vfs.is_empty() && vfs.iter().all(|vf| rect_fully_off(b, vf))
}

/// A live window's rect in global CG points. Read NSWindow.frame on the
/// main thread and flip to top-left — the same space appkit_place_points
/// writes. Never Tauri outer_position / current_monitor scale: that is
/// how mixed-scale desks saved a far hop and the next show honored it.
#[cfg(target_os = "macos")]
pub(crate) fn live_points_rect(w: &tauri::WebviewWindow) -> Option<state::CompanionBounds> {
    let w = w.clone();
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    let out = slot.clone();
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
            let nsw = raw as *mut Object;
            let frame: NSRect = msg_send![nsw, frame];
            let rect = state::CompanionBounds {
                x: frame.origin.x,
                y: primary.size.height - (frame.origin.y + frame.size.height),
                w: frame.size.width,
                h: frame.size.height,
            };
            if let Ok(mut g) = out.lock() {
                *g = Some(rect);
            }
        }
    });
    slot.lock().ok().and_then(|g| *g)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn live_points_rect(w: &tauri::WebviewWindow) -> Option<state::CompanionBounds> {
    let p = w.outer_position().ok()?;
    let s = w.outer_size().ok()?;
    let scale = w
        .current_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    let scale = if scale > 0.0 { scale } else { 1.0 };
    Some(state::CompanionBounds {
        x: p.x as f64 / scale,
        y: p.y as f64 / scale,
        w: s.width as f64 / scale,
        h: s.height as f64 / scale,
    })
}

/// The rail's rect in global CG points. Same AppKit frame as every
/// drawer — never Tauri outer_position / current_monitor scale. A
/// mixed-scale desk used to make pane_center_fallback think the primary
/// center covered a phantom rail and dump first-open chrome onto the
/// portrait hanging below the main display.
fn revealed_anchor() -> Option<state::CompanionBounds> {
    REVEALED_ANCHOR.lock().ok().and_then(|g| *g)
}

pub(crate) fn remember_anchor(b: state::CompanionBounds) {
    if let Ok(mut g) = REVEALED_ANCHOR.lock() {
        *g = Some(b);
    }
}

/// The rail's rect in global CG points. A parked rail sits fully off an
/// outer edge, so placement uses the last revealed frame instead of the
/// live (off-screen) window.
pub(crate) fn rail_points_rect(app: &tauri::AppHandle) -> Option<state::CompanionBounds> {
    if !rail_stage_is_revealed() {
        if let Some(anchor) = revealed_anchor() {
            return Some(anchor);
        }
    }
    let rail = app.get_webview_window("main")?;
    live_points_rect(&rail)
}

/// The click-eater test, in points: more than a shared 1px edge of
/// overlap with the rail. A pane on another display can never cover it —
/// mixed-scale physical px alone cannot say so.
pub(crate) fn points_cover_rail(app: &tauri::AppHandle, r: &state::CompanionBounds) -> bool {
    let Some(rail) = rail_points_rect(app) else {
        return false;
    };
    let (ox, oy) = rect_overlap(
        r,
        &VisibleFrame {
            x: rail.x,
            y: rail.y,
            w: rail.w,
            h: rail.h,
        },
    );
    ox > 1.0 && oy > 1.0
}

/// The live frame covers the rail right now.
#[allow(dead_code)]
fn live_covers_rail(app: &tauri::AppHandle, w: &tauri::WebviewWindow) -> bool {
    live_points_rect(w)
        .map(|r| points_cover_rail(app, &r))
        .unwrap_or(false)
}

fn rects_overlap_px(a: &state::CompanionBounds, b: &state::CompanionBounds) -> bool {
    let (ox, oy) = rect_overlap(
        a,
        &VisibleFrame {
            x: b.x,
            y: b.y,
            w: b.w,
            h: b.h,
        },
    );
    ox > 1.0 && oy > 1.0
}

/// Live, AppKit-visible chrome drawers except `except`, plus the rail.
/// The rail is not a drawer but it occupies desk — without it, blossom
/// parked chrome on the rail origin.
pub(crate) fn occupied_drawer_rects(
    app: &tauri::AppHandle,
    except: Option<&str>,
) -> Vec<state::CompanionBounds> {
    let mut out = Vec::new();
    for label in state::DRAWER_LABELS {
        if except == Some(label) {
            continue;
        }
        let Some(w) = app.get_webview_window(label) else {
            continue;
        };
        if !w.is_visible().unwrap_or(false) {
            continue;
        }
        if !pane_appkit_visible(&w) {
            continue;
        }
        if let Some(r) = live_points_rect(&w) {
            out.push(r);
        }
    }
    // The rail is not a drawer, but it occupies desk. Without this,
    // blossom/tile parked chrome on the rail origin (the center pile).
    if except != Some("main") {
        if let Some(r) = rail_points_rect(app) {
            out.push(r);
        }
    }
    out
}

#[allow(dead_code)]
fn rect_overlaps_occupied(
    app: &tauri::AppHandle,
    b: &state::CompanionBounds,
    except: Option<&str>,
) -> bool {
    occupied_drawer_rects(app, except)
        .iter()
        .any(|o| rects_overlap_px(b, o))
}

/// Auto-center leftover from the old 36px cascade. Unused by place_chrome
/// now — overlapping / center is a legal user place.
#[allow(dead_code)]
fn rect_is_center_pile(app: &tauri::AppHandle, b: &state::CompanionBounds) -> bool {
    let pvfs = points_frames(&screen_visible_frames());
    let Some(vf) = pvfs.first().copied() else {
        return false;
    };
    let c = centered_on_frame(vf, (b.w, b.h), 0.0);
    (b.x - c.x).abs() < 64.0 && (b.y - c.y).abs() < 64.0
}

fn tile_candidate_ok(
    app: &tauri::AppHandle,
    t: &state::CompanionBounds,
    vf: &VisibleFrame,
    occupied: &[state::CompanionBounds],
) -> bool {
    rect_mostly_on(t, vf)
        && !points_cover_rail(app, t)
        && !occupied.iter().any(|o| rects_overlap_px(t, o))
}

pub(crate) fn tile_past_occupied(
    app: &tauri::AppHandle,
    mut r: state::CompanionBounds,
    vf: &VisibleFrame,
    occupied: &[state::CompanionBounds],
) -> Option<state::CompanionBounds> {
    // Never clamp back onto occupied (that stacked chrome on the rail).
    // A pane may hang off the display as long as Law B (half w AND half h)
    // still sits on `vf`.
    for _ in 0..8 {
        let hit = occupied.iter().find(|o| rects_overlap_px(&r, o));
        let Some(o) = hit else {
            break;
        };
        let mut next = None;
        let shifts = [
            (o.x + o.w + DRAWER_GAP, r.y),
            (o.x - DRAWER_GAP - r.w, r.y),
            (r.x, o.y + o.h + DRAWER_GAP),
            (r.x, o.y - DRAWER_GAP - r.h),
        ];
        for (x, y) in shifts {
            let t = state::CompanionBounds {
                x,
                y,
                w: r.w,
                h: r.h,
            };
            if tile_candidate_ok(app, &t, vf, occupied) {
                next = Some(t);
                break;
            }
        }
        match next {
            Some(t) => r = t,
            None => return None,
        }
    }
    if tile_candidate_ok(app, &r, vf, occupied) {
        Some(r)
    } else {
        None
    }
}

/// lib.rs bounds writers never name VisibleFrame: fully off the UNION of
/// every display's points visibleFrame. No display info → false (never
/// treat a pane as lost when the screens cannot be known).
#[allow(dead_code)]
pub(crate) fn bounds_fully_off(_app: &tauri::AppHandle, rect: &state::CompanionBounds) -> bool {
    rect_fully_off_all(rect, &points_frames(&screen_visible_frames()))
}

/// Already-visible gate: the live frame lacks a 24px handle on EVERY
/// display. No display/frame info → false (never yank a pane in use).
pub(crate) fn drawer_live_off(w: &tauri::WebviewWindow) -> bool {
    let screens = screen_visible_frames();
    if screens.points.is_empty() {
        return false;
    }
    live_points_rect(w)
        .map(|r| !screens.points.iter().any(|(vf, _)| rect_handle_on(&r, vf)))
        .unwrap_or(false)
}

/// Single-frame clamp: keep a 24px handle of the rect on this frame.
/// Caller guarantees some overlap.
fn clamp_to_frame(b: state::CompanionBounds, vf: &VisibleFrame) -> state::CompanionBounds {
    let (min_x, max_x) = {
        let lo = vf.x - b.w + DRAWER_HANDLE;
        let hi = vf.x + vf.w - DRAWER_HANDLE;
        if lo <= hi { (lo, hi) } else { (vf.x, vf.x) }
    };
    let (min_y, max_y) = {
        let lo = vf.y - b.h + DRAWER_HANDLE;
        let hi = vf.y + vf.h - DRAWER_HANDLE;
        if lo <= hi { (lo, hi) } else { (vf.y, vf.y) }
    };
    state::CompanionBounds {
        x: b.x.clamp(min_x, max_x),
        y: b.y.clamp(min_y, max_y),
        w: b.w,
        h: b.h,
    }
}

/// same handle law, but keep `b`'s bottom when that
/// still leaves a 24px handle on `vf`. Dest buttons sit on the bottom
/// dock — a y-shift of the whole rect walks them. Top/sides may move.
pub(crate) fn clamp_to_frame_keep_bottom(
    b: state::CompanionBounds,
    vf: &VisibleFrame,
) -> state::CompanionBounds {
    let bottom = b.y + b.h;
    let r = clamp_to_frame(b, vf);
    let kept = state::CompanionBounds {
        x: r.x,
        y: bottom - r.h,
        w: r.w,
        h: r.h,
    };
    if rect_handle_on(&kept, vf) {
        kept
    } else {
        r
    }
}

/// The one clamp: a 24px handle on SOME display. Fully off ALL displays →
/// None (inboard-slot fallback). Touching a display but handle-thin
/// everywhere → clamped onto the display it overlaps most. Never 0,0,
/// never screen-center. No display info → honored as-is.
fn clamp_drawer_rect(
    b: state::CompanionBounds,
    vfs: &[VisibleFrame],
) -> Option<state::CompanionBounds> {
    clamp_drawer_rect_ex(b, vfs, false)
}

fn clamp_drawer_rect_ex(
    b: state::CompanionBounds,
    vfs: &[VisibleFrame],
    keep_bottom: bool,
) -> Option<state::CompanionBounds> {
    if vfs.is_empty() {
        return Some(b);
    }
    if rect_fully_off_all(&b, vfs) {
        return None;
    }
    if vfs.iter().any(|vf| rect_handle_on(&b, vf)) {
        return Some(b);
    }
    let best = vfs
        .iter()
        .filter(|vf| !rect_fully_off(&b, vf))
        .max_by(|a, c| {
            let (aw, ah) = rect_overlap(&b, a);
            let (bw, bh) = rect_overlap(&b, c);
            (aw * ah)
                .partial_cmp(&(bw * bh))
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    Some(if keep_bottom {
        clamp_to_frame_keep_bottom(b, best)
    } else {
        clamp_to_frame(b, best)
    })
}

/// The one clamp, against every live display's points
/// visibleFrame — lets panes.rs park the companion without naming
/// VisibleFrame. Reuse of clamp_drawer_rect, not a copy.
#[allow(dead_code)]
pub(crate) fn clamp_on_displays(b: state::CompanionBounds) -> Option<state::CompanionBounds> {
    clamp_drawer_rect(b, &points_frames(&screen_visible_frames()))
}

/// same every-display clamp, but the dest dock's
/// bottom stays put when a handle still fits.
pub(crate) fn clamp_on_displays_keep_bottom(
    b: state::CompanionBounds,
) -> Option<state::CompanionBounds> {
    clamp_drawer_rect_ex(b, &points_frames(&screen_visible_frames()), true)
}

/// Host dismiss clamps on the ONE display the live
/// hosted rect sits on — the every-display pick walked the restored idle
/// rect to the big middle screen whenever the stash origin looked
/// handle-thin. That display is where the live rect is mostly on, else
/// the one it overlaps most, else first; clamp_to_frame keeps a 24px
/// handle of the RESTORE rect on it. Never centered, never another
/// monitor. No screens → honored as-is; no live rect → the old
/// every-display rescue (there is no "this display" to stay on).
/// set_companion_host off-path is the only caller.
pub(crate) fn clamp_on_host_display(
    live: Option<state::CompanionBounds>,
    restore: state::CompanionBounds,
) -> state::CompanionBounds {
    let pvfs = points_frames(&screen_visible_frames());
    if pvfs.is_empty() {
        return restore;
    }
    let Some(live) = live else {
        return clamp_drawer_rect_ex(restore, &pvfs, true).unwrap_or(restore);
    };
    let vf = pvfs
        .iter()
        .find(|vf| rect_mostly_on(&live, vf))
        .or_else(|| {
            pvfs.iter()
                .filter(|vf| !rect_fully_off(&live, vf))
                .max_by(|a, c| {
                    let (aw, ah) = rect_overlap(&live, a);
                    let (bw, bh) = rect_overlap(&live, c);
                    (aw * ah)
                        .partial_cmp(&(bw * bh))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .or_else(|| pvfs.first());
    match vf {
        Some(vf) => clamp_to_frame_keep_bottom(restore, vf),
        None => restore,
    }
}

/// the companion's dock must
/// stay clickable — a keep-bottom place whose bottom edge already hangs
/// below every display (the dock-click drop, a hug from a low-parked
/// idle, a host grow pinned to a sunk bottom) strands the window with no
/// usable handle, and ⌘⇧O / tray / dock Reopen could not recover it
/// (Matt's lost-companion report). Law: SOME display must show a 24px
/// handle AND the rect's bottom edge (the dock) at/above that display's
/// visible bottom. Fail → dock-visible clamp onto the display the rect
/// is mostly on / overlaps most / first: the bottom edge comes back onto
/// the screen (never a top-sliver "rescue" that leaves the dock below),
/// x fully on when it fits. A hugged bar re-places through the 120 hug
/// floor — the rescue never unhugs. No-op while any display satisfies
/// the law — never yanks a pane in use.
pub(crate) fn ensure_companion_dock_onscreen(w: &tauri::WebviewWindow) {
    let pvfs = points_frames(&screen_visible_frames());
    if pvfs.is_empty() {
        return;
    }
    let Some(live) = live_points_rect(w) else {
        return;
    };
    let dock_ok =
        |vf: &VisibleFrame| rect_handle_on(&live, vf) && live.y + live.h <= vf.y + vf.h + 0.5;
    if pvfs.iter().any(dock_ok) {
        return;
    }
    let vf = pvfs
        .iter()
        .find(|vf| rect_mostly_on(&live, vf))
        .or_else(|| {
            pvfs.iter()
                .filter(|vf| !rect_fully_off(&live, vf))
                .max_by(|a, c| {
                    let (aw, ah) = rect_overlap(&live, a);
                    let (bw, bh) = rect_overlap(&live, c);
                    (aw * ah)
                        .partial_cmp(&(bw * bh))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .or_else(|| pvfs.first());
    let Some(vf) = vf else { return };
    let y = if live.h <= vf.h {
        live.y.clamp(vf.y, vf.y + vf.h - live.h)
    } else {
        // Taller than the display: bottom AT the visible bottom — the
        // dock shows even though the top hangs off.
        vf.y + vf.h - live.h
    };
    let x = if live.w <= vf.w {
        live.x.clamp(vf.x, vf.x + vf.w - live.w)
    } else {
        live.x
            .clamp(vf.x - live.w + DRAWER_HANDLE, vf.x + vf.w - DRAWER_HANDLE)
    };
    let r = state::CompanionBounds {
        x,
        y,
        w: live.w,
        h: live.h,
    };
    if crate::panes::companion_hugged() {
        place_hug_rect(w, "companion", &r);
    } else {
        place_logical_rect(w, "companion", &r);
    }
}

/// The host grow follows the display the companion
/// sits on — the visibleFrame the live rect is mostly on (else first
/// screen). Landscape (vf.w >= vf.h) grows wide 1080×620; portrait
/// (vf.h > vf.w) grows tall 620×1080; no screens → 1080×620. Origin
/// stays the live rect's; clamp_on_displays runs after and never swaps
/// w/h — this picker does. set_companion_host on-path is the only caller.
pub(crate) fn host_grown_rect(cur: state::CompanionBounds) -> state::CompanionBounds {
    let pvfs = points_frames(&screen_visible_frames());
    let vf = pvfs
        .iter()
        .find(|vf| rect_mostly_on(&cur, vf))
        .or_else(|| pvfs.first());
    let (w, h) = match vf {
        Some(vf) if vf.h > vf.w => (620.0, 1080.0),
        _ => (1080.0, 620.0),
    };
    state::CompanionBounds {
        x: cur.x,
        y: cur.y,
        w,
        h,
    }
}

/// Keep `cur`'s bottom-center; apply new `w`/`h`.
/// Dest buttons sit on the ghostdock at the bottom of the companion —
/// pinning that edge (and the horizontal center) means dest toggle
/// expand/contract does not walk the cursor. Clamp after may still
/// shift for a 24px handle at a screen edge.
pub(crate) fn pin_bottom_center(
    cur: state::CompanionBounds,
    w: f64,
    h: f64,
) -> state::CompanionBounds {
    let cx = cur.x + cur.w / 2.0;
    let bottom = cur.y + cur.h;
    state::CompanionBounds {
        x: cx - w / 2.0,
        y: bottom - h,
        w,
        h,
    }
}

/// place `r`, then if macOS constrainFrameRect slid
/// the frame, put the bottom back so dests stay.
pub(crate) fn place_logical_rect_keep_bottom(
    w: &tauri::WebviewWindow,
    label: &str,
    r: &state::CompanionBounds,
) {
    let target_bottom = r.y + r.h;
    place_logical_rect(w, label, r);
    let Some(live) = live_points_rect(w) else {
        return;
    };
    if (live.y + live.h - target_bottom).abs() <= 0.5 {
        return;
    }
    let fixed = state::CompanionBounds {
        x: live.x,
        y: target_bottom - live.h,
        w: live.w,
        h: live.h,
    };
    appkit_place_points(w, &fixed);
    if let Ok(p) = w.outer_position() {
        drawer_expect(label, (p.x, p.y));
    }
}

/// Center `size` on `vf` (points, top-left), cascaded so first-opens of
/// different labels do not land on one origin. Clamped to stay inside
/// the visible frame when the pane fits.
pub(crate) fn centered_on_frame(vf: VisibleFrame, size: (f64, f64), cascade: f64) -> state::CompanionBounds {
    let mut r = state::CompanionBounds {
        x: vf.x + ((vf.w - size.0) / 2.0) + cascade,
        y: vf.y + ((vf.h - size.1) / 2.0) + cascade,
        w: size.0,
        h: size.1,
    };
    if r.x + r.w > vf.x + vf.w {
        r.x = vf.x + vf.w - r.w;
    }
    if r.y + r.h > vf.y + vf.h {
        r.y = vf.y + vf.h - r.h;
    }
    r.x = r.x.max(vf.x);
    r.y = r.y.max(vf.y);
    r
}

/// Stage-originated settings/stars: park on the rail's display, inboard of
/// the rail. `stale` is a last-place on any screen and is ignored — a
/// saved frame on another display must not win this open.
pub(crate) fn place_beside_rail_display(
    rail: &state::CompanionBounds,
    size: (f64, f64),
    frames: &[VisibleFrame],
    stale: Option<&state::CompanionBounds>,
) -> Option<state::CompanionBounds> {
    let _ = stale;
    if frames.is_empty() {
        return None;
    }
    let vf = frames
        .iter()
        .copied()
        .find(|vf| rect_mostly_on(rail, vf))
        .unwrap_or(frames[0]);
    let w = size.0.min(vf.w).max(1.0);
    let h = size.1.min(vf.h).max(1.0);
    let inboard_right = (rail.x + rail.w / 2.0) < vf.x + vf.w * 2.0 / 3.0;
    let mut x = if inboard_right {
        rail.x + rail.w + DRAWER_GAP
    } else {
        rail.x - DRAWER_GAP - w
    };
    let mut y = rail.y;
    if x < vf.x {
        x = vf.x;
    }
    if y < vf.y {
        y = vf.y;
    }
    if x + w > vf.x + vf.w {
        x = (vf.x + vf.w - w).max(vf.x);
    }
    if y + h > vf.y + vf.h {
        y = (vf.y + vf.h - h).max(vf.y);
    }
    Some(state::CompanionBounds { x, y, w, h })
}

/// Show a drawer and force it beside the rail's current display.
/// Does not honor a saved frame on another screen.
pub(crate) fn show_drawer_beside_rail(app: &tauri::AppHandle, label: &str) -> Result<(), String> {
    let w = crate::panes::ensure_window(app, label)?;
    let _ = crate::panes::restore_if_minimized(&w);
    let pvfs = points_frames(&screen_visible_frames());
    let spec = crate::panes::pane_spec(label);
    let size = live_points_rect(&w)
        .map(|r| (r.w, r.h))
        .or_else(|| spec.map(|(_, sw, sh, _)| (sw, sh)))
        .unwrap_or((780.0, 640.0));
    let st = state::load_state(app).unwrap_or_default();
    let stale = saved_bounds(&st, label);
    if let Some(rail) = rail_points_rect(app) {
        if let Some(r) = place_beside_rail_display(&rail, size, &pvfs, stale.as_ref()) {
            place_logical_rect(&w, label, &r);
        }
    }
    let _ = w.unminimize();
    w.show().map_err(|e| e.to_string())?;
    w.set_focus().map_err(|e| e.to_string())?;
    let _ = w.set_ignore_cursor_events(false);
    crate::panes::emit_pane_vis(app, label, true);
    schedule_drawer_mount_check(app, label);
    drawer_order_front(app, &w, label);
    Ok(())
}

/// This-show-only blossom: park `size` beside the live rail on the
/// rail's display, then beside any already-shown drawer. No leash —
/// next drag persists Law B as usual. Inboard of the rail (right unless
/// the rail sits in the right third), top-aligned, clamped. None if the
/// rail is unknown or the pane cannot sit without covering it.
pub(crate) fn blossom_beside_rail(
    app: &tauri::AppHandle,
    size: (f64, f64),
    pvfs: &[VisibleFrame],
    except_label: &str,
) -> Option<state::CompanionBounds> {
    let rail = rail_points_rect(app)?;
    let vf = pvfs
        .iter()
        .find(|vf| rect_mostly_on(&rail, vf))
        .copied()
        .or_else(|| pvfs.first().copied())?;
    let occupied = occupied_drawer_rects(app, Some(except_label));
    let inboard_right = (rail.x + rail.w / 2.0) < vf.x + vf.w * 2.0 / 3.0;
    let inboard_x = if inboard_right {
        rail.x + rail.w + DRAWER_GAP
    } else {
        rail.x - DRAWER_GAP - size.0
    };
    let other_x = if inboard_right {
        rail.x - DRAWER_GAP - size.0
    } else {
        rail.x + rail.w + DRAWER_GAP
    };
    // Inboard, other side, below, above — never clamp x onto the rail
    // (that was the stack: pane too wide for the leftover strip, then
    // shoved back over the rail origin).
    let starts = [
        (inboard_x, rail.y),
        (other_x, rail.y),
        (inboard_x, rail.y + rail.h + DRAWER_GAP),
        (rail.x, rail.y + rail.h + DRAWER_GAP),
        (inboard_x, rail.y - DRAWER_GAP - size.1),
    ];
    for (x, y) in starts {
        let mut r = state::CompanionBounds {
            x,
            y,
            w: size.0,
            h: size.1,
        };
        if r.y + r.h > vf.y + vf.h {
            r.y = vf.y + vf.h - r.h;
        }
        r.y = r.y.max(vf.y);
        if let Some(t) = tile_past_occupied(app, r, &vf, &occupied) {
            return Some(t);
        }
    }
    None
}

pub(crate) const EDGE_CORNER_GAP: f64 = 48.0;

