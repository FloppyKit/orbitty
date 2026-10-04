//! Planet right-click menu + NSMenu action handlers (DIET-HABITAT).
//!
//! Extracted from habitat_mac/input.rs.

#![allow(unexpected_cfgs)]

use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::panic::{catch_unwind, AssertUnwindSafe};
use tauri::Emitter;

use super::audio;
use super::input::{add_menu_item, pop_context_menu};
use super::input::boombox_click;
use super::{
    app_handle, boombox_hud, layout_decor, lock, menu_target, ns_string, sender_project_id,
    set_caption, world_entry, DecorKind, Habitat, Placement,
};

/// Words on the disc menu. Beach ball is always the last row; `beach_ball`
/// is the check the menu paints on it (the words do not change).
pub(crate) fn planet_menu_items(
    is_marble: bool,
    has_box: bool,
    beach_ball: bool,
) -> Vec<&'static str> {
    let _ = beach_ball;
    vec![
        if is_marble { "Expand" } else { "Minimize" },
        if has_box {
            "Remove boombox"
        } else {
            "Place boombox"
        },
        "Beach ball",
    ]
}

/// Disc / marble context menu: Minimize/Expand, boombox, beach ball.
pub(crate) unsafe fn planet_context_menu(
    this: &Object,
    event: *mut Object,
    project_id: &str,
    is_marble: bool,
) {
    let (has_box, beach_ball) = {
        let h = lock();
        h.worlds
            .get(project_id)
            .map(|w| (w.boombox_pos.is_some(), w.beach_ball))
            .unwrap_or((false, false))
    };
    let rows = planet_menu_items(is_marble, has_box, beach_ball);
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
    // One row toggles marble ↔ expanded. Boombox is a live place/remove.
    // Beach ball is checkable and stays off marbles (no walkers there).
    add_menu_item(
        menu,
        rows[0],
        sel!(planetToggleMarble:),
        target,
        project_id,
        None,
        true,
    );
    add_menu_item(
        menu,
        rows[1],
        sel!(planetToggleBoombox:),
        target,
        project_id,
        None,
        true,
    );
    add_menu_item(
        menu,
        rows[2],
        sel!(planetToggleBeachBall:),
        target,
        project_id,
        Some(beach_ball),
        !is_marble,
    );
    let view = this as *const Object as *mut Object;
    pop_context_menu(menu, event, view);
    let _: () = msg_send![menu, release];
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

/// Marble ↔ expanded. Rail owns collapse; next sync applies the size.
/// The disc menu and an empty-surface double-click both land here.
pub(crate) fn emit_marble_toggle(project_id: &str) {
    let is_marble = lock()
        .planets
        .get(project_id)
        .map(|p| p.is_marble)
        .unwrap_or(false);
    if let Some(app) = app_handle() {
        let event = if is_marble {
            "habitat:expand"
        } else {
            "habitat:collapse"
        };
        let _ = app.emit_to("main", event, project_id.to_string());
    }
}

pub(crate) extern "C" fn menu_toggle_marble(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(id) = sender_project_id(sender) else {
            return;
        };
        emit_marble_toggle(&id);
    }));
}

/// Kept registered for old OrbittyPlanetMenuTarget slots.
pub(crate) extern "C" fn menu_minimize(this: &Object, cmd: Sel, sender: *mut Object) {
    menu_toggle_marble(this, cmd, sender);
}

pub(crate) extern "C" fn menu_restore(this: &Object, cmd: Sel, sender: *mut Object) {
    menu_toggle_marble(this, cmd, sender);
}

pub(crate) extern "C" fn menu_claim_front(_this: &Object, _cmd: Sel, sender: *mut Object) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(id) = sender_project_id(sender) else {
            return;
        };
        if let Some(app) = app_handle() {
            if let Err(e) = crate::claim_front::claim_front_to_planet(app, &id) {
                eprintln!("planet-claim-front: {e}");
                if let Some(planet) = lock().planets.get_mut(&id) {
                    set_caption(planet, format!("claim: {e}"));
                }
            } else if let Some(planet) = lock().planets.get_mut(&id) {
                set_caption(planet, "claimed here".into());
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

#[cfg(test)]
mod tests {
    use super::planet_menu_items;

    #[test]
    fn planet_menu_is_minimize_boombox_beach_ball() {
        for marble in [false, true] {
            for has_box in [false, true] {
                for ball in [false, true] {
                    let items = planet_menu_items(marble, has_box, ball);
                    assert_eq!(items.len(), 3, "{items:?}");
                    assert_eq!(items[0], if marble { "Expand" } else { "Minimize" });
                    assert_eq!(
                        items[1],
                        if has_box {
                            "Remove boombox"
                        } else {
                            "Place boombox"
                        }
                    );
                    assert_eq!(items[2], "Beach ball");
                    let blob = items.join("\n").to_ascii_lowercase();
                    assert!(!blob.contains("plant"), "{blob}");
                    assert!(!blob.contains("mushroom"), "{blob}");
                    assert!(!blob.contains("building"), "{blob}");
                    assert!(!blob.contains("music"), "{blob}");
                    assert!(!blob.contains("front window"), "{blob}");
                }
            }
        }
    }
}
