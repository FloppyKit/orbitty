//! Coordinator + soul.md. Titles-only context. No LLM in this module.

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
pub(crate) fn safe_id(project_id: &str) -> String {
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

// Companion: plug map
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

/// pub(crate): mouth_hop keeps the BYOK OpenRouter key next to plug.json.
pub(crate) fn companions_root() -> PathBuf {
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
    /// At GROK_ROLL_EVERY we fork+handoff so resume stays thin; the
    /// bubble is unchanged (forever mouth).
    #[serde(default)]
    pub(crate) grok_sends: u32,
}

/// Backend session roll — not a new bubble. Pocket + last ~6 turns
/// ride the handoff. Keeps replies fast without dumping leftover map.
pub(crate) const GROK_ROLL_EVERY: u32 = 16;

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
/// stateless hop lane (Grok gets it via --rules).
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
fn strip_frontmatter(md: &str) -> String {
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
fn find_grok() -> Option<PathBuf> {
    if let Some(p) = find_on_path("grok") {
        return Some(p);
    }
    let home = std::env::var("HOME").ok()?;
    let p = PathBuf::from(home).join(".grok").join("bin").join("grok");
    p.is_file().then_some(p)
}

pub(crate) fn is_uuid(s: &str) -> bool {
    let lens = [8usize, 4, 4, 4, 12];
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip(lens)
            .all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// v4 UUID via macOS `uuidgen` (always present on the dogfood target).
pub(crate) fn mint_uuid() -> Result<String, String> {
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
pub(crate) fn run_mouth_cli(
    tool: &str,
    bin: &Path,
    args: &[String],
    envs: &[(&str, &str)],
    timeout_secs: u64,
) -> Result<String, String> {
    run_mouth_cli_watched(tool, bin, args, envs, timeout_secs, None)
}

/// run_mouth_cli + a stderr watch: a reader thread forwards stderr lines
/// as they arrive, and `kill_on` matching a line kills the child AT ONCE,
/// returning that line as the error. Grok prints a
/// dead-resume 404 (`session get failed: 404 Not Found`) in ~0.5s but
/// STAYS ALIVE — an unwatched poll waits out the whole timeout for a
/// corpse. The kill is line-matched, never time-based: a fat `--rules`
/// soul may slow startup, and a live slow generate keeps the full
/// timeout.
pub(crate) fn run_mouth_cli_watched(
    tool: &str,
    bin: &Path,
    args: &[String],
    envs: &[(&str, &str)],
    timeout_secs: u64,
    kill_on: Option<&dyn Fn(&str) -> bool>,
) -> Result<String, String> {
    use std::io::BufRead as _;
    let mut child = Command::new(bin)
        .args(args)
        .envs(envs.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("launch {}: {e}", bin.display()))?;
    // stderr → line channel. EOF (child dead, pipe closed) drops the
    // sender. The thread is never joined — a grandchild inheriting the
    // pipe must not hang the send path.
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let mut stderr_lines: Vec<String> = Vec::new();
    loop {
        // Drain arrived stderr lines; a kill-line ends it now.
        while let Ok(line) = rx.try_recv() {
            if kill_on.is_some_and(|k| k(&line)) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{tool}: {line}"));
            }
            stderr_lines.push(line);
        }
        match child.try_wait() {
            Ok(Some(_)) => {
                // stderr was taken for the watch, so wait_with_output
                // only collects stdout here.
                let out = child
                    .wait_with_output()
                    .map_err(|e| format!("collect {tool} output: {e}"))?;
                if !out.status.success() {
                    // The last stderr lines may still be in flight behind
                    // the exit — drain bounded (Disconnected = reader hit
                    // EOF; a line cap guards a grandchild holding the
                    // pipe), then the usual one honest error line.
                    loop {
                        match rx.recv_timeout(std::time::Duration::from_secs(2)) {
                            Ok(line) if stderr_lines.len() < 400 => stderr_lines.push(line),
                            Ok(_) | Err(_) => break,
                        }
                    }
                    let stderr = stderr_lines.join("\n").into_bytes();
                    return Err(tool_error_line(tool, &stderr, &out.stdout, out.status.code()));
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
        // tang/female are reserved 3D bodies
        // bundled at companion/3d — never pack folders, so the pack-art
        // check is skipped for them.
        let reserved_3d = pid == "tang" || pid == "female";
        if !reserved_3d && crate::packs::companion_pack_art(&app, pid).is_none() {
            return Err(format!("companion pack {pid:?} missing"));
        }
    }
    crate::state::update(&app, |st| {
        st.companion.image_pack_id = id.clone();
        Ok(())
    })?;
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
    crate::state::update(&app, |st| {
        st.companion.agent_profile = p.clone();
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(p)
}

/// Persist the active companion mode (spark |
/// chat | work | hold). Job law only — same transcript. Unknown mode =
/// honest error, fail closed — never a silent spark.
#[tauri::command]
pub fn set_companion_mode(app: tauri::AppHandle, mode: String) -> Result<String, String> {
    let m = crate::state::normalize_companion_mode(&mode).ok_or_else(|| {
        format!(
            "unknown companion mode {:?} — spark | chat | work | hold",
            mode.trim()
        )
    })?;
    crate::state::update(&app, |st| {
        st.companion.switch_mode(&m);
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(m)
}

/// Drop a them-line onto the live transcript — NO
/// LLM, no send path. `mode` is ignored (one companion thread). Returns
/// the active mode.
pub fn companion_note(
    app: tauri::AppHandle,
    line: String,
    mode: Option<String>,
) -> Result<String, String> {
    let line = line.trim().replace('\n', " ");
    if line.is_empty() {
        return Err("companion note needs a line.".into());
    }
    let target = crate::state::update(&app, |st| {
        // load_state healed the mode, so st.companion.mode is allowlist-clean.
        let target = match mode.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(raw) => crate::state::normalize_companion_mode(raw).ok_or_else(|| {
                format!("unknown companion mode {raw:?} — spark | chat | work | hold")
            })?,
            None => st.companion.mode.clone(),
        };
        // One transcript: notes always land on the live thread.
        let _ = target;
        st.companion.last_bot_line = line.clone();
        st.companion.push_turn("them", &line);
        Ok(target)
    })?;
    let _ = app.emit("state:changed", ());
    Ok(target)
}

/// Human "this chat is thin / new session needed". Snapshots the live
/// thread, cheap-scans it (vivo_thin.py), notes the bubble. Does not
/// wipe `turns` (forever mouth). Operator still starts the next job.
#[tauri::command]
pub fn companion_thin(app: tauri::AppHandle) -> Result<String, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/dude".into());
    let script = PathBuf::from(&home)
        .join(".buzz/RESEARCH/RGL_TESTBED/scripts/vivo_thin.py");
    let py = PathBuf::from("/opt/homebrew/bin/python3");
    let py = if py.exists() {
        py
    } else {
        PathBuf::from("python3")
    };
    if !script.exists() {
        let _ = crate::companion::companion_play(&app, "nod");
        return companion_note(
            app,
            "THIN stamped (scanner missing). Start a new job when ready.".into(),
            None,
        );
    }
    let out = Command::new(&py)
        .arg(&script)
        .arg("stamp")
        .output()
        .map_err(|e| format!("thin stamp: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .next()
        .unwrap_or("THIN stamped. Start a new job when ready.")
        .trim()
        .to_string();
    let _ = crate::companion::companion_play(&app, "nod");
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return companion_note(
            app,
            format!("THIN stamp failed. {err}").chars().take(200).collect(),
            None,
        );
    }
    let _ = line;
    if let Ok(st) = crate::state::load_state(&app) {
        let mut n = load_now();
        let hint = st.companion.last_user_line.trim();
        if !hint.is_empty() {
            n.last_error = clip_now(hint, 160);
        }
        save_now(&n);
    }
    Ok(stdout.trim().to_string())
}

/// Overwrite fields of now.json (current job packet). Empty string
/// clears that field. Missing args leave the field.
#[tauri::command]
pub fn companion_now(
    job: Option<String>,
    last_error: Option<String>,
    last_green: Option<String>,
    do_not_retry: Option<String>,
    files: Option<Vec<String>>,
) -> Result<String, String> {
    let mut n = load_now();
    if let Some(v) = job {
        n.job = v;
    }
    if let Some(v) = last_error {
        n.last_error = v;
    }
    if let Some(v) = last_green {
        n.last_green = v;
    }
    if let Some(v) = do_not_retry {
        n.do_not_retry = v;
    }
    if let Some(v) = files {
        n.files = v;
    }
    save_now(&n);
    Ok(serde_json::to_string_pretty(&n).unwrap_or_else(|_| "{}".into()))
}

/// The ctl-lit half of the dock LED. Any harness
/// opts in (`orbittyctl companion hack -- on [--who cursor]`); off
/// clears the who. Persisted on the companion slice (a flag, never a
/// transcript); state:changed re-paints the dock. Off when every source
/// is idle — a stuck on-light is a leftover, not a feature.
/// `project_id` is the live agent's
/// pinned planet (pickup passes it) so the dest/planet menus glow that
/// one planet; cleared on off like the who.
pub fn set_companion_hack(
    app: tauri::AppHandle,
    live: bool,
    who: Option<String>,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let (hack_live, hack_who, hack_project_id) = crate::state::update(&app, |st| {
        apply_hack(st, live, who, project_id);
        Ok((
            st.companion.hack_live,
            st.companion.hack_who.clone(),
            st.companion.hack_project_id.clone(),
        ))
    })?;
    let _ = app.emit("state:changed", ());
    Ok(serde_json::json!({
        "live": hack_live,
        "who": hack_who,
        "projectId": hack_project_id,
    }))
}

/// Hack fields only. Callers must already be inside `state::update` or
/// go through `set_companion_hack`, which is.
fn apply_hack(
    st: &mut crate::state::PersistedState,
    live: bool,
    who: Option<String>,
    project_id: Option<String>,
) {
    st.companion.hack_live = live;
    st.companion.hack_who = if live {
        who.map(|w| w.trim().to_string())
            .filter(|w| !w.is_empty())
            .unwrap_or_default()
    } else {
        String::new()
    };
    st.companion.hack_project_id = if live {
        project_id
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
    } else {
        None
    };
}

fn run_is_seated(r: &crate::state::AgentRun) -> bool {
    !r.agent_shortcut_id.trim().is_empty()
        || r.herdr
            .as_ref()
            .is_some_and(|h| !h.agent_name.trim().is_empty())
}

fn seated_live<'a>(st: &'a crate::state::PersistedState) -> Option<&'a crate::state::AgentRun> {
    st.agent_runs.iter().find(|r| {
        run_is_seated(r) && matches!(r.state.as_str(), "working" | "starting" | "blocked")
    })
}

/// Who lights the dock: the shortcut's harness, else the herdr kind.
fn who_from_run(st: &crate::state::PersistedState, r: &crate::state::AgentRun) -> Option<String> {
    if !r.agent_shortcut_id.trim().is_empty() {
        if let Some(s) = st
            .agent_shortcuts
            .iter()
            .find(|s| s.id == r.agent_shortcut_id)
        {
            let h = s.harness.trim();
            if !h.is_empty() {
                return Some(h.to_string());
            }
        }
    }
    r.herdr.as_ref().and_then(|h| {
        let k = h.kind.trim();
        if k.is_empty() {
            None
        } else {
            Some(k.to_string())
        }
    })
}

/// Pickup lights the dock before the harness is up; a dead process
/// that never ran its off-hook leaves hackLive stuck. Both ways: seated
/// working|starting|blocked and dark LED → on (who from shortcut
/// harness or herdr kind); no seated run and lit LED → off.
pub fn heal_stuck_hack(app: &tauri::AppHandle) {
    let _ = crate::state::update(app, |st| {
        let seated = seated_live(st).cloned();
        match (seated.as_ref(), st.companion.hack_live) {
            (Some(r), false) => {
                let who = who_from_run(st, r);
                let project_id = if r.planet_id.trim().is_empty() {
                    None
                } else {
                    Some(r.planet_id.clone())
                };
                apply_hack(st, true, who, project_id);
            }
            (None, true) => apply_hack(st, false, None, None),
            _ => {}
        }
        Ok(())
    });
}

/// Persist Talk source (`system` | `hotkey`).
#[tauri::command]
pub fn set_companion_talk_source(app: tauri::AppHandle, source: String) -> Result<String, String> {
    let src = crate::state::normalize_talk_source(&source);
    crate::state::update(&app, |st| {
        st.companion.talk_source = src.clone();
        Ok(())
    })?;
    crate::talk::stop_if_any();
    let _ = app.emit("state:changed", ());
    Ok(src)
}

/// Persist expand/collapse so the window reopens the way it was left.
#[tauri::command]
pub fn set_companion_expanded(app: tauri::AppHandle, expanded: bool) -> Result<(), String> {
    crate::state::update(&app, |st| {
        st.companion.expanded = expanded;
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    // Expanding while hugged restores the tall
    // idle FIRST — the transcript is never a 120px slab.
    if expanded {
        crate::panes::unhug_companion(&app);
        // Nor a composer-grown frame — shed the
        // sprite-on grow extra with the hug.
        crate::panes::ungrow_companion_composer(&app);
    }
    // Expand forces the slab (mouse on) NOW,
    // before the JS re-render; collapse reapplies the ghost from kept hits.
    crate::companion_clear::expanded_changed(&app, expanded);
    Ok(())
}

/// Persist the Full/Head view for ONE
/// surface (`surface` = "collapsed" | "expanded", `view` = "full" |
/// "head"). Two prefs, never one global bool — the other surface's key
/// is untouched. Returns the value actually written.
#[tauri::command]
pub fn set_companion_view(
    app: tauri::AppHandle,
    surface: String,
    view: String,
) -> Result<String, String> {
    let written = crate::state::update(&app, |st| {
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
            "all" => {
                let v = crate::state::normalize_companion_view(&view, "full");
                st.companion.view_collapsed = v.clone();
                st.companion.view_expanded = v.clone();
                v
            }
            other => return Err(format!("bad companion surface: {other:?}")),
        };
        Ok(written)
    })?;
    let _ = app.emit("state:changed", ());
    Ok(written)
}

/// Persist the one sprite flag (default on; a
/// missing key loads true). Same shape as set_companion_speak. R-click on
/// the Full/Head button is the only writer; the per-surface Full/Head
/// prefs are never touched here — they survive Off.
/// Idle collapsed + sprite off hugs the window to
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
    let expanded = crate::state::update(&app, |st| {
        let changed = st.companion.sprite != enabled;
        if changed {
            st.companion.sprite = enabled;
        }
        Ok((changed, st.companion.expanded))
    })?;
    if expanded.0 {
        let _ = app.emit("state:changed", ());
    }
    if enabled {
        crate::panes::unhug_companion(&app);
    } else if !expanded.1 {
        // Shed any sprite-on composer grow FIRST
        // — the hug stash must be the true tall idle, never grown.
        crate::panes::ungrow_companion_composer(&app);
        crate::panes::hug_companion(&app, hug_height);
    }
    Ok(())
}

// S2.7 speak replies: macOS `say`, default system voice

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
    crate::state::update(&app, |st| {
        st.companion.speak_replies = enabled;
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Companion `say` volume 0.0–1.0. Volume 0 mutes the next speak.
#[tauri::command]
pub fn set_companion_speak_volume(app: tauri::AppHandle, volume: f32) -> Result<(), String> {
    let volume = volume.clamp(0.0, 1.0);
    crate::state::update(&app, |st| {
        st.companion.speak_volume = volume;
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Draw / hide the CC + transcript. Composer stays.
/// Captions off while expanded also collapses — empty big box is never
/// a destination (one save, one state:changed, no JS race).
#[tauri::command]
pub fn set_companion_captions(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let collapsed = crate::state::update(&app, |st| {
        st.companion.captions = enabled;
        let collapsed = !enabled && st.companion.expanded;
        if collapsed {
            st.companion.expanded = false;
        }
        Ok(collapsed)
    })?;
    let _ = app.emit("state:changed", ());
    if collapsed {
        crate::companion_clear::expanded_changed(&app, false);
    }
    Ok(())
}

// STARS prompts opt-in + strip ack
// The companion window never writes state.json wholesale; these two
// load-modify-save commands are its only STARS writes. The rail's STARS
// pane writes checkins through its own full persist.

/// Persist the Prompts opt-in (default off). Mirrored by the rail's STARS
/// pane toggle and the companion's Prompts toggle — same key either way.
#[tauri::command]
pub fn set_stars_prompts(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    crate::state::update(&app, |st| {
        st.stars.prompts_opt_in = enabled;
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Dismiss the companion's prompt strip: `ack` is `"{day}:{sig}"` — the
/// strip stays hidden until the day rolls or a fresh check-in changes the
/// sig. One show per day, no loop.
#[tauri::command]
pub fn ack_stars_prompt(app: tauri::AppHandle, ack: String) -> Result<(), String> {
    crate::state::update(&app, |st| {
        st.stars.prompts_ack = ack;
        Ok(())
    })?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Read one bot reply aloud with the DEFAULT system voice (`say "<text>"`
/// — no -v, no voice listing). Kills the previous `say` first, so a new
/// reply always wins. macOS only; one honest Err line, never a crash.
/// Volume rides `[[volm 0–1]]` so we don't touch system output volume.
#[tauri::command]
pub fn speak_reply(app: tauri::AppHandle, text: String) -> Result<(), String> {
    stop_say();
    let text = text.trim().to_string();
    if text.is_empty() {
        return Ok(());
    }
    let st = crate::state::load_state(&app).ok();
    if st.as_ref().is_some_and(|s| !s.companion.speak_replies) {
        return Ok(());
    }
    let vol = st
        .as_ref()
        .map(|s| s.companion.speak_volume.clamp(0.0, 1.0))
        .unwrap_or(1.0);
    if vol <= 0.0 {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let spoken = if (vol - 1.0).abs() < 0.01 {
            text
        } else {
            format!("[[volm {vol:.2}]] {text}")
        };
        let child = Command::new("say")
            .arg(&spoken)
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

// A typed line can name a Launchpad Action

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
/// push_turn lands on the one companion transcript
/// (and a shadow copy under the current mode for old readers).
/// Land the you-row + last_user_line NOW, before the 180s model wait.
/// Collapsed CC and expanded transcript both read persist; waiting until
/// the reply meant a hung send showed nothing and state:changed wiped
/// the optimistic bubble.
fn persist_companion_you(app: &tauri::AppHandle, line: &str) {
    if crate::state::update(app, |st| {
        st.companion.last_user_line = line.to_string();
        let you_already_saved = st
            .companion
            .turns
            .last()
            .is_some_and(|t| t.role == "you" && t.text == line);
        if !you_already_saved {
            st.companion.push_turn("you", line);
        }
        Ok(())
    })
    .is_ok()
    {
        let _ = app.emit("state:changed", ());
    }
}

fn persist_action_intercept(app: &tauri::AppHandle, you: Option<&str>, them: &str) {
    if crate::state::update(app, |st| {
        if let Some(you) = you {
            st.companion.last_user_line = you.to_string();
            let you_already_saved = st
                .companion
                .turns
                .last()
                .is_some_and(|t| t.role == "you" && t.text == you);
            if !you_already_saved {
                st.companion.push_turn("you", you);
            }
        }
        st.companion.last_bot_line = them.to_string();
        st.companion.push_turn("them", them);
        Ok(())
    })
    .is_ok()
    {
        let _ = app.emit("state:changed", ());
    }
}

// GO-MH-2-APP: leftover map inject (read-only, fail open)

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
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
struct NowPacket {
    #[serde(default)]
    job: String,
    #[serde(default)]
    last_error: String,
    #[serde(default)]
    last_green: String,
    #[serde(default)]
    do_not_retry: String,
    #[serde(default)]
    files: Vec<String>,
}

fn now_path() -> PathBuf {
    companions_root().join("now.json")
}

fn load_now() -> NowPacket {
    let Ok(raw) = std::fs::read_to_string(now_path()) else {
        return NowPacket::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_now(n: &NowPacket) {
    let Ok(body) = serde_json::to_string_pretty(n) else {
        return;
    };
    let _ = std::fs::write(now_path(), body + "\n");
}

fn clip_now(s: &str, n: usize) -> String {
    let t = s.trim().replace('\n', " ");
    if t.chars().count() <= n {
        t
    } else {
        t.chars().take(n.saturating_sub(1)).collect::<String>() + "…"
    }
}

/// Five-line current-job packet. Empty file = no block. Fail open.
fn now_block() -> String {
    let n = load_now();
    let mut lines: Vec<String> = Vec::new();
    if !n.job.trim().is_empty() {
        lines.push(format!("job: {}", clip_now(&n.job, 160)));
    }
    if !n.last_error.trim().is_empty() {
        lines.push(format!("last_error: {}", clip_now(&n.last_error, 160)));
    }
    if !n.last_green.trim().is_empty() {
        lines.push(format!("last_green: {}", clip_now(&n.last_green, 120)));
    }
    if !n.do_not_retry.trim().is_empty() {
        lines.push(format!("do_not_retry: {}", clip_now(&n.do_not_retry, 120)));
    }
    let files: Vec<_> = n
        .files
        .iter()
        .map(|f| f.trim())
        .filter(|f| !f.is_empty())
        .take(4)
        .collect();
    if !files.is_empty() {
        lines.push(format!("files: {}", files.join(", ")));
    }
    if lines.is_empty() {
        return String::new();
    }
    format!("[now]\n{}\n", lines.join("\n"))
}

fn mode_job_line(mode: &str) -> &'static str {
    match mode {
        "chat" => "join and finesse; stay in the bit; no vaults; THIN drops the bit",
        "work" => "build/answer; no alliance; no so-close; task-stuck → say so",
        "hold" => "parked; notes only",
        _ => "desk light; thin; route heavy work out",
    }
}

pub(crate) fn wrap_with_pocket(app: &tauri::AppHandle, line: &str) -> String {
    const PREPEND_CAP: usize = 900;
    const MAX_ITEMS: usize = 5;
    const ITEM_TEXT_CAP: usize = 120;

    let mode = crate::state::load_state(app)
        .ok()
        .map(|s| s.companion.mode.clone())
        .unwrap_or_else(|| "spark".into());
    let mode_line = format!(
        "[orbitty-mode: {mode}] {}\n{}",
        mode_job_line(&mode),
        now_block()
    );

    let raw = match std::fs::read_to_string(companions_root().join("leftovers.json")) {
        Ok(s) => s,
        Err(_) => return format!("{mode_line}\nUSER:\n{line}"),
    };
    let map: LeftoverMap = match serde_json::from_str(&raw) {
        Ok(m) => m,
        Err(_) => return format!("{mode_line}\nUSER:\n{line}"),
    };

    let mut items: Vec<&LeftoverItem> = map
        .items
        .iter()
        .filter(|it| {
            let text = it.text.trim();
            !text.is_empty() && !text.eq_ignore_ascii_case("desk test")
        })
        .filter(|it| {
            let t = it.tag.trim().to_ascii_lowercase();
            t == "open" || t == "keep"
        })
        .collect();
    // Open first (live), then keep (working set).
    items.sort_by_key(|it| {
        if it.tag.trim().eq_ignore_ascii_case("open") {
            0
        } else {
            1
        }
    });
    items.truncate(MAX_ITEMS);

    let header = format!("{mode_line}[pocket]\n");
    let footer = "\nUSER:\n";
    if items.is_empty() {
        return format!("{header}{footer}{line}");
    }

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
        return format!("{header}{footer}{line}");
    }
    format!("{header}{}{footer}{line}", lines.concat())
}

/// Companion composer send path.
/// Default: Goose mouth (`goose run`, plug.json session). `grok` mouth;
/// `hermes` compat; `berd` hidden escape (bound session from
/// `~/.agents/companions/orbitty-primary/session.json` via `berdctl`).
/// Never hosts a model inside Orbitty. Async; UI disables Send in flight.
/// Appends `companion-send` / `companion-reply`
/// (best-effort) — content is `{ "n": <chars> }` only, never the line.
/// Optional `path` — the one-file chip. Validated
/// BEFORE any intercept or persist so a bad path never fakes a send;
/// the original path (never a copy) rides the prompt to the mouth.
#[tauri::command]
pub async fn companion_send(
    app: tauri::AppHandle,
    line: String,
    path: Option<String>,
) -> Result<String, String> {
    let line = line.trim().replace('\n', " ");
    let attach = match path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => Some(crate::attach::stat_path(p)?),
        None => None,
    };
    if line.is_empty() && attach.is_none() {
        return Err("Type a line first.".into());
    }
    crate::event_log::append_best(
        &app,
        "companion-send",
        None,
        None,
        Some("user"),
        true,
        &serde_json::json!({ "n": line.chars().count(), "file": attach.is_some() }),
    );
    let out = companion_send_inner(app.clone(), line, attach).await;
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

async fn companion_send_inner(
    app: tauri::AppHandle,
    line: String,
    attach: Option<crate::attach::AttachInfo>,
) -> Result<String, String> {
    // A chip means the line is for the AI — the local
    // todo:/go:/Action intercepts cannot take a file, so they stand down
    // while one is attached (never a silent drop of the path).
    let no_intercepts = attach.is_some();

    // Local `todo:` mint — never pings the model. Before Actions so a
    // shortcut named "todo" cannot steal the intercept.
    #[cfg(unix)]
    if !no_intercepts {
        if let Some(out) = crate::companion_todo::intercept(&app, &line) {
            let them = match out {
                Ok(s) => s,
                Err(e) => e,
            };
            persist_action_intercept(&app, Some(line.as_str()), &them);
            return Ok(them);
        }
    }

    #[cfg(unix)]
    if !no_intercepts {
        if let Some(out) = crate::companion_go::intercept(&app, &line) {
            let them = match out {
                Ok(s) => s,
                Err(e) => e,
            };
            persist_action_intercept(&app, Some(line.as_str()), &them);
            return Ok(them);
        }
    }

    // A line naming a Launchpad Action runs it
    // locally — Hermes/Berd never sees it. No match → unchanged send.
    if !no_intercepts {
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
    }

    let backend = companion_backend();
    // A file-only send still needs a you-row — the
    // chip's name stands in for the empty line (never a blank bubble).
    let you_line = match &attach {
        Some(a) if line.is_empty() => format!("[file] {}", a.name),
        _ => line.clone(),
    };
    persist_companion_you(&app, &you_line);
    // Pin Quick mouth when LM Studio is serving the catalog E4B and
    // replyModel is still empty. One models GET, then the field sticks.
    crate::mouth_scan::pin_reply_if_empty(&app);
    // Existing verbs: the tool answers. A miss stays on the mouth below.
    if attach.is_none() && !crate::mouth_gist::needs_write_lane(&line) {
        if let Some(spoken) = crate::mouth_route::run(&app, &line) {
            if crate::state::update(&app, |st| {
                st.companion.last_bot_line = spoken.clone();
                st.companion.push_turn("them", &spoken);
                Ok(())
            })
            .is_ok()
            {
                let _ = app.emit("state:changed", ());
            }
            return Ok(spoken);
        }
    }
    // The conductor — a slim local fast ack in
    // front of the mouth (Settings toggle; off = today's path exactly).
    // Runs on the RAW line (pocket/desk wraps are for the mouth, not the
    // triage). A chip send is always for the AI, so the conductor stands
    // down while one rides. `chat` splashes the ack over the wait mark
    // (companion:ack — never a saved turn) and the mouth below lands the
    // real reply in the same bubble; any other verb's ack IS the
    // them-row — no mouth spend. Propose-don't-save lives inside: only
    // Matt's yes reaches action add.
    let mut hop_ack = "On it.".to_string();
    #[cfg(unix)]
    if attach.is_none() {
        match crate::mouth_ack::conduct(&app, &line).await {
            Some(crate::mouth_ack::Step::Final(them)) => {
                persist_action_intercept(&app, None, &them);
                return Ok(them);
            }
            Some(crate::mouth_ack::Step::Chat(ack)) => {
                hop_ack = ack.clone();
                let _ = app.emit(
                    "companion:ack",
                    serde_json::json!({ "line": ack, "verb": "chat" }),
                );
            }
            None => {}
        }
    }
    #[cfg(not(unix))]
    let _ = &hop_ack;
    // Grok hops get orbitty.hop.v0 (no pocket dump). Other backends
    // still get the leftover wrap. Persist + event log keep the raw line.
    let prompt = wrap_with_pocket(&app, &line);
    // Chip: original path always. Office may add MarkItDown markdown
    // (sidecar, detect-only). Images may ride Grok --prompt-json.
    let (prompt, vision_path) = match &attach {
        Some(a) => {
            let a = a.clone();
            let p = prompt.clone();
            let b = backend.clone();
            match tauri::async_runtime::spawn_blocking(move || {
                crate::attach_send::prepare(&a, &p, &b)
            })
            .await
            {
                Ok(prep) => (prep.prompt, prep.vision_path),
                Err(_) => (prompt, None),
            }
        }
        None => (prompt, None),
    };
    // Desk wrap only when the line looks like a board mint — not every
    // "hi". Planet ids stay available for todo: intercept without a chew.
    let prompt = if crate::companion_desk::line_needs_desk(&line) {
        crate::companion_desk::wrap_with_desk(&app, &prompt)
    } else {
        prompt
    };
    // Plug map — unknown backend is an honest error,
    // never a silent Berd fallback.
    let reply = match backend.as_str() {
        "goose" => companion_send_goose(&app, &prompt).await?,
        // The grok lane runs through the router —
        // Free Mode skips paid Grok; a cooked lane hops Ollama →
        // OpenRouter free with the hop packet. Same bubble either way.
        "grok" => {
            crate::mouth_hop::send_with_hops(
                &app,
                &line,
                &prompt,
                &hop_ack,
                vision_path.as_deref(),
            )
            .await?
        }
        "hermes" => companion_send_hermes(&app, &prompt).await?,
        "berd" => companion_send_berd(&prompt).await?,
        other => {
            return Err(format!(
                "Unknown ORBITTY_COMPANION_BACKEND {other:?} — goose, grok, hermes, or berd."
            ))
        }
    };

    // You-row already persisted before the model wait. Reply only here.
    if crate::state::update(&app, |st| {
        st.companion.last_bot_line = reply.clone();
        let you_already_saved = st
            .companion
            .turns
            .last()
            .is_some_and(|t| t.role == "you" && t.text == you_line);
        if !you_already_saved {
            st.companion.push_turn("you", &you_line);
        }
        st.companion.push_turn("them", &reply);
        Ok(())
    })
    .is_ok()
    {
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

/// Default mouth. `goose run -q` with the default
/// profile (full Goose tools — BLOCK-MH-4) and the soul as --system,
/// spawned with GOOSE_MODE=always_allow (no yes/no UI in the bubble).
/// First send names the session `orbitty-companion`,
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
    let roll_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_goose_reply(&bin, &soul, plug, &line, &roll_app)
    })
    .await
    .map_err(|e| format!("send task: {e}"))?
}

/// Product goose session name (never the probe's `orbitty-plug-goose`).
pub(crate) const GOOSE_SESSION_NAME: &str = "orbitty-companion";

/// Goose mouth argv: `run -q` + full-tools builtins, soul as --system,
/// resume by `--session-id ID --resume` when an id is known, else a fresh
/// `--name` session. Shared by the send path and mouth_roll's rollover.
pub(crate) fn companion_mode_tools(app: &tauri::AppHandle) -> bool {
    crate::state::load_state(app)
        .ok()
        .map(|s| matches!(s.companion.mode.as_str(), "spark" | "work"))
        .unwrap_or(true)
}

pub(crate) fn goose_args(
    soul: &str,
    line: &str,
    resume_id: Option<&str>,
    tools_ok: bool,
) -> Vec<String> {
    // Spark/work: Orbitty mechanic — tools on. Chat/hold: talk only.
    let mut args: Vec<String> = vec!["run".into(), "-q".into()];
    if tools_ok {
        args.push("--with-builtin=developer,computercontroller".into());
    } else {
        args.push("--no-profile".into());
    }
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
    // Equals form, one argv: clap takes everything after `=` literally,
    // even a value that starts with `-`.
    args.push(format!("--system={soul}"));
    args.push("-t".into());
    args.push(line.to_string());
    args
}

fn run_goose_reply(
    bin: &Path,
    soul: &str,
    mut plug: PlugState,
    line: &str,
    app: &tauri::AppHandle,
) -> Result<String, String> {
    let resume_id = plug
        .goose_session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| is_goose_session_id(s))
        .map(str::to_string);
    let first_send = resume_id.is_none();
    let args = goose_args(soul, line, resume_id.as_deref(), companion_mode_tools(app));

    // Config is `smart_approve` and the bubble has no yes/no UI — without
    // always_allow, tool calls would hang or be denied.
    // 300s (berd's 5 min) — with extensions actually
    // loaded, 180s killed real tool runs mid-reply. Grok/Hermes keep 180.
    // A dead resume rolls the mouth onto a fresh
    // session with a handoff — the bubble is one thread, never cleared.
    let reply = match run_mouth_cli("Goose", bin, &args, &[("GOOSE_MODE", "always_allow")], 300) {
        Ok(r) => r,
        Err(e) if !first_send && crate::mouth_roll::goose_context_dead(&e) => {
            let handoff = crate::mouth_roll::handoff_prompt(app, line);
            return crate::mouth_roll::goose_rollover(app, bin, soul, &mut plug, &handoff);
        }
        Err(e) => return Err(e),
    };
    if reply.is_empty() {
        // Empty after a resume = a thin/dead session — same rollover.
        if !first_send {
            let handoff = crate::mouth_roll::handoff_prompt(app, line);
            return crate::mouth_roll::goose_rollover(app, bin, soul, &mut plug, &handoff);
        }
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
pub(crate) fn capture_goose_session(bin: &Path) {
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

/// Grok mouth. `grok -p <prompt>` with the soul as
/// --rules, JSON out. Session UUID lives in plug.json: first send
/// `-s UUID`, later `--resume UUID` (re-passing `-s` on a live id fails —
/// a failed create is retried once as a resume to self-heal that race).
/// Honest error when grok is missing — no silent Berd fallback.
/// pub(crate): mouth_hop wraps this — Grok is the paid lane of the
/// router; a cooked lane (429/402/quota/session-wall) hops free lanes.
pub(crate) async fn companion_send_grok(
    app: &tauri::AppHandle,
    line: &str,
    vision_path: Option<String>,
) -> Result<String, String> {
    let bin = find_grok().ok_or(
        "`grok` not found — install the Grok CLI or put it on PATH (also checks ~/.grok/bin/grok).",
    )?;
    let soul = companion_soul_text(app);
    let plug = load_plug();
    let line = line.to_string();
    let roll_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_grok_reply(&bin, &soul, plug, &line, vision_path.as_deref(), &roll_app)
    })
    .await
    .map_err(|e| format!("send task: {e}"))?
}

pub(crate) fn grok_args(
    sid: &str,
    minted: bool,
    soul: &str,
    line: &str,
    tools_ok: bool,
    vision_json: Option<&str>,
) -> Vec<String> {
    let mut args: Vec<String> = match vision_json {
        Some(j) => vec!["--prompt-json".into(), j.to_string()],
        None => vec!["-p".into(), line.to_string()],
    };
    if minted {
        args.push("--resume".into());
    } else {
        args.push("-s".into());
    }
    args.push(sid.to_string());
    args.extend([
        "--output-format".into(),
        "streaming-json".into(),
        "--always-approve".into(),
        "--no-subagents".into(),
        "--disable-web-search".into(),
    ]);
    if !tools_ok {
        args.push("--disallowed-tools".into());
        args.push("bash,shell,computer".into());
    }
    args.extend([
        // Grok's clap is the same: equals form keeps a dash-leading
        // soul a value, one argv.
        format!("--rules={soul}"),
    ]);
    args
}

fn run_grok_reply(
    bin: &Path,
    soul: &str,
    mut plug: PlugState,
    line: &str,
    vision_path: Option<&str>,
    app: &tauri::AppHandle,
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

    // Thin the backend every N sends so a week-old resume does not
    // chew every ping. Bubble + leftovers + soul stay; only Grok's
    // session id forks (same as death-rollover).
    if plug.grok_session_minted && plug.grok_sends >= GROK_ROLL_EVERY {
        plug.grok_sends = 0;
        save_plug(&plug);
        let handoff = crate::mouth_roll::handoff_prompt(app, line);
        let text = crate::mouth_roll::grok_fresh_rollover(app, bin, soul, &mut plug, &handoff)?;
        plug.grok_sends = 1;
        save_plug(&plug);
        return Ok(text);
    }

    let vision_json = vision_path.map(|p| crate::attach_vision::prompt_json(line, p));
    let args = grok_args(
        &sid,
        plug.grok_session_minted,
        soul,
        line,
        companion_mode_tools(app),
        vision_json.as_deref(),
    );
    // streaming-json: thought/tool lines splash the wait slot;
    // assembled text is the reply. Dead resume still 404s on stderr
    // in ~0.5s — kill_on is the same door as before.
    let streamed = |args: &[String], watch: Option<&dyn Fn(&str) -> bool>| {
        crate::mouth_blip::run_streamed(app, "Grok", bin, args, 180, watch)
    };
    let text = match if plug.grok_session_minted {
        streamed(
            &args,
            Some(&crate::mouth_roll::grok_session_gone_line),
        )
    } else {
        streamed(&args, None)
    } {
        Ok(r) => r,
        Err(e) if !plug.grok_session_minted && e.to_ascii_lowercase().contains("session") => {
            plug.grok_session_minted = true;
            save_plug(&plug);
            let retry = grok_args(
                &sid,
                true,
                soul,
                line,
                companion_mode_tools(app),
                vision_json.as_deref(),
            );
            streamed(
                &retry,
                Some(&crate::mouth_roll::grok_session_gone_line),
            )?
        }
        Err(e) if plug.grok_session_minted && crate::mouth_roll::grok_session_gone(&e) => {
            let handoff = crate::mouth_roll::handoff_prompt(app, line);
            let text = crate::mouth_roll::grok_fresh_rollover(app, bin, soul, &mut plug, &handoff)?;
            plug.grok_sends = 1;
            save_plug(&plug);
            return Ok(text);
        }
        Err(e) if plug.grok_session_minted && crate::mouth_roll::grok_context_dead(&e) => {
            let handoff = crate::mouth_roll::handoff_prompt(app, line);
            let text = crate::mouth_roll::grok_rollover(app, bin, soul, &mut plug, &handoff)?;
            plug.grok_sends = 1;
            save_plug(&plug);
            return Ok(text);
        }
        Err(e) => return Err(e),
    };
    if text.is_empty() && plug.grok_session_minted {
        // Empty after a resume = a thin/dead session — same rollover.
        let handoff = crate::mouth_roll::handoff_prompt(app, line);
        let text = crate::mouth_roll::grok_rollover(app, bin, soul, &mut plug, &handoff)?;
        plug.grok_sends = 1;
        save_plug(&plug);
        return Ok(text);
    }
    if text.is_empty() {
        return Err("Grok sent back an empty reply.".into());
    }
    if !plug.grok_session_minted {
        plug.grok_session_minted = true;
    }
    plug.grok_sends = plug.grok_sends.saturating_add(1);
    save_plug(&plug);
    Ok(text)
}

pub(crate) fn grok_reply_text(raw: &str) -> String {
    let v: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    v.get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
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
        // Hermes stays the compat mouth — `--safe-mode`
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

// Wake a closed claimed window

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
/// rail owns claim writes. Appends `sleeper-wake`
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
    // Rail bumps lastTouchedAt (zombie fade dim) — planet clicks never
    // go through wakeSleeper() in Svelte.
    if out.is_ok() {
        use tauri::Emitter;
        let _ = app.emit_to("main", "habitat:sleeper-woke", id.to_string());
    }
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

    // 1b. Rematch candidate already live (title may have drifted since the
    //     claim stamped) → focus it. The rail's rematchClaims rewrites the
    //     claim id on the next poll; wake stays read-only on state.
    if let Ok(wins) = crate::windows::list_windows() {
        let claimed: std::collections::HashSet<&str> =
            st.claims.iter().map(|c| c.window_id.as_str()).collect();
        let unused = |w: &crate::windows::WindowInfo| !claimed.contains(w.id.as_str());
        let bundle = claim
            .bundle_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(claim.app_name.as_deref().filter(|s| !s.is_empty()));
        let mut pick: Option<&crate::windows::WindowInfo> = None;
        if let (Some(b), Some(title)) = (bundle, claim.title_at_claim.as_deref()) {
            pick = wins.iter().find(|w| {
                unused(w)
                    && (w.bundle_id.as_deref().unwrap_or(w.app_name.as_str()) == b)
                    && w.title == title
            });
        }
        if pick.is_none() {
            if let Some(path) = non_empty(claim.file_path.as_deref()) {
                pick = wins
                    .iter()
                    .find(|w| unused(w) && w.document_path.as_deref() == Some(path));
            }
        }
        if pick.is_none() {
            if let Some(b) = bundle {
                let same: Vec<_> = wins
                    .iter()
                    .filter(|w| {
                        unused(w)
                            && (w.bundle_id.as_deref().unwrap_or(w.app_name.as_str()) == b)
                    })
                    .collect();
                if same.len() == 1 {
                    pick = Some(same[0]);
                }
            }
        }
        if let Some(w) = pick {
            if crate::windows::restore_window(&w.id).is_ok() {
                return Ok("Woke the rematched window.".into());
            }
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

    // 5. App fallback: open a NEW instance when nothing unused was free to
    //    rematch (another planet often already owns the only live window of
    //    this bundle — plain `open -b` would only focus theirs). `-n -b`
    //    mints a window the rail can soft-rematch on the next poll.
    if let Some(bundle) = non_empty(claim.bundle_id.as_deref()) {
        if spawn_detached("open", &["-n", "-b", bundle]) {
            return Ok(format!("Opened a new {bundle} window — rematch on next poll."));
        }
        // Some apps refuse -n; fall back to activate.
        if spawn_detached("open", &["-b", bundle]) {
            return Ok(format!("Reopened the app ({bundle})."));
        }
    }
    if let Some(name) = non_empty(claim.app_name.as_deref()) {
        if spawn_detached("open", &["-na", name]) {
            return Ok(format!("Opened a new {name} window — rematch on next poll."));
        }
        if spawn_detached("open", &["-a", name]) {
            return Ok(format!("Reopened {name}."));
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

// Actions lane: user-authored shell shortcuts

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
/// clipped and returned for the lane's note. Appends
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

/// The logged run body, shared by the invoke
/// command, the control plane (`action.run`), and combo presses. Same
/// contract: `/bin/zsh -lc`, 120s, event log lengths only.
pub(crate) fn run_action_logged(
    app: &tauri::AppHandle,
    command: String,
    cwd: Option<String>,
) -> Result<ActionResult, String> {
    #[cfg(unix)]
    crate::gate_caps::check(app)?;
    #[cfg(unix)]
    let _guard = crate::gate_caps::begin_action();
    crate::event_log::append_best(
        app,
        "action-start",
        None,
        None,
        Some("action"),
        true,
        &serde_json::json!({ "n": command.chars().count() }),
    );
    let pid = crate::task::project_for_cwd(app, cwd.as_deref());
    let card_id = pid.as_deref().and_then(|p| {
        let title: String = {
            let t = command.trim();
            if t.chars().count() > 48 {
                format!("task: {}…", t.chars().take(47).collect::<String>())
            } else {
                format!("task: {t}")
            }
        };
        crate::board::kanban_add_as(app, &title, p, "doing", "action", None, Vec::new())
            .ok()
            .map(|c| c.id)
    });
    let task_id = pid.as_deref().and_then(|p| {
        crate::task::start(app, p, card_id.as_deref())
    });
    let out = run_action_inner(command, cwd);
    let ok = out.as_ref().map(|r| r.ok).unwrap_or(false);
    if let Some(tid) = task_id.as_deref() {
        crate::task::end(app, tid, if ok { "done" } else { "failed" });
    }
    if let Some(cid) = card_id.as_deref() {
        let _ = crate::board::kanban_move(app, cid, if ok { "done" } else { "todo" });
    }
    #[cfg(unix)]
    if let Ok(r) = &out {
        if let Some(usd) = crate::gate_caps::parse_usage_usd(&r.stdout)
            .or_else(|| crate::gate_caps::parse_usage_usd(&r.stderr))
        {
            crate::gate_caps::add_spend(app, usd);
        }
    }
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

// Agents lane: harness shortcuts (no LLM inside Orbitty)

pub(crate) fn find_on_path(bin: &str) -> Option<PathBuf> {
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
/// the row's free-text `extra` + titles-only
/// planet context when the shortcut is attached to a project OR the caller
/// handed titles over (pickup's one assigned todo — a passed title is
/// never silently dropped). Overwritten each launch, same honesty rules
/// as the project context file.
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
    // The row's extra instructions ride the same
    // query file — after the md paths, before the todo/planet block.
    if let Some(extra) = shortcut
        .extra
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty())
    {
        body.push_str(&format!("\n\n## Extra instructions\n\n{extra}\n"));
    }
    if shortcut.project_id.is_some() || !titles.is_empty() {
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

/// The visible-harness door shared by cursor /
/// goose / grok — a `.command` launcher next to the query file (loads it
/// into `$prompt`), opened as a fresh Terminal window. `exec_line` is the
/// harness's own command line — run as a CHILD, never `exec`: exec would
/// replace the shell and skip the EXIT trap.
/// the trap turns the dock
/// LED off when the harness exits (the same idea as writer-done-needed —
/// a stuck on-light is a leftover, not a feature); HUP (Terminal window
/// closed) exits through the same trap. Silent + best-effort: a missing
/// ctl never noises up the harness session.
/// Per-launch Terminal launcher paths in `context/`:
/// `agent-<shortcut>-<run>.command` / `.pid`. Per RUN, not per shortcut —
/// two runs of one shortcut used to share `agent-<shortcut>.pid`, and the
/// second run's EXIT trap deleted the file the first still needed (boot
/// then called the live first run dead; stop could kill the wrong one).
/// `run` is the task id, else the card id, else a launch nonce.
pub(crate) fn terminal_run_paths(context_dir: &Path, shortcut_id: &str, run: &str) -> (PathBuf, PathBuf) {
    let stem = format!("agent-{}-{}", safe_id(shortcut_id), safe_id(run));
    (
        context_dir.join(format!("{stem}.command")),
        context_dir.join(format!("{stem}.pid")),
    )
}

/// Sibling of the running app binary. Nightly's trap must call Nightly's
/// orbittyctl, never a PATH copy.
pub(crate) fn bundled_orbittyctl() -> PathBuf {
    match std::env::current_exe() {
        Ok(exe) => exe
            .parent()
            .map(|d| d.join("orbittyctl"))
            .unwrap_or_else(|| PathBuf::from("orbittyctl")),
        Err(_) => PathBuf::from("orbittyctl"),
    }
}

pub(crate) fn control_sock_path(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .map(|d| d.join("control.sock"))
        .unwrap_or_else(|_| PathBuf::from("control.sock"))
}

/// Empty setting: Ghostty when the app bundle is installed, else Terminal.
pub(crate) fn resolve_agent_terminal(setting: &str, ghostty_installed: bool) -> &'static str {
    match setting.trim().to_ascii_lowercase().as_str() {
        "ghostty" => "Ghostty",
        "terminal" => "Terminal",
        _ => {
            if ghostty_installed {
                "Ghostty"
            } else {
                "Terminal"
            }
        }
    }
}

fn ghostty_installed() -> bool {
    Path::new("/Applications/Ghostty.app").is_dir()
}

pub(crate) fn open_terminal_argv(app_name: &str, script: &Path) -> Vec<String> {
    vec!["-a".into(), app_name.into(), script.display().to_string()]
}

/// POSIX single-quote one value for a zsh line: `'…'`, each `'` → `'\''`.
/// Nothing inside single quotes expands (`$()`, backticks, `;`, newlines).
/// Same law as `sh_quote` (the Terminal query launcher's).
pub(crate) fn sq(s: &str) -> String {
    sh_quote(s)
}

/// `.command` body. The EXIT trap calls the bundled ctl with this app's
/// socket. Every interpolated value is single-quoted (`sq`), and the trap
/// names a cleanup function so no path is re-parsed inside a trap string.
/// The trap never names `~/.local/bin`.
pub(crate) fn agent_command_body(
    query: &Path,
    script: &Path,
    pidf: &Path,
    exec_line: &str,
    ctl: &Path,
    sock: &Path,
    cwd: Option<&str>,
) -> String {
    let cd = match cwd.map(str::trim).filter(|c| !c.is_empty()) {
        Some(dir) => format!("cd {} || exit 1\n", sq(dir)),
        None => String::new(),
    };
    let q = sq(&query.display().to_string());
    let pid = sq(&pidf.display().to_string());
    let script = sq(&script.display().to_string());
    let ctl = sq(&ctl.display().to_string());
    let sock = sq(&sock.display().to_string());
    format!(
        "#!/bin/zsh\n{cd}prompt=\"$(cat {q})\"\necho $$ > {pid}\norbitty_cleanup() {{\n  {ctl} --sock {sock} companion hack -- off >/dev/null 2>&1 || true\n  rm -f {pid} {script}\n}}\ntrap 'exit' HUP\ntrap orbitty_cleanup EXIT\n{exec_line}\n"
    )
}

/// Where a project permission file must never go: `/`, `$HOME` itself,
/// and anything under `$HOME/.cursor` (the user-global Cursor dir). Both
/// sides are canonicalized, so `$HOME/sub/..` or a symlink can't slip
/// past. `None` = allowed. A main (non-linked) checkout — `~/orbitty`
/// included — is allowed: the local exclude keeps the file out of commits.
fn cursor_perm_refusal(workspace: &Path, home: Option<&Path>) -> Option<String> {
    let Ok(ws) = std::fs::canonicalize(workspace) else {
        return Some("workspace does not exist".into());
    };
    if !ws.is_dir() {
        return Some("workspace is not a directory".into());
    }
    if ws == Path::new("/") {
        return Some("workspace is /".into());
    }
    let home = home
        .filter(|h| !h.as_os_str().is_empty())
        .and_then(|h| std::fs::canonicalize(h).ok());
    if let Some(home) = home {
        if ws == home {
            return Some("workspace is $HOME".into());
        }
        if ws.starts_with(home.join(".cursor")) {
            return Some("workspace is inside ~/.cursor".into());
        }
    }
    None
}

pub(crate) fn project_cli_path(workspace: &Path) -> PathBuf {
    workspace.join(".cursor").join("cli.json")
}

/// What this app added to an untracked `cli.json` last time, so the next
/// preset can take its own entries back without dropping the user's.
fn managed_sidecar_path(workspace: &Path) -> PathBuf {
    workspace.join(".cursor").join("orbitty-managed.json")
}

/// Patterns in the repo's local exclude. `**/` so a subdirectory
/// workspace (`sub/.cursor/cli.json`) is covered too.
const CURSOR_EXCLUDES: [&str; 2] = ["**/.cursor/cli.json", "**/.cursor/orbitty-managed.json"];

/// `git -C <ws> <args>` stdout (trimmed) on success; `None` = not a repo /
/// git missing / failed.
fn git_out(workspace: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The exclude file git actually reads for this workspace:
/// `git rev-parse --git-path info/exclude` (the COMMON dir for a linked
/// worktree, correct from any subdirectory). `None` = not a git repo.
fn exclude_file(workspace: &Path) -> Option<PathBuf> {
    let raw = git_out(workspace, &["rev-parse", "--git-path", "info/exclude"])?;
    if raw.is_empty() {
        return None;
    }
    let p = PathBuf::from(raw);
    Some(if p.is_absolute() { p } else { workspace.join(p) })
}

/// `.cursor/cli.json` (relative to the workspace) is tracked in git.
fn cli_json_tracked(workspace: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["ls-files", "--error-unmatch", "--", ".cursor/cli.json"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Keep the project permission file out of commits. Local exclude only —
/// never the product `.gitignore`, never `git add`. Idempotent.
fn local_exclude(exclude: &Path) -> Result<(), String> {
    if let Some(dir) = exclude.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let mut existing = std::fs::read_to_string(exclude).unwrap_or_default();
    let missing: Vec<&str> = CURSOR_EXCLUDES
        .iter()
        .copied()
        .filter(|p| !existing.lines().any(|l| l.trim() == *p))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    for p in missing {
        existing.push_str(p);
        existing.push('\n');
    }
    std::fs::write(exclude, existing).map_err(|e| format!("write {}: {e}", exclude.display()))
}

fn str_list(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// Union-merge one list: the user's entries (file minus what we added
/// last time) stay in order, ours are appended. Returns (merged, added)
/// where `added` is ours that the user didn't already have.
fn union_list(file: &[String], ours_before: &[String], ours_now: &[&str]) -> (Vec<String>, Vec<String>) {
    let mut merged: Vec<String> = file
        .iter()
        .filter(|t| !ours_before.contains(t))
        .cloned()
        .collect();
    let mut added = Vec::new();
    for t in ours_now {
        if !merged.iter().any(|m| m == t) {
            merged.push((*t).to_string());
            added.push((*t).to_string());
        }
    }
    (merged, added)
}

/// Undo for one `install_cursor_permissions` write: the launch that
/// needed it failed, so put the files back the way they were.
pub(crate) struct CursorPermUndo {
    files: Vec<(PathBuf, Option<Vec<u8>>)>,
    made_dir: Option<PathBuf>,
}

impl CursorPermUndo {
    pub(crate) fn rollback(self) {
        for (path, prev) in self.files {
            match prev {
                Some(bytes) => {
                    let _ = std::fs::write(&path, bytes);
                }
                None => {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        if let Some(dir) = self.made_dir {
            let _ = std::fs::remove_dir(dir); // only if now empty
        }
    }
}

/// Write the run workspace's `.cursor/cli.json` permissions for this
/// preset. Never touches the user-global Cursor CLI config.
/// - Refuses `/`, `$HOME` and `~/.cursor/…` (canonical paths).
/// - A TRACKED `.cursor/cli.json` is the repo's own — left untouched.
/// - An untracked one is union-merged: the user's allow/deny stay (a user
///   deny is never dropped), ours are appended; what we added is recorded
///   in `.cursor/orbitty-managed.json` so the next preset can take it back.
/// - In a git repo, `**/.cursor/cli.json` goes in the exclude git reads
///   (`rev-parse --git-path info/exclude`); never `git add`.
/// Cleanup: delete `.cursor/cli.json` + `.cursor/orbitty-managed.json`.
pub(crate) fn install_cursor_permissions(workspace: &Path, preset: &str) -> Result<CursorPermUndo, String> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    install_cursor_permissions_with_home(workspace, preset, home.as_deref())
}

fn install_cursor_permissions_with_home(
    workspace: &Path,
    preset: &str,
    home: Option<&Path>,
) -> Result<CursorPermUndo, String> {
    if let Some(why) = cursor_perm_refusal(workspace, home) {
        return Err(format!("no project workspace for cursor permissions: {why}"));
    }
    let exclude = exclude_file(workspace);
    if exclude.is_some() && cli_json_tracked(workspace) {
        return Err("tracked .cursor/cli.json left untouched".into());
    }
    let path = project_cli_path(workspace);
    let sidecar = managed_sidecar_path(workspace);
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| workspace.join(".cursor"));
    let prev_cli = std::fs::read(&path).ok();
    let prev_side = std::fs::read(&sidecar).ok();
    let mut root = match prev_cli.as_deref() {
        Some(raw) => match serde_json::from_slice::<serde_json::Value>(raw) {
            Ok(v) if v.is_object() => v,
            Ok(_) => return Err(format!("{} is not a JSON object; left untouched", path.display())),
            Err(_) => return Err(format!("{} is not valid JSON; left untouched", path.display())),
        },
        None => serde_json::json!({ "version": 1 }),
    };
    let managed: serde_json::Value = prev_side
        .as_deref()
        .and_then(|b| serde_json::from_slice(b).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let ours = crate::agents::cursor_permissions_value(preset);
    let ours_allow: Vec<&str> = ours["allow"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).collect();
    let ours_deny: Vec<&str> = ours["deny"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).collect();
    let file_perm = root.get("permissions").cloned().unwrap_or_else(|| serde_json::json!({}));
    let (allow, added_allow) = union_list(&str_list(&file_perm["allow"]), &str_list(&managed["allow"]), &ours_allow);
    let (deny, added_deny) = union_list(&str_list(&file_perm["deny"]), &str_list(&managed["deny"]), &ours_deny);
    if root.get("version").is_none() {
        root["version"] = serde_json::json!(1);
    }
    let mut perm = if file_perm.is_object() { file_perm } else { serde_json::json!({}) };
    perm["allow"] = serde_json::json!(allow);
    perm["deny"] = serde_json::json!(deny);
    root["permissions"] = perm;

    // Exclude first: the file never exists un-ignored in a repo.
    if let Some(ex) = exclude.as_deref() {
        local_exclude(ex)?;
    }
    let made_dir = if dir.is_dir() {
        None
    } else {
        std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        Some(dir.clone())
    };
    let undo = CursorPermUndo {
        files: vec![(path.clone(), prev_cli), (sidecar.clone(), prev_side)],
        made_dir,
    };
    let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())? + "\n";
    let side = serde_json::to_string_pretty(&serde_json::json!({
        "allow": added_allow,
        "deny": added_deny,
    }))
    .map_err(|e| e.to_string())?
        + "\n";
    if let Err(e) = std::fs::write(&path, text).and_then(|_| std::fs::write(&sidecar, side)) {
        undo.rollback();
        return Err(format!("write {}: {e}", path.display()));
    }
    Ok(undo)
}

fn open_agent_terminal(
    app: &tauri::AppHandle,
    query: &Path,
    script: &Path,
    pidf: &Path,
    exec_line: &str,
    cwd: Option<&str>,
    planet_id: Option<&str>,
) -> Result<String, String> {
    let ctl = bundled_orbittyctl();
    let sock = control_sock_path(app);
    let setting = crate::state::load_state(app)
        .map(|st| st.agent_terminal.clone())
        .unwrap_or_default();
    let app_name = resolve_agent_terminal(&setting, ghostty_installed());
    let body = agent_command_body(query, script, pidf, exec_line, &ctl, &sock, cwd);
    std::fs::write(script, &body).map_err(|e| format!("write {}: {e}", script.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o755));
    }
    let argv = open_terminal_argv(app_name, script);
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    if spawn_detached("open", &argv_ref) {
        crate::event_log::append_best(
            app,
            "agent-terminal",
            None,
            planet_id,
            Some("agent"),
            true,
            &serde_json::json!({ "app": app_name }),
        );
        Ok(app_name.to_string())
    } else {
        Err(format!(
            "Couldn't open {app_name} launcher. Query file ready: {}",
            query.display()
        ))
    }
}

/// Launch a user-authored agent shortcut in its external harness. Orbitty
/// writes the query file and hands off — it never runs the model.
/// The harness is per row (cursor | goose | grok
/// | hermes) with a named door each — an unknown harness is an honest
/// error, never a silent hermes dump. Each door also owns the dock LED
/// off for the run it launched (pickup lit it): the Terminal `.command`
/// traps EXIT, the hermes door waits on the child.
pub fn launch_agent(
    app: tauri::AppHandle,
    shortcut: crate::state::AgentShortcut,
    project_name: Option<String>,
    titles: Vec<String>,
) -> Result<String, String> {
    #[cfg(unix)]
    crate::gate_caps::check(&app)?;
    let pid = shortcut.project_id.clone().unwrap_or_default();
    let card_id = crate::state::load_state(&app).ok().and_then(|st| {
        titles.first().and_then(|t| {
            st.kanban_cards.iter().find(|c| {
                &c.title == t && (pid.is_empty() || c.project_id == pid)
            }).map(|c| c.id.clone())
        })
    });
    let task_id = if pid.is_empty() {
        None
    } else {
        crate::task::start(&app, &pid, card_id.as_deref())
    };
    let (query, skipped) = write_agent_query(&app, &shortcut, project_name.as_deref(), &titles)?;
    let skip_note = if skipped > 0 {
        format!(" Skipped {skipped} unreadable md path(s).")
    } else {
        String::new()
    };
    // Boot liveness (task::boot): what we handed off to, per door.
    let mut hint = crate::task::LiveHint::default();
    let run_tag = task_id.clone().or_else(|| card_id.clone()).unwrap_or_else(|| {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        format!("n{ms:x}")
    });
    let context_dir = query
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let (terminal_script, terminal_pid_file) =
        terminal_run_paths(&context_dir, &shortcut.id, &run_tag);
    let ws_state = crate::state::load_state(&app).ok();
    let workspace = crate::herdr::planet_cwd(ws_state.as_ref(), &pid, project_name.as_deref());
    let planet_for_log = shortcut.project_id.as_deref().filter(|p| !p.is_empty());
    let out: Result<String, String> = (|| {
    match shortcut.harness.trim().to_ascii_lowercase().as_str() {
        "cursor" => {
            let preset = crate::agents::normalize_permissions(Some(shortcut.permissions.as_str()));
            // The model reaches herdr argv and a zsh line: refuse, never strip.
            let model = crate::agents::cursor_model(shortcut.model.as_deref());
            crate::agents::validate_model(&model)?;
            let ws = Path::new(&workspace);
            // Project permissions exist only for a launch that happened.
            // The file is written once a door is chosen: before
            // herdr::branch() for the herdr door (herdr starts
            // cursor-agent itself, so it must already be there), and again
            // for the terminal door once cursor-agent is found. Either is
            // rolled back if that door doesn't launch (herdr skip, open
            // failure). Refusals (tracked cli.json, /, $HOME, ~/.cursor) are
            // logged and the launch still goes. A main checkout, ~/orbitty
            // included, is allowed; the local exclude keeps it uncommitted.
            let install = || match install_cursor_permissions(ws, preset) {
                Ok(undo) => Some(undo),
                Err(why) => {
                    crate::event_log::append_best(
                        &app,
                        "cursor-permissions",
                        None,
                        planet_for_log,
                        Some("agent"),
                        true,
                        &serde_json::json!({ "reason": why, "preset": preset }),
                    );
                    None
                }
            };
            // Herdr door: herdr starts cursor-agent itself, so the file must
            // be there before branch(). A skip takes it back.
            let undo = install();
            match crate::herdr::branch(&app, &shortcut, project_name.as_deref(), &titles, Some(query.as_path())) {
                crate::herdr::Branch::Skip => {
                    if let Some(u) = undo {
                        u.rollback();
                    }
                }
                crate::herdr::Branch::Up(m, agent) | crate::herdr::Branch::Blocked(m, agent) => {
                    hint.herdr_agent = Some(agent);
                    return Ok(format!("{m} Query: {}.{skip_note}", query.display()));
                }
            }
            // Terminal door: only once the CLI is known to exist.
            let Some(bin) = find_on_path("cursor-agent") else {
                return Err(
                    "cursor-agent not on PATH — install the Cursor CLI or pick another harness."
                        .to_string(),
                );
            };
            let exec = crate::agents::cursor_exec_line(&bin.display().to_string(), &model, preset);
            let undo = install();
            let opened = match open_agent_terminal(
                &app,
                &query,
                &terminal_script,
                &terminal_pid_file,
                &exec,
                Some(workspace.as_str()),
                planet_for_log,
            ) {
                Ok(o) => o,
                Err(e) => {
                    if let Some(u) = undo {
                        u.rollback();
                    }
                    return Err(e);
                }
            };
            hint.pid_file = Some(terminal_pid_file.clone());
            Ok(format!(
                "Opened cursor-agent in {opened}. Query: {}.{skip_note}",
                query.display()
            ))
        }
        // Goose is not a herdr kind. branch() logs the skip, then the
        // terminal door runs. Never a silent hermes.
        "goose" => {
            match crate::herdr::branch(&app, &shortcut, project_name.as_deref(), &titles, Some(query.as_path())) {
                crate::herdr::Branch::Skip => {}
                crate::herdr::Branch::Up(m, agent) | crate::herdr::Branch::Blocked(m, agent) => {
                    hint.herdr_agent = Some(agent);
                    return Ok(format!("{m} Query: {}.{skip_note}", query.display()));
                }
            }
            let Some(bin) = find_on_path("goose") else {
                return Err(
                    "`goose` not on PATH — install the Goose CLI or pick another harness."
                        .to_string(),
                );
            };
            // Same full-tools builtin set as the companion goose mouth
            // (BLOCK-MH-4): the agent can act, not just talk.
            let opened = open_agent_terminal(
                &app,
                &query,
                &terminal_script,
                &terminal_pid_file,
                &format!(
                    "{} run --with-builtin=developer,computercontroller -t \"$prompt\"",
                    sq(&bin.display().to_string())
                ),
                Some(workspace.as_str()),
                planet_for_log,
            )?;
            hint.pid_file = Some(terminal_pid_file.clone());
            Ok(format!(
                "Opened goose run in {opened}. Query: {}.{skip_note}",
                query.display()
            ))
        }
        "grok" => {
            // The herdr branch of this door —
            // pane first, agent second, when herdr is present + up.
            // Skip falls through to the terminal launcher
            // (one skip line was already logged; never a dual-launch).
            match crate::herdr::branch(&app, &shortcut, project_name.as_deref(), &titles, Some(query.as_path())) {
                crate::herdr::Branch::Skip => {}
                crate::herdr::Branch::Up(m, agent) | crate::herdr::Branch::Blocked(m, agent) => {
                    hint.herdr_agent = Some(agent);
                    return Ok(format!("{m} Query: {}.{skip_note}", query.display()));
                }
            }
            let Some(bin) = find_grok() else {
                return Err(
                    "`grok` not on PATH — install the Grok CLI or pick another harness."
                        .to_string(),
                );
            };
            let opened = open_agent_terminal(
                &app,
                &query,
                &terminal_script,
                &terminal_pid_file,
                &format!("{} -p \"$prompt\"", sq(&bin.display().to_string())),
                Some(workspace.as_str()),
                planet_for_log,
            )?;
            hint.pid_file = Some(terminal_pid_file.clone());
            Ok(format!(
                "Opened grok in {opened}. Query: {}.{skip_note}",
                query.display()
            ))
        }
        // Hermes tries herdr first. On skip, hermes -p <profile> chat
        // with the query file. No terminal launcher on that fallback, so
        // a waiter thread owns the LED off. Detached stdio, never a poll.
        "hermes" => {
            match crate::herdr::branch(&app, &shortcut, project_name.as_deref(), &titles, Some(query.as_path())) {
                crate::herdr::Branch::Skip => {}
                crate::herdr::Branch::Up(m, agent) | crate::herdr::Branch::Blocked(m, agent) => {
                    hint.herdr_agent = Some(agent);
                    return Ok(format!("{m} Query: {}.{skip_note}", query.display()));
                }
            }
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
            let spawned = Command::new("hermes")
                .args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            match spawned {
                Ok(mut child) => {
                    hint.pid = Some(child.id());
                    let app2 = app.clone();
                    let tid = task_id.clone();
                    std::thread::spawn(move || {
                        let ok = child.wait().map(|st| st.success()).unwrap_or(false);
                        let _ = set_companion_hack(app2.clone(), false, None, None);
                        // Observed exit: close the task so boot never
                        // has to guess about it.
                        if let Some(tid) = tid {
                            crate::task::end(&app2, &tid, if ok { "done" } else { "failed" });
                        }
                    });
                    Ok(format!(
                        "Launched hermes -p {profile} chat. Query: {}.{skip_note}",
                        query.display()
                    ))
                }
                Err(_) => Err(format!(
                    "`hermes` not on PATH. Query file ready: {}",
                    query.display()
                )),
            }
        }
        // Opencode is a herdr kind, detect-only
        // — legal with `opencode` on PATH, never prompt-installed, never
        // `opencode serve`d as the mouth. No Terminal door exists for it,
        // so a skip is an honest error, not a fall-through.
        "opencode" => match crate::herdr::branch(&app, &shortcut, project_name.as_deref(), &titles, Some(query.as_path())) {
            crate::herdr::Branch::Up(m, agent) | crate::herdr::Branch::Blocked(m, agent) => {
                hint.herdr_agent = Some(agent);
                Ok(format!("{m} Query: {}.{skip_note}", query.display()))
            }
            crate::herdr::Branch::Skip => Err(
                "`opencode` needs herdr up + `opencode` on PATH (detect-only — never prompt-installed). Pick another harness."
                    .to_string(),
            ),
        },
        other => Err(format!(
            "unknown harness {other:?} — cursor | goose | grok | hermes (set per agent row; never a silent hermes)"
        )),
    }
    })();
    if let (Some(tid), Err(_)) = (task_id.as_deref(), &out) {
        crate::task::end(&app, tid, "failed");
    }
    if let (Some(tid), Ok(_)) = (task_id.as_deref(), &out) {
        if hint.pid.is_some() || hint.pid_file.is_some() || hint.herdr_agent.is_some() {
            crate::task::note_live(&app, tid, &hint);
        }
    }
    out
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

#[cfg(test)]
mod launch_tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn terminal_setting_picks_ghostty_when_empty_and_installed() {
        assert_eq!(resolve_agent_terminal("", true), "Ghostty");
        assert_eq!(resolve_agent_terminal("", false), "Terminal");
        assert_eq!(resolve_agent_terminal("ghostty", false), "Ghostty");
        assert_eq!(resolve_agent_terminal("terminal", true), "Terminal");
        assert_eq!(resolve_agent_terminal("nope", true), "Ghostty");
        assert_eq!(resolve_agent_terminal("nope", false), "Terminal");
    }

    #[test]
    fn command_script_uses_bundled_ctl_and_sock() {
        let ctl = PathBuf::from("/Applications/Orbitty Nightly.app/Contents/MacOS/orbittyctl");
        let sock = PathBuf::from("/tmp/nightly/control.sock");
        let body = agent_command_body(
            Path::new("/tmp/q.md"),
            Path::new("/tmp/run.command"),
            Path::new("/tmp/run.pid"),
            "\"/usr/local/bin/cursor-agent\" --model \"grok-4.7-high\" \"$prompt\"",
            &ctl,
            &sock,
            Some("/tmp/workspace"),
        );
        assert!(body.contains(ctl.to_str().unwrap()));
        assert!(body.contains("--sock"));
        assert!(body.contains(sock.to_str().unwrap()));
        assert!(body.contains("cd '/tmp/workspace'"));
        assert!(body.contains("trap orbitty_cleanup EXIT"));
        assert!(!body.contains(".local/bin"));
        let argv = open_terminal_argv("Ghostty", Path::new("/tmp/run.command"));
        assert_eq!(
            argv,
            vec![
                "-a".to_string(),
                "Ghostty".to_string(),
                "/tmp/run.command".to_string()
            ]
        );
    }

    #[test]
    fn cursor_permissions_land_in_the_worktree_exclude() {
        let dir = std::env::temp_dir().join(format!(
            "orbitty-cursor-perm-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let init = std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&dir)
            .status()
            .expect("git init");
        assert!(init.success(), "git init");
        install_cursor_permissions(&dir, "read-only").unwrap();
        let cli_path = project_cli_path(&dir);
        assert!(cli_path.ends_with(".cursor/cli.json"));
        assert!(!cli_path.to_string_lossy().contains("cli-config.json"));
        let raw = std::fs::read_to_string(&cli_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let allow = v["permissions"]["allow"].as_array().unwrap();
        assert!(allow.iter().any(|t| t == "Shell(find)"));
        assert!(allow.iter().any(|t| t == "Shell(head)"));
        assert!(allow.iter().all(|t| !t.as_str().unwrap_or("").starts_with("Write(")));
        assert!(v["permissions"]["deny"].as_array().unwrap().iter().any(|t| t == "Write(**)"));
        let exclude = std::fs::read_to_string(dir.join(".git/info/exclude")).unwrap();
        assert!(exclude.lines().any(|l| l.trim() == "**/.cursor/cli.json"));

        let mut root: serde_json::Value = serde_json::from_str(&raw).unwrap();
        root["editor"] = serde_json::json!({ "vimMode": true });
        std::fs::write(&cli_path, serde_json::to_string_pretty(&root).unwrap()).unwrap();
        install_cursor_permissions(&dir, "workspace-write").unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&cli_path).unwrap()).unwrap();
        assert_eq!(v["editor"]["vimMode"], true);
        assert!(v["permissions"]["allow"].as_array().unwrap().iter().any(|t| t == "Write(**)"));
        assert!(v["permissions"]["allow"].as_array().unwrap().iter().all(|t| {
            !t.as_str().unwrap_or("").starts_with("Write(/")
        }));
        assert!(v["permissions"]["deny"].as_array().unwrap().is_empty());

        install_cursor_permissions(&dir, "full-auto").unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&cli_path).unwrap()).unwrap();
        assert!(v["permissions"]["deny"].as_array().unwrap().is_empty());

        let home = std::env::var("HOME").unwrap_or_default();
        if !home.is_empty() {
            assert!(install_cursor_permissions(Path::new(&home), "read-only").is_err());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "orbitty-cl-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::canonicalize(&d).unwrap()
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .expect("git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn repo_with_commit(dir: &Path) {
        git(dir, &["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("README"), "x\n").unwrap();
        git(dir, &["add", "README"]);
        git(dir, &["commit", "-q", "-m", "init", "--no-verify"]);
    }

    fn arr(v: &serde_json::Value, key: &str) -> Vec<String> {
        v["permissions"][key]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn linked_worktree_cli_json_is_ignored() {
        let base = tmp("wt");
        let main = base.join("main");
        std::fs::create_dir_all(&main).unwrap();
        repo_with_commit(&main);
        let wt = base.join("wt");
        git(&main, &["worktree", "add", "-q", "-b", "wt", wt.to_str().unwrap()]);
        install_cursor_permissions_with_home(&wt, "read-only", None).unwrap();
        assert!(project_cli_path(&wt).is_file());
        assert_eq!(git(&wt, &["status", "--porcelain", "-uall"]).trim(), "");
        git(&wt, &["check-ignore", "-q", ".cursor/cli.json"]);
        let common = std::fs::read_to_string(main.join(".git/info/exclude")).unwrap();
        assert!(common.lines().any(|l| l == "**/.cursor/cli.json"));
        // Idempotent: a second install doesn't duplicate the pattern.
        install_cursor_permissions_with_home(&wt, "read-only", None).unwrap();
        let common = std::fs::read_to_string(main.join(".git/info/exclude")).unwrap();
        assert_eq!(common.lines().filter(|l| *l == "**/.cursor/cli.json").count(), 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn subdirectory_workspace_cli_json_is_ignored() {
        let base = tmp("sub");
        repo_with_commit(&base);
        let sub = base.join("pkg").join("app");
        std::fs::create_dir_all(&sub).unwrap();
        install_cursor_permissions_with_home(&sub, "read-only", None).unwrap();
        assert!(sub.join(".cursor/cli.json").is_file());
        assert_eq!(git(&base, &["status", "--porcelain", "-uall"]).trim(), "");
        git(&sub, &["check-ignore", "-q", ".cursor/cli.json"]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn tracked_cli_json_is_left_untouched() {
        let base = tmp("tracked");
        repo_with_commit(&base);
        std::fs::create_dir_all(base.join(".cursor")).unwrap();
        let theirs = "{\n  \"version\": 1,\n  \"permissions\": {\"allow\": [\"Shell(npm)\"], \"deny\": [\"Shell(rm)\"]}\n}\n";
        std::fs::write(base.join(".cursor/cli.json"), theirs).unwrap();
        git(&base, &["add", ".cursor/cli.json"]);
        git(&base, &["commit", "-q", "-m", "cfg", "--no-verify"]);
        let err = install_cursor_permissions_with_home(&base, "read-only", None).err().unwrap();
        assert!(err.contains("tracked .cursor/cli.json left untouched"), "{err}");
        assert_eq!(std::fs::read_to_string(base.join(".cursor/cli.json")).unwrap(), theirs);
        assert!(!base.join(".cursor/orbitty-managed.json").exists());
        assert_eq!(git(&base, &["status", "--porcelain", "-uall"]).trim(), "");
        // Same from a subdirectory of the repo root? Not applicable: the
        // tracked file is the root's; a sub workspace has its own path.
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn untracked_cli_json_keeps_user_lists_across_presets() {
        let base = tmp("merge");
        let user = serde_json::json!({
            "version": 1,
            "editor": {"vimMode": true},
            "permissions": {"allow": ["Shell(npm)"], "deny": ["Shell(rm)", "Write(**)"]}
        });
        std::fs::create_dir_all(base.join(".cursor")).unwrap();
        std::fs::write(base.join(".cursor/cli.json"), user.to_string()).unwrap();
        install_cursor_permissions_with_home(&base, "read-only", None).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(base.join(".cursor/cli.json")).unwrap()).unwrap();
        let (allow, deny) = (arr(&v, "allow"), arr(&v, "deny"));
        assert_eq!(allow[0], "Shell(npm)");
        assert!(allow.contains(&"Shell(find)".to_string()));
        assert_eq!(&deny[..2], &["Shell(rm)".to_string(), "Write(**)".to_string()]);
        assert!(deny.contains(&"Shell(find:*-delete*)".to_string()));
        assert_eq!(deny.iter().filter(|d| *d == "Write(**)").count(), 1);
        assert_eq!(v["editor"]["vimMode"], true);

        // workspace-write: our read-only denies go, the user's stay —
        // including their own Write(**) deny.
        install_cursor_permissions_with_home(&base, "workspace-write", None).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(base.join(".cursor/cli.json")).unwrap()).unwrap();
        let (allow, deny) = (arr(&v, "allow"), arr(&v, "deny"));
        assert_eq!(deny, vec!["Shell(rm)".to_string(), "Write(**)".to_string()]);
        assert!(allow.contains(&"Shell(npm)".to_string()));
        assert!(allow.contains(&"Write(**)".to_string()));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn failed_launch_rolls_the_permission_file_back() {
        let base = tmp("undo");
        let undo = install_cursor_permissions_with_home(&base, "read-only", None).unwrap();
        assert!(project_cli_path(&base).is_file());
        undo.rollback();
        assert!(!base.join(".cursor").exists());

        std::fs::create_dir_all(base.join(".cursor")).unwrap();
        let mine = "{\"version\":1,\"permissions\":{\"allow\":[\"Shell(npm)\"]}}";
        std::fs::write(project_cli_path(&base), mine).unwrap();
        let undo = install_cursor_permissions_with_home(&base, "full-auto", None).unwrap();
        assert_ne!(std::fs::read_to_string(project_cli_path(&base)).unwrap(), mine);
        undo.rollback();
        assert_eq!(std::fs::read_to_string(project_cli_path(&base)).unwrap(), mine);
        assert!(!base.join(".cursor/orbitty-managed.json").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn refuses_home_and_global_cursor_dir() {
        let home = tmp("home");
        let sub = home.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let h = Some(home.as_path());
        assert!(cursor_perm_refusal(&home, h).is_some());
        assert!(cursor_perm_refusal(&sub.join(".."), h).is_some(), "$HOME/sub/.. must be $HOME");
        assert!(cursor_perm_refusal(&home.join("."), h).is_some());
        #[cfg(unix)]
        {
            let link = home.join("sub/homelink");
            std::os::unix::fs::symlink(&home, &link).unwrap();
            assert!(cursor_perm_refusal(&link, h).is_some(), "symlink to $HOME");
        }
        let gcur = home.join(".cursor/projects");
        std::fs::create_dir_all(&gcur).unwrap();
        assert!(cursor_perm_refusal(&home.join(".cursor"), h).is_some());
        assert!(cursor_perm_refusal(&gcur, h).is_some());
        assert!(cursor_perm_refusal(&sub, h).is_none());
        assert!(cursor_perm_refusal(Path::new("/"), h).is_some());
        let _ = std::fs::remove_dir_all(&home);
    }

    /// N4 decision: the main (non-linked) checkout — a temp `$HOME/orbitty`
    /// stands in for `~/orbitty` via the injected home — gets the preset,
    /// kept out of commits by its own info/exclude.
    #[test]
    fn main_checkout_gets_preset_and_stays_uncommitted() {
        let home = tmp("mainco");
        let orb = home.join("orbitty");
        std::fs::create_dir_all(&orb).unwrap();
        repo_with_commit(&orb);
        // Non-linked: git dir == common dir.
        assert_eq!(
            git(&orb, &["rev-parse", "--git-dir"]).trim(),
            git(&orb, &["rev-parse", "--git-common-dir"]).trim()
        );
        let h = Some(home.as_path());
        assert!(cursor_perm_refusal(&orb, h).is_none());
        install_cursor_permissions_with_home(&orb, "read-only", h).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(project_cli_path(&orb)).unwrap()).unwrap();
        assert!(arr(&v, "allow").contains(&"Shell(find)".to_string()));
        assert!(arr(&v, "deny").contains(&"Write(**)".to_string()));
        let exclude = std::fs::read_to_string(orb.join(".git/info/exclude")).unwrap();
        assert!(exclude.lines().any(|l| l == "**/.cursor/cli.json"));
        assert_eq!(git(&orb, &["status", "--porcelain", "-uall"]).trim(), "");
        git(&orb, &["check-ignore", "-q", ".cursor/cli.json"]);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn sq_escapes_single_quotes() {
        assert_eq!(sq("a b"), "'a b'");
        assert_eq!(sq("it's"), "'it'\\''s'");
        assert_eq!(sq(""), "''");
    }

    fn find_pwned(dir: &Path, hits: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            // The hostile names contain "PWNED" mid-name; a marker that
            // actually ran is a file NAMED PWNED_*.
            if p.file_name().unwrap().to_string_lossy().starts_with("PWNED") {
                hits.push(p.clone());
            }
            if p.is_dir() && !p.is_symlink() {
                find_pwned(&p, hits);
            }
        }
    }

    /// B2: hostile dir / model / query / pid / script / bin / ctl / sock
    /// values. `zsh -n` must parse the body, and running it must execute
    /// nothing from any value, pass each value through verbatim, and still
    /// run the cleanup trap.
    #[test]
    fn command_body_survives_hostile_values_under_zsh() {
        if !Path::new("/bin/zsh").exists() {
            eprintln!("no /bin/zsh; skipped");
            return;
        }
        use std::os::unix::fs::PermissionsExt;
        let base = tmp("zsh");
        let evil = "x 'q' \"dq\" $(touch PWNED_S) `touch PWNED_B`; touch PWNED_C\nnl & | < >";
        let dir = base.join(format!("dir {evil}"));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = |name: String, body: &str| {
            let p = base.join(name);
            std::fs::write(&p, body).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p
        };
        let rec = "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\n";
        let bin = exe(format!("bin {evil}"), rec);
        let ctl = exe(format!("ctl {evil}"), rec);
        let sock = base.join(format!("sock {evil}"));
        let query = base.join(format!("q {evil}.md"));
        std::fs::write(&query, "hello $(touch PWNED_PROMPT)").unwrap();
        let pidf = base.join(format!("pid {evil}.pid"));
        let script = base.join(format!("run {evil}.command"));
        let model = format!("m {evil}");
        let exec = crate::agents::cursor_exec_line(bin.to_str().unwrap(), &model, "read-only");
        let body = agent_command_body(
            &query,
            &script,
            &pidf,
            &exec,
            &ctl,
            &sock,
            Some(dir.to_str().unwrap()),
        );
        std::fs::write(&script, &body).unwrap();
        let n = std::process::Command::new("/bin/zsh").arg("-n").arg(&script).output().unwrap();
        assert!(n.status.success(), "zsh -n: {}\n{body}", String::from_utf8_lossy(&n.stderr));
        let run = std::process::Command::new("/bin/zsh")
            .arg(&script)
            .current_dir(&base)
            .output()
            .unwrap();
        assert!(run.status.success(), "run: {}", String::from_utf8_lossy(&run.stderr));
        let mut hits = Vec::new();
        find_pwned(&base, &mut hits);
        assert!(hits.is_empty(), "executed: {hits:?}");
        let bin_args = std::fs::read_to_string(format!("{}.args", bin.display())).unwrap();
        assert_eq!(
            bin_args,
            format!("--model={model}\nhello $(touch PWNED_PROMPT)\n")
        );
        let ctl_args = std::fs::read_to_string(format!("{}.args", ctl.display())).unwrap();
        assert_eq!(
            ctl_args,
            format!("--sock\n{}\ncompanion\nhack\n--\noff\n", sock.display())
        );
        assert!(!pidf.exists(), "cleanup removed the pid file");
        assert!(!script.exists(), "cleanup removed the script");
        let _ = std::fs::remove_dir_all(&base);
    }

}
