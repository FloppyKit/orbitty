use std::path::{ PathBuf };
use std::sync::atomic::{AtomicBool, Ordering};

/// pub(crate): mouth_hop keeps the BYOK OpenRouter key next to plug.json.
pub(crate) fn companions_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".agents")
        .join("companions")
        .join("orbitty-primary")
}

pub(crate) fn berd_agents_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".agents").join("agents")
}

/// GO-MH-1-APP: four-cast allowlist. Missing/bad JSON = fail open (full stem
/// list) and log once. Never empty the picker on a typo.
#[derive(serde::Deserialize)]
pub(crate) struct CompanionAllowlist {
    pub(crate) version: u64,
    pub(crate) personas: Vec<String>,
    #[serde(default, rename = "default")]
    pub(crate) default_stem: Option<String>,
}

pub(crate) static ALLOWLIST_WARNED: AtomicBool = AtomicBool::new(false);

pub(crate) fn warn_allowlist_once(why: &str) {
    if !ALLOWLIST_WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("orbitty: companion allowlist fail-open ({why})");
    }
}

pub(crate) fn load_companion_allowlist() -> Option<CompanionAllowlist> {
    let p = companions_root().join("allowlist.json");
    if !p.exists() {
        return None;
    }
    let raw = match std::fs::read_to_string(&p) {
        Ok(s) => s,
        Err(e) => {
            warn_allowlist_once(&format!("unreadable: {e}"));
            return None;
        }
    };
    match serde_json::from_str::<CompanionAllowlist>(&raw) {
        Ok(al) if al.version >= 1 && !al.personas.is_empty() => Some(al),
        Ok(_) => {
            warn_allowlist_once("missing version or personas");
            None
        }
        Err(e) => {
            warn_allowlist_once(&format!("bad JSON: {e}"));
            None
        }
    }
}

/// Berd picker roster: allowlist order ∩ stems that still exist as `.md`.
/// Missing/bad allowlist → full `berd_agent_stems()` (fail open).
pub(crate) fn berd_companion_stems() -> (Vec<String>, String) {
    let all = berd_agent_stems();
    let Some(al) = load_companion_allowlist() else {
        return (all, "spark".into());
    };
    let filtered: Vec<String> = al
        .personas
        .iter()
        .filter(|p| all.iter().any(|s| s == *p))
        .cloned()
        .collect();
    if filtered.is_empty() {
        warn_allowlist_once("no personas exist on disk");
        return (all, "spark".into());
    }
    let pref = al
        .default_stem
        .filter(|d| filtered.iter().any(|s| s == d))
        .unwrap_or_else(|| "spark".into());
    (filtered, pref)
}

/// Berd agent stems from `~/.agents/agents/*.md` (sorted).
pub(crate) fn berd_agent_stems() -> Vec<String> {
    let dir = berd_agents_dir();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("md") {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
        .filter(|n| !n.starts_with('.') && !n.starts_with("untitled"))
        .collect();
    out.sort();
    out
}

/// Hermes profile homes (legacy backend / planet handoff only).
pub(crate) fn hermes_profile_dirs() -> Vec<String> {
    let Ok(home) = std::env::var("HOME") else {
        return Vec::new();
    };
    let dir = PathBuf::from(home).join(".hermes").join("profiles");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| !n.starts_with('.'))
        .collect();
    out.sort();
    out
}

pub(crate) fn resolve_agent(persisted: &str, agents: &[String]) -> String {
    resolve_agent_prefer(persisted, agents, "spark")
}

/// Unset / stale persisted profile → `prefer` (spark) if on the list, else
/// grok, else first, else the prefer string so the picker never shows "".
pub(crate) fn resolve_agent_prefer(persisted: &str, agents: &[String], prefer: &str) -> String {
    let p = persisted.trim();
    // Legacy Hermes "default" is not a Berd agent stem — fall through.
    if !p.is_empty() && p != "default" && agents.iter().any(|d| d == p) {
        return p.to_string();
    }
    if agents.iter().any(|d| d == prefer) {
        prefer.into()
    } else if agents.iter().any(|d| d == "spark") {
        "spark".into()
    } else if agents.iter().any(|d| d == "grok") {
        "grok".into()
    } else if let Some(first) = agents.first() {
        first.clone()
    } else {
        prefer.into()
    }
}

/// Read bound Berd session id from companions/orbitty-primary/session.json.
pub(crate) fn load_companion_session_id() -> Result<String, String> {
    let p = companions_root().join("session.json");
    let raw = std::fs::read_to_string(&p).map_err(|_| {
        format!(
            "Companion session not bound — missing {}. Run hermes-to-berd slice 03.",
            p.display()
        )
    })?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", p.display()))?;
    let sid = v
        .get("berd_session_id")
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "null")
        .ok_or_else(|| {
            format!(
                "Companion session.json has no berd_session_id — run 03_companion_session.sh --apply"
            )
        })?;
    Ok(sid.to_string())
}

