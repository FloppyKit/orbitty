//! The rail leash + drawer clamp, lifted out of
//! lib.rs (move only — behavior identical). Chrome drawers are companion /
//! kanban / settings; `sky` and the rail are never drawers.

use crate::state;
use tauri::{Emitter, Manager};

/// Vertical gap between stacked docked panes; also the rail→pane gap.
const DRAWER_GAP: f64 = 8.0;

/// Session leash runtime. `expected` is the programmatic-position ledger —
/// a short deque of our recent set_position targets per label, so a Moved
/// arriving a step late still reads as ours, while any position off that
/// path is a user drag. `drawers` is a warm copy of the persisted map so a
/// 60Hz rail drag never touches disk; every native drawer write keeps it
/// fresh.
#[derive(Default)]
pub(crate) struct DrawerRuntime {
    expected: std::collections::HashMap<String, std::collections::VecDeque<(i32, i32)>>,
    drawers: Option<std::collections::BTreeMap<String, state::DrawerState>>,
    /// The old-save law bit: legacy top-level `companion.bounds` exists.
    companion_legacy_bounds: bool,
    /// Cache holds writes not yet flushed to state.json.
    pub(crate) dirty: bool,
    /// 1s throttle for drag-frequency persists (leash stretch, plucked
    /// bounds), keyed by label.
    last_write: std::collections::HashMap<String, std::time::Instant>,
}

pub(crate) fn drawer_rt() -> &'static std::sync::Mutex<DrawerRuntime> {
    static RT: std::sync::OnceLock<std::sync::Mutex<DrawerRuntime>> = std::sync::OnceLock::new();
    RT.get_or_init(|| std::sync::Mutex::new(DrawerRuntime::default()))
}

/// Record a programmatic position for a drawer (capped ledger).
fn drawer_expect(label: &str, pos: (i32, i32)) {
    if let Ok(mut rt) = drawer_rt().lock() {
        let q = rt.expected.entry(label.to_string()).or_default();
        q.push_back(pos);
        while q.len() > 8 {
            q.pop_front();
        }
    }
}

/// A Moved matching any recent programmatic position (±2px) is ours.
#[allow(dead_code)]
fn drawer_expected_matches(label: &str, pos: (i32, i32)) -> bool {
    drawer_rt()
        .lock()
        .map(|rt| {
            rt.expected
                .get(label)
                .map(|q| {
                    q.iter()
                        .any(|e| (e.0 - pos.0).abs() <= 2 && (e.1 - pos.1).abs() <= 2)
                })
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// Refresh the warm cache from a freshly loaded/saved state.
pub(crate) fn drawer_cache_refresh(st: &state::PersistedState) {
    if let Ok(mut rt) = drawer_rt().lock() {
        rt.drawers = Some(st.drawers.clone());
        rt.companion_legacy_bounds = st.companion.bounds.is_some();
    }
}

/// A visibleFrame (menu bar / Dock excluded) in top-left-origin
/// coordinates — Tauri physical px or CG points, per the caller's space.
#[derive(Clone, Copy)]
struct VisibleFrame {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

// Drawer clamp / sheet-click / sheet-front
// One on-screen law for every drawer label (companion / kanban /
// settings), used on the already-visible path and the slot fallback.
// There is no plucked restore and no bounds writer —
// every show lands on the slot. The on-screen test is the UNION of every display's
// visibleFrame, and no honored rect may cover the rail by more than a
// shared 1px edge. SHEET-FRONT makes the whole pipeline points-native:
// CG points are the one unambiguous space on a mixed-scale desk, so
// placement (LogicalPosition/LogicalSize), the rail-cover test, and the
// persisted rect all speak points. A saved rect is read as points first;
// ~2x pane_spec / ~2x live size, or fully off every points frame, reads
// it as a physical-on-@2x leftover — halved into points and re-honored.
// Shown means STAY shown: show + a sync AppKit
// orderFront ladder, then the pane is left visible and hit-testing — the
// mouse/placement gate is telemetry only, never a hide trigger. Hide is
// only banner × / hide_window / nav-guard on a broken URL. A frame fully
// off all displays, or 2x pane_spec, never persists.

/// Px of a drawer that must stay on some display — the grab handle.
const DRAWER_HANDLE: f64 = 24.0;

/// Every display's visibleFrame in both spaces: Tauri physical px (per
/// the display's own scale — the same per-screen mapping outer_position
/// reports) and CG points, each points frame paired with its scale for
/// the saved-rect points retry.
#[derive(Default)]
struct ScreenVfs {
    physical: Vec<VisibleFrame>,
    points: Vec<(VisibleFrame, f64)>,
}

#[cfg(target_os = "macos")]
fn screen_visible_frames() -> ScreenVfs {
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
    let mut out = ScreenVfs::default();
    unsafe {
        let screens: *mut Object = msg_send![class!(NSScreen), screens];
        if screens.is_null() {
            return out;
        }
        let count: usize = msg_send![screens, count];
        if count == 0 {
            return out;
        }
        // AppKit global coords are points with a bottom-left origin; flip
        // against the primary screen's height into top-left points, then
        // each display's own backing scale gives Tauri physical px.
        let s0: *mut Object = msg_send![screens, objectAtIndex: 0usize];
        let primary: NSRect = msg_send![s0, frame];
        let primary_h = primary.size.height;
        for i in 0..count {
            let s: *mut Object = msg_send![screens, objectAtIndex: i];
            let vf: NSRect = msg_send![s, visibleFrame];
            let scale: f64 = msg_send![s, backingScaleFactor];
            let scale = if scale > 0.0 { scale } else { 1.0 };
            let pts = VisibleFrame {
                x: vf.origin.x,
                y: primary_h - (vf.origin.y + vf.size.height),
                w: vf.size.width,
                h: vf.size.height,
            };
            out.physical.push(VisibleFrame {
                x: pts.x * scale,
                y: pts.y * scale,
                w: pts.w * scale,
                h: pts.h * scale,
            });
            out.points.push((pts, scale));
        }
    }
    out
}

#[cfg(not(target_os = "macos"))]
fn screen_visible_frames() -> ScreenVfs {
    ScreenVfs::default()
}

/// The points-space frames, scales dropped.
fn points_frames(screens: &ScreenVfs) -> Vec<VisibleFrame> {
    screens.points.iter().map(|(vf, _)| *vf).collect()
}


/// Front truth for one of our windows: NSWindow
/// isVisible, full stop. The CG on-screen-only list lags and misses
/// overlay-transparent drawers entirely, and isOnActiveSpace false-fails
/// the unpinned companion — both are telemetry now, never a hide trigger.
#[cfg(target_os = "macos")]
pub(crate) fn pane_appkit_visible(w: &tauri::WebviewWindow) -> bool {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    let Ok(raw) = w.ns_window() else {
        return false;
    };
    if raw.is_null() {
        return false;
    }
    unsafe {
        let nsw = raw as *mut Object;
        msg_send![nsw, isVisible]
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn pane_appkit_visible(w: &tauri::WebviewWindow) -> bool {
    w.is_visible().unwrap_or(false)
}

/// AppKit raise, ON the main thread and WAITED on.
/// `heavy` is the rail ladder (dock / Show Orbitty): activateIgnoringOtherApps
/// + orderWindow:1 relativeTo:0 so planet panels cannot bury it.
/// Chrome drawers use the light path — orderFrontRegardless + makeKey only.
/// The extra hop+activate on every Companion/Board/Settings click was the
/// beach ball on window load.
fn appkit_raise(w: &tauri::WebviewWindow, heavy: bool) {
    #[cfg(target_os = "macos")]
    {
        let w = w.clone();
        crate::main_thread_sync(move || {
            use objc::runtime::Object;
            use objc::{class, msg_send, sel, sel_impl};
            let Ok(raw) = w.ns_window() else {
                return;
            };
            if raw.is_null() {
                return;
            }
            unsafe {
                let nsw = raw as *mut Object;
                if heavy {
                    let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
                    if !ns_app.is_null() {
                        let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
                    }
                    let _: () = msg_send![nsw, orderFrontRegardless];
                    // NSWindowAbove (1) relative to 0 = front of every window
                    // at this level, so planet panels cannot keep the rail
                    // buried after a Home / disc click.
                    let _: () = msg_send![nsw, orderWindow: 1i64 relativeTo: 0i64];
                    let _: () = msg_send![nsw, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
                } else {
                    let _: () = msg_send![nsw, orderFrontRegardless];
                    let _: () = msg_send![nsw, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
                }
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (w, heavy);
}

/// Full raise for the rail (dock / Show Orbitty / ⌘⇧O).
pub(crate) fn appkit_order_front(app: &tauri::AppHandle, w: &tauri::WebviewWindow) {
    let _ = app;
    appkit_raise(w, true);
}

/// Light raise for chrome drawers. One hop, no activate, no orderWindow.
pub(crate) fn appkit_order_chrome(app: &tauri::AppHandle, w: &tauri::WebviewWindow) {
    let _ = app;
    appkit_raise(w, false);
}

/// A companion summon (tray Show Companion /
/// left-click, ⌘⇧O, dock Reopen — every one routes through show_drawer)
/// heavy-raises like the rail. FRONT-HOLD leftover: raise_now is activate
/// + level 4 + orderFront in one hop so CHROME-Z cannot leave the window
/// at 0 under the planets. Other drawers keep the light path.
fn drawer_order_front(app: &tauri::AppHandle, w: &tauri::WebviewWindow, label: &str) {
    if label == "companion" {
        crate::companion_clear::raise_now(app);
    } else {
        appkit_order_chrome(app, w);
    }
}

// Created-at stamps

/// When ensure_window actually BUILT the pane — stamped there, never on a
/// cache hit. The mount check reads this to tell a still-hydrating first
/// paint (< ~3s old) from a warm pane whose WebArea is genuinely empty
/// (that one still earns the SPA-law reload + navigate).
fn created_at_stamps(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>> {
    static STAMPS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>,
    > = std::sync::OnceLock::new();
    STAMPS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

pub(crate) fn stamp_window_created(label: &str) {
    if let Ok(mut m) = created_at_stamps().lock() {
        m.insert(label.to_string(), std::time::Instant::now());
    }
}

fn window_created_at(label: &str) -> Option<std::time::Instant> {
    created_at_stamps().lock().ok()?.get(label).copied()
}

// The SPA must actually mount

/// Labels whose SPA reported its own mount (`pane_mounted`, invoked by
/// +page.svelte after the page component rendered). Cleared when that
/// webview starts a new page load (reload / navigate), see
/// `clear_pane_mounted`. A reported mount is the cheap truth the mount
/// check needs — no AX read.
fn mounted_panes() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static S: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    S.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

fn pane_reported_mounted(label: &str) -> bool {
    mounted_panes()
        .lock()
        .map(|m| m.contains(label))
        .unwrap_or(false)
}

/// The page itself says it mounted. Cheap, no main-thread work.
#[tauri::command]
pub fn pane_mounted(webview_window: tauri::WebviewWindow) {
    if let Ok(mut m) = mounted_panes().lock() {
        m.insert(webview_window.label().to_string());
    }
}

/// A new document is loading in this webview — its old mount no longer
/// counts until the fresh page reports again.
pub(crate) fn clear_pane_mounted(label: &str) {
    if let Ok(mut m) = mounted_panes().lock() {
        m.remove(label);
    }
}

/// After a verified drawer show, the webview must grow an AXWebArea — the
/// same "is the SPA really mounted" truth Pixel reads. The empty-glass
/// failure mode (CG on=1, AXWindow with an AXGroup and no AXWebArea) was
/// invisible forever on transparent chrome; on opaque chrome it is a dead
/// slab. Check after a beat, re-check once (WKWebView vends the web area
/// lazily on the first AX query, and a cold webview mounts late), then
/// reload the webview ONCE. An AXWebArea with ZERO AX
/// children is a loaded-but-never-hydrated document — also not mounted
/// (the rail's web area always has children). A mount that still reads
/// unmounted after the reload earns ONE navigate to the bare index.html —
/// the same URL the rail actually loaded (WebviewUrl::App("index.html")
/// resolved), read off the live rail window rather than hand-parsed. A
/// cold WKWebView occasionally never starts the initial load, and a fresh
/// document is the whole lever. Never a loop, never a hide; AX-unknown
/// (untrusted / window gone) reads as mounted — never blind-reload.
fn schedule_drawer_mount_check(app: &tauri::AppHandle, label: &str) {
    #[cfg(target_os = "macos")]
    {
        if !state::DRAWER_LABELS.contains(&label) {
            return;
        }
        let handle = app.clone();
        let label = label.to_string();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(900));
            let Some(w) = handle.get_webview_window(&label) else {
                return;
            };
            if drawer_mount_ok(&w) {
                return;
            }
            // A pane built < ~3s ago is still hydrating — do NOT reload
            // mid-hydrate (beach ball). Wait out the hydrate window, then
            // re-check. Returning here used to abandon cold empty glass
            // forever (first launch after install).
            if let Some(created) = window_created_at(&label) {
                let age = created.elapsed();
                if age < std::time::Duration::from_secs(3) {
                    let wait = std::time::Duration::from_secs(3)
                        .saturating_sub(age)
                        + std::time::Duration::from_millis(500);
                    std::thread::sleep(wait);
                    if drawer_mount_ok(&w) {
                        return;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(1200));
            if drawer_mount_ok(&w) {
                return;
            }
            // Reload/navigate MUST run on the main thread. Calling wry
            // from this worker unwrap-panics (Orbitty vanishes) and left
            // empty-glass panes on screen. Catch unwind as belt+suspenders.
            eprintln!(
                "pane-show: {label} no mounted AXWebArea after show — main-thread reload once"
            );
            let w_reload = w.clone();
            let label_reload = label.clone();
            crate::main_thread_sync(move || {
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = w_reload.reload();
                }))
                .is_err()
                {
                    eprintln!(
                        "pane-show: {label_reload} main-thread reload panicked (caught)"
                    );
                }
            });
            std::thread::sleep(std::time::Duration::from_millis(1500));
            if drawer_mount_ok(&w) {
                return;
            }
            let target = handle
                .get_webview_window("main")
                .and_then(|rail| rail.url().ok())
                .filter(|u| u.query().is_none())
                .or_else(|| tauri::Url::parse("tauri://localhost/index.html").ok());
            if let Some(url) = target {
                eprintln!(
                    "pane-spa: {label} web area still empty after reload — main-thread navigate to {url} once"
                );
                let w_nav = w.clone();
                let label_nav = label.clone();
                crate::main_thread_sync(move || {
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = w_nav.navigate(url);
                    }))
                    .is_err()
                    {
                        eprintln!(
                            "pane-spa: {label_nav} main-thread navigate panicked (caught)"
                        );
                    }
                });
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, label);
}

/// Visible-and-mounted read, on the main thread and waited on. A hidden
/// pane reads mounted (nothing to reload); AX-unknown reads mounted.
/// An EMPTY AXWebArea (zero AX children) reads
/// NOT mounted — the document loaded but the SPA never hydrated.
#[cfg(target_os = "macos")]
fn drawer_mount_ok(w: &tauri::WebviewWindow) -> bool {
    // First-show beachball: the AX probe below runs ON the main thread,
    // and the first AX read of a never-shown WKWebView makes WebKit build
    // the whole page's accessibility tree while AppKit waits (warm
    // Settings = a long DOM, ~0.9s after its first show). A page that
    // already reported its own mount needs no AX read at all.
    if pane_reported_mounted(w.label()) {
        return true;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let w = w.clone();
    crate::main_thread_sync(move || {
        let ok = !pane_appkit_visible(&w) || ax_window_has_web_area(&w).unwrap_or(true);
        let _ = tx.send(ok);
    });
    rx.recv().unwrap_or(true)
}

/// Does this window's AX subtree contain a MOUNTED AXWebArea (one with AX
/// children)? None = cannot say (AX untrusted, window gone, no AX match).
/// The correlation is the one windows_mac.rs already relies on: NSWindow
/// windowNumber → AX window via the private-but-stable HIServices
/// `_AXUIElementGetWindow`.
#[cfg(target_os = "macos")]
fn ax_window_has_web_area(w: &tauri::WebviewWindow) -> Option<bool> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    use std::ffi::c_void;
    type CFRef = *const c_void;
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXUIElementCreateApplication(pid: i32) -> CFRef;
        fn AXUIElementCopyAttributeValue(
            element: CFRef,
            attribute: CFRef,
            value: *mut CFRef,
        ) -> i32;
        fn _AXUIElementGetWindow(element: CFRef, window_id: *mut u32) -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: CFRef);
        fn CFArrayGetCount(array: CFRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFRef, idx: isize) -> CFRef;
    }
    if !unsafe { AXIsProcessTrusted() } {
        return None;
    }
    let raw = w.ns_window().ok()?;
    if raw.is_null() {
        return None;
    }
    let number: i64 = unsafe { msg_send![raw as *mut Object, windowNumber] };
    if number <= 0 {
        return None;
    }
    unsafe {
        let ax_app = AXUIElementCreateApplication(std::process::id() as i32);
        if ax_app.is_null() {
            return None;
        }
        let key = CFString::new("AXWindows");
        let mut windows: CFRef = std::ptr::null();
        let err =
            AXUIElementCopyAttributeValue(ax_app, key.as_concrete_TypeRef() as CFRef, &mut windows);
        CFRelease(ax_app);
        if err != 0 || windows.is_null() {
            return None;
        }
        let mut found = None;
        let count = CFArrayGetCount(windows);
        for i in 0..count {
            let ax_win = CFArrayGetValueAtIndex(windows, i);
            if ax_win.is_null() {
                continue;
            }
            let mut id = 0u32;
            if _AXUIElementGetWindow(ax_win, &mut id) != 0 || id as i64 != number {
                continue;
            }
            found = Some(ax_subtree_has_web_area(ax_win, 0));
            break;
        }
        CFRelease(windows);
        found
    }
}

/// Depth-capped subtree walk for a MOUNTED "AXWebArea" (one with at least
/// one AX child — a zero-child web area is a document
/// that never hydrated, i.e. not mounted). Querying is itself the trigger —
/// WKWebView lazily builds its AX tree on the first AX read.
#[cfg(target_os = "macos")]
unsafe fn ax_subtree_has_web_area(el: *const std::ffi::c_void, depth: u32) -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use std::ffi::c_void;
    type CFRef = *const c_void;
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCopyAttributeValue(
            element: CFRef,
            attribute: CFRef,
            value: *mut CFRef,
        ) -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: CFRef);
        fn CFArrayGetCount(array: CFRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFRef, idx: isize) -> CFRef;
    }
    if el.is_null() || depth > 10 {
        return false;
    }
    let role_key = CFString::new("AXRole");
    let mut role: CFRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(el, role_key.as_concrete_TypeRef() as CFRef, &mut role) == 0
        && !role.is_null()
    {
        // Copy-rule result: ownership moves into the wrapper; Drop releases.
        let is_web = CFString::wrap_under_create_rule(role as _).to_string() == "AXWebArea";
        if is_web {
            // Mounted means the web area has AX
            // children. An empty one (kids=0) is a loaded document whose
            // SPA never ran — not mounted. A web area never nests another
            // web area, so this subtree is done either way.
            let kids_key = CFString::new("AXChildren");
            let mut kids: CFRef = std::ptr::null();
            if AXUIElementCopyAttributeValue(
                el,
                kids_key.as_concrete_TypeRef() as CFRef,
                &mut kids,
            ) != 0
                || kids.is_null()
            {
                return false;
            }
            let count = CFArrayGetCount(kids);
            CFRelease(kids);
            return count > 0;
        }
    }
    let children_key = CFString::new("AXChildren");
    let mut children: CFRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(el, children_key.as_concrete_TypeRef() as CFRef, &mut children)
        != 0
        || children.is_null()
    {
        return false;
    }
    let count = CFArrayGetCount(children);
    let mut found = false;
    for i in 0..count {
        let child = CFArrayGetValueAtIndex(children, i);
        if !child.is_null() && ax_subtree_has_web_area(child, depth + 1) {
            found = true;
            break;
        }
    }
    CFRelease(children);
    found
}

/// No overlap with the visibleFrame at all.
fn rect_fully_off(b: &state::CompanionBounds, vf: &VisibleFrame) -> bool {
    b.x + b.w <= vf.x || b.x >= vf.x + vf.w || b.y + b.h <= vf.y || b.y >= vf.y + vf.h
}

/// Overlap lengths (w, h) of two rects; 0 on an axis when disjoint.
fn rect_overlap(b: &state::CompanionBounds, vf: &VisibleFrame) -> (f64, f64) {
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
fn rect_mostly_on(b: &state::CompanionBounds, vf: &VisibleFrame) -> bool {
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
fn place_chrome(
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
fn rect_fully_off_all(b: &state::CompanionBounds, vfs: &[VisibleFrame]) -> bool {
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

fn remember_anchor(b: state::CompanionBounds) {
    if let Ok(mut g) = REVEALED_ANCHOR.lock() {
        *g = Some(b);
    }
}

/// The rail's rect in global CG points. A parked rail sits fully off an
/// outer edge, so placement uses the last revealed frame instead of the
/// live (off-screen) window.
fn rail_points_rect(app: &tauri::AppHandle) -> Option<state::CompanionBounds> {
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
fn occupied_drawer_rects(
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

fn tile_past_occupied(
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
fn drawer_live_off(w: &tauri::WebviewWindow) -> bool {
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
fn clamp_to_frame_keep_bottom(
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
fn centered_on_frame(vf: VisibleFrame, size: (f64, f64), cascade: f64) -> state::CompanionBounds {
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
fn place_beside_rail_display(
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
fn blossom_beside_rail(
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
static REVEALED_ANCHOR: std::sync::Mutex<Option<state::CompanionBounds>> =
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
fn drawer_slot_fallback(
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
fn pane_center_fallback(
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
fn appkit_place_points(w: &tauri::WebviewWindow, r: &state::CompanionBounds) {
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
fn appkit_place_points(w: &tauri::WebviewWindow, r: &state::CompanionBounds) {
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
    if visible && !ghosted && pane_appkit_visible(&w) && !drawer_live_off(&w) {
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

/// Companion window drag. Tauri startDragging() jumps the frame to the
/// cursor (locationInWindow 0,0). Tauri setPosition is the wrong space
/// (PANE-STAY). Follow NSEvent mouseLocation with an AppKit setFrame so
/// a click does not throw the dock into a corner.
#[derive(Clone, Copy)]
struct CompanionDragGrab {
    dx: f64,
    dy: f64,
}

fn companion_drag_grab() -> &'static std::sync::Mutex<Option<CompanionDragGrab>> {
    static S: std::sync::OnceLock<std::sync::Mutex<Option<CompanionDragGrab>>> =
        std::sync::OnceLock::new();
    S.get_or_init(|| std::sync::Mutex::new(None))
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DragPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DragSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DragRect {
    origin: DragPoint,
    size: DragSize,
}

fn pane_drag_apply(app: &tauri::AppHandle, w: &tauri::WebviewWindow, label: &str) -> bool {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    let Ok(raw) = w.ns_window() else {
        return false;
    };
    if raw.is_null() {
        return false;
    }
    unsafe {
        let nsw = raw as *mut Object;
        let buttons: usize = msg_send![class!(NSEvent), pressedMouseButtons];
        if buttons & 1 == 0 {
            if let Ok(mut g) = companion_drag_grab().lock() {
                *g = None;
            }
            let _ = w.emit("pane-drag-end", ());
            if state::DRAWER_LABELS.contains(&label) {
                persist_shown_place(app, label);
            }
            return false;
        }
        let grab = match companion_drag_grab().lock() {
            Ok(g) => *g,
            Err(_) => return false,
        };
        let Some(grab) = grab else {
            return false;
        };
        let frame: DragRect = msg_send![nsw, frame];
        let mouse: DragPoint = msg_send![class!(NSEvent), mouseLocation];
        let next = DragPoint {
            x: mouse.x - grab.dx,
            y: mouse.y - grab.dy,
        };
        // Mouse still: nothing to do (no AppKit call, no Moved event).
        if (next.x - frame.origin.x).abs() < 0.5 && (next.y - frame.origin.y).abs() < 0.5 {
            return true;
        }
        // Origin only. setFrame:display:YES forced a synchronous redraw of
        // the whole transparent WKWebView window on the main thread every
        // tick — a move needs no repaint; the window server just
        // recomposites.
        let _: () = msg_send![nsw, setFrameOrigin: next];
    }
    if label == "companion" {
        if let Ok(p) = w.outer_position() {
            drawer_expect("companion", (p.x, p.y));
        }
    }
    true
}

#[tauri::command]
pub fn companion_drag_begin(app: tauri::AppHandle) {
    pane_drag_begin(app, "companion".into());
}

/// Native AppKit follow for any labeled pane. Tauri startDragging
/// jumps; setPosition is the wrong space — that is the fake grab-hand.
#[tauri::command]
pub fn pane_drag_begin(app: tauri::AppHandle, label: String) {
    let Some(w) = app.get_webview_window(&label) else {
        return;
    };
    crate::main_thread_sync({
        let w = w.clone();
        move || {
            use objc::runtime::Object;
            use objc::{class, msg_send, sel, sel_impl};
            let Ok(raw) = w.ns_window() else {
                return;
            };
            if raw.is_null() {
                return;
            }
            unsafe {
                let nsw = raw as *mut Object;
                let frame: DragRect = msg_send![nsw, frame];
                let mouse: DragPoint = msg_send![class!(NSEvent), mouseLocation];
                if let Ok(mut g) = companion_drag_grab().lock() {
                    *g = Some(CompanionDragGrab {
                        dx: mouse.x - frame.origin.x,
                        dy: mouse.y - frame.origin.y,
                    });
                }
            }
            let _ = w.emit("pane-drag-begin", ());
        }
    });
    let app = app.clone();
    let label_loop = label.clone();
    std::thread::spawn(move || {
        // Fixed-deadline cadence (~120 Hz). The old sleep(16ms)-after-work
        // loop ran at 16ms + dispatch + setFrame ≈ 18–20ms, beating
        // against 60 Hz vsync so the pane skipped a frame every few
        // frames (the hitch). Unchanged ticks are a cheap no-op.
        const TICK: std::time::Duration = std::time::Duration::from_millis(8);
        let mut deadline = std::time::Instant::now();
        loop {
            crate::main_thread_sync({
                let w = w.clone();
                let app = app.clone();
                let label = label_loop.clone();
                move || {
                    let _ = pane_drag_apply(&app, &w, &label);
                }
            });
            let alive = companion_drag_grab()
                .lock()
                .ok()
                .and_then(|g| *g)
                .is_some();
            if !alive {
                break;
            }
            deadline += TICK;
            let now = std::time::Instant::now();
            if deadline > now {
                std::thread::sleep(deadline - now);
            } else {
                // Main thread was busy; resync instead of bursting.
                deadline = now;
            }
        }
    });
}

/// Park a WARM-created drawer on a real display while it stays hidden.
/// Same place law as show_drawer. Never show(), never pin; a pane that is
/// already visible is never parked.
pub(crate) fn park_warm_drawer(app: &tauri::AppHandle, label: &str) {
    if !state::DRAWER_LABELS.contains(&label) {
        return;
    }
    let Some(w) = app.get_webview_window(label) else {
        return;
    };
    if w.is_visible().unwrap_or(false) {
        return;
    }
    let st = state::load_state(app).unwrap_or_default();
    drawer_cache_refresh(&st);
    place_chrome(app, &w, label, &st);
}

#[cfg(test)]
mod host_dock_tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> state::CompanionBounds {
        state::CompanionBounds { x, y, w, h }
    }

    #[test]
    fn pin_keeps_bottom_center() {
        let cur = rect(100.0, 200.0, 520.0, 780.0);
        let p = pin_bottom_center(cur, 1080.0, 620.0);
        assert!((p.y + p.h - (cur.y + cur.h)).abs() < 1e-9);
        assert!((p.x + p.w / 2.0 - (cur.x + cur.w / 2.0)).abs() < 1e-9);
        assert_eq!(p.w, 1080.0);
        assert_eq!(p.h, 620.0);
    }

    #[test]
    fn clamp_keep_bottom_does_not_walk_dock() {
        let vf = VisibleFrame {
            x: 0.0,
            y: 38.0,
            w: 1512.0,
            h: 874.0,
        };
        let pinned = rect(200.0, 100.0, 1080.0, 620.0);
        let bottom = pinned.y + pinned.h;
        let c = clamp_to_frame_keep_bottom(pinned, &vf);
        assert!((c.y + c.h - bottom).abs() < 1e-9);
        assert_eq!(c.w, 1080.0);
        assert_eq!(c.h, 620.0);
    }

    #[test]
    fn beside_rail_ignores_foreign_display() {
        let primary = VisibleFrame {
            x: 0.0,
            y: 0.0,
            w: 1920.0,
            h: 1080.0,
        };
        let portrait = VisibleFrame {
            x: 1920.0,
            y: 0.0,
            w: 900.0,
            h: 1600.0,
        };
        let frames = [primary, portrait];
        let rail = rect(8.0, 80.0, 360.0, 700.0);
        let stale = rect(2400.0, 200.0, 780.0, 640.0);
        let placed = place_beside_rail_display(&rail, (640.0, 480.0), &frames, Some(&stale))
            .expect("place");
        assert!(
            rect_mostly_on(&placed, &primary),
            "placed on the rail display"
        );
        assert!(
            !rect_mostly_on(&placed, &portrait),
            "stale portrait display did not win"
        );
        let gap = placed.x - (rail.x + rail.w);
        assert!(gap <= 48.0 && gap >= -placed.w, "adjacent, gap {gap}");

        let rail_p = rect(1928.0, 40.0, 280.0, 700.0);
        let stale_primary = rect(400.0, 120.0, 780.0, 640.0);
        let on_portrait =
            place_beside_rail_display(&rail_p, (500.0, 400.0), &frames, Some(&stale_primary))
                .expect("portrait");
        assert!(rect_mostly_on(&on_portrait, &portrait));
        assert!(!rect_mostly_on(&on_portrait, &primary));
        let gap_p = on_portrait.x - (rail_p.x + rail_p.w);
        assert!(gap_p <= 48.0 && gap_p >= -on_portrait.w, "portrait gap {gap_p}");

        let tight = VisibleFrame {
            x: 0.0,
            y: 0.0,
            w: 900.0,
            h: 1600.0,
        };
        let rail_left = rect(8.0, 40.0, 360.0, 700.0);
        let crowded = place_beside_rail_display(&rail_left, (780.0, 640.0), &[tight], Some(&stale))
            .expect("crowded");
        assert!(rect_mostly_on(&crowded, &tight));
        assert!(crowded.x + crowded.w <= tight.x + tight.w + 0.5);
    }

    #[test]
    fn stage_parked_strip_spans_edge() {
        assert_eq!(stage_parked_size("left", 1440.0, 875.0), (360.0, 779.0));
        assert_eq!(stage_parked_size("right", 1440.0, 875.0), (360.0, 779.0));
        assert_eq!(stage_parked_size("top", 1440.0, 875.0), (1344.0, 420.0));
        assert_eq!(stage_parked_size("bottom", 1440.0, 875.0), (1344.0, 420.0));
        assert_eq!(stage_parked_size("left", 200.0, 100.0), (360.0, 420.0));
        assert_eq!(stage_parked_size("top", 100.0, 200.0), (360.0, 420.0));
        assert_eq!(stage_parked_size("nope", 1440.0, 875.0), (360.0, 779.0));
    }
}
