//! Agent roster. Pickup moves one assigned todo to doing and launches
//! that row's harness. Orbitty never hosts the model.

use serde_json::{json, Value};

use crate::state::{AgentShortcut, PersistedState};

/// The four named harness doors in `coordinator::launch_agent`.
pub const HARNESSES: [&str; 4] = ["cursor", "goose", "grok", "hermes"];

/// New cursor rows, and a cursor launch whose row has no model.
pub const CURSOR_DEFAULT_MODEL: &str = "grok-4.7-high";

/// Row model, with the cursor default when the row leaves it empty.
pub(crate) fn row_model(harness: &str, model: Option<&str>) -> Option<String> {
    let set = model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if harness == "cursor" && set.is_none() {
        Some(CURSOR_DEFAULT_MODEL.into())
    } else {
        set
    }
}

/// Model strings reach a shell line and herdr argv. Only
/// `^[A-Za-z0-9._:/@+-]{1,100}$` — anything else is refused, never
/// stripped or guessed at.
pub(crate) fn validate_model(model: &str) -> Result<(), String> {
    let ok_len = (1..=100).contains(&model.len());
    let ok_chars = model
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '/' | '@' | '+' | '-'));
    if ok_len && ok_chars {
        Ok(())
    } else {
        Err(format!(
            "invalid model {model:?}: use 1-100 characters from letters, digits and . _ : / @ + -"
        ))
    }
}

/// What the cursor door passes as `--model`.
pub(crate) fn cursor_model(row: Option<&str>) -> String {
    row_model("cursor", row).unwrap_or_else(|| CURSOR_DEFAULT_MODEL.into())
}

/// "read-only" | "workspace-write" | "full-auto". Anything else is read-only.
pub(crate) fn normalize_permissions(raw: Option<&str>) -> &'static str {
    match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "workspace-write" | "workspace_write" => "workspace-write",
        "full-auto" | "full_auto" | "fullauto" => "full-auto",
        _ => "read-only",
    }
}

/// Shell + read tokens for a cursor preset. `Shell(git:…)` is the CLI's
/// command:args form; the spaced `Shell(git status)` form is the same
/// command written the way the GO names it.
pub(crate) fn cursor_allow_tokens(preset: &str) -> Vec<&'static str> {
    let mut allow = vec![
        "Shell(ls)",
        "Shell(find)",
        "Shell(head)",
        "Shell(tail)",
        "Shell(cat)",
        "Shell(wc)",
        "Shell(rg)",
        "Shell(grep)",
        "Shell(pwd)",
        "Shell(git status)",
        "Shell(git log)",
        "Shell(git diff)",
        "Shell(git show)",
        "Shell(git:status)",
        "Shell(git:log)",
        "Shell(git:diff)",
        "Shell(git:show)",
        // command:args globs so arguments still match
        // (`git log --oneline -3`, `git diff --stat`).
        "Shell(git:status*)",
        "Shell(git:log*)",
        "Shell(git:diff*)",
        "Shell(git:show*)",
        "Read(**)",
    ];
    if matches!(preset, "workspace-write" | "full-auto") {
        allow.push("Write(**)");
    }
    allow
}

/// Read-only denies: the Write tool, plus the argument forms that turn an
/// allowed read command into a write or an arbitrary exec (deny beats
/// allow): find -delete/-exec/-execdir/-ok/-okdir/-fprint*/-fls,
/// rg --pre, git --output=<file>.
pub(crate) fn cursor_read_only_deny_tokens() -> Vec<&'static str> {
    vec![
        "Write(**)",
        "Shell(find:*-delete*)",
        "Shell(find:*-exec*)",
        "Shell(find:*-ok*)",
        "Shell(find:*-fprint*)",
        "Shell(find:*-fls*)",
        "Shell(rg:*--pre*)",
        "Shell(git:*--output*)",
    ]
}

/// Project `.cursor/cli.json` `permissions` object for one preset.
/// read-only denies writes. workspace-write allows relative `Write(**)`
/// (workspace-scoped; no absolute path). full-auto clears deny so
/// `--force` is not blocked; the flags themselves are separate.
pub(crate) fn cursor_permissions_value(preset: &str) -> Value {
    let preset = normalize_permissions(Some(preset));
    let allow = cursor_allow_tokens(preset);
    let deny: Vec<&str> = if preset == "read-only" {
        cursor_read_only_deny_tokens()
    } else {
        vec![]
    };
    json!({ "allow": allow, "deny": deny })
}

/// Args after herdr's `--`, and the flags on the terminal exec line.
/// `--force` / `--trust` only for an explicit full-auto row.
pub(crate) fn cursor_agent_args(model: &str, preset: &str) -> Vec<String> {
    let mut args = vec!["--model".into(), model.to_string()];
    if normalize_permissions(Some(preset)) == "full-auto" {
        args.push("--force".into());
        args.push("--trust".into());
    }
    args
}

/// Every value single-quoted (`crate::coordinator::sq`); the model is
/// validated by the caller (`validate_model`) as well.
pub(crate) fn cursor_exec_line(bin: &str, model: &str, preset: &str) -> String {
    use crate::coordinator::sq;
    let mut line = format!("{} --model {}", sq(bin), sq(model));
    if normalize_permissions(Some(preset)) == "full-auto" {
        line.push_str(" --force --trust");
    }
    line.push_str(" \"$prompt\"");
    line
}

/// Lowercase + allowlist. Unknown = honest error — never a fall-through.
fn normalize_harness(raw: &str) -> Result<String, String> {
    let h = raw.trim().to_ascii_lowercase();
    if HARNESSES.contains(&h.as_str()) {
        Ok(h)
    } else {
        Err(format!(
            "unknown harness {raw:?} — cursor | goose | grok | hermes"
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
        "permissions": a.permissions,
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
    permissions: Option<&str>,
) -> Result<Value, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("agent.add: empty name".into());
    }
    let harness = normalize_harness(harness)?;
    let model = row_model(&harness, model);
    if let Some(m) = model.as_deref() {
        validate_model(m).map_err(|e| format!("agent.add: {e}"))?;
    }
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
        model,
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
        permissions: normalize_permissions(permissions).to_string(),
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

    // Light the dock before the column move. The move's background
    // attach writes the agentRuns row; a hack save that loaded first
    // and wrote second used to wipe that row. Hack is narrow and locked
    // now, and it lands before the move so the row is never the stale copy.
    crate::coordinator::set_companion_hack(
        app.clone(),
        true,
        Some(agent.harness.clone()),
        agent.project_id.clone(),
    )?;
    // The board's own door: todo → doing (same save + emit + log path).
    if let Err(e) = crate::board::kanban_move(app, &card.id, "doing") {
        let _ = crate::coordinator::set_companion_hack(app.clone(), false, None, None);
        return Err(e);
    }

    // The query file's planet block names the card's planet (the pin, when
    // the row is pinned — pickup filtered on it).
    let project_name = before
        .projects
        .iter()
        .find(|p| p.id == card.project_id)
        .map(|p| p.name.clone());
    let urls: Vec<String> = card
        .links
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|l| l.url.clone())
        .collect();
    let handoff = match crate::plans::gate_for_card(&card.id, &card.title, &urls) {
        crate::plans::Gate::Refuse { note } => {
            let _ = crate::board::kanban_note_append(app, &card.id, &note);
            let _ = crate::coordinator::set_companion_hack(app.clone(), false, None, None);
            return Ok(json!({
                "picked": true,
                "launched": false,
                "cardId": card.id,
                "title": card.title,
                "status": "doing",
                "agentId": agent.id,
                "agent": agent.name,
                "note": note,
            }));
        }
        crate::plans::Gate::Launch(parsed) => {
            let project = project_name.clone().unwrap_or_default();
            let run = crate::plans::resolve_run(&parsed.header, &parsed.id(), &project);
            Some(crate::coordinator::GoLaunch {
                parsed,
                run,
                route: "pickup",
                card_id: card.id.clone(),
                project_id: card.project_id.clone(),
            })
        }
        crate::plans::Gate::Skip => None,
    };
    let launch = match crate::coordinator::launch_agent(
        app.clone(),
        agent.clone(),
        project_name,
        vec![card.title.clone()],
        handoff,
    ) {
        Ok(l) => l,
        Err(e) => {
            let _ = crate::board::kanban_note_append(app, &card.id, &e);
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
    use super::*;

    fn fn_body<'a>(src: &'a str, sig: &str) -> &'a str {
        let start = src.find(sig).unwrap_or_else(|| panic!("{sig} missing"));
        let len = src[start..].find("\n}\n").expect("fn end");
        &src[start..start + len]
    }

    /// Wiring guard: agent_pickup runs the seal gate before launch_agent,
    /// a refusal appends a note and returns without launching, and the
    /// resolved GO rides into launch_agent.
    #[test]
    fn pickup_runs_the_go_gate_before_launch() {
        let body = fn_body(include_str!("agents.rs"), "pub fn agent_pickup(");
        let gate = body
            .find("crate::plans::gate_for_card(")
            .expect("pickup must call the GO gate");
        let launch = body
            .find("crate::coordinator::launch_agent(")
            .expect("pickup launches");
        assert!(gate < launch, "the gate must run before launch_agent");
        let between = &body[gate..launch];
        let r = between.find("Gate::Refuse").expect("refuse arm");
        let end = between[r..].find("Gate::Launch").map(|i| r + i).unwrap_or(between.len());
        let arm = &between[r..end];
        assert!(arm.contains("kanban_note_append"), "refusal must append a note: {arm}");
        assert!(arm.contains("return Ok(") && arm.contains("\"launched\": false"), "{arm}");
        assert!(body[launch..].contains("handoff,"), "the GO must reach launch_agent");
    }

    #[test]
    fn cursor_rows_default_the_model_other_harnesses_do_not() {
        assert_eq!(
            row_model("cursor", None).as_deref(),
            Some(CURSOR_DEFAULT_MODEL)
        );
        assert_eq!(
            row_model("cursor", Some("  ")).as_deref(),
            Some(CURSOR_DEFAULT_MODEL)
        );
        assert_eq!(
            row_model("cursor", Some("gpt-5")).as_deref(),
            Some("gpt-5")
        );
        assert_eq!(cursor_model(None), CURSOR_DEFAULT_MODEL);
        assert_eq!(cursor_model(Some("sonnet")), "sonnet");
        assert!(row_model("grok", None).is_none());
        assert!(row_model("hermes", None).is_none());
    }

    #[test]
    fn model_must_match_the_safe_charset() {
        for ok in ["grok-4.7-high", "gpt-5", "anthropic/claude-4.5@latest", "a:b+c_d.e", "x"] {
            assert!(validate_model(ok).is_ok(), "{ok}");
        }
        let long = "a".repeat(101);
        for bad in [
            "",
            "m$(touch /tmp/x)",
            "m`id`",
            "m'q",
            "m\"q",
            "a;b",
            "a b",
            "a\nb",
            "a|b",
            "a&b",
            long.as_str(),
        ] {
            let err = validate_model(bad).unwrap_err();
            assert!(err.contains("invalid model"), "{bad:?}: {err}");
        }
        assert!(validate_model(&"a".repeat(100)).is_ok());
    }

    #[test]
    fn git_arg_globs_and_read_only_escape_denies() {
        for preset in ["read-only", "workspace-write", "full-auto"] {
            let v = cursor_permissions_value(preset);
            let allow = v["allow"].as_array().unwrap();
            for t in ["Shell(git:status*)", "Shell(git:log*)", "Shell(git:diff*)", "Shell(git:show*)"] {
                assert!(allow.iter().any(|a| a == t), "{preset} missing {t}");
            }
        }
        let ro = cursor_permissions_value("read-only");
        let deny = ro["deny"].as_array().unwrap();
        for t in [
            "Write(**)",
            "Shell(find:*-delete*)",
            "Shell(find:*-exec*)",
            "Shell(find:*-ok*)",
            "Shell(rg:*--pre*)",
            "Shell(git:*--output*)",
        ] {
            assert!(deny.iter().any(|d| d == t), "read-only missing deny {t}");
        }
        assert!(cursor_permissions_value("workspace-write")["deny"].as_array().unwrap().is_empty());
    }

    #[test]
    fn presets_gate_writes_and_force() {
        assert_eq!(normalize_permissions(None), "read-only");
        assert_eq!(normalize_permissions(Some("nope")), "read-only");
        assert_eq!(normalize_permissions(Some("workspace-write")), "workspace-write");
        assert_eq!(normalize_permissions(Some("full-auto")), "full-auto");

        let ro = cursor_permissions_value("read-only");
        let allow = ro["allow"].as_array().unwrap();
        for token in [
            "Shell(find)",
            "Shell(head)",
            "Shell(ls)",
            "Shell(git status)",
            "Shell(git:diff)",
            "Read(**)",
        ] {
            assert!(allow.iter().any(|t| t == token), "missing {token}");
        }
        assert!(allow.iter().all(|t| !t.as_str().unwrap_or("").starts_with("Write(")));
        assert!(ro["deny"].as_array().unwrap().iter().any(|t| t == "Write(**)"));
        let ro_line = cursor_exec_line("/usr/local/bin/cursor-agent", "grok-4.7-high", "read-only");
        assert!(ro_line.contains("--model 'grok-4.7-high'"));
        assert!(!ro_line.contains("--force"));
        assert!(!ro_line.contains("--trust"));

        let ww = cursor_permissions_value("workspace-write");
        let allow = ww["allow"].as_array().unwrap();
        assert!(allow.iter().any(|t| t == "Write(**)"));
        assert!(allow.iter().all(|t| {
            let s = t.as_str().unwrap_or("");
            !s.starts_with("Write(/")
        }));
        assert!(ww["deny"].as_array().unwrap().is_empty());
        let ww_line = cursor_exec_line("cursor-agent", "grok-4.7-high", "workspace-write");
        assert!(!ww_line.contains("--force"));
        assert!(!ww_line.contains("--trust"));

        let fa = cursor_permissions_value("full-auto");
        assert!(fa["deny"].as_array().unwrap().is_empty());
        let fa_args = cursor_agent_args("grok-4.7-high", "full-auto");
        assert!(fa_args.iter().any(|a| a == "--force"));
        assert!(fa_args.iter().any(|a| a == "--trust"));
        let fa_line = cursor_exec_line("cursor-agent", "grok-4.7-high", "full-auto");
        assert!(fa_line.contains("--force"));
        assert!(fa_line.contains("--trust"));
        let ro_args = cursor_agent_args("grok-4.7-high", "read-only");
        assert!(!ro_args.iter().any(|a| a == "--force" || a == "--trust"));
    }

    #[test]
    fn old_agent_row_defaults_permissions_to_read_only() {
        let raw = r#"{"id":"a","name":"n","harness":"cursor","profile":""}"#;
        let a: crate::state::AgentShortcut = serde_json::from_str(raw).unwrap();
        assert_eq!(a.permissions, "read-only");
        assert!(a.model.is_none());
    }
}
