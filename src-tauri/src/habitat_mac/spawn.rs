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
// The ring math is still here for slot tests. New worlds no longer use it:
// GO-ORBITTY-SPAWN-CENTER places a fresh marble at the visible-frame center.
// Center = the `main` window's outer-frame center, converted into
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
    /// Visible-frame center, same screen-local y-down coords as slot x/y.
    vis_cx: f64,
    vis_cy: f64,
}

impl SpawnRing {
    /// Visible-frame center in the same coords as a world's x/y.
    pub(crate) fn visible_center(&self) -> (&str, f64, f64) {
        (&self.screen_id, self.vis_cx, self.vis_cy)
    }
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
        vis_cx: vx0 + vf.size.width / 2.0,
        vis_cy: vy0 + vf.size.height / 2.0,
    };
    for k in 0..RING_SLOTS {
        let (sx, sy) = ring_slot(&ring, k);
        ring.visible[k] = !clips(sx, sy);
    }
    Some(ring)
}

/// Worlds already sitting on a visible ring slot — only those occupy
/// slots; pre-existing (mid-right or dragged) worlds are not on the ring.
/// New worlds no longer claim slots; the counter stays with the ring math.
#[allow(dead_code)]
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

/// New worlds land as a marble at the visible-frame center (GO-ORBITTY-SPAWN-CENTER).
/// `nudge` steps later worlds down-right so two fresh marbles do not stack.
/// Headless fallback: screen center.
pub(crate) fn default_state(screen_id: &str, nudge: usize) -> WorldState {
    let step = nudge as f64;
    let placed = spawn_ring().map(|ring| {
        (
            ring.screen_id.clone(),
            ring.vis_cx + step * 40.0,
            ring.vis_cy + step * 28.0,
        )
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
                size.width * 0.5 + step * 40.0,
                size.height * 0.5 + step * 28.0,
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

