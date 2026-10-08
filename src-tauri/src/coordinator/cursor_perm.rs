use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use super::hermes::{spawn_detached};
use super::paths::{bundled_orbittyctl, control_sock_path, ghostty_installed, open_terminal_argv, resolve_agent_terminal, sq};

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
pub(crate) fn cursor_perm_refusal(workspace: &Path, home: Option<&Path>) -> Option<String> {
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
pub(crate) fn managed_sidecar_path(workspace: &Path) -> PathBuf {
    workspace.join(".cursor").join("orbitty-managed.json")
}

/// Patterns in the repo's local exclude. `**/` so a subdirectory
/// workspace (`sub/.cursor/cli.json`) is covered too.
pub(crate) const CURSOR_EXCLUDES: [&str; 2] = ["**/.cursor/cli.json", "**/.cursor/orbitty-managed.json"];

/// `git -C <ws> <args>` stdout (trimmed) on success; `None` = not a repo /
/// git missing / failed.
pub(crate) fn git_out(workspace: &Path, args: &[&str]) -> Option<String> {
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
pub(crate) fn exclude_file(workspace: &Path) -> Option<PathBuf> {
    let raw = git_out(workspace, &["rev-parse", "--git-path", "info/exclude"])?;
    if raw.is_empty() {
        return None;
    }
    let p = PathBuf::from(raw);
    Some(if p.is_absolute() { p } else { workspace.join(p) })
}

/// `.cursor/cli.json` (relative to the workspace) is tracked in git.
pub(crate) fn cli_json_tracked(workspace: &Path) -> bool {
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
pub(crate) fn local_exclude(exclude: &Path) -> Result<(), String> {
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

pub(crate) fn str_list(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// Union-merge one list: the user's entries (file minus what we added
/// last time) stay in order, ours are appended. Returns (merged, added)
/// where `added` is ours that the user didn't already have.
pub(crate) fn union_list(file: &[String], ours_before: &[String], ours_now: &[&str]) -> (Vec<String>, Vec<String>) {
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

pub(crate) fn install_cursor_permissions_with_home(
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

pub(crate) fn open_agent_terminal(
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

/// A row value failed launch validation: log it, and hand back the
/// human-facing error (the launch is refused; nothing ran).
pub(crate) fn launch_refused(app: &tauri::AppHandle, planet: Option<&str>, harness: &str, why: &str) -> String {
    crate::event_log::append_best(
        app,
        "agent-launch-refused",
        None,
        planet,
        Some("agent"),
        true,
        &serde_json::json!({ "harness": harness, "reason": why }),
    );
    format!("{harness} launch refused: {why}")
}

