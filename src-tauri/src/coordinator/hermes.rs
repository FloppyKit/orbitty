//! Coordinator + soul.md. Titles-only context. No LLM in this module.

#![allow(unused_imports)]
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};

#[allow(unused_imports)]
use super::allowlist::*;
#[allow(unused_imports)]
use super::plug::*;
#[allow(unused_imports)]
use super::mouth_cli::*;
#[allow(unused_imports)]
use super::commands::*;
#[allow(unused_imports)]
use super::intercept::*;
#[allow(unused_imports)]
use super::send::*;
#[allow(unused_imports)]
use super::soul_wake::*;
#[allow(unused_imports)]
use super::action::*;
#[allow(unused_imports)]
use super::paths::*;
#[allow(unused_imports)]
use super::cursor_perm::*;
#[allow(unused_imports)]
use super::launch_agent::*;
#[cfg(target_os = "macos")]
#[allow(unused_imports)]
use super::mac;

pub(crate) fn app_data(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Project ids are app-generated (`p-…`); filter anyway so a hand-edited
/// state.json can never become a path traversal.
pub(crate) fn safe_id(project_id: &str) -> String {
    project_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub(crate) fn context_path(app: &tauri::AppHandle, project_id: &str) -> Result<PathBuf, String> {
    let dir = app_data(app)?.join("context");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir.join(format!("{}.md", safe_id(project_id))))
}

/// Titles-only context, overwritten every launch (never appended).
pub(crate) fn write_context(
    app: &tauri::AppHandle,
    project_id: &str,
    project_name: &str,
    titles: &[String],
) -> Result<PathBuf, String> {
    let path = context_path(app, project_id)?;
    let mut body = format!(
        "# {project_name}\n\nOrbitty planet context — titles only. No file contents, no PHI.\n"
    );
    body.push('\n');
    if titles.is_empty() {
        body.push_str("(no claimed windows or open to-dos)\n");
    } else {
        for t in titles {
            body.push_str(&format!("- {}\n", t.replace('\n', " ")));
        }
    }
    std::fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(target_os = "macos")]
pub(crate) fn try_open_hermes_app() -> bool {
    Command::new("open")
        .args(["-a", "Hermes"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn try_open_hermes_app() -> bool {
    false
}

/// Detached spawn: the child outlives the invoke; a NotFound error just
/// falls through to the next launcher.
pub(crate) fn spawn_detached(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

/// Shared Hermes handoff (S5 planets only — the S2.4 companion sends
/// through its own named-thread path below). Tries in order:
/// focus a running Hermes app → `open -a Hermes` → `hermes desktop` →
/// `hermes gui` → PATH `hermes` (grok chat with `cli_query`;
/// `--create-if-missing` is the existing last-resort, same as S5). Err
/// when no Hermes exists anywhere — the UI shows the path hint.
pub(crate) fn route_to_hermes(ctx: &Path, cli_query: &str) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    if mac::focus_running_hermes() {
        return Ok(format!("Focused the running Hermes. Context: {}", ctx.display()));
    }
    if try_open_hermes_app() {
        return Ok(format!("Opened Hermes.app. Context: {}", ctx.display()));
    }
    if spawn_detached("hermes", &["desktop"]) {
        return Ok(format!("Launched `hermes desktop`. Context: {}", ctx.display()));
    }
    if spawn_detached("hermes", &["gui"]) {
        return Ok(format!("Launched `hermes gui`. Context: {}", ctx.display()));
    }
    if spawn_detached("hermes", &["-p", "grok", "chat", "--create-if-missing", cli_query]) {
        return Ok(format!("Launched Hermes CLI (grok). Context: {}", ctx.display()));
    }
    Err(format!(
        "No Hermes found — install the Hermes app or put `hermes` on PATH. Context file ready: {}",
        ctx.display()
    ))
}

/// Launch/focus the existing Hermes harness for a project. Writes the
/// titles-only context file first, then hands off via route_to_hermes.
#[tauri::command]
pub fn launch_coordinator(
    app: tauri::AppHandle,
    project_id: String,
    project_name: String,
    titles: Vec<String>,
) -> Result<String, String> {
    let ctx = write_context(&app, &project_id, &project_name, &titles)?;
    let query = format!(
        "Orbitty planet \"{project_name}\" — titles-only context: {}",
        ctx.display()
    );
    route_to_hermes(&ctx, &query)
}

// Companion: plug map
// The mouth is a plug map. Order: non-empty ORBITTY_COMPANION_BACKEND
// (override) → companion.backend in state → legacy "goose" only when
// goose is on PATH → unset. goose, grok, hermes, and direct are the
// Mouth picks. berd (berdctl) is the hidden escape and only arrives
// through the env var. direct never spawns a CLI. Never Cursor /
// OpenCode / ocx / Claude / Codex / Pi. An unknown value is an honest
// error at send — never a silent Berd fallback.
// Unset returns one plain row and does not call a model. Local verbs
// still answer when nothing is picked.

