//! End the writer when a seated card is reset to start (doing → todo).
//! Not Done — Done is the far-rim walk-off. Best-effort: never fail the
//! save that dropped the run row.

use std::fs;
use std::path::Path;
use std::process::Command;

use tauri::Manager;

use crate::state::{AgentRunHerdr, PersistedState};

/// Stop the seat that started the run (core: kill then rm; tui: pane
/// and workspace close) and the Terminal harness pidfile for each
/// dropped card, then heal the dock LED from whatever is still seated.
pub fn stop_resets(app: &tauri::AppHandle, before: &PersistedState, drops: &[String]) {
    if drops.is_empty() {
        return;
    }
    for card_id in drops {
        let run = before.agent_runs.iter().find(|r| r.card_id == *card_id);
        if let Some(h) = run.and_then(|r| r.herdr.as_ref()) {
            herdr_stop(h);
        }
        let shortcut_id = before
            .kanban_cards
            .iter()
            .find(|c| c.id == *card_id)
            .and_then(|c| c.assigned_agent_id.as_deref())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                run.map(|r| r.agent_shortcut_id.as_str())
                    .filter(|s| !s.is_empty())
            });
        let task_id = before
            .kanban_cards
            .iter()
            .find(|c| c.id == *card_id)
            .and_then(|c| c.task_id.as_deref());
        if let Some(sid) = shortcut_id {
            terminal_stop(app, sid, card_id, task_id);
        }
    }
    // LED heal is after the run row is dropped (attach_on_moves save).
}

pub(crate) fn herdr_stop(h: &AgentRunHerdr) {
    crate::herdr::client::stop(h);
}

/// End exactly this card's Terminal run. Launchers are per run now
/// (`agent-<shortcut>-<task|card>.pid`); the task's recorded path wins,
/// then the card-tagged path (no task), then the legacy shared
/// `agent-<shortcut>.pid` for runs launched before per-run files.
fn terminal_stop(app: &tauri::AppHandle, shortcut_id: &str, card_id: &str, task_id: Option<&str>) {
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    let ctx = dir.join("context");
    let recorded = task_id.and_then(|t| crate::task::recorded_pid_file(app, t));
    for pidf in stop_candidates(&ctx, shortcut_id, card_id, task_id, recorded) {
        if pidf.is_file() {
            kill_pidfile(&pidf);
            return;
        }
    }
}

fn stop_candidates(
    ctx: &Path,
    shortcut_id: &str,
    card_id: &str,
    task_id: Option<&str>,
    recorded: Option<std::path::PathBuf>,
) -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    // A task with a recorded per-run file is new-era: never fall back to
    // the legacy shared file (it could belong to another run).
    let legacy_ok = recorded.is_none();
    v.extend(recorded);
    if let Some(t) = task_id {
        v.push(crate::coordinator::terminal_run_paths(ctx, shortcut_id, t).1);
    }
    v.push(crate::coordinator::terminal_run_paths(ctx, shortcut_id, card_id).1);
    if legacy_ok {
        v.push(ctx.join(format!("agent-{}.pid", crate::coordinator::safe_id(shortcut_id))));
    }
    v
}

fn kill_pidfile(pidf: &Path) {
    let Ok(raw) = fs::read_to_string(pidf) else {
        return;
    };
    let pid = raw.trim().parse::<i32>().ok().filter(|p| *p > 1);
    let _ = fs::remove_file(pidf);
    let Some(pid) = pid else {
        return;
    };
    kill_term(pid);
}

fn kill_term(pid: i32) {
    // Process group first (the .command zsh + grok/cursor/goose child).
    let _ = Command::new("kill")
        .args(["-TERM", &format!("-{pid}")])
        .status();
    let _ = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn alive(pid: i32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[test]
    fn stop_prefers_this_runs_pid_file_then_legacy() {
        let ctx = Path::new("/ctx");
        // Recorded per-run file: no legacy shared fallback.
        let v = stop_candidates(ctx, "s1", "c9", Some("t-7"), Some(ctx.join("rec.pid")));
        assert_eq!(
            v,
            vec![
                ctx.join("rec.pid"),
                ctx.join("agent-s1-t-7.pid"),
                ctx.join("agent-s1-c9.pid"),
            ]
        );
        // Old task with no recorded file: legacy last.
        let v = stop_candidates(ctx, "s1", "c9", Some("t-7"), None);
        assert_eq!(
            v,
            vec![
                ctx.join("agent-s1-t-7.pid"),
                ctx.join("agent-s1-c9.pid"),
                ctx.join("agent-s1.pid"),
            ]
        );
        let v = stop_candidates(ctx, "s1", "c9", None, None);
        assert_eq!(v, vec![ctx.join("agent-s1-c9.pid"), ctx.join("agent-s1.pid")]);
    }

    #[test]
    fn kill_pidfile_ends_sleep() {
        let dir = std::env::temp_dir().join(format!(
            "orbitty-stop-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let pidf = dir.join("agent-test.pid");
        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(std::process::Stdio::null())
            .spawn()
            .expect("spawn sleep");
        let pid = child.id() as i32;
        fs::write(&pidf, format!("{pid}\n")).unwrap();
        assert!(alive(pid));
        kill_pidfile(&pidf);
        thread::sleep(Duration::from_millis(200));
        let _ = child.try_wait();
        assert!(!alive(pid), "sleep {pid} should be dead");
        assert!(!pidf.exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
