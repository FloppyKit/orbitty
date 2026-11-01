//! Backend pick: flag off → TUI (today). Flag on + healthy → hc.
//! Flag on + DOWN → fallback (unknown, never Died). Hot path never
//! spawns a server.

use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use serde_json::{json, Value};

use super::core;
use super::health::{self, Source};
use super::tui;

static FLAG: Mutex<Option<bool>> = Mutex::new(None);
static CHILD_ENV: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

pub(crate) static TEST_GATE: Mutex<()> = Mutex::new(());

/// Plain line when the flag is on and no herdr-core binary was found.
pub(crate) const MISSING_CORE_LINE: &str =
    "Agent terminals are off: herdr-core isn't installed. Orbitty will use its own launcher.";

pub(crate) fn missing_core_line() -> &'static str {
    MISSING_CORE_LINE
}

/// Test stand-in for `ORBITTY_HC`: this path was an override, not the bundle.
#[cfg(test)]
static LOCATED_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

#[cfg(test)]
pub(crate) fn force_located_override(path: Option<PathBuf>) {
    *LOCATED_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = path;
}

pub(crate) fn force_flag(on: Option<bool>) {
    *FLAG.lock().unwrap_or_else(|e| e.into_inner()) = on;
}

pub(crate) fn set_child_env(pairs: Vec<(String, String)>) {
    *CHILD_ENV.lock().unwrap_or_else(|e| e.into_inner()) = pairs;
}

pub(crate) fn reset_overrides() {
    force_flag(None);
    set_child_env(Vec::new());
    tui::set_bin_override(None);
    core::set_bin_override(None);
    health::reset();
    #[cfg(test)]
    force_located_override(None);
}

/// Apply test child env (fake hc mode and log) onto a command we built.
pub(crate) fn cmd(c: &mut Command) {
    for (k, v) in CHILD_ENV.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        c.env(k, v);
    }
}

pub(crate) fn core_on() -> bool {
    if let Some(v) = *FLAG.lock().unwrap_or_else(|e| e.into_inner()) {
        return v;
    }
    if let Ok(raw) = std::env::var("ORBITTY_FLAG_HERDR_CORE") {
        if let Some(b) = crate::flags::parse_env_bit(&raw) {
            return b;
        }
    }
    #[cfg(test)]
    {
        return false;
    }
    #[cfg(not(test))]
    {
        disk_on()
    }
}

#[cfg(not(test))]
fn disk_on() -> bool {
    if let Ok(eff) = crate::flags::get("herdr_core") {
        if eff.source != crate::flags::Source::Default {
            return eff.on;
        }
    }
    let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) else {
        return false;
    };
    let dir = std::path::PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join(crate::state::instance::STABLE_ID);
    crate::flags::on_in_dir(&dir, "herdr_core")
}

pub(crate) fn locate() -> Option<PathBuf> {
    if core_on() {
        core::locate()
    } else {
        tui::locate()
    }
}

pub(crate) fn available() -> bool {
    locate().is_some()
}

#[allow(dead_code)] // habitat::herdr::herdr_cmd still re-exports this
pub(crate) fn command(bin: &Path) -> Command {
    let mut c = tui::herdr_cmd(bin);
    cmd(&mut c);
    c
}

#[allow(dead_code)] // habitat::herdr::run_json still re-exports this
pub(crate) fn run_json(bin: &Path, args: &[&str], timeout_ms: u64) -> Result<Value, String> {
    if core_on() {
        core::run_json(bin, args, timeout_ms)
    } else {
        tui::run_json(bin, args, timeout_ms)
    }
}

/// Flag on: `hc server status` only. Flag off: today's spawn-once wait.
pub(crate) fn ensure_server(bin: &Path) -> Result<(), String> {
    if core_on() {
        let bin = core::locate().unwrap_or_else(|| bin.to_path_buf());
        core::server_status(&bin).map(|_| ())
    } else {
        tui::ensure_server(bin)
    }
}

fn found_via_override(path: &Path) -> bool {
    #[cfg(test)]
    {
        if let Some(forced) = LOCATED_OVERRIDE
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            if forced == path {
                return true;
            }
        }
    }
    std::env::var_os("ORBITTY_HC").is_some_and(|over| Path::new(&over) == path)
}

fn is_local_bin(path: &Path) -> bool {
    let c: Vec<Component> = path.components().collect();
    let n = c.len();
    n >= 3
        && matches!(c[n - 3], Component::Normal(s) if s == ".local")
        && matches!(c[n - 2], Component::Normal(s) if s == "bin")
        && matches!(c[n - 1], Component::Normal(s) if s == "herdr-core")
}

fn is_bundled_sibling(path: &Path) -> bool {
    path.file_name().is_some_and(|n| n == "herdr-core")
        && path
            .parent()
            .and_then(|p| p.file_name())
            .is_some_and(|n| n == "MacOS")
}

/// End-user boot. `herdr-core server start` once, detached, only when
/// locate found the bundled Contents/MacOS sibling and status is not ok.
/// `~/.local/bin/herdr-core` and an `ORBITTY_HC` override are never started
/// or stopped. The hot path (`ensure_server`) does not call this.
pub(crate) fn boot_server() {
    if !core_on() {
        return;
    }
    let Some(bin) = core::locate() else {
        return;
    };
    if found_via_override(&bin) || is_local_bin(&bin) || !is_bundled_sibling(&bin) {
        return;
    }
    if core::server_status(&bin).is_ok() {
        return;
    }
    // herdr-core server start, once, detached. The exit code is not a verdict.
    let child = match core::command(&bin)
        .args(["server", "start"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            health::force_fallback(health::now_ms(), "transport");
            return;
        }
    };
    std::thread::spawn(move || {
        let mut child = child;
        let _ = child.wait();
    });
}

pub(crate) fn alive(name: &str) -> Option<bool> {
    if !core_on() {
        return tui::agent_alive(name);
    }
    if core::locate().is_none() || health::is_down() {
        if core::locate().is_none() {
            health::force_fallback(health::now_ms(), "transport");
        }
        return None;
    }
    let bin = core::locate()?;
    match core::alive(&bin, name) {
        Some(v) => Some(v),
        None => {
            health::force_fallback(health::now_ms(), "transport");
            None
        }
    }
}

#[allow(dead_code)] // flag-based status; a seated run uses run_status so it sticks
pub(crate) fn agent_status(bin: &Path, target: &str) -> Option<String> {
    if core_on() {
        let b = core::locate().unwrap_or_else(|| bin.to_path_buf());
        core::terminal(&b, target).ok().map(|v| core::map_status(&v))
    } else {
        tui::agent_status(bin, target)
    }
}

/// Wake delivery. `hc send` to the seat label. Missing hc is an error, not a launch.
pub(crate) fn deliver_wake(seat: &str, summary: &str) -> Result<(), String> {
    let bin = core::locate().ok_or_else(|| "hc missing".to_string())?;
    let text = summary.chars().take(200).collect::<String>();
    core::send(&bin, seat, &text).map(|_| ())
}

pub(crate) fn status_value() -> Value {
    if !core_on() {
        let present = tui::locate().is_some();
        return json!({
            "up": present,
            "source": "tui",
            "version": "",
            "status": "none",
        });
    }
    health::poll();
    // up/source follow is_down (source AND faulted), never source alone.
    let down = health::is_down();
    let (up, version) = if !down {
        if let Some(bin) = core::locate() {
            match core::server_status(&bin) {
                Ok(v) => (true, v.get("version").and_then(Value::as_str).unwrap_or("").to_string()),
                Err(_) => (false, String::new()),
            }
        } else {
            (false, String::new())
        }
    } else {
        (false, String::new())
    };
    let line = if core::locate().is_none() {
        missing_core_line()
    } else {
        ""
    };
    json!({
        "up": up && !down,
        "source": if down { Source::Fallback.as_str() } else { Source::Herdr.as_str() },
        "version": version,
        "status": if up { "none" } else { "unknown" },
        "line": line,
    })
}

fn is_core(h: &crate::state::AgentRunHerdr) -> bool {
    h.backend == "core"
}

/// One `hc run --bg` with cwd and orbitty/card/planet labels. No retry.
pub(crate) fn launch_core(
    bin: &Path,
    cwd: &str,
    card_id: &str,
    planet_id: &str,
    kind: &str,
    extra: &[String],
) -> Result<String, String> {
    core::launch_once(bin, cwd, card_id, planet_id, kind, extra)
}

pub(crate) fn workspace_create(bin: &Path, cwd: &str, label: &str) -> Result<serde_json::Value, String> {
    tui::workspace_create(bin, cwd, label)
}

pub(crate) fn agent_start(
    bin: &Path,
    name: &str,
    kind: &str,
    pane: &str,
    extra: &[String],
) -> Result<serde_json::Value, String> {
    let owned = tui::start_argv(name, kind, pane, extra);
    let args: Vec<&str> = owned.iter().map(String::as_str).collect();
    tui::agent_start(bin, &args)
}

pub(crate) fn prompt_with(
    bin: &Path,
    h: &crate::state::AgentRunHerdr,
    text: &str,
) -> Result<serde_json::Value, String> {
    if is_core(h) {
        core::send(bin, h.terminal.trim(), text)
    } else {
        tui::agent_prompt(bin, &h.agent_name, text)
    }
}

pub(crate) fn wait_run(
    bin: &Path,
    h: &crate::state::AgentRunHerdr,
    status: Option<&str>,
) -> Result<serde_json::Value, String> {
    if is_core(h) {
        core::wait(bin, h.terminal.trim())
    } else {
        let args = tui::wait_args(&h.agent_name, status);
        tui::agent_wait(bin, &args)
    }
}

pub(crate) fn run_status(bin: &Path, h: &crate::state::AgentRunHerdr) -> Option<String> {
    if is_core(h) {
        let id = if h.terminal.trim().is_empty() {
            h.agent_name.as_str()
        } else {
            h.terminal.trim()
        };
        core::terminal(bin, id).ok().map(|v| core::map_status(&v))
    } else {
        tui::agent_status(bin, &h.agent_name)
    }
}

/// The run's own backend, never the live flag.
pub(crate) fn stop(h: &crate::state::AgentRunHerdr) {
    if is_core(h) {
        let Some(bin) = core::locate() else {
            return;
        };
        let t = h.terminal.trim();
        if !t.is_empty() {
            core::stop(&bin, t);
        }
        return;
    }
    let Some(bin) = tui::locate() else {
        return;
    };
    if !h.pane.trim().is_empty() {
        let _ = tui::pane_close(&bin, h.pane.trim());
    }
    if !h.workspace.trim().is_empty() {
        let _ = tui::workspace_close(&bin, h.workspace.trim());
    }
}

pub(crate) fn read_recent(h: &crate::state::AgentRunHerdr) -> Result<String, String> {
    if is_core(h) {
        let bin = core::locate().ok_or_else(|| "hc missing".to_string())?;
        let id = if h.terminal.trim().is_empty() {
            h.agent_name.as_str()
        } else {
            h.terminal.trim()
        };
        core::read_text(&bin, id, 16)
    } else {
        let bin = tui::locate().ok_or_else(|| "herdr missing".to_string())?;
        tui::agent_read(&bin, &h.agent_name)
    }
}

/// TUI focus is today's workspace + agent focus, then the TUI.
/// Core has no confirmed deep link (TO CONFIRM); focus is a no-op.
pub(crate) fn focus(h: &crate::state::AgentRunHerdr) {
    if is_core(h) {
        return;
    }
    let Some(bin) = tui::locate() else {
        return;
    };
    let _ = tui::focus_workspace(&bin, &h.workspace);
    let _ = tui::focus_agent(&bin, &h.agent_name);
    tui::open_tui(&bin);
}
