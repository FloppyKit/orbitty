//! Marble drift_pass + orbit restore (DIET-HABITAT tick carve).
//!
//! Extracted from habitat_mac/tick.rs.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::time::Instant;

use super::marble_tracks::assign_tracks;
use super::{
    screens, visible_frames, ClusterState, Habitat, NSPoint, NSRect,
    BARY_PREFIX, DRIFT_JOIN_DIST, DRIFT_LEAVE_DIST, HOST_JOIN_DIST, HOST_LEAVE_DIST, HOST_PREFIX,
    HOST_SWITCH_CLOSER, MARBLE_RADIUS, OVAL_RATIO, TRACK_T_MAX, TRACK_T_MIN,
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
    assign_tracks(h, groups, marbles, expanded, visible, all, now, t, pad);
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
pub(super) fn oval_offset(r: f64, theta: f64, rot: f64) -> (f64, f64) {
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
pub(super) fn host_ring_r(live_radius: f64) -> f64 {
    live_radius + 2.0 * (2.0 * MARBLE_RADIUS) + MARBLE_RADIUS
}

/// screen-space offset on the hosted CIRCLE at
/// angle θ — constant r around the PLANET center, rotated in the plane
/// by the sun's hashed orientation (front/back is the plane's sin θ,
/// not screen-down; the same θ drives the front/back boolean). No oval
/// squash, drawn scale stays 1.0.
pub(super) fn circle_offset(r: f64, theta: f64, rot: f64) -> (f64, f64) {
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
pub(super) struct TrackParams {
    pub(super) r: f64,
    pub(super) period: f64,
    pub(super) phase0: f64,
    pub(super) dir: f64,
}

pub(super) fn track_params(id: &str, r_min: f64, r_max: f64) -> TrackParams {
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
pub(super) fn sun_rot(sun_key: &str) -> f64 {
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

