//! Walker frame in top-left screen points, for the peek capsule.
//!
//! `Being.frame` is view-local and flipped. Convert to window base, then
//! `NSWindow` `convertRectToScreen` (not the view). The y-flip matches
//! `live_points_rect`.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::sync::{Arc, Mutex};

use super::{lock, NSRect};
use crate::state::CompanionBounds;

pub(crate) fn walker_screen_points(entry_id: &str) -> Option<CompanionBounds> {
    let slot = Arc::new(Mutex::new(None));
    let out = slot.clone();
    let id = entry_id.to_string();
    crate::main_thread_sync(move || {
        let rect = walker_on_main(&id);
        if let Ok(mut guard) = out.lock() {
            *guard = rect;
        }
    });
    slot.lock().ok().and_then(|guard| *guard)
}

fn same_entry(have: &str, want: &str) -> bool {
    if have == want {
        return true;
    }
    let bare_have = have.strip_prefix("agent:").unwrap_or(have);
    let bare_want = want.strip_prefix("agent:").unwrap_or(want);
    !bare_have.is_empty() && bare_have == bare_want
}

fn walker_on_main(entry_id: &str) -> Option<CompanionBounds> {
    let found = {
        let habitat = lock();
        let mut hit = None;
        for planet in habitat.planets.values() {
            for being in &planet.walkers {
                if same_entry(&being.entry_id, entry_id) {
                    hit = Some((planet.view, being.frame));
                    break;
                }
            }
            if hit.is_some() {
                break;
            }
        }
        hit
    };
    let (view, frame) = found?;
    if view == 0 {
        return None;
    }
    unsafe { frame_to_points(view, frame) }
}

unsafe fn frame_to_points(view: usize, frame: NSRect) -> Option<CompanionBounds> {
    unsafe {
        let view = view as *mut Object;
        if view.is_null() {
            return None;
        }
        // `convertRectToScreen:` is NSWindow's selector. Sending it to the
        // view raises NSInvalidArgumentException; mouseUp's catch_unwind
        // then aborts because Rust cannot catch that foreign exception.
        let window: *mut Object = msg_send![view, window];
        if window.is_null() {
            return None;
        }
        let in_window: NSRect =
            msg_send![view, convertRect: frame toView: std::ptr::null::<Object>()];
        let screen: NSRect = msg_send![window, convertRectToScreen: in_window];
        let screens: *mut Object = msg_send![class!(NSScreen), screens];
        if screens.is_null() {
            return None;
        }
        let count: usize = msg_send![screens, count];
        if count == 0 {
            return None;
        }
        let primary_screen: *mut Object = msg_send![screens, objectAtIndex: 0usize];
        if primary_screen.is_null() {
            return None;
        }
        let primary: NSRect = msg_send![primary_screen, frame];
        Some(CompanionBounds {
            x: screen.origin.x,
            y: primary.size.height - (screen.origin.y + screen.size.height),
            w: screen.size.width,
            h: screen.size.height,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::frame_to_points;
    use super::NSRect;
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};
    use std::sync::atomic::{AtomicBool, Ordering};

    static VIEW_CONVERT_HIT: AtomicBool = AtomicBool::new(false);
    static WINDOW_TO_SCREEN_HIT: AtomicBool = AtomicBool::new(false);

    extern "C" fn test_window(this: &Object, _: Sel) -> *mut Object {
        let ptr: usize = unsafe { *this.get_ivar("win") };
        ptr as *mut Object
    }

    /// View-local rect → window base. `convertRectToScreen:` is not
    /// implemented here: that selector belongs to NSWindow, and sending
    /// it to the view is the walker-click abort.
    extern "C" fn test_convert_rect(_: &Object, _: Sel, rect: NSRect, _to: *mut Object) -> NSRect {
        VIEW_CONVERT_HIT.store(true, Ordering::SeqCst);
        NSRect::new(
            rect.origin.x + 1.0,
            rect.origin.y + 2.0,
            rect.size.width,
            rect.size.height,
        )
    }

    extern "C" fn test_to_screen(_: &Object, _: Sel, rect: NSRect) -> NSRect {
        WINDOW_TO_SCREEN_HIT.store(true, Ordering::SeqCst);
        NSRect::new(rect.origin.x + 5.0, rect.origin.y + 7.0, 77.0, 88.0)
    }

    fn test_class(name: &str, build: fn(&mut ClassDecl)) -> &'static Class {
        if let Some(cls) = Class::get(name) {
            return cls;
        }
        let mut decl = ClassDecl::new(name, class!(NSObject)).expect("class");
        build(&mut decl);
        decl.register()
    }

    /// Walker click → `present` → `walker_screen_points` → this conversion.
    /// The view must not be asked for `convertRectToScreen:` (NSWindow's
    /// selector). A miss raises NSInvalidArgumentException inside mouseUp's
    /// catch_unwind, and Rust aborts on the foreign exception.
    #[test]
    fn walker_click_present_converts_on_the_window_then_places() {
        VIEW_CONVERT_HIT.store(false, Ordering::SeqCst);
        WINDOW_TO_SCREEN_HIT.store(false, Ordering::SeqCst);
        let view_cls = test_class("PeekPlaceTestView", |decl| {
            decl.add_ivar::<usize>("win");
            unsafe {
                decl.add_method(
                    sel!(window),
                    test_window as extern "C" fn(&Object, Sel) -> *mut Object,
                );
                decl.add_method(
                    sel!(convertRect:toView:),
                    test_convert_rect as extern "C" fn(&Object, Sel, NSRect, *mut Object) -> NSRect,
                );
            }
        });
        let win_cls = test_class("PeekPlaceTestWindow", |decl| unsafe {
            decl.add_method(
                sel!(convertRectToScreen:),
                test_to_screen as extern "C" fn(&Object, Sel, NSRect) -> NSRect,
            );
        });
        unsafe {
            let window: *mut Object = msg_send![win_cls, new];
            let view: *mut Object = msg_send![view_cls, new];
            assert!(!window.is_null() && !view.is_null());
            (*view).set_ivar("win", window as usize);
            let points = frame_to_points(view as usize, NSRect::new(10.0, 20.0, 30.0, 40.0))
                .expect("screen points");
            assert!(VIEW_CONVERT_HIT.load(Ordering::SeqCst));
            assert!(WINDOW_TO_SCREEN_HIT.load(Ordering::SeqCst));
            assert_eq!(points.x, 16.0);
            assert_eq!(points.w, 77.0);
            assert_eq!(points.h, 88.0);
            let host = crate::drawers::VisibleFrame {
                x: points.x - 50.0,
                y: points.y - 50.0,
                w: 2000.0,
                h: 2000.0,
            };
            let placed = crate::drawers::place_beside_walker(&points, (420.0, 320.0), &[host]);
            assert!((placed.x - (points.x + points.w + 8.0)).abs() < 1e-6);
            assert!((placed.y - points.y).abs() < 1e-6);
            assert_eq!(placed.w, 420.0);
            assert_eq!(placed.h, 320.0);
        }
    }
}
