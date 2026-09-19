//! Spawn ring around the rail + default WorldState placement (DIET-HABITAT).
//!
//! Extracted from habitat_mac/mod.rs.

#![allow(unexpected_cfgs)]

use std::collections::HashMap;
use tauri::Manager;

use super::geom::CGSize;
use super::world::WorldState;
use super::{
    app_handle, now_epoch, screens, visible_frames, EXPANDED_RADIUS, NSPoint,
};

// spawn ring (GO-ORBITTY-PLANET-SPAWN)
//
// New worlds appear on a ring around the rail, not mid-right of the
// display. Center = the `main` window's outer-frame center, converted into
// the same screen-local flipped (y-down) coords WorldState.x/y already
// use. Radius = half the rail's outer width + 80pt. First slot is
// 3 o'clock, then counterclockwise in 45° steps; if the rail sits on the
// right third of its screen (or 3 o'clock would clip the visibleFrame)
// the walk mirrors — 9 o'clock first, then clockwise — so the first
// visible slot is always inboard. A slot is skipped when the expanded
// disc (EXPANDED_RADIUS, not the marble) would clip that screen's
// visibleFrame. Existing worlds never move and do not occupy slots; only
// worlds already sitting on the ring (spawned here this session) count as
// claimed. Rail missing (headless test) → mid-right fallback, no panic.

pub(crate) const RING_SLOTS: usize = 8;
pub(crate) const RING_CLEARANCE: f64 = 80.0;

pub(crate) struct SpawnRing {
    screen_id: String,
    cx: f64,
    cy: f64,
    radius: f64,
    mirrored: bool,
    visible: [bool; RING_SLOTS],
}

pub(crate) fn ring_slot(ring: &SpawnRing, k: usize) -> (f64, f64) {
    let step = std::f64::consts::FRAC_PI_4 * (k as f64);
    // y is down, so visual up is negative y: both walks go up first.
    let (dx, dy) = if ring.mirrored {
        (-step.cos(), -step.sin()) // 9 o'clock, then clockwise
    } else {
        (step.cos(), -step.sin()) // 3 o'clock, then counterclockwise
    };
    (ring.cx + ring.radius * dx, ring.cy + ring.radius * dy)
}

pub(crate) fn spawn_ring() -> Option<SpawnRing> {
    let window = app_handle()?.get_webview_window("main")?;
    let scale = window.scale_factor().ok()?;
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    // Tauri reports physical pixels from the primary screen's top-left
    // (y down); AppKit frames are points from its bottom-left (y up).
    let w = size.width as f64 / scale;
    let h = size.height as f64 / scale;
    let cx_t = pos.x as f64 / scale + w / 2.0;
    let cy_t = pos.y as f64 / scale + h / 2.0;
    let all = unsafe { screens() };
    let primary_h = all.first().map(|(_, f)| f.size.height)?;
    let center = NSPoint {
        x: cx_t,
        y: primary_h - cy_t,
    };
    let (screen_id, sf) = all
        .iter()
        .find(|(_, f)| f.contains(center))
        .or_else(|| all.first())
        .map(|(id, f)| (id.clone(), *f))?;
    let cx = center.x - sf.origin.x;
    let cy = sf.size.height - (center.y - sf.origin.y);
    let visible = unsafe { visible_frames() };
    let vf = visible
        .iter()
        .find(|(id, _)| id == &screen_id)
        .map(|(_, f)| *f)
        .unwrap_or(sf);
    // visibleFrame as a y-down rect in screen-local coords.
    let vx0 = vf.origin.x - sf.origin.x;
    let vy0 = sf.size.height - (vf.origin.y + vf.size.height - sf.origin.y);
    let clips = |sx: f64, sy: f64| {
        sx - EXPANDED_RADIUS < vx0
            || sx + EXPANDED_RADIUS > vx0 + vf.size.width
            || sy - EXPANDED_RADIUS < vy0
            || sy + EXPANDED_RADIUS > vy0 + vf.size.height
    };
    let radius = w / 2.0 + RING_CLEARANCE;
    let mirrored = cx > sf.size.width * 2.0 / 3.0 || clips(cx + radius, cy);
    let mut ring = SpawnRing {
        screen_id,
        cx,
        cy,
        radius,
        mirrored,
        visible: [false; RING_SLOTS],
    };
    for k in 0..RING_SLOTS {
        let (sx, sy) = ring_slot(&ring, k);
        ring.visible[k] = !clips(sx, sy);
    }
    Some(ring)
}

/// Worlds already sitting on a visible ring slot — only those occupy
/// slots; pre-existing (mid-right or dragged) worlds are not on the ring.
pub(crate) fn ring_claimed(ring: &SpawnRing, worlds: &HashMap<String, WorldState>) -> usize {
    worlds
        .values()
        .filter(|w| {
            w.screen_id == ring.screen_id
                && (0..RING_SLOTS).any(|k| {
                    if !ring.visible[k] {
                        return false;
                    }
                    let (sx, sy) = ring_slot(ring, k);
                    (w.x - sx).abs() < 1.0 && (w.y - sy).abs() < 1.0
                })
        })
        .count()
}

/// New worlds land on the spawn ring around the rail; `existing_on_screen`
/// is the count of ring slots already claimed this session. Headless
/// fallback: mid-right, clearly on-screen, staggered per existing world.
pub(crate) fn default_state(screen_id: &str, existing_on_screen: usize) -> WorldState {
    let placed = spawn_ring().and_then(|ring| {
        let open: Vec<usize> = (0..RING_SLOTS).filter(|&k| ring.visible[k]).collect();
        if open.is_empty() {
            return None;
        }
        let lap = existing_on_screen / open.len();
        let (sx, sy) = ring_slot(&ring, open[existing_on_screen % open.len()]);
        // Past a full lap, ride the same slots a touch outward so a ninth
        // world never lands exactly on the first.
        let grow = lap as f64 * 96.0;
        let dx = (sx - ring.cx) / ring.radius;
        let dy = (sy - ring.cy) / ring.radius;
        Some((ring.screen_id.clone(), sx + dx * grow, sy + dy * grow))
    });
    let (screen_id, x, y) = match placed {
        Some(p) => p,
        None => {
            let all = unsafe { screens() };
            let size = all
                .iter()
                .find(|(id, _)| id == screen_id)
                .or_else(|| all.first())
                .map(|(_, f)| f.size)
                .unwrap_or(CGSize {
                    width: 1440.0,
                    height: 900.0,
                });
            (
                screen_id.to_string(),
                size.width * 0.72 + (existing_on_screen as f64) * 40.0,
                size.height * 0.45,
            )
        }
    };
    WorldState {
        screen_id,
        x,
        y,
        last_used: now_epoch(),
        radius: 0.0,
        plant: false,
        mushroom: false,
        boombox: false,
        music: String::new(),
        boombox_volume: 1.0,
        boombox_muted: false,
        projection: "oblique".into(),
        first_seen: now_epoch(),
        plants: Vec::new(),
        mushrooms: Vec::new(),
        buildings: Vec::new(),
        boombox_pos: None,
        beach_ball: false,
    }
}

