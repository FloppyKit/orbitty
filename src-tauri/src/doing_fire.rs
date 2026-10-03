//! `GO-*` todo→doing mints/reuses `Who runs this doing GO?`.
//! Neededs and Dogfood titles are a no-op. Never companion send.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::state::{KanbanCard, PersistedState};

/// The one fact title. The standing rule matches every phrase of it, so
/// the specific GO lives in the why, never in the title.
pub const DOING_FIRE_TITLE: &str = "Who runs this doing GO?";

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// `planet=<name or id>` for the why line — the display name when the
/// card's projectId resolves, else the raw id, else the inbox. The why
/// stays one-token-per-key (machine-parseable), so a name with whitespace
/// falls back to the id.
fn planet_label(state: &PersistedState, project_id: &str) -> String {
    if project_id.is_empty() {
        return "inbox".into();
    }
    state
        .projects
        .iter()
        .find(|p| p.id == project_id)
        .map(|p| p.name.trim().to_string())
        .filter(|n| !n.is_empty() && !n.chars().any(char::is_whitespace))
        .unwrap_or_else(|| project_id.to_string())
}

/// The fire rule: `kind=todo`, was `todo` in the before snapshot, now
/// `doing`, and the title carries the bus's GO token
/// (`needed_rules::extract_go` — the same regex). A card born `doing`
/// never fired; Neededs and Dogfood cards never mint this question.
fn fired<'a>(before: &PersistedState, after: &'a PersistedState) -> Vec<(&'a KanbanCard, String)> {
    let mut out = Vec::new();
    for card in &after.kanban_cards {
        if card.kind.as_deref() != Some("todo") || card.status != "doing" {
            continue;
        }
        let was_todo = before
            .kanban_cards
            .iter()
            .any(|b| b.id == card.id && b.status == "todo");
        if !was_todo {
            continue;
        }
        if card.title.starts_with("Dogfood ") {
            continue;
        }
        let Some(go) = crate::needed_rules::extract_go(&card.title) else {
            continue;
        };
        out.push((card, go));
    }
    out
}

/// What a todo→doing GO drag should do. Pure so the tests don't need a socket.
pub(crate) enum Drag {
    /// One row. Launch it. Do not ask who, and do not mark the card done.
    Launch { agent_id: String },
    /// Stay in Doing. One Needed, one sentence.
    Block { why: String },
}

/// Assigned id wins. Else the saved default runner (a fresh install has
/// none — the user chooses). Two name matches block. Never a baked-in kitchen.
pub(crate) fn drag_decision(
    assigned_agent_id: Option<&str>,
    agents: &[(String, String, String)],
    chosen: Option<&crate::plans::RunnerSpec>,
) -> Drag {
    let id = assigned_agent_id.map(str::trim).filter(|s| !s.is_empty());
    if let Some(id) = id {
        let n = agents.iter().filter(|(aid, _, _)| aid == id).count();
        return if n == 1 {
            Drag::Launch {
                agent_id: id.to_string(),
            }
        } else if n == 0 {
            Drag::Block {
                why: "Assigned agent is gone. Pick another row.".into(),
            }
        } else {
            Drag::Block {
                why: "Two agent rows match this assignment.".into(),
            }
        };
    }
    let Some(spec) = chosen else {
        return Drag::Block {
            why: "Choose a default runner in Settings. A new install does not ship one."
                .into(),
        };
    };
    let name = spec.runner.trim();
    let harness = spec.harness.trim().to_ascii_lowercase();
    let by_name: Vec<_> = agents
        .iter()
        .filter(|(_, n, _)| !name.is_empty() && n.eq_ignore_ascii_case(name))
        .map(|(id, _, _)| id.clone())
        .collect();
    if by_name.len() > 1 {
        return Drag::Block {
            why: format!("Two agent rows match {name}."),
        };
    }
    if let Some(id) = by_name.into_iter().next() {
        return Drag::Launch { agent_id: id };
    }
    let by_harness: Vec<_> = agents
        .iter()
        .filter(|(_, _, h)| !harness.is_empty() && h.eq_ignore_ascii_case(&harness))
        .map(|(id, _, _)| id.clone())
        .collect();
    if by_harness.len() > 1 {
        return Drag::Block {
            why: format!("Two agent rows use harness {harness}. Name the default runner."),
        };
    }
    if let Some(id) = by_harness.into_iter().next() {
        return Drag::Launch { agent_id: id };
    }
    Drag::Block {
        why: format!(
            "No agent row for runner {name} ({harness}). Add one, or change the default."
        ),
    }
}

fn block(app: &tauri::AppHandle, card: &KanbanCard, why: &str) {
    let _ = crate::board::kanban_note_append(app, &card.id, why);
    let _ = crate::board::needed_open(app, DOING_FIRE_TITLE, &card.project_id, Some(why));
}

fn launch_assigned(app: &tauri::AppHandle, card: &KanbanCard, go: &str) {
    let urls: Vec<String> = card
        .links
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|l| l.url.clone())
        .collect();
    let gate = crate::plans::gate_for_card(&card.id, &card.title, &urls);
    if let crate::plans::Gate::Refuse { note } = &gate {
        let note = note.clone();
        block(app, card, &note);
        crate::event_log::append_best(
            app,
            "go-launch",
            None,
            Some(&card.project_id),
            Some("agent"),
            true,
            &serde_json::json!({
                "id": go,
                "cardId": card.id,
                "route": "doing",
                "launched": false,
                "note": note,
            }),
        );
        return;
    }
    if let Err(why) = crate::gate_caps::check(app) {
        let _ = crate::board::kanban_note_append(app, &card.id, &why);
        return;
    }
    let Ok(st) = crate::state::load_state(app) else {
        return;
    };
    let rows: Vec<(String, String, String)> = st
        .agent_shortcuts
        .iter()
        .map(|a| (a.id.clone(), a.name.clone(), a.harness.clone()))
        .collect();
    let chosen = crate::plans::chosen_runner(&crate::plans::Roots::real());
    match drag_decision(card.assigned_agent_id.as_deref(), &rows, chosen.as_ref()) {
        Drag::Block { why } => block(app, card, &why),
        Drag::Launch { agent_id } => {
            let Some(mut agent) = st
                .agent_shortcuts
                .iter()
                .find(|a| a.id == agent_id)
                .cloned()
            else {
                block(app, card, "Assigned agent is gone. Pick another row.");
                return;
            };
            if let Some(spec) = chosen.as_ref() {
                if agent.model.as_deref().map(str::trim).unwrap_or("").is_empty()
                    && !spec.model.trim().is_empty()
                {
                    agent.model = Some(spec.model.trim().to_string());
                }
                if let Some(p) = spec.permissions.as_deref().map(str::trim).filter(|s| !s.is_empty())
                {
                    agent.permissions = p.to_string();
                }
            }
            let project_name = st
                .projects
                .iter()
                .find(|p| p.id == card.project_id)
                .map(|p| p.name.clone());
            let handoff = match gate {
                crate::plans::Gate::Launch(parsed) => {
                    let project = project_name.clone().unwrap_or_default();
                    let run = crate::plans::resolve_run(&parsed.header, &parsed.id(), &project);
                    Some(crate::coordinator::GoLaunch {
                        parsed,
                        run,
                        route: "doing",
                        card_id: card.id.clone(),
                        project_id: card.project_id.clone(),
                    })
                }
                _ => None,
            };
            let _ = crate::coordinator::set_companion_hack(
                app.clone(),
                true,
                Some(agent.harness.clone()),
                agent.project_id.clone(),
            );
            if let Err(e) = crate::coordinator::launch_agent(
                app.clone(),
                agent.clone(),
                project_name,
                vec![card.title.clone()],
                handoff,
            ) {
                let _ = crate::coordinator::set_companion_hack(app.clone(), false, None, None);
                block(app, card, &e);
                return;
            }
            crate::event_log::append_best(
                app,
                "doing-assign",
                None,
                Some(&card.project_id),
                Some("agent"),
                true,
                &serde_json::json!({
                    "cardId": card.id,
                    "go": go,
                    "agentId": agent.id,
                    "harness": agent.harness,
                }),
            );
        }
    }
}

/// The wire: call with the disk-before snapshot and the just-saved state
/// from each kanban-move door. Best-effort — a fire never fails the save.
pub fn fire_on_moves(app: &tauri::AppHandle, before: &PersistedState, after: &PersistedState) {
    for (card, go) in fired(before, after) {
        launch_assigned(app, card, &go);
    }
}

/// Doing-fire + walker attach after the column save has returned.
/// Needed mint and cursor-go stay the same; they must not hitch the drag.
pub fn spawn_fire_and_attach(app: &tauri::AppHandle, before: PersistedState, after: PersistedState) {
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("orbitty-doing-fx".into())
        .spawn(move || {
            fire_on_moves(&app, &before, &after);
            #[cfg(unix)]
            crate::agent_runs::attach_on_moves(&app, &before, &after);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wiring guard: a doing GO goes through the seal gate before the
    /// "who runs this" question is minted; a refusal appends a note and
    /// returns.
    #[test]
    fn doing_fire_runs_the_go_gate_before_launch() {
        let src = include_str!("doing_fire.rs");
        let start = src.find("fn launch_assigned(").unwrap();
        let body = &src[start..];
        let gate = body
            .find("crate::plans::gate_for_card(")
            .expect("doing drag must call the GO gate");
        let launch = body.find("launch_agent(").expect("launch path");
        assert!(gate < launch, "the gate must run before launch");
        assert!(body.contains("Gate::Refuse"), "{body}");
        assert!(body.contains("return;"), "{body}");
    }

    #[test]
    fn unassigned_without_a_saved_default_blocks() {
        match drag_decision(None, &[], None) {
            Drag::Block { why } => assert!(why.contains("Choose a default runner"), "{why}"),
            Drag::Launch { .. } => panic!("must not launch"),
        }
    }

    #[test]
    fn assigned_row_launches_and_two_names_block() {
        let rows = vec![
            ("a-pi".into(), "pi".into(), "pi".into()),
            ("a-2".into(), "pi".into(), "cursor".into()),
        ];
        match drag_decision(Some("a-pi"), &rows, None) {
            Drag::Launch { agent_id } => assert_eq!(agent_id, "a-pi"),
            Drag::Block { why } => panic!("{why}"),
        }
        let spec = crate::plans::RunnerSpec {
            runner: "pi".into(),
            harness: "pi".into(),
            model: String::new(),
            runner_fallback: None,
            permissions: None,
        };
        match drag_decision(None, &rows, Some(&spec)) {
            Drag::Block { why } => assert!(why.contains("Two agent rows"), "{why}"),
            Drag::Launch { .. } => panic!("ambiguous name must block"),
        }
    }

    fn card(id: &str, kind: &str, status: &str, title: &str) -> KanbanCard {
        KanbanCard {
            id: id.into(),
            title: title.into(),
            project_id: "p-1".into(),
            window_id: None,
            kind: Some(kind.into()),
            status: status.into(),
            created_at: 0.0,
            last_touched_at: 0.0,
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
            actor: None,
            assigned_agent_id: None,
            parent_id: None,
            rating: None,
            trackers: None,
            task_id: None,
            task_reason: None,
            links: None,
        }
    }

    #[test]
    fn fire_rule() {
        let mut before = PersistedState::default();
        before.kanban_cards = vec![
            card("c-1", "todo", "todo", "GO-ORBITTY-RAIL-CLOSE"),
            card("c-2", "todo", "doing", "GO-ORBITTY-BORN-DOING"),
            card("c-3", "needed", "todo", "GO-ORBITTY-NEEDED"),
            card("c-4", "todo", "todo", "Dogfood GO-ORBITTY-X?"),
            card("c-5", "todo", "todo", "plain todo"),
            card("c-6", "todo", "doing", "GO-ORBITTY-STAYS-DOING"),
        ];
        let mut after = before.clone();
        after.kanban_cards[0].status = "doing".into(); // the drag — fires
        after.kanban_cards[3].status = "doing".into(); // Dogfood — no-op
        after.kanban_cards[4].status = "doing".into(); // no GO token — no-op
        let hits = fired(&before, &after);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0.id, "c-1");
        assert_eq!(hits[0].1, "GO-ORBITTY-RAIL-CLOSE");
    }
}
