//! World-view drawRect / blit.
//! Class registration stays on the host; selector body lives here.
//! Planet / Being stay crate-private on the host.

#![allow(unexpected_cfgs)]

use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

use super::{
    ball_pos, boombox_hud, card_color, droid, ensure_town_art, fill_oval, lock, now_epoch,
    ns_color, ns_string, pack_still_aspect, panic_msg, radius_y, set_fill, station_anchor,
    walker_fit, walker_hidden, CGSize, DecorKind, Habitat, NSPoint, NSRect, Pose, Projection,
    CAPTION_TTL, LOCATE_TTL, PLANET_LOCATE_TTL, STATION_BOARD, STATION_FIELD, STATION_HOUSE,
    VIEW_PADDING, YARD_EAST, YARD_NORTH, YARD_SOUTH, YARD_WEST,
};

// GO-ORBITTY-WORLD-PROPS: the sign's blank face inside the punched PNG
// (brand/town/sign.png), measured as fractions of the cropped sprite.
// The blit is bottom-pinned at STATION_BOARD and scaled so the face is
// SIGN_FACE_W disc units wide; the three tick columns live on the face.
const SIGN_IMG_W: f64 = 1037.0;
const SIGN_IMG_H: f64 = 1277.0;
const SIGN_FACE_X0: f64 = 0.1456;
const SIGN_FACE_X1: f64 = 0.8534;
const SIGN_FACE_Y0: f64 = 0.2161;
const SIGN_FACE_Y1: f64 = 0.6727;
/// Face width in disc units — the blit scale is driven by it.
/// PROPS leftover: ~half the PROPS face (0.33 → 0.16); the smaller blit
/// still sits on the disc, so STATION_BOARD did not move.
/// Leftover #2: another 10% off (0.16 → 0.144).
const SIGN_FACE_W: f64 = 0.144;
/// The long vertical wood post under the board, measured in the punched
/// PNG: the pole runs x ∈ [102, 231] px → center fraction ≈ 0.1606 (the
/// board hangs RIGHT of it). Leftover #2: the blit pins THIS x to
/// STATION_BOARD.0 — planet center sits on the post, not the face
/// midpoint.
const SIGN_POST_X: f64 = 0.1606;

// GO-ORBITTY-WORLD-PROPS: the punched igloo (brand/town/igloo.png,
// flipped so the door faces east). IGLOO_DOOR is the door-base point as
// a fraction of the sprite; the doorstep pins to STATION_HOUSE (spawn /
// home is the door, not the centroid). PROPS leftover: fixed width
// IGLOO_W, ~10% under the PROPS on-screen blit — the old 1.6×-walker
// grow + MAX_W cap shoved it off the west rim; on-disc wins over
// "igloo ≥ 1.6× walker", and the station moved east to fit.
const IGLOO_IMG_W: f64 = 768.0;
const IGLOO_IMG_H: f64 = 435.0;
const IGLOO_DOOR: (f64, f64) = (0.78, 0.92);
/// Drawn blit width in disc units (oval radius = 1).
/// PROPS leftover #2: 25% smaller (0.57 → 0.4275) with the WEST EDGE
/// HELD at ≈ -0.9446 — the door (and STATION_HOUSE with it) moved west
/// to ≈ -0.611; the left rim never drifted.
const IGLOO_W: f64 = 0.4275;

/// The sign's blit rect in view coords: bottom-pinned at STATION_BOARD,
/// the wood post (SIGN_POST_X) pinned to the station's x — the board
/// hangs right of the post, so the white face sits just east of disc
/// center.
fn sign_blit_rect(radius: f64, projection: Projection, marble_scale: f64) -> NSRect {
    let side = (radius + VIEW_PADDING) * 2.0;
    let center = NSPoint {
        x: side / 2.0,
        y: side / 2.0,
    };
    let r = radius * marble_scale;
    let ry = radius_y(r, projection);
    let face_w = SIGN_FACE_W * r;
    let blit_w = face_w / (SIGN_FACE_X1 - SIGN_FACE_X0);
    let blit_h = blit_w * SIGN_IMG_H / SIGN_IMG_W;
    let base_y = center.y + STATION_BOARD.1 * ry;
    NSRect::new(
        center.x + STATION_BOARD.0 * r - SIGN_POST_X * blit_w,
        base_y - blit_h,
        blit_w,
        blit_h,
    )
}

/// The sign's blank face in view coords — the tick area.
fn sign_face_rect(radius: f64, projection: Projection, marble_scale: f64) -> NSRect {
    let blit = sign_blit_rect(radius, projection, marble_scale);
    NSRect::new(
        blit.origin.x + SIGN_FACE_X0 * blit.size.width,
        blit.origin.y + SIGN_FACE_Y0 * blit.size.height,
        (SIGN_FACE_X1 - SIGN_FACE_X0) * blit.size.width,
        (SIGN_FACE_Y1 - SIGN_FACE_Y0) * blit.size.height,
    )
}

/// Planet board columns (todo / doing / done), view-local, same math as
/// the blit. `None` if marble. Done is column 2.
///
/// GO-ORBITTY-WORLD-PROPS: the columns live on the sign's painted face
/// (recomputed from the face rect, not the old 9×30 glyph).
pub(crate) fn board_column_rects(
    radius: f64,
    projection: Projection,
    marble_scale: f64,
) -> [NSRect; 3] {
    let face = sign_face_rect(radius, projection, marble_scale);
    let inset_x = face.size.width * 0.06;
    let gap = face.size.width * 0.05;
    let col_w = (face.size.width - inset_x * 2.0 - gap * 2.0) / 3.0;
    let inset_y = face.size.height * 0.08;
    let col_h = face.size.height - inset_y * 2.0;
    [
        NSRect::new(face.origin.x + inset_x, face.origin.y + inset_y, col_w, col_h),
        NSRect::new(
            face.origin.x + inset_x + col_w + gap,
            face.origin.y + inset_y,
            col_w,
            col_h,
        ),
        NSRect::new(
            face.origin.x + inset_x + 2.0 * (col_w + gap),
            face.origin.y + inset_y,
            col_w,
            col_h,
        ),
    ]
}

/// Disc-local south / near lip of the three tick columns on the sign
/// face — the edge a Done walker walks up to. Same rects as the
/// Done-column hit-test.
///
/// Flipped view: larger y is front/south. `STATION_BOARD` is the back
/// anchor the blit is bottom-pinned to (the face extends north of it);
/// the lip is the south edge of the face columns, not that station point.
pub(crate) fn board_near_lip(
    radius: f64,
    projection: Projection,
    marble_scale: f64,
) -> (f64, f64) {
    let cols = board_column_rects(radius, projection, marble_scale);
    let side = (radius + VIEW_PADDING) * 2.0;
    let center = NSPoint {
        x: side / 2.0,
        y: side / 2.0,
    };
    let r = (radius * marble_scale).max(1e-6);
    let ry = radius_y(r, projection).max(1e-6);
    let vx = (cols[0].origin.x + cols[2].max_x()) / 2.0;
    // South edge of the rails (front in disc space).
    let vy = cols[0].max_y();
    ((vx - center.x) / r, (vy - center.y) / ry)
}

pub(crate) fn hit_board_column(planet: &super::Planet, local: NSPoint) -> Option<usize> {
    if planet.is_marble {
        return None;
    }
    board_column_rects(planet.radius, planet.projection, planet.marble_scale)
        .iter()
        .position(|r| r.contains(local))
}

// Right-click decor (simple pixels on the disc surface). L leaf, S stem,
// R cap, W stem/spots, . clear. PLANT is the sprout; TREE takes over at
// half grown (ORBITTY_PLANET_PLACE_GROW: sprout → tree, ~7 days).
const PLANT: [&str; 6] = ["..L..", ".L.L.", "..S..", "..S..", "..S..", ".SSS."];

// T trunk, G canopy.
const TREE: [&str; 9] = [
    "...GGG...",
    "..GGGGG..",
    ".GGGGGGG.",
    "..GGGGG..",
    "...GGG...",
    "....T....",
    "....T....",
    "....T....",
    "..TTTTT..",
];

const MUSHROOM: [&str; 7] = [
    "..RRR..",
    ".RRRRR.",
    "RRWRRWR",
    ".RRRRR.",
    "...W...",
    "...W...",
    "..WWW..",
];

// B body, H handle, K speaker cone, W cone glint.
const HOUSE: [&str; 8] = [
    "..RRRRR.",
    ".RRRRRRR",
    "RRRRRRRR",
    "WWWWWWWW",
    "WWDDDWWW",
    "WWDDDWWW",
    "WWDDDWWW",
    "WWWWWWWW",
];

const BOOMBOX: [&str; 7] = [
    "..HHHH..",
    ".HHHHHH.",
    "BBBBBBBB",
    "BKKBBKKB",
    "BKWBBWKB",
    "BKKBBKKB",
    "BBBBBBBB",
];

// GO-ORBITTY-WORLD-YARD leftover: the beach ball — orange/white panels
// on the same cheap pixel painter as the decor, never a PNG hunt.
// O orange panel, W white panel.
const BALL: [&str; 7] = [
    "..OOO..",
    ".OWWWO.",
    "OOWWWOO",
    "OOWWWOO",
    "OOWWWOO",
    ".OWWWO.",
    "..OOO..",
];

// GO-ORBITTY-WORLD-TOWN: the town blits — cheap pixel glyphs on the same
// painter as the plants, never sprite packs. I ice, D door. The igloo is
// the home station (spawn / Done's door), NOT the user HOUSE shack.
// WORLD-PROPS: the bundled PNG is the igloo now — this glyph is the
// fallback only when `town/igloo.png` failed to load.
const IGLOO: [&str; 8] = [
    "....IIII....",
    "..IIIIIIII..",
    ".IIIIIIIIII.",
    "IIIIIIIIIIII",
    "IIIIIDDDIIII",
    "IIIIDDDDDIII",
    "IIIIDDDDDIII",
    "IIIIDDDDDIII",
];

// GO-ORBITTY-WORLD-PROPS leftover: the paddock reads as GRASS — NES-ish
// blade tufts (same painter as the plant glyphs) scattered across the
// four-edge yard AABB (offsets from STATION_FIELD), over a faint darker
// dirt underlay. g dark blade, m mid, G light.
const TUFT: [&str; 4] = ["g...G", "g.m.G", "ggmGG", "gmGGg"];
const TUFT_SMALL: [&str; 3] = ["G.g", "mGg", "gmG"];
const TUFT_GREENS: [(u8, (f64, f64, f64)); 3] = [
    (b'g', (0.22, 0.48, 0.24)),
    (b'm', (0.31, 0.59, 0.30)),
    (b'G', (0.40, 0.70, 0.36)),
];

/// Tuft scatter — disc-local offsets from STATION_FIELD inside the
/// paddock AABB (inset ~0.04 so the art stays on the green). Even
/// indices draw TUFT, odd draw TUFT_SMALL. touch+pad-inset-y leftover:
/// the same 16 blades rescaled bbox→bbox on Y ONLY into the shortened
/// box (y bbox [-0.53, 0.40] → [-0.45, 0.42]) — northern tufts sit on
/// the new north edge (-0.45), southern tufts on the new south edge
/// (0.42); x untouched — east tufts still stop at the shack's LEFT
/// wall (inset east 0.50), west stays — the box is never bald.
const GRASS_TUFTS: [(f64, f64); 16] = [
    (-0.307, -0.387),
    (-0.061, -0.450),
    (0.202, -0.367),
    (0.447, -0.429),
    (-0.219, -0.160),
    (0.079, -0.222),
    (0.360, -0.119),
    (0.500, -0.284),
    (-0.324, 0.068),
    (-0.096, 0.026),
    (0.167, 0.089),
    (0.413, 0.130),
    (-0.201, 0.317),
    (0.115, 0.420),
    (0.343, 0.379),
    (0.009, 0.234),
];

/// Decor growth window: planted → fully grown.
const GROW_SECS: f64 = 7.0 * 86400.0;

fn growth_t(planted_at: f64, now: f64) -> f64 {
    ((now - planted_at) / GROW_SECS).clamp(0.0, 1.0)
}

/// Plant look at growth t: sprout art until half grown, then a tree
/// (scale + extra pixels). Returns the art and its pixel scale.
fn plant_metrics(t: f64) -> (&'static [&'static str], f64) {
    let scale = 2.0 * (0.6 + 0.8 * t);
    if t < 0.5 {
        (&PLANT[..], scale)
    } else {
        (&TREE[..], scale)
    }
}

/// Mushroom cluster: 1 cap at planting → up to 5 at full growth. Offsets
/// are view px at scale 1.5 per mini shroom.
const SHROOM_OFFSETS: [(f64, f64); 5] = [
    (0.0, 0.0),
    (-9.0, 4.0),
    (9.0, 5.0),
    (-5.0, -6.0),
    (6.0, -5.0),
];
const SHROOM_SCALE: f64 = 1.5;

fn shroom_count(t: f64) -> usize {
    1 + (t * 4.0).round() as usize
}

const BOOMBOX_SCALE: f64 = 2.0;

fn shaded(color: (f64, f64, f64), factor: f64) -> (f64, f64, f64) {
    (
        (color.0 * factor).min(1.0),
        (color.1 * factor).min(1.0),
        (color.2 * factor).min(1.0),
    )
}

/// Draw an NSImage into a flipped world view without inverting it.
unsafe fn draw_nsimage(img: *mut Object, dest: NSRect, fraction: f64) {
    let zero = NSRect::default();
    let _: () = msg_send![img,
        drawInRect: dest
        fromRect: zero
        operation: 2usize
        fraction: fraction
        respectFlipped: true
        hints: std::ptr::null::<Object>()];
}

/// Sleeping walker (GO-ORBITTY-SLEEPERS): a truly static frame — idle[0]
/// when the pack ships an idle cycle, else walk[0]. No cycling at all.
fn pack_frame_still(h: &Habitat, pack: Option<usize>) -> usize {
    let Some((_, pf)) = pack.and_then(|p| h.pack_frames.get(p)) else {
        return 0;
    };
    pf.idle.first().or_else(|| pf.walk.first()).copied().unwrap_or(0)
}

/// Current frame for a being: moving walkers cycle the walk art (up to 8
/// frames at 10Hz, desynced per walker); paused walkers freeze on the
/// still frame (idle[0] else walk[0]) — no idle cycling while standing.
fn pack_frame_ptr(h: &Habitat, pack: Option<usize>, walker_index: usize, moving: bool) -> usize {
    let Some((_, pf)) = pack.and_then(|p| h.pack_frames.get(p)) else {
        return 0;
    };
    if moving {
        let n = pf.walk.len();
        if n == 0 {
            return 0;
        }
        let cycle = if n >= 2 { n.min(8) } else { 1 };
        pf.walk[((h.anim_frame / 2) as usize + walker_index) % cycle]
    } else {
        pack_frame_still(h, pack)
    }
}

/// Copy of everything drawRect needs, so the lock isn't held across drawing.
struct WalkerDraw {
    frame: NSRect,
    bob: f64,
    icon: usize,
    /// Generated-person layer picks (drawn when pack_frame is 0).
    look: crate::chars::CharLook,
    /// Per-walker size multiplier from the claim.
    user_scale: f64,
    /// 4-step walk cycle (0 = idle / contact).
    walk_frame: u8,
    /// Pack-frame NSImage (0 = generated person).
    pack_frame: usize,
    dim: f64,
    /// Facing left (pack frames ship facing right).
    flip: bool,
    /// 2.5D front-arc emphasis; 1.0 in flat projection.
    scale: f64,
    /// GO-ORBITTY-RAIL-ROW-MENU locate: stroke a ring around this walker.
    locate: bool,
}

struct DecorDraw {
    kind: DecorKind,
    /// Laid-out bounds (same rect hit-tests use).
    frame: NSRect,
    /// Growth 0..1 (boombox always 1).
    t: f64,
    /// Boombox playing (drawn dim when off).
    on: bool,
}

struct DrawData {
    radius: f64,
    sprite_scale: f64,
    is_marble: bool,
    /// Drawn scale multiplier — always 1.0 since GO-ORBITTY-MARBLE-SOLAR
    /// (fake Z is z-order only, no scale-throb).
    marble_scale: f64,
    color: (f64, f64, f64),
    rotation: f64,
    projection: Projection,
    walkers: Vec<WalkerDraw>,
    dots: Vec<NSRect>,
    /// Fresh caption text (expired ones are dropped here, cleared on tick).
    caption: Option<String>,
    /// Display name (GO-ORBITTY-PLANET-MARK): under-label on expanded
    /// planets, hover pill on marbles.
    name: String,
    /// Cursor over a marble's disc — pill shows the name when no click
    /// caption is live.
    hover_name: bool,
    decor: Vec<DecorDraw>,
    /// GO-ORBITTY-WORLD-TOWN: board ticks per column (todo/doing/done),
    /// colors pre-hashed from card ids. Empty on marbles (no town).
    board: [Vec<(f64, f64, f64)>; 3],
    /// GO-ORBITTY-WORLD-PROPS: retained town NSImages (0 = missing →
    /// glyph fallback). Loaded once per session by ensure_town_art.
    town_igloo: usize,
    town_sign: usize,
    town_boombox: usize,
    town_ball: usize,
    /// Dest-menu find: 0 just armed, 1 expired. None = no glow.
    locate_t: Option<f64>,
    /// GO-ORBITTY-WORLD-YARD leftover: the beach ball's view-local center
    /// this frame. None = hidden (toggle off / < 2 Doing / marble).
    ball: Option<(f64, f64)>,
    /// GO-ORBITTY-BOOMBOX leftover (click): the open boombox HUD —
    /// geometry + live slider/mute state. None = closed.
    hud: Option<boombox_hud::HudDraw>,
    todo_trees: Vec<crate::habitat::TodoTreeTick>,
    tree_grow_days: f64,
}

pub(crate) extern "C" fn draw_rect(this: &Object, _cmd: Sel, _dirty: NSRect) {
    // Called by AppKit inside the display cycle: a panic here cannot unwind
    // through the extern boundary, so catch it instead of aborting the app.
    let _ = catch_unwind(AssertUnwindSafe(|| draw_rect_guarded(this))).map_err(|p| {
        eprintln!("habitat: drawRect panicked: {}", panic_msg(&*p));
    });
}

fn draw_rect_guarded(this: &Object) {
    let view_ptr = this as *const Object as usize;
    let data = {
        let mut h = lock();
        ensure_town_art(&mut h);
        // GO-ORBITTY-DOING-WALKER: bake this view's droid atlases BEFORE
        // the immutable draw pass — once per seed, then frame_ptr only
        // reads. Never a per-frame procgen.
        let droid_seeds = h
            .planets
            .iter()
            .find(|(_, p)| p.view == view_ptr)
            .map(|(_, p)| p.walkers.iter().filter_map(|b| b.droid).collect::<Vec<u64>>())
            .unwrap_or_default();
        for seed in droid_seeds {
            droid::ensure_baked(&mut h, seed);
        }
        let town_igloo = h.town_igloo;
        let town_sign = h.town_sign;
        let town_boombox = h.town_boombox;
        let town_ball = h.town_ball;
        let now = now_epoch();
        // WORLD-YARD leftover (ball in view space): per-pack still-frame
        // aspects for the keep-up head math (0.0 = generated person).
        let pack_aspects: Vec<f64> = h
            .pack_frames
            .iter()
            .map(|(_, pf)| pack_still_aspect(pf))
            .collect();
        h.planets.iter().find(|(_, p)| p.view == view_ptr).map(|(pid, p)| {
            let walkers = p
                .walkers
                .iter()
                .enumerate()
                .filter_map(|(i, b)| {
                    let roam = p.roam.get(&b.entry_id);
                    // GO-ORBITTY-CADENCE: a Done walker past the hide line is
                    // gone from the world (off the left rim, WORLD-TOWN)
                    // until the rail's eviction lands — never drawn
                    // mid-padding.
                    if walker_hidden(p, b) {
                        return None;
                    }
                    let walking = roam
                        .map(|r| {
                            r.pause_end.is_none()
                                && ((r.tx - r.x).powi(2) + (r.ty - r.y).powi(2)).sqrt() >= 0.025
                        })
                        .unwrap_or(false);
                    // Pose-driven frame law: sit freezes on idle frame 0;
                    // stand/roam/walkoff cycle only while stepping; work
                    // KEEPS the walk cycle after arrival — the field is
                    // open interior now (WORLD-PLACES), not the front rim
                    // that made CADENCE-PATH still it.
                    let cycling = match b.pose {
                        Pose::Work => true,
                        Pose::Stand | Pose::Roam | Pose::WalkOff => walking,
                        Pose::Sit => false,
                    };
                    let y = roam.map(|r| r.y).unwrap_or(0.0);
                    let locate = h
                        .locate
                        .as_ref()
                        .is_some_and(|(id, at)| id == &b.entry_id && at.elapsed() < LOCATE_TTL);
                    // Sleepers freeze on idle frame 0 — no walk cycle, no
                    // idle cycling, no bob (bob is zeroed in anim_tick). A
                    // Done sleeper walks off instead: the walk-off narrates
                    // the eviction, so the still-sleeper law yields to it.
                    let sleeper_still = b.asleep && b.pose != Pose::WalkOff;
                    Some(WalkerDraw {
                        frame: b.frame,
                        bob: b.bob,
                        icon: b.icon,
                        look: b.look,
                        user_scale: b.user_scale,
                        locate,
                        walk_frame: if cycling && !sleeper_still {
                            ((h.anim_frame / 2) % 4) as u8
                        } else {
                            0
                        },
                        // GO-ORBITTY-DOING-WALKER: a droid blits its baked
                        // atlas (Sit = seated frame; cycling = the two
                        // walk frames) — window walkers keep the bitty
                        // sheets / generated person below.
                        pack_frame: if let Some(seed) = b.droid {
                            droid::frame_ptr(&h, seed, b.pose, cycling && !sleeper_still, h.anim_frame, i)
                        } else if sleeper_still {
                            pack_frame_still(&h, b.pack)
                        } else {
                            pack_frame_ptr(&h, b.pack, i, cycling)
                        },
                        dim: b.dim,
                        flip: match b.pose {
                            // Sit at the front rim (WORLD-PLACES): the
                            // anchor is due south (x = 0), so this never
                            // mirrors — right-facing art stands along the
                            // rim. (The old east gate flipped to face the
                            // planet center.)
                            Pose::Sit => station_anchor(Pose::Sit)
                                .map(|(ax, _)| ax > 0.0)
                                .unwrap_or(false),
                            _ => roam.map(|r| r.facing_left).unwrap_or(false),
                        },
                        scale: match p.projection {
                            Projection::Oblique => 1.0 + 0.25 * y,
                            Projection::Flat => 1.0,
                        },
                    })
                })
                .collect::<Vec<_>>();
            let mut decor = Vec::new();
            if !p.is_marble {
                for (dref, frame) in &p.decor_frames {
                    let (t, on) = match dref.kind {
                        DecorKind::Plant => (
                            p.plants
                                .get(dref.index)
                                .map(|d| growth_t(d.planted_at, now))
                                .unwrap_or(1.0),
                            true,
                        ),
                        DecorKind::Mushroom => (
                            p.mushrooms
                                .get(dref.index)
                                .map(|d| growth_t(d.planted_at, now))
                                .unwrap_or(1.0),
                            true,
                        ),
                        DecorKind::Building => (
                            p.buildings
                                .get(dref.index)
                                .map(|d| growth_t(d.planted_at, now))
                                .unwrap_or(1.0),
                            true,
                        ),
                        DecorKind::Boombox => (1.0, p.boombox),
                    };
                    decor.push(DecorDraw {
                        kind: dref.kind,
                        frame: *frame,
                        t,
                        on,
                    });
                }
            }
            // WORLD-TOWN: board ticks hashed to colors per column —
            // marbles stay bare (no town blits).
            let mut board: [Vec<(f64, f64, f64)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
            if !p.is_marble {
                for c in &p.board_cards {
                    // WORLD-TOUCH: a Done tick stays in doing until that
                    // walker has touched the board (via_board). The rail
                    // already wrote status=done; the blit waits.
                    let mut status = c.status.as_str();
                    if status == "done" {
                        if let Some(wid) = c.window_id.as_deref() {
                            if p.walkers.iter().any(|b| {
                                b.entry_id == wid && b.pose == Pose::WalkOff
                            }) && p
                                .roam
                                .get(wid)
                                .is_some_and(|r| !r.via_board)
                            {
                                status = "doing";
                            }
                        }
                    }
                    let col = match status {
                        "doing" => 1,
                        "done" => 2,
                        _ => 0,
                    };
                    board[col].push(card_color(&c.id));
                }
            }
            // GO-ORBITTY-WORLD-YARD leftover (ball in view space): the
            // keep-up ball's view-pixel point this frame. ball_pos returns
            // it ready to blit — heads come from the blit's own math
            // (walker_head_view), so there is NO second projection here.
            // Gates: toggle off / < 2 Doing / marble → None → hidden.
            let ball = if p.is_marble {
                None
            } else {
                ball_pos(&pack_aspects, p, Instant::now())
            };
            // BOOMBOX click leftover: the open HUD rides the draw —
            // geometry from the live decor frame, slider/mute from the
            // world slice.
            let hud = if h.boombox_hud.as_deref() == Some(pid.as_str()) {
                boombox_hud::rects(p).map(|rects| {
                    let (value, muted) = h
                        .worlds
                        .get(pid)
                        .map(|w| (w.boombox_volume, w.boombox_muted))
                        .unwrap_or((1.0, false));
                    boombox_hud::HudDraw { rects, value, muted }
                })
            } else {
                None
            };
            DrawData {
                radius: p.radius,
                sprite_scale: p.sprite_scale,
                is_marble: p.is_marble,
                marble_scale: p.marble_scale,
                color: p.color,
                rotation: p.rotation,
                projection: p.projection,
                walkers,
                dots: p.dots.iter().map(|b| b.frame).collect::<Vec<_>>(),
                ball,
                caption: p
                    .caption
                    .as_ref()
                    .filter(|(_, at)| at.elapsed() < CAPTION_TTL)
                    .map(|(text, _)| text.clone()),
                name: p.name.clone(),
                hover_name: p.hover_name,
                decor,
                board,
                town_igloo,
                town_sign,
                town_boombox,
                town_ball,
                locate_t: h.locate_planet.as_ref().and_then(|(id, at)| {
                    if id == pid && at.elapsed() < PLANET_LOCATE_TTL {
                        Some(
                            (at.elapsed().as_secs_f64()
                                / PLANET_LOCATE_TTL.as_secs_f64())
                            .min(1.0),
                        )
                    } else {
                        None
                    }
                }),
                hud,
                todo_trees: p.todo_trees.clone(),
                tree_grow_days: p.tree_grow_days,
            }
        })
    };
    let Some(data) = data else {
        return;
    };
    unsafe { draw_world(this, &data) }
}

unsafe fn draw_world(this: &Object, d: &DrawData) {
    let bounds: NSRect = msg_send![this, bounds];
    let center = NSPoint {
        x: bounds.mid_x(),
        y: bounds.mid_y(),
    };
    // marble_scale is 1.0 everywhere (fake Z is z-order only); the
    // multiply stays so the disc math has one path.
    let r = d.radius * d.marble_scale;
    // 2.5D: the disc is an ellipse (ry ≈ rx*0.55); flat keeps ry == r.
    let ry = radius_y(r, d.projection);
    let rect = NSRect::new(center.x - r, center.y - ry, r * 2.0, ry * 2.0);
    let base = d.color;
    let dark = shaded(base, 0.6);
    let light = shaded(base, 1.3);

    // Soft shadow under the disc.
    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
    let shadow: *mut Object = msg_send![class!(NSShadow), alloc];
    let shadow: *mut Object = msg_send![shadow, init];
    let _: () = msg_send![shadow, setShadowOffset: CGSize { width: 0.0, height: 3.0 }];
    let _: () = msg_send![shadow, setShadowBlurRadius: if d.is_marble { 4.0 } else { 8.0 }];
    let black: *mut Object = msg_send![class!(NSColor), blackColor];
    let shadow_color: *mut Object = msg_send![black, colorWithAlphaComponent: 0.35f64];
    let _: () = msg_send![shadow, setShadowColor: shadow_color];
    let _: () = msg_send![shadow, set];
    set_fill(ns_color(base.0, base.1, base.2, 1.0));
    fill_oval(rect);
    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];

    // Everything inside the disc.
    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
    let clip: *mut Object = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect];
    let _: () = msg_send![clip, addClip];

    if !d.is_marble {
        // Fake spin: features slide across the disc and narrow at the limbs.
        let band_y: [f64; 3] = [-0.34, 0.04, 0.38];
        for i in 0..3 {
            let phase = d.rotation + (i as f64) * (std::f64::consts::TAU / 3.0);
            let fx = center.x + phase.sin() * r * 0.55;
            let fw = r * 0.46 * phase.cos().abs().max(0.18);
            let fh = ry * 0.22;
            let fy = center.y + band_y[i] * ry;
            let c = if i % 2 == 0 { light } else { dark };
            set_fill(ns_color(c.0, c.1, c.2, 0.30));
            fill_oval(NSRect::new(fx - fw / 2.0, fy - fh / 2.0, fw, fh));
        }
    }

    // Terminator shading: clear top-left → dark bottom-right.
    let clear: *mut Object = msg_send![class!(NSColor), clearColor];
    let shade: *mut Object = msg_send![black, colorWithAlphaComponent: 0.28f64];
    let pair = [clear, shade];
    let colors: *mut Object =
        msg_send![class!(NSArray), arrayWithObjects: pair.as_ptr() count: 2usize];
    let locations: [f64; 2] = [0.4, 1.0];
    let space: *mut Object = msg_send![class!(NSColorSpace), deviceRGBColorSpace];
    let grad: *mut Object = msg_send![class!(NSGradient), alloc];
    let grad: *mut Object = msg_send![grad,
        initWithColors: colors
        atLocations: locations.as_ptr()
        colorSpace: space];
    if !grad.is_null() {
        let start = NSPoint {
            x: rect.origin.x + r * 0.3,
            y: rect.origin.y + ry * 0.3,
        };
        let end = NSPoint {
            x: rect.max_x() - r * 0.15,
            y: rect.max_y() - ry * 0.15,
        };
        let _: () = msg_send![grad, drawFromPoint: start toPoint: end options: 0usize];
    }

    // Top-left highlight.
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let hl: *mut Object =
        msg_send![white, colorWithAlphaComponent: if d.is_marble { 0.25 } else { 0.16 }];
    set_fill(hl);
    fill_oval(NSRect::new(
        center.x - r * 0.62,
        center.y - ry * 0.66,
        r * 0.6,
        ry * 0.34,
    ));

    // GO-ORBITTY-WORLD-PROPS: the paddock is a grass patch, never fence
    // posts. Leftover: no more filled-green blotch — a faint darker dirt
    // underlay in the same four-edge AABB (west/south edges PINNED;
    // east pulled back to the shack's LEFT wall, north to the tuft
    // tops), then blade-tuft pixel art scattered across it so the read
    // is grass. West side open toward the igloo, south open toward the
    // Needed rim. Painted under the decor so plants sit on it.
    if !d.is_marble {
        let gx = center.x + YARD_WEST * r;
        let gy = center.y + YARD_NORTH * ry;
        let gw = (YARD_EAST - YARD_WEST) * r;
        let gh = (YARD_SOUTH - YARD_NORTH) * ry;
        let corner = 0.10 * r;
        let dirt: *mut Object = msg_send![class!(NSBezierPath),
            bezierPathWithRoundedRect: NSRect::new(gx, gy, gw, gh)
            xRadius: corner
            yRadius: corner];
        set_fill(ns_color(0.24, 0.36, 0.20, 0.45));
        let _: () = msg_send![dirt, fill];
        let tuft_scale = (0.012 * r).clamp(1.5, 4.0);
        for (i, &(tx, ty)) in GRASS_TUFTS.iter().enumerate() {
            let art = if i % 2 == 0 { &TUFT[..] } else { &TUFT_SMALL[..] };
            draw_pixel_art(
                center.x + (STATION_FIELD.0 + tx) * r,
                center.y + (STATION_FIELD.1 + ty) * ry,
                art,
                tuft_scale,
                &TUFT_GREENS[..],
                0.95,
            );
        }
    }

    // Placed decor on the surface (expanded planets only): plants grow
    // sprout → tree, mushrooms 1 cap → cluster, boombox sits and plays.
    for decor in &d.decor {
        let cx = decor.frame.mid_x();
        let base_y = decor.frame.max_y();
        match decor.kind {
            DecorKind::Plant => {
                let (art, scale) = plant_metrics(decor.t);
                draw_pixel_art(
                    cx,
                    base_y,
                    art,
                    scale,
                    &[
                        (b'L', (0.30, 0.62, 0.32)),
                        (b'S', (0.16, 0.45, 0.22)),
                        (b'G', (0.30, 0.62, 0.32)),
                        (b'T', (0.35, 0.22, 0.12)),
                    ],
                    1.0,
                );
            }
            DecorKind::Mushroom => {
                // The laid-out frame bounds the whole cluster; each mini
                // shroom is bottom-anchored at its offset from the base.
                let n = shroom_count(decor.t);
                let mini = 7.0 * SHROOM_SCALE;
                for (dx, dy) in SHROOM_OFFSETS.iter().take(n) {
                    draw_pixel_art(
                        cx + dx,
                        base_y + dy,
                        &MUSHROOM[..],
                        SHROOM_SCALE,
                        &[(b'R', (0.75, 0.22, 0.22)), (b'W', (0.95, 0.93, 0.88))],
                        1.0,
                    );
                }
                let _ = mini;
            }
            DecorKind::Building => {
                let scale = 1.6 + decor.t * 1.4;
                draw_pixel_art(
                    cx,
                    base_y,
                    &HOUSE[..],
                    scale,
                    &[
                        (b'R', (0.62, 0.22, 0.18)),
                        (b'W', (0.86, 0.78, 0.62)),
                        (b'D', (0.28, 0.16, 0.10)),
                    ],
                    1.0,
                );
            }
            DecorKind::Boombox => {
                let alpha = if decor.on { 1.0 } else { 0.55 };
                if d.town_boombox != 0 {
                    draw_nsimage(
                        d.town_boombox as *mut Object,
                        decor.frame,
                        alpha,
                    );
                } else {
                    draw_pixel_art(
                        cx,
                        base_y,
                        &BOOMBOX[..],
                        BOOMBOX_SCALE,
                        &[
                            (b'H', (0.55, 0.55, 0.60)),
                            (b'B', (0.18, 0.18, 0.22)),
                            (b'K', (0.05, 0.05, 0.07)),
                            (b'W', (0.85, 0.85, 0.90)),
                        ],
                        alpha,
                    );
                }
            }
        }
    }
    if !d.is_marble {
        super::todo_tree::paint_ticks(
            &d.todo_trees,
            d.radius,
            d.projection,
            d.tree_grow_days,
            now_epoch(),
        );
    }

    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];

    // Rim stroke.
    let stroke_color = ns_color(dark.0, dark.1, dark.2, 0.8);
    let _: () = msg_send![stroke_color, setStroke];
    let ring: *mut Object =
        msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect.inset(0.75)];
    let _: () = msg_send![ring, setLineWidth: if d.is_marble { 1.0 } else { 1.5 }];
    let _: () = msg_send![ring, stroke];

    // GO-ORBITTY-WORLD-PROPS: the town at real scale — bundled PNGs
    // drawn UNCLIPPED like the walkers (buildings stand on the disc
    // stage: the sign rises past the back rim; the igloo sits fully
    // on-disc since the leftover). Marbles skip town blits. The cheap
    // glyphs remain as fallbacks only when a PNG failed to load.
    if !d.is_marble {
        // Sign (billboard) at the back station, bottom-pinned, no
        // titles — three tick columns on the blank face, same
        // todo/doing/done tints as the old rails.
        let blit = sign_blit_rect(d.radius, d.projection, d.marble_scale);
        if d.town_sign != 0 {
            draw_nsimage(d.town_sign as *mut Object, blit, 1.0);
        }
        let cols = board_column_rects(d.radius, d.projection, d.marble_scale);
        let rail_tints = [
            (1.0, 1.0, 1.0, 0.16),    // todo — faint white
            (1.0, 0.75, 0.35, 0.22),  // doing — faint amber
            (0.45, 0.85, 0.50, 0.20), // done — faint green
        ];
        let col_w = cols[0].size.width;
        let col_h = cols[0].size.height;
        let tick = (col_w * 0.55).clamp(2.0, 6.0);
        let tick_step = tick * 1.4;
        let max_ticks =
            (((col_h - tick * 0.7) / tick_step).floor() as usize).max(1);
        for (ci, ticks) in d.board.iter().enumerate() {
            let col = cols[ci];
            let (tr, tg, tb, ta) = rail_tints[ci];
            set_fill(ns_color(tr, tg, tb, ta));
            let rail: *mut Object = msg_send![class!(NSBezierPath),
                bezierPathWithRoundedRect: col
                xRadius: 2.0f64
                yRadius: 2.0f64];
            let _: () = msg_send![rail, fill];
            for (ti, &(cr, cg, cb)) in ticks.iter().take(max_ticks).enumerate() {
                set_fill(ns_color(cr, cg, cb, 0.95));
                let t: *mut Object = msg_send![class!(NSBezierPath),
                    bezierPathWithRoundedRect: NSRect::new(
                        col.origin.x + (col_w - tick) / 2.0,
                        col.origin.y + tick * 0.35 + ti as f64 * tick_step,
                        tick,
                        tick,
                    )
                    xRadius: 1.0f64
                    yRadius: 1.0f64];
                let _: () = msg_send![t, fill];
            }
        }
        // Igloo at the house station — the door the walkers come out of.
        // Fixed width (IGLOO_W), doorstep pinned to the station; the
        // station sits far enough east that the whole blit is on-disc.
        if d.town_igloo != 0 {
            let blit_w = IGLOO_W * r;
            let blit_h = blit_w * IGLOO_IMG_H / IGLOO_IMG_W;
            let door_x = center.x + STATION_HOUSE.0 * r;
            let door_y = center.y + STATION_HOUSE.1 * ry;
            draw_nsimage(
                d.town_igloo as *mut Object,
                NSRect::new(
                    door_x - IGLOO_DOOR.0 * blit_w,
                    door_y - IGLOO_DOOR.1 * blit_h,
                    blit_w,
                    blit_h,
                ),
                1.0,
            );
        } else {
            let town = (r / 72.0).clamp(0.7, 2.0);
            let ix = center.x + STATION_HOUSE.0 * r;
            let iy = center.y + STATION_HOUSE.1 * ry;
            draw_pixel_art(
                ix,
                iy + 4.0 * town,
                &IGLOO[..],
                2.8 * town,
                &[(b'I', (0.88, 0.93, 0.97)), (b'D', (0.20, 0.24, 0.30))],
                1.0,
            );
        }
    }

    // Occupancy dots: white ring + project-color core.
    for frame in &d.dots {
        let white85: *mut Object = msg_send![white, colorWithAlphaComponent: 0.85f64];
        set_fill(white85);
        fill_oval(frame.inset(0.5));
        set_fill(ns_color(base.0, base.1, base.2, 1.0));
        fill_oval(frame.inset(3.0));
    }

    // Walkers: user-pack frame when one is loaded (GO-LS-TAURI-USER-PACKS),
    // else the generated Orbitty person (GO-ORBITTY-CHARS); both carry the
    // app-icon badge. `dim` fades zombies (7+ days untouched) without
    // touching the walk physics. Pack frames draw fitted into a readable
    // box, aspect-correct and nearest-neighbor — never a squashed strip —
    // and flip when their roam takes them leftward. The person is painted
    // per frame from its layer grid at the walker's own scale (gear slider
    // × per-claim scale), bottom-anchored so it stands on the disc.
    for w in &d.walkers {
        let frame = &w.frame;
        let mut sprite_rect = *frame;
        if w.pack_frame != 0 {
            let img = w.pack_frame as *mut Object;
            let size: CGSize = msg_send![img, size];
            let aspect = if size.width > 0.0 && size.height > 0.0 {
                size.width / size.height
            } else {
                1.0
            };
            let fit = walker_fit(d.radius, d.sprite_scale * w.user_scale) * w.scale;
            let (dw, dh) = if aspect >= 1.0 {
                (fit, fit / aspect)
            } else {
                (fit * aspect, fit)
            };
            let cx = frame.mid_x();
            // Bottom-aligned in the hit box (+ walk bob).
            let base_y = frame.max_y() - 2.0 + w.bob;
            let art = NSRect::new(cx - dw / 2.0, base_y - dh, dw, dh);
            sprite_rect = art;
            let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
            let ctx: *mut Object = msg_send![class!(NSGraphicsContext), currentContext];
            if !ctx.is_null() {
                let _: () = msg_send![ctx, setImageInterpolation: 1usize];
            }
            if w.flip {
                let tf: *mut Object = msg_send![class!(NSAffineTransform), transform];
                let _: () = msg_send![tf, translateXBy: cx yBy: 0.0];
                let _: () = msg_send![tf, scaleXBy: -1.0 yBy: 1.0];
                let _: () = msg_send![tf, translateXBy: -cx yBy: 0.0];
                let _: () = msg_send![tf, concat];
            }
            draw_nsimage(img, art, w.dim);
            let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
        } else {
            let fit = walker_fit(d.radius, d.sprite_scale * w.user_scale) * w.scale;
            let px = (fit / crate::chars::H as f64).clamp(1.5, 8.0);
            let grid = crate::chars::grid(&w.look, w.walk_frame);
            let pal = crate::chars::palette(&w.look);
            let cx = frame.mid_x();
            let base_y = frame.max_y() - 2.0 + w.bob;
            let ox = cx - (crate::chars::W as f64 * px) / 2.0;
            let oy = base_y - crate::chars::H as f64 * px;
            sprite_rect = NSRect::new(ox, oy, crate::chars::W as f64 * px, crate::chars::H as f64 * px);
            for (row, line) in grid.iter().enumerate() {
                for (col, ch) in line.iter().enumerate() {
                    let Some(c) = crate::chars::glyph_color(*ch, &pal) else {
                        continue;
                    };
                    let draw_col = if w.flip { crate::chars::W - 1 - col } else { col };
                    set_fill(ns_color(c.0, c.1, c.2, w.dim));
                    let cell = NSRect::new(
                        ox + draw_col as f64 * px,
                        oy + row as f64 * px,
                        px,
                        px,
                    );
                    let _: () = msg_send![class!(NSBezierPath), fillRect: cell];
                }
            }
        }
        if w.icon != 0 {
            // Sit on the drawn sprite's lower-right, overlapping the hem.
            let bs = 14.0;
            let badge = NSRect::new(
                sprite_rect.max_x() - bs + 2.0,
                sprite_rect.max_y() - bs + 2.0,
                bs,
                bs,
            );
            let white85: *mut Object =
                msg_send![white, colorWithAlphaComponent: 0.85f64 * w.dim];
            set_fill(white85);
            fill_oval(badge.inset(-1.0));
            draw_nsimage(w.icon as *mut Object, badge, w.dim);
        }
        if w.locate {
            // Locate stroke (GO-ORBITTY-RAIL-ROW-MENU): a ~500ms ring around
            // the walker's hit frame — glow only, never a focus/raise.
            let ring_color: *mut Object =
                msg_send![white, colorWithAlphaComponent: 0.85f64];
            let _: () = msg_send![ring_color, setStroke];
            let ring: *mut Object = msg_send![class!(NSBezierPath),
                bezierPathWithRoundedRect: frame.inset(-4.0)
                xRadius: 6.0f64
                yRadius: 6.0f64];
            let _: () = msg_send![ring, setLineWidth: 2.5f64];
            let _: () = msg_send![ring, stroke];
        }
    }

    // GO-ORBITTY-WORLD-YARD leftover: the beach ball, drawn with the
    // walkers above the grass. Bundled PNG when present; cheap orange/
    // white panels as fallback. Marbles never carry one (ball_pos gates).
    // Head-kiss leftover: bottom-anchored ON the kiss point (sprite top
    // + a flat 6 px down in walker_head_view) — ≈1–2 px of hair under
    // the ball's bottom, a keep-up touch, never the ball's center
    // hovering on the sprite-box top (transparent pack padding read as
    // air) and never the crown buried.
    if let Some((bx, by)) = d.ball {
        if d.town_ball != 0 {
            let s = (0.028 * d.radius).clamp(16.0, 20.0);
            draw_nsimage(
                d.town_ball as *mut Object,
                NSRect::new(bx - s / 2.0, by - s, s, s),
                1.0,
            );
        } else {
            let scale = (0.022 * d.radius).clamp(1.5, 3.0);
            draw_pixel_art(
                bx,
                by,
                &BALL[..],
                scale,
                &[(b'O', (0.95, 0.52, 0.14)), (b'W', (0.96, 0.95, 0.90))],
                1.0,
            );
        }
    }

    // Click caption: a small pill above the disc, drawn last so it sits over
    // any walker crossing the top of the orbit. GO-ORBITTY-PLANET-MARK: a
    // hovered marble falls back to its name here — a live click caption
    // (placed / walker / boombox) always wins, and mouse-leave just drops
    // the fallback, so those pills are never cleared early.
    let pill = d.caption.as_ref().or(if d.hover_name {
        Some(&d.name)
    } else {
        None
    });
    if let Some(text) = pill {
        draw_caption(center, ry, text);
    }

    // GO-ORBITTY-PLANET-MARK: an expanded planet wears its name under the
    // oval — always on, no hover hunt. Marbles stay bare (hover pill).
    if !d.is_marble {
        draw_under_label(center, ry, &d.name);
    }

    draw_planet_locate(d, center, r, ry, rect);

    // GO-ORBITTY-BOOMBOX leftover (click): the boombox HUD floats over
    // everything — caption, under-label, locate glow, walkers.
    if let Some(hud) = &d.hud {
        boombox_hud::draw(hud);
    }
}

/// Dest-menu find glow. Marble: saturn ring stays in a UL→LR plane,
/// crosses the front of the disc, hides behind it, and a highlight
/// travels around the ring (in-plane spin — not tumbling). Expanded:
/// rim glow + name ~15% smaller than the last pass.
unsafe fn draw_planet_locate(
    d: &DrawData,
    center: NSPoint,
    r: f64,
    ry: f64,
    rect: NSRect,
) {
    let Some(t) = d.locate_t else { return };
    let fade = (1.0 - t).clamp(0.0, 1.0);
    if fade <= 0.02 {
        return;
    }
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let base = d.color;
    if d.is_marble {
        let tilt = -42.0f64;
        let rw = r * 2.70;
        let rh = ry * 0.90;
        let ring_rect = NSRect::new(center.x - rw / 2.0, center.y - rh / 2.0, rw, rh);
        let pad = r * 6.0;
        let spin = t * 360.0;
        // Ears: the ring outside the disc (front and back tips).
        let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
        clip_outside_oval(rect, pad);
        concat_tilt(center, tilt);
        stroke_saturn_ellipse(ring_rect, fade, base);
        stroke_saturn_highlight(center, ring_rect, spin, fade);
        let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
        // Front crossing: inside the disc, only the near long edge
        // (lower half of the unrotated ellipse, then tilted).
        let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
        let disc: *mut Object =
            msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect];
        let _: () = msg_send![disc, addClip];
        concat_tilt(center, tilt);
        let front: *mut Object = msg_send![class!(NSBezierPath),
            bezierPathWithRect: NSRect::new(center.x - pad, center.y, pad * 2.0, pad)];
        let _: () = msg_send![front, addClip];
        stroke_saturn_ellipse(ring_rect, fade, base);
        stroke_saturn_highlight(center, ring_rect, spin, fade);
        let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
    } else {
        let glow: *mut Object = msg_send![white, colorWithAlphaComponent: 0.5f64 * fade];
        let _: () = msg_send![glow, setStroke];
        let outer: *mut Object =
            msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect.inset(-3.5)];
        let _: () = msg_send![outer, setLineWidth: 5.5f64];
        let _: () = msg_send![outer, stroke];
        let ink: *mut Object =
            msg_send![white, colorWithAlphaComponent: 0.95f64 * fade];
        let _: () = msg_send![ink, setStroke];
        let rim: *mut Object =
            msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect.inset(-0.5)];
        let _: () = msg_send![rim, setLineWidth: 2.6f64];
        let _: () = msg_send![rim, stroke];
        let tint = ns_color(base.0, base.1, base.2, 0.7 * fade);
        let _: () = msg_send![tint, setStroke];
        let color_ring: *mut Object =
            msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect];
        let _: () = msg_send![color_ring, setLineWidth: 1.8f64];
        let _: () = msg_send![color_ring, stroke];
        draw_center_flash(center, r, &d.name, fade);
    }
}

unsafe fn concat_tilt(center: NSPoint, degrees: f64) {
    let tf: *mut Object = msg_send![class!(NSAffineTransform), transform];
    let _: () = msg_send![tf, translateXBy: center.x yBy: center.y];
    let _: () = msg_send![tf, rotateByDegrees: degrees];
    let _: () = msg_send![tf, translateXBy: -center.x yBy: -center.y];
    let _: () = msg_send![tf, concat];
}

unsafe fn clip_outside_oval(rect: NSRect, pad: f64) {
    let hole: *mut Object = msg_send![class!(NSBezierPath), bezierPath];
    let c = NSPoint {
        x: rect.mid_x(),
        y: rect.mid_y(),
    };
    let _: () = msg_send![hole,
        appendBezierPathWithRect: NSRect::new(c.x - pad, c.y - pad, pad * 2.0, pad * 2.0)];
    let disc: *mut Object =
        msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect];
    let _: () = msg_send![hole, appendBezierPath: disc];
    let _: () = msg_send![hole, setWindingRule: 1usize];
    let _: () = msg_send![hole, addClip];
}

unsafe fn stroke_saturn_ellipse(
    ring_rect: NSRect,
    fade: f64,
    base: (f64, f64, f64),
) {
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let glow: *mut Object = msg_send![white, colorWithAlphaComponent: 0.32f64 * fade];
    let _: () = msg_send![glow, setStroke];
    let outer: *mut Object =
        msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: ring_rect];
    let _: () = msg_send![outer, setLineWidth: 5.0f64];
    let _: () = msg_send![outer, stroke];
    let ink: *mut Object = msg_send![white, colorWithAlphaComponent: 0.78f64 * fade];
    let _: () = msg_send![ink, setStroke];
    let inner: *mut Object =
        msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: ring_rect];
    let _: () = msg_send![inner, setLineWidth: 2.0f64];
    let _: () = msg_send![inner, stroke];
    let tint = ns_color(base.0, base.1, base.2, 0.5 * fade);
    let _: () = msg_send![tint, setStroke];
    let color_ring: *mut Object =
        msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: ring_rect.inset(1.5)];
    let _: () = msg_send![color_ring, setLineWidth: 1.4f64];
    let _: () = msg_send![color_ring, stroke];
}

/// Bright chunk traveling around the ellipse — the ring spinning in its
/// plane, not the oval tumbling around the planet.
unsafe fn stroke_saturn_highlight(
    center: NSPoint,
    ring_rect: NSRect,
    spin_deg: f64,
    fade: f64,
) {
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
    let pie: *mut Object = msg_send![class!(NSBezierPath), bezierPath];
    let _: () = msg_send![pie, moveToPoint: center];
    let reach = ring_rect.size.width.max(ring_rect.size.height);
    let _: () = msg_send![pie,
        appendBezierPathWithArcWithCenter: center
        radius: reach
        startAngle: spin_deg
        endAngle: spin_deg + 70.0
        clockwise: 0u8];
    let _: () = msg_send![pie, closePath];
    let _: () = msg_send![pie, addClip];
    let ink: *mut Object = msg_send![white, colorWithAlphaComponent: fade];
    let _: () = msg_send![ink, setStroke];
    let hi: *mut Object =
        msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: ring_rect];
    let _: () = msg_send![hi, setLineWidth: 3.2f64];
    let _: () = msg_send![hi, stroke];
    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
}

unsafe fn draw_center_flash(center: NSPoint, r: f64, text: &str, fade: f64) {
    let ns_text = ns_string(text);
    let max_w = (r * 2.0 * 0.78).max(24.0);
    let probe = 48.0;
    let probe_font: *mut Object = msg_send![class!(NSFont), boldSystemFontOfSize: probe];
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let probe_keys = [ns_string("NSFont"), ns_string("NSColor")];
    let probe_vals = [probe_font, white];
    let probe_attrs: *mut Object = msg_send![class!(NSDictionary),
        dictionaryWithObjects: probe_vals.as_ptr()
        forKeys: probe_keys.as_ptr()
        count: 2usize];
    if probe_attrs.is_null() {
        return;
    }
    let probe_size: CGSize = msg_send![ns_text, sizeWithAttributes: probe_attrs];
    let font_size = if probe_size.width > 1.0 {
        (probe * max_w / probe_size.width).clamp(16.0, r * 0.80)
    } else {
        28.0
    };
    let font: *mut Object = msg_send![class!(NSFont), boldSystemFontOfSize: font_size];
    let ink: *mut Object = msg_send![white, colorWithAlphaComponent: fade];
    let shadow: *mut Object = msg_send![class!(NSShadow), alloc];
    let shadow: *mut Object = msg_send![shadow, init];
    let _: () = msg_send![shadow, setShadowOffset: CGSize { width: 0.0, height: -1.0 }];
    let _: () = msg_send![shadow, setShadowBlurRadius: 6.0f64];
    let black: *mut Object = msg_send![class!(NSColor), blackColor];
    let shadow_color: *mut Object =
        msg_send![black, colorWithAlphaComponent: 0.8f64 * fade];
    let _: () = msg_send![shadow, setShadowColor: shadow_color];
    let keys = [ns_string("NSFont"), ns_string("NSColor"), ns_string("NSShadow")];
    let vals = [font, ink, shadow];
    let attrs: *mut Object = msg_send![class!(NSDictionary),
        dictionaryWithObjects: vals.as_ptr()
        forKeys: keys.as_ptr()
        count: 3usize];
    if attrs.is_null() {
        return;
    }
    let size: CGSize = msg_send![ns_text, sizeWithAttributes: attrs];
    let _: () = msg_send![ns_text,
        drawAtPoint: NSPoint {
            x: center.x - size.width / 2.0,
            y: center.y - size.height / 2.0,
        }
        withAttributes: attrs];
}

/// Expanded-planet under-label: the planet name in 11pt white, centered
/// just below the oval, soft shadow instead of the pill backer so it reads
/// on any wallpaper without looking like a second caption.
unsafe fn draw_under_label(center: NSPoint, r: f64, text: &str) {
    let ns_text = ns_string(text);
    let font: *mut Object = msg_send![class!(NSFont), systemFontOfSize: 11.0f64];
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let shadow: *mut Object = msg_send![class!(NSShadow), alloc];
    let shadow: *mut Object = msg_send![shadow, init];
    let _: () = msg_send![shadow, setShadowOffset: CGSize { width: 0.0, height: -1.0 }];
    let _: () = msg_send![shadow, setShadowBlurRadius: 2.0f64];
    let black: *mut Object = msg_send![class!(NSColor), blackColor];
    let shadow_color: *mut Object = msg_send![black, colorWithAlphaComponent: 0.6f64];
    let _: () = msg_send![shadow, setShadowColor: shadow_color];
    let keys = [ns_string("NSFont"), ns_string("NSColor"), ns_string("NSShadow")];
    let vals = [font, white, shadow];
    let attrs: *mut Object = msg_send![class!(NSDictionary),
        dictionaryWithObjects: vals.as_ptr()
        forKeys: keys.as_ptr()
        count: 3usize];
    if attrs.is_null() {
        return;
    }
    let size: CGSize = msg_send![ns_text, sizeWithAttributes: attrs];
    let _: () = msg_send![ns_text,
        drawAtPoint: NSPoint {
            x: center.x - size.width / 2.0,
            y: center.y + r + 4.0,
        }
        withAttributes: attrs];
}

/// Tiny pixel-art painter for decor: `art` rows of glyph chars, each glyph
/// mapped to an rgb color, bottom-anchored at (cx, base_y) — plants grow
/// upward from the point they were planted at.
unsafe fn draw_pixel_art(
    cx: f64,
    base_y: f64,
    art: &[&str],
    scale: f64,
    colors: &[(u8, (f64, f64, f64))],
    alpha: f64,
) {
    let ox = cx - (art.first().map(|r| r.len()).unwrap_or(0) as f64 * scale) / 2.0;
    let oy = base_y - art.len() as f64 * scale;
    for (row, line) in art.iter().enumerate() {
        for (col, ch) in line.bytes().enumerate() {
            let Some(&(_, c)) = colors.iter().find(|(k, _)| *k == ch) else {
                continue;
            };
            set_fill(ns_color(c.0, c.1, c.2, alpha));
            let _: () = msg_send![class!(NSBezierPath),
                fillRect: NSRect::new(
                    ox + col as f64 * scale,
                    oy + row as f64 * scale,
                    scale,
                    scale,
                )];
        }
    }
}

/// Caption pill: dark rounded backer + white 11pt text, centered above the
/// disc (VIEW_PADDING leaves headroom). NSString drawing is flipped-context
/// aware, so the point is the text's top-left in this view's coords.
unsafe fn draw_caption(center: NSPoint, r: f64, text: &str) {
    let ns_text = ns_string(text);
    let font: *mut Object = msg_send![class!(NSFont), systemFontOfSize: 11.0f64];
    let white: *mut Object = msg_send![class!(NSColor), whiteColor];
    let keys = [ns_string("NSFont"), ns_string("NSColor")];
    let vals = [font, white];
    let attrs: *mut Object = msg_send![class!(NSDictionary),
        dictionaryWithObjects: vals.as_ptr()
        forKeys: keys.as_ptr()
        count: 2usize];
    if attrs.is_null() {
        return;
    }
    let size: CGSize = msg_send![ns_text, sizeWithAttributes: attrs];
    let pad_x = 8.0;
    let pad_y = 4.0;
    let pill_w = size.width + pad_x * 2.0;
    let pill_h = size.height + pad_y * 2.0;
    let pill = NSRect::new(
        center.x - pill_w / 2.0,
        (center.y - r - pill_h - 6.0).max(1.0),
        pill_w,
        pill_h,
    );
    let black: *mut Object = msg_send![class!(NSColor), blackColor];
    let bg: *mut Object = msg_send![black, colorWithAlphaComponent: 0.72f64];
    set_fill(bg);
    let path: *mut Object = msg_send![class!(NSBezierPath),
        bezierPathWithRoundedRect: pill
        xRadius: pill_h / 2.0
        yRadius: pill_h / 2.0];
    let _: () = msg_send![path, fill];
    let _: () = msg_send![ns_text,
        drawAtPoint: NSPoint {
            x: pill.origin.x + pad_x,
            y: pill.origin.y + pad_y,
        }
        withAttributes: attrs];
}
