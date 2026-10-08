#![allow(unused_imports)]
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};
#[allow(unused_imports)]
use super::hermes::*;
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
use super::cursor_perm::*;
#[allow(unused_imports)]
use super::launch_agent::*;
#[cfg(target_os = "macos")]
#[allow(unused_imports)]
use super::mac;

/// Query file = concatenated readable .md paths (missing ones skipped) +
/// the row's free-text `extra` + titles-only
/// planet context when the shortcut is attached to a project OR the caller
/// handed titles over (pickup's one assigned todo — a passed title is
/// never silently dropped). Overwritten each launch, same honesty rules
/// as the project context file.
pub(crate) fn write_agent_query(
    app: &tauri::AppHandle,
    shortcut: &crate::state::AgentShortcut,
    project_name: Option<&str>,
    titles: &[String],
    go_text: Option<&str>,
) -> Result<(PathBuf, usize), String> {
    let dir = app_data(app)?.join("context");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let path = dir.join(format!("agent-{}.md", safe_id(&shortcut.id)));
    let (body, skipped) = agent_query_body(shortcut, project_name, titles, go_text);
    crate::plans::enforce_cap(&body)?;
    std::fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok((path, skipped))
}

/// The query file text (pure, so the GO handoff is testable): header, the
/// full GO text when a GO launches, md paths, extra, planet titles.
pub(crate) fn agent_query_body(
    shortcut: &crate::state::AgentShortcut,
    project_name: Option<&str>,
    titles: &[String],
    go_text: Option<&str>,
) -> (String, usize) {
    let mut body = format!("# {}\n\nOrbitty agent shortcut context.\n", shortcut.name);
    if let Some(go_text) = go_text.map(str::trim).filter(|s| !s.is_empty()) {
        body.push_str("\n\n## GO\n\n");
        body.push_str(go_text);
        body.push('\n');
    }
    let mut skipped = 0usize;
    for raw in &shortcut.md_paths {
        let p = raw.trim();
        if p.is_empty() {
            continue;
        }
        let expanded = if p == "~" {
            std::env::var("HOME").unwrap_or_default()
        } else if let Some(rest) = p.strip_prefix("~/") {
            format!("{}/{rest}", std::env::var("HOME").unwrap_or_default())
        } else {
            p.to_string()
        };
        match std::fs::read_to_string(&expanded) {
            Ok(text) => {
                body.push_str(&format!("\n\n--- {expanded} ---\n\n{text}"));
            }
            Err(_) => skipped += 1,
        }
    }
    // The row's extra instructions ride the same
    // query file — after the md paths, before the todo/planet block.
    if let Some(extra) = shortcut
        .extra
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty())
    {
        body.push_str(&format!("\n\n## Extra instructions\n\n{extra}\n"));
    }
    if shortcut.project_id.is_some() || !titles.is_empty() {
        let name = project_name.unwrap_or("Planet");
        let heading = if go_text.is_some() {
            format!("\n\n## Planet context — {name}\n")
        } else {
            format!("\n\n## Planet context — {name} (titles only, no file contents)\n")
        };
        body.push_str(&heading);
        if titles.is_empty() {
            body.push_str("(no claimed windows or open to-dos)\n");
        } else {
            for t in titles {
                body.push_str(&format!("- {}\n", t.replace('\n', " ")));
            }
        }
    }
    (body, skipped)
}

/// Resolved GO handed to every launch door (pickup, Terminal, herdr, clerk).
pub struct GoLaunch {
    pub parsed: crate::plans::ParsedGo,
    pub run: crate::plans::ResolvedRun,
    pub route: &'static str,
    pub card_id: String,
    pub project_id: String,
}

/// The visible-harness door shared by cursor /
/// goose / grok — a `.command` launcher next to the query file (loads it
/// into `$prompt`), opened as a fresh Terminal window. `exec_line` is the
/// harness's own command line — run as a CHILD, never `exec`: exec would
/// replace the shell and skip the EXIT trap.
/// the trap turns the dock
/// LED off when the harness exits (the same idea as writer-done-needed —
/// a stuck on-light is a leftover, not a feature); HUP (Terminal window
/// closed) exits through the same trap. Silent + best-effort: a missing
/// ctl never noises up the harness session.
/// Per-launch Terminal launcher paths in `context/`:
/// `agent-<shortcut>-<run>.command` / `.pid`. Per RUN, not per shortcut —
/// two runs of one shortcut used to share `agent-<shortcut>.pid`, and the
/// second run's EXIT trap deleted the file the first still needed (boot
/// then called the live first run dead; stop could kill the wrong one).
/// `run` is the task id, else the card id, else a launch nonce.
pub(crate) fn terminal_run_paths(context_dir: &Path, shortcut_id: &str, run: &str) -> (PathBuf, PathBuf) {
    let stem = format!("agent-{}-{}", safe_id(shortcut_id), safe_id(run));
    (
        context_dir.join(format!("{stem}.command")),
        context_dir.join(format!("{stem}.pid")),
    )
}

/// Sibling of the running app binary. Nightly's trap must call Nightly's
/// orbittyctl, never a PATH copy.
pub(crate) fn bundled_orbittyctl() -> PathBuf {
    match std::env::current_exe() {
        Ok(exe) => exe
            .parent()
            .map(|d| d.join("orbittyctl"))
            .unwrap_or_else(|| PathBuf::from("orbittyctl")),
        Err(_) => PathBuf::from("orbittyctl"),
    }
}

pub(crate) fn control_sock_path(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .map(|d| d.join("control.sock"))
        .unwrap_or_else(|_| PathBuf::from("control.sock"))
}

/// Empty setting: Ghostty when the app bundle is installed, else Terminal.
pub(crate) fn resolve_agent_terminal(setting: &str, ghostty_installed: bool) -> &'static str {
    match setting.trim().to_ascii_lowercase().as_str() {
        "ghostty" => "Ghostty",
        "terminal" => "Terminal",
        _ => {
            if ghostty_installed {
                "Ghostty"
            } else {
                "Terminal"
            }
        }
    }
}

pub(crate) fn ghostty_installed() -> bool {
    Path::new("/Applications/Ghostty.app").is_dir()
}

pub(crate) fn open_terminal_argv(app_name: &str, script: &Path) -> Vec<String> {
    vec!["-a".into(), app_name.into(), script.display().to_string()]
}

/// POSIX single-quote one value for a zsh line: `'…'`, each `'` → `'\''`.
/// Nothing inside single quotes expands (`$()`, backticks, `;`, newlines).
/// Same law as `sh_quote` (the Terminal query launcher's).
pub(crate) fn sq(s: &str) -> String {
    sh_quote(s)
}

