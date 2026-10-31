//! Companion RAM. Flag `memory_ram` off: callers keep today's prompts.
//! Tests pass temp roots; this module never writes `~/.agents`.

mod graph;
mod ram;
mod recall;
mod rules;
mod tags;

pub(crate) use ram::{build, core_for_app, fit, render};
pub(crate) use recall::{finish_reply, recall};

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub(crate) const NOTES_RAM_CHARS: usize = 1_200;
pub(crate) const NOTES_FILE_BYTES: usize = 8 * 1024;

pub(crate) fn enabled(app: &tauri::AppHandle) -> bool {
    crate::flags::on(app, "memory_ram")
}

/// Side-effecting prompt build. `build` stays pure; age-out notes land here.
pub(crate) fn settle(app_data: &std::path::Path, archive: &std::path::Path, ctx: &ram::RamCtx) -> ram::RamCore {
    let ctx = graph::prepare(app_data, archive, ctx);
    let built = build(&ctx);
    let fitted = fit(&built);
    if let Err(e) = graph::note_leaves(app_data, archive, &ctx, &fitted) {
        eprintln!("orbitty: companion graph: {e}");
    }
    fitted
}

/// Pocket / hop preamble when the flag is on. Turns that survived the cap
/// are inside the render. Callers must not append the raw transcript.
pub(crate) fn prompt_block(app: &tauri::AppHandle) -> String {
    render(&fit(&core_for_app(app)))
}

pub(crate) fn wrap_line(app: &tauri::AppHandle, line: &str) -> String {
    format!("{}\nUSER:\n{line}", prompt_block(app))
}

pub(crate) fn recall_for_app(
    app: &tauri::AppHandle,
    query: &str,
    planet: Option<&str>,
) -> Result<serde_json::Value, String> {
    if !enabled(app) {
        return Ok(serde_json::json!({ "enabled": false, "hits": [] }));
    }
    let app_data = crate::coordinator::app_data(app)?;
    let hits = recall(
        &crate::coordinator::companions_root(),
        &app_data,
        query,
        planet,
    );
    recall::mark_recalled(query, &hits);
    Ok(serde_json::json!({ "enabled": true, "hits": hits }))
}

pub(crate) fn propose_for_app(
    app: &tauri::AppHandle,
    names: &[String],
    planet: Option<String>,
) -> Result<serde_json::Value, String> {
    if !enabled(app) {
        return Ok(serde_json::json!({ "enabled": false, "wrote": false }));
    }
    let app_data = crate::coordinator::app_data(app).unwrap_or_default();
    let draft = tags::propose_live(&app_data, names, planet.clone())?;
    #[cfg(unix)]
    {
        let project = planet.as_deref().unwrap_or("");
        if let Err(e) = crate::board::needed_open(app, &draft.title, project, Some(&draft.why)) {
            tags::rollback_live();
            return Err(e);
        }
    }
    Ok(serde_json::json!({
        "enabled": true,
        "wrote": false,
        "kind": draft.kind,
        "title": draft.title,
        "names": draft.names,
    }))
}

pub(crate) fn accept_for_app(
    app: &tauri::AppHandle,
    planet: Option<&str>,
) -> Result<serde_json::Value, String> {
    if !enabled(app) {
        return Ok(serde_json::json!({ "enabled": false, "wrote": false }));
    }
    let app_data = crate::coordinator::app_data(app)?;
    let wrote = tags::accept_live(&app_data, planet)?;
    Ok(serde_json::json!({ "enabled": true, "wrote": true, "tags": wrote }))
}

fn off() -> serde_json::Value {
    serde_json::json!({ "enabled": false })
}

pub(crate) fn rule_list_for_app(app: &tauri::AppHandle) -> Result<serde_json::Value, String> {
    if !enabled(app) {
        return Ok(off());
    }
    let app_data = crate::coordinator::app_data(app)?;
    let rules = rules::list(&app_data);
    Ok(serde_json::json!({ "enabled": true, "rules": rules }))
}

pub(crate) fn rule_set_for_app(
    app: &tauri::AppHandle,
    text: &str,
    id: Option<&str>,
) -> Result<serde_json::Value, String> {
    if !enabled(app) {
        return Ok(off());
    }
    let app_data = crate::coordinator::app_data(app)?;
    let row = rules::set(&app_data, id, text, "user").map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "enabled": true, "rule": row }))
}

pub(crate) fn rule_rm_for_app(app: &tauri::AppHandle, id: &str) -> Result<serde_json::Value, String> {
    if !enabled(app) {
        return Ok(off());
    }
    let app_data = crate::coordinator::app_data(app)?;
    let removed = rules::rm(&app_data, id).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "enabled": true, "removed": removed, "id": id }))
}

pub(crate) fn rule_propose_for_app(
    app: &tauri::AppHandle,
    text: &str,
    id: Option<String>,
    remove: bool,
    planet: Option<String>,
) -> Result<serde_json::Value, String> {
    if !enabled(app) {
        return Ok(serde_json::json!({ "enabled": false, "wrote": false }));
    }
    let draft = rules::propose_live(text, id, remove)?;
    #[cfg(unix)]
    {
        let project = planet.as_deref().unwrap_or("");
        if let Err(e) = crate::board::needed_open(app, &draft.title, project, Some(&draft.why)) {
            rules::rollback_live();
            return Err(e);
        }
    }
    let _ = planet;
    Ok(serde_json::json!({
        "enabled": true,
        "wrote": false,
        "kind": draft.kind,
        "title": draft.title,
    }))
}

pub(crate) fn rule_accept_for_app(app: &tauri::AppHandle) -> Result<serde_json::Value, String> {
    if !enabled(app) {
        return Ok(serde_json::json!({ "enabled": false, "wrote": false }));
    }
    let app_data = crate::coordinator::app_data(app)?;
    let mut wrote = rules::accept_live(&app_data)?;
    if let Some(obj) = wrote.as_object_mut() {
        obj.insert("enabled".into(), serde_json::json!(true));
        obj.insert("wrote".into(), serde_json::json!(true));
    }
    Ok(wrote)
}

pub(crate) fn tags_unpin_for_app(
    app: &tauri::AppHandle,
    name: &str,
    planet: Option<&str>,
) -> Result<serde_json::Value, String> {
    if !enabled(app) {
        return Ok(off());
    }
    let app_data = crate::coordinator::app_data(app)?;
    let removed = tags::unpin(&app_data, name, planet)?;
    Ok(serde_json::json!({ "enabled": true, "unpinned": removed, "name": name }))
}

pub(crate) fn get_for_app(app: &tauri::AppHandle, id: &str) -> Result<serde_json::Value, String> {
    if !enabled(app) {
        return Ok(off());
    }
    let app_data = crate::coordinator::app_data(app)?;
    let archive = crate::coordinator::companions_root();
    match graph::get(&app_data, &archive, id) {
        None => Ok(serde_json::json!({ "enabled": true, "found": false, "id": id })),
        Some((node, arch)) => Ok(serde_json::json!({
            "enabled": true,
            "found": true,
            "node": node,
            "archive": arch.map(|l| serde_json::json!({
                "id": l.id,
                "text": l.text,
                "role": l.role,
                "ts": l.ts,
            })),
        })),
    }
}

pub(crate) fn links_for_app(
    app: &tauri::AppHandle,
    id: &str,
    rel: Option<&str>,
) -> Result<serde_json::Value, String> {
    if !enabled(app) {
        return Ok(off());
    }
    let app_data = crate::coordinator::app_data(app)?;
    let links = graph::links(&app_data, id, rel);
    Ok(serde_json::json!({ "enabled": true, "links": links }))
}

/// User-rule text for the flag-on handoff. `core_for_app` and the roll
/// seam both call this. It reads `rules.json`. Callers do not pass the text in.
pub(crate) fn user_rules_for_handoff(app_data: &Path) -> String {
    rules::render_text(app_data)
}

/// Flag-on handoff after `consume_armed_reason`. Rules come only from
/// `user_rules_for_handoff` (the same loader `core_for_app` / `handoff_line`
/// use). `armed` is not constructible outside `consume_armed_reason`.
pub(crate) fn handoff_after_armed_roll(
    app_data: &Path,
    armed: crate::mouth_roll::ArmedRoll,
    mode: &str,
    wrapped_line: &str,
) -> String {
    let reason = armed.reason();
    let ctx = ram::RamCtx {
        rules: String::new(),
        user_rules: user_rules_for_handoff(app_data),
        soul: String::new(),
        now: String::new(),
        model: String::new(),
        planet_line: None,
        notes: None,
        cards: Vec::new(),
        card_id: None,
        pinned_tags: Vec::new(),
        open_asks: Vec::new(),
        answered: Vec::new(),
        // One turn so `handoff_from_turns` emits the consumed reason.
        // Empty turns return the line unchanged and hide the roll.
        turns: vec![crate::state::CompanionTurn {
            role: "you".into(),
            text: "roll-keep".into(),
        }],
        now_secs: 0.0,
    };
    let core = fit(&build(&ctx));
    handoff_from_core(reason, mode, &core, &[], wrapped_line)
}

/// Handoff when the flag is on. A line that already carries the RAM block
/// is returned unchanged so state turns are not appended. Otherwise the
/// history is only the turns still in RAM.
pub(crate) fn handoff_line(
    app: &tauri::AppHandle,
    reason: crate::mouth_roll::ResetReason,
    wrapped_line: &str,
) -> String {
    if wrapped_line.starts_with("[rules]\n") || wrapped_line.contains("\n[rules]\n") {
        return wrapped_line.to_string();
    }
    let core = fit(&core_for_app(app));
    let state = crate::state::load_state(app).ok();
    let mode = state
        .as_ref()
        .map(|s| s.companion.mode.clone())
        .unwrap_or_else(|| "spark".into());
    let state_turns = state.map(|s| s.companion.turns).unwrap_or_default();
    handoff_from_core(reason, &mode, &core, &state_turns, wrapped_line)
}

/// Flag-on handoff body. `state_turns` is the saved transcript (what
/// `load_state` would re-inject). Only `core.turns` is spoken.
pub(crate) fn handoff_from_core(
    reason: crate::mouth_roll::ResetReason,
    mode: &str,
    core: &ram::RamCore,
    state_turns: &[crate::state::CompanionTurn],
    wrapped_line: &str,
) -> String {
    let preamble = render(core);
    let line = format!("{preamble}\n{wrapped_line}");
    let turns = &core.turns;
    let _ = state_turns;
    handoff_visible(reason, mode, turns, &line)
}

fn handoff_visible(
    reason: crate::mouth_roll::ResetReason,
    mode: &str,
    turns: &[crate::state::CompanionTurn],
    line: &str,
) -> String {
    let body = crate::mouth_roll::handoff_from_turns(reason, mode, turns, line);
    // The release string is `body`. Tests also prefix every chosen turn so a
    // slice longer than RAM (the saved transcript) fails the forget assertion.
    // `handoff_from_turns` keeps only the last 6, which would hide `turn-0`.
    #[cfg(test)]
    {
        let listed = turns
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        return format!("{listed}\n{body}");
    }
    #[cfg(not(test))]
    body
}

pub(crate) fn safe_planet(project_id: &str) -> Option<String> {
    let id = crate::coordinator::safe_id(project_id);
    if id.is_empty() || id == "." || id == ".." || id.contains("..") {
        return None;
    }
    Some(id)
}

pub(crate) fn planet_dir(app_data: &Path, project_id: &str) -> Option<PathBuf> {
    Some(app_data.join("planets").join(safe_planet(project_id)?))
}

pub(crate) fn notes_path(app_data: &Path, project_id: &str) -> Option<PathBuf> {
    Some(planet_dir(app_data, project_id)?.join("notes.md"))
}

pub(crate) fn planet_tags_path(app_data: &Path, project_id: &str) -> Option<PathBuf> {
    Some(planet_dir(app_data, project_id)?.join("tags.json"))
}

pub(crate) fn global_tags_path(app_data: &Path) -> PathBuf {
    app_data.join("tags").join("global.json")
}

pub(crate) fn ensure_planet(app_data: &Path, project_id: &str) -> io::Result<PathBuf> {
    let dir = planet_dir(app_data, project_id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad planet id"))?;
    ensure_dir(&dir)?;
    ensure_dir(&dir.join("tabs"))?;
    Ok(dir)
}

pub(crate) fn write_notes(app_data: &Path, project_id: &str, text: &str) -> io::Result<()> {
    let dir = ensure_planet(app_data, project_id)?;
    let mut out = String::new();
    for c in text.chars() {
        if out.len() + c.len_utf8() > NOTES_FILE_BYTES {
            break;
        }
        out.push(c);
    }
    write_0600(&dir.join("notes.md"), &out)
}

pub(crate) fn read_notes_head(app_data: &Path, project_id: &str) -> String {
    let Some(path) = notes_path(app_data, project_id) else {
        return String::new();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return String::new();
    };
    text.chars().take(NOTES_RAM_CHARS).collect()
}

pub(crate) fn ensure_dir(dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let mut b = fs::DirBuilder::new();
        b.recursive(true).mode(0o700);
        b.create(dir)?;
        let mut p = fs::metadata(dir)?.permissions();
        p.set_mode(0o700);
        let _ = fs::set_permissions(dir, p);
        return Ok(());
    }
    #[cfg(not(unix))]
    fs::create_dir_all(dir)
}

pub(crate) fn write_0600(path: &Path, body: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let mut opts = OpenOptions::new();
    opts.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(path)?.permissions();
        p.set_mode(0o600);
        let _ = fs::set_permissions(path, p);
    }
    f.write_all(body.as_bytes())?;
    if !body.ends_with('\n') {
        f.write_all(b"\n")?;
    }
    f.flush()?;
    f.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests;
