#![allow(unused_imports)]
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};
#[allow(unused_imports)]
use super::hermes::*;
#[allow(unused_imports)]
use super::allowlist::*;
#[allow(unused_imports)]
use super::plug::*;
#[allow(unused_imports)]
use super::mouth_cli::*;
#[allow(unused_imports)]
use super::commands::*;
#[allow(unused_imports)]
use super::intercept::*;
#[allow(unused_imports)]
use super::send::*;
#[allow(unused_imports)]
use super::action::*;
#[allow(unused_imports)]
use super::paths::*;
#[allow(unused_imports)]
use super::cursor_perm::*;
#[allow(unused_imports)]
use super::launch_agent::*;
#[cfg(target_os = "macos")]
#[allow(unused_imports)]
use super::mac;

pub(crate) fn soul_file(app: &tauri::AppHandle, project_id: &str) -> Result<PathBuf, String> {
    let dir = app_data(app)?.join("souls").join(safe_id(project_id));
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir.join("soul.md"))
}

/// Template per the S5 lock: title + "What this planet is" + Journal.
pub(crate) fn soul_template(project_name: &str) -> String {
    format!("# {project_name}\n\n## What this planet is\n\n\n\n## Journal\n")
}

/// Create the template only when the file is missing; never clobber.
pub(crate) fn ensure_soul(
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

pub(crate) fn non_empty(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

/// Single-quote a value for safe embedding in a zsh .command script.
pub(crate) fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Write a tiny .command launcher and `open` it — a fresh Terminal window
/// running the line (same visible-new-shell trick as the cursor-agent
/// launch). Never the dead window's pty.
pub(crate) fn open_in_terminal(app: &tauri::AppHandle, key: &str, line: &str) -> Result<(), String> {
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

pub(crate) fn wake_sleeper_inner(app: &tauri::AppHandle, id: &str) -> Result<String, String> {
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

