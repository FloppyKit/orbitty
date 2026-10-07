//! Force window/webview clear so CSS --bg-fill alpha is the fill.

#[cfg(target_os = "macos")]
mod imp {
    // objc 0.2 macros expand to a `cfg(feature = "cargo-clippy")` probe that
    // newer rustc flags; harmless upstream noise.
    #![allow(unexpected_cfgs)]

    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use objc::runtime::{Class, Object};
    use objc::{class, msg_send, sel, sel_impl};

    /// Toll-free-bridged NSString, intentionally leaked (used as a constant).
    unsafe fn ns_string(s: &str) -> *mut Object {
        let cf = CFString::new(s);
        let raw = cf.as_concrete_TypeRef() as *mut Object;
        std::mem::forget(cf);
        raw
    }

    /// AppKit view surgery — main thread only (callers guarantee it).
    pub fn ensure_transparent(window: &tauri::WebviewWindow) {
        let Ok(raw) = window.ns_window() else {
            return;
        };
        let ns_window = raw as *mut Object;
        if ns_window.is_null() {
            return;
        }
        unsafe {
            let _: () = msg_send![ns_window, setOpaque: false];
            let clear: *mut Object = msg_send![class!(NSColor), clearColor];
            let _: () = msg_send![ns_window, setBackgroundColor: clear];
            let main = window.label() == "main";
            let _: () = msg_send![ns_window, setHasShadow: !main];
            if main {
                let _: () = msg_send![ns_window, invalidateShadow];
            }
            let content: *mut Object = msg_send![ns_window, contentView];
            strip_webview_fill(content);
        }
    }

    /// Stage rail: a shadow on the transparent window reads as a window outline.
    pub fn no_shadow(window: &tauri::WebviewWindow) {
        let Ok(raw) = window.ns_window() else {
            return;
        };
        let ns_window = raw as *mut Object;
        if ns_window.is_null() {
            return;
        }
        unsafe {
            let _: () = msg_send![ns_window, setHasShadow: false];
            let _: () = msg_send![ns_window, invalidateShadow];
        }
        eprintln!("orbitty:main-hasShadow false");
    }

    /// No traffic lights: hide red/yellow/green on the overlay titlebar. The
    /// only close affordance is the banner × (hide rail/Lists, never quit);
    /// Quit lives on the tray menu alone. Main thread only.
    pub fn hide_traffic_lights(window: &tauri::WebviewWindow) {
        let Ok(raw) = window.ns_window() else {
            return;
        };
        let ns_window = raw as *mut Object;
        if ns_window.is_null() {
            return;
        }
        unsafe {
            // NSWindowButton: Close=0, Miniaturize=1, Zoom=2.
            for button in 0..=2usize {
                let btn: *mut Object = msg_send![ns_window, standardWindowButton: button];
                if !btn.is_null() {
                    let _: () = msg_send![btn, setHidden: true];
                }
            }
        }
    }

    /// Undo `pin_stay_visible` (the companion pins
    /// only while shown). Back to NSWindow defaults: collection behavior 0,
    /// canHide YES, hidesOnDeactivate NO. Main thread only.
    pub fn unpin_stay_visible(window: &tauri::WebviewWindow) {
        let Ok(raw) = window.ns_window() else {
            return;
        };
        let ns_window = raw as *mut Object;
        if ns_window.is_null() {
            return;
        }
        unsafe {
            // kanban / stars keep FullScreenNone (1<<9) so zoom never
            // promotes into a full-screen Space. Every other label
            // returns to behavior 0.
            let behavior: usize = if matches!(window.label(), "kanban" | "stars" | "companion") {
                crate::window_chrome::fullscreen_none_bits(0)
            } else {
                0
            };
            let _: () = msg_send![ns_window, setCollectionBehavior: behavior];
            let _: () = msg_send![ns_window, setCanHide: true];
            let _: () = msg_send![ns_window, setHidesOnDeactivate: false];
        }
    }

    /// Show Desktop survival: pin the window to every space and take it out
    /// of the hide/deactivate machinery so F11/Mission Control "Show Desktop"
    /// can't tuck the rail or Lists. canJoinAllSpaces (1) | stationary (1<<4)
    /// | fullScreenAuxiliary (1<<8) = 273, same recipe as the planet panels.
    /// Honest residual: macOS may still slide a window aside on some builds;
    /// these flags are the whole documented lever. Main thread only.
    pub fn pin_stay_visible(window: &tauri::WebviewWindow) {
        let Ok(raw) = window.ns_window() else {
            return;
        };
        let ns_window = raw as *mut Object;
        if ns_window.is_null() {
            return;
        }
        unsafe {
            let _: () = msg_send![ns_window, setCollectionBehavior: 273usize];
            let _: () = msg_send![ns_window, setCanHide: false];
            let _: () = msg_send![ns_window, setHidesOnDeactivate: false];
        }
    }

    /// Walk the view hierarchy; on the WKWebView kill the default white
    /// backing (private drawsBackground KVC + public under-page color).
    unsafe fn strip_webview_fill(view: *mut Object) {
        if view.is_null() {
            return;
        }
        if let Some(wk) = Class::get("WKWebView") {
            let is_webview: bool = msg_send![view, isKindOfClass: wk];
            if is_webview {
                let no: *mut Object = msg_send![class!(NSNumber), numberWithBool: false];
                let _: () = msg_send![view, setValue: no forKey: ns_string("drawsBackground")];
                let responds: bool =
                    msg_send![view, respondsToSelector: sel!(setUnderPageBackgroundColor:)];
                if responds {
                    let clear: *mut Object = msg_send![class!(NSColor), clearColor];
                    let _: () = msg_send![view, setUnderPageBackgroundColor: clear];
                }
            }
        }
        let subviews: *mut Object = msg_send![view, subviews];
        if subviews.is_null() {
            return;
        }
        let count: usize = msg_send![subviews, count];
        for i in 0..count {
            let sub: *mut Object = msg_send![subviews, objectAtIndex: i];
            strip_webview_fill(sub);
        }
    }

    /// Remove the titled/closable/miniaturizable bits. A titled window on
    /// macOS 26 draws a rim even with a clear background and no shadow.
    pub fn strip_titled(window: &tauri::WebviewWindow) {
        let Ok(raw) = window.ns_window() else {
            return;
        };
        let ns_window = raw as *mut Object;
        if ns_window.is_null() {
            return;
        }
        unsafe {
            let mask: usize = msg_send![ns_window, styleMask];
            const TITLED: usize = 1;
            const CLOSABLE: usize = 2;
            const MINI: usize = 4;
            const RESIZABLE: usize = 8;
            const FULL: usize = 1 << 15;
            let mut next = mask & !(TITLED | CLOSABLE | MINI);
            next |= FULL;
            if mask & RESIZABLE != 0 {
                next |= RESIZABLE;
            }
            let _: () = msg_send![ns_window, setStyleMask: next];
        }
    }

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

    fn ns_window(window: &tauri::WebviewWindow) -> Option<*mut Object> {
        let Ok(raw) = window.ns_window() else {
            return None;
        };
        if raw.is_null() {
            return None;
        }
        Some(raw as *mut Object)
    }

    unsafe fn window_frame(ns_window: *mut Object) -> NSRect {
        msg_send![ns_window, frame]
    }

    unsafe fn button(ns_window: *mut Object, which: usize) -> *mut Object {
        msg_send![ns_window, standardWindowButton: which]
    }

    /// Screen-point delta after a style-mask write. Restores the pre-mask
    /// frame when AppKit nudged it by more than 1pt.
    unsafe fn set_mask_keep_frame(ns_window: *mut Object, mask: usize) -> (f64, f64, f64, f64) {
        let before = window_frame(ns_window);
        let _: () = msg_send![ns_window, setStyleMask: mask];
        let after = window_frame(ns_window);
        let dx = after.origin.x - before.origin.x;
        let dy = after.origin.y - before.origin.y;
        let dw = after.size.width - before.size.width;
        let dh = after.size.height - before.size.height;
        let nudged = dx.abs() > 1.0 || dy.abs() > 1.0 || dw.abs() > 1.0 || dh.abs() > 1.0;
        if nudged {
            let _: () = msg_send![ns_window, setFrame: before display: false];
        }
        let now = window_frame(ns_window);
        (
            now.origin.x - before.origin.x,
            now.origin.y - before.origin.y,
            now.size.width - before.size.width,
            now.size.height - before.size.height,
        )
    }

    /// Unhide, enable and place the three buttons so their centres sit on
    /// `row_centre` (from the top of the window). The titlebar container
    /// is narrowed to the button cluster so the rest of the row hits the
    /// webview (one drag source: JS `startPaneDrag`).
    pub fn show_and_position_lights(window: &tauri::WebviewWindow, tl_x: f64, row_centre: f64) {
        let Some(ns_window) = ns_window(window) else {
            return;
        };
        unsafe {
            let _: () = msg_send![ns_window, setTitlebarAppearsTransparent: true];
            // NSWindowTitleHidden = 1
            let _: () = msg_send![ns_window, setTitleVisibility: 1usize];
            let _: () = msg_send![ns_window, setMovableByWindowBackground: false];
            let close = button(ns_window, 0);
            let mini = button(ns_window, 1);
            let zoom = button(ns_window, 2);
            if close.is_null() || mini.is_null() || zoom.is_null() {
                return;
            }
            let close_frame: NSRect = msg_send![close, frame];
            let mini_frame: NSRect = msg_send![mini, frame];
            let mut space = mini_frame.origin.x - close_frame.origin.x;
            if space < 8.0 {
                space = 20.0;
            }
            // Narrow the titlebar container BEFORE placing. setFrame on
            // the container shifts buttons that were already positioned,
            // which left centres 3px high until the next resize.
            let container: *mut Object = msg_send![close, superview];
            let container: *mut Object = if container.is_null() {
                std::ptr::null_mut()
            } else {
                msg_send![container, superview]
            };
            if !container.is_null() {
                let bounds: NSRect = msg_send![zoom, bounds];
                let zin: NSRect = msg_send![zoom, convertRect: bounds toView: container];
                let mut crect: NSRect = msg_send![container, frame];
                let width = (zin.origin.x + zin.size.width + 8.0).max(crect.size.width.min(80.0));
                if width + 1.0 < crect.size.width {
                    crect.size.width = width;
                    let _: () = msg_send![container, setFrame: crect];
                }
            }
            let win = window_frame(ns_window);
            let centre_from_bottom = win.size.height - row_centre;
            let buttons = [close, mini, zoom];
            for (i, btn) in buttons.iter().copied().enumerate() {
                let _: () = msg_send![btn, setHidden: false];
                let _: () = msg_send![btn, setEnabled: true];
                let cur: NSRect = msg_send![btn, frame];
                let bounds: NSRect = msg_send![btn, bounds];
                let in_win: NSRect =
                    msg_send![btn, convertRect: bounds toView: std::ptr::null_mut::<Object>()];
                let cur_centre = in_win.origin.y + in_win.size.height / 2.0;
                let dx = (tl_x + i as f64 * space) - in_win.origin.x;
                let dy = centre_from_bottom - cur_centre;
                let origin = NSPoint {
                    x: cur.origin.x + dx,
                    y: cur.origin.y + dy,
                };
                let _: () = msg_send![btn, setFrameOrigin: origin];
            }
        }
    }

    /// Visible-frame size in points (width, height). Public NSScreen API.
    pub fn visible_frame_size(window: &tauri::WebviewWindow) -> Option<(f64, f64)> {
        let Some(ns_window) = ns_window(window) else {
            return None;
        };
        unsafe {
            let screen: *mut Object = msg_send![ns_window, screen];
            if screen.is_null() {
                return None;
            }
            let vf: NSRect = msg_send![screen, visibleFrame];
            if vf.size.width < 1.0 || vf.size.height < 1.0 {
                return None;
            }
            Some((vf.size.width, vf.size.height))
        }
    }

    /// `orderOut:` on a later run-loop turn. Deminiaturize is async, so an
    /// immediate hide leaves the Dock tile and the later deminiaturize
    /// shows the window again.
    pub fn order_out_after(window: &tauri::WebviewWindow, delay: f64) {
        let Some(ns_window) = ns_window(window) else {
            return;
        };
        unsafe {
            let _: () = msg_send![
                ns_window,
                performSelector: sel!(orderOut:)
                withObject: std::ptr::null_mut::<Object>()
                afterDelay: delay
            ];
        }
    }

    pub fn apply_fullscreen_none(window: &tauri::WebviewWindow) {
        let Some(ns_window) = ns_window(window) else {
            return;
        };
        unsafe {
            let cur: usize = msg_send![ns_window, collectionBehavior];
            let next = crate::window_chrome::fullscreen_none_bits(cur);
            if next != cur {
                let _: () = msg_send![ns_window, setCollectionBehavior: next];
            }
        }
    }

    /// `on` → titled / closable / miniaturizable / full-size, keep
    /// resizable. `off` → borderless | resizable. Frame restored ±1pt.
    pub fn apply_companion_mask(window: &tauri::WebviewWindow, on: bool) -> (f64, f64, f64, f64) {
        let Some(ns_window) = ns_window(window) else {
            return (0.0, 0.0, 0.0, 0.0);
        };
        unsafe {
            let cur: usize = msg_send![ns_window, styleMask];
            let next = if on {
                crate::window_chrome::companion_mask_on(cur)
            } else {
                crate::window_chrome::companion_mask_off(cur)
            };
            let delta = set_mask_keep_frame(ns_window, next);
            if on {
                let _: () = msg_send![ns_window, setTitlebarAppearsTransparent: true];
                let _: () = msg_send![ns_window, setTitleVisibility: 1usize];
                let _: () = msg_send![ns_window, setMovableByWindowBackground: false];
            }
            delta
        }
    }

    pub fn apple_action_on_double_click() -> Option<String> {
        unsafe {
            let defs: *mut Object = msg_send![class!(NSUserDefaults), standardUserDefaults];
            if defs.is_null() {
                return None;
            }
            let key = ns_string("AppleActionOnDoubleClick");
            let val: *mut Object = msg_send![defs, stringForKey: key];
            if val.is_null() {
                return None;
            }
            let c: *const i8 = msg_send![val, UTF8String];
            if c.is_null() {
                return None;
            }
            Some(std::ffi::CStr::from_ptr(c).to_string_lossy().into_owned())
        }
    }

    pub fn perform_zoom(window: &tauri::WebviewWindow) {
        let Some(ns_window) = ns_window(window) else {
            return;
        };
        unsafe {
            let _: () = msg_send![ns_window, zoom: std::ptr::null_mut::<Object>()];
        }
    }

    pub fn perform_miniaturize(window: &tauri::WebviewWindow) {
        let Some(ns_window) = ns_window(window) else {
            return;
        };
        unsafe {
            let _: () = msg_send![ns_window, performMiniaturize: std::ptr::null_mut::<Object>()];
        }
    }

    pub fn is_zoomed(window: &tauri::WebviewWindow) -> bool {
        let Some(ns_window) = ns_window(window) else {
            return false;
        };
        unsafe { msg_send![ns_window, isZoomed] }
    }

    pub fn is_miniaturized(window: &tauri::WebviewWindow) -> bool {
        let Some(ns_window) = ns_window(window) else {
            return false;
        };
        unsafe { msg_send![ns_window, isMiniaturized] }
    }

    pub fn style_mask(window: &tauri::WebviewWindow) -> usize {
        let Some(ns_window) = ns_window(window) else {
            return 0;
        };
        unsafe { msg_send![ns_window, styleMask] }
    }

    pub fn collection_behavior(window: &tauri::WebviewWindow) -> usize {
        let Some(ns_window) = ns_window(window) else {
            return 0;
        };
        unsafe { msg_send![ns_window, collectionBehavior] }
    }

}

pub fn ensure_transparent(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    imp::ensure_transparent(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

/// Undo the stay-visible pin (companion hide paths).
pub fn unpin_stay_visible(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    imp::unpin_stay_visible(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub fn no_shadow(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    imp::no_shadow(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

/// Stage rail: drop the titled frame so the clear window has no rim.
/// Keeps the full-size content view (and resizable, if it was on).
pub fn stage_borderless(window: &tauri::WebviewWindow) {
    let _ = window.set_decorations(false);
    #[cfg(target_os = "macos")]
    imp::strip_titled(window);
}

pub fn hide_traffic_lights(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    imp::hide_traffic_lights(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub fn pin_stay_visible(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    imp::pin_stay_visible(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub fn show_and_position_lights(window: &tauri::WebviewWindow, tl_x: f64, row_centre: f64) {
    #[cfg(target_os = "macos")]
    imp::show_and_position_lights(window, tl_x, row_centre);
    #[cfg(not(target_os = "macos"))]
    let _ = (window, tl_x, row_centre);
}

pub fn visible_frame_size(window: &tauri::WebviewWindow) -> Option<(f64, f64)> {
    #[cfg(target_os = "macos")]
    {
        return imp::visible_frame_size(window);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        None
    }
}

pub fn order_out_after(window: &tauri::WebviewWindow, delay: f64) {
    #[cfg(target_os = "macos")]
    imp::order_out_after(window, delay);
    #[cfg(not(target_os = "macos"))]
    let _ = (window, delay);
}

pub fn apply_fullscreen_none(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    imp::apply_fullscreen_none(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

/// Restored-frame delta (dx, dy, dw, dh) in screen points.
pub fn apply_companion_mask(window: &tauri::WebviewWindow, on: bool) -> (f64, f64, f64, f64) {
    #[cfg(target_os = "macos")]
    {
        return imp::apply_companion_mask(window, on);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, on);
        (0.0, 0.0, 0.0, 0.0)
    }
}

pub fn apple_action_on_double_click() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        return imp::apple_action_on_double_click();
    }
    #[cfg(not(target_os = "macos"))]
    None
}

pub fn perform_zoom(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    imp::perform_zoom(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub fn perform_miniaturize(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    imp::perform_miniaturize(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub fn is_zoomed(window: &tauri::WebviewWindow) -> bool {
    #[cfg(target_os = "macos")]
    {
        return imp::is_zoomed(window);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        false
    }
}

pub fn is_miniaturized(window: &tauri::WebviewWindow) -> bool {
    #[cfg(target_os = "macos")]
    {
        return imp::is_miniaturized(window);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        false
    }
}

pub fn style_mask(window: &tauri::WebviewWindow) -> usize {
    #[cfg(target_os = "macos")]
    {
        return imp::style_mask(window);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        0
    }
}

pub fn collection_behavior(window: &tauri::WebviewWindow) -> usize {
    #[cfg(target_os = "macos")]
    {
        return imp::collection_behavior(window);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        0
    }
}

