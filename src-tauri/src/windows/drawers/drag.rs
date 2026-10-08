use crate::state;
use tauri::{Emitter, Manager};

use super::rt::drawer_cache_refresh;
use super::rt::drawer_expect;
use super::show::persist_shown_place;
use super::place::place_chrome;

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

