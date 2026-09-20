//! Walker/planet anim_tick (DIET-HABITAT tick carve).
//!
//! Extracted from habitat_mac/tick.rs.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use std::time::{Duration, Instant};

use super::{
    ball_arc_heads, ball_hop_secs, droid_pack, emit_board_checkin, emit_walked_off, house_spawn,
    hash_str, layout_beings, lock, pack_still_aspect, sleeper_target, station_target, walkoff_target,
    walker_head_view, yard_clamp, yard_contains, yard_dwell_ms, yard_pick, yard_pick_sailing,
    yard_seed, BallState, Drag, Pose, RoamState, CAPTION_TTL, HOME_HIDE_X, LOCATE_TTL,
    PLANET_LOCATE_TTL, YARD_ARRIVE, YARD_SAIL,
};

pub(crate) fn anim_tick() {
    let mut h = lock();
    let now = Instant::now();
    let dt = h
        .last_anim
        .map(|t| now.duration_since(t).as_secs_f64())
        .unwrap_or(0.05)
        .min(0.25);
    h.last_anim = Some(now);
    // motion off (toggle or Reduce Motion) →
    // marbles sit at default size and orbit state clears, so a re-enable
    // re-seeds from the live frame. Runs before the Reduce Motion early
    // return so stilling also restores size.
    if h.reduce_motion || !h.marble_drift {
        super::marble_motion::orbit_restore(&mut h);
    }
    if h.reduce_motion {
        return; // static sprites under Reduce Motion
    }
    h.bob_phase += dt;
    h.anim_frame = h.anim_frame.wrapping_add(1);
    let bob_phase = h.bob_phase;
    // BUDGET still-planet law: an expanded planet dirties
    // its view ONLY on real motion this tick — a roam step or pause flip,
    // a bob change, a live locate stroke, or a visible caption. An
    // all-sleeper / empty planet paints nothing per tick; sleepers never
    // keep the view dirty. Marbles already `continue`.
    let locate = h.locate.clone();
    let locate_planet = h.locate_planet.clone();
    // PROPS leftover (town usable): a walker mid-drag is owned by
    // drag_tick — the pose branches below must NOT retarget it back to
    // the station on the same press (that fight was "drag moves roam
    // for a frame; next tick retargets"). The drop itself is handled in
    // drag_tick's release (a column drop is a status move).
    let drag_walker = match &h.drag {
        Some(Drag::Walker {
            project_id,
            entry_id,
            ..
        }) => Some((project_id.clone(), entry_id.clone())),
        _ => None,
    };
    // per-pack still-frame
    // aspects for the keep-up head math, snapshotted before the planet
    // loop (0.0 = no decoded frames → the generated person's height).
    let pack_aspects: Vec<f64> = h
        .pack_frames
        .iter()
        .map(|(_, pf)| pack_still_aspect(pf))
        .collect();
    for (pid, planet) in h.planets.iter_mut() {
        if planet.is_marble {
            continue; // marbles are bare and still
        }
        let mut dirty = false;
        // WORLD-PLACES leftover / WORLD-YARD: layout_beings reruns ONLY
        // when a roam x/y actually changed this tick — re-laying out
        // every tick held the habitat lock on the AppKit thread (menu
        // beach-ball). YARD undid the in-place forbid for the Doing
        // wander ONLY: a walking Work body moves every tick (layout
        // reruns, law 4). YARD leftover: ball OFF the amble never parks
        // (a Doing body re-lays out every tick); ball ON the dart parks
        // for each dwell and re-lays out on the darts. Bob alone still
        // dirties the display below.
        let mut moved = false;
        let now_i = Instant::now();
        let ids: Vec<String> = planet.walkers.iter().map(|b| b.entry_id.clone()).collect();
        // bob only on a tick the walker actually
        // steps (pause_end None and dist >= 0.025). Paused and asleep
        // walkers stand at bob 0; sleepers never enter the step branch.
        // a Doing walker bobs while
        // WALKING the grass. Ball OFF the smooth amble never parks, so
        // the bob never stands on the green; ball ON the dart dwell
        // (600–1400 ms) stands at bob 0 between darts — the keep-up
        // ready stance. Stand/Sit still park idle.
        let mut stepping = vec![false; ids.len()];
        // Ambient random roam dies the moment any walker on this planet
        // works or sits — the board, not the clock, drives motion. Sleepers
        // don't count (a closed window is not visible work) and are never
        // retargeted themselves.
        let any_work_sit = planet
            .walkers
            .iter()
            .any(|b| !b.asleep && matches!(b.pose, Pose::Work | Pose::Sit));
        // the beach
        // ball flips the Doing footwork — OFF keeps the smooth amble,
        // ON restores the exact pre-amble dart (the dwell-and-dart IS
        // the keep-up footwork). Captured once per planet per tick.
        let keep_up = planet.beach_ball;
        // WORLD-STOP: Done walks to the south/near lip of the painted
        // rails, not STATION_BOARD (back of the blit). Same rects as the
        // Done-column hit-test.
        let lip = super::draw::board_near_lip(
            planet.radius,
            planet.projection,
            planet.marble_scale,
        );
        for (i, id) in ids.iter().enumerate() {
            let (pose, asleep) = planet
                .walkers
                .get(i)
                .map(|b| (b.pose, b.asleep))
                .unwrap_or((Pose::Roam, false));
            // create/get the roam BEFORE the
            // sleeper continue — a new/asleep claim must not miss its
            // house roam this tick. The seed is pose-aware (the old 0.2
            // cos/sin circle fallback is gone): Stand/Work come from the
            // igloo (WORLD-TOWN: STATION_HOUSE only — user buildings stay
            // décor), Sit seeds AT the rim station, WalkOff keeps the
            // current roam or starts at the igloo — never past
            // HOME_HIDE_X, so the walk off the left rim is visible.
            // Computed before the roam borrow.
            let house = house_spawn(planet, id);
            let roam = planet.roam.entry(id.clone()).or_insert_with(|| match pose {
                Pose::Stand | Pose::Work => {
                    let (tx, ty) = station_target(pose, id);
                    RoamState {
                        x: house.0,
                        y: house.1,
                        tx,
                        ty,
                        pause_end: None,
                        facing_left: false,
                        facing_dir: 0,
                        off_emitted: false,
                        via_board: false,
                        via_home: false,
                        via_checkin: false,
                        checkin_emit_at: None,
                        wander_seed: 0,
                    }
                }
                Pose::Sit => {
                    let (tx, ty) = station_target(Pose::Sit, id);
                    RoamState {
                        x: tx,
                        y: ty,
                        tx,
                        ty,
                        pause_end: None,
                        facing_left: false,
                        facing_dir: 0,
                        off_emitted: false,
                        via_board: false,
                        via_home: false,
                        via_checkin: false,
                        checkin_emit_at: None,
                        wander_seed: 0,
                    }
                }
                Pose::WalkOff => RoamState {
                    x: house.0,
                    y: house.1,
                    tx: lip.0,
                    ty: lip.1,
                    pause_end: None,
                    facing_left: false,
                        facing_dir: 0,
                    off_emitted: false,
                    via_board: false,
                    via_home: false,
                    via_checkin: false,
                    checkin_emit_at: None,
                    wander_seed: 0,
                },
                Pose::Roam => {
                    let mut seed = hash_str(id);
                    let (x, y) = yard_pick(&mut seed);
                    RoamState {
                        x,
                        y,
                        tx: x,
                        ty: y,
                        pause_end: None,
                        facing_left: false,
                        facing_dir: 0,
                        off_emitted: false,
                        via_board: false,
                        via_home: false,
                        via_checkin: false,
                        checkin_emit_at: None,
                        wander_seed: seed,
                    }
                },
            });
            // A walker that left WalkOff (re-claimed) re-arms its emit so
            // the NEXT Done walks off and evicts again — and re-arms the
            // board visit too (LEFTOVER via_board).
            if pose != Pose::WalkOff {
                roam.off_emitted = false;
                roam.via_board = false;
                roam.via_home = false;
            }
            // Mid-drag: drag_tick owns this walker's x/y/tx/ty for the
            // press — no station retarget, no step, no bob here.
            if drag_walker
                .as_ref()
                .is_some_and(|(dpid, deid)| dpid == pid && deid == id)
            {
                continue;
            }
            // Closed sleepers park in the camp (north of igloo, west of
            // grass) — dim + still, never the green. Minimized is NOT
            // asleep (full-color on the green). A Done card still walks
            // a sleeper off: the complete ritual closes the window, and
            // the walk-off narrates the eviction.
            if asleep && pose != Pose::WalkOff {
                let (tx, ty) = sleeper_target(id);
                if (roam.x - tx).abs() > 1e-9 || (roam.y - ty).abs() > 1e-9 {
                    roam.x = tx;
                    roam.y = ty;
                    moved = true;
                }
                roam.tx = tx;
                roam.ty = ty;
                roam.pause_end = None;
                continue;
            }
            if planet
                .tree_chores
                .iter()
                .any(|c| c.walker_id == *id)
            {
                continue;
            }
            match pose {
                // Needed: bad news skips the queue — snap to the gate, never
                // a scenic route. Sit = idle frame 0, no bob, full bright.
                Pose::Sit => {
                    let (tx, ty) = station_target(Pose::Sit, id);
                    if (roam.x - tx).abs() > 1e-9 || (roam.y - ty).abs() > 1e-9 {
                        roam.x = tx;
                        roam.y = ty;
                        moved = true;
                    }
                    roam.tx = tx;
                    roam.ty = ty;
                    roam.pause_end = None;
                }
                // Stand walks to the board lip (not the post foot).
                // Work ambles the grass AABB. Never moonwalk on one pixel.
                Pose::Stand | Pose::Work => {
                    // Back from off the left rim (re-claim after Done):
                    // the walker was invisible, so re-enter AT THE IGLOO
                    // and walk the return — never a snap onto the station.
                    if roam.x < HOME_HIDE_X {
                        roam.x = house.0;
                        roam.y = house.1;
                        // the return from hide
                        // is a fresh claim journey — re-arm the board
                        // check-in so this claim touches the board too.
                        roam.via_checkin = false;
                        moved = true;
                    }
                    if pose == Pose::Stand {
                        roam.tx = lip.0;
                        roam.ty = lip.1;
                        roam.pause_end = None;
                    } else {
                        if roam.wander_seed == 0 {
                            roam.wander_seed = yard_seed(id);
                            let (tx, ty) = yard_pick(&mut roam.wander_seed);
                            roam.tx = tx;
                            roam.ty = ty;
                            roam.pause_end = None;
                        } else if !yard_contains(roam.tx, roam.ty) {
                            let (tx, ty) = yard_pick(&mut roam.wander_seed);
                            roam.tx = tx;
                            roam.ty = ty;
                            roam.pause_end = None;
                        }
                        if !keep_up {
                            // Ball off: the amble never dwells — a pause
                            // left by a mid-dwell toggle-off dies now,
                            // never a standing park on the green.
                            roam.pause_end = None;
                        }
                    }
                    let mut dx = roam.tx - roam.x;
                    let mut dy = roam.ty - roam.y;
                    let mut dist = (dx * dx + dy * dy).sqrt();
                    // CHECKIN (+ PROPS leftover retry): a
                    // Stand (Todo) walker at the near lip of the board
                    // checks in — the rail flips that window's card
                    // todo→doing and the next sync walks it to the field
                    // as Work. The emit is fire-and-forget, so while the
                    // snapshot STILL says todo it re-fires on a ~0.5s
                    // throttle: a missed rail (or an AX-untrusted no-op
                    // sync) used to park the walker at the lip forever.
                    // via_checkin is NOT set on this auto path — it stays
                    // the "user dropped on Todo, don't auto-flip" flag
                    // (only the drag-drop column paths set it). The rail's
                    // listener is idempotent (todo→doing only), so the
                    // repeats are safe.
                    if pose == Pose::Stand && !roam.via_checkin && dist < 0.05 {
                        let still_todo = planet
                            .walker_entries
                            .iter()
                            .any(|e| &e.id == id && e.card_status == "todo");
                        let due = roam
                            .checkin_emit_at
                            .is_none_or(|t| t.elapsed() >= Duration::from_millis(500));
                        if still_todo && due {
                            roam.checkin_emit_at = Some(now_i);
                            emit_board_checkin(id);
                        }
                    }
                    if pose == Pose::Work {
                        if keep_up {
                            // Ball ON — the exact pre-amble dart: on the
                            // grass point (dist < YARD_ARRIVE, 0.04)
                            // dwell 600–1400 ms (hashed off the stream)
                            // STANDING — the dwell is the pause, no step,
                            // no bob — then pick a NEW random point in
                            // the box and dart on.
                            if dist < YARD_ARRIVE && roam.pause_end.is_none() {
                                // Arrival slop (<= YARD_ARRIVE) snaps onto
                                // the green — the one clamp that can move
                                // a body, sub-0.04, invisible.
                                let (cx, cy) = yard_clamp(roam.x, roam.y);
                                roam.x = cx;
                                roam.y = cy;
                                let dwell = yard_dwell_ms(&mut roam.wander_seed);
                                roam.pause_end =
                                    Some(now_i + Duration::from_millis(dwell));
                                dirty = true; // point reached — the walk cycle parks
                            }
                            if let Some(end) = roam.pause_end {
                                if now_i < end {
                                    continue; // dwelling on the point
                                }
                                roam.pause_end = None;
                                let (tx, ty) = yard_pick(&mut roam.wander_seed);
                                roam.tx = tx;
                                roam.ty = ty;
                                dx = roam.tx - roam.x;
                                dy = roam.ty - roam.y;
                                dist = (dx * dx + dy * dy).sqrt();
                                dirty = true; // new point — the walk cycle resumes
                            }
                        } else if dist < YARD_SAIL {
                            // Ball OFF — smooth wander: BEFORE arriving
                            // (dist < YARD_SAIL, 0.12) the NEXT point is
                            // already picked — no pause, no snap-turn.
                            // The pick prefers a heading that is not a
                            // reverse of the current dx,dy (an about-face
                            // is skipped up to 3 stream steps), so the
                            // body curves onto the next leg instead of
                            // darting back.
                            let (tx, ty) = yard_pick_sailing(
                                &mut roam.wander_seed,
                                roam.x,
                                roam.y,
                                dx,
                                dy,
                            );
                            roam.tx = tx;
                            roam.ty = ty;
                            dx = roam.tx - roam.x;
                            dy = roam.ty - roam.y;
                            dist = (dx * dx + dy * dy).sqrt();
                            dirty = true; // target handed off mid-stride
                        }
                    }
                    if dist >= 0.025 {
                        // Ball OFF: slow amble on the grass (0.045·dt —
                        // ball-in-the-air leftover: 0.075 still read
                        // brisk). Ball ON: the dart keeps the full
                        // 0.16·dt on the green — the keep-up footwork.
                        // Either way the off-grass walk-IN (from the
                        // igloo / board lip / a hand drop) keeps the old
                        // 0.16·dt — the visible approach is unchanged.
                        let speed = if pose == Pose::Work
                            && !keep_up
                            && yard_contains(roam.x, roam.y)
                        {
                            0.045
                        } else {
                            0.16
                        };
                        let step = (speed * dt).min(dist);
                        roam.x += dx / dist * step;
                        roam.y += dy / dist * step;
                        roam.facing_left = dx < -0.002;
                        roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
                        stepping[i] = true;
                        moved = true; // walking (to the lip / across the grass)
                    }
                    // WORLD-YARD law 6: a Doing walker ON the grass never
                    // leaves it — hard clamp to the inset AABB (a no-op on
                    // the convex walk between two in-box points; the wall
                    // against drift/bugs — never the igloo x <= -0.50,
                    // never the sign footprint). The walk-IN from the
                    // igloo / board lip / a hand drop stays a visible
                    // approach: its target is in the box and converges —
                    // clamping THAT leg would pop the body onto the edge.
                    if pose == Pose::Work && yard_contains(roam.x, roam.y) {
                        let (cx, cy) = yard_clamp(roam.x, roam.y);
                        if (cx - roam.x).abs() > 1e-12 || (cy - roam.y).abs() > 1e-12 {
                            roam.x = cx;
                            roam.y = cy;
                            moved = true;
                        }
                    }
                }
                // Done: walk to the near lip of the board, blit flips,
                // dwell so checkout reads, walk home to the igloo, dwell
                // at the door, then unclaim. No north walk-through. No
                // left-rim curtain; last walker lets the planet shrink.
                Pose::WalkOff => {
                    let is_droid = planet
                        .walkers
                        .get(i)
                        .and_then(|b| b.droid)
                        .is_some();
                    // Bare / kind:window Done: skip board lip — home then off.
                    // Real kind:todo keeps the checkout parade.
                    let checkout_board = planet
                        .walker_entries
                        .iter()
                        .any(|e| &e.id == id && e.card_kind == "todo");
                    if !checkout_board {
                        roam.via_board = true;
                    }
                    if is_droid {
                        let (tx, ty) = walkoff_target(id, roam.x, roam.y);
                        let dx = tx - roam.x;
                        let dy = ty - roam.y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        roam.tx = tx;
                        roam.ty = ty;
                        if dist < 0.05 || roam.x < HOME_HIDE_X {
                            if !roam.off_emitted {
                                roam.off_emitted = true;
                                emit_walked_off(id);
                                dirty = true;
                            }
                        } else {
                            let step = (0.16 * dt).min(dist);
                            roam.x += dx / dist * step;
                            roam.y += dy / dist * step;
                            roam.facing_left = dx < -0.002;
                        roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
                            stepping[i] = true;
                            moved = true;
                        }
                        continue;
                    }
                    if roam.x < HOME_HIDE_X {
                        roam.x = house.0;
                        roam.y = house.1;
                        roam.via_board = false;
                        roam.via_home = false;
                        roam.off_emitted = false;
                        roam.pause_end = None;
                        moved = true;
                    }
                    if !roam.via_board {
                        let dx = lip.0 - roam.x;
                        let dy = lip.1 - roam.y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist < 0.05 {
                            roam.via_board = true;
                            roam.x = lip.0;
                            roam.y = lip.1;
                            roam.tx = lip.0;
                            roam.ty = lip.1;
                            roam.pause_end =
                                Some(now_i + Duration::from_millis(400));
                            dirty = true; // lift blit hold this frame
                            moved = true;
                        } else {
                            roam.tx = lip.0;
                            roam.ty = lip.1;
                            let step = (0.16 * dt).min(dist);
                            roam.x += dx / dist * step;
                            roam.y += dy / dist * step;
                            roam.facing_left = dx < -0.002;
                        roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
                            stepping[i] = true;
                            moved = true;
                            continue;
                        }
                    }
                    if let Some(end) = roam.pause_end {
                        if now_i < end {
                            continue;
                        }
                        roam.pause_end = None;
                    }
                    if !roam.via_home {
                        let dx = house.0 - roam.x;
                        let dy = house.1 - roam.y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist < 0.05 {
                            roam.via_home = true;
                            roam.x = house.0;
                            roam.y = house.1;
                            roam.tx = house.0;
                            roam.ty = house.1;
                            roam.pause_end =
                                Some(now_i + Duration::from_millis(400));
                            dirty = true;
                            moved = true;
                            continue;
                        } else {
                            roam.tx = house.0;
                            roam.ty = house.1;
                            let step = (0.16 * dt).min(dist);
                            roam.x += dx / dist * step;
                            roam.y += dy / dist * step;
                            roam.facing_left = dx < -0.002;
                        roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
                            stepping[i] = true;
                            moved = true;
                            continue;
                        }
                    }
                    roam.tx = house.0;
                    roam.ty = house.1;
                    if !roam.off_emitted {
                        roam.off_emitted = true;
                        emit_walked_off(id);
                        dirty = true;
                    }
                }
                Pose::Roam => {
                    if any_work_sit {
                        // Amble off while anyone works/sits: stand in
                        // place — no re-park, no pause cycle.
                        roam.tx = roam.x;
                        roam.ty = roam.y;
                        roam.pause_end = None;
                        continue;
                    }
                    if let Some(end) = roam.pause_end {
                        if now_i < end {
                            continue;
                        }
                        // Pause ended — both branches below (re-park / step)
                        // mark the dirty walk-cycle change.
                        roam.pause_end = None;
                    }
                    let dx = roam.tx - roam.x;
                    let dy = roam.ty - roam.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < 0.025 {
                        let a = bob_phase * 2.7 + i as f64 * 4.1;
                        let rad = 0.22 + 0.42 * ((bob_phase * 1.3 + i as f64).sin() * 0.5 + 0.5);
                        roam.tx = a.cos() * rad;
                        roam.ty = a.sin() * rad;
                        roam.pause_end = Some(
                            now_i + Duration::from_millis(700 + ((i as u64 * 410) % 1400)),
                        );
                        dirty = true; // target reached — the walk cycle parks
                    } else {
                        let step = (0.16 * dt).min(dist);
                        roam.x += dx / dist * step;
                        roam.y += dy / dist * step;
                        roam.facing_left = dx < -0.002;
                        roam.facing_dir = droid_pack::dir_from_delta(dx, dy, roam.facing_dir);
                        stepping[i] = true;
                        moved = true; // roam stepped
                    }
                }
            }
        }
        // WORLD-PLACES leftover / WORLD-YARD: full layout ONLY on a real
        // roam position change — re-laying out every tick held the
        // habitat lock on the AppKit thread (menu beach-ball). The Doing
        // amble moves every tick (layout reruns there, law 4); the
        // ball-on dart moves only between dwells. A move implies a
        // display refresh.
        if super::todo_tree::tick(planet, dt, pid) {
            moved = true;
        }
        if moved {
            layout_beings(planet);
            dirty = true;
        }
        for (i, b) in planet.walkers.iter_mut().enumerate() {
            let bob = if stepping.get(i).copied().unwrap_or(false) {
                (bob_phase * 2.2 + (i as f64) * 1.1).sin() * 1.4
            } else {
                0.0
            };
            if bob != b.bob {
                b.bob = bob;
                dirty = true;
            }
        }
        // beach-ball
        // keep-up. The per-planet toggle (right-click menu, persisted
        // next to the boombox) plus ≥ 2 awake Doing walkers → the ball
        // hops head to head IN VIEW PIXELS — TOUCH walker A's head, pop
        // straight up the screen, arc, drop, TOUCH walker B's LIVE head
        // (a darting walker still gets tagged) — one hop per 1.0–1.45s,
        // then the next hop with NO hold on the head (2px-kiss leftover:
        // constant speed, touch → pop → next hop, never a linger, no
        // rest on the ground), round-robin through the Doing set (never the
        // same pair back-to-back when 3+). One or zero Doing → hidden. The ball is
        // NOT a third wanderer: it lands on bodies, never on a random
        // grass pixel, and the walkers' dart IS the keep-up footwork.
        // Heads come from walker_head_view — the same blit math draw.rs
        // uses — so a touch overlaps the drawn sprite exactly. In flight
        // the ball moves every tick → the view stays dirty.
        if planet.beach_ball {
            let doing: Vec<(String, (f64, f64, f64))> = planet
                .walkers
                .iter()
                .filter(|b| !b.asleep && b.pose == Pose::Work)
                .filter_map(|b| {
                    walker_head_view(&pack_aspects, planet, &b.entry_id)
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
                if let Some((hx, _, _)) = walker_head_view(&pack_aspects, planet, &to_id) {
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
        if !dirty {
            // Transient overlays keep painting for their window only: the
            // locate stroke (~500ms) and a visible caption (~2s). Their
            // set/expire sites dirty the transitions; this covers the ticks
            // in between so the ring/pill never half-draws.
            dirty = locate.as_ref().is_some_and(|(id, at)| {
                at.elapsed() < LOCATE_TTL && planet.walkers.iter().any(|b| &b.entry_id == id)
            }) || locate_planet.as_ref().is_some_and(|(id, at)| {
                id == pid && at.elapsed() < PLANET_LOCATE_TTL
            }) || planet
                .caption
                .as_ref()
                .is_some_and(|(_, at)| at.elapsed() < CAPTION_TTL);
        }
        if dirty {
            unsafe {
                let view = planet.view as *mut Object;
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
        }
    }
    // The marble drift lives in drift_tick, which runs every habitat
    // tick (~16ms) so drifting marbles don't step at this 3-tick walker
    // cadence.
}
