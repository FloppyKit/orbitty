//! Planet layout: decor frames, walker/dot frames, Being mint (DIET-HABITAT).
//!
//! Extracted from habitat_mac/mod.rs. Planet/Being stay in mod (private fields).

#![allow(unexpected_cfgs)]

use super::art::{base64_decode, fallback_pack_index, ns_image_from_bytes};
use super::draw;
use super::geom::{disc_to_view, walker_fit};
use super::pose::{droid_glance, hash_str, pose_for, sleeper_target, station_target, yard_pick};
use super::roam::RoamState;
use super::{
    house_spawn, Being, DecorKind, DecorRef, Habitat, HabitatEntry, NSPoint, NSRect, Planet, Pose,
    DOT_SIZE, VIEW_PADDING,
};

pub(crate) fn plant_cap(first_seen: f64, now: f64) -> usize {
    let extra = ((now - first_seen).max(0.0) / 172_800.0).floor() as usize;
    (3 + extra).min(5)
}

pub(crate) fn building_cap(first_seen: f64, now: f64) -> usize {
    let extra = ((now - first_seen).max(0.0) / 259_200.0).floor() as usize;
    (1 + extra).min(3)
}

pub(crate) fn layout_decor(planet: &mut Planet) {
    planet.decor_frames.clear();
    if planet.is_marble {
        return;
    }
    let side = (planet.radius + VIEW_PADDING) * 2.0;
    let center = NSPoint {
        x: side / 2.0,
        y: side / 2.0,
    };
    let size = (planet.radius * 0.22).clamp(14.0, 28.0);
    for (i, d) in planet.plants.iter().enumerate() {
        let p = disc_to_view(center, planet.radius, planet.projection, d.x, d.y);
        planet.decor_frames.push((
            DecorRef {
                kind: DecorKind::Plant,
                index: i,
            },
            NSRect::new(p.x - size / 2.0, p.y - size / 2.0, size, size),
        ));
    }
    for (i, d) in planet.mushrooms.iter().enumerate() {
        let p = disc_to_view(center, planet.radius, planet.projection, d.x, d.y);
        planet.decor_frames.push((
            DecorRef {
                kind: DecorKind::Mushroom,
                index: i,
            },
            NSRect::new(p.x - size / 2.0, p.y - size / 2.0, size, size),
        ));
    }
    for (i, d) in planet.buildings.iter().enumerate() {
        let p = disc_to_view(center, planet.radius, planet.projection, d.x, d.y);
        planet.decor_frames.push((
            DecorRef {
                kind: DecorKind::Building,
                index: i,
            },
            NSRect::new(p.x - size / 2.0, p.y - size / 2.0, size, size),
        ));
    }
    if let Some(b) = planet.boombox_pos {
        let p = disc_to_view(center, planet.radius, planet.projection, b.x, b.y);
        let box_size = (planet.radius * 0.28).clamp(24.0, 32.0);
        planet.decor_frames.push((
            DecorRef {
                kind: DecorKind::Boombox,
                index: 0,
            },
            NSRect::new(p.x - box_size / 2.0, p.y - box_size / 2.0, box_size, box_size),
        ));
    }
}

pub(crate) fn layout_beings(planet: &mut Planet) {
    let side = (planet.radius + VIEW_PADDING) * 2.0;
    let center = NSPoint {
        x: side / 2.0,
        y: side / 2.0,
    };
    let ids: Vec<String> = planet.walkers.iter().map(|b| b.entry_id.clone()).collect();
    let lip = draw::board_near_lip(planet.radius, planet.projection, planet.marble_scale);
    for (i, id) in ids.iter().enumerate() {
        let pose = planet.walkers.get(i).map(|b| b.pose).unwrap_or(Pose::Roam);
        let asleep = planet.walkers.get(i).map(|b| b.asleep).unwrap_or(false);
        // WORLD-PLACES: a fresh board-pose roam starts at the house.
        let house = house_spawn(planet, id);
        planet.roam.entry(id.clone()).or_insert_with(|| {
            // Closed sleeper: park in the camp (north of igloo, west of
            // grass) — never seed on the green. Minimized is not asleep.
            if asleep && pose != Pose::WalkOff {
                let (tx, ty) = sleeper_target(id);
                return RoamState {
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
                };
            }
            // a fresh roam for a board pose seeds
            // AT THE IGLOO (STATION_HOUSE — user buildings stay décor)
            // and walks to the station — the world narrates
            // igloo → board/field instead of popping in at the column.
            // Sit still seeds AT the front rim (bad news skips the
            // queue). A Done walker seeds AT THE IGLOO TOO and walks
            // to the near lip of the board (WORLD-STOP). Live card
            // moves keep the existing roam entry, so those walks stay
            // visible.
            match pose {
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
                    let (tx, ty) = station_target(pose, id);
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
                    // Bare claim: appear ON the grass — never igloo→board lip.
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
                }
            }
        });
    }
    let live: std::collections::HashSet<String> = ids.iter().cloned().collect();
    planet.roam.retain(|k, _| live.contains(k));
    for b in planet.walkers.iter_mut() {
        // Per-claim size (GO-ORBITTY-CHARS) multiplies the gear's global
        // slider; the hit box matches the drawn size.
        let fit = walker_fit(planet.radius, planet.sprite_scale * b.user_scale);
        let (x, y) = planet
            .roam
            .get(&b.entry_id)
            .map(|r| (r.x, r.y))
            .unwrap_or((0.0, 0.0));
        let pt = disc_to_view(center, planet.radius, planet.projection, x, y);
        b.frame = NSRect::new(pt.x - fit / 2.0, pt.y - fit / 2.0, fit, fit);
    }
    let dot_ring = planet.radius * 0.55;
    let dcount = planet.dots.len().max(1) as f64;
    for (i, b) in planet.dots.iter_mut().enumerate() {
        let angle = -std::f64::consts::FRAC_PI_2
            + (i as f64) * (std::f64::consts::TAU / dcount)
            + std::f64::consts::PI / 7.0;
        b.frame = NSRect::new(
            center.x + angle.cos() * dot_ring - DOT_SIZE / 2.0,
            center.y + angle.sin() * dot_ring - DOT_SIZE / 2.0,
            DOT_SIZE,
            DOT_SIZE,
        );
    }
    layout_decor(planet);
}

pub(crate) fn make_being(h: &mut Habitat, entry: &HabitatEntry) -> Being {
    // an agent entry carries a bake seed, not a
    // window — no icon decode, no pack wear; the draw blits the baked
    // droid atlas. Everything else (pose, roam, stations) stays generic.
    let droid = if entry.agent_seed.is_empty() {
        None
    } else {
        Some(hash_str(&entry.agent_seed))
    };
    let mut icon = 0usize;
    if droid.is_none() {
        if let Some(b64) = entry.icon_base64.as_deref() {
            let key = entry
                .bundle_id
                .clone()
                .unwrap_or_else(|| entry.app_name.clone());
            if let Some(&ptr) = h.icon_cache.get(&key) {
                icon = ptr;
            } else if let Some(bytes) = base64_decode(b64) {
                let img = unsafe { ns_image_from_bytes(&bytes) };
                if !img.is_null() {
                    h.icon_cache.insert(key, img as usize);
                    icon = img as usize;
                }
            }
        }
    }
    let mut label = if entry.title.is_empty() {
        entry.app_name.clone()
    } else {
        format!("{} — {}", entry.app_name, entry.title)
    };
    // Captions stay one short line.
    if label.chars().count() > 56 {
        label = label.chars().take(55).collect::<String>() + "…";
    }
    // explicit editor picks win; otherwise the persisted
    // seed derives the same person every launch.
    let look = entry
        .char_look
        .map(|mut l| {
            crate::chars::normalize(&mut l);
            l
        })
        .unwrap_or_else(|| crate::chars::look_from_seed(entry.char_seed));
    // Sprite picker: a user-picked pack wins. Otherwise the Season 1 bitty
    // for this look's Kind+Palette (GO-ORBITTY-S1-BITTYS) — the planet's
    // default population is the blessed bitmap crew, never the glyph grid.
    // A droid never wears a pack — its atlas blits in the draw.
    let pack = if droid.is_some() {
        None
    } else {
        let pack_id = if entry.sprite_pack_id.is_empty() {
            crate::chars::default_pack(&look)
        } else {
            entry.sprite_pack_id.clone()
        };
        h.pack_frames
            .iter()
            .position(|(id, _)| id == &pack_id)
            .or_else(|| fallback_pack_index(h))
    };
    let user_scale = if entry.sprite_scale <= 0.0 {
        1.0
    } else {
        entry.sprite_scale.clamp(0.5, 1.25)
    };
    // Rail Done AX-closes the window, so the walker is asleep during
    // WalkOff. Sleeper dim (0.45) is for a CLOSED window only — minimized
    // stays full-bright on the green. Checkout stays full-bright until
    // they actually leave.
    let pose = pose_for(entry);
    Being {
        entry_id: entry.id.clone(),
        label,
        frame: NSRect::default(),
        bob: 0.0,
        icon,
        look,
        user_scale,
        pack,
        dim: if pose == Pose::WalkOff {
            1.0
        } else if entry.asleep || entry.age == "fade" || entry.age == "gone" {
            0.45
        } else {
            1.0
        },
        asleep: entry.asleep,
        sprite_pack_id: entry.sprite_pack_id.clone(),
        pose,
        droid,
        droid_glance: droid_glance(entry),
    }
}

