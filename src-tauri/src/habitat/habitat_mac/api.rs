//! Tauri-facing habitat commands (DIET-HABITAT).
//!
//! Extracted from habitat_mac/mod.rs. sync body stays in sync.rs.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};
use tauri::AppHandle;

use super::sync as sync_mod;
use super::{app_handle, lock, panic_msg};
use crate::habitat::HabitatSnapshot;

// sync (port of OverlayController.relayout)

/// True on the AppKit main thread. The Tauri command handler for
/// `sync_habitat` runs there, so this is the common path.
fn on_main_thread() -> bool {
    unsafe { msg_send![class!(NSThread), isMainThread] }
}

pub fn sync(app: &AppHandle, snapshot: HabitatSnapshot) -> Result<(), String> {
    // CADENCE-FIX: already on main → run inline. The old code queued the
    // closure with run_on_main_thread and then sat in recv_timeout(2s) —
    // on the main thread that is main waiting for a closure queued behind
    // itself, so every board drag beach-balled for the full 2s before the
    // sync landed.
    if on_main_thread() {
        return match catch_unwind(AssertUnwindSafe(|| sync_mod::do_sync(&snapshot))) {
            Ok(()) => Ok(()),
            Err(p) => {
                let msg = panic_msg(&*p);
                eprintln!("habitat: sync panicked (caught): {msg}");
                Err(format!("habitat sync failed: {msg}"))
            }
        };
    }
    let app = app.clone();
    // Off main (hypothetical IPC caller): keep the hop, but never block
    // main — recv_timeout on a worker thread is harmless.
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let result = catch_unwind(AssertUnwindSafe(|| sync_mod::do_sync(&snapshot)));
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(p)) => {
            let msg = panic_msg(&*p);
            eprintln!("habitat: sync panicked (caught): {msg}");
            Err(format!("habitat sync failed: {msg}"))
        }
        Err(_) => Err("habitat sync: no result from main thread".to_string()),
    }
}

// do_sync / flush_pose_events / being_fields_eq → sync.rs (DIET-HABITAT)

/// the rail's `habitat:locate` — stroke the
/// walker's frame for ~500ms. Glow only: no focus, no raise, no wake.
pub fn locate_walker(app: AppHandle, entry_id: String) -> Result<(), String> {
    let app = app.clone();
    app.run_on_main_thread(move || {
        let mut h = lock();
        h.locate = Some((entry_id.clone(), Instant::now()));
        for p in h.planets.values() {
            if p.walkers.iter().any(|b| b.entry_id == entry_id) {
                unsafe {
                    let view = p.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Dest-menu find: glow that planet ~1.4s. Raises nothing — caller may
/// also `bring_planet_front`. Marble = diagonal ring; expanded = rim + name.
pub fn locate_planet(app: AppHandle, project_id: String) -> Result<(), String> {
    let app = app.clone();
    app.run_on_main_thread(move || {
        let mut h = lock();
        h.locate_planet = Some((project_id.clone(), Instant::now()));
        if let Some(p) = h.planets.get(&project_id) {
            unsafe {
                let view = p.view as *mut Object;
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

// rail Home button

/// Order every planet panel front. Peer surfaces keep their relative order;
/// this just lifts the whole set above whatever covered it.
pub fn bring_all_front() {
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    let _ = app.run_on_main_thread(|| {
        let h = lock();
        if h.planets_hidden {
            return;
        }
        for planet in h.planets.values() {
            unsafe {
                let panel = planet.panel as *mut Object;
                let _: () = msg_send![panel, orderFrontRegardless];
            }
        }
    });
}

/// Hide or show every planet disc. Hidden discs orderOut; shown lift front.
pub fn set_planets_hidden(hidden: bool) {
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    let _ = app.run_on_main_thread(move || {
        let mut h = lock();
        h.planets_hidden = hidden;
        for planet in h.planets.values() {
            unsafe {
                let panel = planet.panel as *mut Object;
                if hidden {
                    let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
                } else {
                    let _: () = msg_send![panel, orderFrontRegardless];
                }
            }
        }
    });
}

/// Order one planet's panel front by project id — that disc only. An
/// unknown or panel-less id is a no-op, never a bring_all_front fallback
/// (the user picked one name; lifting every disc would lie).
pub fn bring_planet_front(project_id: &str) {
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    let id = project_id.to_string();
    let _ = app.run_on_main_thread(move || {
        let h = lock();
        if h.planets_hidden {
            return;
        }
        if let Some(planet) = h.planets.get(&id) {
            unsafe {
                let panel = planet.panel as *mut Object;
                let _: () = msg_send![panel, orderFrontRegardless];
            }
        }
    });
}

// yard meow gate (GO-ORBITTY-HERDING-CATS)

/// Is any planet's boombox actually playing right now? The yard's meow
/// loop skips while this is true — voices never stack. Read-only; called
/// from the yard's main-thread hop (lock order yard → habitat, never the
/// reverse, so no deadlock).
pub fn boombox_playing() -> bool {
    let h = lock();
    for p in h.planets.values() {
        let Some(ptr) = p.audio else {
            continue;
        };
        unsafe {
            let player = ptr as *mut Object;
            if player.is_null() {
                continue;
            }
            let playing: bool = msg_send![player, isPlaying];
            if playing {
                return true;
            }
        }
    }
    false
}

// tray escape hatch

/// Bring back every minimized window that has a home (Swift restoreAll).
/// Parked zombies sit in `dots` (marble-scale occupancy) but are still real
/// minimized windows, so they come back too; deminiaturize is a no-op on
/// already-open dot windows. Sleepers (closed windows) are skipped — their
/// door is wake, not deminiaturize.
pub fn restore_all() {
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    let _ = app.run_on_main_thread(|| {
        let ids: Vec<String> = {
            let h = lock();
            h.planets
                .values()
                .flat_map(|p| {
                    p.walkers
                        .iter()
                        .chain(p.dots.iter())
                        // Sleepers are closed windows — deminiaturize on a
                        // dead id can only fail; wake is their door.
                        .filter(|b| !b.asleep)
                        .map(|b| b.entry_id.clone())
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        for id in ids {
            let _ = crate::windows::deminiaturize_window(&id);
        }
    });
}
