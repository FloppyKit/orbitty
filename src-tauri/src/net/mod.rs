//! GO-ORBITTY-NET-GUARD: one permit door for an outbound host call.
//!
//! Nothing is wired to the network here. There is no socket, no thread, and
//! no timer. A future adapter asks this door and gets a verdict:
//!
//! ```ignore
//! let flag_on = crate::flags::on(app, net::FLAG);
//! let verdict = net::permit_at(&app_data_dir, flag_on, host, "sync");
//! if !verdict.allow { return Err(verdict.reason); }
//! ```
//!
//! Three rules decide, in this order:
//! 1. Flag `net_guard` off → deny, even a listed host. The row ships off and
//!    is manual-only (no verify sweep flips it).
//! 2. The kill file present → deny, even when the flag is on. Removing the
//!    file is the only clear.
//! 3. A host must be an exact entry in `net-allow.json`. An empty or missing
//!    file denies every host. No wildcards. No secrets in the file — it holds
//!    host names only.
//!
//! Both files sit beside the channel flag file (`flags-<channel>.json`) in the
//! app-data dir. Nothing goes in `state.json`.
//!
//! This module is plain std + serde_json on purpose: `orbittyctl` includes it
//! by `#[path]` (the `ledger` precedent) so the CLI reads and writes the same
//! files the app's door reads. Keep it free of `tauri` imports.

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// The flag row this door obeys. Manual-only, default off.
pub const FLAG: &str = "net_guard";
/// The allow list, beside `flags-<channel>.json`.
pub const ALLOW_FILE: &str = "net-allow.json";
/// The kill file. Presence denies everything; the contents are ignored.
pub const KILL_FILE: &str = "net-kill";

pub fn allow_path(dir: &Path) -> PathBuf {
    dir.join(ALLOW_FILE)
}

pub fn kill_path(dir: &Path) -> PathBuf {
    dir.join(KILL_FILE)
}

/// One answer from the door. `reason` is one plain line, safe to log or print.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub allow: bool,
    pub reason: String,
}

fn allow_verdict(host: &str, purpose: &str) -> Verdict {
    Verdict {
        allow: true,
        reason: format!("{host} is listed in {ALLOW_FILE} (purpose: {purpose})"),
    }
}

fn deny(reason: String) -> Verdict {
    Verdict {
        allow: false,
        reason,
    }
}

/// The stored form of a host: trimmed and lowercase, host only. Rejects a
/// scheme, a port, a path, a wildcard, and anything that looks like a secret.
pub fn host_ok(host: &str) -> Result<String, String> {
    let h = host.trim().to_ascii_lowercase();
    if h.is_empty() {
        return Err("empty host".into());
    }
    if h.len() > 253 {
        return Err(format!("host {host:?} is longer than 253 bytes"));
    }
    if h.starts_with('.') || h.ends_with('.') || h.starts_with('-') || h.contains("..") {
        return Err(format!("host {host:?} has an empty label"));
    }
    for c in h.chars() {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-' || c == '_') {
            return Err(format!(
                "host {host:?} has {c:?}: a host is a-z 0-9 . - _ only (no scheme, port, path, or wildcard)"
            ));
        }
    }
    Ok(h)
}

/// Exact entries only. Read is fail-closed: a corrupt file is an error, never
/// an empty list that silently allows nothing (or, worse, everything).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AllowList {
    hosts: BTreeSet<String>,
}

impl AllowList {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.hosts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty()
    }

    pub fn contains(&self, host: &str) -> bool {
        self.hosts.contains(host)
    }

    pub fn hosts(&self) -> impl Iterator<Item = &str> {
        self.hosts.iter().map(String::as_str)
    }

    /// True when the host was newly added, false when it was already there.
    pub fn add(&mut self, host: &str) -> Result<bool, String> {
        let h = host_ok(host)?;
        Ok(self.hosts.insert(h))
    }

    /// True when the host was listed, false when it was not.
    pub fn remove(&mut self, host: &str) -> Result<bool, String> {
        let h = host_ok(host)?;
        Ok(self.hosts.remove(&h))
    }
}

/// `{"v":1,"hosts":["a.example"]}`. Zero bytes reads as an empty list; any
/// other malformed body is an error.
pub fn parse_allow(bytes: &[u8]) -> Result<AllowList, String> {
    if bytes.iter().all(|b| b.is_ascii_whitespace()) {
        return Ok(AllowList::empty());
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("{ALLOW_FILE}: corrupt ({e})"))?;
    let Some(obj) = value.as_object() else {
        return Err(format!("{ALLOW_FILE}: corrupt (not an object)"));
    };
    if obj.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(format!("{ALLOW_FILE}: corrupt (v)"));
    }
    let Some(hosts_v) = obj.get("hosts") else {
        return Err(format!("{ALLOW_FILE}: corrupt (hosts)"));
    };
    let Some(arr) = hosts_v.as_array() else {
        return Err(format!("{ALLOW_FILE}: corrupt (hosts)"));
    };
    let mut list = AllowList::empty();
    for v in arr {
        let Some(s) = v.as_str() else {
            return Err(format!("{ALLOW_FILE}: corrupt (hosts entry)"));
        };
        let h = host_ok(s).map_err(|e| format!("{ALLOW_FILE}: {e}"))?;
        list.hosts.insert(h);
    }
    Ok(list)
}

pub fn serialize_allow(list: &AllowList) -> Vec<u8> {
    let hosts: Vec<&String> = list.hosts.iter().collect();
    let value = json!({ "v": 1, "hosts": hosts });
    let mut bytes = serde_json::to_vec_pretty(&value).unwrap_or_else(|_| b"{}".to_vec());
    bytes.push(b'\n');
    bytes
}

/// Missing file is an empty list (deny every host on first use).
pub fn read_allow(dir: &Path) -> Result<AllowList, String> {
    let path = allow_path(dir);
    match fs::read(&path) {
        Ok(bytes) => parse_allow(&bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AllowList::empty()),
        Err(e) => Err(format!("{ALLOW_FILE}: read {}: {e}", path.display())),
    }
}

/// Same 0600 write as the flag file: temp file, sync, rename, mode.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let wrote = (|| -> Result<(), String> {
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        let mut f = opts
            .open(&tmp)
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.write_all(bytes)
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.sync_all()
            .map_err(|e| format!("sync {}: {e}", tmp.display()))?;
        drop(f);
        fs::rename(&tmp, path).map_err(|e| format!("rename to {}: {e}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("mode {}: {e}", path.display()))?;
        }
        Ok(())
    })();
    if wrote.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    wrote
}

pub fn write_allow(dir: &Path, list: &AllowList) -> Result<(), String> {
    write_atomic(&allow_path(dir), &serialize_allow(list))
}

pub fn kill_set(dir: &Path) -> bool {
    kill_path(dir).exists()
}

pub fn kill_on(dir: &Path) -> Result<(), String> {
    write_atomic(&kill_path(dir), b"")
}

/// Idempotent: clearing a kill file that is already gone is not an error.
pub fn kill_off(dir: &Path) -> Result<(), String> {
    match fs::remove_file(kill_path(dir)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{KILL_FILE}: remove: {e}")),
    }
}

/// The door. Pure: every input is passed in, nothing is read from disk.
pub fn permit(flag_on: bool, allow: &AllowList, kill: bool, host: &str, purpose: &str) -> Verdict {
    if !flag_on {
        return deny(format!("{FLAG} is off (flag {FLAG})"));
    }
    if kill {
        return deny(format!("kill file is set ({KILL_FILE})"));
    }
    let h = match host_ok(host) {
        Ok(h) => h,
        Err(e) => return deny(e),
    };
    if allow.contains(&h) {
        allow_verdict(&h, purpose)
    } else {
        deny(format!("{h} is not in {ALLOW_FILE}"))
    }
}

/// The door as an adapter calls it: reads the two files, takes the flag bit
/// from the caller. A corrupt allow file denies (never allows).
pub fn permit_at(dir: &Path, flag_on: bool, host: &str, purpose: &str) -> Verdict {
    if !flag_on {
        return permit(flag_on, &AllowList::empty(), false, host, purpose);
    }
    let kill = kill_set(dir);
    if kill {
        return permit(flag_on, &AllowList::empty(), true, host, purpose);
    }
    match read_allow(dir) {
        Ok(list) => permit(flag_on, &list, false, host, purpose),
        Err(e) => deny(e),
    }
}

/// The readout: flag, allow count and hosts, kill set or not. Fail-closed on a
/// corrupt allow file (`allow` is null and `allowError` says why).
pub fn status(dir: &Path, flag_on: bool) -> Value {
    let kill = kill_set(dir);
    let mut out = json!({
        "flag": FLAG,
        "on": flag_on,
        "allow": 0,
        "hosts": [],
        "kill": kill,
        "allowFile": allow_path(dir).display().to_string(),
        "killFile": kill_path(dir).display().to_string(),
    });
    match read_allow(dir) {
        Ok(list) => {
            out["allow"] = json!(list.len());
            out["hosts"] = json!(list.hosts().collect::<Vec<&str>>());
        }
        Err(e) => {
            out["allow"] = Value::Null;
            out["allowError"] = json!(e);
        }
    }
    out
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
