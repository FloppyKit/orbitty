//! GO-ORBITTY-HERDR-ADAPTER — the Herdr branch of the ONE spawn hook
//! (`coordinator::launch_agent`; doing-fire / pickup funnel there).
//! Herdr is first-class IF PRESENT: `herdr` on PATH + its server up →
//! the run gets a herdr workspace + pane (`workspace create --cwd
//! <planet cwd> --label <planet>` then `agent start <name> --kind
//! grok|opencode --pane <root>`) and the doing-run row carries
//! `agentRun.herdr`. Missing / server down → ONE skip line (the install
//! URL when herdr itself is absent) and the caller's old launcher
//! (Goose/Grok Terminal, `cursor-go`) still fires; the DOING-WALKER row
//! still mints. Never a second kernel, never a dual-launch — the cursor
//! harness never enters this branch.
//!
//! `agent_not_ready` / a login wall on start → the row goes `blocked`
//! (the walker stands the near rim) and STOPS: no retry loop — the
//! human looks at the pane. The watch (`agent wait` / `agent get`) only
//! paints walker state (blocked ↔ working); `done` stays owned by the
//! card's doing→done drag (DOING-WALKER law), and the watch owns the
//! dock-LED off for the run it launched (the Terminal doors' EXIT-trap
//! job). OpenCode is detect-only: kind `opencode` is legal only with
//! `opencode` on PATH — never prompt-installed, never `opencode serve`
//! as the mouth, never sub-agent fans as planet children.
//!
//! CLI JSON in / JSON out only: envelopes `{"id", "result"|"error"}`
//! (`herdr api schema`); an error envelope or unparseable answer is an
//! honest skip/block, never a guess. Locks:
//! `~/.buzz/PLANS/ORBITTY_HERDR_PATH_LOCK_v0.md` +
//! `~/Downloads/ORBITTY_HERDR_RUNNER_v0.md`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::state::{AgentRunHerdr, AgentShortcut, KanbanCard, PersistedState};

/// The one install hint — rides the single skip line, never a nag loop.
pub const INSTALL_HINT: &str = "install: https://herdr.dev/install.sh (or `brew install herdr`)";

/// The branch's answer to the launch door. `Skip` = not ours — the
/// caller's old launcher (or its honest error) owns the launch. `Up` /
/// `Blocked` carry the human-facing one-liner; the launch door appends
/// its query-file note.
pub enum Branch {
    Skip,
    Up(String),
    Blocked(String),
}

/// Run one herdr CLI call and parse its JSON envelope. Bounded: the
/// child is killed after `timeout_ms`, so a hung socket never parks the
/// launch door. `Err` carries `code: message` from the error envelope
/// (or the honest transport failure) — the caller decides skip vs
/// blocked; this fn never retries.
fn run_json(bin: &Path, args: &[&str], timeout_ms: u64) -> Result<Value, String> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let out = child
                    .wait_with_output()
                    .map_err(|e| format!("output: {e}"))?;
                let text = String::from_utf8_lossy(&out.stdout);
                let v: Value = serde_json::from_str(text.trim())
                    .map_err(|e| format!("bad json ({}): {e}", out.status))?;
                if let Some(err) = v.get("error") {
                    let code = err.get("code").and_then(Value::as_str).unwrap_or("error");
                    let msg = err.get("message").and_then(Value::as_str).unwrap_or("");
                    return Err(format!("{code}: {msg}"));
                }
                return Ok(v);
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("timeout".into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("wait: {e}")),
        }
    }
}

/// Server up = `workspace list` answers.
fn server_up(bin: &Path) -> Result<(), String> {
    run_json(bin, &["workspace", "list"], 10_000).map(|_| ())
}

fn is_server_not_running(why: &str) -> bool {
    why.starts_with("server_not_running")
}

/// Leftover pane: if the binary is present and the socket is down, start
/// `herdr server` (headless) once and wait for the socket. Fail → skip,
/// old launcher still fires. Never a nag loop, never the TUI `herdr`.
fn ensure_server(bin: &Path) -> Result<(), String> {
    match server_up(bin) {
        Ok(()) => return Ok(()),
        Err(e) if is_server_not_running(&e) => {}
        Err(e) => return Err(e),
    }
    let child = Command::new(bin)
        .arg("server")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("herdr server start: {e}"))?;
    std::thread::spawn(move || {
        let mut child = child;
        let _ = child.wait();
    });
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if server_up(bin).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err("server_not_running: started herdr server but the socket never came up".into())
}

/// `~` / `~/…` expansion — claims store the raw AX string.
fn expand_home(p: &str) -> String {
    if p == "~" {
        std::env::var("HOME").unwrap_or_default()
    } else if let Some(rest) = p.strip_prefix("~/") {
        format!("{}/{rest}", std::env::var("HOME").unwrap_or_default())
    } else {
        p.to_string()
    }
}

/// The planet's cwd for `workspace create`: the newest claim cwd on the
/// planet (the terminal-class wake trail) when one exists; the Orbitty
/// repo for the `orbitty` planet when no workspaceRoot is set (there is
/// no second planet DB — Project is {id, name}); else $HOME.
fn planet_cwd(st: Option<&PersistedState>, planet_id: &str, project_name: Option<&str>) -> String {
    if let Some(st) = st {
        let mut hits: Vec<(f64, String)> = st
            .claims
            .iter()
            .filter(|c| c.project_id == planet_id)
            .filter_map(|c| c.cwd.as_deref().map(|w| (c.last_touched_at, expand_home(w))))
            .filter(|(_, w)| !w.is_empty() && Path::new(w).is_dir())
            .collect();
        hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        if let Some((_, w)) = hits.pop() {
            return w;
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let is_orbitty = project_name
        .map(|n| n.trim().eq_ignore_ascii_case("orbitty"))
        .unwrap_or(false);
    if is_orbitty && !home.is_empty() {
        let repo = format!("{home}/goose artifacts/livesprite-tauri");
        if Path::new(&repo).is_dir() {
            return repo;
        }
    }
    if home.is_empty() { "/".into() } else { home }
}

fn is_doing(c: &KanbanCard) -> bool {
    c.kind.as_deref() == Some("todo") && c.status == "doing"
}

/// The doing-run row this launch rides: the doing card whose title is
/// the launch's first title (pickup hands `[card.title]`), else the
/// newest doing card whose row names this shortcut. Never mints — the
/// DOING-WALKER attach door owns the row; this only FINDS it.
fn resolve_run(st: &PersistedState, shortcut: &AgentShortcut, titles: &[String]) -> Option<KanbanCard> {
    let has_row = |id: &str| st.agent_runs.iter().any(|r| r.card_id == id);
    if let Some(t) = titles.first() {
        if let Some(c) = st.kanban_cards.iter().find(|c| {
            is_doing(c)
                && &c.title == t
                && has_row(&c.id)
                && (c.assigned_agent_id.as_deref() == Some(shortcut.id.as_str())
                    || st
                        .agent_runs
                        .iter()
                        .any(|r| r.card_id == c.id && r.agent_shortcut_id == shortcut.id))
        }) {
            return Some(c.clone());
        }
    }
    st.agent_runs
        .iter()
        .filter(|r| r.agent_shortcut_id == shortcut.id)
        .filter_map(|r| st.kanban_cards.iter().find(|c| c.id == r.card_id))
        .filter(|c| is_doing(c))
        .max_by(|a, b| {
            a.last_touched_at
                .partial_cmp(&b.last_touched_at)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

/// The ONE skip line — event log only, once per launch attempt. Never a
/// nag loop, never a prompt-install.
fn skip_line(app: &tauri::AppHandle, shortcut: &AgentShortcut, reason: &str) {
    crate::event_log::append_best(
        app,
        "herdr-skip",
        None,
        shortcut.project_id.as_deref(),
        Some("agent"),
        true,
        &serde_json::json!({
            "agentId": shortcut.id,
            "harness": shortcut.harness,
            "reason": reason,
        }),
    );
}

/// Persist `agentRun.herdr` + the paint state on the one run row,
/// through the one save door (`state:changed` → the rail reloads → the
/// walker repaints). A launch with no doing-run row persists nothing —
/// never a twin row, never a second planet DB. Best-effort.
fn persist_run(app: &tauri::AppHandle, card: Option<&KanbanCard>, rec: AgentRunHerdr, state: &str) {
    let Some(card) = card else { return };
    let Ok(before) = crate::state::load_state(app) else { return };
    if !before.agent_runs.iter().any(|r| r.card_id == card.id) {
        return;
    }
    let mut after = before.clone();
    if let Some(r) = after.agent_runs.iter_mut().find(|r| r.card_id == card.id) {
        r.herdr = Some(rec);
        // The card's doing→done drag owns done — the branch never
        // resurrects a walked-off row.
        if r.state != "done" {
            r.state = state.into();
        }
    }
    let _ = crate::board::save_with_events(app, &before, &after);
}

/// `herdr agent get <target>` → the pane's agent_status. None = gone /
/// unreadable — the watch's end signal, never a paint.
fn agent_status(bin: &Path, target: &str) -> Option<String> {
    let v = run_json(bin, &["agent", "get", target], 10_000).ok()?;
    v["result"]["agent"]["agent_status"]
        .as_str()
        .map(str::to_string)
}

/// Paint walker state only: write the row's state through the one save
/// door when (and only when) it changes — no write, no event, no storm.
/// `done` rows are untouchable (the card's drag + walk-off own them).
fn paint(app: &tauri::AppHandle, card_id: Option<&str>, state: &str) {
    let Some(cid) = card_id else { return };
    let Ok(before) = crate::state::load_state(app) else { return };
    let Some(row) = before.agent_runs.iter().find(|r| r.card_id == cid) else {
        return;
    };
    if row.state == state || row.state == "done" {
        return;
    }
    let planet_id = row.planet_id.clone();
    let mut after = before.clone();
    if let Some(r) = after.agent_runs.iter_mut().find(|r| r.card_id == cid) {
        r.state = state.into();
    }
    if crate::board::save_with_events(app, &before, &after).is_ok() {
        crate::event_log::append_best(
            app,
            "herdr-watch",
            None,
            Some(&planet_id),
            Some("agent"),
            true,
            &serde_json::json!({ "cardId": cid, "state": state }),
        );
    }
}

/// The watch: block in `agent wait` (60s slices, ~2h cap), then mirror
/// the pane state onto the run row — blocked ↔ working only. The
/// agent's own `done` ends the watch (the card's drag owns the done
/// paint + the far-rim walk-off); a vanished row/card ends it too. The
/// watch owns the dock-LED off for the run it launched. Never a retry
/// loop: one wait at a time, no writes without a transition.
fn spawn_watch(app: &tauri::AppHandle, bin: &Path, card: Option<&KanbanCard>, agent_name: &str) {
    let app = app.clone();
    let bin: PathBuf = bin.to_path_buf();
    let card_id = card.map(|c| c.id.clone());
    let agent_name = agent_name.to_string();
    std::thread::spawn(move || {
        for _ in 0..120 {
            if let Some(cid) = card_id.as_deref() {
                let live = crate::state::load_state(&app)
                    .map(|st| {
                        st.agent_runs
                            .iter()
                            .any(|r| r.card_id == cid && r.state != "done")
                            && st
                                .kanban_cards
                                .iter()
                                .any(|c| c.id == cid && c.status == "doing")
                    })
                    .unwrap_or(false);
                if !live {
                    break;
                }
            }
            let waited = run_json(
                &bin,
                &[
                    "agent", "wait", &agent_name,
                    "--until", "working", "--until", "idle",
                    "--until", "blocked", "--until", "done",
                    "--timeout", "60000",
                ],
                65_000,
            );
            match waited {
                Ok(_) => match agent_status(&bin, &agent_name).as_deref() {
                    Some("blocked") => paint(&app, card_id.as_deref(), "blocked"),
                    // idle = at the prompt between turns — the run is
                    // still live on the disc.
                    Some("working") | Some("idle") => paint(&app, card_id.as_deref(), "working"),
                    Some("done") => break,
                    _ => {}
                },
                // Timeout is the normal still-working case; a gone agent
                // (or a dead server) ends the watch.
                Err(_) => {
                    if agent_status(&bin, &agent_name).is_none() {
                        break;
                    }
                }
            }
        }
        let _ = crate::coordinator::set_companion_hack(app, false, None, None);
    });
}

/// The Herdr branch of `launch_agent`. One attempt, no retries; every
/// skip logs exactly one line. Called only from the grok / opencode
/// doors — every other harness keeps its old launcher, silently.
pub fn branch(
    app: &tauri::AppHandle,
    shortcut: &AgentShortcut,
    project_name: Option<&str>,
    titles: &[String],
) -> Branch {
    // Kind law: grok is always a herdr kind; opencode is detect-only
    // (on PATH = legal, missing = skip that kind — never prompt-
    // installed); every other harness is not a herdr lane.
    let kind = match shortcut.harness.trim().to_ascii_lowercase().as_str() {
        "grok" => "grok",
        "opencode" => {
            if crate::coordinator::find_on_path("opencode").is_none() {
                skip_line(
                    app,
                    shortcut,
                    "opencode not on PATH — kind skipped (detect-only; never prompt-installed)",
                );
                return Branch::Skip;
            }
            "opencode"
        }
        _ => return Branch::Skip,
    };
    let Some(bin) = crate::coordinator::find_on_path("herdr") else {
        skip_line(
            app,
            shortcut,
            &format!("herdr not on PATH — {INSTALL_HINT}; the old launcher fires"),
        );
        return Branch::Skip;
    };
    if let Err(why) = ensure_server(&bin) {
        skip_line(
            app,
            shortcut,
            &format!("herdr server not up ({why}) — the old launcher fires"),
        );
        return Branch::Skip;
    }

    let st = crate::state::load_state(app).ok();
    let card = st.as_ref().and_then(|s| resolve_run(s, shortcut, titles));
    let planet_id = card
        .as_ref()
        .map(|c| c.project_id.clone())
        .or_else(|| shortcut.project_id.clone())
        .unwrap_or_default();
    let cwd = planet_cwd(st.as_ref(), &planet_id, project_name);
    let label = project_name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or("orbitty");

    // Pane first, agent second.
    let (workspace, tab, pane) =
        match run_json(&bin, &["workspace", "create", "--cwd", &cwd, "--label", label], 15_000) {
            Ok(v) => {
                let r = &v["result"];
                let w = r["workspace"]["workspace_id"].as_str().unwrap_or("").to_string();
                let t = r["tab"]["tab_id"].as_str().unwrap_or("").to_string();
                let p = r["root_pane"]["pane_id"].as_str().unwrap_or("").to_string();
                if w.is_empty() || p.is_empty() {
                    skip_line(
                        app,
                        shortcut,
                        "workspace create answered without workspace/root_pane ids; the old launcher fires",
                    );
                    return Branch::Skip;
                }
                (w, t, p)
            }
            Err(e) => {
                skip_line(
                    app,
                    shortcut,
                    &format!("workspace create failed ({e}); the old launcher fires"),
                );
                return Branch::Skip;
            }
        };

    // The agent's name is the card id when a run row rides (the
    // skeleton's `<cardId>`), else a stable per-shortcut name.
    let agent_name = card
        .as_ref()
        .map(|c| c.id.clone())
        .unwrap_or_else(|| format!("orbitty-{}", shortcut.id));
    let rec = AgentRunHerdr {
        planet_id: planet_id.clone(),
        workspace: workspace.clone(),
        tab,
        pane: pane.clone(),
        agent_name: agent_name.clone(),
        kind: kind.into(),
    };
    match run_json(&bin, &["agent", "start", &agent_name, "--kind", kind, "--pane", &pane], 45_000)
    {
        Ok(_) => {
            persist_run(app, card.as_ref(), rec, "working");
            spawn_watch(app, &bin, card.as_ref(), &agent_name);
            crate::event_log::append_best(
                app,
                "herdr-launch",
                None,
                Some(&planet_id),
                Some("agent"),
                true,
                &serde_json::json!({
                    "cardId": card.as_ref().map(|c| c.id.clone()),
                    "workspace": workspace,
                    "pane": pane,
                    "agent": agent_name,
                    "kind": kind,
                }),
            );
            Branch::Up(format!("herdr: agent {agent_name} ({kind}) up in workspace {workspace}."))
        }
        Err(e) => {
            // agent_not_ready / login wall: the pane EXISTS — the human
            // looks at it. Blocked, no retry loop, no old-launcher
            // dual-fire. The watch still rides: a human clearing the
            // wall flips the walker back to working.
            persist_run(app, card.as_ref(), rec, "blocked");
            spawn_watch(app, &bin, card.as_ref(), &agent_name);
            crate::event_log::append_best(
                app,
                "herdr-blocked",
                None,
                Some(&planet_id),
                Some("agent"),
                true,
                &serde_json::json!({
                    "cardId": card.as_ref().map(|c| c.id.clone()),
                    "workspace": workspace,
                    "pane": pane,
                    "agent": agent_name,
                    "kind": kind,
                    "error": e,
                }),
            );
            Branch::Blocked(format!(
                "herdr: agent {agent_name} not ready in workspace {workspace} ({e}) — blocked; look at the pane."
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AgentRun, Claim, Project};

    fn card(id: &str, status: &str, title: &str, agent: Option<&str>) -> KanbanCard {
        KanbanCard {
            id: id.into(),
            title: title.into(),
            project_id: "p-1".into(),
            window_id: None,
            kind: Some("todo".into()),
            status: status.into(),
            created_at: 0.0,
            last_touched_at: 0.0,
            completed_at: None,
            goal_aligned: true,
            clinical_or_master: false,
            points_awarded: 0,
            starred: false,
            notes: None,
            difficulty: None,
            subtasks: None,
            due_at: None,
            tag_ids: None,
            needed_state: None,
            needed_why: None,
            needed_answer: None,
            actor: None,
            assigned_agent_id: agent.map(str::to_string),
        }
    }

    fn run(card_id: &str, agent: &str) -> AgentRun {
        AgentRun {
            card_id: card_id.into(),
            planet_id: "p-1".into(),
            agent_shortcut_id: agent.into(),
            state: "working".into(),
            herdr: None,
        }
    }

    fn shortcut(id: &str) -> AgentShortcut {
        AgentShortcut {
            id: id.into(),
            name: id.into(),
            harness: "grok".into(),
            profile: String::new(),
            model: None,
            md_paths: Vec::new(),
            project_id: Some("p-1".into()),
            extra: None,
        }
    }

    fn claim(project: &str, touched: f64, cwd: Option<&str>) -> Claim {
        Claim {
            window_id: format!("w-{touched}"),
            project_id: project.into(),
            last_touched_at: touched,
            bundle_id: None,
            app_name: None,
            title_at_claim: None,
            title_index: None,
            sprite_pack_id: None,
            char_seed: None,
            char_look: None,
            sprite_scale: None,
            walker_home: None,
            file_path: None,
            hermes_profile: None,
            hermes_session: None,
            action_id: None,
            command: None,
            cwd: cwd.map(str::to_string),
        }
    }

    #[test]
    fn resolve_run_prefers_the_titled_doing_card() {
        let mut st = PersistedState::default();
        st.kanban_cards = vec![
            card("c-1", "doing", "GO-ORBITTY-X", Some("a-1")),
            card("c-2", "doing", "GO-ORBITTY-Y", Some("a-1")),
            card("c-3", "todo", "GO-ORBITTY-Z", Some("a-1")),
        ];
        st.kanban_cards[0].last_touched_at = 1.0;
        st.kanban_cards[1].last_touched_at = 2.0;
        st.agent_runs = vec![run("c-1", "a-1"), run("c-2", "a-1"), run("c-3", "a-1")];
        // pickup hands [card.title] — the exact title wins.
        let hit = resolve_run(&st, &shortcut("a-1"), &["GO-ORBITTY-Y".to_string()]);
        assert_eq!(hit.map(|c| c.id), Some("c-2".to_string()));
        // A titled card that is not doing never title-matches; the
        // fallback is the shortcut's NEWEST doing row.
        let miss = resolve_run(&st, &shortcut("a-1"), &["GO-ORBITTY-Z".to_string()]);
        assert_eq!(miss.map(|c| c.id), Some("c-2".to_string()));
        // No titles → the shortcut's newest doing row.
        let fb = resolve_run(&st, &shortcut("a-1"), &[]);
        assert_eq!(fb.map(|c| c.id), Some("c-2".to_string()));
        // A card with no run row never resolves on title alone, and an
        // unknown shortcut has no fallback.
        st.kanban_cards.push(card("c-4", "doing", "GO-ORBITTY-NOROW", Some("a-1")));
        let none = resolve_run(&st, &shortcut("a-9"), &["GO-ORBITTY-NOROW".to_string()]);
        assert!(none.is_none());
    }

    #[test]
    fn planet_cwd_newest_live_claim_wins() {
        let mut st = PersistedState::default();
        let tmp = std::env::temp_dir().to_string_lossy().to_string();
        st.projects = vec![Project { id: "p-1".into(), name: "orbitty".into() }];
        st.claims = vec![
            claim("p-1", 1.0, Some("/definitely/gone")),
            claim("p-1", 2.0, Some(&tmp)),
            claim("p-9", 9.0, None),
        ];
        // Newest EXISTING claim cwd on this planet wins (gone dirs skip).
        assert_eq!(planet_cwd(Some(&st), "p-1", Some("orbitty")), tmp);
        // No claim on the planet + the orbitty name → the repo when it
        // exists, else HOME — never invented.
        let home = std::env::var("HOME").unwrap_or_default();
        let repo = format!("{home}/goose artifacts/livesprite-tauri");
        let want = if Path::new(&repo).is_dir() { repo } else { home.clone() };
        assert_eq!(planet_cwd(Some(&st), "p-9", Some("orbitty")), want);
        // A non-orbitty planet with no claim cwd → HOME.
        assert_eq!(planet_cwd(Some(&st), "p-9", Some("mars")), home);
        // No state at all → HOME.
        assert_eq!(planet_cwd(None, "p-1", Some("mars")), home);
    }

    #[test]
    fn expand_home_law() {
        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(expand_home("~"), home);
        assert_eq!(expand_home("~/x"), format!("{home}/x"));
        assert_eq!(expand_home("/tmp"), "/tmp");
    }

    #[test]
    fn server_not_running_code() {
        assert!(is_server_not_running(
            "server_not_running: no herdr server is running"
        ));
        assert!(!is_server_not_running("timeout"));
        assert!(!is_server_not_running("bad json"));
    }
}
