//! Today's herdr TUI helpers, moved byte-for-byte. Flag off calls these
//! and nothing else. Only this file may look up the `herdr` binary.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

static BIN_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

pub(crate) fn set_bin_override(path: Option<PathBuf>) {
    *BIN_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = path;
}

pub(crate) fn locate() -> Option<PathBuf> {
    if let Some(p) = BIN_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Some(p);
    }
    crate::coordinator::find_on_path("herdr")
}

/// PATH-only check, the old `which("herdr")`.
#[allow(dead_code)]
pub(crate) fn which_herdr() -> bool {
    let path = std::env::var("PATH").unwrap_or_default();
    for dir in path.split(':') {
        if Path::new(dir).join("herdr").is_file() {
            return true;
        }
    }
    false
}

/// Run one herdr CLI call and parse its JSON envelope. Bounded: the
/// child is killed after `timeout_ms`, so a hung socket never parks the
/// launch door. `Err` carries `code: message` from the error envelope
/// (or the honest transport failure) — the caller decides skip vs
/// blocked; this fn never retries.
pub(crate) fn herdr_cmd(bin: &Path) -> Command {
    let mut c = Command::new(bin);
    let home = std::env::var("HOME").unwrap_or_default();
    let mut path = std::env::var("PATH").unwrap_or_default();
    for p in [
        format!("{home}/.local/bin"),
        format!("{home}/.grok/bin"),
        "/opt/homebrew/bin".into(),
        "/usr/local/bin".into(),
    ] {
        if !p.is_empty() && !path.split(':').any(|x| x == p) {
            path = format!("{p}:{path}");
        }
    }
    c.env("PATH", path);
    if !home.is_empty() {
        c.env("HOME", home);
    }
    c
}

pub(crate) fn run_json(bin: &Path, args: &[&str], timeout_ms: u64) -> Result<Value, String> {
    let mut child = herdr_cmd(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let out = child
                    .wait_with_output()
                    .map_err(|e| format!("output: {e}"))?;
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let text = stdout.trim();
                let v: Value = serde_json::from_str(text).map_err(|e| {
                    let tail = if text.is_empty() {
                        stderr.trim().to_string()
                    } else {
                        text.chars().take(180).collect()
                    };
                    format!("bad json ({}): {e} {tail}", out.status)
                })?;
                if let Some(err) = v.get("error") {
                    let code = err.get("code").and_then(Value::as_str).unwrap_or("error");
                    let msg = err.get("message").and_then(Value::as_str).unwrap_or("");
                    return Err(format!("{code}: {msg}"));
                }
                return Ok(v);
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("timeout".into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("wait: {e}")),
        }
    }
}

/// Server up = `workspace list` answers.
pub(crate) fn server_up(bin: &Path) -> Result<(), String> {
    run_json(bin, &["workspace", "list"], 10_000).map(|_| ())
}

pub(crate) fn is_server_not_running(why: &str) -> bool {
    why.starts_with("server_not_running")
}

/// Leftover pane: if the binary is present and the socket is down, start
/// `herdr server` (headless) once and wait for the socket. Fail → skip,
/// old launcher still fires. Never a nag loop, never the TUI `herdr`.
/// Flag-on hot path does not call this.
pub(crate) fn ensure_server(bin: &Path) -> Result<(), String> {
    match server_up(bin) {
        Ok(()) => return Ok(()),
        Err(e) if is_server_not_running(&e) => {}
        Err(e) => return Err(e),
    }
    let child = Command::new(bin)
        .arg("server")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("herdr server start: {e}"))?;
    std::thread::spawn(move || {
        let mut child = child;
        let _ = child.wait();
    });
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if server_up(bin).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err("server_not_running: started herdr server but the socket never came up".into())
}

/// `herdr agent get <target>` → the pane's agent_status. None = gone /
/// unreadable — the watch's end signal, never a paint.
pub(crate) fn agent_status(bin: &Path, target: &str) -> Option<String> {
    let v = run_json(bin, &["agent", "get", target], 10_000).ok()?;
    v["result"]["agent"]["agent_status"]
        .as_str()
        .map(str::to_string)
}

/// Boot liveness for one herdr agent. `None` = can't tell (no herdr
/// CLI, timeout, spawn failure, bad JSON, any other error) — the caller
/// must treat that as maybe-alive, never as dead. Only two answers mean
/// gone: the server is genuinely not running (panes live in it), or
/// herdr itself says `agent_not_found`.
pub(crate) fn agent_alive(agent_name: &str) -> Option<bool> {
    let bin = if let Some(p) = BIN_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        p
    } else {
        crate::coordinator::find_on_path("herdr")?
    };
    let server = server_up(&bin);
    let get = if server.is_ok() {
        Some(run_json(&bin, &["agent", "get", agent_name], 10_000).map(|_| ()))
    } else {
        None
    };
    alive_from(server, get)
}

/// herdr 0.8.0: `herdr agent get <bogus>` prints
/// `{"error":{"code":"agent_not_found","message":"agent target <x> not found"}}`
/// and exits 1; run_json turns that into `agent_not_found: …`.
fn is_agent_not_found(why: &str) -> bool {
    why.starts_with("agent_not_found")
}

/// Pure verdict over the two probe results (unit-tested).
pub(crate) fn alive_from(server: Result<(), String>, get: Option<Result<(), String>>) -> Option<bool> {
    match server {
        Ok(()) => {}
        Err(e) if is_server_not_running(&e) => return Some(false),
        Err(_) => return None,
    }
    match get? {
        // The pane answered — alive even without an agent_status field.
        Ok(()) => Some(true),
        Err(e) if is_agent_not_found(&e) => Some(false),
        Err(_) => None,
    }
}

/// TUI preview: `herdr agent read <t> --source recent --lines 16`.
pub(crate) fn agent_read(bin: &Path, target: &str) -> Result<String, String> {
    let out = herdr_cmd(bin)
        .args(["agent", "read", target, "--source", "recent", "--lines", "16"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("spawn: {e}"))?;
    let raw = String::from_utf8_lossy(&out.stdout);
    let text = if raw.trim().starts_with('{') {
        serde_json::from_str::<Value>(raw.trim())
            .ok()
            .and_then(|v| {
                v["result"]["text"]
                    .as_str()
                    .or_else(|| v["result"]["output"].as_str())
                    .map(str::to_string)
            })
            .unwrap_or_default()
    } else {
        raw.into_owned()
    };
    Ok(text)
}
