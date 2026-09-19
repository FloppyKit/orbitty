//! Beach-ball keep-up flight helpers (view-space hops).
//!
//! Extracted from habitat_mac/mod.rs (DIET-HABITAT).

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use std::time::Instant;

use super::geom::{disc_to_view, walker_fit, CGSize, NSPoint, Projection};
use super::pose::yard_step;
use super::{PackFrames, Planet, VIEW_PADDING};

// GO-ORBITTY-WORLD-YARD leftover: beach-ball keep-up

// Ball in VIEW SPACE (leftover: ball in the air). The old disc-unit
// BALL_HEAD / BALL_ARC lerped disc x,y and bulged disc −y — on the 2.5D
// oval that is "back of the oval," not "up in the air," so the hop read
// as a marble skating the grass left–right and landings hid behind the
// sprite. The hop now lives entirely in view pixels: heads are computed
// with the same math the draw blit uses, and the arc lifts UP THE SCREEN
// (−y in the flipped view).

/// Mid-hop lift as a multiple of the target walker's drawn sprite height
/// (view px): clearly off the grass at mid-hop, never a bulge along the
/// oval. Hashed per hop in [BALL_ARC_HEADS_MIN, BALL_ARC_HEADS_MAX].
pub(crate) const BALL_ARC_HEADS_MIN: f64 = 1.2;
pub(crate) const BALL_ARC_HEADS_MAX: f64 = 1.8;

/// Live beach-ball keep-up flight (in-memory only — never persisted).
/// `from` is the VIEW-PIXEL head of the last lander, snapshotted at hop
/// start (f = 0 touches it); the ball arcs to the current target's LIVE
/// view-pixel head (f = 1 touches it — a darting walker still gets
/// tagged) over `dur` secs, then hops to the next Doing body
/// round-robin with no rest on the ground. 2px-kiss leftover: NO
/// contact hold — the hop is done at elapsed >= dur, linear f, constant
/// speed; touch → pop → next hop, never a linger on the head. `arc` is
/// the mid-hop screen lift in px, snapshotted at hop start (≈1.2–1.8 ×
/// the target's sprite height). `stream` feeds the per-hop duration/arc
/// hashes.
#[derive(Clone, Debug)]
pub(crate) struct BallState {
    pub(crate) from: (f64, f64),
    pub(crate) to_id: String,
    pub(crate) at: Instant,
    pub(crate) dur: f64,
    pub(crate) arc: f64,
    pub(crate) stream: u64,
}

/// Hop duration: 1.0–1.45s off the ball's own stream (advanced per
/// hop). Outer-space keep-up leftover: slower than the 0.6–0.9s yard
/// hop — the same screen-up parabola, just unhurried. Walkers are NOT
/// slowed to match (dart stays 0.16·dt, ball-off amble 0.045·dt).
pub(crate) fn ball_hop_secs(stream: &mut u64) -> f64 {
    *stream = yard_step(*stream);
    1.0 + (((*stream >> 33) & 0xffff) as f64 / 65535.0) * 0.45
}

/// Arc lift multiplier: BALL_ARC_HEADS_MIN–MAX × the target's sprite
/// height, off the ball's own stream (advanced per hop, after the
/// duration draw).
pub(crate) fn ball_arc_heads(stream: &mut u64) -> f64 {
    *stream = yard_step(*stream);
    BALL_ARC_HEADS_MIN
        + (((*stream >> 33) & 0xffff) as f64 / 65535.0) * (BALL_ARC_HEADS_MAX - BALL_ARC_HEADS_MIN)
}

/// Aspect (w/h) of a pack's still frame, for the keep-up head math —
/// 0.0 when the pack has no decoded frames (the generated person draws
/// instead, same fallback as the blit). Main thread only (NSImage send).
pub(crate) fn pack_still_aspect(pf: &PackFrames) -> f64 {
    let Some(&img) = pf.idle.first().or_else(|| pf.walk.first()) else {
        return 0.0;
    };
    let size: CGSize = unsafe { msg_send![img as *mut Object, size] };
    if size.width > 0.0 && size.height > 0.0 {
        size.width / size.height
    } else {
        1.0
    }
}

/// A walker's HEAD in view pixels — the same point the draw blit stands
/// the sprite under: disc feet → `disc_to_view` → bottom-anchor
/// `base_y` (frame.max_y() − 2 + bob; the hit box carries NO 2.5D
/// emphasis, the sprite fit does — same split as the blit) → sprite
/// height `dh` from `walker_fit` (pack aspect, else the generated
/// person's pixel grid) → kiss = top-center `(cx, base_y − dh)` plus a
/// flat 12 px DOWN (flipped view: +y is down; NOT a fraction of dh —
/// +3 floated 2–3 px of air, +6 still floated on the thicker pack
/// padding, 0.18·dh buried the crown). Pack PNGs carry transparent
/// padding above the hair, so the raw box top reads as air — the
/// +12 px drop lands past the padding with a few px of hair/crown
/// UNDER the bottom-anchored ball at f = 0 / f = 1 (a real touch: not
/// hover, not a burial; cheap — no opaque-pixel scan; still no contact
/// hold — hop_done at elapsed >= dur, linear f). `pack_aspects` is
/// per-pack
/// `pack_still_aspect` output (0.0 = the generated person draws).
/// Returns `(head_x, head_y, dh)`; None when the walker or its roam is
/// gone.
pub(crate) fn walker_head_view(
    pack_aspects: &[f64],
    planet: &Planet,
    entry_id: &str,
) -> Option<(f64, f64, f64)> {
    let b = planet.walkers.iter().find(|b| b.entry_id == entry_id)?;
    let r = planet.roam.get(&b.entry_id)?;
    let side = (planet.radius + VIEW_PADDING) * 2.0;
    let center = NSPoint {
        x: side / 2.0,
        y: side / 2.0,
    };
    let pt = disc_to_view(center, planet.radius, planet.projection, r.x, r.y);
    let fit = walker_fit(planet.radius, planet.sprite_scale * b.user_scale);
    let base_y = pt.y + fit / 2.0 - 2.0 + b.bob;
    let scale = match planet.projection {
        Projection::Oblique => 1.0 + 0.25 * r.y,
        Projection::Flat => 1.0,
    };
    let fit = fit * scale;
    let aspect = b
        .pack
        .and_then(|p| pack_aspects.get(p))
        .copied()
        .unwrap_or(0.0);
    let dh = if aspect > 0.0 {
        if aspect >= 1.0 { fit / aspect } else { fit }
    } else {
        crate::chars::H as f64 * (fit / crate::chars::H as f64).clamp(1.5, 8.0)
    };
    Some((pt.x, base_y - dh + 12.0, dh))
}

/// The ball's VIEW-PIXEL position right now (draw blits it as returned —
/// no second projection): head-to-head lerp in screen space — `from`
/// (the last lander's head at hop start) to the target's LIVE head (it
/// tracks a darting walker) — plus the mid-hop lift straight up the
/// screen (−y in the flipped view). f = 0 and f = 1 are TOUCHES (ball
/// overlapping a head); sin(πf)·arc lifts the middle clearly off the
/// grass above BOTH heads. Linear f, constant speed — no ease, no
/// dwell, no slowdown in the last stretch; the next hop starts at
/// elapsed >= dur (f clamps at 1 only for the tick in between). A
/// target that vanished mid-hop holds at the launch point until the
/// next hop re-targets. None = hidden (toggle off / fewer than 2
/// Doing).
pub(crate) fn ball_pos(
    pack_aspects: &[f64],
    planet: &Planet,
    now: Instant,
) -> Option<(f64, f64)> {
    let ball = planet.ball.as_ref()?;
    let to = walker_head_view(pack_aspects, planet, &ball.to_id)
        .map(|(hx, hy, _)| (hx, hy))
        .unwrap_or(ball.from);
    let f = (now.duration_since(ball.at).as_secs_f64() / ball.dur).min(1.0);
    Some((
        ball.from.0 + (to.0 - ball.from.0) * f,
        ball.from.1 + (to.1 - ball.from.1) * f - ball.arc * (std::f64::consts::PI * f).sin(),
    ))
}

