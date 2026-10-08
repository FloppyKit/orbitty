use std::path::{ PathBuf };
use std::process::{Command, Stdio};

// Actions lane: user-authored shell shortcuts

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub ok: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub(crate) fn clip_output(bytes: &[u8]) -> String {
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
pub(crate) fn action_dir(cwd: Option<&str>) -> PathBuf {
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
    // CheatSheet click (command only, no name): a thin row still goes to
    // companion_thin, never `/bin/zsh -lc orbittyctl companion thin`.
    crate::actions::dispatch_action(&app, "", command, cwd)
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

pub(crate) fn run_action_inner(command: String, cwd: Option<String>) -> Result<ActionResult, String> {
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

