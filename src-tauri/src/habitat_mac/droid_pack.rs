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
    roots.push(PathBuf::from(
        "/Users/dude/goose artifacts/livesprite-tauri/brand/droid-crew/packs",
    ));
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
/// Tiny steps and near-diagonals keep `prev` so the body doesn't flicker.
pub(crate) fn dir_from_delta(dx: f64, dy: f64, prev: u8) -> u8 {
    if dx * dx + dy * dy < 0.002 * 0.002 {
        return prev;
    }
    let ax = dx.abs();
    let ay = dy.abs();
    if ax >= ay * 1.15 {
        if dx < 0.0 {
            DIR_W
        } else {
            DIR_E
        }
    } else if ay >= ax * 1.15 {
        if dy > 0.0 {
            DIR_S
        } else {
            DIR_N
        }
    } else {
        prev
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

    #[test]
    fn hold_when_still() {
        assert_eq!(dir_from_delta(0.0, 0.0, DIR_N), DIR_N);
    }
}
