//! 8 stills + plant/harvest walks.
//! Sheet: brand/grove/tree.png (8×128). Ascii PHASES stay as fallback.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::sync::OnceLock;
use tauri::{Emitter, Manager};

use super::{
    app_handle, droid_pack, ns_color, ns_string, radius_y, set_fill, NSPoint, NSRect, Planet, Pose,
    STATION_BOARD, STATION_COMSTACK, STATION_SUNDIAL, VIEW_PADDING,
};
use crate::habitat::TodoTreeTick;
use crate::todo_trees::{growth_t, phase};

const SHEET_FRAME: f64 = 128.0;
const SHEET_COUNT: isize = 8;

/// 8 NES-honest stills. `.` clear; `L` leaf, `S` stem, `T` trunk, `G` canopy, `F` fruit.
const PHASES: [&[&str]; 8] = [
    &["..L..", "..S..", "..S.."],
    &[".L.L.", "..S..", "..S..", ".SSS."],
    &[".LLL.", "L.S.L", "..S..", "..S..", ".SSS."],
    &[
        "..GGG..", "..GGG..", "...T...", "...T...", "..TTT..",
    ],
    &[
        "...GGG...", "..GGGGG..", "...GGG...", "....T....", "....T....", "...TTT...",
    ],
    &[
        "...GGGG...", "..GGGGGG..", ".GGGGGGGG.", "...GGGG...", ".....T....", ".....T....", "....TTT...",
    ],
    &[
        "....GGG....", "...GGGGG...", "..GGGGGGG..", "...GGGGG...", "....GGG....", ".....T.....", ".....T.....", "....TTT....",
    ],
    &[
        "....GFG....", "...GGGGG...", "..GGFGGGG..", "...GGGGG...", "....GGG....", ".....T.....", ".....T.....", "....TTT....",
    ],
];

const GREENS: [(u8, (f64, f64, f64)); 4] = [
    (b'L', (0.32, 0.64, 0.30)),
    (b'S', (0.18, 0.46, 0.22)),
    (b'G', (0.28, 0.62, 0.32)),
    (b'T', (0.36, 0.22, 0.12)),
];
const FRUIT: (u8, (f64, f64, f64)) = (b'F', (0.78, 0.28, 0.22));

fn sheet_frames() -> &'static [usize] {
    static FRAMES: OnceLock<Vec<usize>> = OnceLock::new();
    FRAMES.get_or_init(|| unsafe { load_sheet() })
}

unsafe fn load_sheet() -> Vec<usize> {
    let Some(app) = app_handle() else {
        return Vec::new();
    };
    let Ok(dir) = app.path().resource_dir() else {
        return Vec::new();
    };
    let Ok(bytes) = std::fs::read(dir.join("grove/tree.png")) else {
        return Vec::new();
    };
    let rep = super::bitmap_rep_from_bytes(&bytes);
    if rep.is_null() {
        return Vec::new();
    }
    super::slice_rep_frames(rep, SHEET_FRAME as isize, SHEET_FRAME as isize, SHEET_COUNT)
}

fn dest_rect(cx: f64, base_y: f64, radius: f64, ph: usize) -> NSRect {
    let t = (ph as f64 / 7.0).clamp(0.0, 1.0);
    let s = (radius * 0.82 * (0.38 + 0.62 * t)).clamp(22.0, 68.0);
    NSRect::new(cx - s / 2.0, base_y - s, s, s)
}

/// Paint one growth frame. False = sheet missing, caller may use glyphs.
pub(crate) unsafe fn paint_phase(cx: f64, base_y: f64, radius: f64, ph: usize) -> bool {
    let frames = sheet_frames();
    let Some(&ptr) = frames.get(ph.min(7)) else {
        return false;
    };
    if ptr == 0 {
        return false;
    }
    draw_nearest(ptr as *mut Object, dest_rect(cx, base_y, radius, ph.min(7)));
    true
}

unsafe fn draw_nearest(img: *mut Object, dest: NSRect) {
    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
    let ctx: *mut Object = msg_send![class!(NSGraphicsContext), currentContext];
    if !ctx.is_null() {
        // NSImageInterpolationNone — keep the 8-frame pixel sheet crisp.
        let _: () = msg_send![ctx, setImageInterpolation: 1usize];
    }
    let key = ns_string("NSImageHintInterpolation");
    let val: *mut Object = msg_send![class!(NSNumber), numberWithInteger: 1isize];
    let hints: *mut Object = msg_send![class!(NSDictionary), dictionaryWithObject: val forKey: key];
    let zero = NSRect::default();
    let _: () = msg_send![img,
        drawInRect: dest
        fromRect: zero
        operation: 2usize
        fraction: 1.0
        respectFlipped: true
        hints: hints];
    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChoreKind {
    Plant,
    Harvest,
    Build(crate::starter::Outpost),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Board,
    Spot,
    Finish,
}

#[derive(Clone, Debug)]
pub(crate) struct TreeChore {
    pub project_id: String,
    pub card_id: String,
    pub walker_id: String,
    pub kind: ChoreKind,
    stage: Stage,
    pub spot: (f64, f64),
    house: bool,
}

impl TreeChore {
    fn target(&self) -> (f64, f64) {
        match self.stage {
            Stage::Board => (STATION_BOARD.0, STATION_BOARD.1 + 0.12),
            Stage::Spot | Stage::Finish => self.spot,
        }
    }
}

pub(crate) fn layout(planet: &mut Planet) {
    planet.todo_tree_frames.clear();
    if planet.is_marble {
        return;
    }
    let side = (planet.radius + VIEW_PADDING) * 2.0;
    let center = NSPoint {
        x: side / 2.0,
        y: side / 2.0,
    };
    let r = planet.radius;
    let ry = radius_y(r, planet.projection);
    for t in &planet.todo_trees {
        let cx = center.x + t.x * r;
        let base_y = center.y + t.y * ry;
        let ph = phase(growth_t(
            t.planted_at,
            t.due_at,
            now(),
            planet.tree_grow_days,
        ));
        let frame = if sheet_frames().get(ph).copied().unwrap_or(0) != 0 {
            dest_rect(cx, base_y, r, ph)
        } else {
            let h = PHASES[ph].len() as f64;
            let w = PHASES[ph][0].len() as f64;
            let scale = (r * 0.045).clamp(6.0, 12.0);
            NSRect::new(
                cx - w * scale / 2.0,
                base_y - h * scale,
                w * scale,
                h * scale,
            )
        };
        planet.todo_tree_frames.push((t.card_id.clone(), frame));
    }
}

pub(crate) fn hit(planet: &Planet, local: NSPoint) -> Option<String> {
    // Sign/board owns competing pixels — trees stay hittable off the blit.
    if super::board_geom::hit_board_open(planet, local) {
        return None;
    }
    for (id, frame) in planet.todo_tree_frames.iter().rev() {
        if frame.contains(local) {
            return Some(id.clone());
        }
    }
    None
}

pub(crate) unsafe fn paint_ticks(
    trees: &[TodoTreeTick],
    radius: f64,
    projection: super::Projection,
    grow_days: f64,
    now_epoch: f64,
) {
    let side = (radius + VIEW_PADDING) * 2.0;
    let center = NSPoint {
        x: side / 2.0,
        y: side / 2.0,
    };
    let r = radius;
    let ry = radius_y(r, projection);
    for t in trees {
        if t.status == "harvesting" {
            continue;
        }
        let tnorm = growth_t(t.planted_at, t.due_at, now_epoch, grow_days);
        let ph = phase(tnorm);
        let cx = center.x + t.x * r;
        let base_y = center.y + t.y * ry;
        if !paint_phase(cx, base_y, r, ph) {
            let art = PHASES[ph];
            let scale = (r * 0.045).clamp(6.0, 12.0);
            blit(cx, base_y, art, scale);
        }
    }
}

unsafe fn blit(cx: f64, base_y: f64, art: &[&str], scale: f64) {
    let ox = cx - (art.first().map(|r| r.len()).unwrap_or(0) as f64 * scale) / 2.0;
    let oy = base_y - art.len() as f64 * scale;
    let mut colors = GREENS.to_vec();
    colors.push(FRUIT);
    for (row, line) in art.iter().enumerate() {
        for (col, ch) in line.bytes().enumerate() {
            let Some(&(_, c)) = colors.iter().find(|(k, _)| *k == ch) else {
                continue;
            };
            set_fill(ns_color(c.0, c.1, c.2, 1.0));
            let _: () = msg_send![class!(NSBezierPath),
                fillRect: NSRect::new(
                    ox + col as f64 * scale,
                    oy + row as f64 * scale,
                    scale,
                    scale,
                )];
        }
    }
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Diff snapshot vs last paint: start plant/harvest walks.
pub(crate) fn sync_chores(planet: &mut Planet, project_id: &str, incoming: &[TodoTreeTick]) {
    let old: std::collections::HashSet<String> =
        planet.todo_trees.iter().map(|t| t.card_id.clone()).collect();
    let new: std::collections::HashSet<String> =
        incoming.iter().map(|t| t.card_id.clone()).collect();
    let plant: Vec<TodoTreeTick> = incoming
        .iter()
        .filter(|t| !old.contains(&t.card_id))
        .cloned()
        .collect();
    let harvest: Vec<TodoTreeTick> = planet
        .todo_trees
        .iter()
        .filter(|t| !new.contains(&t.card_id))
        .cloned()
        .collect();
    for t in plant {
        enqueue(planet, project_id, &t, ChoreKind::Plant);
    }
    for t in harvest {
        enqueue(planet, project_id, &t, ChoreKind::Harvest);
    }
}

/// True when this planet already has a chore for `key`.
/// Plant passes the card id. Build passes `outpost:comms` or `outpost:stars`.
pub(crate) fn chore_exists(chores: &[TreeChore], project_id: &str, key: &str) -> bool {
    chores
        .iter()
        .any(|c| c.project_id == project_id && c.card_id == key)
}

/// Walk a builder to the outpost foot. Keyed `outpost:comms|stars`, never
/// the card id, so the card's Plant chore does not swallow it. No awake
/// walker → build at once on a thread (no walk, no panic).
pub(crate) fn enqueue_build(planet: &mut Planet, project_id: &str, kind: crate::starter::Outpost) {
    let key = crate::starter::build_chore_key(kind);
    if chore_exists(&planet.tree_chores, project_id, &key) {
        return;
    }
    let (walker_id, house) = pick_walker(planet, &key);
    if house {
        std::thread::spawn(move || {
            if let Some(app) = app_handle() {
                crate::starter::outpost_built(app, kind);
            }
        });
        return;
    }
    let (sx, sy) = match kind {
        crate::starter::Outpost::Comms => STATION_COMSTACK,
        crate::starter::Outpost::Stars => STATION_SUNDIAL,
    };
    planet.tree_chores.push(TreeChore {
        project_id: project_id.into(),
        card_id: key,
        walker_id,
        kind: ChoreKind::Build(kind),
        stage: Stage::Spot,
        spot: nudge_toward_center(sx, sy, 0.06),
        house: false,
    });
}

fn nudge_toward_center(x: f64, y: f64, dist: f64) -> (f64, f64) {
    let len = (x * x + y * y).sqrt();
    if len <= dist || len < 1e-9 {
        return (0.0, 0.0);
    }
    let scale = (len - dist) / len;
    (x * scale, y * scale)
}

fn enqueue(planet: &mut Planet, project_id: &str, t: &TodoTreeTick, kind: ChoreKind) {
    if chore_exists(&planet.tree_chores, project_id, &t.card_id) {
        return;
    }
    let (walker_id, house) = pick_walker(planet, &t.card_id);
    if house {
        if kind == ChoreKind::Harvest {
            emit_gone(&t.card_id);
        }
        return;
    }
    planet.tree_chores.push(TreeChore {
        project_id: project_id.into(),
        card_id: t.card_id.clone(),
        walker_id,
        kind,
        stage: Stage::Board,
        spot: (t.x, t.y),
        house,
    });
}

fn emit_gone(card_id: &str) {
    if let Some(app) = app_handle() {
        let _ = app.emit(
            "habitat:todo-tree-gone",
            serde_json::json!({ "cardId": card_id }),
        );
    }
}

fn pick_walker(planet: &Planet, card_id: &str) -> (String, bool) {
    if let Some(b) = planet
        .walkers
        .iter()
        .find(|b| b.pose != Pose::WalkOff && !b.asleep)
    {
        return (b.entry_id.clone(), false);
    }
    (format!("chore:{card_id}"), true)
}

pub(crate) fn tick(planet: &mut Planet, dt: f64, project_id: &str) -> bool {
    let mut i = 0;
    let mut finished_harvest: Vec<String> = Vec::new();
    let mut finished_build: Vec<crate::starter::Outpost> = Vec::new();
    let mut moved = false;
    let build_ids: Vec<String> = planet
        .tree_chores
        .iter()
        .filter(|c| matches!(c.kind, ChoreKind::Build(_)))
        .map(|c| c.walker_id.clone())
        .collect();
    let discs = super::walk_around::tree_discs(
        &planet.todo_trees.iter().map(|t| (t.x, t.y)).collect::<Vec<_>>(),
    );
    while i < planet.tree_chores.len() {
        if planet.tree_chores[i].project_id != project_id {
            i += 1;
            continue;
        }
        let is_build = matches!(planet.tree_chores[i].kind, ChoreKind::Build(_));
        if !is_build && build_ids.iter().any(|id| id == &planet.tree_chores[i].walker_id) {
            i += 1;
            continue;
        }
        let (tx, ty) = planet.tree_chores[i].target();
        let wid = planet.tree_chores[i].walker_id.clone();
        let Some(roam) = planet.roam.get_mut(&wid) else {
            // A vanished roam used to skip this chore forever. Build still
            // finishes (same as the no-walker house path). Plant/harvest stay.
            if let ChoreKind::Build(kind) = planet.tree_chores[i].kind {
                finished_build.push(kind);
                planet.tree_chores.remove(i);
                continue;
            }
            i += 1;
            continue;
        };
        let dx = tx - roam.x;
        let dy = ty - roam.y;
        let dist = (dx * dx + dy * dy).sqrt();
        roam.tx = tx;
        roam.ty = ty;
        if dist < 0.04 {
            match planet.tree_chores[i].stage {
                Stage::Board => planet.tree_chores[i].stage = Stage::Spot,
                Stage::Spot => {
                    if planet.tree_chores[i].kind == ChoreKind::Harvest {
                        finished_harvest.push(planet.tree_chores[i].card_id.clone());
                    }
                    if let ChoreKind::Build(kind) = planet.tree_chores[i].kind {
                        finished_build.push(kind);
                    }
                    planet.tree_chores.remove(i);
                    continue;
                }
                Stage::Finish => {
                    planet.tree_chores.remove(i);
                    continue;
                }
            }
            i += 1;
            continue;
        }
        let aim = super::walk_around::skirt_aim((roam.x, roam.y), (tx, ty), &discs);
        let adx = aim.0 - roam.x;
        let ady = aim.1 - roam.y;
        let adist = (adx * adx + ady * ady).sqrt();
        let (sx, sy, slen) = if adist < 1e-6 {
            (0.0, 0.02, 0.02)
        } else {
            (adx, ady, adist)
        };
        let step = (0.16 * dt).min(slen);
        roam.x += sx / slen * step;
        roam.y += sy / slen * step;
        roam.facing_left = sx < -0.002;
        roam.facing_dir = droid_pack::dir_from_delta(sx, sy, roam.facing_dir);
        moved = true;
        i += 1;
    }
    for id in finished_harvest {
        emit_gone(&id);
    }
    for kind in finished_build {
        std::thread::spawn(move || {
            if let Some(app) = app_handle() {
                crate::starter::outpost_built(app, kind);
            }
        });
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::super::{Being, Projection};
    use super::{enqueue, enqueue_build, ChoreKind, Planet, Pose};
    use crate::chars::CharLook;
    use crate::habitat::TodoTreeTick;
    use crate::starter::{Outpost, ID_COMMS};

    fn awake_planet() -> Planet {
        let walker = Being {
            entry_id: "w".into(),
            label: "w".into(),
            frame: super::NSRect::default(),
            bob: 0.0,
            icon: 0,
            look: CharLook::default(),
            user_scale: 1.0,
            pack: None,
            dim: 1.0,
            asleep: false,
            sprite_pack_id: String::new(),
            pose: Pose::Stand,
            droid: None,
            droid_glance: 0,
        };
        Planet {
            panel: 0,
            view: 0,
            name: "p".into(),
            radius: 80.0,
            sprite_scale: 1.0,
            is_marble: false,
            color: (0.2, 0.5, 0.3),
            rotation: 0.0,
            projection: Projection::default(),
            walkers: vec![walker],
            dots: Vec::new(),
            caption: None,
            hover_name: false,
            orbit: None,
            marble_scale: 1.0,
            pin_until: None,
            plants: Vec::new(),
            mushrooms: Vec::new(),
            buildings: Vec::new(),
            boombox_pos: None,
            boombox: false,
            beach_ball: false,
            ball: None,
            audio: None,
            audio_volume: 1.0,
            decor_frames: Vec::new(),
            roam: std::collections::HashMap::new(),
            walker_entries: Vec::new(),
            dot_entries: Vec::new(),
            board_cards: Vec::new(),
            todo_trees: Vec::new(),
            todo_tree_frames: Vec::new(),
            tree_grow_days: 14.0,
            tree_chores: Vec::new(),
        }
    }

    #[test]
    fn s9_enqueue_build_keeps_plant_and_build_and_dedups() {
        let mut planet = awake_planet();
        let plant = TodoTreeTick {
            card_id: ID_COMMS.into(),
            x: 0.2,
            y: 0.1,
            planted_at: 1.0,
            due_at: None,
            status: "todo".into(),
        };
        enqueue(&mut planet, "p", &plant, ChoreKind::Plant);
        enqueue_build(&mut planet, "p", Outpost::Comms);
        assert_eq!(planet.tree_chores.len(), 2);
        assert!(planet.tree_chores.iter().any(|c| {
            c.card_id == ID_COMMS && c.kind == ChoreKind::Plant && c.walker_id == "w"
        }));
        assert!(planet.tree_chores.iter().any(|c| {
            c.card_id == "outpost:comms"
                && c.kind == ChoreKind::Build(Outpost::Comms)
                && c.walker_id == "w"
        }));
        enqueue_build(&mut planet, "p", Outpost::Comms);
        assert_eq!(planet.tree_chores.len(), 2);
    }

    #[test]
    fn build_chore_without_roam_finishes() {
        let mut planet = awake_planet();
        enqueue_build(&mut planet, "p", Outpost::Comms);
        assert_eq!(planet.tree_chores.len(), 1);
        planet.roam.clear();
        super::tick(&mut planet, 0.1, "p");
        assert!(planet.tree_chores.is_empty());

        let mut veteran = awake_planet();
        let plant = TodoTreeTick {
            card_id: "c-old".into(),
            x: 0.1,
            y: 0.1,
            planted_at: 1.0,
            due_at: None,
            status: "todo".into(),
        };
        enqueue(&mut veteran, "p", &plant, ChoreKind::Plant);
        veteran.roam.clear();
        super::tick(&mut veteran, 0.1, "p");
        assert_eq!(veteran.tree_chores.len(), 1);
    }

    #[test]
    fn blocked_straight_line_reaches_comms() {
        let mut planet = awake_planet();
        planet.todo_trees.push(TodoTreeTick {
            card_id: "c-block".into(),
            x: 0.40,
            y: 0.08,
            planted_at: 1.0,
            due_at: None,
            status: "todo".into(),
        });
        planet.roam.insert(
            "w".into(),
            super::super::roam::RoamState {
                x: 0.0,
                y: 0.05,
                tx: 0.0,
                ty: 0.05,
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
        );
        let plant = TodoTreeTick {
            card_id: ID_COMMS.into(),
            x: 0.40,
            y: 0.08,
            planted_at: 1.0,
            due_at: None,
            status: "todo".into(),
        };
        enqueue(&mut planet, "p", &plant, ChoreKind::Plant);
        enqueue_build(&mut planet, "p", Outpost::Comms);
        for _ in 0..80 {
            super::tick(&mut planet, 1.0, "p");
        }
        assert!(
            planet.tree_chores.iter().all(|c| !matches!(c.kind, ChoreKind::Build(_))),
            "build chore still open"
        );
    }
}

pub(crate) fn emit_moved(card_id: &str, x: f64, y: f64) {
    if let Some(app) = app_handle() {
        let _ = app.emit(
            "habitat:todo-tree-move",
            serde_json::json!({ "cardId": card_id, "x": x, "y": y }),
        );
    }
}
