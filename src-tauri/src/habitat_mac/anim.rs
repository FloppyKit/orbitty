//! Walker/planet anim_tick (DIET-HABITAT tick carve).
//!
//! Extracted from habitat_mac/tick.rs.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use std::time::{Duration, Instant};

use super::{
    droid_pack, emit_board_checkin, emit_walked_off, house_spawn, hash_str, layout_beings, lock,
    pack_still_aspect, sleeper_target, station_target, walkoff_target, yard_clamp, yard_contains,
    yard_dwell_ms, yard_pick, yard_pick_sailing, yard_seed, Drag, Pose, RoamState, CAPTION_TTL,
    HOME_HIDE_X, LOCATE_TTL, PLANET_LOCATE_TTL, YARD_ARRIVE, YARD_SAIL,
};

pub(crate) fn anim_tick() {
    let mut h = lock();
    let now = Instant::now();
    let dt = h
        .last_anim
        .map(|t| now.duration_since(t).as_secs_f64())
        .unwrap_or(0.05)
        .min(0.25);
    h.last_anim = Some(now);
    // motion off (toggle or Reduce Motion) →
    // marbles sit at default size and orbit state clears, so a re-enable
    // re-seeds from the live frame. Runs before the Reduce Motion early
    // return so stilling also restores size.
    if h.reduce_motion || !h.marble_drift {
        super::marble_motion::orbit_restore(&mut h);
    }
    if h.reduce_motion {
        return; // static sprites under Reduce Motion
    }
    h.bob_phase += dt;
    h.anim_frame = h.anim_frame.wrapping_add(1);
    let bob_phase = h.bob_phase;
    // BUDGET still-planet law: an expanded planet dirties
    // its view ONLY on real motion this tick — a roam step or pause flip,
    // a bob change, a live locate stroke, or a visible caption. An
    // all-sleeper / empty planet paints nothing per tick; sleepers never
    // keep the view dirty. Marbles already `continue`.
    let locate = h.locate.clone();
    let locate_planet = h.locate_planet.clone();
    // PROPS leftover (town usable): a walker mid-drag is owned by
    // drag_tick — the pose branches below must NOT retarget it back to
    // the station on the same press (that fight was "drag moves roam
    // for a frame; next tick retargets"). The drop itself is handled in
    // drag_tick's release (a column drop is a status move).
    let drag_walker = match &h.drag {
        Some(Drag::Walker {
            project_id,
            entry_id,
            ..
        }) => Some((project_id.clone(), entry_id.clone())),
        _ => None,
    };
    // per-pack still-frame
    // aspects for the keep-up head math, snapshotted before the planet
    // loop (0.0 = no decoded frames → the generated person's height).
    let pack_aspects: Vec<f64> = h
        .pack_frames
        .iter()
        .map(|(_, pf)| pack_still_aspect(pf))
        .collect();
    for (pid, planet) in h.planets.iter_mut() {
        if planet.is_marble {
            continue; // marbles are bare and still
        }
        super::anim_planet::tick_expanded(
            planet,
            pid,
            dt,
            bob_phase,
            &drag_walker,
            &locate,
            &locate_planet,
            &pack_aspects,
        );
    }
    // The marble drift lives in drift_tick, which runs every habitat
    // tick (~16ms) so drifting marbles don't step at this 3-tick walker
    // cadence.
}
