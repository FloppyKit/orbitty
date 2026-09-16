//! Leftover: click-off.
//!
//! Tauri `onFocusChanged` does not fire on this borderless always-on-top
//! pane (same-app clicks stay inside Orbitty; desktop clicks often never
//! resign key). Poll NSEvent mouse vs the window frame — same trick as
//! companion_clear. Arm ~300ms after show so the opening pip click does
//! not immediately hide.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, Once, OnceLock};
use std::time::{Duration, Instant};
use tauri::Manager;

fn labels() -> &'static Mutex<HashSet<String>> {
    static V: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    V.get_or_init(|| Mutex::new(HashSet::new()))
}

fn armed_at() -> &'static Mutex<HashMap<String, Instant>> {
    static V: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    V.get_or_init(|| Mutex::new(HashMap::new()))
}

static APP: Mutex<Option<tauri::AppHandle>> = Mutex::new(None);

pub fn bind(app: tauri::AppHandle) {
    if let Ok(mut g) = APP.lock() {
        *g = Some(app);
    }
    start_poll();
}

pub fn watching(on: bool) {
    watch("next-card", on);
}

pub fn watch(label: &str, on: bool) {
    if let Ok(mut g) = labels().lock() {
        if on {
            g.insert(label.to_string());
        } else {
            g.remove(label);
        }
    }
    if let Ok(mut g) = armed_at().lock() {
        if on {
            g.insert(
                label.to_string(),
                Instant::now() + Duration::from_millis(350),
            );
        } else {
            g.remove(label);
        }
    }
}

fn start_poll() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        std::thread::spawn(|| loop {
            let labels: Vec<String> = labels()
                .lock()
                .ok()
                .map(|g| g.iter().cloned().collect())
                .unwrap_or_default();
            if labels.is_empty() {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            let Some(app) = APP.lock().ok().and_then(|g| g.clone()) else {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            };
            #[cfg(target_os = "macos")]
            {
                let now = Instant::now();
                for label in labels {
                    let armed = armed_at()
                        .lock()
                        .ok()
                        .and_then(|g| g.get(&label).copied())
                        .map(|t| now >= t)
                        .unwrap_or(false);
                    if !armed {
                        continue;
                    }
                    let app = app.clone();
                    let lab = label.clone();
                    crate::main_thread_sync(move || {
                        if click_outside(&app, &lab) {
                            watch(&lab, false);
                            let _ = crate::panes::hide_window(app, lab);
                        }
                    });
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        });
    });
}

#[cfg(target_os = "macos")]
fn click_outside(app: &tauri::AppHandle, label: &str) -> bool {
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
    let Some(w) = app.get_webview_window(label) else {
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
