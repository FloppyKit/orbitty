//! GO-ORBITTY-NEXT-CARD leftover: click-off.
//!
//! Tauri `onFocusChanged` does not fire on this borderless always-on-top
//! pane (same-app clicks stay inside Orbitty; desktop clicks often never
//! resign key). Poll NSEvent mouse vs the window frame — same trick as
//! companion_clear. Arm ~300ms after show so the opening pip click does
//! not immediately hide.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};
use tauri::Manager;

static ON: AtomicBool = AtomicBool::new(false);
static ARMED_AT: Mutex<Option<Instant>> = Mutex::new(None);
static APP: Mutex<Option<tauri::AppHandle>> = Mutex::new(None);

pub fn bind(app: tauri::AppHandle) {
    if let Ok(mut g) = APP.lock() {
        *g = Some(app);
    }
    start_poll();
}

pub fn watching(on: bool) {
    ON.store(on, Ordering::SeqCst);
    if let Ok(mut g) = ARMED_AT.lock() {
        *g = if on {
            Some(Instant::now() + Duration::from_millis(350))
        } else {
            None
        };
    }
}

fn start_poll() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        std::thread::spawn(|| loop {
            if !ON.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            let armed = ARMED_AT
                .lock()
                .ok()
                .and_then(|g| *g)
                .map(|t| Instant::now() >= t)
                .unwrap_or(false);
            if !armed {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            let Some(app) = APP.lock().ok().and_then(|g| g.clone()) else {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            };
            #[cfg(target_os = "macos")]
            {
                let app = app.clone();
                crate::main_thread_sync(move || {
                    if click_outside(&app) {
                        ON.store(false, Ordering::SeqCst);
                        let _ = crate::panes::hide_window(app, "next-card".to_string());
                    }
                });
            }
            std::thread::sleep(Duration::from_millis(50));
        });
    });
}

#[cfg(target_os = "macos")]
fn click_outside(app: &tauri::AppHandle) -> bool {
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
    let Some(w) = app.get_webview_window("next-card") else {
        return false;
    };
    if !w.is_visible().unwrap_or(false) {
        return false;
    }
    unsafe {
        let buttons: usize = msg_send![class!(NSEvent), pressedMouseButtons];
        if buttons & 1 == 0 {
            return false;
        }
        let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        let Ok(raw) = w.ns_window() else {
            return false;
        };
        if raw.is_null() {
            return false;
        }
        let nsw = raw as *mut Object;
        let frame: NSRect = msg_send![nsw, frame];
        let inside = mouse.x >= frame.origin.x
            && mouse.y >= frame.origin.y
            && mouse.x <= frame.origin.x + frame.size.width
            && mouse.y <= frame.origin.y + frame.size.height;
        !inside
    }
}

#[cfg(not(target_os = "macos"))]
fn click_outside(_app: &tauri::AppHandle) -> bool {
    false
}
