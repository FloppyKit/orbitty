//! Planet / decor / walker / boombox / todo-tree drag stroke (DIET-HABITAT).
//!
//! Extracted from habitat_mac/tick.rs. Called every 16ms habitat tick hop.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::time::{Duration, Instant};
use tauri::Emitter;

use super::boombox_hud;
use super::input::{boombox_click, BOOMBOX_CLICK_SLOP};
use super::{
    app_handle, default_state, emit_board_checkin, layout_beings, layout_decor, lock, now_epoch,
    save_worlds, screens, view_to_disc, DecorKind, DecorPoint, Drag, Habitat, NSPoint, NSRect,
    DRIFT_PIN_SECS, VIEW_PADDING,
};

pub(crate) fn drag_tick() {
    let mut h = lock();
    let Some(d) = h.drag.as_mut() else {
        return;
    };
    unsafe {
        let buttons: usize = msg_send![class!(NSEvent), pressedMouseButtons];
        if buttons & 1 == 0 {
            let (project_id, moved, kind, entry_id) = match d {
                Drag::Panel { project_id, moved, .. } => {
                    (project_id.clone(), *moved, 0u8, String::new())
                }
                // Boombox decor gets its own code: a press-release that
                // never moved is a click (HUD toggle), not a placement.
                Drag::Decor { project_id, decor, moved, .. } => (
                    project_id.clone(),
                    *moved,
                    if decor.kind == DecorKind::Boombox { 3u8 } else { 1u8 },
                    String::new(),
                ),
                Drag::Walker {
                    project_id,
                    entry_id,
                    moved,
                    ..
                } => (project_id.clone(), *moved, 2u8, entry_id.clone()),
                // HUD slider: no click/drag question on release — the
                // stroke already wrote the remembered volume live.
                Drag::BoomboxSlider { project_id } => {
                    (project_id.clone(), 0.0, 4u8, String::new())
                }
                Drag::TodoTree {
                    project_id,
                    card_id,
                    moved,
                    ..
                } => (project_id.clone(), *moved, 5u8, card_id.clone()),
            };
            h.drag = None;
            // drift paused while this drag held
            // but the shared clock kept ticking — re-arm every orbit lerp
            // at the live frame so marbles ease back onto their rings
            // instead of snapping to the advanced θ.
            for planet in h.planets.values_mut() {
                if !planet.is_marble {
                    continue;
                }
                if let Some(o) = planet.orbit.as_mut() {
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    o.lerp_start = Instant::now();
                    o.lerp_from = NSPoint {
                        x: frame.mid_x(),
                        y: frame.mid_y(),
                    };
                }
            }
            if kind == 0 {
                let panel_and_marble = h.planets.get(&project_id).map(|p| (p.panel, p.is_marble));
                if let Some((panel, is_marble)) = panel_and_marble {
                    persist_position(&mut h, &project_id, panel);
                    // drag wins — the dropped
                    // marble sits ~2s, then eases in polar onto whatever
                    // ring its cluster describes at the drop spot. Orbit
                    // state is KEPT so the 220/280 hysteresis still
                    // decides membership; the pin branch re-arms the lerp.
                    if let Some(planet) = h.planets.get_mut(&project_id) {
                        planet.pin_until =
                            Some(Instant::now() + Duration::from_secs_f64(DRIFT_PIN_SECS));
                    }
                    if moved < 4.0 && is_marble {
                        if let Some(app) = app_handle() {
                            let _ = app.emit_to("main", "habitat:expand", project_id);
                        }
                    }
                }
            } else if kind == 1 {
                h.worlds_dirty = true;
            } else if kind == 3 {
                if moved < BOOMBOX_CLICK_SLOP {
                    boombox_click(&mut h, &project_id);
                } else {
                    h.worlds_dirty = true;
                    // A drag is not a click — and an open HUD never
                    // rides the box to its new spot.
                    boombox_hud::close(&mut h);
                }
            } else if kind == 4 {
                // HUD slider: the value was applied live during the
                // stroke; set_volume's worlds_dirty persists at 10Hz.
            } else if kind == 5 {
                if let Some(planet) = h.planets.get(&project_id) {
                    if let Some(t) = planet.todo_trees.iter().find(|t| t.card_id == entry_id)
                    {
                        super::todo_tree::emit_moved(&entry_id, t.x, t.y);
                    }
                }
            } else if moved >= 4.0 {
                // kind == 2 (walker): a real drag, not a click — the
                // pending click dies. PROPS leftover (town usable): the
                // drop point hit-tests the board columns — a drop on the
                // billboard is a STATUS MOVE, never a failed park. A
                // drop on grass/disc with no column just resumes the
                // pose walk next anim tick.
                h.pending_click = None;
                let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
                let col = h.planets.get(&project_id).and_then(|planet| {
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    let local = NSPoint {
                        x: mouse.x - frame.origin.x,
                        y: frame.origin.y + frame.size.height - mouse.y,
                    };
                    super::draw::hit_board_column(planet, local)
                });
                match col {
                    // Todo column: the card stays/returns todo. Spending
                    // this roam's check-in is what makes "stays todo"
                    // hold — the walker parks at the board lip instead
                    // of re-checking-in a beat later. The rail flips
                    // doing→todo (a todo card no-ops there).
                    Some(0) => {
                        if let Some(planet) = h.planets.get_mut(&project_id) {
                            if let Some(r) = planet.roam.get_mut(&entry_id) {
                                r.via_checkin = true;
                            }
                        }
                        if let Some(app) = app_handle() {
                            let _ = app.emit("habitat:board-todo", entry_id);
                        }
                    }
                    // Doing column: the board touch by hand — emit the
                    // check-in (rail flips todo→doing) and mark it spent
                    // on this roam entry, so a drag back to Todo STAYS
                    // Todo (same stickiness as the walked check-in).
                    Some(1) => {
                        if let Some(planet) = h.planets.get_mut(&project_id) {
                            if let Some(r) = planet.roam.get_mut(&entry_id) {
                                r.via_checkin = true;
                            }
                        }
                        emit_board_checkin(&entry_id);
                    }
                    // Done column: no longer mass-evicts walkers via
                    // habitat:board-done (BOARD-MARK). Drop is a no-op;
                    // complete stays on the board UI / walker Done path.
                    Some(2) => {}
                    _ => {}
                }
            }
            return;
        }
        let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        // HUD slider strokes compute the value under the drag borrow,
        // then apply after the match (set_volume wants the whole h).
        let mut slider: Option<(String, f32)> = None;
        match h.drag.as_mut() {
            Some(Drag::Panel {
                project_id,
                grab_dx,
                grab_dy,
                start,
                moved,
            }) => {
                *moved = moved.max((mouse.x - start.x).hypot(mouse.y - start.y));
                let (dx, dy, pid) = (*grab_dx, *grab_dy, project_id.clone());
                if let Some(planet) = h.planets.get_mut(&pid) {
                    // A dragged planet is user-placed. Orbit state is
                    // kept through the drag so the hysteresis — not the
                    // drag — decides whether it leaves its ring;
                    // drift_pass re-arms the lerp under the pin.
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    let cx = mouse.x - dx;
                    let cy = mouse.y - dy;
                    let _: () = msg_send![panel,
                        setFrameOrigin: NSPoint {
                            x: cx - frame.size.width / 2.0,
                            y: cy - frame.size.height / 2.0,
                        }];
                }
            }
            Some(Drag::Decor {
                project_id,
                decor,
                start,
                moved,
            }) => {
                *moved = moved.max((mouse.x - start.x).hypot(mouse.y - start.y));
                let dref = *decor;
                let pid = project_id.clone();
                // Boombox: do not slide the sprite until past click slop.
                // Under that, mouse-up is the HUD toggle and boombox_pos stays.
                if dref.kind == DecorKind::Boombox && *moved < BOOMBOX_CLICK_SLOP {
                    return;
                }
                if let Some(planet) = h.planets.get_mut(&pid) {
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    let local = NSPoint {
                        x: mouse.x - frame.origin.x,
                        y: frame.origin.y + frame.size.height - mouse.y,
                    };
                    let side = (planet.radius + VIEW_PADDING) * 2.0;
                    let center = NSPoint { x: side / 2.0, y: side / 2.0 };
                    let (x, y) = view_to_disc(center, planet.radius, planet.projection, local);
                    match dref.kind {
                        DecorKind::Plant => {
                            if let Some(d) = planet.plants.get_mut(dref.index) {
                                d.x = x;
                                d.y = y;
                            }
                        }
                        DecorKind::Mushroom => {
                            if let Some(d) = planet.mushrooms.get_mut(dref.index) {
                                d.x = x;
                                d.y = y;
                            }
                        }
                        DecorKind::Building => {
                            if let Some(d) = planet.buildings.get_mut(dref.index) {
                                d.x = x;
                                d.y = y;
                            }
                        }
                        DecorKind::Boombox => {
                            planet.boombox_pos = Some(DecorPoint { x, y });
                        }
                    }
                    let snap = (
                        planet.plants.clone(),
                        planet.mushrooms.clone(),
                        planet.buildings.clone(),
                        planet.boombox_pos,
                    );
                    layout_decor(planet);
                    let view = planet.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                    drop(planet);
                    if let Some(w) = h.worlds.get_mut(&pid) {
                        w.plants = snap.0;
                        w.mushrooms = snap.1;
                        w.buildings = snap.2;
                        w.boombox_pos = snap.3;
                    }
                }
            }
            Some(Drag::Walker {
                project_id,
                entry_id,
                start,
                moved,
            }) => {
                *moved = moved.max((mouse.x - start.x).hypot(mouse.y - start.y));
                let pid = project_id.clone();
                let eid = entry_id.clone();
                if let Some(planet) = h.planets.get_mut(&pid) {
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    let local = NSPoint {
                        x: mouse.x - frame.origin.x,
                        y: frame.origin.y + frame.size.height - mouse.y,
                    };
                    let side = (planet.radius + VIEW_PADDING) * 2.0;
                    let center = NSPoint { x: side / 2.0, y: side / 2.0 };
                    let (x, y) = view_to_disc(center, planet.radius, planet.projection, local);
                    if let Some(r) = planet.roam.get_mut(&eid) {
                        r.x = x;
                        r.y = y;
                        r.tx = x;
                        r.ty = y;
                    }
                    layout_beings(planet);
                    let view = planet.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
            Some(Drag::TodoTree {
                project_id,
                card_id,
                start,
                moved,
            }) => {
                *moved = moved.max((mouse.x - start.x).hypot(mouse.y - start.y));
                let pid = project_id.clone();
                let cid = card_id.clone();
                if let Some(planet) = h.planets.get_mut(&pid) {
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    let local = NSPoint {
                        x: mouse.x - frame.origin.x,
                        y: frame.origin.y + frame.size.height - mouse.y,
                    };
                    let side = (planet.radius + VIEW_PADDING) * 2.0;
                    let center = NSPoint { x: side / 2.0, y: side / 2.0 };
                    let (x, y) = view_to_disc(center, planet.radius, planet.projection, local);
                    if let Some(t) = planet.todo_trees.iter_mut().find(|t| t.card_id == cid) {
                        let (cx, cy) = crate::todo_trees::clamp_to_grove(x, y);
                        t.x = cx;
                        t.y = cy;
                    }
                    super::todo_tree::layout(planet);
                    let view = planet.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
            Some(Drag::BoomboxSlider { project_id }) => {
                let pid = project_id.clone();
                if let Some(planet) = h.planets.get(&pid) {
                    let panel = planet.panel as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    let local = NSPoint {
                        x: mouse.x - frame.origin.x,
                        y: frame.origin.y + frame.size.height - mouse.y,
                    };
                    if let Some(r) = boombox_hud::rects(planet) {
                        slider = Some((pid, boombox_hud::slider_value(&r, local)));
                    }
                }
            }
            None => {}
        }
        if let Some((pid, v)) = slider {
            boombox_hud::set_volume(&mut h, &pid, v);
        }
    }
}

fn persist_position(h: &mut Habitat, project_id: &str, panel: usize) {
    unsafe {
        let panel = panel as *mut Object;
        let frame: NSRect = msg_send![panel, frame];
        let center = NSPoint {
            x: frame.mid_x(),
            y: frame.mid_y(),
        };
        let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        let all = screens();
        let chosen = all
            .iter()
            .find(|(_, f)| f.contains(center))
            .or_else(|| all.iter().find(|(_, f)| f.contains(mouse)))
            .or_else(|| all.first());
        let Some((screen_id, sframe)) = chosen else { return };
        let entry = h
            .worlds
            .entry(project_id.to_string())
            .or_insert_with(|| default_state(screen_id, 0));
        entry.screen_id = screen_id.clone();
        entry.x = center.x - sframe.origin.x;
        entry.y = sframe.origin.y + sframe.size.height - center.y;
        entry.last_used = now_epoch();
        save_worlds(h);
    }
}

