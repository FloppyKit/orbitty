//! World-view drawRect / blit.
//! Class registration stays on the host; selector body lives here.
//! Planet / Being stay crate-private on the host.
//!
//! Disc painting lives in draw_world.rs (DIET-HABITAT).

#![allow(unexpected_cfgs)]

use objc::runtime::{Object, Sel};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

pub(crate) use super::board_geom::{
    board_column_rects, board_near_lip, hit_board_column, sign_blit_rect,
};

use super::decor_art::growth_t;
#[path = "draw_world.rs"]
mod draw_world;
use draw_world::{DecorDraw, DrawData, WalkerDraw};
use super::{
    ball_pos, boombox_hud, card_color, droid, droid_pack, ensure_town_art, lock, now_epoch,
    pack_still_aspect, panic_msg, station_anchor, walker_hidden, DecorKind, Habitat, NSPoint, NSRect,
    Pose, Projection, CAPTION_TTL, LOCATE_TTL, PLANET_LOCATE_TTL,
};

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
        droid_pack::ensure_loaded(&mut h);
        // bake this view's droid atlases BEFORE
        // the immutable draw pass — once per seed, then frame_ptr only
        // reads. Never a per-frame procgen.
        let droid_seeds = h
            .planets
            .iter()
            .find(|(_, p)| p.view == view_ptr)
            .map(|(_, p)| p.walkers.iter().filter_map(|b| b.droid).collect::<Vec<u64>>())
            .unwrap_or_default();
        let kits = droid_pack::ready(&h);
        for seed in droid_seeds {
            if !kits {
                droid::ensure_baked(&mut h, seed);
            }
        }
        let town_igloo = h.town_igloo;
        let town_sign = h.town_sign;
        let town_boombox = h.town_boombox;
        let town_ball = h.town_ball;
        let now = now_epoch();
        // per-pack still-frame
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
                    // a Done walker past the hide line is
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
                        // a droid blits its baked
                        // atlas (Sit = seated frame; cycling = the two
                        // walk frames) — window walkers keep the bitty
                        // sheets / generated person below.
                        pack_frame: if let Some(seed) = b.droid {
                            let dir = droid_pack::dir_for(
                                b.pose,
                                roam.map(|r| r.facing_dir).unwrap_or(0),
                            );
                            let painted = droid_pack::frame_ptr(
                                &h,
                                seed,
                                dir,
                                cycling && !sleeper_still,
                                h.anim_frame,
                                i,
                            );
                            if painted != 0 {
                                painted
                            } else {
                                droid::frame_ptr(
                                    &h,
                                    seed,
                                    b.pose,
                                    cycling && !sleeper_still,
                                    h.anim_frame,
                                    i,
                                    b.droid_glance,
                                )
                            }
                        } else if sleeper_still {
                            pack_frame_still(&h, b.pack)
                        } else {
                            pack_frame_ptr(&h, b.pack, i, cycling)
                        },
                        dim: b.dim,
                        flip: if b.droid.is_some() && droid_pack::ready(&h) {
                            // 4-dir kits ship west as its own sheet.
                            false
                        } else {
                            match b.pose {
                                // Sit at the front rim (WORLD-PLACES): the
                                // anchor is due south (x = 0), so this never
                                // mirrors — right-facing art stands along the
                                // rim. (The old east gate flipped to face the
                                // planet center.)
                                Pose::Sit => station_anchor(Pose::Sit)
                                    .map(|(ax, _)| ax > 0.0)
                                    .unwrap_or(false),
                                _ => roam.map(|r| r.facing_left).unwrap_or(false),
                            }
                        },
                        scale: match p.projection {
                            Projection::Oblique => 1.0 + 0.25 * y,
                            Projection::Flat => 1.0,
                        },
                        droid_glance: b.droid_glance,
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
            // the
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
    unsafe { draw_world::draw_world(this, &data) }
}
