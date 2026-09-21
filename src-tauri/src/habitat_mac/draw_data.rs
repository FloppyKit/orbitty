//! DrawData / WalkerDraw / DecorDraw + shared blit primitives (DIET-HABITAT).
#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};

use super::super::{
    boombox_hud, DecorKind, NSPoint, NSRect, Projection,
};

// the punched igloo (brand/town/igloo.png,
// flipped so the door faces east). IGLOO_DOOR is the door-base point as
// a fraction of the sprite; the doorstep pins to STATION_HOUSE (spawn /
// home is the door, not the centroid). PROPS leftover: fixed width
// IGLOO_W, ~10% under the PROPS on-screen blit — the old 1.6×-walker
// grow + MAX_W cap shoved it off the west rim; on-disc wins over
// "igloo ≥ 1.6× walker", and the station moved east to fit.
pub(crate) const IGLOO_IMG_W: f64 = 768.0;
pub(crate) const IGLOO_IMG_H: f64 = 435.0;
pub(crate) const IGLOO_DOOR: (f64, f64) = (0.78, 0.92);
/// Drawn blit width in disc units (oval radius = 1).
/// PROPS leftover #2: 25% smaller (0.57 → 0.4275) with the WEST EDGE
/// HELD at ≈ -0.9446 — the door (and STATION_HOUSE with it) moved west
/// to ≈ -0.611; the left rim never drifted.
pub(crate) const IGLOO_W: f64 = 0.4275;
/// Sundial / comms blit width in disc units (square sprites, bottom-pinned).
/// Sundial +20% from first sit; comms +1/3.
pub(crate) const SUNDIAL_W: f64 = 0.186;
pub(crate) const COMSTACK_W: f64 = 0.180;

pub(crate) fn shaded(color: (f64, f64, f64), factor: f64) -> (f64, f64, f64) {
    (
        (color.0 * factor).min(1.0),
        (color.1 * factor).min(1.0),
        (color.2 * factor).min(1.0),
    )
}

/// Draw an NSImage into a flipped world view without inverting it.
pub(crate) unsafe fn draw_nsimage(img: *mut Object, dest: NSRect, fraction: f64) {
    let zero = NSRect::default();
    let _: () = msg_send![img,
        drawInRect: dest
        fromRect: zero
        operation: 2usize
        fraction: fraction
        respectFlipped: true
        hints: std::ptr::null::<Object>()];
}

pub(crate) struct WalkerDraw {
    pub(crate) frame: NSRect,
    pub(crate) bob: f64,
    pub(crate) icon: usize,
    /// Generated-person layer picks (drawn when pack_frame is 0).
    pub(crate) look: crate::chars::CharLook,
    /// Per-walker size multiplier from the claim.
    pub(crate) user_scale: f64,
    /// 4-step walk cycle (0 = idle / contact).
    pub(crate) walk_frame: u8,
    /// Pack-frame NSImage (0 = generated person).
    pub(crate) pack_frame: usize,
    pub(crate) dim: f64,
    /// Facing left (pack frames ship facing right).
    pub(crate) flip: bool,
    /// 2.5D front-arc emphasis; 1.0 in flat projection.
    pub(crate) scale: f64,
    /// MENU locate: stroke a ring around this walker.
    pub(crate) locate: bool,
    /// 0 working, 1 blocked, 2 needs-you.
    pub(crate) droid_glance: u8,
}

pub(crate) struct DecorDraw {
    pub(crate) kind: DecorKind,
    /// Laid-out bounds (same rect hit-tests use).
    pub(crate) frame: NSRect,
    /// Growth 0..1 (boombox always 1).
    pub(crate) t: f64,
    /// Boombox playing (drawn dim when off).
    pub(crate) on: bool,
}

pub(crate) struct DrawData {
    pub(crate) radius: f64,
    pub(crate) sprite_scale: f64,
    pub(crate) is_marble: bool,
    /// Drawn scale multiplier — always 1.0 since GO-ORBITTY-MARBLE-SOLAR
    /// (fake Z is z-order only, no scale-throb).
    pub(crate) marble_scale: f64,
    pub(crate) color: (f64, f64, f64),
    pub(crate) rotation: f64,
    pub(crate) projection: Projection,
    pub(crate) walkers: Vec<WalkerDraw>,
    pub(crate) dots: Vec<NSRect>,
    /// Fresh caption text (expired ones are dropped here, cleared on tick).
    pub(crate) caption: Option<String>,
    /// Display name (GO-ORBITTY-PLANET-MARK): under-label on expanded
    /// planets, hover pill on marbles.
    pub(crate) name: String,
    /// Cursor over a marble's disc — pill shows the name when no click
    /// caption is live.
    pub(crate) hover_name: bool,
    pub(crate) decor: Vec<DecorDraw>,
    /// board ticks per column (todo/doing/done),
    /// colors pre-hashed from card ids. Empty on marbles (no town).
    pub(crate) board: [Vec<(f64, f64, f64)>; 3],
    /// retained town NSImages (0 = missing →
    /// glyph fallback). Loaded once per session by ensure_town_art.
    pub(crate) town_igloo: usize,
    pub(crate) town_sign: usize,
    pub(crate) town_boombox: usize,
    pub(crate) town_ball: usize,
    pub(crate) town_sundial: usize,
    pub(crate) town_comstack: usize,
    /// Dest-menu find: 0 just armed, 1 expired. None = no glow.
    pub(crate) locate_t: Option<f64>,
    /// the beach ball's view-local center
    /// this frame. None = hidden (toggle off / < 2 Doing / marble).
    pub(crate) ball: Option<(f64, f64)>,
    /// the open boombox HUD —
    /// geometry + live slider/mute state. None = closed.
    pub(crate) hud: Option<boombox_hud::HudDraw>,
    pub(crate) todo_trees: Vec<crate::habitat::TodoTreeTick>,
    pub(crate) tree_grow_days: f64,
}
