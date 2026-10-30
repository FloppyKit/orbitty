use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use super::action::{find_on_path};
use super::allowlist::{berd_agent_stems, berd_agents_dir, companions_root, resolve_agent_prefer};

/// Prefer PATH, then Berd.app, then ~/.local/bin.
pub(crate) fn find_berdctl() -> Option<PathBuf> {
    if let Some(p) = find_on_path("berdctl") {
        return Some(p);
    }
    let app = PathBuf::from("/Applications/Berd.app/Contents/MacOS/berdctl");
    if app.is_file() {
        return Some(app);
    }
    None
}

/// Berd only accepts berdctl when it can find the app control discovery file.
/// Agent sessions get `BERDCTL_LOCK` from Berd; Orbitty.app does not — so we
/// locate `~/Library/Application Support/xyz.block.berd/berdctl/control-*.json`
/// for a live Berd pid and pass it through on every spawn.
pub(crate) fn berdctl_lock_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("BERDCTL_LOCK") {
        let pb = PathBuf::from(p.trim());
        if pb.is_file() {
            return Some(pb);
        }
    }
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home)
        .join("Library/Application Support/xyz.block.berd/berdctl");
    let entries = std::fs::read_dir(&dir).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for ent in entries.flatten() {
        let path = ent.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.starts_with("control-") || path.extension().and_then(|e| e.to_str()) != Some("json")
        {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        let Some(pid) = v.get("pid").and_then(|p| p.as_u64()) else {
            continue;
        };
        // Alive check: kill -0 (no signal delivered).
        let alive = Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !alive {
            continue;
        }
        let modified = ent
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        if best.as_ref().map(|(t, _)| modified >= *t).unwrap_or(true) {
            best = Some((modified, path));
        }
    }
    best.map(|(_, p)| p)
}

/// berdctl Command with BERDCTL_LOCK set when we can discover Berd's control file.
pub(crate) fn berdctl_cmd(bin: &Path) -> Result<Command, String> {
    let lock = berdctl_lock_path().ok_or_else(|| {
        "Berd: berdctl needs a live Berd app control lock — open Berd desktop, then try again."
            .to_string()
    })?;
    let mut cmd = Command::new(bin);
    cmd.env("BERDCTL_LOCK", &lock);
    Ok(cmd)
}

pub(crate) fn update_soul_pointer(agent_stem: &str) {
    let p = companions_root().join("soul.json");
    let agent_path = berd_agents_dir().join(format!("{agent_stem}.md"));
    let mut v = if p.exists() {
        serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_default())
            .unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    if let Some(obj) = v.as_object_mut() {
        obj.insert(
            "agent_stem".into(),
            serde_json::Value::String(agent_stem.to_string()),
        );
        obj.insert(
            "agent_path".into(),
            serde_json::Value::String(agent_path.display().to_string()),
        );
        obj.insert(
            "companion_id".into(),
            serde_json::Value::String("orbitty-primary".into()),
        );
    }
    let _ = std::fs::create_dir_all(companions_root());
    let _ = std::fs::write(p, serde_json::to_string_pretty(&v).unwrap_or_default() + "\n");
}

// Goose/grok mouths + plug.json sessions

/// Persisted plug sessions: `companions/orbitty-primary/plug.json`.
/// Product sessions only — NEVER the Berd `session.json` id (`20260825_2`)
/// and never the orbitty-plug probe ids; those stay in the cascade's own
/// state.json. This file is the only plug state Orbitty writes.
/// pub(crate): mouth_roll.rs rolls a dead session onto a fresh id and
/// saves through this same map.
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub(crate) struct PlugState {
    /// Goose `YYYYMMDD_N`, captured from `goose session list` after the
    /// first `--name orbitty-companion` send (`--name` is not a unique
    /// resume key — resume needs `--session-id ID --resume`).
    #[serde(default)]
    pub(crate) goose_session_id: Option<String>,
    /// Grok UUID, minted on first use. First send `-s UUID`; later sends
    /// `--resume UUID` (re-passing `-s` on a live id fails).
    #[serde(default)]
    pub(crate) grok_session_id: Option<String>,
    #[serde(default)]
    pub(crate) grok_session_minted: bool,
    /// Successful Grok mouth sends on the current backend session.
    /// Count only. A reset is a dead session, a char budget, thin, or
    /// manual_test — not a fixed send interval.
    #[serde(default)]
    pub(crate) grok_sends: u32,
    /// Prompt + reply chars on the current Grok session. Flag on: a
    /// budget reset fires at `MH_SESSION_BUDGET_CHARS`.
    #[serde(default)]
    pub(crate) grok_chars: u64,
    /// `thin` or `manual_test`, consumed by the next Grok send.
    #[serde(default)]
    pub(crate) reset_pending: Option<String>,
}

pub(crate) fn plug_path() -> PathBuf {
    companions_root().join("plug.json")
}

pub(crate) fn load_plug() -> PlugState {
    let Ok(raw) = std::fs::read_to_string(plug_path()) else {
        return PlugState::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Atomic-ish (tmp + rename), best-effort — a lost write just means the
/// next send re-learns the session id.
pub(crate) fn save_plug(plug: &PlugState) {
    let p = plug_path();
    let _ = std::fs::create_dir_all(companions_root());
    let Ok(body) = serde_json::to_string_pretty(plug) else {
        return;
    };
    let tmp = p.with_extension("json.tmp");
    if std::fs::write(&tmp, body + "\n").is_ok() {
        let _ = std::fs::rename(&tmp, &p);
    }
}

/// Soul text for the goose/grok mouth: the picked agent's md
/// (`~/.agents/agents/{agent_profile or spark}.md`), capped ~4k chars so
/// argv stays sane. Missing file → a one-line floor, never an empty flag.
/// YAML frontmatter is stripped (clap reads a leading-dash value as
/// another flag — Matt's spark.md ping died on `---`), and the body
/// never starts with `-`.
/// pub(crate): mouth_hop rides the same soul as the system message of a
/// stateless hop lane (Grok gets it via --rules). RAM reads this same
/// string when `memory_ram` is on. The 4096 cap stays here.
pub(crate) fn companion_soul_text(app: &tauri::AppHandle) -> String {
    const SOUL_CAP: usize = 4096;
    let persisted = crate::state::load_state(app)
        .map(|s| s.companion.agent_profile)
        .unwrap_or_default();
    let stems = berd_agent_stems();
    let stem = resolve_agent_prefer(&persisted, &stems, "spark");
    let text = std::fs::read_to_string(berd_agents_dir().join(format!("{stem}.md")))
        .unwrap_or_default();
    let body = strip_frontmatter(&text);
    let soul = if body.is_empty() {
        "You are Spark, a warm, brief desk companion.".to_string()
    } else {
        body
    };
    // Belt-and-suspenders next to the `--flag=value` argv form: a value
    // that starts with `-` is a flag to clap.
    let soul = if soul.starts_with('-') {
        format!("Spark:\n{soul}")
    } else {
        soul
    };
    if soul.chars().count() > SOUL_CAP {
        soul.chars().take(SOUL_CAP).collect::<String>()
    } else {
        soul
    }
}

/// Drop a leading YAML frontmatter block (`---` fence … closing `---`
/// line), keeping the markdown body. No closing fence → not frontmatter,
/// returned unchanged (the leading-`-` guard above still applies).
pub(crate) fn strip_frontmatter(md: &str) -> String {
    let t = md.trim();
    let mut lines = t.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    if first.trim() != "---" {
        return t.to_string();
    }
    let rest: Vec<&str> = lines.collect();
    match rest.iter().position(|l| l.trim() == "---") {
        Some(close) => rest[close + 1..].join("\n").trim().to_string(),
        None => t.to_string(),
    }
}

/// Grok bin: PATH + the usual fallback dirs, then `~/.grok/bin/grok` —
/// Orbitty.app often has a thin PATH.
pub(crate) fn find_grok() -> Option<PathBuf> {
    if let Some(p) = find_on_path("grok") {
        return Some(p);
    }
    let home = std::env::var("HOME").ok()?;
    let p = PathBuf::from(home).join(".grok").join("bin").join("grok");
    p.is_file().then_some(p)
}

