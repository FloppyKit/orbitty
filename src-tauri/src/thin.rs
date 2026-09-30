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
pub(crate) mod tests {
    use super::*;

    fn fp_window(start: u32, n: u32) -> Vec<String> {
        (start..start + n)
            .map(|i| fingerprint("you", &format!("line {i}")))
            .collect()
    }

    #[test]
    fn release_gate_and_seed_follow_the_dev_flag() {
        assert!(release_gate(true).is_ok());
        assert!(release_gate(false).unwrap_err().contains("dev-only"));
        assert!(seed_thin_row(true));
        assert!(!seed_thin_row(false));
    }

    #[test]
    fn button_and_shortcut_share_one_predicate() {
        assert!(routes_to_companion_thin("thin", "anything"));
        assert!(routes_to_companion_thin(
            "Thin",
            "python3 /tmp/vivo_thin.py stamp"
        ));
        assert!(routes_to_companion_thin(
            "custom",
            "orbittyctl companion thin"
        ));
        assert!(routes_to_companion_thin("custom", "companion.thin"));
        assert!(!routes_to_companion_thin(
            "companion",
            "orbittyctl companion open"
        ));
        assert!(!routes_to_companion_thin(
            "note",
            "orbittyctl companion thinking"
        ));
        assert!(!routes_to_companion_thin(
            "kanban",
            "orbittyctl kanban open"
        ));
    }

    #[test]
    fn defaults_are_not_the_research_nest_or_homebrew() {
        let home = std::env::temp_dir().join(format!(
            "orbitty-thin-cfg-{}-{}",
            std::process::id(),
            unix_now()
        ));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let cfg = resolve_config(&home, Path::new("/work/src-tauri"));
        assert_eq!(cfg.python, "python3");
        assert!(!cfg.python.contains("homebrew"));
        assert_eq!(cfg.script, Path::new("/work/scripts/thin/vivo_thin.py"));
        assert!(check_paths(&home, &cfg).is_ok(), "the default config passes check_paths");
        assert_eq!(cfg.output_dir, home.join(".config/orbitty/thin-stamps"));
        assert!(!cfg.output_dir.to_string_lossy().contains(".buzz"));
        assert!(!cfg.script.to_string_lossy().contains(".buzz"));

        let dir = home.join(".config/orbitty");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("thin.json"),
            r#"{"script":"~/bin/vivo_thin.py","python":"python3.12","outputDir":"~/stamps"}"#,
        )
        .unwrap();
        let cfg = resolve_config(&home, Path::new("/work/src-tauri"));
        assert_eq!(cfg.script, home.join("bin/vivo_thin.py"));
        assert_eq!(cfg.python, "python3.12");
        assert_eq!(cfg.output_dir, home.join("stamps"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn context_size_rounds_tokens_up() {
        assert_eq!(context_size(&[]), (0, 0));
        assert_eq!(context_size(&["abcd".into()]), (4, 1));
        assert_eq!(context_size(&["abcde".into(), "x".into()]), (6, 2));
    }

    #[test]
    fn turn_count_is_not_the_capped_index() {
        let now = 1_000u64;
        let window = fp_window(0, 40);
        let (meter, obs) = reconcile(&Meter::default(), "grok:s1", &window, 0, 10, now);
        assert_eq!(obs.turn_count, 40);
        assert_eq!(obs.last_reset_reason, "meter_start");
        assert_eq!(obs.secs_since_reset, 0);
        assert_eq!(obs.session_id, "grok:s1");

        // 15 oldest dropped, 15 new. The list is still 40 long.
        let slid = fp_window(15, 40);
        let (meter, obs) = reconcile(&meter, "grok:s1", &slid, 15, 25, now + 9);
        assert_eq!(obs.turn_count, 55);
        assert_ne!(obs.turn_count, slid.len() as u64);
        assert_eq!(obs.secs_since_reset, 9);
        assert_eq!(obs.last_reset_reason, "meter_start");

        // Whole window replaced; the event log saw 80 mouth turns.
        let replaced = fp_window(200, 40);
        let (_meter, obs) = reconcile(&meter, "grok:s1", &replaced, 80, 105, now + 20);
        assert_eq!(obs.turn_count, 135);
        assert!(obs.turn_count > 40);
    }

    #[test]
    fn session_change_resets_the_clock_and_reason() {
        let window = fp_window(0, 10);
        let (meter, _) = reconcile(&Meter::default(), "grok:old", &window, 0, 4, 50);
        let fresh = fp_window(90, 40);
        let (_meter, obs) = reconcile(&meter, "grok:new", &fresh, 70, 80, 90);
        assert_eq!(obs.last_reset_reason, "session_changed");
        assert_eq!(obs.turn_count, 70);
        assert_eq!(obs.secs_since_reset, 0);
        assert_eq!(obs.reset_at, 90);
        assert_eq!(obs.session_id, "grok:new");
    }

    #[test]
    fn same_stamp_twice_does_not_add_turns() {
        let window = fp_window(0, 5);
        let (meter, obs) = reconcile(&Meter::default(), "grok:s", &window, 0, 3, 10);
        assert_eq!(obs.turn_count, 5);
        let (_meter, obs) = reconcile(&meter, "grok:s", &window, 0, 3, 15);
        assert_eq!(obs.turn_count, 5);
        assert_eq!(obs.secs_since_reset, 5);
    }

    #[test]
    fn event_lines_count_only_mouth_turns_after_the_cursor() {
        let log = "\
{\"tags\":[[\"e-type\",\"companion-send\"]]}\n\
{\"tags\":[[\"e-type\",\"action-start\"]]}\n\
{\"tags\":[[\"e-type\",\"companion-reply\"]]}\n\
not json companion-send\n";
        let (n, total) = count_turn_events(log, 0);
        assert_eq!(total, 4);
        assert_eq!(n, 2);
        let (n, total) = count_turn_events(log, 2);
        assert_eq!(total, 4);
        assert_eq!(n, 1);
        let (n, _) = count_turn_events(log, 99);
        assert_eq!(n, 0);
    }

    #[test]
    fn script_writes_one_stamp_and_does_not_shell_out() {
        let src = include_str!("../../scripts/thin/vivo_thin.py");
        assert!(!src.contains("orbittyctl"));
        assert!(!src.contains(".buzz"));
        assert!(!src.contains("/opt/homebrew"));
        assert!(!src.contains("last_error"));
        assert!(!src.contains("nod"));

        let home = std::env::temp_dir().join(format!(
            "orbitty-thin-stamp-{}-{}",
            std::process::id(),
            unix_now()
        ));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let out_dir = home.join("stamps");
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/thin/vivo_thin.py");
        let payload = json!({
            "session_id": "grok:test-session",
            "turn_count": 57,
            "context_chars": 11,
            "context_tokens_est": 3,
            "secs_since_reset": 12,
            "last_reset_reason": "meter_start",
            "ts": "2026-09-29T22:15:00Z",
            "mode": "spark",
            "turns": [
                {"role": "you", "text": "hello"},
                {"role": "them", "text": "there"}
            ]
        });
        let mut child = Command::new("python3")
            .arg(&script)
            .arg("stamp")
            .env("ORBITTY_THIN_OUTPUT", &out_dir)
            .env("HOME", &home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("python3");
        {
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(payload.to_string().as_bytes())
                .unwrap();
        }
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "stderr {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let events = std::fs::read_to_string(out_dir.join("events.jsonl")).unwrap();
        let ev: Value = serde_json::from_str(events.lines().next().unwrap()).unwrap();
        assert_eq!(ev["session_id"], "grok:test-session");
        assert_eq!(ev["turn_count"], 57);
        assert_eq!(ev["context_chars"], 11);
        assert_eq!(ev["context_tokens_est"], 3);
        assert_eq!(ev["secs_since_reset"], 12);
        assert_eq!(ev["last_reset_reason"], "meter_start");
        assert_eq!(ev["ts"], "2026-09-29T22:15:00Z");
        assert_eq!(ev["human_stamp_turn"], 57);
        let sid = ev["id"].as_str().unwrap();
        let turns = std::fs::read_to_string(out_dir.join(sid).join("turns.jsonl")).unwrap();
        let rows: Vec<Value> = turns
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(rows[0]["turn"].as_i64(), Some(56));
        assert_eq!(rows[1]["turn"].as_i64(), Some(57));
        assert!(out_dir.join(sid).join("event.json").is_file());
        assert!(!home.join(".buzz").exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn script_refuses_without_an_output_dir() {
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/thin/vivo_thin.py");
        let home = std::env::temp_dir().join(format!(
            "orbitty-thin-noout-{}-{}",
            std::process::id(),
            unix_now()
        ));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let mut child = Command::new("python3")
            .arg(&script)
            .arg("stamp")
            .env("HOME", &home)
            .env_remove("ORBITTY_THIN_OUTPUT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // Close stdin so a buggy script cannot block on a payload read
        // before it notices the missing directory.
        drop(child.stdin.take());
        let out = child.wait_with_output().unwrap();
        assert!(!out.status.success());
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("output dir"), "{err}");
        assert!(!home.join(".buzz").exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    /// The body of `fn <name>` in `src` (up to the first line that is a lone `}`).
    pub(crate) fn fn_body<'a>(src: &'a str, sig: &str) -> &'a str {
        let at = src.find(sig).unwrap_or_else(|| panic!("missing {sig}"));
        let rest = &src[at..];
        let end = rest.find("\n}\n").unwrap_or(rest.len());
        &rest[..end]
    }

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "orbitty-thin-{tag}-{}-{}",
            std::process::id(),
            unix_now()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::canonicalize(&d).unwrap()
    }

    // B1: the gate and the seeding use one predicate, and it is the dev-thin cfg.
    #[test]
    fn gate_and_seed_share_the_dev_thin_cfg() {
        assert_eq!(THIN_ENABLED, cfg!(any(debug_assertions, feature = "dev-thin")));
        let coord = include_str!("coordinator.rs");
        let actions = include_str!("actions.rs");
        let thin = fn_body(coord, "pub fn companion_thin(");
        assert!(thin.contains("release_gate(crate::thin::THIN_ENABLED)"), "{thin}");
        let bake = fn_body(actions, "pub fn bake_defaults(");
        assert!(bake.contains("seed_thin_row(crate::thin::THIN_ENABLED)"), "{bake}");
        // No thin item may be gated on debug_assertions alone.
        let bare = format!("cfg!({})", "debug_assertions");
        assert!(!thin.contains(&bare) && !bake.contains(&bare));
        let src = include_str!("thin.rs");
        for needle in [
            format!("#[cfg({})]", "debug_assertions"),
            format!("#[cfg(not({}))]", "debug_assertions"),
        ] {
            assert!(!src.contains(&needle), "thin.rs still has {needle}");
        }
        let cargo = include_str!("../Cargo.toml");
        assert!(cargo.contains("\n[features]\n") && cargo.contains("\ndev-thin = []\n"));
        let nightly = include_str!("../../scripts/install-nightly.sh");
        assert!(nightly.contains("npm run tauri build -- --features dev-thin"));
        let stable = include_str!("../../scripts/install-stable.sh");
        assert!(!stable.contains("dev-thin"));
    }

    // N2 (T2, T13): the gate runs first; exactly one nod per attempt.
    #[test]
    fn thin_flow_gates_first_and_nods_once() {
        use std::cell::Cell;
        let (stamps, nods, notes) = (Cell::new(0), Cell::new(0), Cell::new(0));
        let r = thin_flow(
            release_gate(false),
            || {
                stamps.set(stamps.get() + 1);
                Ok("x".into())
            },
            || nods.set(nods.get() + 1),
            |_| {
                notes.set(notes.get() + 1);
                Ok(())
            },
        );
        assert!(r.unwrap_err().contains("dev-only"));
        assert_eq!((stamps.get(), nods.get(), notes.get()), (0, 0, 0));

        let r = thin_flow(
            release_gate(true),
            || {
                stamps.set(stamps.get() + 1);
                Ok("stamped".into())
            },
            || nods.set(nods.get() + 1),
            |_| {
                notes.set(notes.get() + 1);
                Ok(())
            },
        );
        assert_eq!(r.unwrap(), "stamped");
        assert_eq!((stamps.get(), nods.get(), notes.get()), (1, 1, 0));

        let noted = std::cell::RefCell::new(String::new());
        let r = thin_flow(
            Ok(()),
            || Err(StampError::failed("boom")),
            || nods.set(nods.get() + 1),
            |l| {
                *noted.borrow_mut() = l;
                Ok(())
            },
        );
        assert_eq!(r.unwrap(), "THIN stamp failed. boom");
        assert_eq!(*noted.borrow(), "THIN stamp failed. boom");
        assert_eq!(nods.get(), 2);

        // companion_thin wires the real gate, the real stamp, and one nod.
        let body = fn_body(include_str!("coordinator.rs"), "pub fn companion_thin(");
        assert!(body.contains("crate::thin::thin_flow("), "{body}");
        assert!(body.contains("|| crate::thin::stamp(&app)"), "{body}");
        assert_eq!(body.matches("companion_play(").count(), 1, "{body}");
        assert_eq!(body.matches("\"nod\"").count(), 1, "{body}");
    }

    // N2 (T11, T12): no landed stamp = failure; the meter moves only after landing.
    #[test]
    fn meter_advances_only_after_a_landed_stamp() {
        let log = std::cell::RefCell::new(Vec::<&str>::new());
        let r = stamp_steps(
            || {
                log.borrow_mut().push("run");
                Ok("ok".into())
            },
            || {
                log.borrow_mut().push("landed");
                true
            },
            || log.borrow_mut().push("save"),
        );
        assert_eq!(r.ok().as_deref(), Some("ok"));
        assert_eq!(*log.borrow(), vec!["run", "landed", "save"]);

        log.borrow_mut().clear();
        let r = stamp_steps(
            || {
                log.borrow_mut().push("run");
                Ok("ok".into())
            },
            || false,
            || log.borrow_mut().push("save"),
        );
        assert!(r.err().unwrap().user_line().contains("no stamp landed"));
        assert_eq!(*log.borrow(), vec!["run"]);

        log.borrow_mut().clear();
        let r = stamp_steps(
            || Err(StampError::failed("exit 2")),
            || {
                log.borrow_mut().push("landed");
                true
            },
            || log.borrow_mut().push("save"),
        );
        assert!(r.is_err());
        assert!(log.borrow().is_empty());
    }

    // N2 (T11): the landing check really reads the last events.jsonl line.
    #[test]
    #[cfg(any(debug_assertions, feature = "dev-thin"))]
    fn landing_check_matches_the_last_event() {
        let dir = scratch("landed");
        let obs = Observation {
            session_id: "grok:s".into(),
            turn_count: 7,
            secs_since_reset: 3,
            last_reset_reason: "meter_start".into(),
            reset_at: 1,
        };
        assert!(!stamp_landed(&dir, &obs), "no events file");
        let good = r#"{"session_id":"grok:s","turn_count":7,"context_chars":1,"context_tokens_est":1,"secs_since_reset":3,"last_reset_reason":"meter_start","ts":"t"}"#;
        std::fs::write(dir.join("events.jsonl"), format!("{good}\n")).unwrap();
        assert!(stamp_landed(&dir, &obs));
        let stale = good.replace("\"turn_count\":7", "\"turn_count\":6");
        std::fs::write(dir.join("events.jsonl"), format!("{good}\n{stale}\n")).unwrap();
        assert!(!stamp_landed(&dir, &obs), "last line is an older stamp");
        let other = good.replace("grok:s", "grok:other");
        std::fs::write(dir.join("events.jsonl"), format!("{other}\n")).unwrap();
        assert!(!stamp_landed(&dir, &obs));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // N5: thin.json may not point stamps (or the script) into ~/.buzz, or be relative.
    #[test]
    fn thin_json_refuses_buzz_and_relative_paths() {
        let home = scratch("buzzguard");
        let buzz = home.join(".buzz");
        std::fs::create_dir_all(buzz.join("RESEARCH")).unwrap();
        let cfg_dir = home.join(".config/orbitty");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        let manifest = Path::new("/work/src-tauri");
        let check = |out: &str| {
            std::fs::write(
                cfg_dir.join("thin.json"),
                format!(r#"{{"outputDir":{}}}"#, serde_json::to_string(out).unwrap()),
            )
            .unwrap();
            check_paths(&home, &resolve_config(&home, manifest))
        };
        assert!(check("~/stamps").is_ok());
        assert!(check("~/.config/orbitty/thin-stamps").is_ok());
        assert!(check("~/.buzzard/stamps").is_ok());
        for bad in [
            "~/.buzz",
            "~/.buzz/RESEARCH/stamps",
            "~/.BUZZ/research/new",
            "$HOME/.Buzz/x",
        ] {
            let e = check(bad).expect_err(bad);
            assert!(e.contains("inside ~/.buzz"), "{bad}: {e}");
        }
        // `..` is refused even when the folder it climbs out of is missing.
        let through = format!("{}/.buzz/../.buzz/y", home.display());
        for dots in [
            "~/missing/../.buzz/x",
            "~/a/b/../../.buzz/new",
            "~/missing/../stamps",
            through.as_str(),
        ] {
            let e = check(dots).expect_err(dots);
            assert!(e.contains("may not contain .."), "{dots}: {e}");
        }
        assert!(!buzz.join("x").exists() && !buzz.join("new").exists());
        for rel in ["stamps", "./stamps", "~dude/stamps", "~user"] {
            let e = check(rel).expect_err(rel);
            assert!(e.contains("absolute"), "{rel}: {e}");
        }
        // A symlink into .buzz, a dangling one to a .buzz path not made yet,
        // and the firmlink path to the same folder.
        std::os::unix::fs::symlink(buzz.join("RESEARCH"), home.join("link")).unwrap();
        assert!(check("~/link/stamps").is_err());
        std::os::unix::fs::symlink(buzz.join("not-yet"), home.join("dangle")).unwrap();
        assert!(check("~/dangle").is_err());
        let firm = PathBuf::from(format!("/System/Volumes/Data{}", home.display()));
        if firm.is_dir() && std::fs::metadata(&firm).is_ok() {
            assert!(check(&format!("{}/.buzz/RESEARCH", firm.display())).is_err());
        }
        // The legacy ~/.buzz script is refused as the scanner too.
        std::fs::write(
            cfg_dir.join("thin.json"),
            r#"{"script":"~/.buzz/RESEARCH/RGL_TESTBED/scripts/vivo_thin.py"}"#,
        )
        .unwrap();
        let e = check_paths(&home, &resolve_config(&home, manifest)).unwrap_err();
        assert!(e.contains("script") && e.contains("inside ~/.buzz"), "{e}");
        // Nothing was created under .buzz by the checks.
        let under: Vec<_> = std::fs::read_dir(&buzz).unwrap().collect();
        assert_eq!(under.len(), 1);
        let _ = std::fs::remove_dir_all(&home);
    }

    // stamp() runs the path check before it creates the output dir.
    #[test]
    fn stamp_checks_paths_before_mkdir() {
        let body = fn_body(include_str!("thin.rs"), "pub fn stamp(app: &tauri::AppHandle)");
        let check = body.find("check_paths(&home, &cfg)").expect("check_paths call");
        let mkdir = body.find("create_dir_all(&cfg.output_dir)").expect("mkdir");
        assert!(check < mkdir);
        // The meter is saved in exactly one place, inside stamp_steps (after
        // the landing check), never on its own before the script runs.
        let steps = body.find("stamp_steps(").expect("stamp_steps call");
        assert_eq!(body.matches("save_meter(").count(), 1, "{body}");
        assert!(body.find("save_meter(").unwrap() > steps, "{body}");
    }

    // N-H: a thin.json python *path* gets the .buzz, relative and `..` checks.
    #[test]
    fn thin_json_python_path_is_checked_too() {
        let home = scratch("pyguard");
        std::fs::create_dir_all(home.join(".buzz/RESEARCH/RGL_TESTBED/.venv/bin")).unwrap();
        let cfg_dir = home.join(".config/orbitty");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        let check = |py: &str| {
            std::fs::write(
                cfg_dir.join("thin.json"),
                format!(r#"{{"python":{}}}"#, serde_json::to_string(py).unwrap()),
            )
            .unwrap();
            check_paths(&home, &resolve_config(&home, Path::new("/work/src-tauri")))
        };
        assert!(check("python3").is_ok(), "bare name on PATH");
        assert!(check("python3.12").is_ok());
        assert!(check("/usr/bin/python3").is_ok());
        assert!(check("~/.pyenv/shims/python").is_ok());
        for bad in [
            "~/.buzz/RESEARCH/RGL_TESTBED/.venv/bin/python",
            "$HOME/.BUZZ/RESEARCH/RGL_TESTBED/.venv/bin/python",
            &format!("{}/.Buzz/x/python", home.display()),
        ] {
            let e = check(bad).expect_err(bad);
            assert!(e.contains("python") && e.contains("inside ~/.buzz"), "{bad}: {e}");
        }
        let e = check("bin/python").unwrap_err();
        assert!(e.contains("python") && e.contains("absolute"), "{e}");
        let e = check("~/missing/../.buzz/py").unwrap_err();
        assert!(e.contains("may not contain .."), "{e}");
        let _ = std::fs::remove_dir_all(&home);
    }

    // N-G: both install scripts remove the built bundle before tauri build,
    // behind a guard that needs a non-empty SRC ending in .app.
    #[test]
    fn install_scripts_clear_the_bundle_before_building() {
        for (name, src) in [
            ("install-nightly.sh", include_str!("../../scripts/install-nightly.sh")),
            ("install-stable.sh", include_str!("../../scripts/install-stable.sh")),
        ] {
            let guard = r#"if [[ -n "${SRC:-}" && "$SRC" == *.app && "$SRC" == src-tauri/target/release/bundle/macos/* ]]; then"#;
            let g = src.find(guard).unwrap_or_else(|| panic!("{name}: guard missing"));
            let rm = src.find("  rm -rf -- \"$SRC\"\n").unwrap_or_else(|| panic!("{name}: rm missing"));
            let build = src.find("\nnpm run tauri build").unwrap_or_else(|| panic!("{name}: build"));
            assert!(g < rm && rm < build, "{name}: rm must be guarded and before tauri build");
            assert_eq!(src.matches("rm -rf -- \"$SRC\"").count(), 1, "{name}: one bundle rm");
            let set = src.find("\nSRC=\"src-tauri/target/release/bundle/macos/").expect("SRC set");
            assert!(set < g, "{name}: SRC is set before the rm");
        }
    }
}
