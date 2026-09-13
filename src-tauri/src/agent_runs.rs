//! GO-ORBITTY-DOING-WALKER — the agent-walker join. A `kind=todo` card
//! dragged `todo` → `doing` already launches its worker (doing-fire /
//! agent pickup / `launch_agent` own the processes); this door only
//! ATTACHES the `agentRuns` row the habitat mirrors as the seeded droid
//! walker on that planet — never a second spawn, never a process at all.
//! Locks: `~/.buzz/PLANS/ORBITTY_HERDR_PATH_LOCK_v0.md` (the
//! DOING-WALKER slice) + `~/Downloads/ORBITTY_HERDR_RUNNER_v0.md`.
//!
//! Additive persist on the one state.json (no second schema):
//! todo→doing mints/refreshes `{cardId, planetId, agentShortcutId,
//! state: "working"}`; doing→done marks the row `done` (the far-rim
//! walk-off narrates, then the rail's walked-off door evicts the row);
//! doing→todo (or the card vanishing) drops the row. One row per card —
//! a re-drag upserts, never twins. Best-effort like doing-fire: a failure
//! here never fails the save that carried the move.
//!
//! Wired from the same two move doors as doing-fire (`board::kanban_move`
//! and the UI's whole-state `save_state` command) as a before/after card
//! diff. `state: "blocked"` / `"starting"` are read-supported in the
//! snapshot mapping for the Herdr adapter; this phrase writes neither.

use serde_json::json;

use crate::state::{AgentRun, KanbanCard, PersistedState};

/// The attach gate: a worker door fired for this card — a `GO-*` title
/// (doing-fire) or an assigned agent row (pickup / `launch_agent`). A
/// plain todo dragged to Doing by hand starts nothing, so it gets no
/// walker either.
fn gated(card: &KanbanCard) -> bool {
    crate::needed_rules::extract_go(&card.title).is_some()
        || card.assigned_agent_id.is_some()
}

/// The pure before/after diff: (cards to mint/refresh, card ids to mark
/// done, card ids whose rows to drop). A card born `doing` never fired
/// (the drag is the signal); a vanished card drops its row.
fn transitions(
    before: &PersistedState,
    after: &PersistedState,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut mints = Vec::new();
    let mut dones = Vec::new();
    let mut drops = Vec::new();
    for card in &after.kanban_cards {
        if card.kind.as_deref() != Some("todo") {
            continue;
        }
        let was = before
            .kanban_cards
            .iter()
            .find(|b| b.id == card.id)
            .map(|b| b.status.as_str());
        match (was, card.status.as_str()) {
            (Some(w), "doing") if w != "doing" => {
                if gated(card) {
                    mints.push(card.id.clone());
                }
            }
            (Some("doing"), "done") => dones.push(card.id.clone()),
            (Some(w), "todo") if w != "todo" => drops.push(card.id.clone()),
            _ => {}
        }
    }
    for b in &before.kanban_cards {
        if !after.kanban_cards.iter().any(|c| c.id == b.id) {
            drops.push(b.id.clone());
        }
    }
    (mints, dones, drops)
}

/// The wire: call with the disk-before snapshot and the just-saved state
/// from each kanban-move door, AFTER doing-fire (its Needed mint saves
/// again — this loads the post-fire state). Best-effort — an attach
/// never fails the save that carried the move.
pub fn attach_on_moves(app: &tauri::AppHandle, before: &PersistedState, after: &PersistedState) {
    let (mints, dones, drops) = transitions(before, after);
    if mints.is_empty() && dones.is_empty() && drops.is_empty() {
        return;
    }
    let Ok(mut cur) = crate::state::load_state(app) else {
        return;
    };
    let prev = cur.clone();
    let mut changed = false;
    for card_id in &mints {
        let Some(card) = after.kanban_cards.iter().find(|c| &c.id == card_id) else {
            continue;
        };
        // Upsert — one row per card, never a second walker.
        match cur.agent_runs.iter_mut().find(|r| &r.card_id == card_id) {
            Some(r) => {
                r.planet_id = card.project_id.clone();
                r.agent_shortcut_id =
                    card.assigned_agent_id.clone().unwrap_or_default();
                r.state = "working".into();
            }
            None => cur.agent_runs.push(AgentRun {
                card_id: card.id.clone(),
                planet_id: card.project_id.clone(),
                agent_shortcut_id: card
                    .assigned_agent_id
                    .clone()
                    .unwrap_or_default(),
                state: "working".into(),
            }),
        }
        changed = true;
        crate::event_log::append_best(
            app,
            "agent-run-attach",
            None,
            Some(&card.project_id),
            Some("agent"),
            true,
            &json!({
                "cardId": card.id,
                "agentId": card.assigned_agent_id,
            }),
        );
    }
    for card_id in &dones {
        if let Some(r) = cur.agent_runs.iter_mut().find(|r| &r.card_id == card_id) {
            if r.state != "done" {
                r.state = "done".into();
                changed = true;
                crate::event_log::append_best(
                    app,
                    "agent-run-done",
                    None,
                    Some(&r.planet_id),
                    Some("agent"),
                    true,
                    &json!({ "cardId": card_id }),
                );
            }
        }
    }
    for card_id in &drops {
        let n = cur.agent_runs.len();
        cur.agent_runs.retain(|r| &r.card_id != card_id);
        changed |= cur.agent_runs.len() != n;
    }
    if !changed {
        return;
    }
    // The one save door — claims are untouched, so the diff is silent.
    let _ = crate::board::save_with_events(app, &prev, &cur);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: &str, kind: &str, status: &str, title: &str, agent: Option<&str>) -> KanbanCard {
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
            assigned_agent_id: agent.map(str::to_string),
        }
    }

    #[test]
    fn attach_gate_and_transitions() {
        let mut before = PersistedState::default();
        before.kanban_cards = vec![
            card("c-go", "todo", "todo", "GO-ORBITTY-DOING-WALKER", None),
            card("c-assigned", "todo", "todo", "plain assigned", Some("a-1")),
            card("c-plain", "todo", "todo", "plain todo", None),
            card("c-fin", "todo", "doing", "GO-ORBITTY-FINISH", None),
            card("c-back", "todo", "doing", "GO-ORBITTY-BACK", None),
            card("c-gone", "todo", "doing", "GO-ORBITTY-GONE", None),
            card("c-needed", "needed", "todo", "GO-ORBITTY-NEEDED", None),
            card("c-born", "todo", "todo", "GO-ORBITTY-BORN", None),
        ];
        let mut after = before.clone();
        after.kanban_cards[0].status = "doing".into(); // GO-* drag — mints
        after.kanban_cards[1].status = "doing".into(); // assigned drag — mints
        after.kanban_cards[2].status = "doing".into(); // plain — gated out
        after.kanban_cards[3].status = "done".into(); // doing → done
        after.kanban_cards[4].status = "todo".into(); // doing → todo — drop
        after.kanban_cards.retain(|c| c.id != "c-gone"); // vanished — drop
        // Born doing (not in before as todo): c-born stays todo here; a
        // born-doing card has no todo row in before, so no mint.
        let (mints, dones, drops) = transitions(&before, &after);
        assert_eq!(mints, vec!["c-go".to_string(), "c-assigned".to_string()]);
        assert_eq!(dones, vec!["c-fin".to_string()]);
        assert!(drops.contains(&"c-back".to_string()));
        assert!(drops.contains(&"c-gone".to_string()));
        assert!(!drops.contains(&"c-needed".to_string()));
    }

    #[test]
    fn redoing_remints() {
        let mut before = PersistedState::default();
        before.kanban_cards = vec![card("c-1", "todo", "done", "GO-ORBITTY-X", None)];
        let mut after = before.clone();
        after.kanban_cards[0].status = "doing".into(); // done → doing re-mints
        let (mints, dones, drops) = transitions(&before, &after);
        assert_eq!(mints, vec!["c-1".to_string()]);
        assert!(dones.is_empty());
        assert!(drops.is_empty());
    }
}
