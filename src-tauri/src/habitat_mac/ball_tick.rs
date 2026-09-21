//! Beach-ball keep-up hop this anim tick (DIET-HABITAT).
//!
//! Extracted from habitat_mac/anim.rs. Walker footwork stays in anim_tick;
//! this owns the in-flight hop + last-stretch glance.

#![allow(unexpected_cfgs)]

use std::time::Instant;

use super::droid_pack;
use super::{
    ball_arc_heads, ball_hop_secs, walker_head_view, yard_seed, BallState, Planet, Pose,
};

/// Advance keep-up for this planet. Returns true if the view must redraw.
pub(crate) fn keep_up_tick(
    planet: &mut Planet,
    pack_aspects: &[f64],
    now_i: Instant,
) -> bool {
    let mut dirty = false;
    // The per-planet toggle (right-click menu, persisted next to the
    // boombox) plus ≥ 2 awake Doing walkers → the ball hops head to head
    // IN VIEW PIXELS — TOUCH walker A's head, pop straight up the screen,
    // arc, drop, TOUCH walker B's LIVE head (a darting walker still gets
    // tagged) — one hop per 1.0–1.45s, then the next hop with NO hold on
    // the head (2px-kiss leftover: constant speed, touch → pop → next hop,
    // never a linger, no rest on the ground), round-robin through the
    // Doing set (never the same pair back-to-back when 3+). One or zero
    // Doing → hidden. The ball is NOT a third wanderer: it lands on
    // bodies, never on a random grass pixel, and the walkers' dart IS the
    // keep-up footwork. Heads come from walker_head_view — the same blit
    // math draw.rs uses — so a touch overlaps the drawn sprite exactly.
    // In flight the ball moves every tick → the view stays dirty.
    if planet.beach_ball {
        // Bare claims roam; real todos Work. The ball needs bodies on
        // the grass either way — Work-only hid it after window→Roam.
        let doing: Vec<(String, (f64, f64, f64))> = planet
            .walkers
            .iter()
            .filter(|b| !b.asleep && matches!(b.pose, Pose::Work | Pose::Roam))
            .filter_map(|b| {
                walker_head_view(pack_aspects, planet, &b.entry_id)
                    .map(|head| (b.entry_id.clone(), head))
            })
            .collect();
        if doing.len() >= 2 {
            // 2px-kiss leftover: the hop is done at elapsed >= dur —
            // NO contact hold, no ease, no dwell, no slowdown in the
            // last stretch. Linear f; the ball touches the lander's
            // LIVE head at f = 1 and the next hop pops immediately.
            // Never a rest on the grass.
            let hop_done = match &planet.ball {
                Some(ball) => {
                    !doing.iter().any(|(id, _)| id == &ball.to_id)
                        || now_i.duration_since(ball.at).as_secs_f64() >= ball.dur
                }
                None => true,
            };
            if hop_done {
                // `from` = the VIEW-PIXEL head of the last lander,
                // snapshotted NOW: the previous hop ended on that
                // head (f = 1), so the new hop leaves from the touch
                // point — never from the feet, never a grass pixel.
                let (from, mut stream, last_to) = match &planet.ball {
                    Some(ball) => (
                        doing
                            .iter()
                            .find(|(id, _)| id == &ball.to_id)
                            .map(|(_, (hx, hy, _))| (*hx, *hy))
                            .unwrap_or(ball.from),
                        ball.stream,
                        Some(ball.to_id.clone()),
                    ),
                    None => (
                        (doing[0].1 .0, doing[0].1 .1),
                        yard_seed(&doing[0].0),
                        None,
                    ),
                };
                // Round-robin: land on the NEXT Doing body after the
                // last target — cycles the set (with exactly 2 it
                // alternates; 3+ never repeats a pair back-to-back).
                let next = last_to
                    .as_ref()
                    .and_then(|lt| doing.iter().position(|(id, _)| id == lt))
                    .map(|i| (i + 1) % doing.len())
                    .unwrap_or(1);
                let dur = ball_hop_secs(&mut stream);
                // The lift is SCREEN-space: 1.2–1.8 × the target's
                // drawn sprite height — clearly off the grass at
                // mid-hop, never a bulge along the oval.
                let arc = ball_arc_heads(&mut stream) * doing[next].1 .2;
                planet.ball = Some(BallState {
                    from,
                    to_id: doing[next].0.clone(),
                    at: now_i,
                    dur,
                    arc,
                    stream,
                });
            }
            dirty = true; // in flight — redraw every tick
        } else if planet.ball.take().is_some() {
            dirty = true; // dropped below 2 Doing — hidden
        }
    } else if planet.ball.take().is_some() {
        dirty = true; // toggled off — hidden
    }
    // Optional glance: the keep-up target faces the incoming ball in
    // the last stretch of the hop (cheap; the walk facing resumes the
    // tick after it lands). Both x's are VIEW px (the head helper) —
    // never a view/disc mix.
    if let Some(ball) = &planet.ball {
        let f = (now_i.duration_since(ball.at).as_secs_f64() / ball.dur).min(1.0);
        if f > 0.45 {
            let from_x = ball.from.0;
            let to_id = ball.to_id.clone();
            if let Some((hx, _, _)) = walker_head_view(pack_aspects, planet, &to_id) {
                let ball_x = from_x + (hx - from_x) * f;
                if let Some(r) = planet.roam.get_mut(&to_id) {
                    r.facing_left = ball_x < hx;
                    r.facing_dir = if ball_x < hx {
                        droid_pack::DIR_W
                    } else {
                        droid_pack::DIR_E
                    };
                }
            }
        }
    }
    dirty
}
