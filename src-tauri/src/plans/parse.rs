#![allow(unused_imports)]
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[allow(unused_imports)]
use super::dirs::*;
#[allow(unused_imports)]
use super::locate::*;
#[allow(unused_imports)]
use super::seal::*;
#[allow(unused_imports)]
use super::resolve::*;
#[allow(unused_imports)]
use super::rules::*;

pub(crate) fn snapshot(roots: &Roots) -> Value {
    let chosen = chosen_runner(roots);
    let run = chosen.clone().unwrap_or_else(builtin_runner);
    json!({
        "plansDir": plans_dir(roots).display().to_string(),
        "busDir": bus_dir_in(roots).display().to_string(),
        "repoRoot": repo_root(roots).display().to_string(),
        "runnerChosen": chosen.is_some(),
        "runner": if chosen.is_some() { run.runner } else { String::new() },
        "harness": if chosen.is_some() { run.harness } else { String::new() },
        "model": if chosen.is_some() { run.model } else { String::new() },
        "permissions": chosen.as_ref().and_then(|r| r.permissions.clone()),
        "runnerFallback": chosen.as_ref().and_then(|r| r.runner_fallback.clone()),
    })
}

pub(crate) fn settings_path(roots: &Roots) -> PathBuf {
    roots.home.join(".config/orbitty/settings.json")
}

pub(crate) fn load_settings(roots: &Roots) -> Settings {
    let p = settings_path(roots);
    let Ok(raw) = std::fs::read_to_string(&p) else {
        return Settings::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub(crate) fn save_settings(roots: &Roots, s: &Settings) -> Result<(), String> {
    let p = settings_path(roots);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(&p, raw + "\n").map_err(|e| format!("write {}: {e}", p.display()))
}

pub(crate) fn expand_home(roots: &Roots, raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        return roots.home.join(rest);
    }
    if raw == "~" {
        return roots.home.clone();
    }
    PathBuf::from(raw)
}

pub(crate) fn expand_text(text: &str, repo: &str, plans: &str) -> String {
    text.replace("{REPO_ROOT}", repo)
        .replace("{PLANS_DIR}", plans)
}

pub(crate) fn nonblank(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub(crate) fn seal_timestamp() -> String {
    chrono::Local::now()
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

pub(crate) fn parse_file(path: &Path) -> Result<ParsedGo, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("GO unreadable {}: {e}", path.display()))?;
    parse_text(path, raw)
}

pub(crate) fn parse_text(path: &Path, raw: String) -> Result<ParsedGo, String> {
    let trimmed = raw.trim_start_matches('\u{feff}');
    if !trimmed.starts_with("---") {
        return Ok(ParsedGo {
            path: path.to_path_buf(),
            yaml: String::new(),
            header: GoHeader::default(),
            body: raw.clone(),
            raw,
            legacy: true,
        });
    }
    let (yaml, body) = split_front(trimmed)?;
    let header: GoHeader =
        serde_yaml::from_str(&yaml).map_err(|e| format!("malformed header: {e}"))?;
    let header = normalize_header(header);
    Ok(ParsedGo {
        path: path.to_path_buf(),
        raw,
        yaml,
        header,
        body,
        legacy: false,
    })
}

pub(crate) fn normalize_header(mut h: GoHeader) -> GoHeader {
    h.id = nonblank(h.id.as_deref());
    h.title = nonblank(h.title.as_deref());
    h.status = nonblank(h.status.as_deref()).map(|s| s.to_ascii_lowercase());
    h.sealed_at = nonblank(h.sealed_at.as_deref());
    h.sealed_by = nonblank(h.sealed_by.as_deref());
    h.seal_hash = nonblank(h.seal_hash.as_deref());
    h.runner = nonblank(h.runner.as_deref());
    h.harness = nonblank(h.harness.as_deref());
    h.model = nonblank(h.model.as_deref());
    h.workspace = nonblank(h.workspace.as_deref());
    h.plans_dir = nonblank(h.plans_dir.as_deref());
    h.card_id = nonblank(h.card_id.as_deref());
    h.lock = nonblank(h.lock.as_deref());
    if h.runner_fallback.as_ref().is_some_and(|f| f.blank()) {
        h.runner_fallback = None;
    }
    if h.tools.as_ref().is_some_and(|t| t.is_empty()) {
        h.tools = None;
    }
    h
}

pub(crate) fn split_front(raw: &str) -> Result<(String, String), String> {
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    if !raw.starts_with("---") {
        return Err("malformed header: no opening ---".into());
    }
    let after = &raw[3..];
    let after = after.strip_prefix('\r').unwrap_or(after);
    let after = after
        .strip_prefix('\n')
        .ok_or_else(|| "malformed header: opening ---".to_string())?;
    let mut line_start = 0usize;
    while line_start <= after.len() {
        let rest = &after[line_start..];
        let nl = rest.find('\n').map(|i| line_start + i);
        let line_end = nl.unwrap_or(after.len());
        let line = after[line_start..line_end].trim_end_matches('\r');
        if line == "---" {
            let body_at = nl.map(|i| i + 1).unwrap_or(after.len());
            return Ok((after[..line_start].to_string(), after[body_at..].to_string()));
        }
        let Some(i) = nl else { break };
        line_start = i + 1;
    }
    Err("malformed header: no closing ---".into())
}

pub(crate) fn file_url_to_path(url: &str) -> Option<PathBuf> {
    let u = url.trim();
    let rest = u
        .strip_prefix("file://")
        .or_else(|| u.strip_prefix("FILE://"))?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let decoded = percent_decode(rest);
    if decoded.is_empty() || !decoded.starts_with('/') {
        return None;
    }
    Some(PathBuf::from(decoded))
}

pub(crate) fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (from_hex(b[i + 1]), from_hex(b[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub(crate) fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub(crate) fn is_inside(path: &Path, dir: &Path) -> bool {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    path.starts_with(dir)
}

pub(crate) fn walk_md(dir: &Path, out: &mut Vec<(PathBuf, usize)>, depth: usize) {
    if depth > WALK_LIMIT {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            walk_md(&p, out, depth + 1);
        } else if name.ends_with(".md") {
            out.push((p, depth));
        }
    }
}

pub(crate) fn walk_any(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 8 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            walk_any(&p, out, depth + 1);
        } else if p.is_file() {
            out.push(p);
        }
    }
}

pub(crate) fn herdr_bin_present() -> bool {
    which("herdr")
}

pub(crate) fn which(bin: &str) -> bool {
    let path = std::env::var("PATH").unwrap_or_default();
    for dir in path.split(':') {
        if Path::new(dir).join(bin).is_file() {
            return true;
        }
    }
    false
}


pub(crate) fn legacy_token_from_path(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix('_')?.strip_suffix("_PROMPT.md")?;
    if rest.is_empty() {
        return None;
    }
    Some(rest.replace('_', "-"))
}
