//! Habitat snapshot sync — claim / unclaim / layout apply (DIET-HABITAT).
//!
//! Extracted from habitat_mac/mod.rs (`do_sync` + pose-event flush).

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::time::Instant;
use tauri::{Emitter, Manager};

use super::audio;
use super::draw;
use super::todo_tree;
use super::sync_budget;
use super::pose::{
    droid_glance, pose_for, pose_name, station_target, sticky_pose, Pose, HOME_HIDE_X,
};
use super::{
    app_handle, color_for, default_state, effective_radius, emit_walked_off, ensure_pack_loaded,
    layout_beings, layout_decor, load_pack_frames, load_worlds, lock, locked_projection,
    locked_projection_str, make_being, make_panel, now_epoch, ns_string, panel_frame,
    panel_set_frame, primary_screen_id, ring_claimed, spawn_ring, unload_dead_packs,
    world_view_class, Being, HabitatEntry, HabitatPlanet, HabitatSnapshot, MAX_MARBLE_PANELS,
    NSRect, Planet, VIEW_PADDING,
};

pub(crate) fn do_sync(snapshot: &HabitatSnapshot) {
    // Pose transitions are collected under the lock and appended to the
    // event log AFTER the lock drops, on a background thread — jsonl IO
    // never runs on the AppKit thread inside the habitat lock (that was
    // the second half of the board-drag beach ball). A missed pose line
    // beats a stalled UI.
    let mut pose_events: Vec<(String, String, Pose, Pose)> = Vec::new();
    let mut h = lock();
    load_worlds(&mut h);
    // Re-arm the once-only Done fast-forward: the set tracks entries that
    // already emitted; an entry whose card leaves Done (uncheck, new open
    // card) drops out so its next Done emits again.
    {
        let done_ids: std::collections::HashSet<&str> = snapshot
            .planets
            .iter()
            .flat_map(|p| p.entries.iter())
            .filter(|e| pose_for(e) == Pose::WalkOff)
            .map(|e| e.id.as_str())
            .collect();
        h.walkoff_fast.retain(|id| done_ids.contains(id.as_str()));
    }
    let scale = if snapshot.sprite_scale <= 0.0 {
        1.0
    } else {
        snapshot.sprite_scale.clamp(0.5, 1.25)
    };
    h.sprite_scale = scale;
    // A drift toggle flip persists live marble x/y at once — the orbit
    // path never dirties, so a toggle would otherwise lose the drifted
    // positions until quit.
    if h.marble_drift != snapshot.marble_drift {
        h.worlds_dirty = true;
    }
    h.marble_drift = snapshot.marble_drift;
    h.planet_names = if snapshot.all_planets.is_empty() {
        snapshot
            .planets
            .iter()
            .map(|p| (p.project_id.clone(), p.name.clone()))
            .collect()
    } else {
        snapshot
            .all_planets
            .iter()
            .map(|p| (p.project_id.clone(), p.name.clone()))
            .collect()
    };
    for planet in h.planets.values_mut() {
        if (planet.sprite_scale - scale).abs() > 0.001 {
            planet.sprite_scale = scale;
            layout_beings(planet);
        }
    }
    // User packs: first sync and every import/remove reloads frame art.
    // A reload re-indexes pack_frames, so Being.pack indices built before
    // it are stale — the pose-only fast path below must not reuse them.
    let packs_reloaded = !h.packs_loaded;
    if !h.packs_loaded {
        h.packs_loaded = true;
        load_pack_frames(&mut h);
    }

    // Tear down panels for projects that left the habitat.
    let incoming: Vec<&str> = snapshot.planets.iter().map(|p| p.project_id.as_str()).collect();
    let stale: Vec<String> = h
        .planets
        .keys()
        .filter(|k| !incoming.contains(&k.as_str()))
        .cloned()
        .collect();
    for project_id in stale {
        if let Some(mut planet) = h.planets.remove(&project_id) {
            audio::stop_audio(&mut planet);
            unsafe {
                let panel = planet.panel as *mut Object;
                let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
                let _: () = msg_send![panel, release];
            }
        }
    }

    let Some(primary_id) = primary_screen_id() else {
        return;
    };

    let (force_marble, mut marble_panels) = sync_budget::apply_budgets(&mut h, snapshot);

    for HabitatPlanet {
        project_id,
        name,
        collapsed,
        entries,
        board_cards,
        todo_trees,
    } in &snapshot.planets
    {
        // Effective collapse: the snapshot's flag, or a cap-3 victim this
        // sync. Collapsed layout stays empty — marbles are bare.
        let collapsed_eff = *collapsed || force_marble.contains(project_id);
        let radius = effective_radius(h.worlds.get(project_id), collapsed_eff);
        let side = (radius + VIEW_PADDING) * 2.0;
        let color = color_for(project_id);

        // CADENCE-FIX v1.1: a CLAIMED window is a walker — open or
        // minimized no longer decides (both full-color on the green).
        // Only a CLOSED claim is asleep (dim + still, sleeper camp).
        // Dots are parked zombies only. Marbles stay bare until expanded.
        let mut sorted = entries.clone();
        sorted.sort_by(|a, b| {
            (a.app_name.to_lowercase() + &a.title.to_lowercase())
                .cmp(&(b.app_name.to_lowercase() + &b.title.to_lowercase()))
        });
        // Zombie aging (TS-7): 14+ days untouched parks the sprite down to
        // marble-scale occupancy; faded ones still walk but draw dimmed.
        // sleepers (closed claimed windows) always
        // walk — asleep and dim — and are never parked into dots or culled
        // for age `gone`; only Done/Unclaim evicts them.
        let (parked, walking): (Vec<&HabitatEntry>, Vec<&HabitatEntry>) = sorted
            .iter()
            .partition(|e| e.age == "park" && !e.asleep);
        let (walker_entries, dot_entries): (Vec<&HabitatEntry>, Vec<&HabitatEntry>) =
            if collapsed_eff {
                (Vec::new(), Vec::new())
            } else {
                (walking, parked)
            };

        // a Done entry that gets no walker to walk off
        // (marble planet, or a parked dot) evicts immediately — fast-
        // forward, never a phantom walk. Walkers holding a Done card walk
        // off visibly; anim_tick emits at the rim. CADENCE-FIX: the fast-
        // forward fires ONCE per Done spell (walkoff_fast), not on every
        // sync while the rail's unclaim round-trips.
        let walking_ids: std::collections::HashSet<&str> =
            walker_entries.iter().map(|e| e.id.as_str()).collect();
        for e in entries.iter() {
            if pose_for(e) == Pose::WalkOff
                && !walking_ids.contains(e.id.as_str())
                && h.walkoff_fast.insert(e.id.clone())
            {
                emit_walked_off(&e.id);
            }
        }

        // CADENCE-FIX pose-only fast path: same walker/dot set, same
        // being-defining fields — only the board card fields moved (a
        // board drag). Skip the Being rebuild entirely: no make_being, no
        // icon re-decode, no pack re-lookup; poses update in place below
        // and roam state (positions) stays.
        let pose_only = !packs_reloaded
            && h.planets.get(project_id).is_some_and(|planet| {
                planet.walker_entries.len() == walker_entries.len()
                    && planet.dot_entries.len() == dot_entries.len()
                    && planet
                        .walker_entries
                        .iter()
                        .zip(walker_entries.iter())
                        .all(|(a, b)| being_fields_eq(a, b))
                    && planet
                        .dot_entries
                        .iter()
                        .zip(dot_entries.iter())
                        .all(|(a, b)| being_fields_eq(a, b))
            });

        // Beings are built before borrowing the planet: make_being needs the
        // shared icon cache.
        let built = if pose_only {
            None
        } else {
            let walkers: Vec<Being> = walker_entries
                .iter()
                .map(|e| make_being(&mut h, e))
                .collect();
            let dots: Vec<Being> = dot_entries.iter().map(|e| make_being(&mut h, e)).collect();
            // Lazy packs (GO-ORBITTY-PLANET-BUDGET): decode a sprite pack only
            // when an EXPANDED planet wears it this sync. Marbles never load a
            // pack — their being lists are empty, so this never fires for them.
            if !collapsed_eff {
                for idx in walkers.iter().filter_map(|b| b.pack) {
                    ensure_pack_loaded(&mut h, idx);
                }
            }
            Some((walkers, dots))
        };

        let decor = h.worlds.get(project_id).cloned();
        if let Some(planet) = h.planets.get_mut(project_id) {
            planet.name = name.clone();
            // WORLD-TOWN: the board blit's ticks ride the snapshot; a
            // board drag changes them without touching any being field,
            // so this dirties the view on its own (pose-only path too).
            let board_changed = planet.board_cards != *board_cards;
            planet.board_cards = board_cards.clone();
            planet.tree_grow_days = snapshot.tree_grow_days;
            todo_tree::sync_chores(planet, project_id, todo_trees);
            planet.todo_trees = todo_trees.clone();
            todo_tree::layout(planet);
            let want_proj = locked_projection(collapsed_eff);
            let proj_changed = planet.projection != want_proj;
            planet.projection = want_proj;
            if let Some(w) = decor {
                // LEFTOVER hitch: layout_decor only when the décor itself
                // moved — a board drag / tick sync re-runs the frame math
                // for nothing otherwise.
                let decor_changed = planet.plants != w.plants
                    || planet.mushrooms != w.mushrooms
                    || planet.buildings != w.buildings
                    || planet.boombox_pos != w.boombox_pos
                    || planet.boombox != w.boombox
                    || planet.beach_ball != w.beach_ball;
                if decor_changed {
                    planet.plants = w.plants;
                    planet.mushrooms = w.mushrooms;
                    planet.buildings = w.buildings;
                    planet.boombox_pos = w.boombox_pos;
                    planet.boombox = w.boombox;
                    // the beach-ball toggle rides the
                    // same mirror; a sync that turns it off hides the live
                    // ball at once.
                    planet.beach_ball = w.beach_ball;
                    if !planet.beach_ball {
                        planet.ball = None;
                    }
                    layout_decor(planet);
                }
            }
            let geometry_changed = planet.radius != radius || planet.is_marble != collapsed_eff;
            if geometry_changed {
                // Marble ↔ planet swap: resize the panel around its center so
                // the world stays where the user put it (keeps stack order).
                unsafe {
                    let panel = planet.panel as *mut Object;
                    let view = planet.view as *mut Object;
                    let frame: NSRect = msg_send![panel, frame];
                    panel_set_frame(
                        panel,
                        NSRect::new(
                            frame.mid_x() - side / 2.0,
                            frame.mid_y() - side / 2.0,
                            side,
                            side,
                        ),
                        true,
                    );
                    let _: () = msg_send![view, setFrame: NSRect::new(0.0, 0.0, side, side)];
                }
                planet.radius = radius;
                planet.is_marble = collapsed_eff;
                // Marble ↔ planet swap drops any fake-Z orbit state; an
                // expanded planet never translates or scales.
                planet.orbit = None;
                planet.marble_scale = 1.0;
            }
            match built {
                // Pose-only fast path: update poses in place (sticky
                // WalkOff), keep every Being and all roam state.
                None => {
                    let mut any_pose = false;
                    for (b, e) in planet.walkers.iter_mut().zip(walker_entries.iter()) {
                        let next = sticky_pose(b.pose, pose_for(e));
                        let glance = droid_glance(e);
                        if next != b.pose {
                            pose_events.push((
                                project_id.clone(),
                                b.entry_id.clone(),
                                b.pose,
                                next,
                            ));
                            b.pose = next;
                            any_pose = true;
                        }
                        if glance != b.droid_glance {
                            b.droid_glance = glance;
                            any_pose = true;
                        }
                    }
                    planet.walker_entries =
                        walker_entries.iter().map(|e| (*e).clone()).collect();
                    planet.dot_entries = dot_entries.iter().map(|e| (*e).clone()).collect();
                    if geometry_changed || proj_changed {
                        layout_beings(planet);
                    }
                    if any_pose || geometry_changed || proj_changed || board_changed {
                        unsafe {
                            let view = planet.view as *mut Object;
                            let _: () = msg_send![view, setNeedsDisplay: true];
                        }
                    }
                }
                Some((mut walkers, dots)) => {
                    // one event per pose transition (first
                    // appearances stay silent — the claim event already exists, and
                    // a restart never replays a backlog into the log).
                    let prev_poses: HashMap<String, Pose> = planet
                        .walkers
                        .iter()
                        .map(|b| (b.entry_id.clone(), b.pose))
                        .collect();
                    // CADENCE-FIX: a walk-off in flight survives a blank
                    // card-status blip — never Roam mid-walk.
                    for b in walkers.iter_mut() {
                        if let Some(&prev) = prev_poses.get(&b.entry_id) {
                            b.pose = sticky_pose(prev, b.pose);
                        }
                    }
                    planet.color = color;
                    planet.walkers = walkers;
                    planet.dots = dots;
                    planet.walker_entries =
                        walker_entries.iter().map(|e| (*e).clone()).collect();
                    planet.dot_entries = dot_entries.iter().map(|e| (*e).clone()).collect();
                    layout_beings(planet);
                    for b in &planet.walkers {
                        if let Some(prev) = prev_poses.get(&b.entry_id) {
                            if *prev != b.pose {
                                pose_events.push((
                                    project_id.clone(),
                                    b.entry_id.clone(),
                                    *prev,
                                    b.pose,
                                ));
                            }
                        }
                    }
                    unsafe {
                        let view = planet.view as *mut Object;
                        let _: () = msg_send![view, setNeedsDisplay: true];
                    }
                }
            }
        } else {
            if collapsed_eff && marble_panels >= MAX_MARBLE_PANELS {
                // 13th+ marble (GO-ORBITTY-PLANET-BUDGET): the project
                // stays on the rail list with NO panel. A slot frees when
                // a marble expands or is unclaimed; that sync's sig change
                // materializes the waitlist in rail order.
                continue;
            }
            if !h.worlds.contains_key(project_id) {
                // Slot index = ring slots already claimed this session;
                // existing worlds are not on the ring and do not occupy.
                let on_screen = match spawn_ring() {
                    Some(ring) => ring_claimed(&ring, &h.worlds),
                    None => h
                        .worlds
                        .values()
                        .filter(|w| w.screen_id == primary_id)
                        .count(),
                };
                h.worlds
                    .insert(project_id.clone(), default_state(&primary_id, on_screen));
            }
            let state = h.worlds[project_id].clone();
            // A planet that didn't exist above can't have taken the
            // pose-only fast path (its gate requires the planet), so the
            // beings are always built here.
            let Some((walkers, dots)) = built else {
                continue;
            };
            let mut planet = Planet {
                panel: 0,
                view: 0,
                name: name.clone(),
                radius,
                sprite_scale: h.sprite_scale,
                is_marble: collapsed_eff,
                color,
                rotation: 0.0,
                projection: locked_projection(collapsed_eff),
                walkers,
                dots,
                caption: None,
                hover_name: false,
                orbit: None,
                marble_scale: 1.0,
                pin_until: None,
                plants: state.plants.clone(),
                mushrooms: state.mushrooms.clone(),
                buildings: state.buildings.clone(),
                boombox_pos: state.boombox_pos,
                boombox: state.boombox,
                beach_ball: state.beach_ball,
                ball: None,
                audio: None,
                audio_volume: 1.0,
                decor_frames: Vec::new(),
                roam: HashMap::new(),
                walker_entries: walker_entries.iter().map(|e| (*e).clone()).collect(),
                dot_entries: dot_entries.iter().map(|e| (*e).clone()).collect(),
                board_cards: board_cards.clone(),
                todo_trees: todo_trees.clone(),
                todo_tree_frames: Vec::new(),
                tree_grow_days: snapshot.tree_grow_days,
                tree_chores: Vec::new(),
            };
            layout_beings(&mut planet);
            todo_tree::layout(&mut planet);
            let Some(cls) = world_view_class() else {
                // Class registration failed — skip this planet and leave the
                // rail (list/focus/stow) fully usable.
                eprintln!("habitat: no world view class; skipping planet {project_id}");
                continue;
            };
            unsafe {
                let view: *mut Object = msg_send![cls, alloc];
                let view: *mut Object =
                    msg_send![view, initWithFrame: NSRect::new(0.0, 0.0, side, side)];
                let panel = make_panel();
                let _: () = msg_send![panel, setContentView: view];
                let _: () = msg_send![view, release]; // panel owns it now
                // Borderless panels never draw a title, but the name shows up
                // in window lists / screen-recording prompts.
                let _: () = msg_send![panel, setTitle: ns_string(&format!("Orbitty — {name}"))];
                panel_set_frame(panel, panel_frame(&state, side), false);
                if h.planets_hidden {
                    let _: () = msg_send![panel, orderOut: std::ptr::null::<Object>()];
                } else {
                    let _: () = msg_send![panel, orderFrontRegardless];
                }
                planet.panel = panel as usize;
                planet.view = view as usize;
            }
            // Boombox persisted on: start playing once the planet exists.
            // A hard-muted save starts nothing — the cursor_tick fader
            // door brings it up on unmute.
            if state.boombox && !state.music.is_empty() {
                let live = if state.boombox_muted {
                    0.0
                } else {
                    audio::live_volume(state.boombox_volume, planet.radius, h.reduce_motion)
                };
                audio::start_audio(&mut planet, &state.music, live);
            }
            h.planets.insert(project_id.clone(), planet);
            // a fresh panel must not
            // debut behind the user's windows — orderFrontRegardless ran
            // above; arm the locate glow so the first blits ring the disc
            // (project_add arms pre-sync too; this is the guarantee).
            h.locate_planet = Some((project_id.clone(), Instant::now()));
            if collapsed_eff {
                marble_panels += 1;
            }
        }
        // heal worlds.json so a leftover "flat"
        // expanded save does not linger. Live disc never reads this.
        if let Some(w) = h.worlds.get_mut(project_id) {
            let want = locked_projection_str(collapsed_eff);
            if w.projection != want {
                w.projection = want.to_string();
                h.worlds_dirty = true;
            }
        }
    }
    // walked-off re-delivery + Reduce Motion
    // fast-forward. anim_tick emits when a Done walker crosses the hide
    // line; this pass re-emits on every sync while one lingers hidden (the
    // rail's handler is idempotent, so a missed first emit heals here).
    // Under Reduce Motion there is no walk to narrate — Done walkers snap
    // past the hide line at once instead of freezing mid-disc.
    let reduce_motion = h.reduce_motion;
    for planet in h.planets.values_mut() {
        if planet.is_marble {
            continue;
        }
        // Under Reduce Motion anim_tick never steps, so a posed walker
        // would freeze wherever it was — snap every station-posed walker
        // to its station here instead. The world stays truthful, just
        // still: positions are the board, motion is what's reduced.
        if reduce_motion {
            let posed: Vec<(String, Pose)> = planet
                .walkers
                .iter()
                .filter(|b| !b.asleep || b.pose == Pose::WalkOff)
                .map(|b| (b.entry_id.clone(), b.pose))
                .collect();
            let mut snapped = false;
            for (id, pose) in posed {
                let Some((tx, ty)) = (match pose {
                    // PROPS leftover (town usable): Stand snaps to the
                    // board's south lip too — never the post foot
                    // (STATION_BOARD sits behind the sign PNG).
                    Pose::Stand => Some(draw::board_near_lip(
                        planet.radius,
                        planet.projection,
                        planet.marble_scale,
                    )),
                    Pose::WalkOff | Pose::Work | Pose::Sit => {
                        Some(station_target(pose, &id))
                    }
                    Pose::Roam => None,
                }) else {
                    continue;
                };
                if let Some(r) = planet.roam.get_mut(&id) {
                    if (r.x - tx).abs() > 1e-9 || (r.y - ty).abs() > 1e-9 {
                        r.x = tx;
                        r.y = ty;
                        r.tx = tx;
                        r.ty = ty;
                        r.pause_end = None;
                        snapped = true;
                    }
                }
            }
            if snapped {
                layout_beings(planet);
                unsafe {
                    let view = planet.view as *mut Object;
                    let _: () = msg_send![view, setNeedsDisplay: true];
                }
            }
        }
        let done_ids: Vec<String> = planet
            .walkers
            .iter()
            .filter(|b| b.pose == Pose::WalkOff)
            .map(|b| b.entry_id.clone())
            .collect();
        for id in done_ids {
            if planet.roam.get(&id).is_some_and(|r| r.x < HOME_HIDE_X) {
                emit_walked_off(&id);
            }
        }
    }
    // Lazy packs (GO-ORBITTY-PLANET-BUDGET): a pack whose last expanded
    // wearer collapsed (or left) drops its decoded bitmaps; the art paths
    // stay so a re-expand re-decodes.
    unload_dead_packs(&mut h);
    // The habitat lock drops BEFORE the event-log appends — jsonl IO runs
    // on a background thread, never on the AppKit thread under the lock.
    drop(h);
    flush_pose_events(pose_events);
}

/// CADENCE-FIX: one event per pose transition, appended off the AppKit
/// thread and off the habitat lock. Best-effort: a missed pose line beats
/// a stalled UI.
pub(crate) fn flush_pose_events(events: Vec<(String, String, Pose, Pose)>) {
    if events.is_empty() {
        return;
    }
    let Some(app) = app_handle() else { return };
    let app = app.clone();
    std::thread::spawn(move || {
        for (project_id, entry_id, from, to) in events {
            crate::event_log::append_best(
                &app,
                "walker-pose",
                None,
                Some(&project_id),
                Some("habitat"),
                true,
                &serde_json::json!({
                    "entryId": entry_id,
                    "from": pose_name(from),
                    "to": pose_name(to),
                }),
            );
        }
    });
}

/// Being-defining fields: everything except the board card fields
/// (card_status / card_kind / needed_state — those only move the pose)
/// and `minimized`, which CADENCE-FIX made pose-irrelevant (a claimed
/// window is a walker either way). The pose-only fast path compares these
/// to skip a full Being rebuild on board drags.
fn being_fields_eq(a: &HabitatEntry, b: &HabitatEntry) -> bool {
    a.id == b.id
        && a.app_name == b.app_name
        && a.title == b.title
        && a.bundle_id == b.bundle_id
        && a.icon_base64 == b.icon_base64
        && a.age == b.age
        && a.sprite_pack_id == b.sprite_pack_id
        && a.char_seed == b.char_seed
        && a.char_look == b.char_look
        && a.sprite_scale == b.sprite_scale
        && a.asleep == b.asleep
        && a.agent_seed == b.agent_seed
}
