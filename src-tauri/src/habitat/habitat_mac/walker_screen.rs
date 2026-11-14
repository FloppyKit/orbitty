//! Walker frame in top-left screen points, for the peek capsule.
//!
//! `Being.frame` is view-local and flipped. `convertRectToScreen` yields
//! AppKit's bottom-left screen rect; the y-flip matches `live_points_rect`.

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
        let screen: NSRect = msg_send![view, convertRectToScreen: frame];
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
