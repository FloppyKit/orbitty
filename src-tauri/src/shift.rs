//! Humming supervisor via ctl. Watch board + agentRuns + AX,
//! drop ghost working rows (they eat the runner cap), Needed-escalate
//! blocked, pickup next assigned todo while Work/Hold. No LLM.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::state::{AgentRun, KanbanCard, PersistedState};

static WANT: AtomicBool = AtomicBool::new(false);
static SPAWNED: AtomicBool = AtomicBool::new(false);

const TICK_SECS: u64 = 15;

fn clip_title(s: &str) -> String {
    let t = s.replace('\n', " ").trim().to_string();
    if t.chars().count() > 48 {
        format!("{}…", t.chars().take(47).collect::<String>())
    } else {
        t
    }
}

fn mode_continues(st: &PersistedState) -> bool {
    matches!(st.companion.mode.to_ascii_lowercase().as_str(), "work" | "hold")
}

fn is_live(r: &AgentRun) -> bool {
    matches!(r.state.as_str(), "working" | "starting")
}

fn is_blocked(r: &AgentRun) -> bool {
    r.state == "blocked"
}

/// Empty-seat working row: no agent, no herdr pen. Ghost GO fires
/// ate the runner cap this way (GO-DESK-MOBILE once).
fn is_ghost(r: &AgentRun) -> bool {
    is_live(r) && r.agent_shortcut_id.trim().is_empty() && r.herdr.is_none()
}

fn doing_card<'a>(st: &'a PersistedState, card_id: &str) -> Option<&'a KanbanCard> {
    st.kanban_cards.iter().find(|c| {
        c.id == card_id && c.kind.as_deref() == Some("todo") && c.status == "doing"
    })
}

fn is_orphan(st: &PersistedState, r: &AgentRun) -> bool {
    doing_card(st, &r.card_id).is_none() && r.state != "done"
}

fn open_needed_title(st: &PersistedState, title: &str) -> bool {
    st.kanban_cards.iter().any(|c| {
        c.kind.as_deref() == Some("needed")
            && c.needed_state.as_deref() == Some("open")
            && c.title.trim() == title
    })
}

fn card_title(st: &PersistedState, card_id: &str) -> String {
    st.kanban_cards
        .iter()
        .find(|c| c.id == card_id)
        .map(|c| clip_title(&c.title))
        .unwrap_or_else(|| "run".into())
}

/// Start the 15s ticker once. `shift on` flips WANT; off skips the body.
pub fn boot(app: &tauri::AppHandle) {
    let on = crate::state::load_state(app)
        .map(|s| s.shift_on)
        .unwrap_or(false);
    WANT.store(on, Ordering::SeqCst);
    if SPAWNED.swap(true, Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    let _ = std::thread::Builder::new()
        .name("orbitty-shift".into())
        .spawn(move || loop {
            std::thread::sleep(Duration::from_secs(TICK_SECS));
            if WANT.load(Ordering::SeqCst) {
                let _ = tick(&handle);
            }
        });
}

pub fn status(app: &tauri::AppHandle) -> Result<Value, String> {
    let mut st = crate::state::load_state(app)?;
    crate::gate_caps::heal_defaults(&mut st);
    crate::gate_caps::roll_day(&mut st);
    let live = crate::gate_caps::in_flight(&st);
    let ax = crate::windows::accessibility_status();
    let runs: Vec<Value> = st
        .agent_runs
        .iter()
        .filter(|r| r.state != "done")
        .map(|r| {
            json!({
                "cardId": r.card_id,
                "title": card_title(&st, &r.card_id),
                "state": r.state,
                "agentId": r.agent_shortcut_id,
                "planetId": r.planet_id,
                "ghost": is_ghost(r),
                "orphan": is_orphan(&st, r),
            })
        })
        .collect();
    let next: Vec<Value> = next_pickups(&st)
        .into_iter()
        .map(|(agent_id, agent, card)| {
            json!({
                "agentId": agent_id,
                "agent": agent,
                "cardId": card.id,
                "title": clip_title(&card.title),
            })
        })
        .collect();
    Ok(json!({
        "on": st.shift_on,
        "mode": st.companion.mode,
        "continues": mode_continues(&st),
        "axTrusted": ax.trusted,
        "runners": { "live": live, "cap": st.runner_cap },
        "spendUsd": st.spend_usd,
        "spendCapUsd": st.spend_cap_usd,
        "spendCapEnabled": st.spend_cap_enabled,
        "runs": runs,
        "next": next,
    }))
}

pub fn set_on(app: &tauri::AppHandle, on: bool) -> Result<Value, String> {
    let before = crate::state::load_state(app)?;
    let mut after = before.clone();
    after.shift_on = on;
    crate::board::save_with_events(app, &before, &after)?;
    WANT.store(on, Ordering::SeqCst);
    boot(app);
    let mut out = json!({ "on": on });
    if on {
        out["tick"] = tick(app).unwrap_or(json!({ "ok": false }));
    }
    Ok(out)
}

pub fn tick(app: &tauri::AppHandle) -> Result<Value, String> {
    let st = crate::state::load_state(app)?;
    let mut parked: Vec<Value> = Vec::new();
    let mut escalated: Vec<Value> = Vec::new();
    let mut picked: Vec<Value> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    let drop: Vec<(String, bool, String, String)> = st
        .agent_runs
        .iter()
        .filter(|r| is_ghost(r) || is_orphan(&st, r))
        .map(|r| {
            (
                r.card_id.clone(),
                is_ghost(r),
                card_title(&st, &r.card_id),
                r.planet_id.clone(),
            )
        })
        .collect();
    if !drop.is_empty() {
        let before = crate::state::load_state(app)?;
        let mut after = before.clone();
        after
            .agent_runs
            .retain(|r| !drop.iter().any(|(id, _, _, _)| id == &r.card_id));
        let _ = crate::board::save_with_events(app, &before, &after);
        for (id, ghost, title, planet) in &drop {
            if *ghost {
                let need = format!("Ghost run: {title}");
                let _ = crate::board::needed_open(
                    app,
                    &need,
                    planet,
                    Some("Empty-seat working row ate a runner slot."),
                );
                crate::event_log::append_best(
                    app,
                    "shift-park",
                    None,
                    Some(planet),
                    Some("agent"),
                    true,
                    &json!({ "cardId": id, "reason": "ghost" }),
                );
            }
            parked.push(json!({
                "cardId": id,
                "title": title,
                "reason": if *ghost { "ghost" } else { "orphan" }
            }));
        }
    }

    let st = crate::state::load_state(app).unwrap_or(st);
    for r in st.agent_runs.iter().filter(|r| is_blocked(r)) {
        let title = card_title(&st, &r.card_id);
        let need = format!("Stuck: {title}");
        if open_needed_title(&st, &need) {
            continue;
        }
        match crate::board::needed_open(
            app,
            &need,
            &r.planet_id,
            Some("Blocked doing-run; shift parked it for a human."),
        ) {
            Ok(_) => escalated.push(json!({ "cardId": r.card_id, "title": title })),
            Err(e) => notes.push(e),
        }
    }

    let st = crate::state::load_state(app)?;
    if crate::interrupt_gate::is_frozen() {
        notes.push("interrupt_gate STOP freeze — shift pickup held".into());
    } else if mode_continues(&st) {
        match crate::gate_caps::check(app) {
            Err(e) => notes.push(e),
            Ok(()) => {
                let st = crate::state::load_state(app)?;
                let live_agents: Vec<String> = st
                    .agent_runs
                    .iter()
                    .filter(|r| is_live(r))
                    .map(|r| r.agent_shortcut_id.clone())
                    .filter(|s| !s.is_empty())
                    .collect();
                for (agent_id, agent_name, _card) in next_pickups(&st) {
                    if live_agents.iter().any(|id| id == &agent_id) {
                        continue;
                    }
                    if crate::gate_caps::check(app).is_err() {
                        break;
                    }
                    match crate::agents::agent_pickup(app, Some(&agent_id), None, None) {
                        Ok(v) if v.get("picked").and_then(|x| x.as_bool()) == Some(true) => {
                            let title = v
                                .get("title")
                                .and_then(|t| t.as_str())
                                .map(clip_title)
                                .unwrap_or_else(|| "next".into());
                            picked.push(json!({
                                "agentId": agent_id,
                                "agent": agent_name,
                                "cardId": v.get("cardId"),
                                "title": title,
                            }));
                            let _ = crate::coordinator::companion_note(
                                app.clone(),
                                format!("Shift: picking up {title}."),
                                Some("work".into()),
                            );
                        }
                        Ok(_) => {}
                        Err(e) => notes.push(e),
                    }
                }
            }
        }
    }

    Ok(json!({
        "ok": true,
        "parked": parked,
        "escalated": escalated,
        "picked": picked,
        "notes": notes,
    }))
}

fn next_pickups(st: &PersistedState) -> Vec<(String, String, KanbanCard)> {
    let mut out = Vec::new();
    for agent in &st.agent_shortcuts {
        let pin = agent
            .project_id
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty());
        let card = st
            .kanban_cards
            .iter()
            .filter(|c| {
                c.kind.as_deref() == Some("todo")
                    && c.status == "todo"
                    && c.assigned_agent_id.as_deref() == Some(agent.id.as_str())
                    && match pin {
                        Some(pid) => c.project_id == pid,
                        None => true,
                    }
            })
            .min_by(|a, b| {
                a.created_at
                    .partial_cmp(&b.created_at)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned();
        if let Some(card) = card {
            out.push((agent.id.clone(), agent.name.clone(), card));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AgentRun;

    fn run(state: &str, agent: &str, herdr: bool) -> AgentRun {
        AgentRun {
            card_id: "c-1".into(),
            planet_id: "p-1".into(),
            agent_shortcut_id: agent.into(),
            state: state.into(),
            herdr: if herdr {
                Some(crate::state::AgentRunHerdr::default())
            } else {
                None
            },
        }
    }

    #[test]
    fn ghost_is_empty_seat_working() {
        assert!(is_ghost(&run("working", "", false)));
        assert!(is_ghost(&run("starting", "", false)));
        assert!(!is_ghost(&run("working", "a-1", false)));
        assert!(!is_ghost(&run("working", "", true)));
        assert!(!is_ghost(&run("blocked", "", false)));
        assert!(!is_ghost(&run("done", "", false)));
    }

    #[test]
    fn work_hold_continue_spark_does_not() {
        let mut st = PersistedState::default();
        st.companion.mode = "work".into();
        assert!(mode_continues(&st));
        st.companion.mode = "hold".into();
        assert!(mode_continues(&st));
        st.companion.mode = "spark".into();
        assert!(!mode_continues(&st));
        st.companion.mode = "chat".into();
        assert!(!mode_continues(&st));
    }
}
