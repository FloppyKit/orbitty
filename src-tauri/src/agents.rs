//! Agent roster. Pickup moves one assigned todo to doing and launches
//! that row's harness. Orbitty never hosts the model.

use serde_json::{json, Value};

use crate::state::{AgentShortcut, PersistedState};

/// Named harness doors in `coordinator::launch_agent`.
pub const HARNESSES: [&str; 5] = ["cursor", "goose", "grok", "hermes", "pi"];

/// Lowercase + allowlist. Unknown = honest error — never a fall-through.
fn normalize_harness(raw: &str) -> Result<String, String> {
    let h = raw.trim().to_ascii_lowercase();
    if HARNESSES.contains(&h.as_str()) {
        Ok(h)
    } else {
        Err(format!(
            "unknown harness {raw:?} — {}",
            HARNESSES.join(" | ")
        ))
    }
}

/// id exact, then name exact, then a unique case-insensitive name — the
/// same resolve law as action.run. Unknown / ambiguous = honest error.
fn find_agent<'a>(st: &'a PersistedState, id_or_name: &str) -> Result<&'a AgentShortcut, String> {
    let raw = id_or_name.trim();
    if raw.is_empty() {
        return Err("empty agent id/name".into());
    }
    if let Some(a) = st.agent_shortcuts.iter().find(|a| a.id == raw) {
        return Ok(a);
    }
    if let Some(a) = st.agent_shortcuts.iter().find(|a| a.name == raw) {
        return Ok(a);
    }
    let lower = raw.to_lowercase();
    let matches: Vec<&AgentShortcut> = st
        .agent_shortcuts
        .iter()
        .filter(|a| a.name.to_lowercase() == lower)
        .collect();
    match matches.len() {
        1 => Ok(matches[0]),
        0 => Err(format!("unknown agent {raw:?}")),
        n => Err(format!("{raw:?} names {n} agents — use the id")),
    }
}

/// Display row: the persisted fields plus the pinned planet's name
/// (`project`), resolved for the ctl pretty-print.
fn agent_row(st: &PersistedState, a: &AgentShortcut) -> Value {
    json!({
        "id": a.id,
        "name": a.name,
        "harness": a.harness,
        "profile": a.profile,
        "model": a.model,
        "mdPaths": a.md_paths,
        "extra": a.extra,
        "projectId": a.project_id,
        "project": a.project_id.as_deref()
            .and_then(|pid| st.projects.iter().find(|p| p.id == pid))
            .map(|p| p.name.clone()),
    })
}

/// `agent.list` — the roster with planet names resolved. Read-profile safe.
pub fn agent_list(app: &tauri::AppHandle) -> Result<Value, String> {
    let st = crate::state::load_state(app)?;
    let rows: Vec<Value> = st.agent_shortcuts.iter().map(|a| agent_row(&st, a)).collect();
    Ok(Value::Array(rows))
}

/// `agent.add` — mint a row through the one save door. Empty name refused;
/// the harness is required and allowlisted (never a product default);
/// `--project` rides the kanban.add name-resolve. Same-name adds do not
/// dedup (ids are unique). Logs `agent-add`.
pub fn agent_add(
    app: &tauri::AppHandle,
    name: &str,
    harness: &str,
    profile: Option<&str>,
    model: Option<&str>,
    md_paths: Vec<String>,
    extra: Option<&str>,
    project: Option<&str>,
) -> Result<Value, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("agent.add: empty name".into());
    }
    let harness = normalize_harness(harness)?;
    let before = crate::state::load_state(app)?;
    let project_id = match project {
        Some(p) => {
            let resolved = crate::board::resolve_project(&before, p, "agent.add")?;
            if resolved.is_empty() {
                None
            } else {
                Some(resolved)
            }
        }
        None => None,
    };
    let agent = AgentShortcut {
        id: crate::board::new_id("a"),
        name: name.to_string(),
        harness,
        profile: profile
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("")
            .to_string(),
        model: model
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        md_paths: md_paths
            .iter()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect(),
        project_id,
        extra: extra
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    };
    let mut after = before.clone();
    after.agent_shortcuts.push(agent.clone());
    crate::board::save_with_events(app, &before, &after)?;
    crate::event_log::append_best(
        app,
        "agent-add",
        None,
        agent.project_id.as_deref(),
        Some("agent"),
        true,
        &json!({ "agentId": agent.id, "name": agent.name, "harness": agent.harness }),
    );
    Ok(agent_row(&after, &agent))
}

/// `agent.rm` — drop the row by id. Cards keep their `assignedAgentId`
/// (same law as project.rm keeping cards). Unknown id = honest error.
/// Logs `agent-rm`.
pub fn agent_rm(app: &tauri::AppHandle, id: &str) -> Result<Value, String> {
    let before = crate::state::load_state(app)?;
    let agent = before
        .agent_shortcuts
        .iter()
        .find(|a| a.id == id.trim())
        .cloned()
        .ok_or_else(|| format!("agent.rm: unknown agent id {id:?}"))?;
    let mut after = before.clone();
    after.agent_shortcuts.retain(|a| a.id != agent.id);
    crate::board::save_with_events(app, &before, &after)?;
    crate::event_log::append_best(
        app,
        "agent-rm",
        None,
        None,
        Some("agent"),
        true,
        &json!({ "agentId": agent.id, "name": agent.name }),
    );
    Ok(json!({ "ok": true, "id": agent.id, "name": agent.name }))
}

/// `agent.pin` — set the row's `projectId` (id or unique planet name, the
/// kanban.add resolve). Unknown agent / planet = honest error. Logs
/// `agent-pin`.
pub fn agent_pin(app: &tauri::AppHandle, id: &str, project: &str) -> Result<Value, String> {
    let before = crate::state::load_state(app)?;
    let idx = before
        .agent_shortcuts
        .iter()
        .position(|a| a.id == id.trim())
        .ok_or_else(|| format!("agent.pin: unknown agent id {id:?}"))?;
    let project_id = crate::board::resolve_project(&before, project, "agent.pin")?;
    if project_id.is_empty() {
        return Err("agent.pin: empty project — the inbox is not a planet".into());
    }
    let mut after = before.clone();
    after.agent_shortcuts[idx].project_id = Some(project_id);
    crate::board::save_with_events(app, &before, &after)?;
    crate::event_log::append_best(
        app,
        "agent-pin",
        None,
        after.agent_shortcuts[idx].project_id.as_deref(),
        Some("agent"),
        true,
        &json!({ "agentId": after.agent_shortcuts[idx].id, "name": after.agent_shortcuts[idx].name }),
    );
    Ok(agent_row(&after, &after.agent_shortcuts[idx]))
}

/// `kanban.assign` — write the card's `assignedAgentId`. The agent may be
/// an id or a (unique) name; empty `--agent` unassigns. Unknown card /
/// agent = honest error. Logs `kanban-assign`.
pub fn kanban_assign(app: &tauri::AppHandle, card_id: &str, agent: &str) -> Result<Value, String> {
    let before = crate::state::load_state(app)?;
    let idx = before
        .kanban_cards
        .iter()
        .position(|c| c.id == card_id.trim())
        .ok_or_else(|| format!("kanban.assign: unknown card id {card_id:?}"))?;
    let agent = agent.trim();
    // Empty agent = unassign; otherwise resolve honestly (id or name).
    let picked: Option<AgentShortcut> = if agent.is_empty() {
        None
    } else {
        Some(find_agent(&before, agent)?.clone())
    };
    let mut after = before.clone();
    after.kanban_cards[idx].assigned_agent_id = picked.as_ref().map(|a| a.id.clone());
    crate::board::save_with_events(app, &before, &after)?;
    crate::event_log::append_best(
        app,
        "kanban-assign",
        None,
        Some(&after.kanban_cards[idx].project_id),
        Some("agent"),
        true,
        &json!({
            "cardId": after.kanban_cards[idx].id,
            "agentId": after.kanban_cards[idx].assigned_agent_id,
        }),
    );
    let c = &after.kanban_cards[idx];
    Ok(json!({
        "id": c.id,
        "title": c.title,
        "assignedAgentId": c.assigned_agent_id,
        "agent": picked.map(|a| a.name),
    }))
}

/// Pickup one assigned open todo and launch the row's harness.
/// `card_id` = that card (Start). Absent = oldest assigned (ctl).
pub fn agent_pickup(
    app: &tauri::AppHandle,
    id: Option<&str>,
    name: Option<&str>,
    card_id: Option<&str>,
) -> Result<Value, String> {
    let before = crate::state::load_state(app)?;
    let agent = if let Some(id) = id {
        before
            .agent_shortcuts
            .iter()
            .find(|a| a.id == id.trim())
            .cloned()
            .ok_or_else(|| format!("agent.pickup: unknown agent id {id:?}"))?
    } else if let Some(name) = name {
        find_agent(&before, name)?.clone()
    } else {
        return Err("agent.pickup: need --id ID or --name N".into());
    };

    let pin = agent
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty());
    let assigned = |c: &crate::state::KanbanCard| {
        c.kind.as_deref() == Some("todo")
            && c.status == "todo"
            && c.assigned_agent_id.as_deref() == Some(agent.id.as_str())
            && match pin {
                Some(pid) => c.project_id == pid,
                None => true,
            }
    };
    let card = if let Some(cid) = card_id.map(str::trim).filter(|s| !s.is_empty()) {
        let c = before
            .kanban_cards
            .iter()
            .find(|c| c.id == cid)
            .cloned()
            .ok_or_else(|| format!("agent.pickup: unknown card {cid}"))?;
        if !assigned(&c) {
            return Err("agent.pickup: card is not an open todo assigned to this agent".into());
        }
        Some(c)
    } else {
        before
            .kanban_cards
            .iter()
            .filter(|c| assigned(c))
            .min_by(|a, b| {
                a.created_at
                    .partial_cmp(&b.created_at)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    };
    let Some(card) = card else {
        return Ok(json!({
            "picked": false,
            "agentId": agent.id,
            "agent": agent.name,
            "reason": "nothing assigned",
        }));
    };

    // The board's own door: todo → doing (same save + emit + log path).
    crate::board::kanban_move(app, &card.id, "doing")?;

    // The query file's planet block names the card's planet (the pin, when
    // the row is pinned — pickup filtered on it).
    let project_name = before
        .projects
        .iter()
        .find(|p| p.id == card.project_id)
        .map(|p| p.name.clone());
    // light the dock BEFORE
    // the harness is up — every harness (cursor | goose | grok | hermes | pi),
    // the same door as cursor-go. The pinned planet rides along so the
    // dest/planet menus glow it while the run is live. Off lands with the
    // harness exit (the launcher's own hook — coordinator's doors); a
    // failed launch turns it straight back off so a bad bin never leaves
    // a stuck on-light.
    crate::coordinator::set_companion_hack(
        app.clone(),
        true,
        Some(agent.harness.clone()),
        agent.project_id.clone(),
    )?;
    let launch = match crate::coordinator::launch_agent(
        app.clone(),
        agent.clone(),
        project_name,
        vec![card.title.clone()],
    ) {
        Ok(l) => l,
        Err(e) => {
            let _ = crate::coordinator::set_companion_hack(app.clone(), false, None, None);
            return Err(e);
        }
    };
    crate::event_log::append_best(
        app,
        "agent-pickup",
        None,
        Some(&card.project_id),
        Some("agent"),
        true,
        &json!({ "agentId": agent.id, "cardId": card.id, "harness": agent.harness }),
    );
    Ok(json!({
        "picked": true,
        "cardId": card.id,
        "title": card.title,
        "status": "doing",
        "agentId": agent.id,
        "agent": agent.name,
        "harness": agent.harness,
        "launch": launch,
    }))
}

#[cfg(test)]
mod tests {
    use super::normalize_harness;

    #[test]
    fn pi_is_a_named_door() {
        assert_eq!(normalize_harness("Pi").unwrap(), "pi");
        assert!(normalize_harness("claude").is_err());
    }
}
