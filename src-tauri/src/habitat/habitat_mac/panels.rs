//! NSPanel frame helpers + screen queries (DIET-HABITAT).
//!
//! Extracted from habitat_mac/mod.rs.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

use super::world::WorldState;
use super::{
    max_radius, ns_string, EXPANDED_RADIUS, MARBLE_RADIUS, MIN_RADIUS, NSRect,
};

struct ScreenSnap {
    frames: Vec<(String, NSRect)>,
    visible: Vec<(String, NSRect)>,
}

fn snap_slot() -> &'static std::sync::Mutex<Option<ScreenSnap>> {
    static SNAP: std::sync::OnceLock<std::sync::Mutex<Option<ScreenSnap>>> = std::sync::OnceLock::new();
    SNAP.get_or_init(|| std::sync::Mutex::new(None))
}

fn display_dirty() -> &'static std::sync::atomic::AtomicBool {
    static DIRTY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
    &DIRTY
}

fn watch_displays() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGDisplayRegisterReconfigurationCallback(
                cb: extern "C" fn(u32, u32, *mut std::ffi::c_void),
                user: *mut std::ffi::c_void,
            ) -> i32;
        }
        extern "C" fn on_display(_id: u32, _flags: u32, _user: *mut std::ffi::c_void) {
            display_dirty().store(true, std::sync::atomic::Ordering::Relaxed);
        }
        unsafe {
            let _ = CGDisplayRegisterReconfigurationCallback(on_display, std::ptr::null_mut());
        }
    });
}

unsafe fn load_snap() -> ScreenSnap {
    let arr: *mut Object = msg_send![class!(NSScreen), screens];
    if arr.is_null() {
        return ScreenSnap { frames: Vec::new(), visible: Vec::new() };
    }
    let count: usize = msg_send![arr, count];
    let mut frames = Vec::with_capacity(count);
    let mut visible = Vec::with_capacity(count);
    let key = ns_string("NSScreenNumber");
    for i in 0..count {
        let s: *mut Object = msg_send![arr, objectAtIndex: i];
        if s.is_null() {
            continue;
        }
        let frame: NSRect = msg_send![s, frame];
        let vis: NSRect = msg_send![s, visibleFrame];
        let desc: *mut Object = msg_send![s, deviceDescription];
        let num: *mut Object = if desc.is_null() {
            std::ptr::null_mut()
        } else {
            msg_send![desc, objectForKey: key.as_ptr()]
        };
        let id = if num.is_null() {
            format!("screen-{:x}", s as usize)
        } else {
            let n: u32 = msg_send![num, unsignedIntValue];
            n.to_string()
        };
        visible.push((id.clone(), vis));
        frames.push((id, frame));
    }
    ScreenSnap { frames, visible }
}

unsafe fn snap() -> ScreenSnap {
    watch_displays();
    let mut slot = snap_slot().lock().unwrap_or_else(|e| e.into_inner());
    if !display_dirty().load(std::sync::atomic::Ordering::Relaxed) {
        if let Some(have) = slot.as_ref() {
            return ScreenSnap {
                frames: have.frames.clone(),
                visible: have.visible.clone(),
            };
        }
    }
    let loaded = load_snap();
    display_dirty().store(false, std::sync::atomic::Ordering::Relaxed);
    *slot = Some(ScreenSnap {
        frames: loaded.frames.clone(),
        visible: loaded.visible.clone(),
    });
    loaded
}

pub(crate) unsafe fn screens() -> Vec<(String, NSRect)> {
    snap().frames
}

/// visibleFrame excludes the menu bar / Dock — drift clamps to it so a
/// marble never wanders under OS chrome. (`screens()` keeps returning the
/// full frame; persisted coords round-trip through that.)
pub(crate) unsafe fn visible_frames() -> Vec<(String, NSRect)> {
    snap().visible
}

pub(crate) fn primary_screen_id() -> Option<String> {
    unsafe { screens().into_iter().next().map(|(id, _)| id) }
}

/// Stored coords are screen-local, flipped (y down) — clamp on-screen.
pub(crate) fn panel_frame(state: &WorldState, side: f64) -> NSRect {
    let all = unsafe { screens() };
    let fallback = NSRect::new(0.0, 0.0, 1440.0, 900.0);
    let (_, frame) = all
        .iter()
        .find(|(id, _)| id == &state.screen_id)
        .or_else(|| all.first())
        .map(|(id, f)| (id.clone(), *f))
        .unwrap_or_else(|| (String::new(), fallback));
    let lo = side / 2.0 + 8.0;
    let cx = state.x.clamp(lo, (frame.size.width - side / 2.0 - 8.0).max(lo));
    let cy = state.y.clamp(lo, (frame.size.height - side / 2.0 - 8.0).max(lo));
    NSRect::new(
        frame.origin.x + cx - side / 2.0,
        frame.origin.y + frame.size.height - cy - side / 2.0,
        side,
        side,
    )
}

/// Expanded radius: the world's scroll-zoomed value when set, else the
/// default. Marbles are always MARBLE_RADIUS regardless of zoom.
pub(crate) fn effective_radius(world: Option<&WorldState>, collapsed: bool) -> f64 {
    if collapsed {
        return MARBLE_RADIUS;
    }
    match world {
        Some(w) if w.radius > 0.0 => w.radius.clamp(MIN_RADIUS, max_radius()),
        _ => EXPANDED_RADIUS,
    }
}

pub(crate) unsafe fn make_panel() -> *mut Object {
    let panel: *mut Object = msg_send![class!(NSPanel), alloc];
    // NSWindowStyleMaskBorderless (0) | NSWindowStyleMaskNonactivatingPanel (1<<7)
    let panel: *mut Object = msg_send![panel,
        initWithContentRect: NSRect::new(0.0, 0.0, 10.0, 10.0)
        styleMask: 128usize
        backing: 2usize // NSBackingStoreBuffered
        defer: false];
    // Peer surface at the normal window level: other apps can cover the
    // planet later; it neither floats forever nor hides under everything.
    let _: () = msg_send![panel, setLevel: 0i64];
    // canJoinAllSpaces (1) | stationary (1<<4) | fullScreenAuxiliary (1<<8)
    let _: () = msg_send![panel, setCollectionBehavior: 273usize];
    let _: () = msg_send![panel, setOpaque: false];
    let clear: *mut Object = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![panel, setBackgroundColor: clear];
    let _: () = msg_send![panel, setHasShadow: false]; // draw_world paints its own
    // Click-through by default; the cursor watcher flips this off only while
    // the pointer is over the planet disc or a sprite/dot.
    let _: () = msg_send![panel, setIgnoresMouseEvents: true];
    let _: () = msg_send![panel, setAcceptsMouseMovedEvents: true];
    let _: () = msg_send![panel, setReleasedWhenClosed: false];
    let _: () = msg_send![panel, setHidesOnDeactivate: false];
    // Show Desktop stay-visible: planets are not hidable app chrome.
    let _: () = msg_send![panel, setCanHide: false];
    let _: () = msg_send![panel, setBecomesKeyOnlyIfNeeded: true];
    panel
}

pub(crate) unsafe fn panel_set_frame(panel: *mut Object, frame: NSRect, display: bool) {
    let _: () = msg_send![panel, setFrame: frame display: display];
}

