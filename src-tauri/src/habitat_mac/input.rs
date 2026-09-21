//! Mouse, hit-tests, planet/walker menus, boombox click.
//! Class registration lives in view_class.rs; selector bodies live here.
//! Boombox slop is the walker 4px. boombox_click is called from tick::drag_tick.

#![allow(unexpected_cfgs)]

use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use serde::Serialize;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;
use tauri::Emitter;

/// popUpContextMenu runs a nested loop. Arm the
/// companion poll quiet so setLevel hops cannot beach-ball the menu.
struct MenuQuiet;
impl MenuQuiet {
    fn arm() -> Self {
        crate::companion_clear::set_menu_quiet(true);
        Self
    }
}
impl Drop for MenuQuiet {
    fn drop(&mut self) {
        crate::companion_clear::set_menu_quiet(false);
    }
}

pub(crate) unsafe fn pop_context_menu(menu: *mut Object, event: *mut Object, view: *mut Object) {
    let _quiet = MenuQuiet::arm();
    let _: () = msg_send![class!(NSMenu), popUpContextMenu: menu withEvent: event forView: view];
}

use super::audio;
use super::{
    app_handle, boombox_hud, building_cap, default_state, layout_beings, layout_decor, lock,
    max_radius, menu_target, now_epoch, ns_string, panel_set_frame, panic_msg, plant_cap,
    primary_screen_id, radius_y, sender_project_id, set_caption, view_to_disc, walker_hidden,
    world_entry, DecorItem,
    draw::hit_board_column,
    DecorKind, DecorPoint, DecorRef, Drag, Habitat, NSPoint, NSRect, PendingClick, Placement,
    Planet, MIN_RADIUS, VIEW_PADDING,
};

/// press-release on the boombox
/// under this distance is a click (toggles the HUD); past it the stroke
/// is a drag — the same ≥4px law as walkers. A drag is not a click.
pub(crate) const BOOMBOX_CLICK_SLOP: f64 = 4.0;

// hit tests + mouse


pub(crate) fn planet_for_view<'a>(
    h: &'a mut Habitat,
    view_ptr: usize,
) -> Option<(&'a String, &'a mut Planet)> {
    h.planets.iter_mut().find(|(_, p)| p.view == view_ptr)
}

/// Hit order: walkers (stowed sprites) first, then dots (open windows), then
/// the planet disc, else click-through. Returns the entry id and whether the
/// being is a walker — the click matrix differs per kind.
pub(crate) fn hit_being(planet: &Planet, local: NSPoint) -> Option<(String, bool)> {
    // a walker hidden off the north rim (Done, waiting
    // on the rail's eviction) is not a hit target — nothing invisible
    // swallows a click. Sleepers get a fatter hit (−10) — the camp is a
    // tight strip next to the igloo and double-click was too fiddly.
    if let Some(b) = planet.walkers.iter().find(|b| {
        if walker_hidden(planet, b) {
            return false;
        }
        let pad = if b.asleep { -10.0 } else { -3.0 };
        b.frame.inset(pad).contains(local)
    }) {
        return Some((b.entry_id.clone(), true));
    }
    planet
        .dots
        .iter()
        .find(|b| b.frame.inset(-3.0).contains(local))
        .map(|b| (b.entry_id.clone(), false))
}

pub(crate) fn over_disc(planet: &Planet, local: NSPoint) -> bool {
    let side = (planet.radius + VIEW_PADDING) * 2.0;
    let dx = local.x - side / 2.0;
    let dy = local.y - side / 2.0;
    // marble_scale (fake Z) keeps the hit ellipse on the drawn disc.
    let rx = planet.radius * planet.marble_scale;
    let ry = radius_y(rx, planet.projection);
    if rx <= 0.0 || ry <= 0.0 {
        return false;
    }
    (dx * dx) / (rx * rx) + (dy * dy) / (ry * ry) <= 1.0
}

pub(crate) fn hit_decor(planet: &Planet, local: NSPoint) -> Option<DecorRef> {
    planet
        .decor_frames
        .iter()
        .rev()
        .find(|(_, r)| r.inset(-4.0).contains(local))
        .map(|(d, _)| *d)
}

/// Interactive region for the cursor watcher: disc + a margin, plus each
/// sprite/dot frame (they orbit outside the disc edge). While this
/// planet's boombox HUD is open its floating glass is interactive too —
/// otherwise the panel clicks through and the controls never see a down.
pub(crate) fn contains_interactive(planet: &Planet, local: NSPoint, hud_open: bool) -> bool {
    let side = (planet.radius + VIEW_PADDING) * 2.0;
    let dx = local.x - side / 2.0;
    let dy = local.y - side / 2.0;
    // marble_scale (fake Z) keeps click-through aligned with the drawn disc.
    let reach = planet.radius * planet.marble_scale + 6.0;
    if dx * dx + dy * dy <= reach * reach {
        return true;
    }
    if planet
        .walkers
        .iter()
        .filter(|b| !walker_hidden(planet, b))
        .chain(planet.dots.iter())
        .any(|b| b.frame.inset(-3.0).contains(local))
    {
        return true;
    }
    hud_open && boombox_hud::contains_hud(planet, local)
}

// mouse_down / mouse_up / scroll_wheel → mouse.rs (DIET-HABITAT)

pub(crate) extern "C" fn right_mouse_down(this: &Object, _cmd: Sel, event: *mut Object) {
    if let Err(p) =
        catch_unwind(AssertUnwindSafe(|| unsafe { right_mouse_down_inner(this, event) }))
    {
        eprintln!("habitat: rightMouseDown panicked: {}", panic_msg(&*p));
    }
}

pub(crate) unsafe fn add_menu_item(
    menu: *mut Object,
    title: &str,
    action: Sel,
    target: usize,
    project_id: &str,
    state: Option<bool>,
    enabled: bool,
) {
    let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
    let item: *mut Object = msg_send![item,
        initWithTitle: ns_string(title)
        action: action
        keyEquivalent: ns_string("")];
    if item.is_null() {
        return;
    }
    let _: () = msg_send![item, setTarget: target as *mut Object];
    let _: () = msg_send![item, setRepresentedObject: ns_string(project_id)];
    if let Some(on) = state {
        let _: () = msg_send![item, setState: if on { 1isize } else { 0isize }];
    }
    if !enabled {
        let _: () = msg_send![item, setEnabled: false];
    }
    let _: () = msg_send![menu, addItem: item];
    let _: () = msg_send![item, release];
}

pub(crate) unsafe fn add_separator(menu: *mut Object) {
    let sep: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
    let _: () = msg_send![menu, addItem: sep];
}

unsafe fn right_mouse_down_inner(this: &Object, event: *mut Object) {
    let loc: NSPoint = msg_send![event, locationInWindow];
    let local: NSPoint = msg_send![this, convertPoint: loc fromView: std::ptr::null::<Object>()];
    let view_ptr = this as *const Object as usize;
    // A right-down anywhere dismisses an open boombox HUD (click-away).
    {
        let mut h = lock();
        boombox_hud::close(&mut h);
    }
    // a walker hit gets the sprite-assignment
    // menu; dots fall through to the planet menu (no walker menu on dots).
    let (walker_hit, boombox_hit) = {
        let mut h = lock();
        let Some((_, planet)) = planet_for_view(&mut h, view_ptr) else {
            return;
        };
        // a droid has no claim, no Being editor,
        // no pack picks — its right-click is the planet menu.
        let being = hit_being(planet, local).filter(|(id, walker)| {
            !*walker
                || !planet
                    .walkers
                    .iter()
                    .any(|b| b.entry_id == *id && b.droid.is_some())
        });
        (
            being,
            hit_decor(planet, local).is_some_and(|d| d.kind == DecorKind::Boombox),
        )
    };
    if let Some((entry_id, true)) = walker_hit {
        super::walker_menu::walker_context_menu(this, event, &entry_id);
        return;
    }
    let (project_id, is_marble) = {
        let mut h = lock();
        let Some((project_id, planet)) = planet_for_view(&mut h, view_ptr) else {
            return;
        };
        if !over_disc(planet, local) {
            return;
        }
        (project_id.clone(), planet.is_marble)
    };
    // the sprite's right-click is
    // EMPTY — no menu, and it never falls through to the disc menu.
    if boombox_hit {
        return;
    }
    super::planet_menu::planet_context_menu(this, event, &project_id, is_marble);
}


// walker menu + pack/assign handlers → walker_menu.rs (DIET-HABITAT)

// planet menu handlers → planet_menu.rs (DIET-HABITAT)

pub(crate) fn boombox_click(h: &mut Habitat, project_id: &str) {
    boombox_hud::toggle(h, project_id);
}

