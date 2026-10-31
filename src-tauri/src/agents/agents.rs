//! Agent roster. Pickup moves one assigned todo to doing and launches
//! that row's harness. Orbitty never hosts the model.

use serde_json::{json, Value};

use crate::state::{AgentShortcut, PersistedState};

/// Named harness doors in `coordinator::launch_agent`.
pub const HARNESSES: [&str; 5] = ["cursor", "goose", "grok", "hermes", "pi"];

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

/// A row value that reaches a harness argv or a shell line (model,
/// hermes profile). Only `^[A-Za-z0-9._:/@+-]{1,100}$`, and never a
/// leading `-` (`--force` / `--yolo` / `-f` would read as a CLI flag) —
/// anything else is refused, never stripped or guessed at.
pub(crate) fn validate_token(kind: &str, value: &str) -> Result<(), String> {
    if value.starts_with('-') {
        return Err(format!(
            "invalid {kind} {value:?}: a {kind} may not start with '-' (it would read as a flag)"
        ));
    }
    let ok_len = (1..=100).contains(&value.len());
    let ok_chars = value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '/' | '@' | '+' | '-'));
    if ok_len && ok_chars {
        Ok(())
    } else {
        Err(format!(
            "invalid {kind} {value:?}: use 1-100 characters from letters, digits and . _ : / @ + -"
        ))
    }
}

pub(crate) fn validate_model(model: &str) -> Result<(), String> {
    validate_token("model", model)
}

/// Hermes's canonical profile id: trimmed and lowercased — hermes does
/// the same (`hermes_cli/profiles.py` `normalize_profile_name`) before it
/// validates, and names profile dirs by it.
pub(crate) fn normalize_profile(profile: &str) -> String {
    profile.trim().to_lowercase()
}

/// Names hermes refuses as a profile (`_RESERVED_NAMES`, minus `default`,
/// which is its alias for the root profile).
const HERMES_RESERVED_PROFILES: [&str; 5] = ["hermes", "test", "tmp", "root", "sudo"];

/// Hermes `--profile` value: hermes's own rule, `^[a-z0-9][a-z0-9_-]{0,63}$`
/// on the lowercased value, not a reserved name (`validate_profile_name`).
/// `default` passes (hermes's root-profile alias).
pub(crate) fn validate_profile(profile: &str) -> Result<(), String> {
    let p = normalize_profile(profile);
    if p == "default" {
        return Ok(());
    }
    if p.starts_with('-') {
        return Err(format!(
            "invalid profile {profile:?}: a profile may not start with '-' (it would read as a flag)"
        ));
    }
    let mut chars = p.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let rest_ok = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if !(first_ok && rest_ok && p.len() <= 64) {
        return Err(format!(
            "invalid profile {profile:?}: hermes profiles are 1-64 characters, a-z 0-9 _ - (lowercased), starting with a letter or digit"
        ));
    }
    if HERMES_RESERVED_PROFILES.contains(&p.as_str()) {
        return Err(format!("invalid profile {profile:?}: {p:?} is reserved by hermes"));
    }
    Ok(())
}

/// Everything `agent.add` checks before it touches state (pure, tested):
/// the resolved model (row value, or the cursor default) and a non-empty
/// profile. Returns the model to store.
pub(crate) fn agent_add_checks(
    harness: &str,
    profile: Option<&str>,
    model: Option<&str>,
) -> Result<Option<String>, String> {
    let model = row_model(harness, model);
    if let Some(m) = model.as_deref() {
        validate_model(m).map_err(|e| format!("agent.add: {e}"))?;
    }
    if let Some(p) = profile.map(str::trim).filter(|s| !s.is_empty()) {
        validate_profile(p).map_err(|e| format!("agent.add: {e}"))?;
    }
    Ok(model)
}

/// Cursor door, at launch: the row model (or the default), re-validated —
/// a window save or hand-edited state never went through `agent.add`.
pub(crate) fn launch_model(shortcut: &AgentShortcut) -> Result<String, String> {
    let model = cursor_model(shortcut.model.as_deref());
    validate_model(&model)?;
    Ok(model)
}

/// Hermes profile when the row leaves it empty.
pub const HERMES_DEFAULT_PROFILE: &str = "grok";

/// Hermes fallback argv (no shell), validated at launch (NB4/NB5).
/// Profile and model each travel as ONE element — `--profile=<p>` (hermes
/// strips it before argparse, `_apply_profile_override`) and
/// `--model=<m>` (the chat subparser's `-m/--model`) — so neither can
/// become a separate flag.
pub(crate) fn hermes_args(shortcut: &AgentShortcut, query: &std::path::Path) -> Result<Vec<String>, String> {
    let profile = match normalize_profile(&shortcut.profile) {
        p if p.is_empty() => HERMES_DEFAULT_PROFILE.to_string(),
        p => p,
    };
    validate_profile(&profile)?;
    let mut args: Vec<String> = vec![
        format!("--profile={profile}"),
        "chat".into(),
        "--create-if-missing".into(),
        "-Q".into(),
        "--query-file".into(),
        query.display().to_string(),
    ];
    if let Some(m) = shortcut.model.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        validate_model(m)?;
        args.push(format!("--model={m}"));
    }
    Ok(args)
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

/// One argv element `--model=<m>`: the value can never be parsed as a
/// separate flag, whatever it holds.
pub(crate) fn cursor_model_arg(model: &str) -> String {
    format!("--model={model}")
}

/// Args after herdr's `--`, and the flags on the terminal exec line.
/// `--force` / `--trust` only for an explicit full-auto row.
pub(crate) fn cursor_agent_args(model: &str, preset: &str) -> Vec<String> {
    let mut args = vec![cursor_model_arg(model)];
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
    let mut line = format!("{} {}", sq(bin), sq(&cursor_model_arg(model)));
    if normalize_permissions(Some(preset)) == "full-auto" {
        line.push_str(" --force --trust");
    }
    // `--` ends the options: a prompt starting with `---` (a GO header) is
    // the prompt, not an unknown option (cursor-agent parses with Commander).
    line.push_str(" -- \"$prompt\"");
    line
}

/// Goose terminal door. The prompt is an option value, so it is attached
/// (`--text="$prompt"`, one argv): clap takes everything after `=`
/// literally, while `-t "$prompt"` rejects a prompt starting with `-`/`---`
/// and `--` does not help (goose 1.44.0).
pub(crate) fn goose_exec_line(bin: &str) -> String {
    format!(
        "{} run --with-builtin=developer,computercontroller --text=\"$prompt\"",
        crate::coordinator::sq(bin)
    )
}

/// Grok seat argv. Read-only is plan mode (cannot edit). Workspace
/// write accepts edits. Full-auto adds no permission-mode flag, so a
/// config `permission_mode` still applies only there. Every preset also
/// gets `--trust` so the seat does not stall on the folder-trust prompt.
/// That grant does not change tool approval. Never the auto-approve flag.
pub(crate) fn grok_permission_args(preset: &str) -> Vec<String> {
    let mut args = match normalize_permissions(Some(preset)) {
        "workspace-write" => vec!["--permission-mode".into(), "acceptEdits".into()],
        "full-auto" => Vec::new(),
        _ => vec!["--permission-mode".into(), "plan".into()],
    };
    args.push("--trust".into());
    args
}

/// Grok terminal door: `--single="$prompt"`, attached for the same reason
/// (`-p ---x` and `-p -- ---x` both fail in grok 1.0.41). Full-auto, so the
/// one-arg form the existing door test calls stays byte-for-byte the same.
pub(crate) fn grok_exec_line(bin: &str) -> String {
    grok_exec_line_for(bin, "full-auto")
}

/// Same door with the agent row's permission preset in front of `--single`.
pub(crate) fn grok_exec_line_for(bin: &str, preset: &str) -> String {
    use crate::coordinator::sq;
    let mut line = sq(bin);
    for arg in grok_permission_args(preset) {
        line.push(' ');
        line.push_str(&sq(&arg));
    }
    line.push_str(" --single=\"$prompt\"");
    line
}

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
    let model = agent_add_checks(&harness, profile, model)?;
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

/// Put a pickup that already moved the card back in todo and drop the run
/// row that attach may have written. The cap refusal never reaches here.
fn unwind_failed_pickup(app: &tauri::AppHandle, card_id: &str) {
    let _ = crate::board::kanban_move(app, card_id, "todo");
    let _ = crate::state::update(app, |st| {
        st.agent_runs.retain(|r| r.card_id != card_id);
        Ok(())
    });
    let _ = crate::coordinator::set_companion_hack(app.clone(), false, None, None);
}

/// Pickup one assigned open todo and launch the row's harness.
/// `card_id` = that card (Start). Absent = oldest assigned (ctl).
pub fn agent_pickup(
    app: &tauri::AppHandle,
    id: Option<&str>,
    name: Option<&str>,
    card_id: Option<&str>,
) -> Result<Value, String> {
    let _actor = crate::state::actor_scope("agent");
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

    // Cap before the column move. The move's background attach writes the
    // run row; counting that row refused the last open slot and left the
    // card in doing with a ghost run. A full cap returns here, card still
    // todo, no row, no attach.
    if let Err(e) = crate::gate_caps::check(app) {
        return Err(e);
    }
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
            unwind_failed_pickup(app, &card.id);
            return Ok(json!({
                "picked": true,
                "launched": false,
                "cardId": card.id,
                "title": card.title,
                "status": "todo",
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
            unwind_failed_pickup(app, &card.id);
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

    /// Cap 2 with 2 live runners refuses before the card moves, so no run
    /// row is written and the card stays in todo.
    #[test]
    fn pickup_cap_full_writes_no_run() {
        let err = crate::gate_caps::launch_decision(2, 2).unwrap_err();
        assert_eq!(err, "Paused new runs. 2 of 2 runners live.");
        let body = fn_body(include_str!("agents.rs"), "pub fn agent_pickup(");
        let check = body
            .find("crate::gate_caps::check(")
            .expect("cap check");
        let move_at = body
            .find("crate::board::kanban_move(")
            .expect("column move");
        assert!(
            check < move_at,
            "the cap check must run before the card leaves todo"
        );
        assert!(
            body[..move_at].contains("return Err"),
            "a full cap returns before any run row can be attached"
        );
        assert!(
            body.contains("unwind_failed_pickup"),
            "a later launch failure puts the card back and drops the row"
        );
    }

    /// Cap 2 with 1 live runner fits. The seat's own row, once attached,
    /// is not counted against it.
    #[test]
    fn pickup_cap_last_slot_launches() {
        assert!(crate::gate_caps::launch_decision(1, 2).is_ok());
        assert_eq!(crate::gate_caps::counted_runners(2, true), 1);
        assert!(crate::gate_caps::launch_decision(crate::gate_caps::counted_runners(2, true), 2).is_ok());
        let err = crate::gate_caps::launch_decision(crate::gate_caps::counted_runners(3, true), 2).unwrap_err();
        assert_eq!(err, "Paused new runs. 2 of 2 runners live.");
        let src = include_str!("../coordinator/launch_agent.rs");
        assert!(
            src.contains("check_launch("),
            "launch_agent must ignore this seat's own row"
        );
        assert!(
            !src.contains("gate_caps::check("),
            "a bare check would count the row this pickup just attached"
        );
    }

    #[test]
    fn grok_read_only_never_always_approve() {
        for preset in ["read-only", "workspace-write", "full-auto", "", "nope"] {
            let args = grok_permission_args(preset);
            let banned = format!("--{}{}", "always", "-approve");
            assert!(!args.iter().any(|a| a == &banned), "{preset}: {args:?}");
            let line = grok_exec_line_for("/usr/bin/grok", preset);
            assert!(!line.contains(&banned), "{line}");
        }
        let ro = grok_permission_args("read-only");
        assert!(
            ro.windows(2).any(|w| w[0] == "--permission-mode" && w[1] == "plan"),
            "{ro:?}"
        );
    }

    #[test]
    fn grok_args_per_preset() {
        assert_eq!(
            grok_permission_args("read-only"),
            vec![
                "--permission-mode".to_string(),
                "plan".to_string(),
                "--trust".to_string(),
            ]
        );
        assert_eq!(
            grok_permission_args("workspace-write"),
            vec![
                "--permission-mode".to_string(),
                "acceptEdits".to_string(),
                "--trust".to_string(),
            ]
        );
        assert_eq!(grok_permission_args("full-auto"), vec!["--trust".to_string()]);
        assert_eq!(grok_permission_args("nope"), grok_permission_args("read-only"));
    }

    #[test]
    fn grok_seat_args_include_trust_every_preset() {
        let banned = format!("--{}{}", "always", "-approve");
        let ro = grok_permission_args("read-only");
        assert!(ro.windows(2).any(|w| w[0] == "--permission-mode" && w[1] == "plan"), "{ro:?}");
        assert!(ro.iter().any(|a| a == "--trust"), "{ro:?}");
        let ww = grok_permission_args("workspace-write");
        assert!(ww.windows(2).any(|w| w[0] == "--permission-mode" && w[1] == "acceptEdits"), "{ww:?}");
        assert!(ww.iter().any(|a| a == "--trust"), "{ww:?}");
        let fa = grok_permission_args("full-auto");
        assert!(fa.iter().any(|a| a == "--trust"), "{fa:?}");
        assert!(!fa.iter().any(|a| a == "--permission-mode"), "{fa:?}");
        for preset in ["read-only", "workspace-write", "full-auto"] {
            let args = grok_permission_args(preset);
            assert!(!args.iter().any(|a| a == &banned), "{preset}: {args:?}");
        }
    }

    #[test]
    fn companion_grok_args_unchanged() {
        let send = include_str!("../coordinator/send.rs");
        let roll = include_str!("../companion/mouth_roll.rs");
        assert!(send.contains("--always-approve"), "companion door keeps its own flag");
        assert!(!send.contains("--trust"), "companion send.rs must not gain --trust");
        assert!(!roll.contains("--trust"), "companion mouth_roll.rs must not gain --trust");
        assert!(!send.contains("grok_permission_args"));
        assert!(!roll.contains("grok_permission_args"));
    }

    #[test]
    fn fallback_grok_line_has_permission_arg() {
        let line = grok_exec_line_for("/usr/local/bin/grok", "read-only");
        assert!(line.contains("'--permission-mode'"), "{line}");
        assert!(line.contains("'plan'"), "{line}");
        assert!(line.contains("'--trust'"), "{line}");
        assert!(line.ends_with(" --single=\"$prompt\""), "{line}");
        assert!(!line.contains(&format!("--{}{}", "always", "-approve")), "{line}");
        let ww = grok_exec_line_for("/usr/local/bin/grok", "workspace-write");
        assert!(ww.contains("'acceptEdits'"), "{ww}");
        assert_eq!(
            grok_exec_line("/usr/local/bin/grok"),
            "'/usr/local/bin/grok' '--trust' --single=\"$prompt\""
        );
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

    /// NB1: a flag-shaped model is refused, and the model always travels
    /// as ONE `--model=<m>` argument (herdr argv and the terminal line).
    #[test]
    fn model_cannot_become_a_flag() {
        for bad in ["--force", "--yolo", "-f", "-", "--trust"] {
            let err = validate_model(bad).unwrap_err();
            assert!(err.contains("may not start with '-'"), "{bad:?}: {err}");
        }
        assert!(validate_model("grok-4.7-high").is_ok());
        assert!(validate_model("a-b").is_ok());

        let args = cursor_agent_args("grok-4.7-high", "read-only");
        assert_eq!(args, vec!["--model=grok-4.7-high".to_string()]);
        assert!(!args.iter().any(|a| a == "--model"));
        // Even an unvalidated flag-shaped value stays inside one element.
        let args = cursor_agent_args("--force", "read-only");
        assert_eq!(args, vec!["--model=--force".to_string()]);

        let line = cursor_exec_line("/usr/local/bin/cursor-agent", "grok-4.7-high", "read-only");
        assert_eq!(
            line,
            "'/usr/local/bin/cursor-agent' '--model=grok-4.7-high' -- \"$prompt\""
        );
        assert!(!line.contains("'--model' "));
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
        assert!(ro_line.contains("'--model=grok-4.7-high'"));
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

    fn row(harness: &str, profile: &str, model: Option<&str>) -> AgentShortcut {
        let mut v = json!({"id": "a", "name": "n", "harness": harness, "profile": profile});
        if let Some(m) = model {
            v["model"] = json!(m);
        }
        serde_json::from_value(v).unwrap()
    }

    /// Nit: the cursor door's launch-time re-check, as a pure helper.
    #[test]
    fn launch_model_revalidates_the_row() {
        assert_eq!(launch_model(&row("cursor", "", None)).unwrap(), CURSOR_DEFAULT_MODEL);
        assert_eq!(launch_model(&row("cursor", "", Some("sonnet-4.5"))).unwrap(), "sonnet-4.5");
        for bad in ["--force", "--yolo", "-f", "m$(id)", "a b"] {
            let err = launch_model(&row("cursor", "", Some(bad))).unwrap_err();
            assert!(err.contains("invalid model"), "{bad:?}: {err}");
        }
    }

    /// NB4 + NB5: hermes argv is validated and each value is one element.
    #[test]
    fn hermes_args_refuse_flag_models_and_profiles() {
        let q = std::path::Path::new("/tmp/q.md");
        let ok = hermes_args(&row("hermes", "work", Some("anthropic/claude-sonnet-4")), q).unwrap();
        assert_eq!(
            ok,
            vec![
                "--profile=work",
                "chat",
                "--create-if-missing",
                "-Q",
                "--query-file",
                "/tmp/q.md",
                "--model=anthropic/claude-sonnet-4",
            ]
        );
        // Defaults: empty profile → grok, no model → no --model.
        let d = hermes_args(&row("hermes", "  ", None), q).unwrap();
        assert_eq!(d[0], "--profile=grok");
        assert!(!d.iter().any(|a| a.starts_with("--model") || a == "-m"));
        for bad in ["--force", "--yolo", "-f", "x --yolo", "m$(touch /tmp/x)", "m`id`", "a;b"] {
            let err = hermes_args(&row("hermes", "work", Some(bad)), q).unwrap_err();
            assert!(err.contains("invalid model"), "model {bad:?}: {err}");
            let err = hermes_args(&row("hermes", bad, None), q).unwrap_err();
            assert!(err.contains("invalid profile"), "profile {bad:?}: {err}");
        }
        assert!(validate_profile("--yolo").unwrap_err().contains("may not start with '-'"));
        assert!(validate_profile("grok").is_ok());
        // Hermes lowercases the profile; so does the argv we pass.
        let w = hermes_args(&row("hermes", " Work ", None), q).unwrap();
        assert_eq!(w[0], "--profile=work");
    }

    /// N3: hermes's own profile rule on the lowercased value.
    #[test]
    fn profile_follows_hermes_rule() {
        for ok in ["work", "Work", "WORK", "a", "0x", "my_prof-2", "default", "Default", &"a".repeat(64)] {
            assert!(validate_profile(ok).is_ok(), "{ok:?}");
        }
        let long = "a".repeat(65);
        for bad in [
            "", "a.b", "-x", "x/y", "../x", "a:b", "x@y", "a+b", "_a", "a b", long.as_str(),
            "hermes", "Tmp", "test", "root", "sudo",
        ] {
            let err = validate_profile(bad).unwrap_err();
            assert!(err.contains("invalid profile"), "{bad:?}: {err}");
        }
        // Models keep the wider token rule.
        assert!(validate_model("a.b").is_ok());
        assert!(validate_model("openai/gpt-5").is_ok());
    }

    /// N2: what agent.add checks, without an app handle.
    #[test]
    fn agent_add_checks_refuse_flag_profiles_and_models() {
        assert_eq!(agent_add_checks("cursor", None, None).unwrap().as_deref(), Some(CURSOR_DEFAULT_MODEL));
        assert_eq!(agent_add_checks("hermes", Some(""), None).unwrap(), None);
        assert_eq!(agent_add_checks("hermes", Some("  "), None).unwrap(), None);
        assert_eq!(
            agent_add_checks("hermes", Some("Work"), Some("sonnet")).unwrap().as_deref(),
            Some("sonnet")
        );
        for (p, m) in [(Some("--yolo"), None), (Some(" -f"), None), (Some("a.b"), None), (None, Some("--force")), (None, Some("m$(id)"))] {
            let err = agent_add_checks("hermes", p, m).unwrap_err();
            assert!(err.starts_with("agent.add: invalid "), "{p:?} {m:?}: {err}");
        }
        // Cursor: an explicit flag-shaped model is refused, not defaulted.
        assert!(agent_add_checks("cursor", None, Some("-f")).is_err());
    }

    #[test]
    fn old_agent_row_defaults_permissions_to_read_only() {
        let raw = r#"{"id":"a","name":"n","harness":"cursor","profile":""}"#;
        let a: crate::state::AgentShortcut = serde_json::from_str(raw).unwrap();
        assert_eq!(a.permissions, "read-only");
        assert!(a.model.is_none());
    }

    #[test]
    fn pi_is_a_named_door() {
        assert_eq!(normalize_harness("Pi").unwrap(), "pi");
        assert!(normalize_harness("claude").is_err());
    }
}
