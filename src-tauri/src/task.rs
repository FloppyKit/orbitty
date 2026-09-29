//! Task boundary: APFS clonefile snapshot of a planet workspace root
//! before/after an agent or action run. Undo restores `before`. Void
//! never snapshots. `cp -R` without `-c` is forbidden.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::event_log;
use crate::state::KanbanCard;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct Meta {
    task_id: String,
    project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    card_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    root: Option<String>,
    started_at: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ended_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    snapshot: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    undone_at: Option<f64>,
    /// Liveness hints for the process running this task, so boot can
    /// tell a still-running (or cleanly exited) agent from a dead one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    /// Terminal `.command` launcher pid file: written with `$$` at start,
    /// removed by the EXIT trap. Missing = the script exited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pid_file: Option<String>,
}

/// What launch_agent learned about the process it handed off to.
#[derive(Default, Clone, Debug)]
pub struct LiveHint {
    pub pid: Option<u32>,
    pub pid_file: Option<PathBuf>,
}

/// Record liveness hints on an open task (best-effort).
pub fn note_live(app: &AppHandle, task_id: &str, hint: &LiveHint) {
    let Ok(root) = tasks_root(app) else {
        return;
    };
    let dir = root.join(task_id);
    let Some(mut meta) = read_meta(&dir) else {
        return;
    };
    if meta.ended_at.is_some() {
        return;
    }
    meta.pid = hint.pid;
    meta.pid_file = hint.pid_file.as_ref().map(|p| p.to_string_lossy().into_owned());
    let _ = write_meta(&dir, &meta);
}

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn new_task_id() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    format!("t-{ms:x}")
}

fn tasks_root(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?
        .join("tasks");
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir tasks: {e}"))?;
    Ok(dir)
}

fn is_void(name: &str, id: &str) -> bool {
    name.trim().eq_ignore_ascii_case("void") || id.trim().eq_ignore_ascii_case("void")
}

fn clone_c(src: &Path, dest: &Path) -> Result<(), String> {
    if dest.exists() {
        let _ = fs::remove_dir_all(dest);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let st = Command::new("/bin/cp")
        .args(["-cR", &src.to_string_lossy(), &dest.to_string_lossy()])
        .status()
        .map_err(|e| format!("cp -cR: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("cp -cR failed ({st})"))
    }
}

fn write_meta(dir: &Path, meta: &Meta) -> Result<(), String> {
    let p = dir.join("meta.json");
    let s = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    fs::write(&p, s).map_err(|e| format!("write meta: {e}"))
}

fn read_meta(dir: &Path) -> Option<Meta> {
    let s = fs::read_to_string(dir.join("meta.json")).ok()?;
    serde_json::from_str(&s).ok()
}

/// A card edit from native must bump `lastTouchedAt` strictly past the
/// copy any window already holds: `merge_window_save` breaks a tie in the
/// window's favor, so an unbumped edit (taskId / taskReason) was wiped by
/// the next stale window save.
fn touch(card: &mut KanbanCard) {
    card.last_touched_at = now_secs().max(card.last_touched_at + 0.001);
}

fn stamp_task_id(card: &mut KanbanCard, task_id: &str) {
    card.task_id = Some(task_id.into());
    touch(card);
}

fn stamp_task_reason(card: &mut KanbanCard, reason: &str) {
    card.task_reason = Some(reason.into());
    touch(card);
}

fn stamp_card(app: &AppHandle, card_id: &str, task_id: &str) {
    let Ok(before) = crate::state::load_state(app) else {
        return;
    };
    let mut after = before.clone();
    let Some(c) = after.kanban_cards.iter_mut().find(|c| c.id == card_id) else {
        return;
    };
    stamp_task_id(c, task_id);
    let _ = crate::board::save_with_events(app, &before, &after);
}

/// Start a task: clone `before` if the planet has a real workspace root.
pub fn start(app: &AppHandle, project_id: &str, card_id: Option<&str>) -> Option<String> {
    let Ok(st) = crate::state::load_state(app) else {
        return None;
    };
    let project = st.projects.iter().find(|p| p.id == project_id)?;
    if is_void(&project.name, &project.id) {
        return None;
    }
    let task_id = new_task_id();
    let Ok(root_dir) = tasks_root(app) else {
        return None;
    };
    let dir = root_dir.join(&task_id);
    let _ = fs::create_dir_all(&dir);
    let mut snapshot = "skipped".to_string();
    let root = project
        .workspace_root
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && Path::new(s).is_dir())
        .map(str::to_string);
    if let Some(ref r) = root {
        if clone_c(Path::new(r), &dir.join("before")).is_ok() {
            snapshot = "cloned".into();
        }
    }
    let meta = Meta {
        task_id: task_id.clone(),
        project_id: project_id.into(),
        card_id: card_id.map(str::to_string),
        root,
        started_at: now_secs(),
        ended_at: None,
        reason: None,
        snapshot,
        undone_at: None,
        pid: None,
        pid_file: None,
    };
    let _ = write_meta(&dir, &meta);
    if let Some(cid) = card_id {
        stamp_card(app, cid, &task_id);
    }
    event_log::append_best(
        app,
        "task-start",
        None,
        Some(project_id),
        Some("agent"),
        true,
        &serde_json::json!({ "taskId": task_id, "cardId": card_id, "snapshot": meta.snapshot }),
    );
    Some(task_id)
}

pub fn end(app: &AppHandle, task_id: &str, reason: &str) {
    let Ok(root_dir) = tasks_root(app) else {
        return;
    };
    let dir = root_dir.join(task_id);
    let Some(mut meta) = read_meta(&dir) else {
        return;
    };
    if meta.ended_at.is_some() {
        return;
    }
    if meta.snapshot == "cloned" {
        if let Some(ref r) = meta.root {
            if Path::new(r).is_dir() {
                let _ = clone_c(Path::new(r), &dir.join("after"));
            }
        }
    }
    meta.ended_at = Some(now_secs());
    meta.reason = Some(reason.into());
    let _ = write_meta(&dir, &meta);
    if let Some(cid) = meta.card_id.as_deref() {
        let Ok(before) = crate::state::load_state(app) else {
            return;
        };
        let mut after = before.clone();
        if let Some(c) = after.kanban_cards.iter_mut().find(|c| c.id == cid) {
            stamp_task_reason(c, reason);
        }
        let _ = crate::board::save_with_events(app, &before, &after);
    }
    event_log::append_best(
        app,
        "task-end",
        None,
        Some(&meta.project_id),
        Some("agent"),
        true,
        &serde_json::json!({ "taskId": task_id, "reason": reason }),
    );
}

pub fn end_card(app: &AppHandle, card: &KanbanCard, reason: &str) {
    if let Some(id) = card.task_id.as_deref() {
        end(app, id, reason);
    }
}

/// Restore `before` onto the planet root. Hidden if no clone.
pub fn undo(app: &AppHandle, card_id: &str) -> Result<(), String> {
    let st = crate::state::load_state(app)?;
    let task_id = st
        .kanban_cards
        .iter()
        .find(|c| c.id == card_id)
        .and_then(|c| c.task_id.clone())
        .or_else(|| {
            if card_id.starts_with("t-") {
                Some(card_id.to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| format!("unknown card {card_id}"))?;
    let dir = tasks_root(app)?.join(&task_id);
    let before_path = dir.join("before");
    if !before_path.is_dir() {
        return Err("no before snapshot".into());
    }
    let mut meta = read_meta(&dir).ok_or_else(|| "missing task meta".to_string())?;
    let root = meta
        .root
        .clone()
        .filter(|r| Path::new(r).is_dir())
        .ok_or_else(|| "workspace root missing".to_string())?;
    let root_path = PathBuf::from(&root);
    let parent = root_path
        .parent()
        .ok_or_else(|| "root has no parent".to_string())?;
    let name = root_path
        .file_name()
        .ok_or_else(|| "root has no name".to_string())?;
    let bak = parent.join(format!(
        ".{}-orbitty-undo-{}",
        name.to_string_lossy(),
        now_secs() as u64
    ));
    let tmp = parent.join(format!(".{}-orbitty-new", name.to_string_lossy()));
    clone_c(&before_path, &tmp)?;
    fs::rename(&root_path, &bak).map_err(|e| format!("park live root: {e}"))?;
    if let Err(e) = fs::rename(&tmp, &root_path) {
        let _ = fs::rename(&bak, &root_path);
        return Err(format!("swap restore: {e}"));
    }
    let _ = fs::remove_dir_all(&bak);
    meta.undone_at = Some(now_secs());
    let _ = write_meta(&dir, &meta);
    event_log::append_best(
        app,
        "task-undo",
        None,
        Some(&meta.project_id),
        Some("user"),
        true,
        &serde_json::json!({ "taskId": task_id, "cardId": card_id }),
    );
    Ok(())
}

pub fn has_before(app: &AppHandle, card_id: &str) -> bool {
    let Ok(st) = crate::state::load_state(app) else {
        return false;
    };
    let Some(card) = st.kanban_cards.iter().find(|c| c.id == card_id) else {
        return false;
    };
    let Some(tid) = card.task_id.as_deref() else {
        return false;
    };
    tasks_root(app)
        .map(|r| r.join(tid).join("before").is_dir())
        .unwrap_or(false)
}

pub fn set_workspace_root(app: &AppHandle, project_id: &str, path: &str) -> Result<(), String> {
    let before = crate::state::load_state(app)?;
    let mut after = before.clone();
    let Some(p) = after.projects.iter_mut().find(|p| p.id == project_id) else {
        return Err(format!("unknown project {project_id}"));
    };
    if is_void(&p.name, &p.id) {
        p.workspace_root = None;
    } else {
        let t = path.trim();
        p.workspace_root = if t.is_empty() { None } else { Some(t.into()) };
    }
    crate::board::save_with_events(app, &before, &after)
}

/// Is this pid a live process? kill(pid, 0): 0 or EPERM = exists.
#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }
    if unsafe { kill(pid as i32, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(1) // EPERM
}

#[cfg(not(unix))]
fn pid_alive(_pid: u32) -> bool {
    false
}

/// Terminal launcher pid file → Some(alive?) ; None = no file (the
/// script's EXIT trap removed it, i.e. it exited).
fn pid_file_alive(path: &Path) -> Option<bool> {
    let s = fs::read_to_string(path).ok()?;
    Some(s.trim().parse::<u32>().map(pid_alive).unwrap_or(false))
}

/// Everything boot learned about one open (no endedAt) task.
#[derive(Default, Debug, Clone, Copy)]
struct Probe {
    /// Recorded child pid: Some(alive?).
    pid: Option<bool>,
    /// Launcher pid file: None = no file known; Some(None) = known but
    /// gone (clean exit); Some(Some(alive?)) = file present.
    pid_file: Option<Option<bool>>,
    /// herdr pane agent for this card: Some(alive?).
    herdr: Option<bool>,
    /// A probe could not answer (herdr timeout / error). Maybe-alive:
    /// leave the task open and let the next boot look again.
    unknown: bool,
    /// The task's card is still in Doing.
    card_doing: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// Process still running — leave the task open, no Needed.
    Alive,
    /// Process gone but the card already left Doing — close quietly.
    Exited,
    /// Process gone and the card is stuck in Doing — the real "died".
    Died,
}

/// Only call a run dead when every known liveness signal says gone.
/// An unanswered probe is never evidence of death.
fn classify(p: &Probe) -> Verdict {
    let alive = p.pid == Some(true)
        || p.pid_file == Some(Some(true))
        || p.herdr == Some(true);
    if alive || p.unknown {
        Verdict::Alive
    } else if p.card_doing {
        Verdict::Died
    } else {
        Verdict::Exited
    }
}

fn probe(meta: &Meta, st: Option<&crate::state::PersistedState>, context_dir: Option<&Path>) -> Probe {
    let card = meta
        .card_id
        .as_deref()
        .and_then(|cid| st.and_then(|s| s.kanban_cards.iter().find(|c| c.id == cid)));
    let card_doing = card.map(|c| c.status == "doing").unwrap_or(false);
    // Pid file: recorded path, else the per-shortcut launcher path
    // (context/agent-<shortcut>.pid) for tasks written before the hint.
    let pid_file = meta
        .pid_file
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| {
            let sc = card.and_then(|c| c.assigned_agent_id.as_deref())?;
            let dir = context_dir?;
            Some(dir.join(format!("agent-{}.pid", crate::coordinator::safe_id(sc))))
        })
        .map(|p| pid_file_alive(&p));
    // Only report a pid-file signal we can trust: a recorded path, or a
    // derived one that actually exists (legacy derive with no file says
    // nothing — that launcher may never have been a Terminal door).
    let pid_file = match (meta.pid_file.is_some(), pid_file) {
        (true, pf) => pf,
        (false, Some(Some(a))) => Some(Some(a)),
        _ => None,
    };
    // Some(None) = there is a herdr run but the probe couldn't answer.
    let herdr_probe: Option<Option<bool>> = meta.card_id.as_deref().and_then(|cid| {
        let run = st?.agent_runs.iter().find(|r| r.card_id == cid)?;
        let rec = run.herdr.as_ref()?;
        Some(crate::herdr::agent_alive(&rec.agent_name))
    });
    Probe {
        pid: meta.pid.map(pid_alive),
        pid_file,
        herdr: herdr_probe.flatten(),
        unknown: matches!(herdr_probe, Some(None)),
        card_doing,
    }
}

/// Orphans (no endedAt): only a run whose process is really gone while
/// its card is still Doing → Needed ("died"). A still-running agent is
/// left open; a gone one whose card already moved closes quietly. Sweep
/// clones older than 7 days. The liveness probes (pid, pid file, herdr
/// CLI) run on a worker thread so boot never waits on them.
pub fn boot(app: &AppHandle) {
    let Ok(root) = tasks_root(app) else {
        return;
    };
    let now = now_secs();
    let week = 7.0 * 86400.0;
    let Ok(rd) = fs::read_dir(&root) else {
        return;
    };
    let mut open: Vec<(PathBuf, Meta)> = Vec::new();
    for ent in rd.flatten() {
        let dir = ent.path();
        if !dir.is_dir() {
            continue;
        }
        let Some(meta) = read_meta(&dir) else {
            continue;
        };
        if meta.ended_at.is_none() {
            open.push((dir, meta));
        } else if let Some(end) = meta.ended_at {
            if now - end > week {
                let _ = fs::remove_dir_all(&dir);
            }
        }
    }
    if open.is_empty() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        // No readable state = can't tell Doing from Done: leave every
        // open task for the next boot rather than guess.
        let Ok(st) = crate::state::load_state(&app) else {
            return;
        };
        let st = Some(st);
        let context_dir = app
            .path()
            .app_data_dir()
            .ok()
            .map(|d| d.join("context"));
        for (dir, mut meta) in open {
            let p = probe(&meta, st.as_ref(), context_dir.as_deref());
            match classify(&p) {
                Verdict::Alive => {}
                Verdict::Exited => {
                    meta.ended_at = Some(now_secs());
                    meta.reason = Some("exited".into());
                    let _ = write_meta(&dir, &meta);
                }
                Verdict::Died => {
                    let why = format!(
                        "task={} card={} This run died — resume or drop.",
                        meta.task_id,
                        meta.card_id.as_deref().unwrap_or("-")
                    );
                    let _ = crate::board::needed_open(
                        &app,
                        "This run died — resume or drop?",
                        &meta.project_id,
                        Some(&why),
                    );
                    meta.ended_at = Some(now_secs());
                    meta.reason = Some("crashed".into());
                    let _ = write_meta(&dir, &meta);
                }
            }
        }
    });
}

pub fn project_for_cwd(app: &AppHandle, cwd: Option<&str>) -> Option<String> {
    let st = crate::state::load_state(app).ok()?;
    let cwd = cwd
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)?;
    st.projects
        .iter()
        .filter(|p| {
            !is_void(&p.name, &p.id)
                && p.workspace_root
                    .as_deref()
                    .map(str::trim)
                    .filter(|r| !r.is_empty())
                    .map(PathBuf::from)
                    .filter(|r| cwd.starts_with(r) || r.starts_with(&cwd))
                    .is_some()
        })
        .map(|p| p.id.clone())
        .next()
}
#[cfg(test)]
mod stamp_tests {
    use super::*;
    use crate::state::{apply_writer_delta, merge_window_save, PersistedState};

    fn card(id: &str, touched: f64) -> KanbanCard {
        let raw = format!(
            r#"{{"id":"{id}","title":"T","projectId":"p","status":"doing","createdAt":1.0,"lastTouchedAt":{touched},"goalAligned":true,"clinicalOrMaster":false,"pointsAwarded":0}}"#
        );
        serde_json::from_str(&raw).unwrap()
    }

    /// N1: a window loaded the card at lastTouchedAt=5.0 (pre-stamp).
    /// Native stamps taskId, then taskReason. The window then saves its
    /// stale copy with the SAME lastTouchedAt — the stamps must survive.
    #[test]
    fn stale_same_touch_window_save_keeps_task_id_and_reason() {
        let mut disk = PersistedState::default();
        disk.kanban_cards.push(card("c1", 5.0));
        let window = disk.clone(); // snapshot before any stamp

        // task::start → stamp_card (through the writer delta, as
        // save_with_events applies it under the lock).
        let before = disk.clone();
        let mut after = before.clone();
        stamp_task_id(&mut after.kanban_cards[0], "t-abc");
        apply_writer_delta(&mut disk, &before, &after);
        assert!(disk.kanban_cards[0].last_touched_at > 5.0);

        // task::end → taskReason.
        let before = disk.clone();
        let mut after = before.clone();
        stamp_task_reason(&mut after.kanban_cards[0], "done");
        apply_writer_delta(&mut disk, &before, &after);

        let (merged, _) = merge_window_save(&disk, &window);
        let c = merged.kanban_cards.iter().find(|c| c.id == "c1").unwrap();
        assert_eq!(c.task_id.as_deref(), Some("t-abc"));
        assert_eq!(c.task_reason.as_deref(), Some("done"));
    }

    #[test]
    fn touch_is_strictly_newer_even_with_a_future_stamp() {
        let mut c = card("c1", 5.0);
        touch(&mut c);
        assert!(c.last_touched_at > 5.0);
        // A clock behind the card's stamp still moves it forward.
        let future = now_secs() + 1000.0;
        let mut c = card("c2", future);
        touch(&mut c);
        assert!(c.last_touched_at > future);
    }
}

#[cfg(test)]
mod boot_liveness_tests {
    use super::*;

    fn p(pid: Option<bool>, pid_file: Option<Option<bool>>, herdr: Option<bool>, doing: bool) -> Probe {
        Probe { pid, pid_file, herdr, unknown: false, card_doing: doing }
    }

    #[test]
    fn probe_error_with_card_in_doing_is_never_died() {
        // herdr timed out / errored: unknown, even with every other
        // signal saying gone and the card still in Doing.
        let mut q = p(Some(false), Some(None), None, true);
        q.unknown = true;
        assert_eq!(classify(&q), Verdict::Alive);
        let mut q = p(None, None, None, true);
        q.unknown = true;
        assert_eq!(classify(&q), Verdict::Alive);
    }

    #[test]
    fn live_process_is_never_died() {
        assert_eq!(classify(&p(Some(true), None, None, true)), Verdict::Alive);
        assert_eq!(classify(&p(None, Some(Some(true)), None, true)), Verdict::Alive);
        assert_eq!(classify(&p(None, None, Some(true), true)), Verdict::Alive);
        // Any one live signal wins over dead ones.
        assert_eq!(classify(&p(Some(false), Some(None), Some(true), true)), Verdict::Alive);
    }

    #[test]
    fn gone_with_card_in_doing_is_died() {
        assert_eq!(classify(&p(Some(false), None, None, true)), Verdict::Died);
        assert_eq!(classify(&p(None, Some(Some(false)), None, true)), Verdict::Died);
        assert_eq!(classify(&p(None, Some(None), None, true)), Verdict::Died);
        assert_eq!(classify(&p(None, None, Some(false), true)), Verdict::Died);
        // No hint at all (synchronous action run the app itself owned).
        assert_eq!(classify(&p(None, None, None, true)), Verdict::Died);
    }

    #[test]
    fn gone_with_card_moved_on_closes_quietly() {
        assert_eq!(classify(&p(Some(false), None, None, false)), Verdict::Exited);
        assert_eq!(classify(&p(None, Some(None), None, false)), Verdict::Exited);
        assert_eq!(classify(&p(None, None, None, false)), Verdict::Exited);
    }

    #[test]
    fn pid_alive_self_and_dead() {
        assert!(pid_alive(std::process::id()));
        assert!(!pid_alive(0));
        // A pid far above the macOS pid_max (99998) is never a process.
        assert!(!pid_alive(4_000_000));
    }

    #[test]
    fn pid_file_states() {
        let dir = std::env::temp_dir().join(format!("orbitty-pidf-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let f = dir.join("agent-x.pid");
        let _ = fs::remove_file(&f);
        assert_eq!(pid_file_alive(&f), None);
        fs::write(&f, format!("{}\n", std::process::id())).unwrap();
        assert_eq!(pid_file_alive(&f), Some(true));
        fs::write(&f, "4000000\n").unwrap();
        assert_eq!(pid_file_alive(&f), Some(false));
        let _ = fs::remove_dir_all(&dir);
    }
}
