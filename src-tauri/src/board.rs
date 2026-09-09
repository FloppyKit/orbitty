//! GO-ORBITTY-MCP-WRITE — board write helpers for the control-plane write
//! tools (`kanban.add` / `kanban.move` / `needed.open`). One mutate path:
//! `load_state` → change cards → `save_state` through the SAME door the
//! Tauri `save_state` command uses (disk-before claim diff + `state:changed`
//! emit), so Launchpad / STARS / planets live-update on an agent write
//! exactly as on a UI write. No claim/unclaim tools here: a done-move keeps
//! the claim so the walk-off still narrates (the rail's habitat:walked-off
//! handler unclaims, same as the checkbox).
//! GO-ORBITTY-PROJECT-ADD adds the planet control plane (`project.add` /
//! `project.rm` / `project.rename`) — same one save door; habitat paints
//! whatever `projects[]` holds, no tick/draw changes here.

use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Emitter;

use crate::event_log;
use crate::state::{KanbanCard, PersistedState, PointsEntry, Project};

/// Same daily points cap as the board (KanbanPage DAILY_CAP).
const DAILY_CAP: u32 = 12;

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

const BASE36: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

fn base36(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(BASE36[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// `<prefix>-<base36 millis>-<6 base36 rand>` — the rail/board id shape
/// (`c-` todos, `n-` needed, `pt-` points rows, `a-` actions).
pub(crate) fn new_id(prefix: &str) -> String {
    use std::io::Read;
    let mut b = [0u8; 8];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut b);
    }
    let mut x = u64::from_le_bytes(b);
    let mut rand = String::with_capacity(6);
    for _ in 0..6 {
        rand.push(BASE36[(x % 36) as usize] as char);
        x /= 36;
    }
    format!("{prefix}-{}-{rand}", base36(now_millis()))
}

/// Local calendar day `YYYY-MM-DD` — the pointsLedger daily-cap key, same
/// as the frontend todayKey().
fn today_key() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// The one save door — same shape as the Tauri `save_state` command:
/// atomic write, claim diff (best-effort), `state:changed` emit.
pub(crate) fn save_with_events(
    app: &tauri::AppHandle,
    before: &PersistedState,
    after: &PersistedState,
) -> Result<(), String> {
    crate::state::save_state(app, after)?;
    event_log::diff_claims(app, &before.claims, &after.claims);
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// A non-empty projectId must name a real project — a typo would orphan
/// the card off every planet and the inbox. "" is the inbox.
fn check_project(state: &PersistedState, project_id: &str) -> Result<(), String> {
    if !project_id.is_empty() && !state.projects.iter().any(|p| p.id == project_id) {
        return Err(format!("unknown project id {project_id:?}"));
    }
    Ok(())
}

/// `kanban.add` — mint `kind: "todo"`, `actor: "agent"`. `status` is
/// `todo` (default) or `doing`; done is never minted (it is earned through
/// the complete door). Logs `board-add`.
pub fn kanban_add(
    app: &tauri::AppHandle,
    title: &str,
    project_id: &str,
    status: &str,
) -> Result<KanbanCard, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("kanban.add: empty title".into());
    }
    let status = match status.trim() {
        "" | "todo" => "todo",
        "doing" => "doing",
        other => {
            return Err(format!(
                "kanban.add: status must be todo|doing, got {other:?} (done is earned via kanban.move)"
            ))
        }
    };
    let before = crate::state::load_state(app)?;
    check_project(&before, project_id)?;
    let now = now_secs();
    let card = KanbanCard {
        id: new_id("c"),
        title: title.to_string(),
        project_id: project_id.to_string(),
        window_id: None,
        kind: Some("todo".into()),
        status: status.into(),
        created_at: now,
        last_touched_at: now,
        completed_at: None,
        goal_aligned: true,
        clinical_or_master: false,
        points_awarded: 0,
        starred: false,
        notes: None,
        difficulty: None,
        subtasks: None,
        due_at: None,
        tag_ids: None,
        needed_state: None,
        needed_why: None,
        needed_answer: None,
        actor: Some("agent".into()),
    };
    let mut after = before.clone();
    after.kanban_cards.push(card.clone());
    save_with_events(app, &before, &after)?;
    event_log::append_best(
        app,
        "board-add",
        None,
        Some(&card.project_id),
        Some("agent"),
        true,
        &serde_json::json!({ "cardId": card.id, "title": card.title }),
    );
    Ok(card)
}

/// `kanban.move` — the board's column rules. `done` is the same complete
/// door as the checkbox (award once per card, 1 pt / 2 clinical|master,
/// daily cap, `completedAt`, actor "agent") and does NOT unclaim — the
/// walk-off still narrates. Moving a window-carded row out of Doing
/// unclaims its window, exactly like the board drag. Same-column = the
/// board's early return (no save, no log). Logs `board-move`.
pub fn kanban_move(app: &tauri::AppHandle, id: &str, target: &str) -> Result<KanbanCard, String> {
    let target = match target.trim() {
        "todo" | "doing" | "done" => target.trim(),
        other => {
            return Err(format!(
                "kanban.move: status must be todo|doing|done, got {other:?}"
            ))
        }
    };
    let before = crate::state::load_state(app)?;
    let idx = before
        .kanban_cards
        .iter()
        .position(|c| c.id == id)
        .ok_or_else(|| format!("kanban.move: unknown card id {id:?}"))?;
    let from = before.kanban_cards[idx].status.clone();
    if from == target {
        return Ok(before.kanban_cards[idx].clone());
    }
    let project_id = before.kanban_cards[idx].project_id.clone();
    let now = now_secs();
    let mut after = before.clone();
    if target == "done" {
        // Complete door — the points math reads the pre-move snapshot.
        let key = today_key();
        let today_pts: u32 = before
            .points_ledger
            .iter()
            .filter(|e| e.day == key)
            .map(|e| e.amount)
            .sum();
        let already = before.points_ledger.iter().any(|e| e.card_id == id);
        let base = if before.kanban_cards[idx].clinical_or_master {
            2
        } else {
            1
        };
        let award = if !already && today_pts + base <= DAILY_CAP {
            base
        } else {
            0
        };
        let c = &mut after.kanban_cards[idx];
        c.status = "done".into();
        c.last_touched_at = now;
        c.completed_at = Some(now);
        c.points_awarded += award;
        c.actor = Some("agent".into());
        if award > 0 {
            after.points_ledger.push(PointsEntry {
                id: new_id("pt"),
                card_id: id.to_string(),
                amount: award,
                day: key,
                at: now,
                actor: Some("agent".into()),
            });
        }
        // No unclaim: the walk-off narrates first; the rail's
        // habitat:walked-off handler unclaims (the existing Done eviction).
    } else {
        let window_id = before.kanban_cards[idx].window_id.clone();
        {
            let c = &mut after.kanban_cards[idx];
            c.status = target.into();
            c.completed_at = None;
            c.last_touched_at = now;
            c.window_id = if target == "doing" {
                window_id.clone()
            } else {
                None
            };
        }
        // Board drag rule: a window-carded row leaving Doing unclaims its
        // window — the board never lies about linkage. diff_claims logs it.
        if from == "doing" {
            if let Some(w) = window_id {
                after.claims.retain(|cl| cl.window_id != w);
            }
        }
    }
    save_with_events(app, &before, &after)?;
    event_log::append_best(
        app,
        "board-move",
        None,
        Some(&project_id),
        Some("agent"),
        true,
        &serde_json::json!({ "from": from, "to": target, "cardId": id }),
    );
    Ok(after.kanban_cards[idx].clone())
}

/// GO-ORBITTY-PROJECT-ADD: `project.add` — mint a planet row
/// `{ id: new_id("p"), name }` (the rail's createProject id shape). Empty
/// name refused; duplicate display names are allowed (ids are unique).
/// Logs `project-add`.
pub fn project_add(app: &tauri::AppHandle, name: &str) -> Result<Project, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("project.add: empty name".into());
    }
    let before = crate::state::load_state(app)?;
    let project = Project {
        id: new_id("p"),
        name: name.to_string(),
    };
    let mut after = before.clone();
    after.projects.push(project.clone());
    save_with_events(app, &before, &after)?;
    event_log::append_best(
        app,
        "project-add",
        None,
        Some(&project.id),
        Some("agent"),
        true,
        &serde_json::json!({ "projectId": project.id, "name": project.name }),
    );
    Ok(project)
}

/// GO-ORBITTY-PROJECT-ADD: `project.rm` — drop the row from `projects`.
/// Cards keep their `projectId`; live claims stay (walkers vanish with
/// the disc on the next habitat sync). Never auto-minted, never
/// auto-killed — ctl is the door. Unknown id = honest error. Logs
/// `project-rm`.
pub fn project_rm(app: &tauri::AppHandle, id: &str) -> Result<Project, String> {
    let before = crate::state::load_state(app)?;
    let idx = before
        .projects
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| format!("project.rm: unknown project id {id:?}"))?;
    let gone = before.projects[idx].clone();
    let mut after = before.clone();
    after.projects.remove(idx);
    save_with_events(app, &before, &after)?;
    event_log::append_best(
        app,
        "project-rm",
        None,
        Some(&gone.id),
        Some("agent"),
        true,
        &serde_json::json!({ "projectId": gone.id, "name": gone.name }),
    );
    Ok(gone)
}

/// GO-ORBITTY-PROJECT-ADD: `project.rename` — change `name`, keep `id`.
/// Empty name = error; unknown id = honest error. Logs `project-rename`.
pub fn project_rename(app: &tauri::AppHandle, id: &str, name: &str) -> Result<Project, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("project.rename: empty name".into());
    }
    let before = crate::state::load_state(app)?;
    let idx = before
        .projects
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| format!("project.rename: unknown project id {id:?}"))?;
    let mut after = before.clone();
    after.projects[idx].name = name.to_string();
    save_with_events(app, &before, &after)?;
    event_log::append_best(
        app,
        "project-rename",
        None,
        Some(id),
        Some("agent"),
        true,
        &serde_json::json!({ "projectId": id, "name": name }),
    );
    Ok(after.projects[idx].clone())
}

/// `needed.open` — mint `kind: "needed"`, `neededState: "open"`,
/// `status: "todo"`, `windowId: null` (the Needed inbox shape). Logs
/// `needed-open`.
pub fn needed_open(
    app: &tauri::AppHandle,
    title: &str,
    project_id: &str,
    why: Option<&str>,
) -> Result<KanbanCard, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("needed.open: empty title".into());
    }
    let before = crate::state::load_state(app)?;
    check_project(&before, project_id)?;
    let now = now_secs();
    let card = KanbanCard {
        id: new_id("n"),
        title: title.to_string(),
        project_id: project_id.to_string(),
        window_id: None,
        kind: Some("needed".into()),
        status: "todo".into(),
        created_at: now,
        last_touched_at: now,
        completed_at: None,
        goal_aligned: false,
        clinical_or_master: false,
        points_awarded: 0,
        starred: false,
        notes: None,
        difficulty: None,
        subtasks: None,
        due_at: None,
        tag_ids: None,
        needed_state: Some("open".into()),
        needed_why: why
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .map(str::to_string),
        needed_answer: None,
        actor: Some("agent".into()),
    };
    let mut after = before.clone();
    after.kanban_cards.push(card.clone());
    save_with_events(app, &before, &after)?;
    event_log::append_best(
        app,
        "needed-open",
        None,
        Some(&card.project_id),
        Some("agent"),
        true,
        &serde_json::json!({ "cardId": card.id, "title": card.title }),
    );
    Ok(card)
}
