//! GO-ORBITTY-DOING-WALKER — the agent walker's body: a seeded 2D
//! android baked ONCE per run (the seed hashes `planetId:cardId`) into
//! three retained NSImages (walk A / walk B / sit), then blitted like a
//! pack PNG. Per-frame procgen is cope; the bitty-crew sheets stay for
//! claimed-WINDOW walkers — this 4-rect-class dummy is the legal agent
//! art until a real droid pack lands.
//!
//! All AppKit objects are created on the main thread inside drawRect
//! (ensure_baked runs from draw_rect_guarded's pre-pass) and cached in
//! `Habitat::droids` for the session, same law as the icon cache.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

use super::{ns_string, CGSize, Habitat, Pose};

/// 12×14 pixel droid. `.` clear; `a` antenna, `h` head, `e` eyes,
/// `b` body, `l` legs. Row 0 is the TOP (bitmap rows paint top-first,
/// same as the decoded pack PNGs).
const W: usize = 12;
const H: usize = 14;

/// Walk frame A — legs under the body (contact pose; also the still).
const WALK_A: [&str; H] = [
    "............",
    ".....a......",
    ".....a......",
    "..hhhhhh....",
    "..hehheh....",
    "..hhhhhh....",
    "...bbbb.....",
    "..bbbbbb....",
    "..bbbbbb....",
    "...bbbb.....",
    "...l..l.....",
    "...l..l.....",
    "...l..l.....",
    "............",
];

/// Walk frame B — legs split wide (the alternation reads as walking).
const WALK_B: [&str; H] = [
    "............",
    ".....a......",
    ".....a......",
    "..hhhhhh....",
    "..hehheh....",
    "..hhhhhh....",
    "...bbbb.....",
    "..bbbbbb....",
    "..bbbbbb....",
    "...bbbb.....",
    "..l....l....",
    "..l....l....",
    ".l......l...",
    "............",
];

/// Sit (near rim, Needed/blocked): the body drops a pixel and the legs
/// tuck into a base row — seated, never a sleeper-dim.
const SIT: [&str; H] = [
    "............",
    "............",
    ".....a......",
    ".....a......",
    "..hhhhhh....",
    "..hehheh....",
    "..hhhhhh....",
    "...bbbb.....",
    "..bbbbbb....",
    "..bbbbbb....",
    "...bbbb.....",
    "..llllll....",
    "............",
    "............",
];

/// The three baked frames of one run's droid (retained NSImages as
/// usize, session-cached — same ownership law as `icon_cache`).
pub(crate) struct DroidAtlas {
    pub walk: [usize; 2],
    pub sit: usize,
}

/// Seeded body hue — six droid colors, picked by the run's seed so two
/// runs on one planet read as different agents.
fn body_color(seed: u64) -> (f64, f64, f64) {
    const HUES: [(f64, f64, f64); 6] = [
        (0.35, 0.55, 0.85), // steel blue
        (0.85, 0.55, 0.30), // rust
        (0.40, 0.70, 0.45), // olive
        (0.60, 0.45, 0.75), // plum
        (0.30, 0.70, 0.70), // teal
        (0.80, 0.45, 0.55), // rose
    ];
    HUES[(seed % 6) as usize]
}

fn shade(c: (f64, f64, f64), f: f64) -> (f64, f64, f64) {
    (c.0 * f, c.1 * f, c.2 * f)
}

fn glyph_color(ch: u8, body: (f64, f64, f64)) -> Option<(f64, f64, f64)> {
    match ch {
        b'a' => Some((0.95, 0.85, 0.35)), // antenna — fixed warm pip
        b'h' => Some(shade(body, 1.25)),  // head — light
        b'e' => Some((0.10, 0.12, 0.16)), // eyes — near-black
        b'b' => Some(body),               // body
        b'l' => Some(shade(body, 0.55)),  // legs — dark
        _ => None,
    }
}

/// Bake one frame into a retained NSImage: a hand-filled RGBA bitmap
/// (row 0 = top, alpha only ever 0/255 so premultiplication is a
/// non-question), wrapped at pixel size — the walker's blit path scales
/// it nearest-neighbor like any pack frame.
unsafe fn bake_frame(grid: &[&str; H], body: (f64, f64, f64)) -> *mut Object {
    let rep: *mut Object = msg_send![class!(NSBitmapImageRep), alloc];
    let rep: *mut Object = msg_send![rep,
        initWithBitmapDataPlanes: std::ptr::null_mut::<*mut u8>()
        pixelsWide: W as isize
        pixelsHigh: H as isize
        bitsPerSample: 8isize
        samplesPerPixel: 4isize
        hasAlpha: true
        isPlanar: false
        colorSpaceName: ns_string("NSCalibratedRGBColorSpace")
        bytesPerRow: (W * 4) as isize
        bitsPerPixel: 32isize];
    if rep.is_null() {
        return std::ptr::null_mut();
    }
    let data: *mut u8 = msg_send![rep, bitmapData];
    if data.is_null() {
        let _: () = msg_send![rep, release];
        return std::ptr::null_mut();
    }
    std::ptr::write_bytes(data, 0, W * H * 4);
    for (row, line) in grid.iter().enumerate() {
        for (col, ch) in line.bytes().enumerate() {
            let Some(c) = glyph_color(ch, body) else {
                continue;
            };
            let o = (row * W + col) * 4;
            *data.add(o) = (c.0 * 255.0).round() as u8;
            *data.add(o + 1) = (c.1 * 255.0).round() as u8;
            *data.add(o + 2) = (c.2 * 255.0).round() as u8;
            *data.add(o + 3) = 255;
        }
    }
    let img: *mut Object = msg_send![class!(NSImage), alloc];
    let img: *mut Object =
        msg_send![img, initWithSize: CGSize { width: W as f64, height: H as f64 }];
    if img.is_null() {
        let _: () = msg_send![rep, release];
        return std::ptr::null_mut();
    }
    let _: () = msg_send![img, addRepresentation: rep];
    let _: () = msg_send![rep, release]; // the image retains it
    img
}

/// Bake-once door: the atlas for `seed` exists or gets built here.
/// Main thread only (drawRect's pre-pass). A failed bake leaves the
/// seed ABSENT so frame_ptr falls back to the generated person instead
/// of caching a half-atlas.
pub(crate) fn ensure_baked(h: &mut Habitat, seed: u64) {
    if h.droids.contains_key(&seed) {
        return;
    }
    let body = body_color(seed);
    let (walk_a, walk_b, sit) = unsafe {
        (
            bake_frame(&WALK_A, body),
            bake_frame(&WALK_B, body),
            bake_frame(&SIT, body),
        )
    };
    if walk_a.is_null() || walk_b.is_null() || sit.is_null() {
        unsafe {
            for img in [walk_a, walk_b, sit] {
                if !img.is_null() {
                    let _: () = msg_send![img as *mut Object, release];
                }
            }
        }
        return;
    }
    h.droids.insert(
        seed,
        DroidAtlas {
            walk: [walk_a as usize, walk_b as usize],
            sit: sit as usize,
        },
    );
}

/// The frame to blit this tick: Sit rides the seated frame; a cycling
/// walker alternates the two walk frames (desynced per walker, same
/// 10Hz half-rate as the pack law); anything else is the contact pose.
/// 0 = no atlas (bake failed) — the caller falls through to the
/// generated-person branch.
pub(crate) fn frame_ptr(
    h: &Habitat,
    seed: u64,
    pose: Pose,
    cycling: bool,
    anim_frame: u64,
    walker_index: usize,
) -> usize {
    let Some(atlas) = h.droids.get(&seed) else {
        return 0;
    };
    if pose == Pose::Sit {
        return atlas.sit;
    }
    if cycling {
        return atlas.walk[((anim_frame / 2) as usize + walker_index) % 2];
    }
    atlas.walk[0]
}
