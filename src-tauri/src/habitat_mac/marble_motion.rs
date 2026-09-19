//! Marble drift_pass + orbit restore (DIET-HABITAT tick carve).
//!
//! Extracted from habitat_mac/tick.rs.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::time::Instant;

use super::{
    screens, visible_frames, ClusterState, Habitat, MarbleOrbit, NSPoint, NSRect,
    BARY_PREFIX, BARY_R_MAX, BARY_R_MIN, DRIFT_JOIN_DIST, DRIFT_LEAVE_DIST, FRONT_DEADBAND,
    HOST_JOIN_DIST, HOST_LEAVE_DIST, HOST_PREFIX, HOST_SWITCH_CLOSER, JOIN_EASE_SECS,
    MARBLE_RADIUS, OVAL_RATIO, TRACK_T_MAX, TRACK_T_MIN,
};

/// One marble drift turn. Track law is on the constants above
/// (HOST_JOIN_DIST, OVAL_RATIO, DRIFT_PIN_SECS). Never write worlds.json.
pub(crate) fn drift_pass(h: &mut Habitat) {
    let now = Instant::now();
    // (project_id, global center, screen_id) for every live marble, and
    // for every expanded planet — the candidate suns. Expanded planets
    // are never moved by us.
    let mut marbles: Vec<(String, NSPoint, String)> = Vec::new();
    // (project_id, global center, screen_id, LIVE radius) for expanded
    // planets — the radius is the scroll-zoomed drawn disc, so the ring
    // and the collider follow a growing chip.
    let mut expanded: Vec<(String, NSPoint, String, f64)> = Vec::new();
    for (pid, planet) in h.planets.iter() {
        let Some(w) = h.worlds.get(pid) else { continue };
        let panel = planet.panel as *mut Object;
        let frame: NSRect = unsafe { msg_send![panel, frame] };
        let center = NSPoint {
            x: frame.mid_x(),
            y: frame.mid_y(),
        };
        if planet.is_marble {
            marbles.push((pid.clone(), center, w.screen_id.clone()));
        } else {
            expanded.push((pid.clone(), center, w.screen_id.clone(), planet.radius));
        }
    }
    // Last tick's sun key per marble — the host / leave-edge hysteresis
    // memory.
    let prev_sun: HashMap<String, String> = marbles
        .iter()
        .filter_map(|(pid, _, _)| {
            let key = h.planets.get(pid)?.orbit.as_ref()?.sun_key.clone();
            Some((pid.clone(), key))
        })
        .collect();
    let all = unsafe { screens() };
    let visible = unsafe { visible_frames() };
    let t = h.drift_phase;
    let pad = MARBLE_RADIUS + 8.0;
    // Barycenter keys still alive this tick — stale homes drop at the end.
    let mut live_bary_keys: Vec<String> = Vec::new();
    // Sun 1: the nearest expanded planet on the same screen
    // (marble index, sun key, sun center) for hosted marbles; the rest
    // fall through to the barycenter pool.
    let mut hosted: Vec<(usize, String, NSPoint)> = Vec::new();
    let mut free: Vec<usize> = Vec::new();
    for (i, (pid, center, screen_id)) in marbles.iter().enumerate() {
        // Candidate suns on this screen: (planet id, center, distance off
        // the LIVE disc edge). Join/leave/switch all measure edge distance
        // (center dist − live radius, floored at 0) so the same 280/360/80
        // law holds at any zoom — a bigger chip keeps its ring.
        let cands: Vec<(&String, NSPoint, f64)> = expanded
            .iter()
            .filter(|(_, _, sid, _)| sid == screen_id)
            .map(|(id, c, _, live_r)| {
                let d = (c.x - center.x).hypot(c.y - center.y);
                (id, *c, (d - live_r).max(0.0))
            })
            .collect();
        let cur_host = prev_sun
            .get(pid)
            .and_then(|k| k.strip_prefix(HOST_PREFIX));
        let mut chosen: Option<(&str, NSPoint)> = None;
        if let Some(cur_id) = cur_host {
            if let Some((id, c, d)) = cands.iter().find(|(id, _, _)| id.as_str() == cur_id) {
                if *d <= HOST_LEAVE_DIST {
                    chosen = Some((id.as_str(), *c));
                    // Host hysteresis: switch only if a rival sun on this
                    // screen is ≥ HOST_SWITCH_CLOSER pt closer, so the sun
                    // never flickers between two near planets.
                    if let Some((nid, nc, nd)) = cands
                        .iter()
                        .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                    {
                        if nid.as_str() != cur_id && *nd <= d - HOST_SWITCH_CLOSER {
                            chosen = Some((nid.as_str(), *nc));
                        }
                    }
                }
            }
        }
        if chosen.is_none() {
            chosen = cands
                .iter()
                .filter(|(_, _, d)| *d <= HOST_JOIN_DIST)
                .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(id, c, _)| (id.as_str(), *c));
        }
        match chosen {
            Some((epid, c)) => hosted.push((i, format!("{HOST_PREFIX}{epid}"), c)),
            None => free.push(i),
        }
    }
    // Sun 2: the frozen barycenter of the unhosted cluster
    // Union-find over hysteresis edges among FREE marbles: ≤ join always
    // links; ≤ leave links only pairs that shared a sun last tick.
    let rn = free.len();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut parent: Vec<usize> = (0..rn).collect();
    for a in 0..rn {
        for b in (a + 1)..rn {
            let (ia, ib) = (free[a], free[b]);
            if marbles[ia].2 != marbles[ib].2 {
                continue; // same screen only — never hop displays
            }
            let d =
                (marbles[ia].1.x - marbles[ib].1.x).hypot(marbles[ia].1.y - marbles[ib].1.y);
            let linked = d <= DRIFT_JOIN_DIST
                || (d <= DRIFT_LEAVE_DIST
                    && matches!(
                        (prev_sun.get(&marbles[ia].0), prev_sun.get(&marbles[ib].0)),
                        (Some(x), Some(y)) if x == y
                    ));
            if linked {
                let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                if ra != rb {
                    parent[ra] = rb;
                }
            }
        }
    }
    let mut comps: HashMap<usize, Vec<usize>> = HashMap::new();
    for k in 0..rn {
        let root = find(&mut parent, k);
        comps.entry(root).or_default().push(k);
    }
    // (sun key, sun center, member marble indices, is_host) for every
    // live sun this tick.
    let mut groups: Vec<(String, NSPoint, Vec<usize>, bool)> = Vec::new();
    {
        let mut by_host: HashMap<String, (NSPoint, Vec<usize>)> = HashMap::new();
        for (i, key, c) in hosted {
            by_host.entry(key).or_insert_with(|| (c, Vec::new())).1.push(i);
        }
        for (key, (c, idxs)) in by_host {
            groups.push((key, c, idxs, true));
        }
    }
    for members in comps.into_values() {
        if members.len() < 2 {
            // Isolate with no sun: sit, orbit state cleared.
            let (pid, _, _) = &marbles[free[members[0]]];
            if let Some(planet) = h.planets.get_mut(pid) {
                planet.orbit = None;
            }
            continue;
        }
        let mut ids: Vec<&str> = members
            .iter()
            .map(|k| marbles[free[*k]].0.as_str())
            .collect();
        ids.sort_unstable();
        let key = format!("{BARY_PREFIX}{}", ids.join("\n"));
        if !h.clusters.contains_key(&key) {
            // Recapture: home = the live centroid ONCE. Never walked —
            // the figure cannot drift across the desk.
            let (mut sx, mut sy) = (0.0, 0.0);
            for k in &members {
                let c = marbles[free[*k]].1;
                sx += c.x;
                sy += c.y;
            }
            let n = members.len() as f64;
            h.clusters.insert(
                key.clone(),
                ClusterState {
                    home: NSPoint {
                        x: sx / n,
                        y: sy / n,
                    },
                },
            );
        }
        let home = h.clusters[&key].home;
        live_bary_keys.push(key.clone());
        groups.push((
            key,
            home,
            members.iter().map(|k| free[*k]).collect(),
            false,
        ));
    }
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
    // Drop barycenter homes whose cluster dissolved (isolation /
    // recapture under a new key).
    h.clusters.retain(|k, _| live_bary_keys.iter().any(|lk| lk == k));
}

/// Screen-space offset on the mild oval at angle θ with semi-major r:
/// the minor axis is OVAL_RATIO · r and the whole oval is rotated in the
/// plane by the sun's hashed orientation. Constant θ speed on an
/// almost-round oval reads as even along the path. Bary tracks only —
/// hosted tracks ride the live-radius circle below (a 0.88 squash would
/// pull the ring onto the face).
fn oval_offset(r: f64, theta: f64, rot: f64) -> (f64, f64) {
    let (s, c) = theta.sin_cos();
    let ex = r * c;
    let ey = OVAL_RATIO * r * s;
    let (rs, rc) = rot.sin_cos();
    (ex * rc - ey * rs, ex * rs + ey * rc)
}

/// RIM, standoff cut to two widths in
/// the hosted ring radius — a CIRCLE around the
/// planet center at the LIVE (scroll-zoomed) radius + two
/// marble-widths + the marble radius. Bigger chip → bigger ring, always
/// ~2 widths off the live edge. Constant r: no accordion, no back tuck.
fn host_ring_r(live_radius: f64) -> f64 {
    live_radius + 2.0 * (2.0 * MARBLE_RADIUS) + MARBLE_RADIUS
}

/// screen-space offset on the hosted CIRCLE at
/// angle θ — constant r around the PLANET center, rotated in the plane
/// by the sun's hashed orientation (front/back is the plane's sin θ,
/// not screen-down; the same θ drives the front/back boolean). No oval
/// squash, drawn scale stays 1.0.
fn circle_offset(r: f64, theta: f64, rot: f64) -> (f64, f64) {
    let (s, c) = theta.sin_cos();
    let (rs, rc) = rot.sin_cos();
    (r * (c * rc - s * rs), r * (c * rs + s * rc))
}

/// FNV-1a over the key: stable hashes for track/orientation picks, no
/// rand, same input → same track every launch.
fn fnv1a(key: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// one marble's own track, FNV-1a over the
/// marble's id — radius ∈ r_min–r_max (bary tracks only, 70–140pt;
/// hosted tracks take the r(θ) halo law instead), period ∈ 90–140s,
/// independent phase0 and direction. Every marble on a sun gets a
/// DIFFERENT ring: never opposite ends of a stick, never one shared θ.
#[derive(Clone, Copy)]
struct TrackParams {
    r: f64,
    period: f64,
    phase0: f64,
    dir: f64,
}

fn track_params(id: &str, r_min: f64, r_max: f64) -> TrackParams {
    let hash = fnv1a(id);
    let pick = |shift: u32| ((hash >> shift) & 0xffff) as f64 / 65535.0;
    TrackParams {
        r: r_min + (r_max - r_min) * pick(0),
        period: TRACK_T_MIN + (TRACK_T_MAX - TRACK_T_MIN) * pick(16),
        phase0: std::f64::consts::TAU * pick(32),
        dir: if hash & 1 == 1 { 1.0 } else { -1.0 },
    }
}

/// One hashed in-plane rotation per SUN (host planet id or barycenter
/// key) — the whole solar system shares an orientation, marbles don't
/// pick their own planes.
fn sun_rot(sun_key: &str) -> f64 {
    std::f64::consts::TAU * ((fnv1a(sun_key) & 0xffff) as f64 / 65535.0)
}

/// motion off (toggle or Reduce Motion) →
/// marbles sit and drift state drops, so a re-enable re-seeds from the
/// live frame. Positions are untouched — worlds.json keeps the live
/// x/y either way.
pub(crate) fn orbit_restore(h: &mut Habitat) {
    if h.clusters.is_empty()
        && h.planets.values().all(|p| {
            !p.is_marble || (p.orbit.is_none() && (p.marble_scale - 1.0).abs() <= 0.001)
        })
    {
        return;
    }
    h.clusters.clear();
    for planet in h.planets.values_mut() {
        if !planet.is_marble {
            continue;
        }
        planet.orbit = None;
        if (planet.marble_scale - 1.0).abs() > 0.001 {
            planet.marble_scale = 1.0;
            unsafe {
                let view = planet.view as *mut Object;
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
        }
    }
}

