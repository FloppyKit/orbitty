// Copyright 2019-2022 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
// NOTICE: Vendored from window-vibrancy 0.6 (macOS NSVisualEffectView path only).
// Copyright 2019-2022 Tauri Programme within The Commons Conservancy.
// Translated onto objc 0.2. The Windows blur module was not copied.
// License text: third_party/window-vibrancy/

//! Dark themes use HudWindow (13). Light themes use UnderWindowBackground (21).
//! State is Active (1). The corner radius is the same 14pt the old
//! `apply_vibrancy` call passed for `main`. It is not a blur strength.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub const MATERIAL_HUD_WINDOW: u64 = 13;
pub const MATERIAL_UNDER_WINDOW_BACKGROUND: u64 = 21;
pub const STATE_ACTIVE: u64 = 1;
pub const CORNER_RADIUS: f64 = 14.0;

/// Labels `ensure_glass` already walks, plus the two real windows phase 0
/// found outside that loop.
pub const GLASS_LABELS: &[&str] = &[
    "main",
    "kanban",
    "companion",
    "stars",
    "settings",
    "sky",
    "cheatsheet",
    "yard-warn",
    "next-card",
];

/// A DOM rect in CSS pixels, top-left origin, y down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DomRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// An AppKit view frame, bottom-left origin, y up, in the content view's superview.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewFrame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Map one DOM rect into the content view's superview.
/// `content_h` is the content view's height. `content_x` / `content_y` are its frame origin.
pub fn dom_to_view_frame(dom: DomRect, content_x: f64, content_y: f64, content_h: f64) -> ViewFrame {
    ViewFrame {
        x: content_x + dom.x,
        y: content_y + content_h - dom.y - dom.h,
        w: dom.w,
        h: dom.h,
    }
}

const MAX_CARD_VIEWS: usize = 48;

fn card_rects() -> &'static Mutex<HashMap<String, Vec<DomRect>>> {
    static CARD_RECTS: OnceLock<Mutex<HashMap<String, Vec<DomRect>>>> = OnceLock::new();
    CARD_RECTS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn store_dom_rects(label: &str, mut rects: Vec<DomRect>) {
    rects.truncate(MAX_CARD_VIEWS);
    if let Ok(mut g) = card_rects().lock() {
        g.insert(label.to_string(), rects);
    }
}

pub fn dom_rects_for(label: &str) -> Vec<DomRect> {
    card_rects()
        .lock()
        .ok()
        .and_then(|g| g.get(label).cloned())
        .unwrap_or_default()
}

/// `true` → HudWindow. `false` → UnderWindowBackground.
pub fn material_for_dark(dark: bool) -> u64 {
    if dark {
        MATERIAL_HUD_WINDOW
    } else {
        MATERIAL_UNDER_WINDOW_BACKGROUND
    }
}

/// Light theme → UnderWindowBackground. Dark theme → HudWindow.
pub fn material_for(light: bool) -> u64 {
    material_for_dark(!light)
}

struct Wanted {
    enabled: bool,
    light: bool,
}

static WANTED: Mutex<Wanted> = Mutex::new(Wanted {
    enabled: true,
    light: false,
});

pub fn set_wanted(enabled: bool, light: bool) {
    if let Ok(mut g) = WANTED.lock() {
        *g = Wanted { enabled, light };
    }
}

pub fn set_light(light: bool) {
    if let Ok(mut g) = WANTED.lock() {
        g.light = light;
    }
}

pub fn wanted() -> (bool, bool) {
    match WANTED.lock() {
        Ok(g) => (g.enabled, g.light),
        Err(_) => (true, false),
    }
}

#[cfg(target_os = "macos")]
#[path = "glass_fr4_diag.rs"]
pub(crate) mod glass_fr4_diag;

/// Clear, then apply one effect view per stored card rect. Main thread.
/// An empty list leaves the window clear.
pub fn sync_one(window: &tauri::WebviewWindow) -> bool {
    let (enabled, light) = wanted();
    let applied = sync_one_os(window, enabled, light);
    #[cfg(target_os = "macos")]
    glass_fr4_diag::after_sync(window);
    applied
}

#[cfg(target_os = "macos")]
fn sync_one_os(window: &tauri::WebviewWindow, enabled: bool, light: bool) -> bool {
    // NSWorkspace, not the WKWebView media query: that query stayed false
    // while Reduce transparency was on, and the window-sized view went opaque.
    let reduce = imp::reduce_transparency();
    let flag = if reduce { "true" } else { "false" };
    let _ = window.eval(format!(
        "document.documentElement.classList.toggle('glass-rt',{flag})"
    ));
    glass_fr4_diag::note(window.label(), enabled, light, reduce, false, 0);
    if !enabled || reduce {
        imp::clear(window);
        return false;
    }
    let frames = imp::frames_for(window);
    glass_fr4_diag::note(
        window.label(),
        enabled,
        light,
        reduce,
        true,
        frames.len(),
    );
    imp::apply(
        window,
        &frames,
        material_for(light),
        STATE_ACTIVE,
        CORNER_RADIUS,
    )
}

#[cfg(not(target_os = "macos"))]
fn sync_one_os(window: &tauri::WebviewWindow, enabled: bool, light: bool) -> bool {
    let _ = (window, enabled, light);
    false
}

#[cfg(target_os = "macos")]
mod imp {
    #![allow(unexpected_cfgs)]

    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};
    use std::sync::OnceLock;

    const NS_VIEW_TAG: i64 = 91376254;
    const APPKIT_10_10: f64 = 1343.0;
    const APPKIT_10_11: f64 = 1404.0;
    const APPKIT_10_14: f64 = 1671.0;
    const APPEARANCE_BASED: u64 = 0;

    #[link(name = "AppKit", kind = "framework")]
    extern "C" {
        static NSAppKitVersionNumber: f64;
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

    extern "C" fn effect_tag(this: &Object, _: Sel) -> i64 {
        unsafe { *this.get_ivar::<i64>("_orbittyTag") }
    }

    fn effect_class() -> &'static Class {
        static CLS: OnceLock<&'static Class> = OnceLock::new();
        CLS.get_or_init(|| {
            let superclass = class!(NSVisualEffectView);
            if let Some(mut decl) = ClassDecl::new("OrbittyVisualEffectView", superclass) {
                decl.add_ivar::<i64>("_orbittyTag");
                unsafe {
                    decl.add_method(
                        sel!(tag),
                        effect_tag as extern "C" fn(&Object, Sel) -> i64,
                    );
                }
                return decl.register();
            }
            Class::get("OrbittyVisualEffectView").unwrap_or(superclass)
        })
    }

    fn resolve_material(appearance: u64) -> u64 {
        let version = unsafe { NSAppKitVersionNumber };
        if (appearance > 9 && version < APPKIT_10_14) || (appearance > 4 && version < APPKIT_10_11)
        {
            return APPEARANCE_BASED;
        }
        appearance
    }

    pub fn reduce_transparency() -> bool {
        unsafe {
            let ws: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
            if ws.is_null() {
                return false;
            }
            let ok: bool = msg_send![
                ws,
                respondsToSelector: sel!(accessibilityDisplayShouldReduceTransparency)
            ];
            if !ok {
                return false;
            }
            msg_send![ws, accessibilityDisplayShouldReduceTransparency]
        }
    }

    pub fn clear(window: &tauri::WebviewWindow) {
        let Some(content) = content_view(window) else {
            return;
        };
        unsafe {
            // Card views sit on the frame, behind the webview parent.
            // viewWithTag walks descendants and returns one, so loop until
            // none remain. A card that went away is removed here.
            let host: *mut Object = msg_send![content, superview];
            let root = if host.is_null() { content } else { host };
            loop {
                let found: *mut Object = msg_send![root, viewWithTag: NS_VIEW_TAG];
                if found.is_null() {
                    break;
                }
                let _: () = msg_send![found, removeFromSuperview];
            }
        }
    }

    pub fn frames_for(window: &tauri::WebviewWindow) -> Vec<super::ViewFrame> {
        let Some(content) = content_view(window) else {
            return Vec::new();
        };
        let dom = super::dom_rects_for(window.label());
        unsafe {
            let cf: NSRect = msg_send![content, frame];
            dom.into_iter()
                .filter(|d| d.x.is_finite() && d.y.is_finite() && d.w >= 8.0 && d.h >= 8.0)
                .map(|d| super::dom_to_view_frame(d, cf.origin.x, cf.origin.y, cf.size.height))
                .filter(|f| f.w.is_finite() && f.h.is_finite() && f.w >= 8.0 && f.h >= 8.0)
                .collect()
        }
    }

    pub fn apply(
        window: &tauri::WebviewWindow,
        frames: &[super::ViewFrame],
        material: u64,
        state: u64,
        radius: f64,
    ) -> bool {
        let version = unsafe { NSAppKitVersionNumber };
        if version < APPKIT_10_10 {
            return false;
        }
        let Some(content) = content_view(window) else {
            return false;
        };
        clear(window);
        if frames.is_empty() {
            return false;
        }
        let material = resolve_material(material);
        unsafe {
            // One effect view per card rect, on the frame, behind the webview.
            // The view frame is the blur. It does not use the window bounds.
            let host: *mut Object = msg_send![content, superview];
            let (parent, anchor) = if host.is_null() {
                (content, std::ptr::null_mut())
            } else {
                (host, content)
            };
            let cls = effect_class();
            for frame in frames {
                let bounds = NSRect {
                    origin: NSPoint {
                        x: frame.x,
                        y: frame.y,
                    },
                    size: NSSize {
                        width: frame.w,
                        height: frame.h,
                    },
                };
                let view: *mut Object = msg_send![cls, alloc];
                if view.is_null() {
                    continue;
                }
                (&mut *view).set_ivar("_orbittyTag", NS_VIEW_TAG);
                let view: *mut Object = msg_send![view, initWithFrame: bounds];
                if view.is_null() {
                    continue;
                }
                let _: () = msg_send![view, setWantsLayer: true];
                // NSWindowBelow = -1. anchor is the webview parent, so each
                // card view is behind it.
                let _: () = msg_send![
                    parent,
                    addSubview: view
                    positioned: -1isize
                    relativeTo: anchor
                ];
                // alloc/init is +1. addSubview retains. Release our claim.
                let _: () = msg_send![view, release];
                let _: () = msg_send![view, setMaterial: material as isize];
                // NSVisualEffectBlendingModeBehindWindow = 0
                let _: () = msg_send![view, setBlendingMode: 0isize];
                let _: () = msg_send![view, setState: state as isize];
                // Not sizable: a resize sends new rects. Mask 18 would fill the window.
                let _: () = msg_send![view, setAutoresizingMask: 0usize];
                let responds: bool = msg_send![view, respondsToSelector: sel!(setCornerRadius:)];
                if responds {
                    let _: () = msg_send![view, setCornerRadius: radius];
                }
            }
        }
        true
    }

    fn content_view(window: &tauri::WebviewWindow) -> Option<*mut Object> {
        let raw = window.ns_window().ok()?;
        if raw.is_null() {
            return None;
        }
        let ns_window = raw as *mut Object;
        unsafe {
            let content: *mut Object = msg_send![ns_window, contentView];
            if content.is_null() {
                None
            } else {
                Some(content)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{material_for, material_for_dark};

    #[test]
    fn material_for_dark_is_hud_or_under_window() {
        assert_eq!(material_for_dark(true), 13);
        assert_eq!(material_for_dark(false), 21);
    }

    #[test]
    fn material_for_light_is_under_window_and_dark_is_hud() {
        assert_eq!(material_for(true), 21);
        assert_eq!(material_for(false), 13);
    }

    #[test]
    fn dom_to_view_frame_flips_y() {
        use super::{dom_to_view_frame, DomRect};
        let dom = DomRect {
            x: 264.0,
            y: 24.0,
            w: 630.0,
            h: 30.0,
        };
        let frame = dom_to_view_frame(dom, 0.0, 0.0, 760.0);
        assert_eq!(frame.x, 264.0);
        assert_eq!(frame.y, 706.0);
        assert_eq!(frame.w, 630.0);
        assert_eq!(frame.h, 30.0);
        let shifted = dom_to_view_frame(dom, 10.0, 32.0, 760.0);
        assert_eq!(shifted.x, 274.0);
        assert_eq!(shifted.y, 738.0);
    }
}
