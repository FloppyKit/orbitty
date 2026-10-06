//! Habitat mouse down/up + scroll (DIET-HABITAT).
//!
//! Extracted from habitat_mac/input.rs. Hit-tests stay in input.rs.

#![allow(unexpected_cfgs)]

use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::panic::{catch_unwind, AssertUnwindSafe};

use super::audio;
use super::input::{
    contains_interactive, hit_being, hit_decor, hit_outpost_plot, over_disc, planet_for_view,
    BOOMBOX_CLICK_SLOP,
};
use super::{
    app_handle, boombox_hud, building_cap, default_state, layout_beings, layout_decor, lock,
    max_radius, now_epoch, ns_string, panel_set_frame, panic_msg, plant_cap, primary_screen_id,
    radius_y, set_caption, view_to_disc, world_entry, DecorItem,
    draw::hit_board_open, DecorKind, DecorPoint, DecorRef, Drag, Habitat, NSPoint, NSRect,
    PendingClick, Placement, Planet, MIN_RADIUS, VIEW_PADDING,
};

pub(crate) extern "C" fn mouse_down(this: &Object, _cmd: Sel, event: *mut Object) {
    // Same rule as drawRect: never let a panic cross the AppKit boundary.
    if let Err(p) = catch_unwind(AssertUnwindSafe(|| unsafe { mouse_down_inner(this, event) })) {
        eprintln!("habitat: mouseDown panicked: {}", panic_msg(&*p));
    }
}

unsafe fn mouse_down_inner(this: &Object, event: *mut Object) {
    unsafe {
        let loc: NSPoint = msg_send![event, locationInWindow];
        let local: NSPoint = msg_send![this, convertPoint: loc fromView: std::ptr::null::<Object>()];
        let view_ptr = this as *const Object as usize;
        let mut h = lock();
        let (pid, panel, over, dref, being, radius, proj) = {
            let Some((project_id, planet)) = planet_for_view(&mut h, view_ptr) else {
                return;
            };
            let panel = planet.panel as *mut Object;
            let _: () = msg_send![panel, orderFrontRegardless];
            if let Some(app) = app_handle() {
                crate::companion_clear::keep_front_if_ours(app);
            }
            (
                project_id.clone(),
                panel,
                over_disc(planet, local),
                hit_decor(planet, local),
                hit_being(planet, local),
                planet.radius,
                planet.projection,
            )
        };
        // an open boombox HUD owns
        // the next down on its planet — slider/speaker/pill eat it, the
        // backer swallows it, anything else dismisses the HUD and falls
        // through to its normal hit. The boombox sprite itself falls
        // through untouched: its press-release is the HUD toggle.
        if let Some(open_pid) = h.boombox_hud.clone() {
            let hud_hit = if open_pid == pid {
                h.planets.get(&pid).and_then(|p| boombox_hud::hit(p, local))
            } else {
                None
            };
            match hud_hit {
                Some(boombox_hud::HudHit::Slider(v)) => {
                    boombox_hud::set_volume(&mut h, &pid, v);
                    h.drag = Some(Drag::BoomboxSlider { project_id: pid });
                    return;
                }
                Some(boombox_hud::HudHit::Speaker) => {
                    boombox_hud::toggle_mute(&mut h, &pid);
                    return;
                }
                Some(boombox_hud::HudHit::Pill) => {
                    // Same picker door as the 6c6e9ae stamp: hop off the
                    // event loop, activate, runModal — never a sheet.
                    boombox_hud::close(&mut h);
                    drop(h);
                    audio::begin_choose_music(&open_pid);
                    return;
                }
                Some(boombox_hud::HudHit::Panel) => return,
                None => {
                    let on_box = open_pid == pid
                        && dref.is_some_and(|d| d.kind == DecorKind::Boombox);
                    if !on_box {
                        boombox_hud::close(&mut h);
                    }
                }
            }
        }
        if let Some((entry_id, walker)) = being {
            if walker {
                let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
                h.pending_click = Some(PendingClick {
                    project_id: pid.clone(),
                    entry_id: entry_id.clone(),
                    walker: true,
                });
                h.drag = Some(Drag::Walker {
                    project_id: pid,
                    entry_id,
                    start: mouse,
                    moved: 0.0,
                });
            } else {
                h.pending_click = Some(PendingClick {
                    project_id: pid,
                    entry_id,
                    walker: false,
                });
            }
            return;
        }
        // BOARD-MARK: sign/board beats overlapping todo trees — kanban
        // open is the preferential hit in the sign blit (and columns).
        // Trees stay draggable outside that zone. Walker drag→column
        // status moves stay in drag_tick.
        let on_board = h.planets.get(&pid).is_some_and(|p| hit_board_open(p, local));
        if on_board {
            h.pending_board_done = None;
            h.pending_board_open = Some(pid);
            return;
        }
        if let Some(planet) = h.planets.get_mut(&pid) {
            if let Some(card_id) = super::todo_tree::hit(planet, local) {
                let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
                h.drag = Some(Drag::TodoTree {
                    project_id: pid,
                    card_id,
                    start: mouse,
                    moved: 0.0,
                });
                return;
            }
            if let Some(kind) = hit_outpost_plot(planet, local) {
                super::todo_tree::enqueue_build(planet, &pid, kind);
                return;
            }
        }
        h.pending_board_done = None;
        h.pending_board_open = None;
        let place = h.placement.take();
        if let Some(place) = place {
            if place.project_id == pid && over {
                let side = (radius + VIEW_PADDING) * 2.0;
                let center = NSPoint { x: side / 2.0, y: side / 2.0 };
                let (x, y) = view_to_disc(center, radius, proj, local);
                let now = now_epoch();
                {
                    let world = world_entry(&mut h, &pid);
                    match place.kind {
                        DecorKind::Plant => {
                            if world.plants.len() < plant_cap(world.first_seen, now) {
                                world.plants.push(DecorItem { x, y, planted_at: now });
                            }
                        }
                        DecorKind::Mushroom => {
                            if world.mushrooms.len() < plant_cap(world.first_seen, now) {
                                world.mushrooms.push(DecorItem { x, y, planted_at: now });
                            }
                        }
                        DecorKind::Building => {
                            if world.buildings.len() < building_cap(world.first_seen, now) {
                                world.buildings.push(DecorItem { x, y, planted_at: now });
                            }
                        }
                        DecorKind::Boombox => {
                            if world.boombox_pos.is_none() {
                                world.boombox_pos = Some(DecorPoint { x, y });
                            }
                        }
                    }
                }
                h.worlds_dirty = true;
                let (plants, mushrooms, buildings, boom) = h
                    .worlds
                    .get(&pid)
                    .map(|w| {
                        (
                            w.plants.clone(),
                            w.mushrooms.clone(),
                            w.buildings.clone(),
                            w.boombox_pos,
                        )
                    })
                    .unwrap_or_default();
                if let Some(planet) = h.planets.get_mut(&pid) {
                    planet.plants = plants;
                    planet.mushrooms = mushrooms;
                    planet.buildings = buildings;
                    planet.boombox_pos = boom;
                    layout_decor(planet);
                    set_caption(planet, "placed".into());
                }
                return;
            }
        }
        if let Some(dref) = dref {
            let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
            h.drag = Some(Drag::Decor {
                project_id: pid,
                decor: dref,
                start: mouse,
                moved: 0.0,
            });
            return;
        }
        if over {
            let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
            let frame: NSRect = msg_send![panel, frame];
            h.drag = Some(Drag::Panel {
                project_id: pid,
                grab_dx: mouse.x - frame.mid_x(),
                grab_dy: mouse.y - frame.mid_y(),
                start: mouse,
                moved: 0.0,
            });
        }
    }
}

pub(crate) extern "C" fn mouse_up(this: &Object, _cmd: Sel, event: *mut Object) {
    if let Err(p) = catch_unwind(AssertUnwindSafe(|| unsafe { super::mouse_up::mouse_up_inner(this, event) })) {
        eprintln!("habitat: mouseUp panicked: {}", panic_msg(&*p));
    }
}

// scroll zoom + right-click menu

pub(crate) extern "C" fn scroll_wheel(this: &Object, _cmd: Sel, event: *mut Object) {
    if let Err(p) = catch_unwind(AssertUnwindSafe(|| unsafe { scroll_wheel_inner(this, event) })) {
        eprintln!("habitat: scrollWheel panicked: {}", panic_msg(&*p));
    }
}

/// Scroll over the disc zooms the planet radius (clamped, persisted in
/// worlds.json). Marbles ignore scroll — click expands first.
unsafe fn scroll_wheel_inner(this: &Object, event: *mut Object) {
    let loc: NSPoint = msg_send![event, locationInWindow];
    let local: NSPoint = msg_send![this, convertPoint: loc fromView: std::ptr::null::<Object>()];
    let mut delta: f64 = msg_send![event, scrollingDeltaY];
    let precise: bool = msg_send![event, hasPreciseScrollingDeltas];
    if !precise {
        delta *= 4.0;
    }
    if delta == 0.0 {
        return;
    }
    let view_ptr = this as *const Object as usize;
    let mut h = lock();
    let applied = {
        let Some((project_id, planet)) = planet_for_view(&mut h, view_ptr) else {
            return;
        };
        if planet.is_marble || !over_disc(planet, local) {
            return;
        }
        let next = (planet.radius + delta * 0.6).clamp(MIN_RADIUS, max_radius());
        if (next - planet.radius).abs() < 0.01 {
            return;
        }
        planet.radius = next;
        // Resize around the center, same recipe as the marble ↔ planet swap.
        let side = (next + VIEW_PADDING) * 2.0;
        let panel = planet.panel as *mut Object;
        let view = planet.view as *mut Object;
        let frame: NSRect = msg_send![panel, frame];
        panel_set_frame(
            panel,
            NSRect::new(
                frame.mid_x() - side / 2.0,
                frame.mid_y() - side / 2.0,
                side,
                side,
            ),
            true,
        );
        let _: () = msg_send![view, setFrame: NSRect::new(0.0, 0.0, side, side)];
        layout_beings(planet);
        let _: () = msg_send![view, setNeedsDisplay: true];
        Some((project_id.clone(), next))
    };
    let Some((project_id, radius)) = applied else {
        return;
    };
    let world = h
        .worlds
        .entry(project_id)
        .or_insert_with(|| default_state(&primary_screen_id().unwrap_or_default(), 0));
    world.radius = radius;
    world.last_used = now_epoch();
    h.worlds_dirty = true;
}

