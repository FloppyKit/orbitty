//! GO-ORBITTY-DOING-FIRE — moving a `GO-*` **todo** `todo` → `doing` is
//! the go. The habitat mints (or reuses) the ONE Needed titled exactly
//! `Who runs this doing GO?` with a machine-parseable why —
//! `card=<id> go=<GO-…> planet=<project name or id>` — and Matt teaches
//! the standing answer once (`<harness> on this planet` + a `RULE:`
//! line). After that, OPEN born-answers each new phrase and the bus
//! dispatch runs that harness on that planet. Lock:
//! `~/.buzz/PLANS/ORBITTY_DOING_FIRE_LOCK_v0.md`.
//!
//! Wired from the kanban-move doors only (`board::kanban_move` and the
//! UI's whole-state `save_state` command) as a before/after card diff:
//! `kind = todo`, `todo` → `doing`, title carries the bus's GO token.
//! Neededs, Dogfood cards, fresh `doing` mints, and every other title
//! are a no-op. Best-effort: a fire never fails the save that carried
//! the move. An already-open card with the same title is REUSED with its
//! why replaced (latest drag wins) — never a second walker, never a
//! `companion send`.

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

/// Mint or reuse the one Needed. Reuse = the open same-title card gets
/// its why REPLACED (latest drag wins) through the one save door — no
/// second card, no walker extra. The mint rides `needed.open`, so the
/// standing-rules gate can born-answer it (actor "rule") and the watch
/// dispatches as if Matt typed the answer; a miss / collide pings open.
fn mint_or_reuse(app: &tauri::AppHandle, card: &KanbanCard, go: &str) {
    let Ok(st) = crate::state::load_state(app) else {
        return;
    };
    let why = format!(
        "card={} go={} planet={}",
        card.id,
        go,
        planet_label(&st, &card.project_id)
    );
    let open = st.kanban_cards.iter().find(|c| {
        c.kind.as_deref() == Some("needed")
            && c.needed_state.as_deref() == Some("open")
            && c.title.trim() == DOING_FIRE_TITLE
    });
    let needed_id = if let Some(existing) = open {
        let id = existing.id.clone();
        let mut after = st.clone();
        if let Some(c) = after.kanban_cards.iter_mut().find(|c| c.id == id) {
            c.needed_why = Some(why.clone());
            c.last_touched_at = now_secs();
        }
        if crate::board::save_with_events(app, &st, &after).is_err() {
            return;
        }
        id
    } else {
        match crate::board::needed_open(app, DOING_FIRE_TITLE, &card.project_id, Some(&why)) {
            Ok(c) => c.id,
            Err(_) => return,
        }
    };
    crate::event_log::append_best(
        app,
        "doing-fire",
        None,
        Some(&card.project_id),
        None,
        true,
        &serde_json::json!({ "cardId": card.id, "go": go, "neededId": needed_id }),
    );
}

/// The wire: call with the disk-before snapshot and the just-saved state
/// from each kanban-move door. Best-effort — a fire never fails the save.
pub fn fire_on_moves(app: &tauri::AppHandle, before: &PersistedState, after: &PersistedState) {
    for (card, go) in fired(before, after) {
        mint_or_reuse(app, card, &go);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
