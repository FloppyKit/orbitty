//! Attach `agentRuns` for the habitat droid. Never a second spawn.
//! One row per card. A save must not strip a live Doing run.

use serde_json::json;
use tauri::Emitter;

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

/// Dock hack LED heal. A drop removes a row. `went_done` is a row this
/// pass flipped to `done`. Window save runs `reconcile_incoming` first,
/// so that flip is a no-op and `went_done` stays empty while the row is
/// already `done` — the LED is still orphaned if nothing seated is
/// working|starting|blocked. `dones` plus the current rows covers that
/// pass. Mint-only and strip-heal-only passes leave both lists empty.
fn needs_hack_heal(
    drops: &[String],
    dones: &[String],
    went_done: &[(String, String)],
    runs: &[AgentRun],
) -> bool {
    if !drops.is_empty() || !went_done.is_empty() {
        return true;
    }
    dones
        .iter()
        .any(|id| runs.iter().any(|r| &r.card_id == id && r.state == "done"))
}

/// Row edits for one attach pass. No AppHandle — the save, the hack
/// heal, and the event log stay in `attach_on_moves`.
fn apply_run_edits<'a>(
    cur: &mut PersistedState,
    after_cards: &'a [KanbanCard],
    mints: &[String],
    dones: &[String],
    drops: &[String],
    heals: &[AgentRun],
) -> (bool, Vec<&'a KanbanCard>, Vec<(String, String)>) {
    let mut changed = false;
    let mut attached: Vec<&KanbanCard> = Vec::new();
    let mut went_done: Vec<(String, String)> = Vec::new();
    for card_id in mints {
        let Some(card) = after_cards.iter().find(|c| &c.id == card_id) else {
            continue;
        };
        // Upsert — one row per card, never a second walker.
        match cur.agent_runs.iter_mut().find(|r| &r.card_id == card_id) {
            Some(r) => {
                r.planet_id = card.project_id.clone();
                r.agent_shortcut_id = card.assigned_agent_id.clone().unwrap_or_default();
                r.state = "working".into();
            }
            None => cur.agent_runs.push(AgentRun {
                card_id: card.id.clone(),
                planet_id: card.project_id.clone(),
                agent_shortcut_id: card.assigned_agent_id.clone().unwrap_or_default(),
                state: "working".into(),
                herdr: None,
            }),
        }
        changed = true;
        attached.push(card);
    }
    for card_id in dones {
        if let Some(r) = cur.agent_runs.iter_mut().find(|r| &r.card_id == card_id) {
            if r.state != "done" {
                r.state = "done".into();
                went_done.push((card_id.clone(), r.planet_id.clone()));
                changed = true;
            }
        }
    }
    for card_id in drops {
        let n = cur.agent_runs.len();
        cur.agent_runs.retain(|r| &r.card_id != card_id);
        changed |= cur.agent_runs.len() != n;
    }
    for row in heals {
        // A mint this pass already rewrote the row — never twins.
        if cur.agent_runs.iter().any(|r| r.card_id == row.card_id) {
            continue;
        }
        let Some(card) = after_cards.iter().find(|c| c.id == row.card_id) else {
            continue;
        };
        cur.agent_runs.push(AgentRun {
            card_id: row.card_id.clone(),
            planet_id: card.project_id.clone(),
            agent_shortcut_id: card.assigned_agent_id.clone().unwrap_or_default(),
            state: row.state.clone(),
            herdr: row.herdr.clone(),
        });
        changed = true;
        attached.push(card);
    }
    (changed, attached, went_done)
}

/// True when heal changed the dock LED bit. `None` is a failed load, so
/// a missing side counts as a flip.
fn hack_flipped(before: Option<bool>, after: Option<bool>) -> bool {
    before != after
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
    // Events collect through the mutation and fire only after the save
    // lands — `agent-run-attach` means the row is on disk, never before.
    let (changed, attached, went_done) = apply_run_edits(
        &mut cur,
        &after.kanban_cards,
        &mints,
        &dones,
        &drops,
        &heals,
    );
    // Window save already stored `done` via reconcile_incoming, so
    // `changed` is false and the old early return skipped the heal.
    // Still clear an orphaned dock LED; do not write a second save.
    let heal_hack = needs_hack_heal(&drops, &dones, &went_done, &cur.agent_runs);
    if changed {
        // The one save door — claims are untouched, so the diff is silent.
        if crate::board::save_with_events(app, &prev, &cur).is_err() {
            return;
        }
    } else if !heal_hack {
        return;
    }
    if heal_hack {
        // Drops, window-save Done, and kanban_move all heal here.
        // The heal writes the LED and emits nothing; the dock repaints
        // only on state:changed, and only when the bit actually flips.
        let before_live = crate::state::load_state(app)
            .ok()
            .map(|s| s.companion.hack_live);
        crate::coordinator::heal_stuck_hack(app);
        let after_live = crate::state::load_state(app)
            .ok()
            .map(|s| s.companion.hack_live);
        if hack_flipped(before_live, after_live) {
            let _ = app.emit("state:changed", ());
        }
    }
    if !changed {
        return;
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
            links: None,
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

    #[test]
    fn needs_hack_heal_truth_table() {
        let went = vec![("c".to_string(), "p-1".to_string())];
        // done-only: this pass flipped the row
        assert!(needs_hack_heal(&[], &[], &went, &[]));
        // done-only: reconcile_incoming already stored done, so went_done is empty
        let mut already = run("c", "done");
        already.agent_shortcut_id = "a-1".into();
        assert!(needs_hack_heal(&[], &["c".to_string()], &[], &[already]));
        // drop-only
        assert!(needs_hack_heal(&["c".to_string()], &[], &[], &[]));

        // mint-only: gated todo→doing, no drop and no done
        let mut before = PersistedState::default();
        before.kanban_cards = vec![card("c-m", "todo", "todo", "GO-ORBITTY-M", Some("a-1"))];
        let mut after = before.clone();
        after.kanban_cards[0].status = "doing".into();
        let (mints, dones, drops) = transitions(&before, &after);
        assert!(!mints.is_empty() && dones.is_empty() && drops.is_empty());
        let mut cur = before.clone();
        let (_, _, went_done) =
            apply_run_edits(&mut cur, &after.kanban_cards, &mints, &dones, &drops, &[]);
        assert!(went_done.is_empty());
        assert!(!needs_hack_heal(&drops, &dones, &went_done, &cur.agent_runs));

        // strip-heal-only: the card stays Doing, the dropped row is restored
        let mut before = PersistedState::default();
        before.kanban_cards = vec![card("c-h", "todo", "doing", "GO-ORBITTY-H", Some("a-1"))];
        before.agent_runs = vec![seated("c-h", "working")];
        let mut after = before.clone();
        after.agent_runs.clear();
        let (mints, dones, drops) = transitions(&before, &after);
        let heals = stripped(&before, &after);
        assert!(mints.is_empty() && dones.is_empty() && drops.is_empty() && !heals.is_empty());
        let mut cur = after.clone();
        let (_, _, went_done) = apply_run_edits(
            &mut cur,
            &after.kanban_cards,
            &mints,
            &dones,
            &drops,
            &heals,
        );
        assert!(went_done.is_empty());
        assert!(cur.agent_runs.iter().any(is_seated_live));
        assert!(!needs_hack_heal(&drops, &dones, &went_done, &cur.agent_runs));
    }

    /// Local copy of `coordinator::seated_live`. That function stays
    /// private; coordinator.rs is outside this GO's fence.
    fn is_seated_live(r: &AgentRun) -> bool {
        let seated = !r.agent_shortcut_id.trim().is_empty()
            || r.herdr
                .as_ref()
                .is_some_and(|h| !h.agent_name.trim().is_empty());
        seated && matches!(r.state.as_str(), "working" | "starting" | "blocked")
    }

    fn seated(id: &str, state: &str) -> AgentRun {
        let mut r = run(id, state);
        r.agent_shortcut_id = format!("agent-{id}");
        r
    }

    #[test]
    fn done_leaves_no_seated_live_run() {
        // Predicate pin: shortcut or herdr name, and working|starting|blocked.
        assert!(is_seated_live(&seated("a", "working")));
        assert!(is_seated_live(&seated("a", "starting")));
        assert!(is_seated_live(&seated("a", "blocked")));
        assert!(!is_seated_live(&seated("a", "done")));
        assert!(!is_seated_live(&run("bare", "working")));
        let mut herdr_row = run("h", "working");
        herdr_row.herdr = Some(crate::state::AgentRunHerdr {
            agent_name: "grok".into(),
            ..Default::default()
        });
        assert!(is_seated_live(&herdr_row));

        // Board move: disk row still working. The same edits attach applies.
        let mut before = PersistedState::default();
        before.kanban_cards = vec![card("A", "todo", "doing", "GO-ORBITTY-A", Some("agent-A"))];
        before.agent_runs = vec![seated("A", "working")];
        let mut after = before.clone();
        after.kanban_cards[0].status = "done".into();
        let (mints, dones, drops) = transitions(&before, &after);
        let heals = stripped(&before, &after);
        assert!(mints.is_empty() && drops.is_empty() && heals.is_empty());
        assert_eq!(dones, vec!["A".to_string()]);
        let mut cur = before.clone();
        let (changed, _, went_done) = apply_run_edits(
            &mut cur,
            &after.kanban_cards,
            &mints,
            &dones,
            &drops,
            &heals,
        );
        assert!(changed);
        assert_eq!(cur.agent_runs[0].state, "done");
        assert!(cur.agent_runs.iter().all(|r| !is_seated_live(r)));
        assert!(needs_hack_heal(&drops, &dones, &went_done, &cur.agent_runs));

        // Window save: reconcile_incoming already stored done, so this
        // pass flips nothing. The heal decision is still on.
        let mut cur = before.clone();
        cur.agent_runs[0].state = "done".into();
        let (changed, _, went_done) = apply_run_edits(
            &mut cur,
            &after.kanban_cards,
            &mints,
            &dones,
            &drops,
            &heals,
        );
        assert!(!changed);
        assert!(went_done.is_empty());
        assert!(cur.agent_runs.iter().all(|r| !is_seated_live(r)));
        assert!(needs_hack_heal(&drops, &dones, &went_done, &cur.agent_runs));

        // A second seated run still working: heal would leave the LED on.
        let mut before = PersistedState::default();
        before.kanban_cards = vec![
            card("A", "todo", "doing", "GO-ORBITTY-A", Some("agent-A")),
            card("B", "todo", "doing", "GO-ORBITTY-B", Some("agent-B")),
        ];
        before.agent_runs = vec![seated("A", "working"), seated("B", "working")];
        let mut after = before.clone();
        after.kanban_cards[0].status = "done".into();
        let (mints, dones, drops) = transitions(&before, &after);
        let heals = stripped(&before, &after);
        let mut cur = before.clone();
        let (_, _, went_done) = apply_run_edits(
            &mut cur,
            &after.kanban_cards,
            &mints,
            &dones,
            &drops,
            &heals,
        );
        let live: Vec<_> = cur
            .agent_runs
            .iter()
            .filter(|r| is_seated_live(r))
            .collect();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].card_id, "B");
        assert!(needs_hack_heal(&drops, &dones, &went_done, &cur.agent_runs));

        // The call site, not only the helper: restoring `if !drops.is_empty()`
        // around heal_stuck_hack is the orphan glow.
        let src = include_str!("agent_runs.rs");
        let start = src
            .find("pub fn attach_on_moves(")
            .expect("attach_on_moves");
        let body = &src[start..];
        assert!(body.contains("needs_hack_heal("));
        let heal = body.find("heal_stuck_hack").expect("heal_stuck_hack");
        let gate = body[..heal].rfind("if ").expect("heal gate");
        let cond = &body[gate..heal];
        assert!(
            !cond.contains("drops.is_empty()"),
            "drops-only heal trigger: {cond}"
        );
    }

    /// Call-site pin from VERIFY_SEC_HACK_LED_1449894 N4.
    /// M1/M2: window-save Done (`changed` false) still heals — the text
    /// keeps `else if !heal_hack`, not a bare `else`.
    /// M9: kanban_move heals on `if heal_hack {`, not `if heal_hack && !changed`.
    /// M8: that gate is not `drops.is_empty()` or `drops.len()`.
    #[test]
    fn attach_call_site_pins_window_kanban_and_drops() {
        let src = include_str!("agent_runs.rs");
        let start = src
            .find("pub fn attach_on_moves(")
            .expect("attach_on_moves");
        let body = &src[start..];
        let from = body.find("let heal_hack").expect("let heal_hack");
        let heal_rel = body[from..]
            .find("heal_stuck_hack")
            .expect("heal_stuck_hack");
        let between = &body[from..from + heal_rel];
        assert!(
            between.contains("else if !heal_hack"),
            "M1/M2 window-save Done heal: {between}"
        );
        assert!(
            between.contains("if heal_hack {"),
            "M9 kanban_move heal: {between}"
        );
        assert!(
            !between.contains("drops.is_empty()") && !between.contains("drops.len()"),
            "M8 drops check: {between}"
        );
        let after = &body[from + heal_rel..];
        let emit = after
            .find("\"state:changed\"")
            .expect("state:changed after heal");
        let next = after.find("if !changed").unwrap_or(usize::MAX);
        assert!(
            emit < next,
            "dock repaint must follow heal_stuck_hack before the event-log return"
        );
    }

    /// T10. The LED flip is inequality, including a missing load.
    #[test]
    fn hack_flipped_is_inequality() {
        assert!(hack_flipped(Some(true), Some(false)));
        assert!(hack_flipped(Some(false), Some(true)));
        assert!(!hack_flipped(Some(true), Some(true)));
        assert!(!hack_flipped(Some(false), Some(false)));
        assert!(!hack_flipped(None, None));
        assert!(hack_flipped(None, Some(true)));
        assert!(hack_flipped(None, Some(false)));
        assert!(hack_flipped(Some(false), None));
    }

    fn strip_rust_comments(src: &str) -> String {
        let chars: Vec<char> = src.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '"' {
                out.push('"');
                i += 1;
                while i < chars.len() {
                    let c = chars[i];
                    out.push(c);
                    i += 1;
                    if c == '\\' && i < chars.len() {
                        out.push(chars[i]);
                        i += 1;
                        continue;
                    }
                    if c == '"' {
                        break;
                    }
                }
                continue;
            }
            if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                i += 2;
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                if i + 1 < chars.len() {
                    i += 2;
                }
                continue;
            }
            out.push(chars[i]);
            i += 1;
        }
        out
    }

    /// T11. Comment-stripped order of the heal emit. E1b / E2' / E4 fail this.
    #[test]
    fn attach_on_moves_pins_hack_emit_order() {
        let src = include_str!("agent_runs.rs");
        let start = src
            .find("pub fn attach_on_moves(")
            .expect("attach_on_moves");
        let body = &src[start..];
        let end = body.find("\n#[cfg(test)]").unwrap_or(body.len());
        let stripped = strip_rust_comments(&body[..end]);
        let before = stripped.find("let before_live").expect("let before_live");
        let heal = stripped
            .find("heal_stuck_hack(app)")
            .expect("heal_stuck_hack(app)");
        let after = stripped.find("let after_live").expect("let after_live");
        let guard = "if hack_flipped(before_live, after_live) {";
        let flipped = stripped.find(guard).expect(guard);
        let emit = stripped
            .find("app.emit(\"state:changed\"")
            .expect("app.emit state:changed");
        let ret = stripped.find("if !changed").expect("if !changed");
        assert!(before < heal, "before-snapshot must precede the heal");
        assert!(heal < after, "heal must precede the after-snapshot");
        assert!(after < flipped, "after-snapshot must precede the flip guard");
        assert!(flipped < emit, "flip guard must precede the emit");
        assert!(emit < ret, "emit must precede if !changed");
    }
}
