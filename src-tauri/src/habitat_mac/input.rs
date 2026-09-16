//! Mouse, hit-tests, planet/walker menus, boombox click.
//! Class registration stays on the host; selector bodies live here.
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

unsafe fn pop_context_menu(menu: *mut Object, event: *mut Object, view: *mut Object) {
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


fn planet_for_view<'a>(
    h: &'a mut Habitat,
    view_ptr: usize,
) -> Option<(&'a String, &'a mut Planet)> {
    h.planets.iter_mut().find(|(_, p)| p.view == view_ptr)
}

/// Hit order: walkers (stowed sprites) first, then dots (open windows), then
/// the planet disc, else click-through. Returns the entry id and whether the
/// being is a walker — the click matrix differs per kind.
fn hit_being(planet: &Planet, local: NSPoint) -> Option<(String, bool)> {
    // a walker hidden off the north rim (Done, waiting
    // on the rail's eviction) is not a hit target — nothing invisible
    // swallows a click.
    if let Some(b) = planet
        .walkers
        .iter()
        .find(|b| !walker_hidden(planet, b) && b.frame.inset(-3.0).contains(local))
    {
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

fn hit_decor(planet: &Planet, local: NSPoint) -> Option<DecorRef> {
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
        let (pid, panel, over, dref, being, radius, proj, board_col) = {
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
                hit_board_column(planet, local),
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
        if let Some(planet) = h.planets.get(&pid) {
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
        }
        // WORLD-TOUCH leftover (town usable): the billboard is a BOARD —
        // all three tick columns eat the mouse, never a panel-drag start
        // from the sign. Done keeps today's complete door
        // (pending_board_done → mouse-up emits habitat:board-done);
        // Todo/Doing plain clicks no-op — their status move is
        // drop-walker (drag_tick's release hit-tests the columns).
        if let Some(col) = board_col {
            h.pending_board_done = if col == 2 { Some(pid) } else { None };
            return;
        }
        h.pending_board_done = None;
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
    if let Err(p) = catch_unwind(AssertUnwindSafe(|| unsafe { mouse_up_inner(this, event) })) {
        eprintln!("habitat: mouseUp panicked: {}", panic_msg(&*p));
    }
}

/// What the click matrix decided; AX calls run after the lock is dropped.
enum BeingAction {
    /// Deminiaturize + focus (walker double), or plain focus (dot single) —
    /// both are the same AX path (`restore_window` = focus_window).
    Focus(String),
    /// Dot double: stow the open window back onto its planet.
    Stow(String),
    /// Sleeper double: wake via the claim's stamps (live → file → session →
    /// recipe → app) — a dead pid:cgid would only fail restore_window.
    Wake(String),
    /// droid double-click — no window to restore;
    /// raise the run's planet (the pane/TTY raise is HERDR-ADAPTER's).
    Planet(String),
}


unsafe fn mouse_up_inner(this: &Object, event: *mut Object) {
    unsafe {
        let loc: NSPoint = msg_send![event, locationInWindow];
        let local: NSPoint = msg_send![this, convertPoint: loc fromView: std::ptr::null::<Object>()];
        let clicks: usize = msg_send![event, clickCount];
        let view_ptr = this as *const Object as usize;
        {
            let mut h = lock();
            if let Some(pid) = h.pending_board_done.take() {
                let still = h.planets.get(&pid).is_some_and(|p| {
                    p.view == view_ptr && hit_board_column(p, local) == Some(2)
                });
                drop(h);
                if still {
                    if let Some(app) = app_handle() {
                        let _ = app.emit("habitat:board-done", pid);
                    }
                }
                return;
            }
        }
        let action = {
            let mut h = lock();
            let Some(pending) = h.pending_click.take() else {
                return;
            };
            let dragged =
                matches!(&h.drag, Some(Drag::Walker { moved, .. }) if *moved >= 4.0);
            let Some(planet) = h.planets.get_mut(&pending.project_id) else {
                return;
            };
            if planet.view != view_ptr {
                return;
            }
            let beings = if pending.walker {
                &planet.walkers
            } else {
                &planet.dots
            };
            // A walker that crossed the Done hide line between mouse-down
            // and mouse-up is off the world — the click dies with it,
            // never a focus/wake on an invisible body.
            let found = beings
                .iter()
                .find(|b| {
                    b.entry_id == pending.entry_id
                        && !(pending.walker && walker_hidden(planet, b))
                })
                .map(|b| {
                    (
                        b.frame.inset(-3.0).contains(local),
                        b.label.clone(),
                        b.asleep,
                        b.droid.is_some(),
                    )
                });
            let Some((still_inside, label, asleep, droid)) = found else {
                return;
            };
            if !still_inside {
                return;
            }
            let entry_id = pending.entry_id.clone();
            // walker single-click is the locate
            // stroke (glow only) — the Being editor moved to the walker
            // right-click menu's "Open being…". A real drag (≥4px) is not a
            // click: the walker just moved house.
            let mut locate_id: Option<String> = None;
            let action = match (pending.walker, clicks >= 2, asleep, droid) {
                // droid single — the same locate
                // stroke as a window walker; the caption names the run.
                (true, false, _, true) => {
                    set_caption(planet, format!("{label} · agent"));
                    if !dragged {
                        locate_id = Some(entry_id.clone());
                    }
                    None
                }
                // Droid double: no window to restore — front the planet
                // (the pane/TTY raise lands with HERDR-ADAPTER).
                (true, true, _, true) => {
                    Some(BeingAction::Planet(pending.project_id.clone()))
                }
                (true, false, false, _) => {
                    set_caption(planet, format!("{label} · stowed"));
                    if !dragged {
                        locate_id = Some(entry_id.clone());
                    }
                    None
                }
                // Sleeper single: caption + locate stroke — never the
                // editor, never a restore attempt on a dead window.
                (true, false, true, _) => {
                    set_caption(planet, format!("{label} · asleep — double-click to wake"));
                    if !dragged {
                        locate_id = Some(entry_id.clone());
                    }
                    None
                }
                // Walker double: restore + focus (window comes back bigger).
                (true, true, false, _) => Some(BeingAction::Focus(entry_id)),
                // Sleeper double: wake through the claim's stamps.
                (true, true, true, _) => Some(BeingAction::Wake(entry_id)),
                // Dot single: focus the open window + caption.
                (false, false, _, _) => {
                    set_caption(planet, format!("{label} · open"));
                    Some(BeingAction::Focus(entry_id))
                }
                // Dot double: stow/minimize back to the planet.
                (false, true, _, _) => Some(BeingAction::Stow(entry_id)),
            };
            if let Some(id) = locate_id {
                h.locate = Some((id, Instant::now()));
                let view = this as *const Object as *mut Object;
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
            action
        };
        match action {
            Some(BeingAction::Focus(id)) => {
                let _ = crate::windows::restore_window(&id);
            }
            Some(BeingAction::Stow(id)) => {
                let _ = crate::windows::minimize_window(&id);
            }
            Some(BeingAction::Planet(pid)) => {
                // Droid double-click: front the run's planet (cheap; the
                // pane/TTY raise is HERDR-ADAPTER's, not this phrase).
                crate::habitat::bring_planet_front(&pid);
            }
            Some(BeingAction::Wake(id)) => {
                let msg = match app_handle() {
                    Some(app) => {
                        crate::coordinator::wake_sleeper_with(app, &id).unwrap_or_else(|e| e)
                    }
                    None => "Habitat isn't ready yet — try the rail row.".into(),
                };
                // Caption the wake outcome on the sleeper's own planet.
                let mut h = lock();
                if let Some(planet) = h
                    .planets
                    .values_mut()
                    .find(|p| p.walkers.iter().any(|b| b.entry_id == id))
                {
                    set_caption(planet, msg);
                }
            }
            None => {}
        }
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

pub(crate) extern "C" fn right_mouse_down(this: &Object, _cmd: Sel, event: *mut Object) {
    if let Err(p) =
        catch_unwind(AssertUnwindSafe(|| unsafe { right_mouse_down_inner(this, event) }))
    {
        eprintln!("habitat: rightMouseDown panicked: {}", panic_msg(&*p));
    }
}

unsafe fn add_menu_item(
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

unsafe fn add_separator(menu: *mut Object) {
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
        walker_context_menu(this, event, &entry_id);
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
    let (plant_left, mush_left, build_left, has_box, beach_ball) = {
        let h = lock();
        let now = now_epoch();
        h.worlds
            .get(&project_id)
            .map(|w| {
                (
                    plant_cap(w.first_seen, now).saturating_sub(w.plants.len()),
                    plant_cap(w.first_seen, now).saturating_sub(w.mushrooms.len()),
                    building_cap(w.first_seen, now).saturating_sub(w.buildings.len()),
                    w.boombox_pos.is_some(),
                    w.beach_ball,
                )
            })
            .unwrap_or((3, 3, 1, false, false))
    };
    // Lock dropped: popUpContextMenu runs modally and the target's actions
    // re-lock on selection.
    let Some(target) = menu_target() else {
        return;
    };
    let menu: *mut Object = msg_send![class!(NSMenu), alloc];
    let menu: *mut Object = msg_send![menu, initWithTitle: ns_string("")];
    if menu.is_null() {
        return;
    }
    add_menu_item(
        menu,
        "Minimize (marble)",
        sel!(planetMinimize:),
        target,
        &project_id,
        None,
        !is_marble,
    );
    add_menu_item(
        menu,
        "Restore",
        sel!(planetRestore:),
        target,
        &project_id,
        None,
        is_marble,
    );
    add_separator(menu);
    add_menu_item(
        menu,
        &format!("Place plant ({plant_left} left)"),
        sel!(planetTogglePlant:),
        target,
        &project_id,
        None,
        plant_left > 0,
    );
    add_menu_item(
        menu,
        &format!("Place mushroom ({mush_left} left)"),
        sel!(planetToggleMushroom:),
        target,
        &project_id,
        None,
        mush_left > 0,
    );
    add_menu_item(
        menu,
        &format!("Place building ({build_left} left)"),
        sel!(planetPlaceBuilding:),
        target,
        &project_id,
        None,
        build_left > 0,
    );
    add_separator(menu);
    // live toggle — Place arms the drop-next-click
    // path, Remove clears the placed box. Always enabled. Choose music
    // stays on the disc too so a miss of the sprite still has a door.
    add_menu_item(
        menu,
        if has_box { "Remove boombox" } else { "Place boombox" },
        sel!(planetToggleBoombox:),
        target,
        &project_id,
        None,
        true,
    );
    add_menu_item(
        menu,
        "Choose music…",
        sel!(planetChooseMusic:),
        target,
        &project_id,
        None,
        true,
    );
    // per-planet beach-ball keep-up — a
    // checkable toggle persisted on the world slice next to the boombox.
    // Marbles have no walkers: no ball.
    add_menu_item(
        menu,
        "Beach ball",
        sel!(planetToggleBeachBall:),
        target,
        &project_id,
        Some(beach_ball),
        !is_marble,
    );
    let view = this as *const Object as *mut Object;
    pop_context_menu(menu, event, view);
    let _: () = msg_send![menu, release];
}


// walker right-click menu (GO-ORBITTY-RAIL-ROW-MENU)

/// What the rail does with a walker-menu pick. `kind` = import first
/// ("image" / "folder"); `pack_id` = direct pick (None = Generated).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WalkerPackEvent {
    entry_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    pack_id: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WalkerAssignEvent {
    entry_id: String,
    project_id: String,
}

fn emit_walker_pack(entry_id: String, kind: Option<&str>, pack_id: Option<String>) {
    if let Some(app) = app_handle() {
        let _ = app.emit_to(
            "main",
            "habitat:walker-pack",
            WalkerPackEvent {
                entry_id,
                kind: kind.map(|k| k.to_string()),
                pack_id,
            },
        );
    }
}

/// The walker's sprite-assignment menu: import (image/folder), Generated,
/// one item per walker pack, and the Being editor door. All verbs emit to
/// the rail — the native side never writes claims.
unsafe fn walker_context_menu(this: &Object, event: *mut Object, entry_id: &str) {
    // Current pick for the check states (raw claim value; "" = Generated).
    let cur_pack = {
        let h = lock();
        h.planets
            .values()
            .flat_map(|p| p.walkers.iter())
            .find(|b| b.entry_id == entry_id)
            .map(|b| b.sprite_pack_id.clone())
            .unwrap_or_default()
    };
    // Walker packs only — companion bodies and habitat tiles stay out.
    let packs = app_handle()
        .and_then(|app| crate::packs::list_packs(app.clone()).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| {
            p.frame_count > 0
                && !p.role.eq_ignore_ascii_case("companion")
                && !p.role.eq_ignore_ascii_case("habitat")
        })
        .collect::<Vec<_>>();
    // Lock dropped: popUpContextMenu runs modally and the target's actions
    // re-lock on selection.
    let Some(target) = menu_target() else {
        return;
    };
    let menu: *mut Object = msg_send![class!(NSMenu), alloc];
    let menu: *mut Object = msg_send![menu, initWithTitle: ns_string("")];
    if menu.is_null() {
        return;
    }
    add_menu_item(
        menu,
        "Use my image…",
        sel!(walkerUseImage:),
        target,
        entry_id,
        None,
        true,
    );
    add_menu_item(
        menu,
        "Folder…",
        sel!(walkerUseFolder:),
        target,
        entry_id,
        None,
        true,
    );
    add_separator(menu);
    // Pack list is a submenu — same shape as Assign to planet. The pick
    // rides the represented object as `entryId␟packId` (unit separator).
    let look: *mut Object = msg_send![class!(NSMenu), alloc];
    let look: *mut Object = msg_send![look, initWithTitle: ns_string("")];
    if !look.is_null() {
        add_menu_item(
            look,
            "Generated",
            sel!(walkerSetPack:),
            target,
            &format!("{entry_id}\u{1f}"),
            Some(cur_pack.is_empty()),
            true,
        );
        for p in &packs {
            add_menu_item(
                look,
                &p.name,
                sel!(walkerSetPack:),
                target,
                &format!("{entry_id}\u{1f}{}", p.id),
                Some(cur_pack == p.id),
                true,
            );
        }
        let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
        let item: *mut Object = msg_send![item,
            initWithTitle: ns_string("Look")
            action: Sel::from_ptr(std::ptr::null())
            keyEquivalent: ns_string("")];
        if !item.is_null() {
            let _: () = msg_send![item, setSubmenu: look];
            let _: () = msg_send![menu, addItem: item];
            let _: () = msg_send![item, release];
        }
        let _: () = msg_send![look, release];
    }
    add_separator(menu);
    // Assign to planet IS the rail claim — the pick
    // emits habitat:walker-assign and the rail rewrites projectId through
    // the same persist door (one windowId, one planet; the open kanban
    // card and the sprite follow the claim). No occupancy teleport.
    let names = {
        let h = lock();
        h.planet_names.clone()
    };
    if !names.is_empty() {
        let sub: *mut Object = msg_send![class!(NSMenu), alloc];
        let sub: *mut Object = msg_send![sub, initWithTitle: ns_string("")];
        if !sub.is_null() {
            for (pid, pname) in &names {
                add_menu_item(
                    sub,
                    pname,
                    sel!(walkerAssign:),
                    target,
                    &format!("{entry_id}\u{1f}{pid}"),
                    None,
                    true,
                );
            }
            let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
            let item: *mut Object = msg_send![item,
                initWithTitle: ns_string("Assign to planet")
                action: Sel::from_ptr(std::ptr::null())
                keyEquivalent: ns_string("")];
            if !item.is_null() {
                let _: () = msg_send![item, setSubmenu: sub];
                let _: () = msg_send![menu, addItem: item];
                let _: () = msg_send![item, release];
            }
            let _: () = msg_send![sub, release];
        }
        add_separator(menu);
    }
    add_menu_item(
        menu,
        "Open being…",
        sel!(walkerOpenBeing:),
        target,
        entry_id,
        None,
        true,
    );
    let view = this as *const Object as *mut Object;
    pop_context_menu(menu, event, view);
    let _: () = msg_send![menu, release];
}

pub(crate) extern "C" fn walker_use_image(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(id) = sender_project_id(sender) {
            emit_walker_pack(id, Some("image"), None);
        }
    }));
}

pub(crate) extern "C" fn walker_use_folder(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(id) = sender_project_id(sender) {
            emit_walker_pack(id, Some("folder"), None);
        }
    }));
}

pub(crate) extern "C" fn walker_set_pack(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(raw) = sender_project_id(sender) else {
            return;
        };
        let Some((entry_id, pack_id)) = raw.split_once('\u{1f}') else {
            return;
        };
        emit_walker_pack(
            entry_id.to_string(),
            None,
            if pack_id.is_empty() {
                None
            } else {
                Some(pack_id.to_string())
            },
        );
    }));
}

pub(crate) extern "C" fn walker_assign(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(raw) = sender_project_id(sender) else {
            return;
        };
        let Some((entry_id, project_id)) = raw.split_once('\u{1f}') else {
            return;
        };
        if project_id.is_empty() {
            return;
        }
        if let Some(app) = app_handle() {
            let _ = app.emit_to(
                "main",
                "habitat:walker-assign",
                WalkerAssignEvent {
                    entry_id: entry_id.to_string(),
                    project_id: project_id.to_string(),
                },
            );
        }
    }));
}

pub(crate) extern "C" fn walker_open_being(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(id) = sender_project_id(sender) {
            if let Some(app) = app_handle() {
                let _ = app.emit_to("main", "habitat:being", id);
            }
        }
    }));
}

pub(crate) extern "C" fn menu_place_building(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(id) = sender_project_id(sender) else { return; };
        let mut h = lock();
        h.placement = Some(Placement {
            project_id: id.clone(),
            kind: DecorKind::Building,
        });
        if let Some(planet) = h.planets.get_mut(&id) {
            set_caption(planet, "click to place a building".into());
        }
    }));
}

pub(crate) extern "C" fn menu_minimize(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        // The rail owns collapse state; it turns the planet into a marble on
        // the next sync.
        if let Some(id) = sender_project_id(sender) {
            if let Some(app) = app_handle() {
                let _ = app.emit_to("main", "habitat:collapse", id);
            }
        }
    }));
}

pub(crate) extern "C" fn menu_restore(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(id) = sender_project_id(sender) {
            if let Some(app) = app_handle() {
                let _ = app.emit_to("main", "habitat:expand", id);
            }
        }
    }));
}

pub(crate) extern "C" fn menu_toggle_plant(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(id) = sender_project_id(sender) else {
            return;
        };
        let mut h = lock();
        h.placement = Some(Placement {
            project_id: id.clone(),
            kind: DecorKind::Plant,
        });
        if let Some(planet) = h.planets.get_mut(&id) {
            set_caption(planet, "click to place a plant".into());
        }
    }));
}

pub(crate) extern "C" fn menu_toggle_mushroom(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(id) = sender_project_id(sender) else {
            return;
        };
        let mut h = lock();
        h.placement = Some(Placement {
            project_id: id.clone(),
            kind: DecorKind::Mushroom,
        });
        if let Some(planet) = h.planets.get_mut(&id) {
            set_caption(planet, "click to place a mushroom".into());
        }
    }));
}

/// Left-click on the placed boombox (press-release without a drag)
/// toggles the HUD — vertical volume slider + hard-mute speaker + the
/// `new track` pill (boombox_hud.rs). Not play/pause: the pick is the
/// play door, the slider's bottom end is Off, the speaker is the mute.
pub(crate) fn boombox_click(h: &mut Habitat, project_id: &str) {
    boombox_hud::toggle(h, project_id);
}

pub(crate) extern "C" fn menu_boombox_slider(_this: &Object, _cmd: Sel, sender: *mut Object) {
    // Leftover selector from the PLANETS-STRIP slider experiment. Volume
    // is the boombox HUD's vertical slider now; keep the slot so an old
    // target class registration cannot miss a method.
    let _ = sender;
}

/// GO-ORBITTY-BOOMBOX Remove: placement cleared, audio stopped, playing
/// off, HUD closed. The music pick is kept — Place boombox + a `new
/// track` pick brings it back.
fn remove_boombox(h: &mut Habitat, project_id: &str) {
    if h.boombox_hud.as_deref() == Some(project_id) {
        boombox_hud::close(h);
    }
    {
        let world = world_entry(h, project_id);
        world.boombox_pos = None;
        world.boombox = false;
    }
    h.worlds_dirty = true;
    if let Some(planet) = h.planets.get_mut(project_id) {
        planet.boombox_pos = None;
        planet.boombox = false;
        audio::stop_audio(planet);
        layout_decor(planet);
        let view = planet.view as *mut Object;
        unsafe {
            let _: () = msg_send![view, setNeedsDisplay: true];
        }
    }
}

/// Disc-menu boombox item: a live toggle, never a dead disabled row —
/// no box arms the drop-next-click placement, a placed box is removed.
pub(crate) extern "C" fn menu_toggle_boombox(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(id) = sender_project_id(sender) else {
            return;
        };
        let mut h = lock();
        let placed = h.worlds.get(&id).is_some_and(|w| w.boombox_pos.is_some());
        if placed {
            remove_boombox(&mut h, &id);
        } else {
            h.placement = Some(Placement {
                project_id: id.clone(),
                kind: DecorKind::Boombox,
            });
            if let Some(planet) = h.planets.get_mut(&id) {
                set_caption(planet, "click to place the boombox".into());
            }
        }
    }));
}

/// the planet menu's checkable beach-ball
/// toggle. Persisted on the world slice (the same worlds_dirty →
/// cursor_tick save door as the boombox); the live planet mirrors it and
/// a toggle-off hides the ball at once. Planet menu only — never
/// Settings, never the rail.
pub(crate) extern "C" fn menu_toggle_beach_ball(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(id) = sender_project_id(sender) else {
            return;
        };
        let mut h = lock();
        let on = {
            let world = world_entry(&mut h, &id);
            world.beach_ball = !world.beach_ball;
            world.beach_ball
        };
        h.worlds_dirty = true;
        if let Some(planet) = h.planets.get_mut(&id) {
            planet.beach_ball = on;
            if !on {
                planet.ball = None;
            }
            set_caption(
                planet,
                if on { "Beach ball on".into() } else { "Beach ball off".into() },
            );
        }
    }));
}

/// Unreachable since the sprite menu died (BOOMBOX leftover: click) —
/// the selector stays registered so an old target class registration
/// cannot miss a method. Now the HUD toggle, same as a sprite click.
pub(crate) extern "C" fn menu_boombox_play_pause(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(id) = sender_project_id(sender) else {
            return;
        };
        let mut h = lock();
        boombox_click(&mut h, &id);
    }));
}

pub(crate) extern "C" fn menu_remove_boombox(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(id) = sender_project_id(sender) else {
            return;
        };
        let mut h = lock();
        remove_boombox(&mut h, &id);
    }));
}

/// Volume-step pick: the represented object rides as `projectId␟step`
/// (unit separator — window titles never carry it). A playing box
/// fades the live player to the new volume — same file, no restart.
/// Unreachable since the sprite menu died (the HUD slider is the
/// volume door); the selector stays registered.
pub(crate) extern "C" fn menu_boombox_volume(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(raw) = sender_project_id(sender) else {
            return;
        };
        let (id, step) = raw.split_once('\u{1f}').unwrap_or((raw.as_str(), ""));
        let step = step.parse::<f32>().unwrap_or(1.0).clamp(0.0, 1.0);
        let mut h = lock();
        let (on, music) = {
            let world = world_entry(&mut h, id);
            world.boombox_volume = step;
            (world.boombox, world.music.clone())
        };
        h.worlds_dirty = true;
        let reduce = h.reduce_motion;
        if on {
            if let Some(planet) = h.planets.get_mut(id) {
                let live = audio::live_volume(step, planet.radius, reduce);
                audio::start_audio(planet, &music, live);
            }
        }
    }));
}
