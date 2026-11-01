//! Open the real terminal for one seat.
//! A core seat gets a window whose only line is `hc attach`.
//! Watch-only unless the caller asks for full attach.
//! A tui seat is focused, then a window runs the bare binary: a null-stdio
//! launch exits at once and shows nothing (S0).
//! This file never starts a core server and never types into the seat.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::state::{AgentRun, AgentRunHerdr};

use super::client;
use super::core;
use super::health;
use super::tui;

const NOTHING: &str = "Nothing to open for this card.";
const FINISHED: &str = "This seat is finished.";
const UNAVAILABLE: &str = "This seat's terminal isn't available right now.";
const OPEN_FAIL: &str = "Couldn't open the terminal.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Observe,
    Full,
}

impl Mode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Mode::Observe => "observe",
            Mode::Full => "full",
        }
    }

    pub(crate) fn from_observe(observe: Option<bool>) -> Self {
        match observe {
            Some(false) => Mode::Full,
            _ => Mode::Observe,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Plan {
    Core { terminal: String },
    Tui { workspace: String, agent: String },
    Refuse { reason: String },
}

/// Seat choice follows the run that started it. `mode` only changes the
/// attach line, never which seat is chosen.
pub(crate) fn plan(run: Option<&AgentRun>, mode: Mode) -> Plan {
    let _mode = mode;
    let Some(run) = run else {
        return Plan::Refuse {
            reason: NOTHING.into(),
        };
    };
    if is_finished(run) {
        return Plan::Refuse {
            reason: FINISHED.into(),
        };
    }
    let Some(h) = run.herdr.as_ref() else {
        return Plan::Refuse {
            reason: NOTHING.into(),
        };
    };
    if h.backend == "core" {
        let terminal = terminal_id(h);
        if terminal.is_empty() {
            return Plan::Refuse {
                reason: NOTHING.into(),
            };
        }
        return Plan::Core { terminal };
    }
    if h.backend == "tui" || h.backend.trim().is_empty() {
        return Plan::Tui {
            workspace: h.workspace.clone(),
            agent: h.agent_name.clone(),
        };
    }
    Plan::Refuse {
        reason: NOTHING.into(),
    }
}

fn is_finished(run: &AgentRun) -> bool {
    matches!(
        run.state.trim().to_ascii_lowercase().as_str(),
        "done" | "finished"
    )
}

fn terminal_id(h: &AgentRunHerdr) -> String {
    let terminal = h.terminal.trim();
    if !terminal.is_empty() {
        return terminal.to_string();
    }
    h.agent_name.trim().to_string()
}

pub(crate) fn script_text(hc: &str, terminal: &str, mode: Mode) -> String {
    let bin = crate::coordinator::sq(hc);
    let id = crate::coordinator::sq(terminal);
    let line = match mode {
        Mode::Observe => format!("{bin} attach --observe {id}"),
        Mode::Full => format!("{bin} attach {id}"),
    };
    format!("#!/bin/zsh\n{line}\n")
}

pub(crate) fn tui_script_text(bin: &str) -> String {
    format!("#!/bin/zsh\n{}\n", crate::coordinator::sq(bin))
}

pub(crate) struct Prepared {
    pub ok: bool,
    pub backend: String,
    pub mode: String,
    pub terminal: Option<String>,
    pub reason: Option<String>,
    pub body: Option<String>,
    pub workspace: String,
    pub agent: String,
}

fn refuse(backend: &str, mode: &str, reason: impl Into<String>) -> Prepared {
    Prepared {
        ok: false,
        backend: backend.into(),
        mode: mode.into(),
        terminal: None,
        reason: Some(reason.into()),
        body: None,
        workspace: String::new(),
        agent: String::new(),
    }
}

/// Decide the jump. Does not write a file and does not open a window.
pub(crate) fn prepare(run: Option<&AgentRun>, mode: Mode) -> Prepared {
    match plan(run, mode) {
        Plan::Refuse { reason } => refuse("", mode.as_str(), reason),
        Plan::Core { terminal } => {
            let Some(bin) = core::locate() else {
                return refuse("core", mode.as_str(), client::missing_core_line());
            };
            if health::is_down() {
                health::poll();
                if health::is_down() {
                    return refuse("core", mode.as_str(), UNAVAILABLE);
                }
            }
            Prepared {
                ok: true,
                backend: "core".into(),
                mode: mode.as_str().into(),
                terminal: Some(terminal.clone()),
                reason: None,
                body: Some(script_text(&bin.display().to_string(), &terminal, mode)),
                workspace: String::new(),
                agent: String::new(),
            }
        }
        Plan::Tui { workspace, agent } => {
            let Some(bin) = tui::locate() else {
                return refuse("tui", "focus", UNAVAILABLE);
            };
            Prepared {
                ok: true,
                backend: "tui".into(),
                mode: "focus".into(),
                terminal: None,
                reason: None,
                body: Some(tui_script_text(&bin.display().to_string())),
                workspace,
                agent,
            }
        }
    }
}

pub(crate) fn find_run<'a>(runs: &'a [AgentRun], token: &str) -> Option<&'a AgentRun> {
    let id = token
        .trim()
        .strip_prefix("agent:")
        .unwrap_or(token.trim())
        .trim();
    if id.is_empty() {
        return None;
    }
    if let Some(run) = runs.iter().find(|r| r.card_id == id) {
        return Some(run);
    }
    runs.iter().find(|r| {
        r.herdr
            .as_ref()
            .is_some_and(|h| h.terminal.trim() == id || h.agent_name.trim() == id)
    })
}

pub(crate) fn command_path(dir: &Path, card: &str) -> PathBuf {
    let stem = crate::coordinator::safe_id(card);
    let stem = if stem.is_empty() {
        "seat".to_string()
    } else {
        stem
    };
    dir.join(format!("jump-{stem}.command"))
}

pub(crate) fn write_command(dir: &Path, card: &str, body: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let path = command_path(dir, card);
    fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = fs::metadata(&path)
            .map_err(|e| format!("stat {}: {e}", path.display()))?
            .permissions();
        perm.set_mode(0o700);
        fs::set_permissions(&path, perm).map_err(|e| format!("mode {}: {e}", path.display()))?;
    }
    Ok(path)
}

pub(crate) fn terminal_argv(app_name: &str, script: &Path) -> Vec<String> {
    crate::coordinator::open_terminal_argv(app_name, script)
}

fn reveal(app_name: &str, script: &Path) -> bool {
    let argv = terminal_argv(app_name, script);
    let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    #[cfg(test)]
    {
        let _kept = refs;
        !app_name.is_empty() && !script.as_os_str().is_empty()
    }
    #[cfg(not(test))]
    {
        crate::coordinator::spawn_detached("open", &refs)
    }
}

fn context_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = crate::coordinator::app_data(app)?.join("context");
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn focus_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        None
    }
    #[cfg(not(test))]
    {
        let root = crate::state::instance::support_dir(crate::state::instance::STABLE_ID).ok()?;
        let dir = root.join("context");
        fs::create_dir_all(&dir).ok()?;
        Some(dir)
    }
}

fn to_value(p: &Prepared) -> Value {
    let mut v = json!({
        "ok": p.ok,
        "backend": p.backend,
        "mode": p.mode,
    });
    if let Some(t) = &p.terminal {
        v["terminal"] = json!(t);
    }
    if let Some(r) = &p.reason {
        v["reason"] = json!(r);
    }
    v
}

fn log_jump(app: &AppHandle, card: &str, planet: Option<&str>, p: &Prepared) {
    crate::event_log::append_best(
        app,
        "herdr-jump",
        None,
        planet,
        Some("agent"),
        true,
        &json!({
            "card": card,
            "backend": p.backend,
            "mode": p.mode,
            "ok": p.ok,
        }),
    );
}

fn open_prepared(
    app: &AppHandle,
    card: &str,
    setting: &str,
    prepared: &Prepared,
) -> Result<(), String> {
    let body = prepared
        .body
        .as_deref()
        .ok_or_else(|| OPEN_FAIL.to_string())?;
    if prepared.backend == "tui" {
        if let Some(bin) = tui::locate() {
            let workspace = prepared.workspace.trim();
            if !workspace.is_empty() {
                let _ = tui::focus_workspace(&bin, workspace);
            }
            let agent = prepared.agent.trim();
            if !agent.is_empty() {
                let _ = tui::focus_agent(&bin, agent);
            }
        }
    }
    let dir = context_dir(app).map_err(|_| OPEN_FAIL.to_string())?;
    let path = write_command(&dir, card, body).map_err(|_| OPEN_FAIL.to_string())?;
    let app_name = crate::coordinator::resolve_agent_terminal(
        setting,
        crate::coordinator::ghostty_installed(),
    );
    if reveal(app_name, &path) {
        Ok(())
    } else {
        Err(OPEN_FAIL.into())
    }
}

pub(crate) fn jump_for(app: &AppHandle, card: &str, observe: Option<bool>) -> Value {
    let mode = Mode::from_observe(observe);
    let requested = card
        .trim()
        .strip_prefix("agent:")
        .unwrap_or(card.trim())
        .trim();
    let loaded = crate::state::load_state(app).ok();
    let runs = loaded
        .as_ref()
        .map(|st| st.agent_runs.clone())
        .unwrap_or_default();
    let setting = loaded
        .as_ref()
        .map(|st| st.agent_terminal.clone())
        .unwrap_or_default();
    let found = find_run(&runs, requested);
    let mut prepared = prepare(found, mode);
    let file_card = found
        .map(|r| r.card_id.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(requested);
    if prepared.ok {
        if let Err(reason) = open_prepared(app, file_card, &setting, &prepared) {
            prepared.ok = false;
            prepared.reason = Some(reason);
            prepared.body = None;
        }
    }
    let event_card = if file_card.is_empty() {
        requested
    } else {
        file_card
    };
    let planet = found
        .map(|r| r.planet_id.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    log_jump(app, event_card, planet.as_deref(), &prepared);
    to_value(&prepared)
}

/// Core branch of `client::focus`. No app handle on that path, so the
/// terminal app is the default (empty setting) and no event is written.
pub(crate) fn focus_core(h: &AgentRunHerdr) {
    let card = terminal_id(h);
    let run = AgentRun {
        card_id: card.clone(),
        state: "working".into(),
        herdr: Some(h.clone()),
        ..AgentRun::default()
    };
    let prepared = prepare(Some(&run), Mode::Observe);
    if !prepared.ok {
        return;
    }
    let Some(dir) = focus_dir() else {
        return;
    };
    let Some(body) = prepared.body.as_deref() else {
        return;
    };
    let Ok(path) = write_command(&dir, &card, body) else {
        return;
    };
    let app_name =
        crate::coordinator::resolve_agent_terminal("", crate::coordinator::ghostty_installed());
    let _ = reveal(app_name, &path);
}
