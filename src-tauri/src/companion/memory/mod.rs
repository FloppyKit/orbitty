//! Companion RAM. Flag `memory_ram` off: callers keep today's prompts.
//! Tests pass temp roots; this module never writes `~/.agents`.

mod ram;
mod recall;
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
    let draft = tags::propose_live(names, planet.clone())?;
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
    let mode = crate::state::load_state(app)
        .map(|s| s.companion.mode)
        .unwrap_or_else(|_| "spark".into());
    let preamble = render(&core);
    let line = format!("{preamble}\n{wrapped_line}");
    crate::mouth_roll::handoff_from_turns(reason, &mode, &core.turns, &line)
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
