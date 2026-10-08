//! One Done walker's checkout for a single anim tick (DIET-HABITAT).
//!
//! Walk to the near lip, dwell, walk home, then unclaim. Extracted from
//! habitat_mac/anim_step.rs `step_pose` WalkOff arm.

use std::time::{Duration, Instant};

use super::droid_pack;
use super::{emit_walked_off, walkoff_target, RoamState, HOME_HIDE_X};

#[allow(clippy::too_many_arguments)]
pub(crate) fn step_walkoff(
    roam: &mut RoamState,
    id: &str,
    dt: f64,
    now_i: Instant,
    house: (f64, f64),
    lip: (f64, f64),
    is_droid: bool,
    checkout_board: bool,
    moved: &mut bool,
    dirty: &mut bool,
    stepping: &mut bool,
) {
    // Done: walk to the near lip of the board, blit flips,
    // dwell so checkout reads, walk home to the igloo, dwell
    // at the door, then unclaim. No north walk-through. No
    // left-rim curtain; last walker lets the planet shrink.
    // Bare / kind:window Done: skip board lip — home then off.
    // Real kind:todo keeps the checkout parade.
    if !checkout_board {
        roam.via_board = true;
    }
    if is_droid {
        let (tx, ty) = walkoff_target(id, roam.x, roam.y);
        let dx = tx - roam.x;
        let dy = ty - roam.y;
        let dist = (dx * dx + dy * dy).sqrt();
        roam.tx = tx;
        roam.ty = ty;
        if dist < 0.05 || roam.x < HOME_HIDE_X {
            if !roam.off_emitted {
                roam.off_emitted = true;
                emit_walked_off(id);
                *dirty = true;
            }
        } else {
            let step = (0.16 * dt).min(dist);
            roam.x += dx / dist * step;
            roam.y += dy / dist * step;
            roam.facing_left = dx < -0.002;
            roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
            *stepping = true;
            *moved = true;
        }
        return;
    }
    if roam.x < HOME_HIDE_X {
        roam.x = house.0;
        roam.y = house.1;
        roam.via_board = false;
        roam.via_home = false;
        roam.off_emitted = false;
        roam.pause_end = None;
        *moved = true;
    }
    if !roam.via_board {
        let dx = lip.0 - roam.x;
        let dy = lip.1 - roam.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 0.05 {
            roam.via_board = true;
            roam.x = lip.0;
            roam.y = lip.1;
            roam.tx = lip.0;
            roam.ty = lip.1;
            roam.pause_end = Some(now_i + Duration::from_millis(400));
            *dirty = true; // lift blit hold this frame
            *moved = true;
        } else {
            roam.tx = lip.0;
            roam.ty = lip.1;
            let step = (0.16 * dt).min(dist);
            roam.x += dx / dist * step;
            roam.y += dy / dist * step;
            roam.facing_left = dx < -0.002;
            roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
            *stepping = true;
            *moved = true;
            return;
        }
    }
    if let Some(end) = roam.pause_end {
        if now_i < end {
            return;
        }
        roam.pause_end = None;
    }
    if !roam.via_home {
        let dx = house.0 - roam.x;
        let dy = house.1 - roam.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 0.05 {
            roam.via_home = true;
            roam.x = house.0;
            roam.y = house.1;
            roam.tx = house.0;
            roam.ty = house.1;
            roam.pause_end = Some(now_i + Duration::from_millis(400));
            *dirty = true;
            *moved = true;
            return;
        } else {
            roam.tx = house.0;
            roam.ty = house.1;
            let step = (0.16 * dt).min(dist);
            roam.x += dx / dist * step;
            roam.y += dy / dist * step;
            roam.facing_left = dx < -0.002;
            roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
            *stepping = true;
            *moved = true;
            return;
        }
    }
    roam.tx = house.0;
    roam.ty = house.1;
    if !roam.off_emitted {
        roam.off_emitted = true;
        emit_walked_off(id);
        *dirty = true;
    }
}
