//! Reduce-motion station snap (DIET-HABITAT).
//!
//! anim_tick does not step when Reduce Motion is on, so a posed walker
//! would freeze mid-disc. Snap station poses to their targets here.
//! Roam is left where it is. Peeled from `sync.rs`.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};

use super::draw;
use super::layout_beings;
use super::pose::{station_target, Pose};
use super::Planet;

pub(crate) fn snap_stations(planet: &mut Planet) {
    let posed: Vec<(String, Pose)> = planet
        .walkers
        .iter()
        .filter(|b| !b.asleep || b.pose == Pose::WalkOff)
        .map(|b| (b.entry_id.clone(), b.pose))
        .collect();
    let mut snapped = false;
    for (id, pose) in posed {
        let Some((tx, ty)) = (match pose {
            // Stand snaps to the board's south lip — never the post foot
            // (STATION_BOARD sits behind the sign PNG).
            Pose::Stand => Some(draw::board_near_lip(
                planet.radius,
                planet.projection,
                planet.marble_scale,
            )),
            Pose::WalkOff | Pose::Work | Pose::Sit => Some(station_target(pose, &id)),
            Pose::Roam => None,
        }) else {
            continue;
        };
        if let Some(r) = planet.roam.get_mut(&id) {
            if (r.x - tx).abs() > 1e-9 || (r.y - ty).abs() > 1e-9 {
                r.x = tx;
                r.y = ty;
                r.tx = tx;
                r.ty = ty;
                r.pause_end = None;
                snapped = true;
            }
        }
    }
    if snapped {
        layout_beings(planet);
        unsafe {
            let view = planet.view as *mut Object;
            let _: () = msg_send![view, setNeedsDisplay: true];
        }
    }
}
