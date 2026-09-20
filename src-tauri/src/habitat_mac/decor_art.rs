//! Decor pixel templates + growth helpers (DIET-HABITAT).
//!
//! Extracted from habitat_mac/draw.rs.


/// The sign's blit rect in view coords: bottom-pinned at STATION_BOARD,
/// the wood post (SIGN_POST_X) pinned to the station's x — the board
/// hangs right of the post, so the white face sits just east of disc
/// center.

// Right-click decor (simple pixels on the disc surface). L leaf, S stem,
// R cap, W stem/spots, . clear. PLANT is the sprout; TREE takes over at
// half grown (ORBITTY_PLANET_PLACE_GROW: sprout → tree, ~7 days).
pub(crate) const PLANT: [&str; 6] = ["..L..", ".L.L.", "..S..", "..S..", "..S..", ".SSS."];

// T trunk, G canopy.
pub(crate) const TREE: [&str; 9] = [
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

pub(crate) const MUSHROOM: [&str; 7] = [
    "..RRR..",
    ".RRRRR.",
    "RRWRRWR",
    ".RRRRR.",
    "...W...",
    "...W...",
    "..WWW..",
];

// B body, H handle, K speaker cone, W cone glint.
pub(crate) const HOUSE: [&str; 8] = [
    "..RRRRR.",
    ".RRRRRRR",
    "RRRRRRRR",
    "WWWWWWWW",
    "WWDDDWWW",
    "WWDDDWWW",
    "WWDDDWWW",
    "WWWWWWWW",
];

pub(crate) const BOOMBOX: [&str; 7] = [
    "..HHHH..",
    ".HHHHHH.",
    "BBBBBBBB",
    "BKKBBKKB",
    "BKWBBWKB",
    "BKKBBKKB",
    "BBBBBBBB",
];

// the beach ball — orange/white panels
// on the same cheap pixel painter as the decor, never a PNG hunt.
// O orange panel, W white panel.
pub(crate) const BALL: [&str; 7] = [
    "..OOO..",
    ".OWWWO.",
    "OOWWWOO",
    "OOWWWOO",
    "OOWWWOO",
    ".OWWWO.",
    "..OOO..",
];

// the town blits — cheap pixel glyphs on the same
// painter as the plants, never sprite packs. I ice, D door. The igloo is
// the home station (spawn / Done's door), NOT the user HOUSE shack.
// WORLD-PROPS: the bundled PNG is the igloo now — this glyph is the
// fallback only when `town/igloo.png` failed to load.
pub(crate) const IGLOO: [&str; 8] = [
    "....IIII....",
    "..IIIIIIII..",
    ".IIIIIIIIII.",
    "IIIIIIIIIIII",
    "IIIIIDDDIIII",
    "IIIIDDDDDIII",
    "IIIIDDDDDIII",
    "IIIIDDDDDIII",
];

// the paddock reads as GRASS — NES-ish
// blade tufts (same painter as the plant glyphs) scattered across the
// four-edge yard AABB (offsets from STATION_FIELD), over a faint darker
// dirt underlay. g dark blade, m mid, G light.
pub(crate) const TUFT: [&str; 4] = ["g...G", "g.m.G", "ggmGG", "gmGGg"];
pub(crate) const TUFT_SMALL: [&str; 3] = ["G.g", "mGg", "gmG"];
pub(crate) const TUFT_GREENS: [(u8, (f64, f64, f64)); 3] = [
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
pub(crate) const GRASS_TUFTS: [(f64, f64); 16] = [
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
pub(crate) const GROW_SECS: f64 = 7.0 * 86400.0;

pub(crate) fn growth_t(planted_at: f64, now: f64) -> f64 {
    ((now - planted_at) / GROW_SECS).clamp(0.0, 1.0)
}

/// Plant look at growth t: sprout art until half grown, then a tree
/// (scale + extra pixels). Returns the art and its pixel scale.
pub(crate) fn plant_metrics(t: f64) -> (&'static [&'static str], f64) {
    let scale = 2.0 * (0.6 + 0.8 * t);
    if t < 0.5 {
        (&PLANT[..], scale)
    } else {
        (&TREE[..], scale)
    }
}

/// Mushroom cluster: 1 cap at planting → up to 5 at full growth. Offsets
/// are view px at scale 1.5 per mini shroom.
pub(crate) const SHROOM_OFFSETS: [(f64, f64); 5] = [
    (0.0, 0.0),
    (-9.0, 4.0),
    (9.0, 5.0),
    (-5.0, -6.0),
    (6.0, -5.0),
];
pub(crate) const SHROOM_SCALE: f64 = 1.5;

pub(crate) fn shroom_count(t: f64) -> usize {
    1 + (t * 4.0).round() as usize
}

pub(crate) const BOOMBOX_SCALE: f64 = 2.0;

