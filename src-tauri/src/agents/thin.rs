//! Dev-only THIN stamp.
//!
//! Built in only for debug builds or with the `dev-thin` Cargo feature
//! (`install-nightly.sh` builds Nightly with it). Stable/consumer release
//! builds refuse before any stamp, nod, or file write, and do not seed ⌘⌥T. Script, python, and output directory come from
//! `~/.config/orbitty/thin.json` when set; the defaults are the repo script,
//! `python3` on PATH, and `~/.config/orbitty/thin-stamps`. Nothing here
//! hard-codes a research-nest path or an absolute Homebrew interpreter.
//!
//! The turn count is per plug session. It starts at the visible window, then
//! advances by whichever is larger: newly seen turns, or `companion-send` /
//! `companion-reply` events since the last stamp. A slide past the 40-cap
//! does not freeze the count on the list index.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// The one THIN build predicate. The release gate (`companion_thin`) and the
/// ⌘⌥T seeding (`actions::bake_defaults`) both pass this, and every compiled-in
/// THIN item uses the same `cfg(any(debug_assertions, feature = "dev-thin"))`.
pub const THIN_ENABLED: bool = cfg!(any(debug_assertions, feature = "dev-thin"));

/// Builds without THIN do not seed the ⌘⌥T row.
pub fn seed_thin_row(enabled: bool) -> bool {
    enabled
}

/// Builds without THIN stop here; this gate is what the command checks first.
pub fn release_gate(enabled: bool) -> Result<(), String> {
    if enabled {
        Ok(())
    } else {
        Err("THIN is dev-only and is not in this build.".into())
    }
}

/// Button `companion_thin` and the thin shortcut both land here.
/// Name `thin`, a legacy `vivo_thin.py` command, or the `companion thin` verb.
/// A longer word such as "thinking" does not match.
pub fn routes_to_companion_thin(name: &str, command: &str) -> bool {
    if name.trim().eq_ignore_ascii_case("thin") {
        return true;
    }
    let c = command.to_ascii_lowercase();
    if c.contains("vivo_thin.py") {
        return true;
    }
    let parts: Vec<&str> = c
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '.' && ch != '_')
        .filter(|s| !s.is_empty())
        .collect();
    parts.iter().any(|p| *p == "companion.thin")
        || parts
            .windows(2)
            .any(|w| w[0] == "companion" && w[1] == "thin")
}

/// `companion_thin`'s body, with the app parts injected: the gate first
/// (nothing else runs when it refuses), then the stamp, then exactly one nod
/// for the attempt, then a note row only on failure.
pub(crate) fn thin_flow(
    gate: Result<(), String>,
    stamp: impl FnOnce() -> Result<String, StampError>,
    mut nod: impl FnMut(),
    note: impl FnOnce(String) -> Result<(), String>,
) -> Result<String, String> {
    gate?;
    let stamped = stamp();
    nod();
    match stamped {
        Ok(line) => Ok(line),
        Err(e) => {
            let line = e.user_line();
            note(line.clone())?;
            Ok(line)
        }
    }
}

/// The tail of a stamp: run the script, require the stamp to have landed,
/// and only then advance the meter.
#[cfg_attr(not(any(debug_assertions, feature = "dev-thin")), allow(dead_code))]
pub(crate) fn stamp_steps(
    run: impl FnOnce() -> Result<String, StampError>,
    landed: impl FnOnce() -> bool,
    save_meter: impl FnOnce(),
) -> Result<String, StampError> {
    let line = run()?;
    if !landed() {
        return Err(StampError::failed("stamp script exited but no stamp landed"));
    }
    save_meter();
    Ok(line)
}

/// True for `~/.buzz` or anything inside it: file identity (dev, ino) of the
/// path and each existing parent against `~/.buzz` (or HOME with a `.buzz`
/// child before it exists), dangling symlinks judged by their target, then
/// the canonical path compared case-insensitively. Any error other than
/// not-found counts as inside.
#[cfg_attr(not(any(debug_assertions, feature = "dev-thin")), allow(dead_code))]
pub(crate) fn inside_buzz(home: &Path, p: &Path) -> bool {
    inside_buzz_at(home, p, 0)
}

#[cfg_attr(not(any(debug_assertions, feature = "dev-thin")), allow(dead_code))]
fn inside_buzz_at(home: &Path, p: &Path, depth: u32) -> bool {
    use std::os::unix::fs::MetadataExt;
    if depth > 32 || !p.is_absolute() {
        return true;
    }
    let ident = |q: &Path| -> Result<Option<(u64, u64)>, ()> {
        match std::fs::metadata(q) {
            Ok(m) => Ok(Some((m.dev(), m.ino()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(()),
        }
    };
    let buzz_dir = home.join(".buzz");
    let (Ok(buzz), Ok(home_id)) = (ident(&buzz_dir), ident(home)) else {
        return true;
    };
    let hit = |id: (u64, u64), child: Option<&str>| {
        Some(id) == buzz
            || (Some(id) == home_id && child.map(str::to_lowercase).as_deref() == Some(".buzz"))
    };
    let mut child: Option<String> = None;
    let mut cur = Some(p);
    let mut resolved = false;
    while let Some(q) = cur {
        match ident(q) {
            Err(()) => return true,
            Ok(Some(id)) => {
                if hit(id, child.as_deref()) {
                    return true;
                }
                // The first existing ancestor: walk its real (symlink-free)
                // parents too, so a symlinked parent into .buzz is caught.
                if !resolved {
                    resolved = true;
                    let Ok(real) = std::fs::canonicalize(q) else {
                        return true;
                    };
                    let mut rchild: Option<String> = None;
                    let mut rcur = Some(real.as_path());
                    while let Some(r) = rcur {
                        match ident(r) {
                            Err(()) => return true,
                            Ok(Some(rid)) if hit(rid, rchild.as_deref()) => return true,
                            _ => {}
                        }
                        rchild = r.file_name().map(|s| s.to_string_lossy().into_owned());
                        rcur = r.parent();
                    }
                }
            }
            Ok(None) => {
                if let Ok(m) = std::fs::symlink_metadata(q) {
                    if m.file_type().is_symlink() {
                        let Ok(t) = std::fs::read_link(q) else {
                            return true;
                        };
                        let t = q.parent().map(|d| d.join(&t)).unwrap_or(t);
                        if inside_buzz_at(home, &t, depth + 1) {
                            return true;
                        }
                    }
                }
            }
        }
        child = q.file_name().map(|s| s.to_string_lossy().into_owned());
        cur = q.parent();
    }
    let lower = |q: &Path| {
        std::fs::canonicalize(q)
            .unwrap_or_else(|_| q.to_path_buf())
            .to_string_lossy()
            .to_lowercase()
    };
    let b = lower(&buzz_dir);
    let b = b.trim_end_matches('/');
    let c = lower(p);
    c == b || c.starts_with(&format!("{b}/"))
}

/// `thin.json` may not send stamps (or take the script) from inside
/// `~/.buzz`, and paths must be absolute after `~/` / `$HOME` expansion
/// (a relative path would land in the app's cwd, `/`).
#[cfg_attr(not(any(debug_assertions, feature = "dev-thin")), allow(dead_code))]
pub(crate) fn check_paths(home: &Path, cfg: &ThinConfig) -> Result<(), String> {
    let mut checks: Vec<(&str, &Path)> =
        vec![("outputDir", cfg.output_dir.as_path()), ("script", cfg.script.as_path())];
    // A bare interpreter name (`python3`) is looked up on PATH; anything with
    // a `/` is a path and gets the same checks as the others.
    let python = PathBuf::from(&cfg.python);
    if cfg.python.contains('/') {
        checks.push(("python", python.as_path()));
    }
    for (what, p) in checks {
        // `..` can walk out of a missing folder into ~/.buzz before any
        // component exists to stat, so it is refused outright.
        if p
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(format!("thin.json {what} {} may not contain ..", p.display()));
        }
        if !p.is_absolute() {
            return Err(format!("thin.json {what} {} must be an absolute path", p.display()));
        }
        if inside_buzz(home, p) {
            return Err(format!(
                "thin.json {what} {} is inside ~/.buzz; refused",
                p.display()
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct ThinConfig {
    pub script: PathBuf,
    pub python: String,
    pub output_dir: PathBuf,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ThinFile {
    #[serde(default)]
    script: Option<String>,
    #[serde(default)]
    python: Option<String>,
    #[serde(default)]
    output_dir: Option<String>,
}

pub fn resolve_config(home: &Path, manifest_dir: &Path) -> ThinConfig {
    let file = read_thin_file(home);
    let script = nonblank(file.script.as_deref())
        .map(|s| expand_home(home, s))
        // No `..` in the default: check_paths refuses any path that has one.
        .unwrap_or_else(|| {
            manifest_dir
                .parent()
                .unwrap_or(manifest_dir)
                .join("scripts/thin/vivo_thin.py")
        });
    // `~/…` and `$HOME` expand like the other paths; a bare name stays a name.
    let python = match nonblank(file.python.as_deref()) {
        Some(s) if s.starts_with("~/") || s.contains("$HOME") => {
            expand_home(home, s).to_string_lossy().into_owned()
        }
        Some(s) => s.to_string(),
        None => "python3".to_string(),
    };
    let output_dir = nonblank(file.output_dir.as_deref())
        .map(|s| expand_home(home, s))
        .unwrap_or_else(|| home.join(".config/orbitty/thin-stamps"));
    ThinConfig {
        script,
        python,
        output_dir,
    }
}

fn read_thin_file(home: &Path) -> ThinFile {
    let p = home.join(".config/orbitty/thin.json");
    let Ok(raw) = std::fs::read_to_string(p) else {
        return ThinFile::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn nonblank(raw: Option<&str>) -> Option<&str> {
    raw.map(str::trim).filter(|s| !s.is_empty())
}

fn expand_home(home: &Path, raw: &str) -> PathBuf {
    if raw == "~" {
        return home.to_path_buf();
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return home.join(rest);
    }
    let home_s = home.to_string_lossy();
    PathBuf::from(raw.replace("$HOME", home_s.as_ref()))
}

/// Chars of the live thread, and a tokens estimate (chars/4, round up).
pub fn context_size(texts: &[String]) -> (u64, u64) {
    let chars: u64 = texts.iter().map(|t| t.chars().count() as u64).sum();
    let tokens = chars.saturating_add(3) / 4;
    (chars, tokens)
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub(crate) struct Meter {
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    turn_count: u64,
    #[serde(default)]
    reset_at: u64,
    #[serde(default)]
    last_reset_reason: String,
    #[serde(default)]
    lines_seen: usize,
    /// Oldest-to-newest fingerprints of the last observed window.
    #[serde(default)]
    window: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub session_id: String,
    pub turn_count: u64,
    pub secs_since_reset: u64,
    pub last_reset_reason: String,
    pub reset_at: u64,
}

/// `new_events` is companion-send + companion-reply lines after `prev.lines_seen`.
/// `lines_seen` is the log length after this read (the new cursor).
pub(crate) fn reconcile(
    prev: &Meter,
    session_id: &str,
    window: &[String],
    new_events: u64,
    lines_seen: usize,
    now: u64,
) -> (Meter, Observation) {
    let session_id = if session_id.trim().is_empty() {
        "unbound".to_string()
    } else {
        session_id.to_string()
    };
    let (turn_count, reset_at, reason) = if prev.session_id.is_empty() {
        (window.len() as u64, now, "meter_start".to_string())
    } else if prev.session_id != session_id {
        (
            std::cmp::max(window.len() as u64, new_events),
            now,
            "session_changed".to_string(),
        )
    } else {
        let added = std::cmp::max(window_added(&prev.window, window), new_events);
        (
            prev.turn_count.saturating_add(added),
            prev.reset_at,
            prev.last_reset_reason.clone(),
        )
    };
    let meter = Meter {
        session_id: session_id.clone(),
        turn_count,
        reset_at,
        last_reset_reason: reason.clone(),
        lines_seen,
        window: window.to_vec(),
    };
    let obs = Observation {
        session_id,
        turn_count,
        secs_since_reset: now.saturating_sub(reset_at),
        last_reset_reason: reason,
        reset_at,
    };
    (meter, obs)
}

fn window_added(prev: &[String], next: &[String]) -> u64 {
    if next.is_empty() {
        return 0;
    }
    let max_k = prev.len().min(next.len());
    let mut overlap = 0;
    for k in (1..=max_k).rev() {
        if prev[prev.len() - k..] == next[..k] {
            overlap = k;
            break;
        }
    }
    (next.len() - overlap) as u64
}

pub fn fingerprint(role: &str, text: &str) -> String {
    let mut h = Sha256::new();
    h.update(role.as_bytes());
    h.update([0xff]);
    h.update(text.as_bytes());
    h.finalize()
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Count mouth turns in `events.jsonl` strictly after `lines_seen`.
/// Returns (new turn events, total lines).
pub fn count_turn_events(text: &str, lines_seen: usize) -> (u64, usize) {
    let all: Vec<&str> = text.lines().collect();
    if lines_seen >= all.len() {
        return (0, all.len());
    }
    let mut n = 0u64;
    for line in all.iter().skip(lines_seen) {
        if line_is_turn(line) {
            n += 1;
        }
    }
    (n, all.len())
}

fn line_is_turn(line: &str) -> bool {
    if !line.contains("companion-send") && !line.contains("companion-reply") {
        return false;
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    let Some(tags) = v.get("tags").and_then(|t| t.as_array()) else {
        return false;
    };
    tags.iter().any(|tag| {
        let Some(arr) = tag.as_array() else {
            return false;
        };
        arr.first().and_then(|s| s.as_str()) == Some("e-type")
            && matches!(
                arr.get(1).and_then(|s| s.as_str()),
                Some("companion-send" | "companion-reply")
            )
    })
}

pub struct StampError {
    user_line: String,
}

impl StampError {
    pub fn user_line(&self) -> String {
        self.user_line.clone()
    }

    fn missing_script() -> Self {
        Self {
            user_line: "THIN stamped (scanner missing). Start a new job when ready.".into(),
        }
    }

    fn failed(err: &str) -> Self {
        let err = err.trim().replace('\n', " ");
        let err: String = err.chars().take(160).collect();
        Self {
            user_line: format!("THIN stamp failed. {err}"),
        }
    }
}

#[cfg(any(debug_assertions, feature = "dev-thin"))]
pub fn stamp(app: &tauri::AppHandle) -> Result<String, StampError> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = resolve_config(&home, &manifest);
    check_paths(&home, &cfg).map_err(|e| StampError::failed(&e))?;
    if !cfg.script.is_file() {
        return Err(StampError::missing_script());
    }
    let st = crate::state::load_state(app).map_err(|e| StampError::failed(&e))?;
    let texts: Vec<String> = st.companion.turns.iter().map(|t| t.text.clone()).collect();
    let (chars, tokens) = context_size(&texts);
    let window: Vec<String> = st
        .companion
        .turns
        .iter()
        .map(|t| fingerprint(&t.role, &t.text))
        .collect();
    let turns: Vec<Value> = st
        .companion
        .turns
        .iter()
        .map(|t| json!({ "role": t.role, "text": t.text }))
        .collect();
    let session_id = read_session_id();
    let events_text = read_events(app);
    let meter = load_meter(&cfg.output_dir);
    let (new_events, cursor) = count_turn_events(&events_text, meter.lines_seen);
    let now = unix_now();
    let (meter, obs) = reconcile(&meter, &session_id, &window, new_events, cursor, now);
    let payload = json!({
        "session_id": obs.session_id,
        "turn_count": obs.turn_count,
        "context_chars": chars,
        "context_tokens_est": tokens,
        "secs_since_reset": obs.secs_since_reset,
        "last_reset_reason": obs.last_reset_reason,
        "ts": stamp_ts(now),
        "mode": st.companion.mode,
        "turns": turns,
    });
    let body = serde_json::to_string(&payload).map_err(|e| StampError::failed(&e.to_string()))?;
    std::fs::create_dir_all(&cfg.output_dir)
        .map_err(|e| StampError::failed(&format!("mkdir {}: {e}", cfg.output_dir.display())))?;
    stamp_steps(
        || run_script(&cfg, &body),
        || stamp_landed(&cfg.output_dir, &obs),
        || save_meter(&cfg.output_dir, &meter),
    )
}

#[cfg(not(any(debug_assertions, feature = "dev-thin")))]
pub fn stamp(_app: &tauri::AppHandle) -> Result<String, StampError> {
    Err(StampError {
        user_line: "THIN is dev-only and is not in this build.".into(),
    })
}

#[cfg(any(debug_assertions, feature = "dev-thin"))]
fn run_script(cfg: &ThinConfig, body: &str) -> Result<String, StampError> {
    let mut child = Command::new(&cfg.python)
        .arg(&cfg.script)
        .arg("stamp")
        .env("ORBITTY_THIN_OUTPUT", &cfg.output_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| StampError::failed(&format!("start {}: {e}", cfg.python)))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(body.as_bytes())
            .map_err(|e| StampError::failed(&format!("payload: {e}")))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| StampError::failed(&format!("wait: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let extra = String::from_utf8_lossy(&out.stdout);
        return Err(StampError::failed(&format!("{err} {extra}")));
    }
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if stdout.is_empty() {
        Err(StampError::failed("stamp script wrote no summary"))
    } else {
        Ok(stdout)
    }
}

#[cfg(any(debug_assertions, feature = "dev-thin"))]
fn stamp_landed(output_dir: &Path, obs: &Observation) -> bool {
    let events = output_dir.join("events.jsonl");
    let Ok(raw) = std::fs::read_to_string(&events) else {
        return false;
    };
    let Some(line) = raw.lines().rev().find(|l| !l.trim().is_empty()) else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    v.get("session_id").and_then(|s| s.as_str()) == Some(obs.session_id.as_str())
        && v.get("turn_count").and_then(|n| n.as_u64()) == Some(obs.turn_count)
        && v.get("context_chars").is_some()
        && v.get("context_tokens_est").is_some()
        && v.get("secs_since_reset").is_some()
        && v.get("last_reset_reason").and_then(|s| s.as_str())
            == Some(obs.last_reset_reason.as_str())
        && v.get("ts")
            .and_then(|s| s.as_str())
            .is_some_and(|s| !s.is_empty())
}

#[cfg(any(debug_assertions, feature = "dev-thin"))]
fn read_events(app: &tauri::AppHandle) -> String {
    use tauri::Manager;
    let Ok(dir) = app.path().app_data_dir() else {
        return String::new();
    };
    std::fs::read_to_string(dir.join("events.jsonl")).unwrap_or_default()
}

#[cfg(any(debug_assertions, feature = "dev-thin"))]
fn read_session_id() -> String {
    let p = crate::coordinator::companions_root().join("plug.json");
    let Ok(raw) = std::fs::read_to_string(p) else {
        return "unbound".into();
    };
    let Ok(v) = serde_json::from_str::<Value>(&raw) else {
        return "unbound".into();
    };
    let grok = v
        .get("grok_session_id")
        .and_then(|s| s.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let goose = v
        .get("goose_session_id")
        .and_then(|s| s.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match (grok, goose) {
        (Some(g), Some(o)) => format!("grok:{g}|goose:{o}"),
        (Some(g), None) => format!("grok:{g}"),
        (None, Some(o)) => format!("goose:{o}"),
        (None, None) => "unbound".into(),
    }
}

fn meter_path(output_dir: &Path) -> PathBuf {
    output_dir.join("meter.json")
}

fn load_meter(output_dir: &Path) -> Meter {
    let Ok(raw) = std::fs::read_to_string(meter_path(output_dir)) else {
        return Meter::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_meter(output_dir: &Path, meter: &Meter) {
    let Ok(body) = serde_json::to_string_pretty(meter) else {
        return;
    };
    let p = meter_path(output_dir);
    let tmp = p.with_extension("json.tmp");
    if std::fs::write(&tmp, body + "\n").is_ok() {
        let _ = std::fs::rename(&tmp, &p);
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn stamp_ts(now: u64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp(now as i64, 0)
        .map(|t| t.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".into())
}

#[cfg(test)]
#[path = "thin_tests.rs"]
pub(crate) mod tests;
