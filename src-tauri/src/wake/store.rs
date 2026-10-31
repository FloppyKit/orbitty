//! `<app data>/wake/` files. Mode 0600, atomic rename, except append-only inbox.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::route::{RouteState, Sub};
use super::SeatId;

pub fn wake_dir(app_dir: &Path) -> PathBuf {
    app_dir.join("wake")
}

pub fn events_path(app_dir: &Path) -> PathBuf {
    app_dir.join("events.jsonl")
}

fn inbox_dir(app_dir: &Path) -> PathBuf {
    wake_dir(app_dir).join("inbox")
}

pub fn seat_file(seat: &SeatId, ext: &str) -> String {
    let key = seat.as_key().replace('/', "_");
    format!("{key}.{ext}")
}

pub fn ensure_tree(app_dir: &Path) -> Result<(), String> {
    let root = wake_dir(app_dir);
    fs::create_dir_all(inbox_dir(app_dir)).map_err(|e| format!("wake dir: {e}"))?;
    set_dir(&root)?;
    set_dir(&inbox_dir(app_dir))?;
    Ok(())
}

fn set_dir(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
    }
    let _ = path;
    Ok(())
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        set_dir(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|s| s.to_str()).unwrap_or("bin")
    ));
    {
        let mut opts = OpenOptions::new();
        opts.create(true).write(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(&tmp)
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.write_all(bytes).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.sync_all().map_err(|e| format!("sync {}: {e}", tmp.display()))?;
    }
    fs::rename(&tmp, path).map_err(|e| format!("rename {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn append_line(path: &Path, line: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        set_dir(parent)?;
    }
    let mut opts = OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(path)
        .map_err(|e| format!("append {}: {e}", path.display()))?;
    f.write_all(line.as_bytes())
        .and_then(|_| f.write_all(b"\n"))
        .map_err(|e| format!("append {}: {e}", path.display()))?;
    f.sync_data().ok();
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct SubFile {
    v: u8,
    subs: Vec<Sub>,
}

pub fn load_subs(app_dir: &Path) -> Vec<Sub> {
    let path = wake_dir(app_dir).join("subscriptions.json");
    let Ok(raw) = fs::read(&path) else {
        return Vec::new();
    };
    serde_json::from_slice::<SubFile>(&raw)
        .map(|f| f.subs)
        .unwrap_or_default()
}

pub fn save_subs(app_dir: &Path, subs: &[Sub]) -> Result<(), String> {
    ensure_tree(app_dir)?;
    let file = SubFile {
        v: 1,
        subs: subs.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&file).map_err(|e| format!("subs: {e}"))?;
    write_atomic(&wake_dir(app_dir).join("subscriptions.json"), &bytes)
}

#[derive(Serialize, Deserialize, Default)]
struct CursorFile {
    v: u8,
    seq: u64,
}

pub fn load_cursor(app_dir: &Path) -> u64 {
    let path = wake_dir(app_dir).join("cursor.json");
    let Ok(raw) = fs::read(&path) else {
        return 0;
    };
    serde_json::from_slice::<CursorFile>(&raw)
        .map(|c| c.seq)
        .unwrap_or(0)
}

pub fn save_cursor(app_dir: &Path, seq: u64) -> Result<(), String> {
    ensure_tree(app_dir)?;
    let file = CursorFile { v: 1, seq };
    let bytes = serde_json::to_vec(&file).map_err(|e| format!("cursor: {e}"))?;
    write_atomic(&wake_dir(app_dir).join("cursor.json"), &bytes)?;
    // Test crash sits after the durable write. A save before routing
    // then drops the batch; a save after routing does not.
    #[cfg(test)]
    if CRASH_AFTER_CURSOR.with(|c| c.get()) {
        panic!("crash after cursor save");
    }
    Ok(())
}

#[cfg(test)]
thread_local! {
    static CRASH_AFTER_CURSOR: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub fn crash_after_cursor_save(on: bool) {
    CRASH_AFTER_CURSOR.with(|c| c.set(on));
}

/// High-water `seq` already on events.jsonl (or events.seq). Inbox ack
/// uses this space. Wall-clock is not a seq.
pub fn log_tip(app_dir: &Path) -> u64 {
    let path = events_path(app_dir);
    let from_log = crate::relay::event_seq::recover_max_seq(
        &path,
        crate::relay::event_seq::TAIL_BYTES,
    );
    let from_file = fs::read_to_string(app_dir.join("events.seq"))
        .ok()
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(0);
    from_log.max(from_file)
}

pub fn load_state(app_dir: &Path) -> RouteState {
    let path = wake_dir(app_dir).join("stats.json");
    let Ok(raw) = fs::read(&path) else {
        return RouteState::default();
    };
    serde_json::from_slice(&raw).unwrap_or_default()
}

pub fn save_state(app_dir: &Path, state: &RouteState) -> Result<(), String> {
    ensure_tree(app_dir)?;
    let bytes = serde_json::to_vec(&state).map_err(|e| format!("stats: {e}"))?;
    write_atomic(&wake_dir(app_dir).join("stats.json"), &bytes)
}

#[derive(Serialize, Deserialize, Default)]
struct AlarmFile {
    v: u8,
    alarms: std::collections::BTreeMap<String, String>,
}

pub fn load_alarms(app_dir: &Path) -> std::collections::BTreeMap<String, String> {
    let path = wake_dir(app_dir).join("alarms.json");
    let Ok(raw) = fs::read(&path) else {
        return std::collections::BTreeMap::new();
    };
    serde_json::from_slice::<AlarmFile>(&raw)
        .map(|f| f.alarms)
        .unwrap_or_default()
}

pub fn save_alarms(
    app_dir: &Path,
    alarms: &std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    ensure_tree(app_dir)?;
    let file = AlarmFile {
        v: 1,
        alarms: alarms.clone(),
    };
    let bytes = serde_json::to_vec(&file).map_err(|e| format!("alarms: {e}"))?;
    write_atomic(&wake_dir(app_dir).join("alarms.json"), &bytes)
}

pub fn inbox_path(app_dir: &Path, seat: &SeatId) -> PathBuf {
    inbox_dir(app_dir).join(seat_file(seat, "jsonl"))
}

pub fn ack_path(app_dir: &Path, seat: &SeatId) -> PathBuf {
    inbox_dir(app_dir).join(seat_file(seat, "ack"))
}

pub fn read_inbox(app_dir: &Path, seat: &SeatId) -> Vec<Value> {
    let Ok(raw) = fs::read_to_string(inbox_path(app_dir, seat)) else {
        return Vec::new();
    };
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

pub fn append_inbox(app_dir: &Path, seat: &SeatId, line: &Value) -> Result<(), String> {
    ensure_tree(app_dir)?;
    let text = serde_json::to_string(line).map_err(|e| format!("inbox: {e}"))?;
    append_line(&inbox_path(app_dir, seat), &text)
}

pub fn load_ack(app_dir: &Path, seat: &SeatId) -> u64 {
    fs::read_to_string(ack_path(app_dir, seat))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

pub fn save_ack(app_dir: &Path, seat: &SeatId, seq: u64) -> Result<(), String> {
    ensure_tree(app_dir)?;
    write_atomic(ack_path(app_dir, seat).as_path(), format!("{seq}\n").as_bytes())
}

pub fn wake_tree_exists(app_dir: &Path) -> bool {
    wake_dir(app_dir).exists()
}

pub fn mode_of(path: &Path) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return fs::metadata(path).map(|m| m.permissions().mode() & 0o777).unwrap_or(0);
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        0
    }
}

/// One local-only kind-31200 line. Never published.
pub fn log_summary(app_dir: &Path, e_type: &str, body: &Value) -> Result<(), String> {
    let path = events_path(app_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("events dir: {e}"))?;
    }
    let id = crate::event_log::uuid_v4_hex();
    let content = serde_json::to_string(body).unwrap_or_else(|_| "{}".into());
    let tags = vec![
        ["t".into(), "local-only".into()],
        ["e-type".into(), e_type.to_string()],
        ["trace".into(), id.clone()],
        ["project".into(), "void".into()],
        ["actor".into(), "wake".into()],
    ];
    crate::event_log::write_event(
        &path,
        crate::event_log::unix_now(),
        crate::event_log::KIND,
        tags,
        content,
        (id, String::new(), String::new()),
    )
    .map(|_| ())
}

pub fn summary_lines(app_dir: &Path) -> Vec<Value> {
    let Ok(raw) = fs::read_to_string(events_path(app_dir)) else {
        return Vec::new();
    };
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| {
            v.get("kind").and_then(Value::as_u64) == Some(crate::event_log::KIND)
                && tag(v, "t") == "local-only"
                && matches!(
                    tag(v, "e-type").as_str(),
                    "wake" | "wake-drop" | "wake-alarm"
                )
        })
        .collect()
}

pub fn tag(v: &Value, name: &str) -> String {
    v.get("tags")
        .and_then(Value::as_array)
        .and_then(|arr| {
            arr.iter().find_map(|pair| {
                let a = pair.as_array()?;
                if a.first()?.as_str()? == name {
                    a.get(1)?.as_str().map(str::to_string)
                } else {
                    None
                }
            })
        })
        .unwrap_or_default()
}

pub fn json_body(v: &Value) -> Value {
    match v.get("content") {
        Some(Value::String(s)) => serde_json::from_str(s).unwrap_or(json!({})),
        Some(other) => other.clone(),
        None => json!({}),
    }
}
