//! Board / sign geometry + hit-tests (DIET-HABITAT).
//!
//! Extracted from habitat_mac/draw.rs.

use super::{radius_y, NSPoint, NSRect, Projection, STATION_BOARD, VIEW_PADDING};


// the sign's blank face inside the punched PNG
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
pub(crate) fn sign_blit_rect(radius: f64, projection: Projection, marble_scale: f64) -> NSRect {
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
pub(crate) fn sign_face_rect(radius: f64, projection: Projection, marble_scale: f64) -> NSRect {
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
/// the columns live on the sign's painted face
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

/// Preferential board/sign hit for opening kanban. Column ticks OR the
/// whole sign blit — trees that overlap this zone lose the click.
pub(crate) fn hit_board_open(planet: &super::Planet, local: NSPoint) -> bool {
    if planet.is_marble {
        return false;
    }
    if hit_board_column(planet, local).is_some() {
        return true;
    }
    sign_blit_rect(planet.radius, planet.projection, planet.marble_scale).contains(local)
}

