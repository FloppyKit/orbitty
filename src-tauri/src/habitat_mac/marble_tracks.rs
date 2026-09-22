//! Per-marble tracks around each sun for one drift turn (DIET-HABITAT).
//!
//! Called from drift_pass. The orbit path never writes worlds.json.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use std::time::Instant;

use super::marble_motion::{
    circle_offset, host_ring_r, oval_offset, sun_rot, track_params, TrackParams,
};
use super::{
    Habitat, MarbleOrbit, NSPoint, NSRect,
    BARY_R_MAX, BARY_R_MIN, FRONT_DEADBAND, HOST_PREFIX, JOIN_EASE_SECS, MARBLE_RADIUS,
};

pub(crate) fn assign_tracks(
    h: &mut Habitat,
    groups: Vec<(String, NSPoint, Vec<usize>, bool)>,
    marbles: Vec<(String, NSPoint, String)>,
    expanded: Vec<(String, NSPoint, String, f64)>,
    visible: Vec<(String, NSRect)>,
    all: Vec<(String, NSRect)>,
    now: Instant,
    t: f64,
    pad: f64,
) {
    // Tracks: every marble onto its own path around its sun
    for (sun_key, sun_raw, idxs, is_host) in groups {
        let rot = sun_rot(&sun_key);
        // Hosted tracks take the live-radius circle — only phase0 /
        // period / dir stay hashed per marble, so the hashed-radius
        // range is bary-only.
        let (r_min, r_max) = if is_host {
            (0.0, 0.0)
        } else {
            (BARY_R_MIN, BARY_R_MAX)
        };
        // Independent tracks: radius / phase0 / direction / period are
        // all hashed from the marble's OWN id — never opposite ends of
        // a stick, never one shared θ.
        let tracks: Vec<(usize, TrackParams)> = idxs
            .iter()
            .map(|m| (*m, track_params(&marbles[*m].0, r_min, r_max)))
            .collect();
        // Host lookup: the panel (z boolean) and the LIVE scroll-zoomed
        // radius (ring + collider). A host group whose planet vanished
        // mid-pass is skipped — next tick re-hosts or goes bary.
        let host_info: Option<(usize, f64)> = if is_host {
            sun_key
                .strip_prefix(HOST_PREFIX)
                .and_then(|epid| h.planets.get(epid))
                .map(|p| (p.panel, p.radius))
        } else {
            None
        };
        if is_host && host_info.is_none() {
            continue;
        }
        // The hosted ring: a CIRCLE at live radius + two marble-widths
        // + the marble radius. Bigger chip → bigger ring.
        let ring_r = host_info.map(|(_, live_r)| host_ring_r(live_r));
        // Screen keep-in: nudge the SUN inward first (per tick only —
        // the frozen home is never written back, and a host planet's
        // panel never moves) so the full-size figure + marble pad stays
        // on the group's screen; only a screen too small even at the
        // best sun spot shrinks r. Marbles are never clamped
        // independently, and neither (0,0) nor visibleFrame.origin is
        // ever a target.
        let mut sun = sun_raw;
        let mut rscale = 1.0;
        let group_screen = &marbles[idxs[0]].2;
        if let Some((_, f)) = visible
            .iter()
            .find(|(id, _)| id == group_screen)
            .or_else(|| visible.first())
        {
            let r_top = match ring_r {
                Some(r) => r,
                None => tracks.iter().map(|(_, p)| p.r).fold(0.0, f64::max),
            };
            let mut inset = f.inset(pad + r_top);
            if inset.origin.x > inset.max_x() || inset.origin.y > inset.max_y() {
                inset = f.inset(pad);
                if inset.origin.x > inset.max_x() || inset.origin.y > inset.max_y() {
                    inset = f.inset(0.0); // degenerate screen — keep discs on
                }
            }
            sun = NSPoint {
                x: sun.x.clamp(inset.origin.x, inset.max_x().max(inset.origin.x)),
                y: sun.y.clamp(inset.origin.y, inset.max_y().max(inset.origin.y)),
            };
            let fit = (sun.x - f.origin.x - pad)
                .min(f.max_x() - sun.x - pad)
                .min(sun.y - f.origin.y - pad)
                .min(f.max_y() - sun.y - pad);
            if r_top > fit {
                rscale = (fit / r_top).clamp(0.0, 1.0);
                if let Some((_, live_r)) = host_info {
                    // The standoff law dies first: a host ring never
                    // scales below one marble-width off the LIVE face.
                    let min_r = live_r + MARBLE_RADIUS + 2.0 * MARBLE_RADIUS;
                    rscale = rscale.max((min_r / r_top).min(1.0));
                }
            }
        }
        for (m, params) in &tracks {
            let (pid, live, screen_id) = &marbles[*m];
            let Some(planet) = h.planets.get_mut(pid) else {
                continue;
            };
            // A new sun (first join, host switch, or a barycenter
            // recapture) re-arms the ease from the live frame — joins
            // never teleport.
            let just_joined = planet
                .orbit
                .as_ref()
                .is_none_or(|o| o.sun_key != sun_key);
            if just_joined {
                planet.orbit = Some(MarbleOrbit {
                    sun_key: sun_key.clone(),
                    lerp_start: now,
                    lerp_from: *live,
                    front: true,
                });
            }
            if planet.pin_until.is_some_and(|until| now < until) {
                // Drag wins: sit, and keep the ease armed at the live
                // frame so the rejoin glides onto the track instead of
                // jumping. The pin never stills anyone else.
                if let Some(o) = planet.orbit.as_mut() {
                    o.lerp_start = now;
                    o.lerp_from = *live;
                }
                continue;
            }
            // Even speed along the path: constant dθ, no Kepler.
            let theta =
                params.phase0 + params.dir * std::f64::consts::TAU * (t / params.period);
            // This marble's own ring radius right now: the LIVE host
            // circle, or the hashed bary oval radius.
            let r_now = match ring_r {
                Some(r) => r * rscale,
                None => params.r * rscale,
            };
            let (dx, dy) = if ring_r.is_some() {
                circle_offset(r_now, theta, rot)
            } else {
                oval_offset(r_now, theta, rot)
            };
            let target = NSPoint {
                x: sun.x + dx,
                y: sun.y + dy,
            };
            // Join ease: ~6s ease-IN from the armed live frame onto THIS
            // marble's own ring, POLAR — θ (shortest arc) and r ease
            // along the ring, never a cartesian straight line through
            // the chip, never a lerp toward another marble.
            let (lerp_start, lerp_from) = planet
                .orbit
                .as_ref()
                .map(|o| (o.lerp_start, o.lerp_from))
                .unwrap_or((now, *live));
            let f = (now.duration_since(lerp_start).as_secs_f64() / JOIN_EASE_SECS).min(1.0);
            let mut pos = if f >= 1.0 {
                target
            } else {
                let e = f * f;
                // Live frame → the track's unrotated frame, then ease
                // angle + radius and re-apply the shape.
                let (rs, rc) = rot.sin_cos();
                let lx = lerp_from.x - sun.x;
                let ly = lerp_from.y - sun.y;
                let fx = lx * rc + ly * rs;
                let fy = ly * rc - lx * rs;
                let from_phi = fy.atan2(fx);
                let from_r = fx.hypot(fy);
                let dphi = (theta - from_phi + std::f64::consts::PI)
                    .rem_euclid(std::f64::consts::TAU)
                    - std::f64::consts::PI;
                let phi = from_phi + dphi * e;
                let r = from_r + (r_now - from_r) * e;
                let (ex, ey) = if ring_r.is_some() {
                    circle_offset(r, phi, rot)
                } else {
                    oval_offset(r, phi, rot)
                };
                NSPoint {
                    x: sun.x + ex,
                    y: sun.y + ey,
                }
            };
            // Collider LAW (not a window, never drawn): vs every expanded
            // planet on this screen the marble CENTER stays ≥ live radius
            // + MARBLE_RADIUS from the planet center. A point inside is
            // pushed out along the bearing from the planet — marbles go
            // AROUND the disc, never through. Catches join leftovers and
            // bary clips; the 3-width ring never triggers it.
            for (_, ec, esid, er) in &expanded {
                if esid != screen_id {
                    continue;
                }
                let min_d = *er + MARBLE_RADIUS;
                let dx = pos.x - ec.x;
                let dy = pos.y - ec.y;
                let d = dx.hypot(dy);
                if d < min_d {
                    let (ux, uy) = if d > 1e-6 { (dx / d, dy / d) } else { (1.0, 0.0) };
                    pos = NSPoint {
                        x: ec.x + ux * min_d,
                        y: ec.y + uy * min_d,
                    };
                }
            }
            let panel = planet.panel as *mut Object;
            unsafe {
                let frame: NSRect = msg_send![panel, frame];
                let _: () = msg_send![panel,
                    setFrameOrigin: NSPoint {
                        x: pos.x - frame.size.width / 2.0,
                        y: pos.y - frame.size.height / 2.0,
                    }];
            }
            // Z-law: boolean front/back vs THIS marble's own host only.
            // sin θ with an equator deadband so the bit never chatters.
            // orderWindow:relativeTo: fires only when the bit flips (or
            // on join, to establish stacking vs the new host) — never a
            // per-tick group restack.
            if let Some((host_panel, _)) = host_info {
                let s = theta.sin();
                let prev = planet.orbit.as_ref().map(|o| o.front).unwrap_or(true);
                let mut front = prev;
                if s > FRONT_DEADBAND {
                    front = true;
                } else if s < -FRONT_DEADBAND {
                    front = false;
                }
                if just_joined || front != prev {
                    let host = host_panel as *mut Object;
                    unsafe {
                        let win: i64 = msg_send![host, windowNumber];
                        // NSWindowAbove = 1, NSWindowBelow = -1.
                        let mode: i64 = if front { 1 } else { -1 };
                        let _: () = msg_send![panel, orderWindow: mode relativeTo: win];
                    }
                    if let Some(o) = planet.orbit.as_mut() {
                        o.front = front;
                    }
                }
            }
            // Live position in memory only — screen-local flipped, same
            // as persist_position. The orbit path NEVER dirties
            // worlds.json; pin/drop/toggle/quit persist directly.
            if let Some((_, sframe)) = all.iter().find(|(id, _)| id == screen_id) {
                if let Some(w) = h.worlds.get_mut(pid) {
                    w.x = pos.x - sframe.origin.x;
                    w.y = sframe.origin.y + sframe.size.height - pos.y;
                }
            }
        }
    }
}
