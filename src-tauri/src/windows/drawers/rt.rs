//! The rail leash + drawer clamp, lifted out of
//! lib.rs (move only — behavior identical). Chrome drawers are companion /
//! kanban / settings; `sky` and the rail are never drawers.

use crate::state;
use tauri::Manager;


/// Vertical gap between stacked docked panes; also the rail→pane gap.
pub(crate) const DRAWER_GAP: f64 = 8.0;

/// Session leash runtime. `expected` is the programmatic-position ledger —
/// a short deque of our recent set_position targets per label, so a Moved
/// arriving a step late still reads as ours, while any position off that
/// path is a user drag. `drawers` is a warm copy of the persisted map so a
/// 60Hz rail drag never touches disk; every native drawer write keeps it
/// fresh.
#[derive(Default)]
pub(crate) struct DrawerRuntime {
    expected: std::collections::HashMap<String, std::collections::VecDeque<(i32, i32)>>,
    pub(crate) drawers: Option<std::collections::BTreeMap<String, state::DrawerState>>,
    /// The old-save law bit: legacy top-level `companion.bounds` exists.
    pub(crate) companion_legacy_bounds: bool,
    /// Cache holds writes not yet flushed to state.json.
    pub(crate) dirty: bool,
    /// 1s throttle for drag-frequency persists (leash stretch, plucked
    /// bounds), keyed by label.
    pub(crate) last_write: std::collections::HashMap<String, std::time::Instant>,
}

pub(crate) fn drawer_rt() -> &'static std::sync::Mutex<DrawerRuntime> {
    static RT: std::sync::OnceLock<std::sync::Mutex<DrawerRuntime>> = std::sync::OnceLock::new();
    RT.get_or_init(|| std::sync::Mutex::new(DrawerRuntime::default()))
}

/// Record a programmatic position for a drawer (capped ledger).
pub(crate) fn drawer_expect(label: &str, pos: (i32, i32)) {
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
pub(crate) struct VisibleFrame {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) w: f64,
    pub(crate) h: f64,
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
pub(crate) const DRAWER_HANDLE: f64 = 24.0;

/// Every display's visibleFrame in both spaces: Tauri physical px (per
/// the display's own scale — the same per-screen mapping outer_position
/// reports) and CG points, each points frame paired with its scale for
/// the saved-rect points retry.
#[derive(Default)]
pub(crate) struct ScreenVfs {
    physical: Vec<VisibleFrame>,
    pub(crate) points: Vec<(VisibleFrame, f64)>,
}

#[cfg(target_os = "macos")]
pub(crate) fn screen_visible_frames() -> ScreenVfs {
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
pub(crate) fn screen_visible_frames() -> ScreenVfs {
    ScreenVfs::default()
}

/// The points-space frames, scales dropped.
pub(crate) fn points_frames(screens: &ScreenVfs) -> Vec<VisibleFrame> {
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
pub(crate) fn drawer_order_front(app: &tauri::AppHandle, w: &tauri::WebviewWindow, label: &str) {
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
pub(crate) fn schedule_drawer_mount_check(app: &tauri::AppHandle, label: &str) {
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

