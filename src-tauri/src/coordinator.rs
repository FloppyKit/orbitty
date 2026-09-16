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
    let mut st = crate::state::load_state(&app)?;
    st.companion.switch_mode(&m);
    crate::state::save_state(&app, &st)?;
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
    let mut st = crate::state::load_state(&app)?;
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
    crate::state::save_state(&app, &st)?;
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
    let mut st = crate::state::load_state(&app)?;
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
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    Ok(serde_json::json!({
        "live": st.companion.hack_live,
        "who": st.companion.hack_who,
        "projectId": st.companion.hack_project_id,
    }))
}

fn run_is_seated(r: &crate::state::AgentRun) -> bool {
    !r.agent_shortcut_id.trim().is_empty()
        || r.herdr
            .as_ref()
            .is_some_and(|h| !h.agent_name.trim().is_empty())
}

/// Pickup lights the dock before the harness is up; a dead process
/// that never ran its off-hook leaves hackLive stuck. If the flag is
/// on and no seated run is live, turn it off.
pub fn heal_stuck_hack(app: &tauri::AppHandle) {
    let Ok(st) = crate::state::load_state(app) else {
        return;
    };
    if !st.companion.hack_live {
        return;
    }
    let live = st.agent_runs.iter().any(|r| {
        run_is_seated(r)
            && matches!(r.state.as_str(), "working" | "starting" | "blocked")
    });
    if live {
        return;
    }
    let _ = set_companion_hack(app.clone(), false, None, None);
}

/// Persist Talk source (`system` | `hotkey`).
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
        "all" => {
            let v = crate::state::normalize_companion_view(&view, "full");
            st.companion.view_collapsed = v.clone();
            st.companion.view_expanded = v.clone();
            v
        }
        other => return Err(format!("bad companion surface: {other:?}")),
    };
    crate::state::save_state(&app, &st)?;
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
    let mut st = crate::state::load_state(&app)?;
    if st.companion.sprite != enabled {
        st.companion.sprite = enabled;
        crate::state::save_state(&app, &st)?;
        let _ = app.emit("state:changed", ());
    }
    if enabled {
        crate::panes::unhug_companion(&app);
    } else if !st.companion.expanded {
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
    let mut st = crate::state::load_state(&app)?;
    st.companion.speak_replies = enabled;
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Companion `say` volume 0.0–1.0. Volume 0 mutes the next speak.
#[tauri::command]
pub fn set_companion_speak_volume(app: tauri::AppHandle, volume: f32) -> Result<(), String> {
    let volume = volume.clamp(0.0, 1.0);
    let mut st = crate::state::load_state(&app)?;
    st.companion.speak_volume = volume;
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
    Ok(())
}

/// Draw / hide the CC + transcript. Composer stays.
#[tauri::command]
pub fn set_companion_captions(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let mut st = crate::state::load_state(&app)?;
    st.companion.captions = enabled;
    crate::state::save_state(&app, &st)?;
    let _ = app.emit("state:changed", ());
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
    if let Ok(mut st) = crate::state::load_state(app) {
        st.companion.last_user_line = line.to_string();
        let you_already_saved = st
            .companion
            .turns
            .last()
            .is_some_and(|t| t.role == "you" && t.text == line);
        if !you_already_saved {
            st.companion.push_turn("you", line);
        }
        let _ = crate::state::save_state(app, &st);
        let _ = app.emit("state:changed", ());
    }
}

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
                st.companion.push_turn("you", you);
            }
        }
        st.companion.last_bot_line = them.to_string();
        st.companion.push_turn("them", them);
        let _ = crate::state::save_state(app, &st);
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

fn wrap_with_pocket(app: &tauri::AppHandle, line: &str) -> String {
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
    // The conductor — a slim local fast ack in
    // front of the mouth (Settings toggle; off = today's path exactly).
    // Runs on the RAW line (pocket/desk wraps are for the mouth, not the
    // triage). A chip send is always for the AI, so the conductor stands
    // down while one rides. `chat` splashes the ack over the wait mark
    // (companion:ack — never a saved turn) and the mouth below lands the
    // real reply in the same bubble; any other verb's ack IS the
    // them-row — no mouth spend. Propose-don't-save lives inside: only
    // Matt's yes reaches action add.
    #[cfg(unix)]
    if attach.is_none() {
        match crate::mouth_ack::conduct(&app, &line).await {
            Some(crate::mouth_ack::Step::Final(them)) => {
                persist_action_intercept(&app, None, &them);
                return Ok(them);
            }
            Some(crate::mouth_ack::Step::Chat(ack)) => {
                let _ = app.emit(
                    "companion:ack",
                    serde_json::json!({ "line": ack, "verb": "chat" }),
                );
            }
            None => {}
        }
    }
    // GO-MH-2-APP: pocket inject AFTER action-intercept (Actions above ran
    // on the raw line). Every mouth gets the wrapped prompt; persist +
    // event log below keep the raw `line`.
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
            crate::mouth_hop::send_with_hops(&app, &line, &prompt, vision_path.as_deref()).await?
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
    if let Ok(mut st) = crate::state::load_state(&app) {
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
        crate::board::kanban_add_as(app, &title, p, "doing", "action", None)
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
fn open_agent_terminal(query: &Path, exec_line: &str) -> Result<(), String> {
    let script = query.with_extension("command");
    let ctl = std::env::var("HOME")
        .map(|h| format!("{h}/.local/bin/orbittyctl"))
        .unwrap_or_else(|_| "orbittyctl".into());
    let body = format!(
        "#!/bin/zsh\nprompt=\"$(cat \"{q}\")\"\ntrap 'exit' HUP\ntrap '\"{ctl}\" companion hack -- off >/dev/null 2>&1 || true' EXIT\n{exec_line}\n",
        q = query.display(),
    );
    std::fs::write(&script, &body)
        .map_err(|e| format!("write {}: {e}", script.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
    }
    if spawn_detached("open", &[script.to_string_lossy().as_ref()]) {
        Ok(())
    } else {
        Err(format!(
            "Couldn't open Terminal launcher. Query file ready: {}",
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
    let out: Result<String, String> = (|| {
    match shortcut.harness.trim().to_ascii_lowercase().as_str() {
        "cursor" => {
            let Some(bin) = find_on_path("cursor-agent") else {
                return Err(
                    "cursor-agent not on PATH — install the Cursor CLI or pick another harness."
                        .to_string(),
                );
            };
            // cursor-agent's chat is a TTY TUI: open it in Terminal via a
            // .command launcher with the query file as the initial prompt.
            // No `exec` — the launcher's EXIT trap owns the LED off.
            let model_arg = shortcut
                .model
                .as_deref()
                .map(str::trim)
                .filter(|m| !m.is_empty())
                .map(|m| format!(" --model \"{}\"", m.replace('"', "")))
                .unwrap_or_default();
            open_agent_terminal(
                &query,
                &format!("\"{}\"{model_arg} \"$prompt\"", bin.display()),
            )?;
            Ok(format!(
                "Opened cursor-agent in Terminal. Query: {}.{skip_note}",
                query.display()
            ))
        }
        // Goose and grok are named doors with the
        // same visible Terminal handoff as cursor — never a silent hermes.
        "goose" => {
            let Some(bin) = find_on_path("goose") else {
                return Err(
                    "`goose` not on PATH — install the Goose CLI or pick another harness."
                        .to_string(),
                );
            };
            // Same full-tools builtin set as the companion goose mouth
            // (BLOCK-MH-4): the agent can act, not just talk.
            open_agent_terminal(
                &query,
                &format!(
                    "\"{}\" run --with-builtin=developer,computercontroller -t \"$prompt\"",
                    bin.display()
                ),
            )?;
            Ok(format!(
                "Opened goose run in Terminal. Query: {}.{skip_note}",
                query.display()
            ))
        }
        "grok" => {
            // The herdr branch of this door —
            // pane first, agent second, when herdr is present + up.
            // Skip falls through to the Terminal launcher unchanged
            // (one skip line was already logged; never a dual-launch).
            match crate::herdr::branch(&app, &shortcut, project_name.as_deref(), &titles, Some(query.as_path())) {
                crate::herdr::Branch::Skip => {}
                crate::herdr::Branch::Up(m) | crate::herdr::Branch::Blocked(m) => {
                    return Ok(format!("{m} Query: {}.{skip_note}", query.display()));
                }
            }
            let Some(bin) = find_grok() else {
                return Err(
                    "`grok` not on PATH — install the Grok CLI or pick another harness."
                        .to_string(),
                );
            };
            open_agent_terminal(&query, &format!("\"{}\" -p \"$prompt\"", bin.display()))?;
            Ok(format!(
                "Opened grok in Terminal. Query: {}.{skip_note}",
                query.display()
            ))
        }
        // Hermes door: hermes -p <profile> chat with the query file.
        // no Terminal
        // launcher here, so a waiter thread owns the off — the dock LED
        // goes dark when the hermes process exits (the Terminal doors do
        // it through the .command EXIT trap). Detached stdio, never a
        // poll.
        "hermes" => {
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
                    let app2 = app.clone();
                    std::thread::spawn(move || {
                        let _ = child.wait();
                        let _ = set_companion_hack(app2, false, None, None);
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
            crate::herdr::Branch::Up(m) | crate::herdr::Branch::Blocked(m) => {
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
