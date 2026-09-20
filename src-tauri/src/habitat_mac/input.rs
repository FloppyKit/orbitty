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
        // BOARD-MARK: any billboard column click opens the full-page
        // kanban for THIS planet only — never companion well, never
        // Done-evict. Walker drag→column status moves stay in drag_tick.
        if board_col.is_some() {
            h.pending_board_done = None;
            h.pending_board_open = Some(pid);
            return;
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
    /// droid single-click leftover: focus herdr pane (caption already set).
    Inspect(String),
}


unsafe fn mouse_up_inner(this: &Object, event: *mut Object) {
    unsafe {
        let loc: NSPoint = msg_send![event, locationInWindow];
        let local: NSPoint = msg_send![this, convertPoint: loc fromView: std::ptr::null::<Object>()];
        let clicks: usize = msg_send![event, clickCount];
        let view_ptr = this as *const Object as usize;
        {
            let mut h = lock();
            // Stale Done-click latch: clear, never emit board-done
            // (billboard Done no longer evicts walkers).
            let _ = h.pending_board_done.take();
            if let Some(pid) = h.pending_board_open.take() {
                let still = h.planets.get(&pid).is_some_and(|p| {
                    p.view == view_ptr && hit_board_column(p, local).is_some()
                });
                if still {
                    if let Some(planet) = h.planets.get_mut(&pid) {
                        set_caption(planet, "board".into());
                    }
                }
                drop(h);
                if still {
                    if let Some(app) = app_handle() {
                        // Full-page kanban (shared pane), this planet only.
                        let _ = crate::panes::show_shared_pane(app, "board");
                        let _ = app.emit("orbitty:focus-project", pid);
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
                    let pad = if b.asleep { -10.0 } else { -3.0 };
                    (
                        b.frame.inset(pad).contains(local),
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
                    let cap = app_handle()
                        .map(|app| crate::droid_inspect::capsule(&app, &entry_id))
                        .unwrap_or_default();
                    set_caption(
                        planet,
                        if cap.is_empty() {
                            format!("{label} · agent")
                        } else {
                            format!("{label} · {cap}")
                        },
                    );
                    if !dragged {
                        locate_id = Some(entry_id.clone());
                    }
                    Some(BeingAction::Inspect(entry_id.clone()))
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
                // Sleeper single OR double: wake through the claim's
                // stamps. Camp sprites are small — requiring a double
                // made reopen feel broken. Locate still glows on single.
                (true, false, true, _) | (true, true, true, _) => {
                    if !dragged && clicks < 2 {
                        locate_id = Some(entry_id.clone());
                    }
                    Some(BeingAction::Wake(entry_id))
                }
                // Walker double: restore + focus (window comes back bigger).
                (true, true, false, _) => Some(BeingAction::Focus(entry_id)),
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
                crate::habitat::bring_planet_front(&pid);
            }
            Some(BeingAction::Inspect(id)) => {
                if let Some(app) = app_handle() {
                    crate::droid_inspect::focus(&app, &id);
                }
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

