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

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGColorGetNumberOfComponents(color: *mut std::ffi::c_void) -> usize;
        fn CGColorGetComponents(color: *mut std::ffi::c_void) -> *const f64;
    }

    const DIAG_MAX_NODES: usize = 320;
    const DIAG_MAX_DEPTH: usize = 8;

    struct DiagWalk {
        label: String,
        nodes: usize,
        truncated: bool,
        saw_container: bool,
        saw_titlebar: bool,
    }

    unsafe fn fmt_cgcolor(cg: *mut std::ffi::c_void) -> String {
        if cg.is_null() {
            return "none".to_string();
        }
        let n = CGColorGetNumberOfComponents(cg);
        let comps = CGColorGetComponents(cg);
        if comps.is_null() || n == 0 {
            return "empty".to_string();
        }
        if n == 2 {
            return format!("ga({:.3},{:.3})", *comps, *comps.add(1));
        }
        if n >= 4 {
            return format!(
                "rgba({:.3},{:.3},{:.3},{:.3})",
                *comps,
                *comps.add(1),
                *comps.add(2),
                *comps.add(3)
            );
        }
        let parts: Vec<String> = (0..n).map(|i| format!("{:.3}", *comps.add(i))).collect();
        format!("c{n}({})", parts.join(","))
    }

    unsafe fn fmt_nscolor(color: *mut Object) -> String {
        if color.is_null() {
            return "null".to_string();
        }
        let cg: *mut std::ffi::c_void = msg_send![color, CGColor];
        fmt_cgcolor(cg)
    }

    unsafe fn class_name(obj: *mut Object) -> String {
        if obj.is_null() {
            return "null".to_string();
        }
        let cls: *const Class = msg_send![obj, class];
        if cls.is_null() {
            return "noclass".to_string();
        }
        (*cls).name().to_string()
    }

    fn fmt_rect(r: NSRect) -> String {
        format!(
            "{:.1},{:.1},{:.1}x{:.1}",
            r.origin.x, r.origin.y, r.size.width, r.size.height
        )
    }

    fn style_bits(mask: usize) -> String {
        let flags = [
            (1usize, "titled"),
            (2, "closable"),
            (4, "miniaturizable"),
            (8, "resizable"),
            (1 << 4, "utility"),
            (1 << 6, "docModal"),
            (1 << 7, "nonactivating"),
            (1 << 8, "textured"),
            (1 << 12, "unifiedTitleToolbar"),
            (1 << 13, "hud"),
            (1 << 14, "fullScreen"),
            (1 << 15, "fullSizeContent"),
        ];
        let names: Vec<&str> = flags
            .iter()
            .filter(|(bit, _)| mask & *bit != 0)
            .map(|(_, name)| *name)
            .collect();
        if names.is_empty() {
            "borderless".to_string()
        } else {
            names.join("+")
        }
    }

    unsafe fn layer_bits(view: *mut Object) -> (bool, String, String, String, String) {
        let wants: bool = msg_send![view, wantsLayer];
        let layer: *mut Object = msg_send![view, layer];
        if layer.is_null() {
            return (
                wants,
                "none".to_string(),
                "none".to_string(),
                "none".to_string(),
                "none".to_string(),
            );
        }
        let bg: *mut std::ffi::c_void = msg_send![layer, backgroundColor];
        let opaque: bool = msg_send![layer, isOpaque];
        let border_w: f64 = msg_send![layer, borderWidth];
        let border_c: *mut std::ffi::c_void = msg_send![layer, borderColor];
        (
            wants,
            fmt_cgcolor(bg),
            if opaque { "true" } else { "false" }.to_string(),
            format!("{border_w:.2}"),
            fmt_cgcolor(border_c),
        )
    }

    unsafe fn walk_view(view: *mut Object, path: &str, depth: usize, walk: &mut DiagWalk) {
        if view.is_null() || walk.truncated {
            return;
        }
        if walk.nodes >= DIAG_MAX_NODES || depth > DIAG_MAX_DEPTH {
            walk.truncated = true;
            return;
        }
        walk.nodes += 1;
        let name = class_name(view);
        let frame: NSRect = msg_send![view, frame];
        let hidden: bool = msg_send![view, isHidden];
        let alpha: f64 = msg_send![view, alphaValue];
        let view_opaque: bool = msg_send![view, isOpaque];
        let (wants, layer_bg, layer_opaque, border_w, border_c) = layer_bits(view);
        eprintln!(
            "GLASS_FR1_DIAG view label={} path={path} class={name} frame={} hidden={hidden} alpha={alpha:.3} wantsLayer={wants} viewOpaque={view_opaque} layerBg={layer_bg} layerOpaque={layer_opaque} borderW={border_w} borderC={border_c}",
            walk.label,
            fmt_rect(frame)
        );
        if (frame.size.height > 0.0 && frame.size.height <= 2.0 && frame.size.width >= 40.0)
            || (frame.size.width > 0.0 && frame.size.width <= 2.0 && frame.size.height >= 40.0)
        {
            eprintln!(
                "GLASS_FR1_DIAG hairline label={} path={path} class={name} frame={} hidden={hidden} layerBg={layer_bg}",
                walk.label,
                fmt_rect(frame)
            );
        }
        if name == "NSTitlebarContainerView" || name == "NSTitlebarView" || name.contains("Titlebar")
        {
            if name == "NSTitlebarContainerView" {
                walk.saw_container = true;
            }
            if name == "NSTitlebarView" {
                walk.saw_titlebar = true;
            }
            eprintln!(
                "GLASS_FR1_DIAG titlebar label={} class={name} frame={} layerBg={layer_bg} hidden={hidden} alpha={alpha:.3}",
                walk.label,
                fmt_rect(frame)
            );
        }
        if name.contains("Separator") {
            eprintln!(
                "GLASS_FR1_DIAG separator label={} path={path} class={name} frame={} hidden={hidden} layerBg={layer_bg}",
                walk.label,
                fmt_rect(frame)
            );
        }
        if let Some(effect_cls) = Class::get("NSVisualEffectView") {
            let is_effect: bool = msg_send![view, isKindOfClass: effect_cls];
            if is_effect {
                let material: isize = msg_send![view, material];
                let state: isize = msg_send![view, state];
                let blending: isize = msg_send![view, blendingMode];
                eprintln!(
                    "GLASS_FR1_DIAG effect label={} path={path} class={name} material={material} state={state} blending={blending} frame={}",
                    walk.label,
                    fmt_rect(frame)
                );
            }
        }
        if let Some(wk_cls) = Class::get("WKWebView") {
            let is_wk: bool = msg_send![view, isKindOfClass: wk_cls];
            if is_wk {
                let draws_obj: *mut Object =
                    msg_send![view, valueForKey: ns_string("drawsBackground")];
                let draws: bool = if draws_obj.is_null() {
                    false
                } else {
                    msg_send![draws_obj, boolValue]
                };
                let under_ok: bool =
                    msg_send![view, respondsToSelector: sel!(underPageBackgroundColor)];
                let under = if under_ok {
                    let c: *mut Object = msg_send![view, underPageBackgroundColor];
                    fmt_nscolor(c)
                } else {
                    "unsupported".to_string()
                };
                eprintln!(
                    "GLASS_FR1_DIAG webview label={} path={path} drawsBackground={draws} underPage={under}",
                    walk.label
                );
            }
        }
        let subs: *mut Object = msg_send![view, subviews];
        if subs.is_null() {
            return;
        }
        let count: usize = msg_send![subs, count];
        for i in 0..count {
            if walk.truncated {
                return;
            }
            let sub: *mut Object = msg_send![subs, objectAtIndex: i];
            let child = if path.is_empty() {
                format!("{i}")
            } else {
                format!("{path}.{i}")
            };
            walk_view(sub, &child, depth + 1, walk);
        }
    }

    unsafe fn dump_one(window: &tauri::WebviewWindow) {
        let Some(ns_window) = ns_window(window) else {
            eprintln!(
                "GLASS_FR1_DIAG begin label={} window=missing",
                window.label()
            );
            return;
        };
        let label = window.label().to_string();
        let number: isize = msg_send![ns_window, windowNumber];
        let mask: usize = msg_send![ns_window, styleMask];
        let opaque: bool = msg_send![ns_window, isOpaque];
        let bg: *mut Object = msg_send![ns_window, backgroundColor];
        let bg_s = fmt_nscolor(bg);
        let tb_trans: bool = msg_send![ns_window, titlebarAppearsTransparent];
        let title_vis: isize = msg_send![ns_window, titleVisibility];
        let frame: NSRect = msg_send![ns_window, frame];
        let scale: f64 = msg_send![ns_window, backingScaleFactor];
        let shadow: bool = msg_send![ns_window, hasShadow];
        let movable: bool = msg_send![ns_window, isMovableByWindowBackground];
        let visible: bool = msg_send![ns_window, isVisible];
        let alpha: f64 = msg_send![ns_window, alphaValue];
        let sep_ok: bool = msg_send![ns_window, respondsToSelector: sel!(titlebarSeparatorStyle)];
        let sep: String = if sep_ok {
            let v: isize = msg_send![ns_window, titlebarSeparatorStyle];
            v.to_string()
        } else {
            "unsupported".to_string()
        };
        let screen: *mut Object = msg_send![ns_window, screen];
        let screen_s = if screen.is_null() {
            "null".to_string()
        } else {
            let sf: NSRect = msg_send![screen, frame];
            fmt_rect(sf)
        };
        let content: *mut Object = msg_send![ns_window, contentView];
        let content_frame = if content.is_null() {
            "null".to_string()
        } else {
            let r: NSRect = msg_send![content, frame];
            fmt_rect(r)
        };
        let layout_ok: bool = msg_send![ns_window, respondsToSelector: sel!(contentLayoutRect)];
        let layout_s = if layout_ok {
            let r: NSRect = msg_send![ns_window, contentLayoutRect];
            fmt_rect(r)
        } else {
            "unsupported".to_string()
        };
        eprintln!(
            "GLASS_FR1_DIAG begin label={label} windowNumber={number} styleMask=0x{mask:x} bits={} isOpaque={opaque} backgroundColor={bg_s} titlebarAppearsTransparent={tb_trans} titleVisibility={title_vis} titlebarSeparatorStyle={sep} hasShadow={shadow} movableByWindowBackground={movable} isVisible={visible} alpha={alpha:.3} backingScale={scale:.2} frame={} screen={screen_s} contentFrame={content_frame} contentLayout={layout_s} subview0=back",
            style_bits(mask),
            fmt_rect(frame)
        );
        let mut walk = DiagWalk {
            label: label.clone(),
            nodes: 0,
            truncated: false,
            saw_container: false,
            saw_titlebar: false,
        };
        let root = if content.is_null() {
            std::ptr::null_mut()
        } else {
            let parent: *mut Object = msg_send![content, superview];
            if parent.is_null() { content } else { parent }
        };
        walk_view(root, "0", 0, &mut walk);
        if !walk.saw_container {
            eprintln!("GLASS_FR1_DIAG titlebar label={label} class=NSTitlebarContainerView absent");
        }
        if !walk.saw_titlebar {
            eprintln!("GLASS_FR1_DIAG titlebar label={label} class=NSTitlebarView absent");
        }
        eprintln!(
            "GLASS_FR1_DIAG end label={label} nodes={} truncated={}",
            walk.nodes, walk.truncated
        );
        let js = r#"(function(){try{var label=(window.__TAURI_INTERNALS__&&window.__TAURI_INTERNALS__.metadata&&window.__TAURI_INTERNALS__.metadata.currentWindow&&window.__TAURI_INTERNALS__.metadata.currentWindow.label)||"";var html=document.documentElement;var hs=getComputedStyle(html);var rootSel=[".rail",".kanban",".stars",".companion",".settings",".sky",".yardwarn",".sheet",".nextwin"];var root=null,rootName="missing";for(var i=0;i<rootSel.length;i++){var el=document.querySelector(rootSel[i]);if(el){root=el;rootName=rootSel[i];break;}}var tileSel=[".k-col",".tracker-card",".ghostdock",".panel",".card",".planet-chip",".s-group"];var tile=null,tileName="missing";var scope=root||document;for(var j=0;j<tileSel.length;j++){var t=scope.querySelector(tileSel[j]);if(t){tile=t;tileName=tileSel[j];break;}}function bg(el){return el?getComputedStyle(el).backgroundColor:"missing";}return{label:label,html:bg(html),body:bg(document.body),root:rootName,rootBg:bg(root),tile:tileName,tileBg:bg(tile),theme:html.getAttribute("data-theme")||"",scheme:hs.colorScheme||"",glassSolid:html.classList.contains("glass-solid"),bg:(hs.getPropertyValue("--bg")||"").trim(),panel:(hs.getPropertyValue("--panel")||"").trim(),panelFill:(hs.getPropertyValue("--panel-fill")||"").trim(),bgFill:(hs.getPropertyValue("--bg-fill")||"").trim(),veil:(hs.getPropertyValue("--glass-veil")||"").trim()};}catch(e){return{err:"eval-failed"};}})()"#;
        if let Err(err) = window.eval_with_callback(js, move |payload| {
            eprintln!("GLASS_FR1_DIAG page {payload}");
            let _ = std::io::Write::flush(&mut std::io::stderr());
        }) {
            eprintln!("GLASS_FR1_DIAG page label={label} eval-error");
            let _ = err;
        }
        let _ = std::io::Write::flush(&mut std::io::stderr());
    }

    pub fn dump_all(app: &tauri::AppHandle) {
        use tauri::Manager;
        let conf = include_str!("../../tauri.conf.json");
        let transparent = conf.contains("\"transparent\": true");
        let private_api = conf.contains("macOSPrivateApi");
        let (enabled, light) = crate::vibrancy_mac::wanted();
        let material = crate::vibrancy_mac::material_for_dark(!light);
        eprintln!(
            "GLASS_FR1_DIAG config transparent={transparent} macOSPrivateApi={} wantedEnabled={enabled} wantedLight={light} material={material}",
            if private_api { "present" } else { "absent" }
        );
        let mut wins: Vec<_> = app.webview_windows().into_iter().collect();
        wins.sort_by(|a, b| a.0.cmp(&b.0));
        if wins.is_empty() {
            eprintln!("GLASS_FR1_DIAG windows=0");
        }
        for (label, window) in wins {
            eprintln!("GLASS_FR1_DIAG open label={label}");
            unsafe { dump_one(&window) };
        }
        let _ = std::io::Write::flush(&mut std::io::stderr());
    }

}

pub fn ensure_transparent(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    imp::ensure_transparent(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

/// One hierarchy dump per process, 2s after the first `ensure_glass`.
/// Silent unless `GLASS_FR1_DIAG=1`. Removed before the last commit.
pub fn fr1_diag_schedule(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        if std::env::var_os("GLASS_FR1_DIAG").as_deref() != Some(std::ffi::OsStr::new("1")) {
            return;
        }
        static ONCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if ONCE.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(2000));
            let app_for_dump = app.clone();
            let _ = app.run_on_main_thread(move || {
                let dumped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    imp::dump_all(&app_for_dump);
                }));
                if dumped.is_err() {
                    eprintln!("GLASS_FR1_DIAG panic");
                    let _ = std::io::Write::flush(&mut std::io::stderr());
                }
            });
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }
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

