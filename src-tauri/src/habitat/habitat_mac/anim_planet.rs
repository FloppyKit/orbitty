//! Per-expanded-planet walker / bob / keep-up anim (DIET-HABITAT).
//!
//! Extracted from habitat_mac/anim.rs `anim_tick` planet loop.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use std::time::Instant;

use super::{
    hash_str, house_spawn, layout_beings, sleeper_target, station_target, yard_pick, Planet,
    Pose, RoamState, CAPTION_TTL, LOCATE_TTL, PLANET_LOCATE_TTL,
};

/// One expanded planet's walker footwork + bob + beach-ball keep-up for this anim tick.
pub(crate) fn tick_expanded(
    planet: &mut Planet,
    pid: &str,
    dt: f64,
    bob_phase: f64,
    drag_walker: &Option<(String, String)>,
    locate: &Option<(String, Instant)>,
    locate_planet: &Option<(String, Instant)>,
    pack_aspects: &[f64],
) {
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
        let is_droid = planet
            .walkers
            .get(i)
            .and_then(|b| b.droid)
            .is_some();
        let still_todo = planet
            .walker_entries
            .iter()
            .any(|e| &e.id == id && e.card_status == "todo");
        let checkout_board = planet
            .walker_entries
            .iter()
            .any(|e| &e.id == id && e.card_kind == "todo");
        super::anim_step::step_pose(
            roam,
            pose,
            id,
            i,
            dt,
            now_i,
            bob_phase,
            house,
            lip,
            keep_up,
            any_work_sit,
            is_droid,
            still_todo,
            checkout_board,
            &mut moved,
            &mut dirty,
            &mut stepping[i],
        );
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
    if super::ball_tick::keep_up_tick(planet, &pack_aspects, now_i) {
        dirty = true;
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
