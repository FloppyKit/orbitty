//! Companion z-order + mouse. The pane is a real window: shown → mouse
//! on, never click-through. The 10Hz poll only holds NSFloatingWindowLevel+1.
//!
//! raise_now = summon (may clamp the dock on-screen). lift_z = z-order
//! only — never setFrame / activate / makeKey on a hit-mousedown (that
//! yank is the dock "running away").

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

/// Planet / walker NSMenu is tracking. Poll skips setLevel hops so a
/// nested run loop cannot beach-ball behind dispatch_sync.
static MENU_QUIET: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_menu_quiet(on: bool) {
    MENU_QUIET.store(on, Ordering::SeqCst);
}

pub(crate) fn menu_quiet() -> bool {
    MENU_QUIET.load(Ordering::SeqCst)
}

/// Expand/collapse: mouse on now, before the JS re-render.
pub(crate) fn expanded_changed(app: &tauri::AppHandle, _expanded: bool) {
    apply_mode(app);
}

/// Host on/off: mouse on now. Frame restore is panes' job.
pub(crate) fn host_changed(app: &tauri::AppHandle, _on: bool) {
    apply_mode(app);
}

/// Companion hid — drop the floating level. Next show forces mouse-on.
pub(crate) fn hidden(app: &tauri::AppHandle) {
    set_front_level(app, false);
}

/// Workspace frontmost pid. raise_now uses this to skip a redundant
/// activate; planet restack uses it to skip a lift when another app is
/// front. Shown z-order is always floating — not this test.
#[cfg(target_os = "macos")]
fn orbitty_is_frontmost() -> bool {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let ws: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if ws.is_null() {
            return false;
        }
        let front: *mut Object = msg_send![ws, frontmostApplication];
        if front.is_null() {
            return false;
        }
        let pid: i32 = msg_send![front, processIdentifier];
        pid == std::process::id() as i32
    }
}

#[cfg(not(target_os = "macos"))]
fn orbitty_is_frontmost() -> bool {
    true
}

/// Shown companion stays at normal level so other windows can cover it.
/// Hide is the same level. Summon still orders it front.
fn set_front_level(app: &tauri::AppHandle, shown: bool) {
    let handle = app.clone();
    #[cfg(target_os = "macos")]
    crate::main_thread_sync(move || {
        apply_front_level(&handle, shown);
    });
    #[cfg(not(target_os = "macos"))]
    let _ = (handle, shown);
}

#[cfg(target_os = "macos")]
fn apply_front_level(handle: &tauri::AppHandle, shown: bool) {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    let Some(w) = handle.get_webview_window("companion") else {
        return;
    };
    let Ok(raw) = w.ns_window() else {
        return;
    };
    if raw.is_null() {
        return;
    }
    let want: i64 = 0;
    let _ = shown;
    unsafe {
        let nsw = raw as *mut Object;
        let cur: i64 = msg_send![nsw, level];
        if cur != want {
            let _: () = msg_send![nsw, setLevel: want];
        }
    }
}

/// Activate (if we are not already frontmost — the extra hop on every
/// dock click was the menu beach ball) + level 4 + heavy orderFront.
/// One main-thread block so apply_mode cannot leave the window at 0
/// under the planets. ⌘⇧O / tray / dock Reopen / hit-mousedown.
/// a summon ALWAYS recovers
/// — never bail on a hidden companion (the old `!is_visible` return
/// stranded a dock-click-dropped window until Quit + relaunch). Show
/// first, then the dock law: bottom edge sunk below every display (or no
/// 24px handle anywhere) → clamp back on-screen so the dock is
/// clickable — THEN raise.
pub(crate) fn raise_now(app: &tauri::AppHandle) {
    let handle = app.clone();
    #[cfg(target_os = "macos")]
    crate::main_thread_sync(move || {
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};
        let Some(w) = handle.get_webview_window("companion") else {
            return;
        };
        if !w.is_visible().unwrap_or(false) {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_ignore_cursor_events(false);
        }
        if !crate::window_chrome::skip_companion_frame(w.is_minimized().unwrap_or(false), "raise-dock")
        {
            crate::drawers::ensure_companion_dock_onscreen(&w);
        }
        let Ok(raw) = w.ns_window() else {
            return;
        };
        if raw.is_null() {
            return;
        }
        unsafe {
            let nsw = raw as *mut Object;
            if !orbitty_is_frontmost() {
                let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
                if !ns_app.is_null() {
                    let activate = sel!(activate);
                    let responds: bool = msg_send![ns_app, respondsToSelector: activate];
                    if responds {
                        let _: () = msg_send![ns_app, activate];
                    } else {
                        let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
                    }
                }
            }
            let _: () = msg_send![nsw, setLevel: 0i64];
            let _: () = msg_send![nsw, orderFrontRegardless];
            let _: () = msg_send![nsw, orderWindow: 1i64 relativeTo: 0i64];
            let _: () = msg_send![nsw, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
        }
    });
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(w) = handle.get_webview_window("companion") {
            // Summon clamp: show a hidden companion, dock on-screen, then
            // raise (same law as the macOS block above).
            if !w.is_visible().unwrap_or(false) {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_ignore_cursor_events(false);
            }
            if !crate::window_chrome::skip_companion_frame(
                w.is_minimized().unwrap_or(false),
                "raise-dock",
            ) {
                crate::drawers::ensure_companion_dock_onscreen(&w);
            }
            crate::drawers::appkit_order_front(&handle, &w);
        }
    }
}

/// Lift z-order only. No setFrame, no activate, no makeKey. Hit-mousedown
/// and planet restack both use this — a drag starts on the same down, and
/// raise_now's clamp + makeKey yanks the pane off the cursor (the dock
/// "running away"). Lost-window rescue stays on raise_now (⌘⇧O / tray /
/// Reopen).
fn lift_z(app: &tauri::AppHandle) {
    let handle = app.clone();
    #[cfg(target_os = "macos")]
    crate::main_thread_sync(move || {
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};
        let Some(w) = handle.get_webview_window("companion") else {
            return;
        };
        if !w.is_visible().unwrap_or(false) {
            return;
        }
        let Ok(raw) = w.ns_window() else {
            return;
        };
        if raw.is_null() {
            return;
        }
        unsafe {
            let nsw = raw as *mut Object;
            let _: () = msg_send![nsw, setLevel: 0i64];
            let _: () = msg_send![nsw, orderFrontRegardless];
        }
    });
    #[cfg(not(target_os = "macos"))]
    let _ = handle;
}

/// Planet mouseDown just orderFront'd a disc at the same level. If we
/// are still the frontmost app, lift the companion back above it —
/// no activate (that beach-balls the click). Safari-front: leave it.
pub(crate) fn keep_front_if_ours(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    if !orbitty_is_frontmost() {
        return;
    }
    lift_z(app);
}

/// Shown companion is a complete window: mouse on, floating level.
/// Called on every companion show, expand/collapse, and host on/off.
pub(crate) fn apply_mode(app: &tauri::AppHandle) {
    let shown = app
        .get_webview_window("companion")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if !shown {
        return;
    }
    set_front_level(app, true);
    force_mouse_on(app);
    crate::window_chrome::sync_companion_chrome(app);
}

fn force_mouse_on(app: &tauri::AppHandle) {
    let handle = app.clone();
    #[cfg(target_os = "macos")]
    crate::main_thread_sync(move || {
        if let Some(w) = handle.get_webview_window("companion") {
            let _ = w.set_ignore_cursor_events(false);
        }
    });
    #[cfg(not(target_os = "macos"))]
    if let Some(w) = handle.get_webview_window("companion") {
        let _ = w.set_ignore_cursor_events(false);
    }
}

/// Hold the shown companion at floating level. Idles longer while hidden.
/// Nested planet menus: skip — dispatch_sync into a tracking loop was
/// the beach ball while digging into Assign-to-planet.
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        if MENU_QUIET.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }
        let shown = app
            .get_webview_window("companion")
            .map(|w| w.is_visible().unwrap_or(false))
            .unwrap_or(false);
        if shown {
            let handle = app.clone();
            #[cfg(target_os = "macos")]
            crate::main_thread_async(move || {
                apply_front_level(&handle, true);
            });
            #[cfg(not(target_os = "macos"))]
            set_front_level(&handle, true);
        }
        std::thread::sleep(std::time::Duration::from_millis(if shown { 100 } else { 300 }));
    });
}
