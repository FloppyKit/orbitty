//! Bitty-crew walkers. Glyph grid is last-resort fallback only.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct CharLook {
    /// Legacy: 0 person · 1–8 cats from the sheet. Inert once `kind` is set.
    pub species: u32,
    pub body: u32,
    pub clothes: u32,
    pub eyes: u32,
    pub mouth: u32,
    pub hat: u32,
    pub colors: u32,
    /// Season 1 bitty kind (index into KINDS). Absent in pre-S1 saves —
    /// the legacy species map applies until the Being editor touches it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<u32>,
    /// Palette index into the kind's 4 sheets (0 = default v1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<u32>,
}

impl Default for CharLook {
    fn default() -> Self {
        look_from_seed(0)
    }
}

/// The Season 1 crew (11 kinds — the cap; no new kinds without a new lock).
/// `(display name, pack slug base, palette tokens)` — token "" is the
/// default `v1` sheet; the rest are the blessed recolor suffixes, in the
/// handoff table's order.
pub const KINDS: [(&str, &str, [&str; 4]); 11] = [
    ("Boy", "orbitty-bitty-boy", ["", "green", "red", "tan"]),
    ("Girl", "orbitty-bitty-girl", ["", "blue", "brown", "green"]),
    ("Man", "orbitty-bitty-man", ["", "brown", "green", "red"]),
    ("Woman", "orbitty-bitty-woman", ["", "blue", "green", "red"]),
    ("Old Man", "orbitty-bitty-old-man", ["", "blue", "brown", "burgundy"]),
    ("Old Woman", "orbitty-bitty-old-woman", ["", "blue", "brown", "green"]),
    ("Cat", "orbitty-bitty-cat", ["", "black", "calico", "gray"]),
    ("Dog", "orbitty-bitty-dog", ["", "black", "spot", "white"]),
    ("Cow", "orbitty-bitty-cow", ["", "black", "brown", "white"]),
    ("Horse", "orbitty-bitty-horse", ["", "black", "palomino", "white"]),
    ("Chicken", "orbitty-bitty-chicken", ["", "black", "brown", "white"]),
];

const KIND_CAT: usize = 6;

/// One Kind entry for the Being editor (name + its palette display names).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BittyKind {
    pub name: &'static str,
    /// Pack slug base, e.g. "orbitty-bitty-old-man".
    pub slug: &'static str,
    /// Display names: "Default" then the capitalized recolor tokens.
    pub palettes: Vec<String>,
}

fn capitalize(token: &str) -> String {
    let mut chars = token.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Being-editor catalog: Kind + that kind's named palettes. The only
/// silhouette controls — the old glyph-layer cyclers are gone from the
/// default path.
pub fn bitty_catalog() -> Vec<BittyKind> {
    KINDS
        .iter()
        .map(|(name, slug, tokens)| BittyKind {
            name,
            slug,
            palettes: tokens
                .iter()
                .map(|t| if t.is_empty() { "Default".to_string() } else { capitalize(t) })
                .collect(),
        })
        .collect()
}

/// A look → (kind index, palette index). Explicit bitty picks win; a
/// pre-S1 look (kind absent) maps its legacy species onto the nearest
/// Kind+Palette: person → default Boy; the old cat coats → Cat palettes
/// (orange → default, tuxedo/void → black, calico → calico,
/// gray/cream/siamese/white → gray). Clothes/eyes/mouth/hat are ignored.
pub fn resolve(look: &CharLook) -> (usize, usize) {
    if let Some(k) = look.kind {
        return (
            k as usize % KINDS.len(),
            look.palette.unwrap_or(0) as usize % 4,
        );
    }
    match look.species {
        0 => (0, 0),
        1 => (KIND_CAT, 0),
        2 | 5 => (KIND_CAT, 1),
        3 => (KIND_CAT, 2),
        _ => (KIND_CAT, 3),
    }
}

/// Reroll: a random Kind + Palette from the Season 1 table. Legacy layer
/// fields zero out — they no longer drive any draw once `kind` is set.
pub fn look_from_seed(seed: u64) -> CharLook {
    let s = if seed == 0 { 0xC0FFEE_u64 } else { seed };
    let mix = |n: u64| s.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(n);
    CharLook {
        species: 0,
        body: 0,
        clothes: 0,
        eyes: 0,
        mouth: 0,
        hat: 0,
        colors: 0,
        kind: Some((mix(1) % KINDS.len() as u64) as u32),
        palette: Some((mix(2) % 4) as u32),
    }
}

/// Clamp/materialize a persisted look: legacy-only looks gain concrete
/// kind+palette via the map; explicit picks wrap into range. Never fails.
pub fn normalize(look: &mut CharLook) {
    let (k, p) = resolve(look);
    look.kind = Some(k as u32);
    look.palette = Some(p as u32);
}

/// The pack a claim draws when the user hasn't picked one: the Season 1
/// bitty for this look's Kind+Palette (e.g. `orbitty-bitty-cat-calico`).
pub fn default_pack(look: &CharLook) -> String {
    let (k, p) = resolve(look);
    let (_, slug, tokens) = KINDS[k];
    match tokens[p] {
        "" => slug.to_string(),
        token => format!("{slug}-{token}"),
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub outline: (f64, f64, f64),
    pub hair: (f64, f64, f64),
    pub hair_hi: (f64, f64, f64),
    pub hair_sh: (f64, f64, f64),
    pub skin: (f64, f64, f64),
    pub skin_sh: (f64, f64, f64),
    pub cloth: (f64, f64, f64),
    pub cloth_hi: (f64, f64, f64),
    pub cloth_sh: (f64, f64, f64),
    pub accent: (f64, f64, f64),
    pub boot: (f64, f64, f64),
    pub boot_hi: (f64, f64, f64),
    pub white: (f64, f64, f64),
    pub eye: (f64, f64, f64),
    pub fur: (f64, f64, f64),
    pub fur_hi: (f64, f64, f64),
    pub fur_sh: (f64, f64, f64),
    pub pink: (f64, f64, f64),
}

fn rgb(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0)
}

fn shade(c: (f64, f64, f64), k: f64) -> (f64, f64, f64) {
    (c.0 * k, c.1 * k, c.2 * k)
}

fn lift(c: (f64, f64, f64), k: f64) -> (f64, f64, f64) {
    (
        (c.0 + (1.0 - c.0) * k).min(1.0),
        (c.1 + (1.0 - c.1) * k).min(1.0),
        (c.2 + (1.0 - c.2) * k).min(1.0),
    )
}

pub fn palette(look: &CharLook) -> Palette {
    let cloth = match look.clothes {
        1 => rgb(138, 142, 148),
        2 => rgb(92, 108, 132),
        3 => rgb(120, 86, 148),
        4 => rgb(92, 124, 92),
        _ => rgb(214, 196, 164),
    };
    let hair = match look.hat {
        1 => rgb(92, 62, 42),
        2 => rgb(48, 36, 32),
        3 => rgb(148, 72, 48),
        4 => rgb(176, 176, 180),
        _ => rgb(212, 176, 108),
    };
    let cloth_tint = match look.colors {
        1 => rgb(120, 132, 148),
        2 => rgb(86, 124, 92),
        3 => rgb(132, 92, 140),
        4 => rgb(196, 140, 64),
        5 => rgb(110, 156, 188),
        6 => rgb(220, 204, 168),
        7 => rgb(160, 160, 164),
        _ => cloth,
    };
    let (fur, eye) = match look.species {
        1 => (rgb(212, 124, 56), rgb(176, 196, 48)),
        2 => (rgb(28, 28, 36), rgb(176, 196, 48)),
        3 => (rgb(204, 116, 56), rgb(176, 196, 48)),
        4 => (rgb(216, 196, 164), rgb(80, 160, 88)),
        5 => (rgb(28, 32, 44), rgb(176, 196, 48)),
        6 => (rgb(228, 200, 140), rgb(176, 196, 48)),
        7 => (rgb(132, 140, 156), rgb(176, 196, 48)),
        8 => (rgb(232, 228, 220), rgb(80, 160, 88)),
        _ => (hair, rgb(48, 36, 32)),
    };
    Palette {
        outline: rgb(18, 14, 16),
        hair,
        hair_hi: lift(hair, 0.28),
        hair_sh: shade(hair, 0.62),
        skin: rgb(228, 176, 140),
        skin_sh: rgb(196, 132, 104),
        cloth: cloth_tint,
        cloth_hi: lift(cloth_tint, 0.22),
        cloth_sh: shade(cloth_tint, 0.68),
        accent: rgb(212, 176, 56),
        boot: rgb(72, 44, 32),
        boot_hi: rgb(108, 68, 44),
        white: rgb(244, 236, 220),
        eye,
        fur,
        fur_hi: lift(fur, 0.22),
        fur_sh: shade(fur, 0.58),
        pink: rgb(228, 148, 148),
    }
}

pub fn glyph_color(g: char, pal: &Palette) -> Option<(f64, f64, f64)> {
    Some(match g {
        'O' => pal.outline,
        'H' => pal.hair,
        'h' => pal.hair_hi,
        'd' => pal.hair_sh,
        'S' => pal.skin,
        's' => pal.skin_sh,
        'C' => pal.cloth,
        'c' => pal.cloth_hi,
        'k' => pal.cloth_sh,
        'A' => pal.accent,
        'B' => pal.boot,
        'b' => pal.boot_hi,
        'W' => pal.white,
        'E' => pal.eye,
        'F' => pal.fur,
        'f' => pal.fur_hi,
        'u' => pal.fur_sh,
        'P' => pal.pink,
        _ => return None,
    })
}

pub const W: usize = 24;
pub const H: usize = 32;
pub type Grid = Vec<Vec<char>>;

fn blank() -> Grid {
    vec![vec!['.'; W]; H]
}

fn put(g: &mut Grid, r: isize, c: isize, ch: char) {
    if r < 0 || c < 0 {
        return;
    }
    let (r, c) = (r as usize, c as usize);
    if r < H && c < W {
        g[r][c] = ch;
    }
}

fn paint(g: &mut Grid, lines: &[&str], or: isize, oc: isize) {
    for (i, line) in lines.iter().enumerate() {
        for (j, ch) in line.chars().enumerate() {
            if ch != '.' {
                put(g, or + i as isize, oc + j as isize, ch);
            }
        }
    }
}

/// guy1 youth — face in front, hair is a cap with spikes, not a helmet.
fn person(look: &CharLook, frame: u8) -> Grid {
    let mut g = blank();
    let head = [
        ".....OOhhhO.............",
        "....OHhhhhhHOO..........",
        "...OHhhhhhhhhHO.........",
        "...OhHHhhhhhhhO.........",
        "..OHhHHHHhhhhhO.........",
        "..OHhHHSSSSSHhO.........",
        "..OHSSWWWWSSHO..........",
        "..OHSSWEWWSSHO..........",
        "..OHSSSSSSSSSO..........",
        "...OSSSsSSSO............",
        "...OSSSSSSSO............",
        "....OSSssSSO............",
        ".....OOOOOO.............",
    ];
    paint(&mut g, &head, 0, 0);

    let torso = match look.clothes {
        1 | 2 => [
            "...OOcCCCCCCO...........",
            "...OcCCCCCCCCO..........",
            "...OcCCCAcCCCO..........",
            "...OcCCCCCCCCO..........",
            "....OkkkkkkkO...........",
            "....OCCCCCCCO...........",
            ".....OCCCCCO............",
        ],
        _ => [
            "...OOcCCCCCCO...........",
            "...OcCCCCCCCCO..........",
            "...OcCCCAcCCCO..........",
            "...OcCCCCCCCCO..........",
            "....OkkkkkkkO...........",
            "....OCCCCCCCO...........",
            ".....OCCCCCO............",
        ],
    };
    paint(&mut g, &torso, 13, 0);
    paint(&mut g, &[".............OS.........", ".............Ss........."], 15, 0);

    let legs = match frame % 4 {
        1 => [
            ".....OBBb..BBb..........",
            ".....OBBB..BBB..........",
            ".....OBBB..BBB..........",
            ".....OOOO..OOO..........",
        ],
        2 => [
            "......OBBbBBbO..........",
            "......OBBBBBBO..........",
            "......OBBB.BBO..........",
            "......OOOO.OOO..........",
        ],
        3 => [
            "....OBBb...BBb..........",
            "....OBBB...BBB..........",
            "....OBBB...BBB..........",
            "....OOOO...OOO..........",
        ],
        _ => [
            ".....OBBb.BBbO..........",
            ".....OBBB.BBBO..........",
            ".....OBBB.BBBO..........",
            ".....OOOO.OOOO..........",
        ],
    };
    paint(&mut g, &legs, 20, 0);
    let _ = look;
    g
}

/// One side-view cat (cats.jpg). Coat is palette-only — no mutant remixes.
fn cat(look: &CharLook, frame: u8) -> Grid {
    let mut g = blank();
    let body = [
        "f.................PP....",
        "fF...............PFP....",
        "FF..............uFFu....",
        "uFFFFFFFFFFFFFWWEWWF....",
        "FFFFFFFFFFFFFFWWWWWF....",
        ".uFFFFWWWWWWWFFFFFu.....",
        "..FFFFFFFFFFFFFFF.......",
        "..uFFFFFFFFFFFFFu.......",
        "...Fu...Fu....Fu........",
        "...WW...WW....WW........",
    ];
    let y = 20;
    paint(&mut g, &body, y, 0);
    // trot: swap mid paws
    if frame % 2 == 1 {
        put(&mut g, y + 8, 6, '.');
        put(&mut g, y + 9, 6, '.');
        put(&mut g, y + 8, 7, 'F');
        put(&mut g, y + 9, 7, 'W');
    }
    let _ = look;
    g
}

pub fn grid(look: &CharLook, frame: u8) -> Grid {
    if look.species == 0 {
        person(look, frame)
    } else {
        cat(look, frame)
    }
}
