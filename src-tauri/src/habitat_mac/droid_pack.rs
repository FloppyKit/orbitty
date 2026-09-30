//! 4-dir FF7 droid kits for agent walkers.
//!
//! Three bodies (dog / penguin / octopus), four folders (e/s/w/n), four
//! walk frames. Seed picks the body. Roam delta picks the folder. Dummy
//! 12×14 in droid.rs stays the fallback if the PNGs are missing.

#![allow(unexpected_cfgs)]

use std::path::PathBuf;
use tauri::Manager;

use super::{app_handle, ns_image_from_bytes, Habitat, Pose};

/// East / south / west / north — south is toward the camera (+disc y).
pub(crate) const DIR_E: u8 = 0;
pub(crate) const DIR_S: u8 = 1;
pub(crate) const DIR_W: u8 = 2;
pub(crate) const DIR_N: u8 = 3;

const KIT_SLUGS: [&str; 3] = [
    "orbitty-droid-dog",
    "orbitty-droid-penguin",
    "orbitty-droid-octopus",
];
const DIRS: [&str; 4] = ["e", "s", "w", "n"];

pub(crate) struct DroidKits {
    /// [kit][dir][frame] retained NSImage pointers.
    walk: [[[usize; 4]; 4]; 3],
}

fn kit_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(app) = app_handle() {
        if let Ok(res) = app.path().resource_dir() {
            roots.push(res.join("droid-crew/packs"));
        }
    }
    roots
}

fn load_png(path: &std::path::Path) -> usize {
    let Ok(bytes) = std::fs::read(path) else {
        return 0;
    };
    let img = unsafe { ns_image_from_bytes(&bytes) };
    if img.is_null() {
        0
    } else {
        img as usize
    }
}

fn load_kits() -> Option<DroidKits> {
    let roots = kit_roots();
    let mut walk = [[[0usize; 4]; 4]; 3];
    for (ki, slug) in KIT_SLUGS.iter().enumerate() {
        let root = roots.iter().find(|r| r.join(slug).join("e/walk_0.png").exists())?;
        let kit = root.join(slug);
        for (di, dir) in DIRS.iter().enumerate() {
            for fi in 0..4 {
                let p = kit.join(dir).join(format!("walk_{fi}.png"));
                let img = load_png(&p);
                if img == 0 {
                    return None;
                }
                walk[ki][di][fi] = img;
            }
        }
    }
    Some(DroidKits { walk })
}

/// Decode once on the main thread. Failed load leaves kits None so
/// droid.rs dummy still draws.
pub(crate) fn ensure_loaded(h: &mut Habitat) {
    if h.droid_kits.is_some() {
        return;
    }
    h.droid_kits = load_kits();
}

pub(crate) fn ready(h: &Habitat) -> bool {
    h.droid_kits.is_some()
}

/// Pick e/s/w/n from a disc-space step. +x east, +y south (toward camera).
///
/// Diagonals face their horizontal travel (E/W by the sign of dx), the
/// same side the window walkers' `facing_left` mirror uses. The old
/// band kept `prev` on any near-diagonal, so a droid that last walked
/// west and then went north-east kept its west sheet — walking
/// backwards. Hysteresis instead of a hold: once horizontal, a step must
/// be clearly vertical (ay ≥ 1.4·ax) to turn N/S; from N/S it turns
/// horizontal below 1.15. Tiny steps keep `prev`.
pub(crate) fn dir_from_delta(dx: f64, dy: f64, prev: u8) -> u8 {
    if dx * dx + dy * dy < 0.002 * 0.002 {
        return prev;
    }
    let ax = dx.abs();
    let ay = dy.abs();
    let was_horizontal = prev == DIR_E || prev == DIR_W;
    let vertical_gate = if was_horizontal { 1.4 } else { 1.15 };
    if ay >= ax * vertical_gate {
        if dy > 0.0 {
            DIR_S
        } else {
            DIR_N
        }
    } else if dx < 0.0 {
        DIR_W
    } else {
        DIR_E
    }
}

/// Sit faces the camera (south). Else the roam's last 4-dir.
pub(crate) fn dir_for(pose: Pose, facing_dir: u8) -> u8 {
    if pose == Pose::Sit {
        DIR_S
    } else {
        facing_dir.min(3)
    }
}

/// NSImage pointer for this droid this tick. 0 = kits missing (dummy).
pub(crate) fn frame_ptr(
    h: &Habitat,
    seed: u64,
    dir: u8,
    cycling: bool,
    anim_frame: u64,
    walker_index: usize,
) -> usize {
    let Some(kits) = h.droid_kits.as_ref() else {
        return 0;
    };
    let kit = (seed % 3) as usize;
    let d = (dir as usize).min(3);
    if cycling {
        let fi = ((anim_frame / 2) as usize + walker_index) % 4;
        kits.walk[kit][d][fi]
    } else {
        kits.walk[kit][d][0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinals() {
        assert_eq!(dir_from_delta(1.0, 0.0, 1), DIR_E);
        assert_eq!(dir_from_delta(-1.0, 0.0, 0), DIR_W);
        assert_eq!(dir_from_delta(0.0, 1.0, 0), DIR_S);
        assert_eq!(dir_from_delta(0.0, -1.0, 0), DIR_N);
    }

    /// All 8 compass steps, from every previous facing: cardinals are
    /// cardinal, diagonals face their horizontal travel — never the
    /// opposite side (the backwards penguin).
    #[test]
    fn eight_directions_from_any_prev() {
        let d = std::f64::consts::FRAC_1_SQRT_2;
        let cases: [((f64, f64), u8); 8] = [
            ((1.0, 0.0), DIR_E),
            ((d, d), DIR_E),   // SE
            ((0.0, 1.0), DIR_S),
            ((-d, d), DIR_W),  // SW
            ((-1.0, 0.0), DIR_W),
            ((-d, -d), DIR_W), // NW
            ((0.0, -1.0), DIR_N),
            ((d, -d), DIR_E),  // NE
        ];
        for prev in [DIR_E, DIR_S, DIR_W, DIR_N] {
            for ((dx, dy), want) in cases {
                assert_eq!(
                    dir_from_delta(dx, dy, prev),
                    want,
                    "step ({dx:.2},{dy:.2}) prev {prev}"
                );
            }
        }
    }

    #[test]
    fn diagonal_never_faces_away_from_travel() {
        // The reported bug: last walked west, now heading north-east.
        assert_eq!(dir_from_delta(0.7, -0.7, DIR_W), DIR_E);
        assert_eq!(dir_from_delta(-0.7, 0.72, DIR_E), DIR_W);
    }

    #[test]
    fn hysteresis_between_horizontal_and_vertical() {
        // ay/ax = 1.25 sits between the gates: a vertical walker stays
        // vertical (≥ 1.15), a horizontal one stays horizontal (< 1.4).
        assert_eq!(dir_from_delta(0.4, 0.5, DIR_S), DIR_S);
        assert_eq!(dir_from_delta(0.4, 0.5, DIR_E), DIR_E);
        // Clearly vertical turns a horizontal walker.
        assert_eq!(dir_from_delta(0.3, 0.6, DIR_E), DIR_S);
        assert_eq!(dir_from_delta(-0.3, -0.6, DIR_W), DIR_N);
    }

    #[test]
    fn hold_when_still() {
        assert_eq!(dir_from_delta(0.0, 0.0, DIR_N), DIR_N);
    }
}
