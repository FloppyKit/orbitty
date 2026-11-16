//! One walker's pose step for a single anim tick (DIET-HABITAT).
//!
//! Sit snaps. Stand/Work walk the lip or the grass. WalkOff checks out
//! then goes home. Roam ambles until someone is working. Peeled from
//! `anim_planet.rs`.

use std::time::{Duration, Instant};

use super::droid_pack;
use super::pose::on_sit_station;
use super::{
    emit_board_checkin, station_target, yard_clamp, yard_contains, yard_dwell_ms, yard_pick,
    yard_pick_sailing, yard_seed, Pose, RoamState, HOME_HIDE_X, YARD_ARRIVE, YARD_SAIL,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn step_pose(
    roam: &mut RoamState,
    pose: Pose,
    id: &str,
    i: usize,
    dt: f64,
    now_i: Instant,
    bob_phase: f64,
    house: (f64, f64),
    lip: (f64, f64),
    keep_up: bool,
    any_work_sit: bool,
    is_droid: bool,
    still_todo: bool,
    checkout_board: bool,
    moved: &mut bool,
    dirty: &mut bool,
    stepping: &mut bool,
) {
match pose {
    // Needed: bad news skips the queue — snap to the gate, never
    // a scenic route. Sit = idle frame 0, no bob, full bright.
    Pose::Sit => {
        let (tx, ty) = station_target(Pose::Sit, id);
        if (roam.x - tx).abs() > 1e-9 || (roam.y - ty).abs() > 1e-9 {
            roam.x = tx;
            roam.y = ty;
            *moved = true;
        }
        roam.tx = tx;
        roam.ty = ty;
        roam.pause_end = None;
    }
    // Stand walks to the board lip (not the post foot).
    // Work ambles the grass AABB. Never moonwalk on one pixel.
    Pose::Stand | Pose::Work => {
        // Back from off the left rim (re-claim after Done):
        // the walker was invisible, so re-enter AT THE IGLOO
        // and walk the return — never a snap onto the station.
        if roam.x < HOME_HIDE_X {
            roam.x = house.0;
            roam.y = house.1;
            // the return from hide
            // is a fresh claim journey — re-arm the board
            // check-in so this claim touches the board too.
            roam.via_checkin = false;
            *moved = true;
        }
        if pose == Pose::Stand {
            roam.tx = lip.0;
            roam.ty = lip.1;
            roam.pause_end = None;
        } else {
            if roam.wander_seed == 0 {
                roam.wander_seed = yard_seed(id);
                let (tx, ty) = yard_pick(&mut roam.wander_seed);
                roam.tx = tx;
                roam.ty = ty;
                roam.pause_end = None;
            } else if !yard_contains(roam.tx, roam.ty) {
                let (tx, ty) = yard_pick(&mut roam.wander_seed);
                roam.tx = tx;
                roam.ty = ty;
                roam.pause_end = None;
            }
            if !keep_up {
                // Ball off: the amble never dwells — a pause
                // left by a mid-dwell toggle-off dies now,
                // never a standing park on the green.
                roam.pause_end = None;
            }
        }
        let mut dx = roam.tx - roam.x;
        let mut dy = roam.ty - roam.y;
        let mut dist = (dx * dx + dy * dy).sqrt();
        // CHECKIN (+ PROPS leftover retry): a
        // Stand (Todo) walker at the near lip of the board
        // checks in — the rail flips that window's card
        // todo→doing and the next sync walks it to the field
        // as Work. The emit is fire-and-forget, so while the
        // snapshot STILL says todo it re-fires on a ~0.5s
        // throttle: a missed rail (or an AX-untrusted no-op
        // sync) used to park the walker at the lip forever.
        // via_checkin is NOT set on this auto path — it stays
        // the "user dropped on Todo, don't auto-flip" flag
        // (only the drag-drop column paths set it). The rail's
        // listener is idempotent (todo→doing only), so the
        // repeats are safe.
        if pose == Pose::Stand && !roam.via_checkin && dist < 0.05 {
            let due = roam
                .checkin_emit_at
                .is_none_or(|t| t.elapsed() >= Duration::from_millis(500));
            if still_todo && due {
                roam.checkin_emit_at = Some(now_i);
                emit_board_checkin(id);
            }
        }
        if pose == Pose::Work {
            if keep_up {
                // Ball ON — the exact pre-amble dart: on the
                // grass point (dist < YARD_ARRIVE, 0.04)
                // dwell 600–1400 ms (hashed off the stream)
                // STANDING — the dwell is the pause, no step,
                // no bob — then pick a NEW random point in
                // the box and dart on.
                if dist < YARD_ARRIVE && roam.pause_end.is_none() {
                    // Arrival slop (<= YARD_ARRIVE) snaps onto
                    // the green — the one clamp that can move
                    // a body, sub-0.04, invisible.
                    let (cx, cy) = yard_clamp(roam.x, roam.y);
                    roam.x = cx;
                    roam.y = cy;
                    let dwell = yard_dwell_ms(&mut roam.wander_seed);
                    roam.pause_end =
                        Some(now_i + Duration::from_millis(dwell));
                    *dirty = true; // point reached — the walk cycle parks
                }
                if let Some(end) = roam.pause_end {
                    if now_i < end {
                        return; // dwelling on the point
                    }
                    roam.pause_end = None;
                    let (tx, ty) = yard_pick(&mut roam.wander_seed);
                    roam.tx = tx;
                    roam.ty = ty;
                    dx = roam.tx - roam.x;
                    dy = roam.ty - roam.y;
                    dist = (dx * dx + dy * dy).sqrt();
                    *dirty = true; // new point — the walk cycle resumes
                }
            } else if dist < YARD_SAIL {
                // Ball OFF — smooth wander: BEFORE arriving
                // (dist < YARD_SAIL, 0.12) the NEXT point is
                // already picked — no pause, no snap-turn.
                // The pick prefers a heading that is not a
                // reverse of the current dx,dy (an about-face
                // is skipped up to 3 stream steps), so the
                // body curves onto the next leg instead of
                // darting back.
                let (tx, ty) = yard_pick_sailing(
                    &mut roam.wander_seed,
                    roam.x,
                    roam.y,
                    dx,
                    dy,
                );
                roam.tx = tx;
                roam.ty = ty;
                dx = roam.tx - roam.x;
                dy = roam.ty - roam.y;
                dist = (dx * dx + dy * dy).sqrt();
                *dirty = true; // target handed off mid-stride
            }
        }
        if dist >= 0.025 {
            // Ball OFF: slow amble on the grass (0.045·dt —
            // ball-in-the-air leftover: 0.075 still read
            // brisk). Ball ON: the dart keeps the full
            // 0.16·dt on the green — the keep-up footwork.
            // Either way the off-grass walk-IN (from the
            // igloo / board lip / a hand drop) keeps the old
            // 0.16·dt — the visible approach is unchanged.
            let speed = if pose == Pose::Work
                && !keep_up
                && yard_contains(roam.x, roam.y)
            {
                0.045
            } else {
                0.16
            };
            let step = (speed * dt).min(dist);
            roam.x += dx / dist * step;
            roam.y += dy / dist * step;
            roam.facing_left = dx < -0.002;
            roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
            *stepping = true;
            *moved = true; // walking (to the lip / across the grass)
        }
        // WORLD-YARD law 6: a Doing walker ON the grass never
        // leaves it — hard clamp to the inset AABB (a no-op on
        // the convex walk between two in-box points; the wall
        // against drift/bugs — never the igloo x <= -0.50,
        // never the sign footprint). The walk-IN from the
        // igloo / board lip / a hand drop stays a visible
        // approach: its target is in the box and converges —
        // clamping THAT leg would pop the body onto the edge.
        if pose == Pose::Work && yard_contains(roam.x, roam.y) {
            let (cx, cy) = yard_clamp(roam.x, roam.y);
            if (cx - roam.x).abs() > 1e-12 || (cy - roam.y).abs() > 1e-12 {
                roam.x = cx;
                roam.y = cy;
                *moved = true;
            }
        }
    }
    Pose::WalkOff => {
        super::anim_walkoff::step_walkoff(
            roam,
            id,
            dt,
            now_i,
            house,
            lip,
            is_droid,
            checkout_board,
            moved,
            dirty,
            stepping,
        );
    }
    Pose::Roam => {
        // GO 57: a Needed spell leaves the body ON the front rim — the
        // Sit seed and the Sit snap write the rim into x, y, tx and ty.
        // Roam is not a sit, so a walker that should be on the grass
        // walks back onto the pad instead of standing on the rim for
        // good (the park below would otherwise freeze it there).
        if on_sit_station(roam.x, roam.y, id) {
            if roam.wander_seed == 0 {
                roam.wander_seed = yard_seed(id);
            }
            let (tx, ty) = yard_pick(&mut roam.wander_seed);
            roam.tx = tx;
            roam.ty = ty;
            roam.pause_end = None;
        }
        // Still on that walk onto the grass: finish the leg before
        // parking. The target is a grass point and the body is not on
        // it yet, so the park would strand it short of the pad (the
        // yard's south edge is only 0.02 from the rim).
        let onto_grass = yard_contains(roam.tx, roam.ty)
            && ((roam.tx - roam.x).powi(2) + (roam.ty - roam.y).powi(2)).sqrt() >= 0.025;
        if any_work_sit && !onto_grass {
            // Amble off while anyone works/sits: stand in
            // place — no re-park, no pause cycle.
            roam.tx = roam.x;
            roam.ty = roam.y;
            roam.pause_end = None;
            return;
        }
        // Beach ball on: keep-up footwork stays on the grass
        // pad (same AABB as Doing). Off: ambient disc circle.
        if keep_up {
            if roam.wander_seed == 0 {
                roam.wander_seed = yard_seed(id);
            }
            if !yard_contains(roam.x, roam.y) {
                let (cx, cy) = yard_clamp(roam.x, roam.y);
                roam.x = cx;
                roam.y = cy;
                *moved = true;
            }
            if !yard_contains(roam.tx, roam.ty) {
                let (tx, ty) = yard_pick(&mut roam.wander_seed);
                roam.tx = tx;
                roam.ty = ty;
                roam.pause_end = None;
            }
            let mut dx = roam.tx - roam.x;
            let mut dy = roam.ty - roam.y;
            let mut dist = (dx * dx + dy * dy).sqrt();
            if dist < YARD_ARRIVE && roam.pause_end.is_none() {
                let (cx, cy) = yard_clamp(roam.x, roam.y);
                roam.x = cx;
                roam.y = cy;
                let dwell = yard_dwell_ms(&mut roam.wander_seed);
                roam.pause_end =
                    Some(now_i + Duration::from_millis(dwell));
                *dirty = true;
            }
            if let Some(end) = roam.pause_end {
                if now_i < end {
                    return;
                }
                roam.pause_end = None;
                let (tx, ty) = yard_pick(&mut roam.wander_seed);
                roam.tx = tx;
                roam.ty = ty;
                dx = roam.tx - roam.x;
                dy = roam.ty - roam.y;
                dist = (dx * dx + dy * dy).sqrt();
                *dirty = true;
            }
            if dist >= 0.025 {
                let step = (0.16 * dt).min(dist);
                roam.x += dx / dist * step;
                roam.y += dy / dist * step;
                roam.facing_left = dx < -0.002;
                roam.facing_dir =
                    droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
                *stepping = true;
                *moved = true;
            }
            let (cx, cy) = yard_clamp(roam.x, roam.y);
            if (cx - roam.x).abs() > 1e-12 || (cy - roam.y).abs() > 1e-12 {
                roam.x = cx;
                roam.y = cy;
                *moved = true;
            }
            return;
        }
        if let Some(end) = roam.pause_end {
            if now_i < end {
                return;
            }
            // Pause ended — both branches below (re-park / step)
            // mark the dirty walk-cycle change.
            roam.pause_end = None;
        }
        let dx = roam.tx - roam.x;
        let dy = roam.ty - roam.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 0.025 {
            let a = bob_phase * 2.7 + i as f64 * 4.1;
            let rad = 0.22 + 0.42 * ((bob_phase * 1.3 + i as f64).sin() * 0.5 + 0.5);
            roam.tx = a.cos() * rad;
            roam.ty = a.sin() * rad;
            roam.pause_end = Some(
                now_i + Duration::from_millis(700 + ((i as u64 * 410) % 1400)),
            );
            *dirty = true; // target reached — the walk cycle parks
        } else {
            let step = (0.16 * dt).min(dist);
            roam.x += dx / dist * step;
            roam.y += dy / dist * step;
            roam.facing_left = dx < -0.002;
            roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
            *stepping = true;
            *moved = true; // roam stepped
        }
    }
}
}

#[cfg(test)]
mod grass_rim_tests {
    use super::*;
    use crate::habitat::habitat_mac::pose::{pose_for, STATION_RIM};
    use crate::habitat::habitat_mac::{
        station_target, yard_contains, Pose, RoamState, STATION_HOUSE,
    };
    use crate::habitat::HabitatEntry;

    /// A claimed window (no agent seed, no door word) in `status`.
    fn window_claim(status: &str, id: &str) -> HabitatEntry {
        HabitatEntry {
            id: id.into(),
            app_name: "app".into(),
            title: "window".into(),
            bundle_id: None,
            minimized: false,
            icon_base64: None,
            age: "fresh".into(),
            sprite_pack_id: String::new(),
            char_seed: 0,
            char_look: None,
            sprite_scale: 0.0,
            asleep: false,
            card_status: status.into(),
            card_kind: "window".into(),
            needed_state: String::new(),
            agent_seed: String::new(),
            outcome: String::new(),
        }
    }

    /// One walker's roam cell, all flags at their fresh-claim values.
    fn roam_at(x: f64, y: f64, tx: f64, ty: f64) -> RoamState {
        RoamState {
            x,
            y,
            tx,
            ty,
            pause_end: None,
            facing_left: false,
            facing_dir: 0,
            off_emitted: false,
            via_board: false,
            via_home: false,
            via_checkin: false,
            checkin_emit_at: None,
            wander_seed: 0,
        }
    }

    /// house_spawn: the doorstep, where a fresh claim walks in from.
    fn house() -> (f64, f64) {
        (STATION_HOUSE.0 + 0.12, STATION_HOUSE.1)
    }

    /// The front rim, as the Sit seed / snap leaves it (exact station).
    fn rim(id: &str) -> (f64, f64) {
        station_target(Pose::Sit, id)
    }

    /// The board's near lip is not what these pin, so any south point does.
    fn lip() -> (f64, f64) {
        (0.0, 0.30)
    }

    /// Ids that cover both rim offsets: inside the inset yard and past it.
    const IDS: [&str; 4] = ["agent:c1", "agent:c2", "window:1", "window:2"];

    /// Walk one pose for `secs` at the anim cadence, ball OFF, someone
    /// else working (any_work_sit), the walker itself not a droid chore.
    fn settle(r: &mut RoamState, pose: Pose, id: &str, secs: f64, any_work_sit: bool) {
        let dt = 1.0 / 60.0;
        for _ in 0..(secs / dt) as usize {
            let mut moved = false;
            let mut dirty = false;
            let mut stepping = false;
            step_pose(
                r,
                pose,
                id,
                0,
                dt,
                std::time::Instant::now(),
                0.0,
                house(),
                lip(),
                false,
                any_work_sit,
                true,
                false,
                true,
                &mut moved,
                &mut dirty,
                &mut stepping,
            );
        }
    }

    /// Still inside the front-rim disc of STATION_RIM (max Sit offset 0.08)?
    fn on_rim(r: &RoamState) -> bool {
        (r.x - STATION_RIM.0).abs() <= 0.09 && (r.y - STATION_RIM.1).abs() <= 0.09
    }

    #[test]
    fn work_from_the_house_reaches_the_grass() {
        for id in IDS {
            let (tx, ty) = station_target(Pose::Work, id);
            let mut r = roam_at(house().0, house().1, tx, ty);
            settle(&mut r, Pose::Work, id, 8.0, true);
            assert!(yard_contains(r.x, r.y), "{id} not on the grass: ({}, {})", r.x, r.y);
            assert!(!on_rim(&r), "{id} parked on the front rim: ({}, {})", r.x, r.y);
        }
    }

    #[test]
    fn work_seeded_on_the_rim_walks_onto_the_grass() {
        for id in IDS {
            let (sx, sy) = rim(id);
            let mut r = roam_at(sx, sy, sx, sy);
            settle(&mut r, Pose::Work, id, 8.0, true);
            assert!(yard_contains(r.x, r.y), "{id} not on the grass: ({}, {})", r.x, r.y);
            assert!(!on_rim(&r), "{id} still on the front rim: ({}, {})", r.x, r.y);
        }
    }

    /// GO 57 design 4: the first snapshot may seed Sit, a later one may
    /// say the walker is Doing. Keeping the roam entry must not freeze a
    /// non-Needed walker on the rim.
    #[test]
    fn doing_walker_left_on_the_rim_walks_onto_the_grass() {
        for id in IDS {
            let (sx, sy) = rim(id);
            let mut r = roam_at(sx, sy, sx, sy);
            settle(&mut r, Pose::Roam, id, 8.0, true);
            assert!(yard_contains(r.x, r.y), "{id} not on the grass: ({}, {})", r.x, r.y);
            assert!(!on_rim(&r), "{id} stuck on the front rim: ({}, {})", r.x, r.y);
        }
    }

    /// The ambience still parks where it stands — the rim is the only
    /// leftover this walks home (no migrating every idle walker).
    #[test]
    fn doing_walker_parked_off_the_rim_stays_put() {
        for id in IDS {
            let mut r = roam_at(0.0, -0.90, 0.0, -0.90);
            settle(&mut r, Pose::Roam, id, 4.0, true);
            assert_eq!((r.x, r.y), (0.0, -0.90), "{id} moved off the disc north");
        }
    }

    /// A real Needed sit is the station itself: it stays, exactly.
    #[test]
    fn needed_sit_stays_on_the_rim() {
        for id in IDS {
            let (sx, sy) = rim(id);
            let mut r = roam_at(sx, sy, sx, sy);
            settle(&mut r, Pose::Sit, id, 8.0, true);
            assert_eq!((r.x, r.y), (sx, sy), "{id} left the Sit station");
            assert!(on_rim(&r), "{id} not on the rim: ({}, {})", r.x, r.y);
        }
    }

    /// FIX2 (Matt 2026-11-16): a claimed kind:"window" ticket (todo or
    /// doing) is not a Needed and not board work — it ambles the grass
    /// pad. Seeded out on the ambient disc circle it must walk onto the
    /// pad and stay there, not roam the disc.
    #[test]
    fn window_claim_on_the_disc_walks_onto_the_grass() {
        // Two disc points the ambient circle really reaches, both off the
        // pad: north of it and east of it.
        let disc = [(0.0, -0.62), (0.66, 0.12)];
        for status in ["todo", "doing"] {
            for id in IDS {
                let pose = pose_for(&window_claim(status, id));
                assert_eq!(pose, Pose::Work, "{status} window claim pose");
                for (x, y) in disc {
                    let mut r = roam_at(x, y, x, y);
                    // Nobody else working: the ambient disc circle's case.
                    settle(&mut r, pose, id, 8.0, false);
                    assert!(
                        yard_contains(r.x, r.y),
                        "{status} {id} roamed off the grass from ({x}, {y}): ({}, {})",
                        r.x,
                        r.y
                    );
                }
            }
        }
    }

    /// A claimed window with a real Needed on it still sits the rim —
    /// FIX2 must not drag a Needed onto the grass.
    #[test]
    fn window_claim_with_a_needed_keeps_sitting_the_rim() {
        for id in IDS {
            let mut e = window_claim("doing", id);
            e.needed_state = "open".into();
            assert_eq!(pose_for(&e), Pose::Sit, "{id} needed window pose");
        }
    }

    /// A Todo walker left on the rim walks to the board lip (the third
    /// non-sit pose): the rim is never a standing place it keeps.
    #[test]
    fn stand_left_on_the_rim_walks_to_the_board_lip() {
        for id in IDS {
            let (sx, sy) = rim(id);
            let mut r = roam_at(sx, sy, sx, sy);
            settle(&mut r, Pose::Stand, id, 8.0, true);
            let d = ((r.x - lip().0).powi(2) + (r.y - lip().1).powi(2)).sqrt();
            assert!(d < 0.03, "{id} not at the board lip: ({}, {})", r.x, r.y);
        }
    }
}
