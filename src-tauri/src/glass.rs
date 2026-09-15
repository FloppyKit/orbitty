//! S4.4 glass — make the webview windows actually show the desktop through.
//!
//! tao/wry already aim at transparency (NSWindow opaque=false + clear color,
//! drawsBackground=NO on the WKWebViewConfiguration), but the observed
//! white/grey wash says some link in that chain doesn't stick on Tahoe, so
//! this module does the surgery explicitly on the live objects:
//!   NSWindow.isOpaque = false, backgroundColor = clear
//!   WKWebView drawsBackground = NO (private KVC) + underPageBackgroundColor = clear
//! Applied at setup and re-applied on every `ensure_glass` invoke (the
//! frontend calls it on mount, after the page is up). No-op off macOS.
//!
//! The transparency surgery runs on drawers too.
//! The SPA hydrates now, so the PAINT-era opaque slab (ensure_opaque +
//! under-page paint) is retired — the CSS --bg-fill alpha (Settings
//! slider) owns the fill on every pane, same as the rail.

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
            let _: () = msg_send![ns_window, setHasShadow: true];
            let content: *mut Object = msg_send![ns_window, contentView];
            strip_webview_fill(content);
        }
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
            let _: () = msg_send![ns_window, setCollectionBehavior: 0usize];
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
