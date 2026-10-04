//! Mouse-up on the world view (DIET-HABITAT).
//!
//! Board-open release, then the being click matrix. AX runs after the
//! lock drops. Peeled from `mouse.rs`.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use std::time::Instant;
use tauri::Emitter;

use super::draw::hit_board_open;
use super::input::{hit_being, hit_decor, over_disc};
use super::{app_handle, lock, set_caption, walker_hidden, DecorKind, Drag, NSPoint};

/// Empty-disc double-click only. Walkers (sleepers and droids included),
/// dots, the board, the boombox, other décor, and todo trees keep their
/// own gestures.
pub(crate) fn empty_surface_double(
    over_disc: bool,
    on_walker: bool,
    on_dot: bool,
    on_sleeper: bool,
    on_droid: bool,
    on_board: bool,
    on_boombox: bool,
    on_decor: bool,
    on_todo: bool,
) -> bool {
    over_disc
        && !on_walker
        && !on_dot
        && !on_sleeper
        && !on_droid
        && !on_board
        && !on_boombox
        && !on_decor
        && !on_todo
}

fn release_is_empty_surface(planet: &super::Planet, local: NSPoint) -> bool {
    let being = hit_being(planet, local);
    let on_walker = being.as_ref().is_some_and(|(_, walker)| *walker);
    let on_dot = being.as_ref().is_some_and(|(_, walker)| !*walker);
    let on_sleeper = being.as_ref().is_some_and(|(id, _)| {
        planet
            .walkers
            .iter()
            .any(|b| b.entry_id == *id && b.asleep)
    });
    let on_droid = being.as_ref().is_some_and(|(id, _)| {
        planet
            .walkers
            .iter()
            .any(|b| b.entry_id == *id && b.droid.is_some())
    });
    let decor = hit_decor(planet, local);
    empty_surface_double(
        over_disc(planet, local),
        on_walker,
        on_dot,
        on_sleeper,
        on_droid,
        hit_board_open(planet, local),
        decor.is_some_and(|d| d.kind == DecorKind::Boombox),
        decor.is_some(),
        super::todo_tree::hit(planet, local).is_some(),
    )
}

/// Double-click with no real drag. Clicks other than 2 do not toggle.
/// A panel drag of ≥4px, and any other grab, do not toggle.
fn surface_toggle_gesture(clicks: usize, drag: Option<&Drag>) -> bool {
    if clicks != 2 {
        return false;
    }
    match drag {
        Some(Drag::Panel { moved, .. }) if *moved >= 4.0 => false,
        Some(Drag::Panel { .. }) | None => true,
        Some(_) => false,
    }
}

/// Stationary empty-surface double-click. A real panel drag (≥4px) and any
/// other grab (décor, walker, slider, todo tree) are not this toggle.
fn surface_toggle_id(
    h: &super::Habitat,
    view_ptr: usize,
    local: NSPoint,
    clicks: usize,
) -> Option<String> {
    if !surface_toggle_gesture(clicks, h.drag.as_ref()) {
        return None;
    }
    let (project_id, planet) = h.planets.iter().find(|(_, p)| p.view == view_ptr)?;
    if !release_is_empty_surface(planet, local) {
        return None;
    }
    Some(project_id.clone())
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


pub(crate) unsafe fn mouse_up_inner(this: &Object, event: *mut Object) {
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
                    p.view == view_ptr && hit_board_open(p, local)
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
                        // In-memory backup for a kanban webview that has
                        // not mounted the focus listener yet. Take-once, 5s.
                        crate::panes::remember_board_focus(&pid);
                        let _ = app.emit("orbitty:focus-project", pid);
                    }
                }
                return;
            }
        }
        let action = {
            let mut h = lock();
            let Some(pending) = h.pending_click.take() else {
                // Same emit as menu_minimize → menu_toggle_marble.
                let toggle = surface_toggle_id(&h, view_ptr, local, clicks);
                drop(h);
                if let Some(id) = toggle {
                    super::planet_menu::emit_marble_toggle(&id);
                }
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

#[cfg(test)]
mod tests {
    use super::empty_surface_double;
    use super::surface_toggle_gesture;
    use super::Drag;
    use super::NSPoint;
    use super::super::{DecorKind, DecorRef};

    fn hit(
        over: bool,
        walker: bool,
        dot: bool,
        sleeper: bool,
        droid: bool,
        board: bool,
        boombox: bool,
        decor: bool,
        todo: bool,
    ) -> bool {
        empty_surface_double(over, walker, dot, sleeper, droid, board, boombox, decor, todo)
    }

    #[test]
    fn only_the_empty_disc_is_a_surface_double() {
        assert!(hit(true, false, false, false, false, false, false, false, false));
        assert!(!hit(false, false, false, false, false, false, false, false, false));
        assert!(!hit(true, true, false, false, false, false, false, false, false));
        assert!(!hit(true, false, true, false, false, false, false, false, false));
        assert!(!hit(true, false, false, true, false, false, false, false, false));
        assert!(!hit(true, false, false, false, true, false, false, false, false));
        assert!(!hit(true, false, false, false, false, true, false, false, false));
        assert!(!hit(true, false, false, false, false, false, true, false, false));
        assert!(!hit(true, false, false, false, false, false, false, true, false));
        assert!(!hit(true, false, false, false, false, false, false, false, true));
    }

    fn panel(moved: f64) -> Drag {
        Drag::Panel {
            project_id: "p".into(),
            grab_dx: 0.0,
            grab_dy: 0.0,
            start: NSPoint { x: 0.0, y: 0.0 },
            moved,
        }
    }

    #[test]
    fn surface_toggle_gesture_is_a_still_double_click() {
        assert!(!surface_toggle_gesture(0, None));
        assert!(!surface_toggle_gesture(1, None));
        assert!(surface_toggle_gesture(2, None));
        assert!(!surface_toggle_gesture(3, None));
        assert!(surface_toggle_gesture(2, Some(&panel(3.9))));
        assert!(!surface_toggle_gesture(2, Some(&panel(4.0))));
        let decor = Drag::Decor {
            project_id: "p".into(),
            decor: DecorRef {
                kind: DecorKind::Plant,
                index: 0,
            },
            start: NSPoint { x: 0.0, y: 0.0 },
            moved: 0.0,
        };
        assert!(!surface_toggle_gesture(2, Some(&decor)));
        let walker = Drag::Walker {
            project_id: "p".into(),
            entry_id: "e".into(),
            start: NSPoint { x: 0.0, y: 0.0 },
            moved: 0.0,
        };
        assert!(!surface_toggle_gesture(2, Some(&walker)));
    }
}
