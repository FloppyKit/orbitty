//! Backend pick: flag off → TUI (today). Flag on + healthy → hc.
//! Flag on + DOWN → fallback (unknown, never Died). Hot path never
//! spawns a server.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use serde_json::{json, Value};

use super::core;
use super::health::{self, Source};
use super::tui;

static FLAG: Mutex<Option<bool>> = Mutex::new(None);
static CHILD_ENV: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

pub(crate) static TEST_GATE: Mutex<()> = Mutex::new(());

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

pub(crate) fn command(bin: &Path) -> Command {
    let mut c = tui::herdr_cmd(bin);
    cmd(&mut c);
    c
}

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

pub(crate) fn alive(name: &str) -> Option<bool> {
    if !core_on() {
        return tui::agent_alive(name);
    }
    if core::locate().is_none() || health::is_down() {
        health::force_fallback(health::now_ms());
        return None;
    }
    let bin = core::locate()?;
    match core::alive(&bin, name) {
        Some(v) => Some(v),
        None => {
            health::force_fallback(health::now_ms());
            None
        }
    }
}

pub(crate) fn agent_status(bin: &Path, target: &str) -> Option<String> {
    if core_on() {
        let b = core::locate().unwrap_or_else(|| bin.to_path_buf());
        core::terminal(&b, target).ok().map(|v| core::map_status(&v))
    } else {
        tui::agent_status(bin, target)
    }
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
    let src = health::source();
    let (up, version) = if src == Source::Herdr {
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
    json!({
        "up": up && src == Source::Herdr,
        "source": src.as_str(),
        "version": version,
        "status": if up { "none" } else { "unknown" },
    })
}
