//! Spawn ring around the rail + default WorldState placement (DIET-HABITAT).
//!
//! Extracted from habitat_mac/mod.rs.

#![allow(unexpected_cfgs)]

use std::collections::HashMap;
use tauri::Manager;

use super::geom::CGSize;
use super::world::WorldState;
use super::{
    app_handle, now_epoch, screens, visible_frames, EXPANDED_RADIUS, NSPoint, NSRect,
    VIEW_PADDING,
};

// spawn ring (GO-ORBITTY-PLANET-SPAWN)
//
// The ring math is still here for slot tests. New worlds no longer use it.
// A fresh marble lands on the middle display (the screen that holds the
// middle of the whole desktop), clear of the inspector, so opening it
// does not cover that window. The inspector's own window is `main`, and
// that window is not the center display. Coords are screen-local y-down,
// the same space WorldState.x/y already use.
// Radius = half the rail's outer width + 80pt. First slot is
// 3 o'clock, then counterclockwise in 45° steps; if the rail sits on the
// right third of its screen (or 3 o'clock would clip the visibleFrame)
// the walk mirrors — 9 o'clock first, then clockwise — so the first
// visible slot is always inboard. A slot is skipped when the expanded
// disc (EXPANDED_RADIUS, not the marble) would clip that screen's
// visibleFrame. Existing worlds never move and do not occupy slots; only
// worlds already sitting on the ring (spawned here this session) count as
// claimed. Rail missing (headless test) → mid-right fallback, no panic.

pub(crate) const RING_SLOTS: usize = 8;
#[allow(dead_code)]
pub(crate) const RING_CLEARANCE: f64 = 80.0;

pub(crate) struct SpawnRing {
    screen_id: String,
    cx: f64,
    cy: f64,
    radius: f64,
    mirrored: bool,
    visible: [bool; RING_SLOTS],
    /// Visible-frame center, same screen-local y-down coords as slot x/y.
    #[allow(dead_code)]
    vis_cx: f64,
    #[allow(dead_code)]
    vis_cy: f64,
}

impl SpawnRing {
    /// Visible-frame center in the same coords as a world's x/y.
    #[allow(dead_code)]
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

#[allow(dead_code)]
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

/// Middle of the desktop, then slid so the opened planet does not cover
/// the inspector. `nudge` steps later worlds down-right.
pub(crate) fn fresh_anchor() -> Option<(String, f64, f64)> {
    let all = unsafe { screens() };
    let (id, frame) = center_screen(&all)?;
    let primary_h = all.first().map(|(_, f)| f.size.height)?;
    let visible = unsafe { visible_frames() };
    let vf = visible
        .iter()
        .find(|(sid, _)| sid == &id)
        .map(|(_, f)| *f)
        .unwrap_or(frame);
    let vx0 = vf.origin.x - frame.origin.x;
    let vy0 = frame.size.height - (vf.origin.y + vf.size.height - frame.origin.y);
    let half = EXPANDED_RADIUS + VIEW_PADDING;
    let min_x = vx0 + half;
    let max_x = (vx0 + vf.size.width - half).max(min_x);
    let min_y = vy0 + half;
    let max_y = (vy0 + vf.size.height - half).max(min_y);
    let cx = vx0 + vf.size.width / 2.0;
    let cy = vy0 + vf.size.height / 2.0;
    let (x, y) = match inspector_local(frame, primary_h) {
        Some(insp) => clear_of(cx, cy, insp, (min_x, min_y, max_x, max_y)),
        None => (cx.clamp(min_x, max_x), cy.clamp(min_y, max_y)),
    };
    Some((id, x, y))
}

fn center_screen(all: &[(String, NSRect)]) -> Option<(String, NSRect)> {
    if all.is_empty() {
        return None;
    }
    let min_x = all.iter().map(|(_, f)| f.origin.x).fold(f64::INFINITY, f64::min);
    let min_y = all.iter().map(|(_, f)| f.origin.y).fold(f64::INFINITY, f64::min);
    let max_x = all.iter().map(|(_, f)| f.max_x()).fold(f64::NEG_INFINITY, f64::max);
    let max_y = all.iter().map(|(_, f)| f.max_y()).fold(f64::NEG_INFINITY, f64::max);
    let mid = NSPoint {
        x: (min_x + max_x) / 2.0,
        y: (min_y + max_y) / 2.0,
    };
    if let Some((id, frame)) = all.iter().find(|(_, f)| f.contains(mid)) {
        return Some((id.clone(), *frame));
    }
    all.iter()
        .min_by(|(_, a), (_, b)| {
            let da = (a.mid_x() - mid.x).hypot(a.mid_y() - mid.y);
            let db = (b.mid_x() - mid.x).hypot(b.mid_y() - mid.y);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(id, frame)| (id.clone(), *frame))
}

/// Inspector center and half-size in screen-local y-down, when that
/// window sits on `screen`.
fn inspector_local(screen: NSRect, primary_h: f64) -> Option<(f64, f64, f64, f64)> {
    let window = app_handle()?.get_webview_window("main")?;
    let scale = window.scale_factor().ok()?;
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    let w = size.width as f64 / scale;
    let h = size.height as f64 / scale;
    let gx = pos.x as f64 / scale + w / 2.0;
    let gy = primary_h - (pos.y as f64 / scale + h / 2.0);
    let local_x = gx - screen.origin.x;
    let local_y = screen.size.height - (gy - screen.origin.y);
    if local_x < -w
        || local_x > screen.size.width + w
        || local_y < -h
        || local_y > screen.size.height + h
    {
        return None;
    }
    Some((local_x, local_y, w / 2.0, h / 2.0))
}

/// Slide `(x, y)` out from under the inspector so the opened planet
/// (expanded disc plus padding) does not cover that window.
fn clear_of(
    x: f64,
    y: f64,
    insp: (f64, f64, f64, f64),
    bounds: (f64, f64, f64, f64),
) -> (f64, f64) {
    let (icx, icy, hw, hh) = insp;
    let pad = EXPANDED_RADIUS + VIEW_PADDING + 16.0;
    let left = icx - hw - pad;
    let right = icx + hw + pad;
    let top = icy - hh - pad;
    let bot = icy + hh + pad;
    let (min_x, min_y, max_x, max_y) = bounds;
    let blocked = |px: f64, py: f64| px > left && px < right && py > top && py < bot;
    let (mut x, mut y) = if blocked(x, y) {
        let dl = (x - left).abs();
        let dr = (right - x).abs();
        let dt = (y - top).abs();
        let db = (bot - y).abs();
        let m = dl.min(dr).min(dt).min(db);
        if m == dl {
            (left, y)
        } else if m == dr {
            (right, y)
        } else if m == dt {
            (x, top)
        } else {
            (x, bot)
        }
    } else {
        (x, y)
    };
    x = x.clamp(min_x, max_x);
    y = y.clamp(min_y, max_y);
    if blocked(x, y) {
        let corners = [
            (min_x, min_y),
            (max_x, min_y),
            (min_x, max_y),
            (max_x, max_y),
        ];
        if let Some((px, py)) = corners
            .into_iter()
            .filter(|(px, py)| !blocked(*px, *py))
            .min_by(|(ax, ay), (bx, by)| {
                let da = (ax - x).hypot(ay - y);
                let db = (bx - x).hypot(by - y);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            return (px, py);
        }
    }
    (x, y)
}

/// New worlds land as a marble on the middle display, clear of the inspector.
/// `nudge` steps later worlds down-right so two fresh marbles do not stack.
/// Headless fallback: screen center.
pub(crate) fn default_state(screen_id: &str, nudge: usize) -> WorldState {
    let step = nudge as f64;
    let placed = fresh_anchor().map(|(id, x, y)| (id, x + step * 40.0, y + step * 28.0));
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

