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

use std::sync::Mutex;

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

/// `true` → HudWindow. `false` → UnderWindowBackground.
pub fn material_for_dark(dark: bool) -> u64 {
    if dark {
        MATERIAL_HUD_WINDOW
    } else {
        MATERIAL_UNDER_WINDOW_BACKGROUND
    }
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

/// Clear, then apply one effect view when vibrancy is wanted. Main thread.
pub fn sync_one(window: &tauri::WebviewWindow) -> bool {
    let (enabled, light) = wanted();
    sync_one_os(window, enabled, light)
}

#[cfg(target_os = "macos")]
fn sync_one_os(window: &tauri::WebviewWindow, enabled: bool, light: bool) -> bool {
    if !enabled {
        imp::clear(window);
        return false;
    }
    imp::apply(
        window,
        material_for_dark(!light),
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

    pub fn clear(window: &tauri::WebviewWindow) {
        let Some(content) = content_view(window) else {
            return;
        };
        unsafe {
            // The effect view sits on the frame, behind the webview parent.
            // viewWithTag walks descendants, so the frame search still finds
            // a view that an older apply left inside the content view.
            let host: *mut Object = msg_send![content, superview];
            let root = if host.is_null() { content } else { host };
            let found: *mut Object = msg_send![root, viewWithTag: NS_VIEW_TAG];
            if !found.is_null() {
                let _: () = msg_send![found, removeFromSuperview];
            }
        }
    }

    pub fn apply(window: &tauri::WebviewWindow, material: u64, state: u64, radius: f64) -> bool {
        let version = unsafe { NSAppKitVersionNumber };
        if version < APPKIT_10_10 {
            return false;
        }
        let Some(content) = content_view(window) else {
            return false;
        };
        clear(window);
        let material = resolve_material(material);
        unsafe {
            // Phase 0: the effect view was a sibling under the webview parent,
            // wantsLayer false, and the desktop blur was not perceptible.
            // Put it on the frame, behind the webview parent, and give it a layer
            // before the material is set so behind-window sampling has a view
            // that is already in the window.
            let host: *mut Object = msg_send![content, superview];
            let (parent, anchor) = if host.is_null() {
                (content, std::ptr::null_mut())
            } else {
                (host, content)
            };
            let bounds: NSRect = msg_send![parent, bounds];
            let cls = effect_class();
            let view: *mut Object = msg_send![cls, alloc];
            if view.is_null() {
                return false;
            }
            (&mut *view).set_ivar("_orbittyTag", NS_VIEW_TAG);
            let view: *mut Object = msg_send![view, initWithFrame: bounds];
            if view.is_null() {
                return false;
            }
            let _: () = msg_send![view, setWantsLayer: true];
            // NSWindowBelow = -1. anchor is the webview parent, so the effect
            // view is behind it. nil anchor (no frame) keeps it at the back.
            let _: () = msg_send![
                parent,
                addSubview: view
                positioned: -1isize
                relativeTo: anchor
            ];
            let _: () = msg_send![view, setMaterial: material as isize];
            // NSVisualEffectBlendingModeBehindWindow = 0
            let _: () = msg_send![view, setBlendingMode: 0isize];
            let _: () = msg_send![view, setState: state as isize];
            // NSViewWidthSizable (2) | NSViewHeightSizable (16)
            let _: () = msg_send![view, setAutoresizingMask: 18usize];
            let responds: bool = msg_send![view, respondsToSelector: sel!(setCornerRadius:)];
            if responds {
                let _: () = msg_send![view, setCornerRadius: radius];
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
    use super::material_for_dark;

    #[test]
    fn material_for_dark_is_hud_or_under_window() {
        assert_eq!(material_for_dark(true), 13);
        assert_eq!(material_for_dark(false), 21);
    }
}
