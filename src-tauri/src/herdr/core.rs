//! hc (herdr-core). Exit codes are never trusted: a bare rc is
//! transport-unknown. `error.kind` in the JSON is the verdict.
//! Never `server start|stop|restart`. Writes are `run`, `send`, `wait`,
//! `kill`, `rm` — one `run --bg` per launch, then kill then rm.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::client::cmd;
use super::tui::herdr_cmd;

static BIN_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

pub(crate) fn set_bin_override(path: Option<PathBuf>) {
    *BIN_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = path;
}

/// ORBITTY_HC, the bundled sibling, ~/.local/bin/herdr-core, then PATH `hc`.
/// Tests never fall through to a real core: no override means missing.
pub(crate) fn locate() -> Option<PathBuf> {
    if let Some(p) = BIN_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Some(p);
    }
    #[cfg(test)]
    {
        return None;
    }
    #[cfg(not(test))]
    {
        real_locate()
    }
}

#[cfg(not(test))]
fn real_locate() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ORBITTY_HC") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("herdr-core");
            if bundled.is_file() {
                return Some(bundled);
            }
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let local = PathBuf::from(home).join(".local/bin/herdr-core");
        if local.is_file() {
            return Some(local);
        }
    }
    crate::coordinator::find_on_path("hc")
}

pub(crate) fn command(bin: &Path) -> Command {
    let mut c = herdr_cmd(bin);
    cmd(&mut c);
    c
}

/// Parse hc JSON. Stdout object wins when it has no error. Stderr
/// `{"error":{"kind",...}}` is the error. A non-zero exit with neither
/// is transport failure — the exit code itself is not a verdict.
pub(crate) fn run_json(bin: &Path, args: &[&str], timeout_ms: u64) -> Result<Value, String> {
    let mut child = command(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;
    // Drain both pipes while waiting. A large `hc read` fills the OS pipe
    // if the parent only try_waits, and the child blocks forever.
    let mut stdout = child.stdout.take().ok_or_else(|| "stdout".to_string())?;
    let mut stderr = child.stderr.take().ok_or_else(|| "stderr".to_string())?;
    let out_h = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let err_h = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let _ = child.wait();
                let out = out_h.join().unwrap_or_default();
                let err = err_h.join().unwrap_or_default();
                return classify(&out, &err);
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = out_h.join();
                    let _ = err_h.join();
                    return Err("timeout".into());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(format!("wait: {e}")),
        }
    }
}

fn classify(stdout: &[u8], stderr: &[u8]) -> Result<Value, String> {
    let stdout_s = String::from_utf8_lossy(stdout);
    let stderr_s = String::from_utf8_lossy(stderr);
    let stdout_v = serde_json::from_str::<Value>(stdout_s.trim()).ok();
    let stderr_v = serde_json::from_str::<Value>(stderr_s.trim()).ok();
    if let Some(err) = stderr_v.as_ref().and_then(|v| v.get("error")) {
        return Err(kind_msg(err));
    }
    if let Some(v) = stdout_v {
        if let Some(err) = v.get("error") {
            return Err(kind_msg(err));
        }
        return Ok(v);
    }
    if stderr_s.to_ascii_lowercase().contains("connection refused")
        || stderr_s.to_ascii_lowercase().contains("socket")
    {
        return Err(format!("socket: {}", stderr_s.trim()));
    }
    Err(format!("transport: {}", stderr_s.trim()))
}

fn kind_msg(err: &Value) -> String {
    let kind = err.get("kind").and_then(Value::as_str).unwrap_or("error");
    let msg = err.get("message").and_then(Value::as_str).unwrap_or("");
    format!("{kind}: {msg}")
}

pub(crate) fn server_status(bin: &Path) -> Result<Value, String> {
    let v = run_json(bin, &["server", "status", "--json"], 2_000)?;
    if v.get("error").is_some() {
        return Err("status: error".into());
    }
    match v.get("version").and_then(Value::as_str) {
        Some(ver) if !ver.is_empty() => Ok(v),
        _ => Err("status: no version".into()),
    }
}

/// Stream still open → true. Process already gone → false (stream ended).
pub(crate) fn events_open(bin: &Path) -> bool {
    let mut child = match command(bin)
        .args(["events", "--json", "--label", "orbitty=1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    let mut out = match child.stdout.take() {
        Some(o) => o,
        None => return false,
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8];
        let n = out.read(&mut buf).unwrap_or(0);
        let _ = tx.send(n);
    });
    let open = match rx.recv_timeout(Duration::from_millis(500)) {
        Ok(n) if n > 0 => true,
        _ => false,
    };
    let _ = child.kill();
    let _ = child.wait();
    open
}

pub(crate) fn alive(bin: &Path, name: &str) -> Option<bool> {
    match run_json(bin, &["get", "--json", name], 2_000) {
        Ok(v) => match v.get("state").and_then(Value::as_str) {
            Some("live") => Some(true),
            Some("finished") => Some(false),
            _ if v.get("address").is_some() => Some(true),
            _ => None,
        },
        Err(e) if e.starts_with("not_found") => Some(false),
        Err(_) => None,
    }
}

pub(crate) fn terminal(bin: &Path, name: &str) -> Result<Value, String> {
    run_json(bin, &["get", "--json", name], 2_000)
}

pub(crate) fn list(bin: &Path) -> Result<Value, String> {
    run_json(bin, &["ls", "--json"], 2_000)
}

pub(crate) fn read_text(bin: &Path, name: &str, lines: u32) -> Result<String, String> {
    // `--lines N` is history above the screen (hc default 0). History stalls
    // (~25s) on a permission prompt, past this 2s bound. A timed-out history
    // read is retried once as the screen. The bound is not raised.
    let n = lines.to_string();
    let v = match run_json(bin, &["read", "--json", "--lines", &n, name], 2_000) {
        Ok(v) => v,
        Err(e) if e == "timeout" && lines != 0 => {
            run_json(bin, &["read", "--json", "--lines", "0", name], 2_000)?
        }
        Err(e) => return Err(e),
    };
    if let Some(t) = v.get("text").and_then(Value::as_str) {
        return Ok(t.to_string());
    }
    if let Some(t) = v.get("screen").and_then(Value::as_str) {
        return Ok(t.to_string());
    }
    Ok(String::new())
}

pub(crate) fn map_status(v: &Value) -> String {
    let word = match v.get("status") {
        Some(Value::String(s)) => Some(s.as_str()),
        Some(Value::Object(obj)) => obj.get("state").and_then(Value::as_str),
        _ => None,
    };
    match word {
        Some(s @ ("idle" | "working" | "done" | "blocked" | "error")) => s.to_string(),
        _ => "none".into(),
    }
}

pub(crate) fn map_state(v: &Value) -> String {
    match v.get("state").and_then(Value::as_str) {
        Some(s @ ("live" | "finished")) => s.to_string(),
        _ => String::new(),
    }
}

/// One `hc run --bg`. Returns the terminal address. Does not retry:
/// a second run after an id is a duplicate seat.
pub(crate) fn launch_once(
    bin: &Path,
    cwd: &str,
    card_id: &str,
    planet_id: &str,
    kind: &str,
    extra: &[String],
) -> Result<String, String> {
    let card_l = format!("card={card_id}");
    let planet_l = format!("planet={planet_id}");
    let mut args: Vec<String> = vec![
        "run".into(),
        "--bg".into(),
        "--json".into(),
        "--cwd".into(),
        cwd.into(),
        "--label".into(),
        "orbitty=1".into(),
        "--label".into(),
        card_l,
        "--label".into(),
        planet_l,
        "--".into(),
        kind.into(),
    ];
    args.extend(extra.iter().cloned());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let v = run_json(bin, &refs, 45_000)?;
    match v
        .get("address")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        Some(addr) => Ok(addr.to_string()),
        None => Err("run: no address".into()),
    }
}

pub(crate) fn send(bin: &Path, terminal: &str, text: &str) -> Result<Value, String> {
    run_json(bin, &["send", "--json", terminal, text], 15_000)
}

pub(crate) fn wait(bin: &Path, terminal: &str) -> Result<Value, String> {
    run_json(bin, &["wait", "--json", "--timeout", "60", terminal], 65_000)
}

pub(crate) fn kill_term(bin: &Path, terminal: &str) -> Result<Value, String> {
    run_json(bin, &["kill", "--json", terminal], 8_000)
}

pub(crate) fn rm_term(bin: &Path, terminal: &str) -> Result<Value, String> {
    run_json(bin, &["rm", "--json", terminal], 8_000)
}

/// Kill, then rm. rm only succeeds once the terminal is finished; both
/// are issued. The exit code is not a verdict.
pub(crate) fn stop(bin: &Path, terminal: &str) {
    let _ = kill_term(bin, terminal);
    let _ = rm_term(bin, terminal);
}
