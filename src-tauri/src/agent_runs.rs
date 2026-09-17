//! Attach `agentRuns` for the habitat droid. Never a second spawn.
//! One row per card. A save must not strip a live Doing run.

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

/// Whole-state save carry: a done card must not leave its run `working`.
/// The walked-off door still drops the row (incoming omitted it); this
/// only rewrites a row that arrived with the save.
pub fn reconcile_incoming(incoming: &mut PersistedState) {
    for card in &incoming.kanban_cards {
        if card.kind.as_deref() != Some("todo") || card.status != "done" {
            continue;
        }
        if let Some(r) = incoming
            .agent_runs
            .iter_mut()
            .find(|r| r.card_id == card.id)
        {
            if r.state != "done" {
                r.state = "done".into();
            }
        }
    }
    incoming.agent_runs.retain(|r| {
        incoming.kanban_cards.iter().any(|c| {
            c.id == r.card_id
                && c.kind.as_deref() == Some("todo")
                && (c.status == "doing" || c.status == "done")
        })
    });
}

/// The strip-trap heal: rows the incoming save DROPPED without moving
/// the card. The card still reads Doing, so the run is still live —
/// restore the row (the wire rebuilds planet/agent from the card's
/// current fields; the row's own state survives). Legit exits never
/// heal: doing→todo and the vanished card fail the still-doing gate,
/// and the rail's walked-off door evicts only a Done card's row.
fn stripped(before: &PersistedState, after: &PersistedState) -> Vec<AgentRun> {
    before
        .agent_runs
        .iter()
        .filter(|r| !after.agent_runs.iter().any(|a| a.card_id == r.card_id))
        .filter(|r| {
            after
                .kanban_cards
                .iter()
                .any(|c| c.id == r.card_id && c.status == "doing")
        })
        .cloned()
        .collect()
}

/// The wire: call with the disk-before snapshot and the just-saved state
/// from each kanban-move door, AFTER doing-fire (its Needed mint saves
/// again — this loads the post-fire state, and the heal re-carries any
/// row that mint's pre-attach snapshot dropped). Best-effort — an attach
/// never fails the save that carried the move.
pub fn attach_on_moves(app: &tauri::AppHandle, before: &PersistedState, after: &PersistedState) {
    let (mints, dones, drops) = transitions(before, after);
    let heals = stripped(before, after);
    if !drops.is_empty() {
        crate::agent_stop::stop_resets(app, before, &drops);
    }
    if mints.is_empty() && dones.is_empty() && drops.is_empty() && heals.is_empty() {
        return;
    }
    let Ok(mut cur) = crate::state::load_state(app) else {
        return;
    };
    let prev = cur.clone();
    let mut changed = false;
    // Events collect through the mutation and fire only after the save
    // lands — `agent-run-attach` means the row is on disk, never before.
    let mut attached: Vec<&KanbanCard> = Vec::new();
    let mut went_done: Vec<(String, String)> = Vec::new();
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
                herdr: None,
            }),
        }
        changed = true;
        attached.push(card);
    }
    for card_id in &dones {
        if let Some(r) = cur.agent_runs.iter_mut().find(|r| &r.card_id == card_id) {
            if r.state != "done" {
                r.state = "done".into();
                went_done.push((card_id.clone(), r.planet_id.clone()));
                changed = true;
            }
        }
    }
    for card_id in &drops {
        let n = cur.agent_runs.len();
        cur.agent_runs.retain(|r| &r.card_id != card_id);
        changed |= cur.agent_runs.len() != n;
    }
    for row in &heals {
        // A mint this pass already rewrote the row — never twins.
        if cur.agent_runs.iter().any(|r| r.card_id == row.card_id) {
            continue;
        }
        let Some(card) = after.kanban_cards.iter().find(|c| c.id == row.card_id) else {
            continue;
        };
        cur.agent_runs.push(AgentRun {
            card_id: row.card_id.clone(),
            planet_id: card.project_id.clone(),
            agent_shortcut_id: card
                .assigned_agent_id
                .clone()
                .unwrap_or_default(),
            state: row.state.clone(),
            herdr: row.herdr.clone(),
        });
        changed = true;
        attached.push(card);
    }
    if !changed {
        return;
    }
    // The one save door — claims are untouched, so the diff is silent.
    if crate::board::save_with_events(app, &prev, &cur).is_err() {
        return;
    }
    if !drops.is_empty() {
        crate::coordinator::heal_stuck_hack(app);
    }
    for card in attached {
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
    for (card_id, planet_id) in went_done {
        crate::event_log::append_best(
            app,
            "agent-run-done",
            None,
            Some(&planet_id),
            Some("agent"),
            true,
            &json!({ "cardId": card_id }),
        );
    }
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
            parent_id: None,
            rating: None,
            trackers: None,
            task_id: None,
            task_reason: None,
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

    fn run(card_id: &str, state: &str) -> AgentRun {
        AgentRun {
            card_id: card_id.into(),
            planet_id: "p-1".into(),
            agent_shortcut_id: String::new(),
            state: state.into(),
            herdr: None,
        }
    }

    #[test]
    fn strip_heal_restores_live_run() {
        // A stale whole-state persist drops the row; the card never left
        // Doing, so the run is still live and heals.
        let mut before = PersistedState::default();
        before.kanban_cards = vec![card("c-run", "todo", "doing", "GO-ORBITTY-X", None)];
        before.agent_runs = vec![run("c-run", "working")];
        let mut after = before.clone();
        after.agent_runs = vec![]; // the strip
        let heals = stripped(&before, &after);
        assert_eq!(heals.len(), 1);
        assert_eq!(heals[0].card_id, "c-run");
        assert_eq!(heals[0].state, "working");
        // A save that CARRIES the row heals nothing.
        let after = before.clone();
        assert!(stripped(&before, &after).is_empty());
    }

    #[test]
    fn strip_heal_never_fights_legit_exits() {
        let mut before = PersistedState::default();
        before.kanban_cards = vec![
            card("c-todo", "todo", "doing", "GO-ORBITTY-A", None),
            card("c-done", "todo", "doing", "GO-ORBITTY-B", None),
            card("c-gone", "todo", "doing", "GO-ORBITTY-C", None),
        ];
        before.agent_runs = vec![
            run("c-todo", "working"),
            run("c-done", "done"),
            run("c-gone", "working"),
        ];
        let mut after = before.clone();
        after.agent_runs = vec![];
        after.kanban_cards[0].status = "todo".into(); // doing→todo drop
        after.kanban_cards[1].status = "done".into(); // walked-off eviction
        after.kanban_cards.retain(|c| c.id != "c-gone"); // card vanished
        assert!(stripped(&before, &after).is_empty());
    }

    #[test]
    fn reconcile_forces_done_keeps_omitted() {
        let mut st = PersistedState::default();
        st.kanban_cards = vec![card("c-1", "todo", "done", "GO-ORBITTY-X", Some("a"))];
        st.agent_runs = vec![run("c-1", "working")];
        reconcile_incoming(&mut st);
        assert_eq!(st.agent_runs[0].state, "done");
        st.agent_runs.clear();
        reconcile_incoming(&mut st);
        assert!(st.agent_runs.is_empty());
    }

    #[test]
    fn reconcile_drops_working_row_on_todo() {
        let mut st = PersistedState::default();
        st.kanban_cards = vec![card("c-1", "todo", "todo", "GO-ORBITTY-X", Some("a"))];
        st.agent_runs = vec![run("c-1", "working")];
        reconcile_incoming(&mut st);
        assert!(st.agent_runs.is_empty());
    }
}
