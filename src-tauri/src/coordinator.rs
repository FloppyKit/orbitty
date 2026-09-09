//! S5 coordinator + soul.md (LIVESPRITE_S5_LOCKS_v0.md, GO-LS-TAURI-S5).
//!
//! Orbitty launches/focuses an EXISTING Hermes (app or CLI) and hands it
//! a titles-only context file. No model, no agent loop inside the sprite
//! engine — this module never embeds an LLM.
//!
//! Context: `app_data/context/{projectId}.md` — overwritten on each launch;
//! project name + claimed window titles + open to-do titles only (no file
//! contents, no PHI).
//! Soul:    `app_data/souls/{projectId}/soul.md` — read always; the ONLY
//! writers are the explicit Save button (`save_soul`) or the human in
//! $EDITOR ("Open in editor"). Never auto-summarized from window titles.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};

fn app_data(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Project ids are app-generated (`p-…`); filter anyway so a hand-edited
/// state.json can never become a path traversal.
fn safe_id(project_id: &str) -> String {
    project_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn context_path(app: &tauri::AppHandle, project_id: &str) -> Result<PathBuf, String> {
    let dir = app_data(app)?.join("context");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir.join(format!("{}.md", safe_id(project_id))))
}

/// Titles-only context, overwritten every launch (never appended).
fn write_context(
    app: &tauri::AppHandle,
    project_id: &str,
    project_name: &str,
    titles: &[String],
) -> Result<PathBuf, String> {
    let path = context_path(app, project_id)?;
    let mut body = format!(
        "# {project_name}\n\nOrbitty planet context — titles only. No file contents, no PHI.\n"
    );
    body.push('\n');
    if titles.is_empty() {
        body.push_str("(no claimed windows or open to-dos)\n");
    } else {
        for t in titles {
            body.push_str(&format!("- {}\n", t.replace('\n', " ")));
        }
    }
    std::fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(target_os = "macos")]
fn try_open_hermes_app() -> bool {
    Command::new("open")
        .args(["-a", "Hermes"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn try_open_hermes_app() -> bool {
    false
}

/// Detached spawn: the child outlives the invoke; a NotFound error just
/// falls through to the next launcher.
fn spawn_detached(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

/// Shared Hermes handoff (S5 planets only — the S2.4 companion sends
/// through its own named-thread path below). Tries in order:
/// focus a running Hermes app → `open -a Hermes` → `hermes desktop` →
/// `hermes gui` → PATH `hermes` (grok chat with `cli_query`;
/// `--create-if-missing` is the existing last-resort, same as S5). Err
/// when no Hermes exists anywhere — the UI shows the path hint.
fn route_to_hermes(ctx: &Path, cli_query: &str) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    if mac::focus_running_hermes() {
        return Ok(format!("Focused the running Hermes. Context: {}", ctx.display()));
    }
    if try_open_hermes_app() {
        return Ok(format!("Opened Hermes.app. Context: {}", ctx.display()));
    }
    if spawn_detached("hermes", &["desktop"]) {
        return Ok(format!("Launched `hermes desktop`. Context: {}", ctx.display()));
    }
    if spawn_detached("hermes", &["gui"]) {
        return Ok(format!("Launched `hermes gui`. Context: {}", ctx.display()));
    }
    if spawn_detached("hermes", &["-p", "grok", "chat", "--create-if-missing", cli_query]) {
        return Ok(format!("Launched Hermes CLI (grok). Context: {}", ctx.display()));
    }
    Err(format!(
        "No Hermes found — install the Hermes app or put `hermes` on PATH. Context file ready: {}",
        ctx.display()
    ))
}

/// Launch/focus the existing Hermes harness for a project. Writes the
/// titles-only context file first, then hands off via route_to_hermes.
#[tauri::command]
pub fn launch_coordinator(
    app: tauri::AppHandle,
    project_id: String,
    project_name: String,
    titles: Vec<String>,
) -> Result<String, String> {
    let ctx = write_context(&app, &project_id, &project_name, &titles)?;
    let query = format!(
        "Orbitty planet \"{project_name}\" — titles-only context: {}",
        ctx.display()
    );
    route_to_hermes(&ctx, &query)
}

// ---------- S2.4 companion: plug map (GO-ORBITTY-PLUG-APP) ----------
// The mouth is a plug map now — off the Berd/Goose combo. Default mouth is
// **Goose** (`goose run` + plug.json session); `grok` is a mouth; `hermes`
// is compat; `berd` (`berdctl` + ~/.agents/companions/orbitty-primary) is
// the hidden escape only. Never Cursor / OpenCode / ocx / Claude / Codex /
// Pi. ORBITTY_COMPANION_BACKEND picks; an unknown value is an honest
// error at send — never a silent Berd fallback.

/// Plug map: unset/empty/`goose` → `goose` (default mouth); `grok`;
/// `hermes` (compat); `berd` (hidden escape). Anything else is returned
/// raw so the send path can fail on it honestly.
fn companion_backend() -> String {
    let raw = std::env::var("ORBITTY_COMPANION_BACKEND").unwrap_or_default();
    let b = raw.trim().to_ascii_lowercase();
    if b.is_empty() {
        "goose".into()
    } else {
        b
    }
}

fn companions_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".agents")
        .join("companions")
        .join("orbitty-primary")
}

fn berd_agents_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".agents").join("agents")
}

/// GO-MH-1-APP: four-cast allowlist. Missing/bad JSON = fail open (full stem
/// list) and log once. Never empty the picker on a typo.
#[derive(serde::Deserialize)]
struct CompanionAllowlist {
    version: u64,
    personas: Vec<String>,
    #[serde(default, rename = "default")]
    default_stem: Option<String>,
}

static ALLOWLIST_WARNED: AtomicBool = AtomicBool::new(false);

fn warn_allowlist_once(why: &str) {
    if !ALLOWLIST_WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("orbitty: companion allowlist fail-open ({why})");
    }
}

fn load_companion_allowlist() -> Option<CompanionAllowlist> {
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
fn berd_companion_stems() -> (Vec<String>, String) {
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
fn berd_agent_stems() -> Vec<String> {
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
fn hermes_profile_dirs() -> Vec<String> {
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

fn resolve_agent(persisted: &str, agents: &[String]) -> String {
    resolve_agent_prefer(persisted, agents, "spark")
}

/// Unset / stale persisted profile → `prefer` (spark) if on the list, else
/// grok, else first, else the prefer string so the picker never shows "".
fn resolve_agent_prefer(persisted: &str, agents: &[String], prefer: &str) -> String {
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
fn load_companion_session_id() -> Result<String, String> {
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

/// Prefer PATH, then Berd.app, then ~/.local/bin.
fn find_berdctl() -> Option<PathBuf> {
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
fn berdctl_lock_path() -> Option<PathBuf> {
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
fn berdctl_cmd(bin: &Path) -> Result<Command, String> {
    let lock = berdctl_lock_path().ok_or_else(|| {
        "Berd: berdctl needs a live Berd app control lock — open Berd desktop, then try again."
            .to_string()
    })?;
    let mut cmd = Command::new(bin);
    cmd.env("BERDCTL_LOCK", &lock);
    Ok(cmd)
}

fn update_soul_pointer(agent_stem: &str) {
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

// ---------- GO-ORBITTY-PLUG-APP: goose/grok mouths + plug.json sessions ----------

/// Persisted plug sessions: `companions/orbitty-primary/plug.json`.
/// Product sessions only — NEVER the Berd `session.json` id (`20260825_2`)
/// and never the orbitty-plug probe ids; those stay in the cascade's own
/// state.json. This file is the only plug state Orbitty writes.
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
struct PlugState {
    /// Goose `YYYYMMDD_N`, captured from `goose session list` after the
    /// first `--name orbitty-companion` send (`--name` is not a unique
    /// resume key — resume needs `--session-id ID --resume`).
    #[serde(default)]
    goose_session_id: Option<String>,
    /// Grok UUID, minted on first use. First send `-s UUID`; later sends
    /// `--resume UUID` (re-passing `-s` on a live id fails).
    #[serde(default)]
    grok_session_id: Option<String>,
    #[serde(default)]
    grok_session_minted: bool,
}

fn plug_path() -> PathBuf {
    companions_root().join("plug.json")
}

fn load_plug() -> PlugState {
    let Ok(raw) = std::fs::read_to_string(plug_path()) else {
        return PlugState::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Atomic-ish (tmp + rename), best-effort — a lost write just means the
/// next send re-learns the session id.
fn save_plug(plug: &PlugState) {
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
fn companion_soul_text(app: &tauri::AppHandle) -> String {
    const SOUL_CAP: usize = 4096;
    let persisted = crate::state::load_state(app)
        .map(|s| s.companion.agent_profile)
        .unwrap_or_default();
    let stems = berd_agent_stems();
    let stem = resolve_agent_prefer(&persisted, &stems, "spark");
    let text = std::fs::read_to_string(berd_agents_dir().join(format!("{stem}.md")))
        .unwrap_or_default()
        .trim()
        .to_string();
    let soul = if text.is_empty() {
        "You are Spark, a warm, brief desk companion.".to_string()
    } else {
        text
    };
    if soul.chars().count() > SOUL_CAP {
        soul.chars().take(SOUL_CAP).collect::<String>()
    } else {
        soul
    }
}

/// Grok bin: PATH + the usual fallback dirs, then `~/.grok/bin/grok` —
/// Orbitty.app often has a thin PATH.
fn find_grok() -> Option<PathBuf> {
    if let Some(p) = find_on_path("grok") {
        return Some(p);
    }
    let home = std::env::var("HOME").ok()?;
    let p = PathBuf::from(home).join(".grok").join("bin").join("grok");
    p.is_file().then_some(p)
}

fn is_uuid(s: &str) -> bool {
    let lens = [8usize, 4, 4, 4, 12];
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip(lens)
            .all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// v4 UUID via macOS `uuidgen` (always present on the dogfood target).
fn mint_uuid() -> Result<String, String> {
    let out = Command::new("uuidgen")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("launch uuidgen: {e}"))?;
    if !out.status.success() {
        return Err("uuidgen failed — cannot mint a grok session id.".into());
    }
    let id = String::from_utf8_lossy(&out.stdout).trim().to_ascii_lowercase();
    if !is_uuid(&id) {
        return Err("uuidgen returned a bad id.".into());
    }
    Ok(id)
}

/// Goose session ids look like `20260909_2` (YYYYMMDD_N).
fn is_goose_session_id(s: &str) -> bool {
    let mut parts = s.split('_');
    let (Some(d), Some(n), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    d.len() == 8
        && d.chars().all(|c| c.is_ascii_digit())
        && !n.is_empty()
        && n.chars().all(|c| c.is_ascii_digit())
}

/// Spawn a mouth CLI, poll with a timeout, kill the child on expiry.
/// stdout on exit 0; one honest stderr line otherwise. No desktop lock is
/// ever set here — BERDCTL_LOCK belongs to the berd escape path only.
fn run_mouth_cli(tool: &str, bin: &Path, args: &[String], timeout_secs: u64) -> Result<String, String> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("launch {}: {e}", bin.display()))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let out = child
                    .wait_with_output()
                    .map_err(|e| format!("collect {tool} output: {e}"))?;
                if !out.status.success() {
                    return Err(tool_error_line(tool, &out.stderr, &out.stdout, out.status.code()));
                }
                return Ok(String::from_utf8_lossy(&out.stdout).trim().to_string());
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "{tool} took longer than {timeout_secs}s — killed. Try again."
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(format!("wait on {}: {e}", tool.to_ascii_lowercase())),
        }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionAgents {
    /// Berd agent stems (or Hermes profiles when backend=hermes).
    pub agents: Vec<String>,
    /// Effective pick after the unset/stale fallback rules.
    pub selected: String,
}

/// Agent picker roster for the companion window. goose/grok/berd share
/// the four-cast stems; hermes keeps its profile dirs; an unknown backend
/// is an honest error (the page degrades to a single "default" row).
#[tauri::command]
pub fn companion_agents(app: tauri::AppHandle) -> Result<CompanionAgents, String> {
    let backend = companion_backend();
    let (agents, selected) = match backend.as_str() {
        "hermes" => {
            let mut a = vec!["default".to_string()];
            a.extend(hermes_profile_dirs());
            let persisted = crate::state::load_state(&app)
                .map(|s| s.companion.agent_profile)
                .unwrap_or_default();
            let selected = resolve_agent(&persisted, &a);
            (a, selected)
        }
        "goose" | "grok" | "berd" => {
            let (agents, prefer) = berd_companion_stems();
            let persisted = crate::state::load_state(&app)
                .map(|s| s.companion.agent_profile)
                .unwrap_or_default();
            let selected = resolve_agent_prefer(&persisted, &agents, &prefer);
            (agents, selected)
        }
        other => {
            return Err(format!(
                "Unknown ORBITTY_COMPANION_BACKEND {other:?} — goose, grok, hermes, or berd."
            ))
        }
    };
    Ok(CompanionAgents { agents, selected })
}

/// Persist the companion body pack. Empty / "spirit" → blessed spirit.
/// Only `role: companion` packs (or a still-valid existing id) are allowed.
#[tauri::command]
pub fn set_companion_pack(app: tauri::AppHandle, pack_id: String) -> Result<Option<String>, String> {
    let raw = pack_id.trim();
    let id = if raw.is_empty() || raw.eq_ignore_ascii_case("spirit") {
        None
    } else {
        Some(raw.to_string())
    };
    if let Some(ref pid) = id {
        // GO-ORBITTY-COMPANION-3D: tang/female are reserved 3D bodies
        // bundled at companion/3d — never pack folders, so the pack-art
        // check is skipped for them.
        let reserved_3d = pid == "tang" || pid == "female";
        if !reserved_3d && crate::packs::companion_pack_art(&app, pid).is_none() {
            return Err(format!("companion pack {pid:?} missing"));
        }
    }
    let mut st = crate::state::load_state(&app)?;
    st.companion.image_pack_id = id.clone();
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    Ok(id)
}

/// Persist the companion's agent pick (soul voice label).
/// goose/grok/berd: the four-cast stems; the soul pointer updates like
/// Berd (goose/grok read that md as --system/--rules on the next send).
/// Hermes path: next send continues that profile's orbitty-companion thread.
#[tauri::command]
pub fn set_companion_agent(app: tauri::AppHandle, profile: String) -> Result<String, String> {
    let backend = companion_backend();
    let p = profile.trim().to_string();
    match backend.as_str() {
        "hermes" => {
            let dirs = hermes_profile_dirs();
            if p != "default" && !dirs.iter().any(|d| d == &p) {
                return Err(format!("No Hermes profile named {p:?}."));
            }
        }
        "goose" | "grok" | "berd" => {
            let stems = berd_agent_stems();
            if !stems.iter().any(|d| d == &p) {
                return Err(format!("No agent named {p:?} under ~/.agents/agents."));
            }
            if let Some(al) = load_companion_allowlist() {
                if !al.personas.iter().any(|d| d == &p) {
                    return Err(format!("{p} is not on the Orbitty four-cast."));
                }
            }
            update_soul_pointer(&p);
        }
        other => {
            return Err(format!(
                "Unknown ORBITTY_COMPANION_BACKEND {other:?} — goose, grok, hermes, or berd."
            ))
        }
    }
    let mut st = crate::state::load_state(&app)?;
    st.companion.agent_profile = p.clone();
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    Ok(p)
}

/// GO-ORBITTY-SETTINGS-WIN: persist Talk source (`system` | `hotkey`).
#[tauri::command]
pub fn set_companion_talk_source(app: tauri::AppHandle, source: String) -> Result<String, String> {
    let src = crate::state::normalize_talk_source(&source);
    let mut st = crate::state::load_state(&app)?;
    st.companion.talk_source = src.clone();
    crate::state::save_state(&app, &st)?;
    crate::talk::stop_if_any();
    let _ = app.emit("state:changed", ());
    Ok(src)
}

/// Persist expand/collapse so the window reopens the way it was left.
#[tauri::command]
pub fn set_companion_expanded(app: tauri::AppHandle, expanded: bool) -> Result<(), String> {
    let mut st = crate::state::load_state(&app)?;
    st.companion.expanded = expanded;
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    // GO-ORBITTY-SPRITE-HUG: expanding while hugged restores the tall
    // idle FIRST — the transcript is never a 120px slab.
    if expanded {
        crate::panes::unhug_companion(&app);
    }
    // GO-ORBITTY-COMPANION-CLEAR: expand forces the slab (mouse on) NOW,
    // before the JS re-render; collapse reapplies the ghost from kept hits.
    crate::companion_clear::expanded_changed(&app, expanded);
    Ok(())
}

/// GO-ORBITTY-COMPANION-HEADROOM: persist the Full/Head view for ONE
/// surface (`surface` = "collapsed" | "expanded", `view` = "full" |
/// "head"). Two prefs, never one global bool — the other surface's key
/// is untouched. Returns the value actually written.
#[tauri::command]
pub fn set_companion_view(
    app: tauri::AppHandle,
    surface: String,
    view: String,
) -> Result<String, String> {
    let mut st = crate::state::load_state(&app)?;
    let written = match surface.trim().to_ascii_lowercase().as_str() {
        "collapsed" => {
            let v = crate::state::normalize_companion_view(&view, "full");
            st.companion.view_collapsed = v.clone();
            v
        }
        "expanded" => {
            let v = crate::state::normalize_companion_view(&view, "head");
            st.companion.view_expanded = v.clone();
            v
        }
        other => return Err(format!("bad companion surface: {other:?}")),
    };
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    Ok(written)
}

/// GO-ORBITTY-SPRITE-OFF: persist the one sprite flag (default on; a
/// missing key loads true). Same shape as set_companion_speak. R-click on
/// the Full/Head button is the only writer; the per-surface Full/Head
/// prefs are never touched here — they survive Off.
/// GO-ORBITTY-SPRITE-HUG: idle collapsed + sprite off hugs the window to
/// the JS-measured CC+dock bar (`hug_height`), keeping the bottom edge;
/// sprite on restores the tall idle. Expanded/hosted never hug. A call
/// with the flag unchanged is the page's hug-height re-report — no
/// persist, no emit, just the bar re-fit.
#[tauri::command]
pub fn set_companion_sprite(
    app: tauri::AppHandle,
    enabled: bool,
    hug_height: Option<f64>,
) -> Result<(), String> {
    let mut st = crate::state::load_state(&app)?;
    if st.companion.sprite != enabled {
        st.companion.sprite = enabled;
        crate::state::save_state(&app, &st)?;
        let _ = app.emit("state:changed", ());
    }
    if enabled {
        crate::panes::unhug_companion(&app);
    } else if !st.companion.expanded {
        crate::panes::hug_companion(&app, hug_height);
    }
    Ok(())
}

// ---------- S2.7 speak replies: macOS `say`, default system voice ----------

/// The in-flight `say` child (at most one). A finished child stays in the
/// slot until the next kill — killing a dead child is a harmless Err.
static SAY_CHILD: std::sync::OnceLock<std::sync::Mutex<Option<std::process::Child>>> =
    std::sync::OnceLock::new();

fn say_slot() -> &'static std::sync::Mutex<Option<std::process::Child>> {
    SAY_CHILD.get_or_init(|| std::sync::Mutex::new(None))
}

/// Kill the in-flight `say`, if any. Child-handle only — never a
/// system-wide pkill, so a user's own `say` in a terminal is untouched.
fn stop_say() {
    if let Ok(mut slot) = say_slot().lock() {
        if let Some(mut child) = slot.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Persist the Speak toggle. Turning it OFF kills any in-flight `say`.
#[tauri::command]
pub fn set_companion_speak(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if !enabled {
        stop_say();
    }
    let mut st = crate::state::load_state(&app)?;
    st.companion.speak_replies = enabled;
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

// ---------- GO-LS-STARS-STATUS: prompts opt-in + strip ack ----------
// The companion window never writes state.json wholesale; these two
// load-modify-save commands are its only STARS writes. The rail's STARS
// pane writes checkins through its own full persist.

/// Persist the Prompts opt-in (default off). Mirrored by the rail's STARS
/// pane toggle and the companion's Prompts toggle — same key either way.
#[tauri::command]
pub fn set_stars_prompts(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let mut st = crate::state::load_state(&app)?;
    st.stars.prompts_opt_in = enabled;
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Dismiss the companion's prompt strip: `ack` is `"{day}:{sig}"` — the
/// strip stays hidden until the day rolls or a fresh check-in changes the
/// sig. One show per day, no loop.
#[tauri::command]
pub fn ack_stars_prompt(app: tauri::AppHandle, ack: String) -> Result<(), String> {
    let mut st = crate::state::load_state(&app)?;
    st.stars.prompts_ack = ack;
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Read one bot reply aloud with the DEFAULT system voice (`say "<text>"`
/// — no -v, no voice listing). Kills the previous `say` first, so a new
/// reply always wins. macOS only; one honest Err line, never a crash.
#[tauri::command]
pub fn speak_reply(text: String) -> Result<(), String> {
    stop_say();
    let text = text.trim().to_string();
    if text.is_empty() {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let child = Command::new("say")
            .arg(&text)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Couldn't speak the reply — `say` failed to start: {e}"))?;
        if let Ok(mut slot) = say_slot().lock() {
            *slot = Some(child);
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        Err("Speak replies needs macOS `say`.".into())
    }
}

// ---------- GO-ORBITTY-ASK-ACTION: a typed line can name a Launchpad Action ----------

/// `Run` carries the shortcut plus `keep_you`: prefix/command chrome never
/// persists a you-line; a bare name typed as a sentence keeps it.
enum ActionIntercept {
    Run(crate::state::ActionShortcut, bool),
    Ambiguous,
}

/// Local match only, in law order: exact name (case-insensitive), whole
/// line == command, then `run|/run|action <name>`. No fuzzy, no NLP —
/// `please run Build` is a chat line. Two equal candidates in one tier →
/// Ambiguous, which never fires.
fn match_action_line(
    shortcuts: &[crate::state::ActionShortcut],
    line: &str,
) -> Option<ActionIntercept> {
    if shortcuts.is_empty() {
        return None;
    }

    let by_name: Vec<_> = shortcuts
        .iter()
        .filter(|a| a.name.trim().eq_ignore_ascii_case(line))
        .collect();
    if by_name.len() == 1 {
        return Some(ActionIntercept::Run(by_name[0].clone(), true));
    }
    if by_name.len() > 1 {
        return Some(ActionIntercept::Ambiguous);
    }

    let by_command: Vec<_> = shortcuts
        .iter()
        .filter(|a| a.command.trim() == line)
        .collect();
    if by_command.len() == 1 {
        return Some(ActionIntercept::Run(by_command[0].clone(), false));
    }
    if by_command.len() > 1 {
        return Some(ActionIntercept::Ambiguous);
    }

    let mut parts = line.splitn(2, char::is_whitespace);
    let head = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or("").trim();
    if matches!(head.to_ascii_lowercase().as_str(), "run" | "/run" | "action") && !rest.is_empty() {
        let by_prefix: Vec<_> = shortcuts
            .iter()
            .filter(|a| a.name.trim().eq_ignore_ascii_case(rest))
            .collect();
        if by_prefix.len() == 1 {
            return Some(ActionIntercept::Run(by_prefix[0].clone(), false));
        }
        if by_prefix.len() > 1 {
            return Some(ActionIntercept::Ambiguous);
        }
    }

    None
}

/// Intercept persist: them-line always lands; you-line only when the line
/// read as chat. Same duplicate-you skip + cap 40 as the normal send path.
fn persist_action_intercept(app: &tauri::AppHandle, you: Option<&str>, them: &str) {
    if let Ok(mut st) = crate::state::load_state(app) {
        if let Some(you) = you {
            st.companion.last_user_line = you.to_string();
            let you_already_saved = st
                .companion
                .turns
                .last()
                .is_some_and(|t| t.role == "you" && t.text == you);
            if !you_already_saved {
                st.companion.turns.push(crate::state::CompanionTurn {
                    role: "you".into(),
                    text: you.to_string(),
                });
            }
        }
        st.companion.last_bot_line = them.to_string();
        st.companion.turns.push(crate::state::CompanionTurn {
            role: "them".into(),
            text: them.to_string(),
        });
        const CAP: usize = 40;
        if st.companion.turns.len() > CAP {
            let drop = st.companion.turns.len() - CAP;
            st.companion.turns.drain(0..drop);
        }
        let _ = crate::state::save_state(app, &st);
        let _ = app.emit("state:changed", ());
    }
}

// ---------- GO-MH-2-APP: leftover map inject (read-only, fail open) ----------

/// Tiny structs with serde defaults — a hand-edited map with extra or
/// missing keys still parses. Schema: leftover_item.schema.json.
#[derive(serde::Deserialize)]
struct LeftoverMap {
    #[serde(default)]
    items: Vec<LeftoverItem>,
}

#[derive(serde::Deserialize)]
struct LeftoverItem {
    #[serde(default)]
    text: String,
    #[serde(default)]
    tag: String,
}

/// Prepend the leftover pocket to the outbound prompt (every mouth:
/// goose, grok, hermes, berd).
/// Fail OPEN: missing file / bad JSON / no usable items → the user line
/// unchanged; a ping is never blocked. The wrap is what the model sees
/// only — transcript, captions, and the event log keep the raw line.
fn wrap_with_pocket(line: &str) -> String {
    const PREPEND_CAP: usize = 2048; // chars, whole prepend block
    const MAX_ITEMS: usize = 12;
    const ITEM_TEXT_CAP: usize = 240; // schema maxLength

    let raw = match std::fs::read_to_string(companions_root().join("leftovers.json")) {
        Ok(s) => s,
        Err(_) => return line.to_string(),
    };
    let map: LeftoverMap = match serde_json::from_str(&raw) {
        Ok(m) => m,
        Err(_) => return line.to_string(),
    };

    let mut items: Vec<&LeftoverItem> = map
        .items
        .iter()
        .filter(|it| {
            let text = it.text.trim();
            !text.is_empty() && !text.eq_ignore_ascii_case("desk test")
        })
        .filter(|it| !it.tag.trim().eq_ignore_ascii_case("let-go"))
        .collect();
    // keep before open; stable within a tag (file order preserved).
    items.sort_by_key(|it| {
        if it.tag.trim().eq_ignore_ascii_case("keep") {
            0
        } else {
            1
        }
    });
    items.truncate(MAX_ITEMS);
    if items.is_empty() {
        return line.to_string();
    }

    // Budget the whole prepend ("[pocket]\n- …\n\nUSER:\n") ≤ 2048 chars.
    // Items give way first (dropped from the end); the USER line never
    // shrinks.
    let header = "[pocket]\n";
    let footer = "\n\nUSER:\n";
    let mut budget = PREPEND_CAP.saturating_sub(header.chars().count() + footer.chars().count());
    let mut lines: Vec<String> = Vec::new();
    for it in items {
        let mut text = it.text.trim().replace('\n', " ");
        if text.chars().count() > ITEM_TEXT_CAP {
            text = text.chars().take(ITEM_TEXT_CAP - 1).collect::<String>() + "…";
        }
        let item_line = format!("- {text}\n");
        let n = item_line.chars().count();
        if n > budget {
            break;
        }
        budget -= n;
        lines.push(item_line);
    }
    if lines.is_empty() {
        return line.to_string();
    }
    format!("{header}{}{footer}{line}", lines.concat())
}

/// GO-COMPANION-BERD / GO-ORBITTY-PLUG-APP — companion composer send path.
/// Default: Goose mouth (`goose run`, plug.json session). `grok` mouth;
/// `hermes` compat; `berd` hidden escape (bound session from
/// `~/.agents/companions/orbitty-primary/session.json` via `berdctl`).
/// Never hosts a model inside Orbitty. Async; UI disables Send in flight.
/// GO-ORBITTY-EVENT-LOG: appends `companion-send` / `companion-reply`
/// (best-effort) — content is `{ "n": <chars> }` only, never the line.
#[tauri::command]
pub async fn companion_send(app: tauri::AppHandle, line: String) -> Result<String, String> {
    let line = line.trim().replace('\n', " ");
    if line.is_empty() {
        return Err("Type a line first.".into());
    }
    crate::event_log::append_best(
        &app,
        "companion-send",
        None,
        None,
        Some("user"),
        true,
        &serde_json::json!({ "n": line.chars().count() }),
    );
    let out = companion_send_inner(app.clone(), line).await;
    crate::event_log::append_best(
        &app,
        "companion-reply",
        None,
        None,
        Some("agent"),
        true,
        &serde_json::json!({ "n": out.as_ref().map(|r| r.chars().count()).unwrap_or(0) }),
    );
    out
}

async fn companion_send_inner(app: tauri::AppHandle, line: String) -> Result<String, String> {

    // GO-ORBITTY-ASK-ACTION: a line naming a Launchpad Action runs it
    // locally — Hermes/Berd never sees it. No match → unchanged send.
    if let Ok(st) = crate::state::load_state(&app) {
        match match_action_line(&st.action_shortcuts, &line) {
            Some(ActionIntercept::Run(shortcut, keep_you)) => {
                let command = shortcut.command.clone();
                let cwd = shortcut.cwd.clone();
                let action_app = app.clone();
                let result =
                    tauri::async_runtime::spawn_blocking(move || run_action(action_app, command, cwd))
                    .await
                    .map_err(|e| format!("run task: {e}"))
                    .and_then(|r| r);
                // Same head as the rail's runAction; timeout/spawn errors
                // land as the them-line string, never an Err to the client.
                let them = match result {
                    Ok(r) => {
                        let out = [r.stdout, r.stderr]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join("\n");
                        let head = if r.ok {
                            format!("{}: done", shortcut.name)
                        } else {
                            format!(
                                "{}: exit {}",
                                shortcut.name,
                                r.code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
                            )
                        };
                        if out.is_empty() { head } else { format!("{head}\n{out}") }
                    }
                    Err(e) => e,
                };
                persist_action_intercept(&app, keep_you.then_some(line.as_str()), &them);
                return Ok(them);
            }
            Some(ActionIntercept::Ambiguous) => {
                let them = "which action? use the Launchpad well.".to_string();
                persist_action_intercept(&app, None, &them);
                return Ok(them);
            }
            None => {}
        }
    }

    let backend = companion_backend();
    // GO-MH-2-APP: pocket inject AFTER action-intercept (Actions above ran
    // on the raw line). Every mouth gets the wrapped prompt; persist +
    // event log below keep the raw `line`.
    let prompt = wrap_with_pocket(&line);
    // GO-ORBITTY-PLUG-APP plug map — unknown backend is an honest error,
    // never a silent Berd fallback.
    let reply = match backend.as_str() {
        "goose" => companion_send_goose(&app, &prompt).await?,
        "grok" => companion_send_grok(&app, &prompt).await?,
        "hermes" => companion_send_hermes(&app, &prompt).await?,
        "berd" => companion_send_berd(&prompt).await?,
        other => {
            return Err(format!(
                "Unknown ORBITTY_COMPANION_BACKEND {other:?} — goose, grok, hermes, or berd."
            ))
        }
    };

    // Both turns persist only once the reply actually landed — a failed
    // send keeps the old captions. S2.5: an optimistic client already has
    // this `you` line on screen; if the saved transcript's last turn IS
    // this same line, persist only the reply so the you turn never
    // doubles. Old clients (no optimistic append) still get both turns.
    if let Ok(mut st) = crate::state::load_state(&app) {
        st.companion.last_user_line = line.clone();
        st.companion.last_bot_line = reply.clone();
        let you_already_saved = st
            .companion
            .turns
            .last()
            .is_some_and(|t| t.role == "you" && t.text == line);
        if !you_already_saved {
            st.companion.turns.push(crate::state::CompanionTurn {
                role: "you".into(),
                text: line,
            });
        }
        st.companion.turns.push(crate::state::CompanionTurn {
            role: "them".into(),
            text: reply.clone(),
        });
        const CAP: usize = 40;
        if st.companion.turns.len() > CAP {
            let drop = st.companion.turns.len() - CAP;
            st.companion.turns.drain(0..drop);
        }
        let _ = crate::state::save_state(&app, &st);
        let _ = app.emit("state:changed", ());
    }
    Ok(reply)
}

async fn companion_send_berd(line: &str) -> Result<String, String> {
    let bin = find_berdctl().ok_or(
        "`berdctl` not found — install Berd.app or put berdctl on PATH.",
    )?;
    let sid = load_companion_session_id()?;
    let line = line.to_string();
    tauri::async_runtime::spawn_blocking(move || run_berd_reply(&bin, &sid, &line))
        .await
        .map_err(|e| format!("send task: {e}"))?
}

/// GO-ORBITTY-PLUG-APP: default mouth. `goose run -q --no-profile` with
/// the soul as --system. First send names the session `orbitty-companion`,
/// then the real `YYYYMMDD_N` id is captured from `goose session list`
/// and persisted in plug.json; later sends resume by
/// `--session-id ID --resume` (`--name` is not a unique resume key).
/// Honest error when goose is missing — no silent Berd fallback.
async fn companion_send_goose(app: &tauri::AppHandle, line: &str) -> Result<String, String> {
    let bin = find_on_path("goose").ok_or(
        "`goose` not found — install the Goose CLI or put it on PATH (Orbitty.app checks PATH + ~/.local/bin).",
    )?;
    let soul = companion_soul_text(app);
    let plug = load_plug();
    let line = line.to_string();
    tauri::async_runtime::spawn_blocking(move || run_goose_reply(&bin, &soul, &plug, &line))
        .await
        .map_err(|e| format!("send task: {e}"))?
}

/// Product goose session name (never the probe's `orbitty-plug-goose`).
const GOOSE_SESSION_NAME: &str = "orbitty-companion";

fn run_goose_reply(bin: &Path, soul: &str, plug: &PlugState, line: &str) -> Result<String, String> {
    let mut args: Vec<String> = vec!["run".into(), "-q".into(), "--no-profile".into()];
    let resume_id = plug
        .goose_session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| is_goose_session_id(s));
    let first_send = resume_id.is_none();
    match resume_id {
        Some(id) => {
            args.push("--session-id".into());
            args.push(id.to_string());
            args.push("--resume".into());
        }
        None => {
            args.push("--name".into());
            args.push(GOOSE_SESSION_NAME.into());
        }
    }
    args.push("--system".into());
    args.push(soul.to_string());
    args.push("-t".into());
    args.push(line.to_string());

    let reply = run_mouth_cli("Goose", bin, &args, 180)?;
    if reply.is_empty() {
        return Err("Goose sent back an empty reply.".into());
    }
    if first_send {
        capture_goose_session(bin);
    }
    Ok(reply)
}

/// After the first `--name` send, learn the real session id from
/// `goose session list` (newest-first, first match wins) and persist it.
/// Best-effort: a miss just means the next send is `--name` again.
fn capture_goose_session(bin: &Path) {
    let out = Command::new(bin)
        .args(["session", "list"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(out) = out else { return };
    if !out.status.success() {
        return;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if !line.contains(GOOSE_SESSION_NAME) {
            continue;
        }
        let id = line.split_whitespace().next().unwrap_or("");
        if is_goose_session_id(id) {
            let mut plug = load_plug();
            plug.goose_session_id = Some(id.to_string());
            save_plug(&plug);
            return;
        }
    }
}

/// GO-ORBITTY-PLUG-APP: grok mouth. `grok -p <prompt>` with the soul as
/// --rules, JSON out. Session UUID lives in plug.json: first send
/// `-s UUID`, later `--resume UUID` (re-passing `-s` on a live id fails —
/// a failed create is retried once as a resume to self-heal that race).
/// Honest error when grok is missing — no silent Berd fallback.
async fn companion_send_grok(app: &tauri::AppHandle, line: &str) -> Result<String, String> {
    let bin = find_grok().ok_or(
        "`grok` not found — install the Grok CLI or put it on PATH (also checks ~/.grok/bin/grok).",
    )?;
    let soul = companion_soul_text(app);
    let plug = load_plug();
    let line = line.to_string();
    tauri::async_runtime::spawn_blocking(move || run_grok_reply(&bin, &soul, plug, &line))
        .await
        .map_err(|e| format!("send task: {e}"))?
}

fn grok_args(sid: &str, minted: bool, soul: &str, line: &str) -> Vec<String> {
    let mut args: Vec<String> = vec!["-p".into(), line.to_string()];
    if minted {
        args.push("--resume".into());
    } else {
        args.push("-s".into());
    }
    args.push(sid.to_string());
    args.extend([
        "--output-format".into(),
        "json".into(),
        "--always-approve".into(),
        "--no-subagents".into(),
        "--disable-web-search".into(),
        "--rules".into(),
        soul.to_string(),
    ]);
    args
}

fn run_grok_reply(
    bin: &Path,
    soul: &str,
    mut plug: PlugState,
    line: &str,
) -> Result<String, String> {
    // Mint a fresh product UUID — never the probe's, never Berd's.
    let has_id = plug
        .grok_session_id
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| is_uuid(s));
    if !has_id {
        plug.grok_session_id = Some(mint_uuid()?);
        plug.grok_session_minted = false;
        save_plug(&plug);
    }
    let sid = plug.grok_session_id.clone().unwrap_or_default();

    let args = grok_args(&sid, plug.grok_session_minted, soul, line);
    let raw = match run_mouth_cli("Grok", bin, &args, 180) {
        Ok(r) => r,
        Err(e) if !plug.grok_session_minted && e.to_ascii_lowercase().contains("session") => {
            // The `-s` create may have landed server-side despite the
            // error — heal: mark minted and retry once as a resume.
            plug.grok_session_minted = true;
            save_plug(&plug);
            let retry = grok_args(&sid, true, soul, line);
            run_mouth_cli("Grok", bin, &retry, 180)?
        }
        Err(e) => return Err(e),
    };

    // --output-format json: the assistant text is the top-level "text"
    // field. Never dump the whole JSON blob into the bubble.
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("Grok: bad JSON reply ({e})"))?;
    let text = v
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if text.is_empty() {
        return Err("Grok sent back an empty reply.".into());
    }
    if !plug.grok_session_minted {
        plug.grok_session_minted = true;
        save_plug(&plug);
    }
    Ok(text)
}

async fn companion_send_hermes(app: &tauri::AppHandle, line: &str) -> Result<String, String> {
    let bin = find_on_path("hermes")
        .ok_or("`hermes` not found — install Hermes or put it on PATH.")?;

    let dir = app_data(app)?.join("context");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let query = dir.join("companion-query.txt");
    std::fs::write(&query, format!("{line}\n"))
        .map_err(|e| format!("write {}: {e}", query.display()))?;

    let dirs = hermes_profile_dirs();
    let persisted = crate::state::load_state(app)
        .map(|s| s.companion.agent_profile)
        .unwrap_or_default();
    let agent = resolve_agent(&persisted, &dirs);

    let mut args: Vec<String> = Vec::new();
    if agent != "default" {
        args.push("-p".into());
        args.push(agent);
    }
    args.extend([
        "chat".into(),
        "--query-file".into(),
        query.display().to_string(),
        "--continue".into(),
        "orbitty-companion".into(),
        "--create-if-missing".into(),
        // GO-ORBITTY-PLUG-APP: hermes stays the compat mouth — `--safe-mode`
        // (probe 05 green on this bin). A bin that rejects the flag gets
        // one retry without it, then the honest error.
        "--safe-mode".into(),
        "-Q".into(),
        "--source".into(),
        "orbitty".into(),
    ]);

    tauri::async_runtime::spawn_blocking(move || {
        match run_hermes_reply(&bin, &args) {
            Err(e) if hermes_rejects_safe_mode(&e) => {
                let plain: Vec<String> =
                    args.iter().filter(|a| a.as_str() != "--safe-mode").cloned().collect();
                run_hermes_reply(&bin, &plain)
            }
            r => r,
        }
    })
    .await
    .map_err(|e| format!("send task: {e}"))?
}

/// True when the failure reads like the bin not knowing `--safe-mode`.
fn hermes_rejects_safe_mode(e: &str) -> bool {
    let l = e.to_ascii_lowercase();
    l.contains("safe-mode")
        || l.contains("unexpected argument")
        || l.contains("unrecognized")
        || l.contains("unknown option")
}

/// berdctl send (fire-and-forget) then poll session get until a new assistant
/// message appears or timeout. Berd must be running.
fn run_berd_reply(bin: &Path, session_id: &str, line: &str) -> Result<String, String> {
    let baseline = berd_session_snapshot(bin, session_id)?;
    let before_text = baseline.last_assistant.clone();
    let before_count = baseline.message_count;

    let send = berdctl_cmd(bin)?
        .args([
            "session",
            "send",
            "--json",
            "--session-id",
            session_id,
            "--prompt",
            line,
            "--if-running",
            "queue",
            "--timeout-ms",
            "120000",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("launch berdctl send: {e}"))?;

    if !send.status.success() {
        return Err(tool_error_line(
            "Berd",
            &send.stderr,
            &send.stdout,
            send.status.code(),
        ));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    let mut saw_running = false;
    loop {
        if std::time::Instant::now() >= deadline {
            return Err("Berd took longer than 5 minutes — try again.".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(400));

        let snap = match berd_session_snapshot(bin, session_id) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if snap.is_running {
            saw_running = true;
            continue;
        }

        let reply = snap.last_assistant.trim().to_string();
        if reply.is_empty() {
            continue;
        }

        let count_grew = snap.message_count > before_count;
        let text_changed = reply != before_text;

        // Done when idle after work and we have a reply that is new by text
        // or by message count (covers identical short replies).
        if saw_running && (text_changed || count_grew) {
            return Ok(reply);
        }
        if !saw_running && (text_changed || count_grew) {
            // Fast path: turn finished between polls without observing running.
            return Ok(reply);
        }
    }
}

struct BerdSnap {
    is_running: bool,
    last_assistant: String,
    message_count: i64,
}

fn berd_session_snapshot(bin: &Path, session_id: &str) -> Result<BerdSnap, String> {
    let out = berdctl_cmd(bin)?
        .args([
            "session",
            "get",
            "--json",
            "--session-id",
            session_id,
            "--messages",
            "8",
            "--timeout-ms",
            "60000",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("berdctl get: {e}"))?;
    if !out.status.success() {
        return Err(tool_error_line(
            "Berd",
            &out.stderr,
            &out.stdout,
            out.status.code(),
        ));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("berdctl get JSON: {e}"))?;
    let is_running = v
        .get("is_running")
        .and_then(|x| x.as_bool())
        .unwrap_or(false)
        || v.get("chat_state").and_then(|x| x.as_str()) == Some("streaming");
    let message_count = v.get("message_count").and_then(|x| x.as_i64()).unwrap_or(0);
    let last_assistant = v
        .get("messages")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .rev()
                .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("assistant"))
                .and_then(|m| m.get("text").and_then(|t| t.as_str()))
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_default();
    Ok(BerdSnap {
        is_running,
        last_assistant,
        message_count,
    })
}

/// Blocking hermes run with a 5-minute guard so a hung run can't wedge the
/// composer forever. stdout under -Q is the reply; stderr is session noise
/// (resumed/session_id lines) plus any error — surfaced as ONE line.
fn run_hermes_reply(bin: &Path, args: &[String]) -> Result<String, String> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("launch hermes: {e}"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let out = child
                    .wait_with_output()
                    .map_err(|e| format!("collect hermes output: {e}"))?;
                if !out.status.success() {
                    return Err(tool_error_line(
                        "Hermes",
                        &out.stderr,
                        &out.stdout,
                        out.status.code(),
                    ));
                }
                let reply = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if reply.is_empty() {
                    return Err("Hermes sent back an empty reply.".into());
                }
                return Ok(reply);
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Hermes took longer than 5 minutes — killed. Try again.".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(format!("wait on hermes: {e}")),
        }
    }
}

/// One honest line from a failed CLI run — never a path dump.
fn tool_error_line(tool: &str, stderr: &[u8], stdout: &[u8], code: Option<i32>) -> String {
    let text = String::from_utf8_lossy(stderr);
    let out = String::from_utf8_lossy(stdout);
    let line = text
        .lines()
        .chain(out.lines())
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("session_id:") && !l.starts_with('↻'));
    match line {
        Some(l) => {
            const MAX: usize = 240;
            if l.len() > MAX {
                format!("{tool}: {}…", &l[..MAX])
            } else {
                format!("{tool}: {l}")
            }
        }
        None => format!(
            "{tool} exited with code {}.",
            code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
        ),
    }
}

fn soul_file(app: &tauri::AppHandle, project_id: &str) -> Result<PathBuf, String> {
    let dir = app_data(app)?.join("souls").join(safe_id(project_id));
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir.join("soul.md"))
}

/// Template per the S5 lock: title + "What this planet is" + Journal.
fn soul_template(project_name: &str) -> String {
    format!("# {project_name}\n\n## What this planet is\n\n\n\n## Journal\n")
}

/// Create the template only when the file is missing; never clobber.
fn ensure_soul(
    app: &tauri::AppHandle,
    project_id: &str,
    project_name: Option<&str>,
) -> Result<PathBuf, String> {
    let path = soul_file(app, project_id)?;
    if !path.exists() {
        let body = soul_template(project_name.unwrap_or("Planet"));
        std::fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    Ok(path)
}

#[tauri::command]
pub fn load_soul(
    app: tauri::AppHandle,
    project_id: String,
    project_name: Option<String>,
) -> Result<String, String> {
    let path = ensure_soul(&app, &project_id, project_name.as_deref())?;
    std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))
}

/// The only programmatic writer of soul.md — gated by the explicit Save
/// button in the UI. Atomic write (tmp + rename) like state.json.
#[tauri::command]
pub fn save_soul(app: tauri::AppHandle, project_id: String, body: String) -> Result<(), String> {
    let path = soul_file(&app, &project_id)?;
    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, body).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("rename to {}: {e}", path.display()))?;
    Ok(())
}

#[tauri::command]
pub fn soul_path(app: tauri::AppHandle, project_id: String) -> Result<String, String> {
    Ok(soul_file(&app, &project_id)?.display().to_string())
}

/// `open` the soul file in the user's default editor (template first so the
/// editor never opens a missing file).
#[tauri::command]
pub fn open_soul_in_editor(
    app: tauri::AppHandle,
    project_id: String,
    project_name: Option<String>,
) -> Result<(), String> {
    let path = ensure_soul(&app, &project_id, project_name.as_deref())?;
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("open {}: {e}", path.display()))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(format!(
            "Open in editor is wired for macOS only; file: {}",
            path.display()
        ))
    }
}

// ---------- GO-ORBITTY-SLEEPERS: wake a closed claimed window ----------

fn non_empty(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

/// Single-quote a value for safe embedding in a zsh .command script.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Write a tiny .command launcher and `open` it — a fresh Terminal window
/// running the line (same visible-new-shell trick as the cursor-agent
/// launch). Never the dead window's pty.
fn open_in_terminal(app: &tauri::AppHandle, key: &str, line: &str) -> Result<(), String> {
    let dir = app_data(app)?.join("context");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let script = dir.join(format!("wake-{}.command", safe_id(key)));
    std::fs::write(&script, format!("#!/bin/zsh\n{line}\n"))
        .map_err(|e| format!("write {}: {e}", script.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
    }
    if spawn_detached("open", &[script.to_string_lossy().as_ref()]) {
        Ok(())
    } else {
        Err(format!("Couldn't open Terminal launcher {}", script.display()))
    }
}

/// Wake a sleeper (rail row click / sleeper double-click). First rung that
/// works wins; every miss falls through honestly. Read-only on state — the
/// rail owns claim writes. GO-ORBITTY-EVENT-LOG: appends `sleeper-wake`
/// (best-effort) with the outcome, never the wake trail details.
pub fn wake_sleeper_with(app: &tauri::AppHandle, id: &str) -> Result<String, String> {
    let out = wake_sleeper_inner(app, id);
    crate::event_log::append_best(
        app,
        "sleeper-wake",
        None,
        None,
        Some("user"),
        true,
        &serde_json::json!({ "windowId": id, "ok": out.is_ok() }),
    );
    out
}

fn wake_sleeper_inner(app: &tauri::AppHandle, id: &str) -> Result<String, String> {
    let st = crate::state::load_state(app)?;
    let claim = st
        .claims
        .iter()
        .find(|c| c.window_id == id)
        .cloned()
        .ok_or_else(|| "That sleeper isn't claimed anymore.".to_string())?;

    // 1. Live window (reopened between polls, not yet rematched) → focus.
    //    A restore that fails means it died mid-race — fall through.
    if let Ok(wins) = crate::windows::list_windows() {
        if wins.iter().any(|w| w.id == id) && crate::windows::restore_window(id).is_ok() {
            return Ok("Woke the live window.".into());
        }
    }

    // 2. File sleeper: the stamped AX document path, still on disk.
    if let Some(path) = non_empty(claim.file_path.as_deref()) {
        if std::path::Path::new(path).exists() && spawn_detached("open", &[path]) {
            return Ok(format!("Opened {path}."));
        }
    }

    // 3. Hermes session sleeper: resume the stamped thread in a fresh
    //    Terminal. Never --create-if-missing — the title was only a hint.
    if let Some(session) = non_empty(claim.hermes_session.as_deref()) {
        if let Some(bin) = find_on_path("hermes") {
            let mut cmd = sh_quote(&bin.display().to_string());
            if let Some(profile) = non_empty(claim.hermes_profile.as_deref()) {
                cmd.push_str(&format!(" -p {}", sh_quote(profile)));
            }
            cmd.push_str(&format!(" chat --continue {}", sh_quote(session)));
            open_in_terminal(app, &claim.window_id, &format!("exec {cmd}"))?;
            return Ok(format!("Resuming Hermes session \"{session}\" in Terminal."));
        }
    }

    // 4. Recipe sleeper: the linked Action, else a stamped command+cwd.
    if let Some(aid) = non_empty(claim.action_id.as_deref()) {
        if let Some(a) = st.action_shortcuts.iter().find(|a| a.id == aid) {
            let dir = action_dir(a.cwd.as_deref());
            let line = format!(
                "cd {} && exec /bin/zsh -lc {}",
                sh_quote(&dir.display().to_string()),
                sh_quote(&a.command)
            );
            open_in_terminal(app, &claim.window_id, &line)?;
            return Ok(format!("Ran the \"{}\" recipe in Terminal.", a.name));
        }
    }
    if let Some(command) = non_empty(claim.command.as_deref()) {
        let dir = action_dir(claim.cwd.as_deref());
        let line = format!(
            "cd {} && exec /bin/zsh -lc {}",
            sh_quote(&dir.display().to_string()),
            sh_quote(command)
        );
        open_in_terminal(app, &claim.window_id, &line)?;
        return Ok("Ran the stamped recipe in Terminal.".into());
    }

    // 5. App fallback: back to the app, not the corpse.
    if let Some(bundle) = non_empty(claim.bundle_id.as_deref()) {
        if spawn_detached("open", &["-b", bundle]) {
            return Ok(format!("Reopened the app ({bundle})."));
        }
    }

    // 6. Honest miss — never fake a path.
    Err("No live window, file, session, recipe, or app to wake — that's all Orbitty knows about this one.".into())
}

/// Rail row click on a sleeper (and the habitat's double-click, via
/// wake_sleeper_with). Returns one honest line either way.
#[tauri::command]
pub fn wake_sleeper(app: tauri::AppHandle, id: String) -> Result<String, String> {
    wake_sleeper_with(&app, &id)
}

// ---------- Actions lane: user-authored shell shortcuts ----------

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub ok: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

fn clip_output(bytes: &[u8]) -> String {
    const MAX: usize = 4000;
    let s = String::from_utf8_lossy(bytes);
    let s = s.trim_end();
    if s.len() > MAX {
        format!("{}…\n(truncated)", &s[..MAX])
    } else {
        s.to_string()
    }
}

/// `~` and `$HOME` expand; a missing dir falls back to $HOME so a typo'd
/// cwd never kills the run.
fn action_dir(cwd: Option<&str>) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    let raw = cwd.map(str::trim).filter(|c| !c.is_empty());
    let Some(raw) = raw else {
        return PathBuf::from(home);
    };
    let expanded = if raw == "~" {
        home.clone()
    } else if let Some(rest) = raw.strip_prefix("~/") {
        format!("{home}/{rest}")
    } else {
        raw.replace("$HOME", &home)
    };
    let path = PathBuf::from(expanded);
    if path.is_dir() {
        path
    } else {
        PathBuf::from(home)
    }
}

/// Run one user-authored shortcut: `/bin/zsh -lc <command>` in cwd or $HOME.
/// 120s guard so a hung command can't wedge the rail's await; output is
/// clipped and returned for the lane's note. GO-ORBITTY-EVENT-LOG: appends
/// `action-start` / `action-end` (best-effort) — lengths and outcome only,
/// never the command line.
#[tauri::command]
pub fn run_action(
    app: tauri::AppHandle,
    command: String,
    cwd: Option<String>,
) -> Result<ActionResult, String> {
    run_action_logged(&app, command, cwd)
}

/// GO-ORBITTY-ACTION-FILE: the logged run body, shared by the invoke
/// command, the control plane (`action.run`), and combo presses. Same
/// contract: `/bin/zsh -lc`, 120s, event log lengths only.
pub(crate) fn run_action_logged(
    app: &tauri::AppHandle,
    command: String,
    cwd: Option<String>,
) -> Result<ActionResult, String> {
    crate::event_log::append_best(
        app,
        "action-start",
        None,
        None,
        Some("action"),
        true,
        &serde_json::json!({ "n": command.chars().count() }),
    );
    let out = run_action_inner(command, cwd);
    let (ok, code) = match &out {
        Ok(r) => (r.ok, r.code),
        Err(_) => (false, None),
    };
    crate::event_log::append_best(
        app,
        "action-end",
        None,
        None,
        Some("action"),
        true,
        &serde_json::json!({ "ok": ok, "code": code }),
    );
    out
}

fn run_action_inner(command: String, cwd: Option<String>) -> Result<ActionResult, String> {
    let dir = action_dir(cwd.as_deref());
    let mut child = Command::new("/bin/zsh")
        .args(["-lc", &command])
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn zsh in {}: {e}", dir.display()))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let out = child
                    .wait_with_output()
                    .map_err(|e| format!("collect output: {e}"))?;
                let code = out.status.code();
                return Ok(ActionResult {
                    ok: out.status.success(),
                    code,
                    stdout: clip_output(&out.stdout),
                    stderr: clip_output(&out.stderr),
                });
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Timed out after 120s — process killed.".to_string());
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(format!("wait: {e}")),
        }
    }
}

// ---------- Agents lane: harness shortcuts (no LLM inside Orbitty) ----------

fn find_on_path(bin: &str) -> Option<PathBuf> {
    let path = std::env::var("PATH").unwrap_or_default();
    for dir in path.split(':') {
        let candidate = PathBuf::from(dir).join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    for dir in ["/usr/local/bin", "/opt/homebrew/bin"] {
        let candidate = PathBuf::from(dir).join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".local/bin").join(bin))
        .filter(|p| p.is_file())
}

/// Query file = concatenated readable .md paths (missing ones skipped) +
/// titles-only planet context when the shortcut is attached to a project.
/// Overwritten each launch, same honesty rules as the project context file.
fn write_agent_query(
    app: &tauri::AppHandle,
    shortcut: &crate::state::AgentShortcut,
    project_name: Option<&str>,
    titles: &[String],
) -> Result<(PathBuf, usize), String> {
    let dir = app_data(app)?.join("context");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let path = dir.join(format!("agent-{}.md", safe_id(&shortcut.id)));
    let mut body = format!("# {}\n\nOrbitty agent shortcut context.\n", shortcut.name);
    let mut skipped = 0usize;
    for raw in &shortcut.md_paths {
        let p = raw.trim();
        if p.is_empty() {
            continue;
        }
        let expanded = if p == "~" {
            std::env::var("HOME").unwrap_or_default()
        } else if let Some(rest) = p.strip_prefix("~/") {
            format!("{}/{rest}", std::env::var("HOME").unwrap_or_default())
        } else {
            p.to_string()
        };
        match std::fs::read_to_string(&expanded) {
            Ok(text) => {
                body.push_str(&format!("\n\n--- {expanded} ---\n\n{text}"));
            }
            Err(_) => skipped += 1,
        }
    }
    if shortcut.project_id.is_some() {
        let name = project_name.unwrap_or("Planet");
        body.push_str(&format!(
            "\n\n## Planet context — {name} (titles only, no file contents)\n"
        ));
        if titles.is_empty() {
            body.push_str("(no claimed windows or open to-dos)\n");
        } else {
            for t in titles {
                body.push_str(&format!("- {}\n", t.replace('\n', " ")));
            }
        }
    }
    std::fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok((path, skipped))
}

/// Launch a user-authored agent shortcut in its external harness. Orbitty
/// writes the query file and hands off — it never runs the model.
#[tauri::command]
pub fn launch_agent(
    app: tauri::AppHandle,
    shortcut: crate::state::AgentShortcut,
    project_name: Option<String>,
    titles: Vec<String>,
) -> Result<String, String> {
    let (query, skipped) = write_agent_query(&app, &shortcut, project_name.as_deref(), &titles)?;
    let skip_note = if skipped > 0 {
        format!(" Skipped {skipped} unreadable md path(s).")
    } else {
        String::new()
    };
    match shortcut.harness.as_str() {
        "cursor" => {
            let Some(bin) = find_on_path("cursor-agent") else {
                return Err(
                    "cursor-agent not on PATH — install the Cursor CLI or pick hermes."
                        .to_string(),
                );
            };
            // cursor-agent's chat is a TTY TUI: open it in Terminal via a
            // .command launcher with the query file as the initial prompt.
            let script = query.with_extension("command");
            let model_arg = shortcut
                .model
                .as_deref()
                .map(str::trim)
                .filter(|m| !m.is_empty())
                .map(|m| format!(" --model \"{}\"", m.replace('"', "")))
                .unwrap_or_default();
            let body = format!(
                "#!/bin/zsh\nprompt=\"$(cat \"{q}\")\"\nexec \"{bin}\"{model_arg} \"$prompt\"\n",
                q = query.display(),
                bin = bin.display(),
            );
            std::fs::write(&script, &body)
                .map_err(|e| format!("write {}: {e}", script.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
            }
            if spawn_detached("open", &[script.to_string_lossy().as_ref()]) {
                Ok(format!(
                    "Opened cursor-agent in Terminal. Query: {}.{skip_note}",
                    query.display()
                ))
            } else {
                Err(format!(
                    "Couldn't open Terminal launcher. Query file ready: {}",
                    query.display()
                ))
            }
        }
        // Default harness: hermes -p <profile> chat with the query file.
        _ => {
            let profile = {
                let p = shortcut.profile.trim();
                if p.is_empty() { "grok" } else { p }
            };
            let mut args: Vec<String> = vec![
                "-p".into(),
                profile.into(),
                "chat".into(),
                "--create-if-missing".into(),
                "-Q".into(),
                "--query-file".into(),
                query.display().to_string(),
            ];
            if let Some(m) = shortcut.model.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
                args.push("-m".into());
                args.push(m.to_string());
            }
            let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
            if spawn_detached("hermes", &arg_refs) {
                Ok(format!(
                    "Launched hermes -p {profile} chat. Query: {}.{skip_note}",
                    query.display()
                ))
            } else {
                Err(format!(
                    "`hermes` not on PATH. Query file ready: {}",
                    query.display()
                ))
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    // objc 0.2 macros expand to a `cfg(feature = "cargo-clippy")` probe that
    // newer rustc flags; harmless upstream noise (same as glass.rs).
    #![allow(unexpected_cfgs)]

    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    /// Pure focus of an already-running Hermes — no relaunch, no reopen
    /// event, no new window. NSRunningApplication is safe off the main
    /// thread (no view work here).
    pub fn focus_running_hermes() -> bool {
        unsafe {
            let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
            if workspace.is_null() {
                return false;
            }
            let apps: *mut Object = msg_send![workspace, runningApplications];
            if apps.is_null() {
                return false;
            }
            let count: usize = msg_send![apps, count];
            for i in 0..count {
                let app: *mut Object = msg_send![apps, objectAtIndex: i];
                if app.is_null() {
                    continue;
                }
                let name: *mut Object = msg_send![app, localizedName];
                if name.is_null() {
                    continue;
                }
                let utf8: *const std::os::raw::c_char = msg_send![name, UTF8String];
                if utf8.is_null() {
                    continue;
                }
                if std::ffi::CStr::from_ptr(utf8)
                    .to_string_lossy()
                    .eq_ignore_ascii_case("hermes")
                {
                    // NSApplicationActivateAllWindows | IgnoringOtherApps.
                    let ok: bool = msg_send![app, activateWithOptions: 3usize];
                    return ok;
                }
            }
            false
        }
    }
}
