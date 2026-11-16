//! `orbittyctl net` — the permit door's verbs. Local files only, no control
//! socket, no app needed: the allow list and the kill file sit beside the
//! channel flag file in the app-data dir (the same dir as the socket).
//!
//! The door body is the lib module by `#[path]` (the `ledger` precedent), so
//! the CLI and the app read and write the same files with the same rules.

use super::parse::Opts;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

#[path = "../../../net/mod.rs"]
#[allow(dead_code)]
mod door;

const PURPOSE: &str = "orbittyctl net permit";

pub(crate) fn run(opts: &Opts) -> Result<String, String> {
    let dir = app_data(opts);
    let words: Vec<&str> = opts.words.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["net", "status"] => status(&dir, opts.json),
        ["net", "allow-add"] => allow_add(&dir, opts),
        ["net", "allow-del"] => allow_del(&dir, opts),
        ["net", "kill", bit] => kill(&dir, bit, opts.json),
        ["net", "permit"] => permit(&dir, opts),
        ["net"] => {
            Err("net: need a verb — status | allow-add | allow-del | kill on|off | permit".into())
        }
        ["net", other, ..] => Err(format!(
            "unknown net command {other} — status | allow-add | allow-del | kill on|off | permit"
        )),
        _ => Err(
            "usage: orbittyctl net status | allow-add | allow-del | kill on|off | permit".into(),
        ),
    }
}

/// Same dir the socket lives in (`--sock`, `ORBITTY_CONTROL_SOCK`, else the
/// stable app-data dir Nightly shares).
fn app_data(opts: &Opts) -> PathBuf {
    super::sock_path(opts)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_default();
            PathBuf::from(home).join("Library/Application Support/space.orbitty.mac")
        })
}

/// Env first, then the channel flag files, else the row default (off). This
/// mirrors `flags::resolve` for the one bit the door needs; the row is
/// manual-only and its default is off, so an unreadable file is off.
fn flag_state(dir: &Path) -> (bool, String) {
    let raw = std::env::var("ORBITTY_FLAG_NET_GUARD")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(raw) = raw {
        match raw.as_str() {
            "1" | "on" | "true" => return (true, "env".into()),
            "0" | "off" | "false" => return (false, "env".into()),
            _ => eprintln!("orbittyctl: ignoring bad ORBITTY_FLAG_NET_GUARD={raw}"),
        }
    }
    for channel in ["nightly", "stable", "alpha"] {
        let name = format!("flags-{channel}.json");
        let Ok(bytes) = fs::read(dir.join(&name)) else {
            continue;
        };
        if let Ok(v) = serde_json::from_slice::<Value>(&bytes) {
            if let Some(bit) = v["on"]["net_guard"].as_bool() {
                return (bit, name);
            }
        }
    }
    (false, "default".into())
}

/// `-- host` or the bare word after the verb.
fn host_arg(o: &Opts, verb: &str) -> Result<String, String> {
    let raw = if !o.tail.is_empty() {
        o.tail.join(" ")
    } else if o.words.len() > 2 {
        o.words[2..].join(" ")
    } else {
        String::new()
    };
    let host = raw.trim().to_string();
    if host.is_empty() {
        return Err(format!(
            "net {verb}: need a host — orbittyctl net {verb} -- example.com"
        ));
    }
    Ok(host)
}

fn hosts_line(list: &door::AllowList) -> String {
    if list.is_empty() {
        return String::new();
    }
    format!(": {}", list.hosts().collect::<Vec<&str>>().join(", "))
}

fn status(dir: &Path, json_out: bool) -> Result<String, String> {
    let (on, source) = flag_state(dir);
    let mut st = door::status(dir, on);
    st["source"] = json!(source);
    if json_out {
        return Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&st).unwrap_or_else(|_| "{}".into())
        ));
    }
    let flag = if on { "on" } else { "off" };
    let kill = if st["kill"].as_bool().unwrap_or(false) {
        "set"
    } else {
        "not set"
    };
    let mut out = format!("{}  {flag}  ({source})\n", door::FLAG);
    match door::read_allow(dir) {
        Ok(list) => {
            out.push_str(&format!(
                "allow  {} host(s){}\n",
                list.len(),
                hosts_line(&list)
            ));
        }
        Err(e) => out.push_str(&format!("allow  corrupt — {e}\n")),
    }
    out.push_str(&format!("kill  {kill}\n"));
    Ok(out)
}

fn allow_add(dir: &Path, o: &Opts) -> Result<String, String> {
    let host = host_arg(o, "allow-add")?;
    let mut list = door::read_allow(dir).map_err(|e| format!("net allow-add: {e}"))?;
    let added = list.add(&host).map_err(|e| format!("net allow-add: {e}"))?;
    door::write_allow(dir, &list).map_err(|e| format!("net allow-add: {e}"))?;
    let stored = door::host_ok(&host).unwrap_or(host.clone());
    if o.json {
        return Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({
                "host": stored,
                "added": added,
                "allow": list.len(),
                "hosts": list.hosts().collect::<Vec<&str>>(),
                "file": door::allow_path(dir).display().to_string(),
            }))
            .unwrap_or_else(|_| "{}".into())
        ));
    }
    if added {
        Ok(format!(
            "allowed {stored}  ({} host(s) in {})\n",
            list.len(),
            door::ALLOW_FILE
        ))
    } else {
        Ok(format!(
            "{stored} was already listed  ({} host(s) in {})\n",
            list.len(),
            door::ALLOW_FILE
        ))
    }
}

fn allow_del(dir: &Path, o: &Opts) -> Result<String, String> {
    let host = host_arg(o, "allow-del")?;
    let mut list = door::read_allow(dir).map_err(|e| format!("net allow-del: {e}"))?;
    let removed = list
        .remove(&host)
        .map_err(|e| format!("net allow-del: {e}"))?;
    if !removed {
        let stored = door::host_ok(&host).unwrap_or(host.clone());
        return Err(format!(
            "net allow-del: {stored} is not in {}",
            door::ALLOW_FILE
        ));
    }
    door::write_allow(dir, &list).map_err(|e| format!("net allow-del: {e}"))?;
    let stored = door::host_ok(&host).unwrap_or(host.clone());
    if o.json {
        return Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({
                "host": stored,
                "removed": true,
                "allow": list.len(),
                "hosts": list.hosts().collect::<Vec<&str>>(),
                "file": door::allow_path(dir).display().to_string(),
            }))
            .unwrap_or_else(|_| "{}".into())
        ));
    }
    Ok(format!(
        "removed {stored}  ({} host(s) left in {})\n",
        list.len(),
        door::ALLOW_FILE
    ))
}

fn kill(dir: &Path, bit: &str, json_out: bool) -> Result<String, String> {
    let on = match bit {
        "on" => true,
        "off" => false,
        other => return Err(format!("unknown kill state {other:?} — on | off")),
    };
    if on {
        door::kill_on(dir).map_err(|e| format!("net kill on: {e}"))?;
    } else {
        door::kill_off(dir).map_err(|e| format!("net kill off: {e}"))?;
    }
    let set = door::kill_set(dir);
    if json_out {
        return Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({
                "kill": set,
                "file": door::kill_path(dir).display().to_string(),
            }))
            .unwrap_or_else(|_| "{}".into())
        ));
    }
    Ok(format!("kill {}\n", if set { "set" } else { "clear" }))
}

/// Ask the door exactly as a future adapter would. Prints the verdict and
/// exits 0 — a deny is data, not a CLI error.
fn permit(dir: &Path, o: &Opts) -> Result<String, String> {
    let host = host_arg(o, "permit")?;
    let (on, source) = flag_state(dir);
    let verdict = door::permit_at(dir, on, &host, PURPOSE);
    if o.json {
        return Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({
                "host": host,
                "purpose": PURPOSE,
                "allow": verdict.allow,
                "reason": verdict.reason,
                "flag": { "on": on, "source": source },
            }))
            .unwrap_or_else(|_| "{}".into())
        ));
    }
    Ok(format!(
        "{}  {host}  — {}\n",
        if verdict.allow { "allow" } else { "deny" },
        verdict.reason
    ))
}
