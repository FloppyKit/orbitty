//! Dev-only THIN stamp.
//!
//! Release builds (`debug_assertions` off) refuse before any stamp, nod, or
//! file write. Script, python, and output directory come from
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

/// Release builds do not seed the ⌘⌥T row.
pub fn seed_thin_row(debug_assertions: bool) -> bool {
    debug_assertions
}

/// Consumer/release builds stop here. The real stamp is compiled only when
/// `debug_assertions` is on; this gate is what the command checks first.
pub fn release_gate(debug_assertions: bool) -> Result<(), String> {
    if debug_assertions {
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
        .unwrap_or_else(|| manifest_dir.join("../scripts/thin/vivo_thin.py"));
    let python = nonblank(file.python.as_deref())
        .unwrap_or("python3")
        .to_string();
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

#[cfg(debug_assertions)]
pub fn stamp(app: &tauri::AppHandle) -> Result<String, StampError> {
    use tauri::Manager;

    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = resolve_config(&home, &manifest);
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
    let line = run_script(&cfg, &body)?;
    if !stamp_landed(&cfg.output_dir, &obs) {
        return Err(StampError::failed(
            "stamp script exited but no stamp landed",
        ));
    }
    save_meter(&cfg.output_dir, &meter);
    Ok(line)
}

#[cfg(not(debug_assertions))]
pub fn stamp(_app: &tauri::AppHandle) -> Result<String, StampError> {
    Err(StampError {
        user_line: "THIN is dev-only and is not in this build.".into(),
    })
}

#[cfg(debug_assertions)]
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

#[cfg(debug_assertions)]
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

#[cfg(debug_assertions)]
fn read_events(app: &tauri::AppHandle) -> String {
    use tauri::Manager;
    let Ok(dir) = app.path().app_data_dir() else {
        return String::new();
    };
    std::fs::read_to_string(dir.join("events.jsonl")).unwrap_or_default()
}

#[cfg(debug_assertions)]
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
mod tests {
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
        assert!(cfg.script.ends_with("scripts/thin/vivo_thin.py"));
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
}
