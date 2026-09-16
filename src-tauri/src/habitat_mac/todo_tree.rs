//! GO-ORBITTY-TODO-TREES — 8 stills + plant/harvest walks.
//! WORLD-YARD Place-plant glyphs stay in draw.rs.

#![allow(unexpected_cfgs)]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use tauri::Emitter;

use super::{
    app_handle, ns_color, radius_y, set_fill, NSPoint, NSRect, Planet, Pose, STATION_BOARD,
    VIEW_PADDING,
};
use crate::habitat::TodoTreeTick;
use crate::todo_trees::{growth_t, phase};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChoreKind {
    Plant,
    Harvest,
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
        let h = PHASES[ph].len() as f64;
        let w = PHASES[ph][0].len() as f64;
        let scale = (r * 0.045).clamp(6.0, 12.0);
        planet.todo_tree_frames.push((
            t.card_id.clone(),
            NSRect::new(
                cx - w * scale / 2.0,
                base_y - h * scale,
                w * scale,
                h * scale,
            ),
        ));
    }
}

pub(crate) fn hit(planet: &Planet, local: NSPoint) -> Option<String> {
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
        let art = PHASES[ph];
        let scale = (r * 0.045).clamp(6.0, 12.0);
        let cx = center.x + t.x * r;
        let base_y = center.y + t.y * ry;
        blit(cx, base_y, art, scale);
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

fn enqueue(planet: &mut Planet, project_id: &str, t: &TodoTreeTick, kind: ChoreKind) {
    if planet
        .tree_chores
        .iter()
        .any(|c| c.project_id == project_id && c.card_id == t.card_id)
    {
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
    let mut moved = false;
    while i < planet.tree_chores.len() {
        if planet.tree_chores[i].project_id != project_id {
            i += 1;
            continue;
        }
        let (tx, ty) = planet.tree_chores[i].target();
        let wid = planet.tree_chores[i].walker_id.clone();
        let Some(roam) = planet.roam.get_mut(&wid) else {
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
        let step = (0.16 * dt).min(dist);
        roam.x += dx / dist * step;
        roam.y += dy / dist * step;
        roam.facing_left = dx < -0.002;
        moved = true;
        i += 1;
    }
    for id in finished_harvest {
        emit_gone(&id);
    }
    moved
}

pub(crate) fn emit_moved(card_id: &str, x: f64, y: f64) {
    if let Some(app) = app_handle() {
        let _ = app.emit(
            "habitat:todo-tree-move",
            serde_json::json!({ "cardId": card_id, "x": x, "y": y }),
        );
    }
}
