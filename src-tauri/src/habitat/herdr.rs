//! Herdr branch of `launch_agent` for grok, cursor, hermes, and
//! opencode (detect-only). Present + server up → pane. Else one skip
//! line and the old launcher. Goose is not a herdr kind: one skip line,
//! then its old launcher. No dual-launch, no retry loop.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

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
    /// (message, herdr agent name) — the name is the task's liveness
    /// hint (task::LiveHint::herdr_agent).
    Up(String, String),
    Blocked(String, String),
}

/// Run one herdr CLI call and parse its JSON envelope. Bounded: the
/// child is killed after `timeout_ms`, so a hung socket never parks the
/// launch door. `Err` carries `code: message` from the error envelope
/// (or the honest transport failure) — the caller decides skip vs
/// blocked; this fn never retries.
#[allow(dead_code)] // re-exported from herdr/mod.rs for older call sites
pub(crate) fn herdr_cmd(bin: &Path) -> Command {
    crate::herdr::client::command(bin)
}

#[allow(dead_code)] // re-exported from herdr/mod.rs for older call sites
pub(crate) fn run_json(bin: &Path, args: &[&str], timeout_ms: u64) -> Result<Value, String> {
    crate::herdr::client::run_json(bin, args, timeout_ms)
}

/// Flag on checks `hc server status` and does not spawn. Flag off is the
/// old spawn-once wait, byte-for-byte, inside `herdr/tui.rs`.
fn ensure_server(bin: &Path) -> Result<(), String> {
    crate::herdr::client::ensure_server(bin)
}

pub(crate) use crate::herdr::tui::is_server_not_running;

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
pub(crate) fn planet_cwd(st: Option<&PersistedState>, planet_id: &str, project_name: Option<&str>) -> String {
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
        let repo = format!("{home}/orbitty");
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
    if let Some(t) = titles.first() {
        if let Some(c) = st.kanban_cards.iter().find(|c| {
            is_doing(c)
                && &c.title == t
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
    // Launch is slow; doing→todo can land first. Never persist a live
    // pane onto a card that is already reset — stop the harness instead.
    let live = before
        .kanban_cards
        .iter()
        .find(|c| c.id == card.id)
        .map(|c| c.status.as_str() == "doing")
        .unwrap_or(false);
    if !live {
        crate::agent_stop::herdr_stop(&rec);
        return;
    }
    let mut after = before.clone();
    if let Some(r) = after.agent_runs.iter_mut().find(|r| r.card_id == card.id) {
        r.herdr = Some(rec);
        if r.state != "done" {
            r.state = state.into();
        }
        if r.agent_shortcut_id.is_empty() {
            r.agent_shortcut_id = card.assigned_agent_id.clone().unwrap_or_default();
        }
    } else {
        after.agent_runs.push(crate::state::AgentRun {
            card_id: card.id.clone(),
            planet_id: card.project_id.clone(),
            agent_shortcut_id: card.assigned_agent_id.clone().unwrap_or_default(),
            state: state.into(),
            herdr: Some(rec),
        });
    }
    let _ = crate::board::save_with_events(app, &before, &after);
}

/// Boot liveness. Flag off is today's TUI probe. Flag on asks hc, and
/// answers None while herdr-core is DOWN (unknown, never died).
pub(crate) fn agent_alive(agent_name: &str) -> Option<bool> {
    crate::herdr::client::alive(agent_name)
}

pub(crate) use crate::herdr::tui::alive_from;

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
    if crate::state::with_actor("agent", || crate::board::save_with_events(app, &before, &after))
        .is_ok()
    {
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

fn wait_args<'a>(agent_name: &'a str, status: Option<&str>) -> Vec<&'a str> {
    crate::herdr::tui::wait_args(agent_name, status)
}

/// The watch: block in `agent wait` (60s slices, ~2h cap), then mirror
/// the pane state onto the run row — blocked ↔ working only. The
/// agent's own `done` ends the watch (the card's drag owns the done
/// paint + the far-rim walk-off); a vanished row/card ends it too.
/// Watch exit heals the dock LED from seated runs (never a blind off —
/// prompt-idle is still live). Never a retry loop: one wait at a time,
/// no writes without a transition.
fn spawn_watch(app: &tauri::AppHandle, bin: &Path, card: Option<&KanbanCard>, rec: AgentRunHerdr) {
    let app = app.clone();
    let bin: PathBuf = bin.to_path_buf();
    let card_id = card.map(|c| c.id.clone());
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
            let status = crate::herdr::client::run_status(&bin, &rec);
            let waited = crate::herdr::client::wait_run(&bin, &rec, status.as_deref());
            match waited {
                Ok(_) => {
                    if rec.backend == "core" {
                        let id = if rec.terminal.trim().is_empty() {
                            rec.agent_name.as_str()
                        } else {
                            rec.terminal.trim()
                        };
                        if crate::herdr::core::terminal(&bin, id)
                            .ok()
                            .as_ref()
                            .map(crate::herdr::core::map_state)
                            .as_deref()
                            == Some("finished")
                        {
                            break;
                        }
                    }
                    match crate::herdr::client::run_status(&bin, &rec).as_deref() {
                        Some("blocked") => paint(&app, card_id.as_deref(), "blocked"),
                        // idle = at the prompt between turns — the run is
                        // still live on the disc.
                        Some("working") | Some("idle") => paint(&app, card_id.as_deref(), "working"),
                        Some("done") | Some("error") => break,
                        _ => {}
                    }
                }
                // Timeout is the normal still-working case; a gone agent
                // (or a dead server) ends the watch.
                Err(_) => {
                    if crate::herdr::client::run_status(&bin, &rec).is_none() {
                        break;
                    }
                }
            }
        }
        crate::coordinator::heal_stuck_hack(&app);
    });
}

fn agent_start_args<'a>(name: &'a str, kind: &'a str, pane: &'a str) -> Vec<&'a str> {
    crate::herdr::tui::agent_start_args(name, kind, pane)
}

/// After start is ready, submit the query file. Fail → seat still up,
/// one log line, no Terminal dual-launch. Login-wall starts never call this.
fn submit_query(
    app: &tauri::AppHandle,
    bin: &Path,
    planet_id: &str,
    rec: &AgentRunHerdr,
    query: Option<&Path>,
) {
    let Some(path) = query else { return };
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if let Err(why) = crate::herdr::client::prompt_with(bin, rec, text) {
        crate::event_log::append_best(
            app,
            "herdr-prompt-fail",
            None,
            Some(planet_id),
            Some("agent"),
            true,
            &serde_json::json!({
                "agent": rec.agent_name,
                "reason": why,
            }),
        );
    }
}

/// herdr `--kind` for this harness. Goose and anything else skip with
/// a line (the caller logs it). Opencode is detect-only.
fn kind_law(harness: &str, opencode_on_path: bool) -> Result<&'static str, String> {
    match harness.trim().to_ascii_lowercase().as_str() {
        "grok" => Ok("grok"),
        "cursor" => Ok("cursor"),
        "hermes" => Ok("hermes"),
        "opencode" => {
            if opencode_on_path {
                Ok("opencode")
            } else {
                Err(
                    "opencode not on PATH — kind skipped (detect-only; never prompt-installed)"
                        .into(),
                )
            }
        }
        other => Err(format!("harness {other} is not a herdr kind")),
    }
}

fn start_argv(name: &str, kind: &str, pane: &str, extra: &[String]) -> Vec<String> {
    crate::herdr::tui::start_argv(name, kind, pane, extra)
}

/// The skip line when the chosen binary is missing. Same words as
/// "herdr not on PATH" so the old launcher still fires.
pub(crate) fn missing_launcher_line() -> String {
    format!("herdr not on PATH — {INSTALL_HINT}; the old launcher fires")
}

pub(crate) fn require_bin() -> Result<PathBuf, String> {
    crate::herdr::locate().ok_or_else(missing_launcher_line)
}

/// The Herdr branch of `launch_agent`. One attempt, no retries; every
/// skip logs exactly one line. grok, cursor, hermes, and opencode are
/// herdr kinds. Every other harness logs a skip and returns `Skip`.
pub fn branch(
    app: &tauri::AppHandle,
    shortcut: &AgentShortcut,
    project_name: Option<&str>,
    titles: &[String],
    query: Option<&Path>,
) -> Branch {
    // Kind law: grok, cursor, and hermes are herdr kinds; opencode is
    // detect-only (on PATH = legal, missing = skip — never prompt-
    // installed). Every other harness logs one skip line.
    let kind = match kind_law(
        &shortcut.harness,
        crate::coordinator::find_on_path("opencode").is_some(),
    ) {
        Ok(kind) => kind,
        Err(reason) => {
            skip_line(app, shortcut, &reason);
            return Branch::Skip;
        }
    };
    let bin = match require_bin() {
        Ok(bin) => bin,
        Err(line) => {
            skip_line(app, shortcut, &line);
            return Branch::Skip;
        }
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
    // The agent's name is the card id when a run row rides (the
    // skeleton's `<cardId>`), else a stable per-shortcut name.
    let agent_name = card
        .as_ref()
        .map(|c| c.id.clone())
        .unwrap_or_else(|| format!("orbitty-{}", shortcut.id));
    let extra = if kind == "cursor" {
        let model = crate::agents::cursor_model(shortcut.model.as_deref());
        let preset = crate::agents::normalize_permissions(Some(shortcut.permissions.as_str()));
        crate::agents::cursor_agent_args(&model, preset)
    } else {
        Vec::new()
    };

    if crate::herdr::core_on() {
        let card_id = card.as_ref().map(|c| c.id.as_str()).unwrap_or(agent_name.as_str());
        // One run. An id in hand is the seat — never a second launch.
        let addr = match crate::herdr::client::launch_core(
            &bin,
            &cwd,
            card_id,
            &planet_id,
            kind,
            &extra,
        ) {
            Ok(addr) => addr,
            Err(e) => {
                skip_line(
                    app,
                    shortcut,
                    &format!("hc run failed ({e}); the old launcher fires"),
                );
                return Branch::Skip;
            }
        };
        let rec = AgentRunHerdr {
            planet_id: planet_id.clone(),
            workspace: String::new(),
            tab: String::new(),
            pane: String::new(),
            agent_name: agent_name.clone(),
            kind: kind.into(),
            backend: "core".into(),
            terminal: addr.clone(),
            extra: Default::default(),
        };
        persist_run(app, card.as_ref(), rec.clone(), "working");
        crate::coordinator::heal_stuck_hack(app);
        spawn_watch(app, &bin, card.as_ref(), rec.clone());
        submit_query(app, &bin, &planet_id, &rec, query);
        crate::event_log::append_best(
            app,
            "herdr-launch",
            None,
            Some(&planet_id),
            Some("agent"),
            true,
            &serde_json::json!({
                "cardId": card.as_ref().map(|c| c.id.clone()),
                "workspace": "",
                "pane": "",
                "terminal": addr,
                "backend": "core",
                "agent": agent_name,
                "kind": kind,
            }),
        );
        return Branch::Up(
            format!("herdr: agent {agent_name} ({kind}) up in terminal {addr}."),
            agent_name,
        );
    }

    // Pane first, agent second. Flag off: today's TUI argv.
    let (workspace, tab, pane) =
        match crate::herdr::client::workspace_create(&bin, &cwd, label) {
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

    let rec = AgentRunHerdr {
        planet_id: planet_id.clone(),
        workspace: workspace.clone(),
        tab,
        pane: pane.clone(),
        agent_name: agent_name.clone(),
        kind: kind.into(),
        backend: "tui".into(),
        terminal: String::new(),
        extra: Default::default(),
    };
    std::thread::sleep(Duration::from_millis(400));
    let mut started = Err("agent start not attempted".into());
    for i in 0..5 {
        started = crate::herdr::client::agent_start(&bin, &agent_name, kind, &pane, &extra);
        if started.is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250 * (i + 1)));
    }
    match started
    {
        Ok(_) => {
            persist_run(app, card.as_ref(), rec.clone(), "working");
            crate::coordinator::heal_stuck_hack(app);
            spawn_watch(app, &bin, card.as_ref(), rec.clone());
            submit_query(app, &bin, &planet_id, &rec, query);
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
                    "terminal": "",
                    "backend": "tui",
                    "agent": agent_name,
                    "kind": kind,
                }),
            );
            Branch::Up(
                format!("herdr: agent {agent_name} ({kind}) up in workspace {workspace}."),
                agent_name.clone(),
            )
        }
        Err(e) => {
            // agent_not_ready / login wall: the pane EXISTS — the human
            // looks at it. Blocked, no retry loop, no old-launcher
            // dual-fire. The watch still rides: a human clearing the
            // wall flips the walker back to working.
            persist_run(app, card.as_ref(), rec.clone(), "blocked");
            crate::coordinator::heal_stuck_hack(app);
            spawn_watch(app, &bin, card.as_ref(), rec.clone());
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
                    "terminal": "",
                    "backend": "tui",
                    "agent": agent_name,
                    "kind": kind,
                    "error": e,
                }),
            );
            Branch::Blocked(
                format!(
                    "herdr: agent {agent_name} not ready in workspace {workspace} ({e}) — blocked; look at the pane."
                ),
                agent_name.clone(),
            )
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
            parent_id: None,
            rating: None,
            trackers: None,
            task_id: None,
            task_reason: None,
            links: None,
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
            permissions: "read-only".into(),
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
            tab: None,
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
        // Assigned doing with no run row still title-matches THIS shortcut
        // (persist_run mints the row). An unknown shortcut has no fallback.
        st.kanban_cards.push(card("c-4", "doing", "GO-ORBITTY-NOROW", Some("a-1")));
        let minted = resolve_run(&st, &shortcut("a-1"), &["GO-ORBITTY-NOROW".to_string()]);
        assert_eq!(minted.map(|c| c.id), Some("c-4".to_string()));
        let none = resolve_run(&st, &shortcut("a-9"), &["GO-ORBITTY-NOROW".to_string()]);
        assert!(none.is_none());
    }

    #[test]
    fn planet_cwd_newest_live_claim_wins() {
        let mut st = PersistedState::default();
        let tmp = std::env::temp_dir().to_string_lossy().to_string();
        st.projects = vec![Project {
            id: "p-1".into(),
            name: "orbitty".into(),
            trackers: None,
            workspace_root: None,
        }];
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
        let repo = format!("{home}/orbitty");
        let want = if Path::new(&repo).is_dir() { repo } else { home.clone() };
        assert_eq!(planet_cwd(Some(&st), "p-9", Some("orbitty")), want);
        // A non-orbitty planet with no claim cwd → HOME.
        assert_eq!(planet_cwd(Some(&st), "p-9", Some("mars")), home);
        // No state at all → HOME.
        assert_eq!(planet_cwd(None, "p-1", Some("mars")), home);
    }

    #[test]
    fn wait_args_skip_until_idle_when_already_idle() {
        let idle = wait_args("c-1", Some("idle"));
        assert!(
            !idle.windows(2).any(|w| w == ["--until", "idle"]),
            "already-idle wait must not --until idle: {idle:?}"
        );
        for until in ["working", "blocked", "done"] {
            assert!(
                idle.windows(2).any(|w| w == ["--until", until]),
                "idle wait still waits for {until}: {idle:?}"
            );
        }
        for status in [None, Some("working"), Some("blocked"), Some("done")] {
            let args = wait_args("c-1", status);
            assert!(
                args.windows(2).any(|w| w == ["--until", "idle"]),
                "non-idle {status:?} still waits for idle: {args:?}"
            );
        }
    }

    #[test]
    fn kind_map_cursor_hermes_grok_and_goose_skips_with_a_line() {
        assert_eq!(kind_law("cursor", false).unwrap(), "cursor");
        assert_eq!(kind_law("hermes", false).unwrap(), "hermes");
        assert_eq!(kind_law("grok", false).unwrap(), "grok");
        assert_eq!(kind_law("opencode", true).unwrap(), "opencode");
        let goose = kind_law("goose", true).unwrap_err();
        assert!(
            goose.contains("goose") && goose.contains("not a herdr kind"),
            "goose skip must be a log line: {goose}"
        );
        let missing = kind_law("opencode", false).unwrap_err();
        assert!(missing.contains("opencode not on PATH"), "{missing}");
    }

    #[test]
    fn cursor_start_argv_passes_model_and_force_only_for_full_auto() {
        let ro = crate::agents::cursor_agent_args("grok-4.7-high", "read-only");
        let argv = start_argv("c-1", "cursor", "w2:p1", &ro);
        assert!(argv.iter().any(|a| a == "--model=grok-4.7-high"));
        assert!(!argv.iter().any(|a| a == "--model"));
        assert!(!argv.iter().any(|a| a == "--force" || a == "--trust"));
        let fa = crate::agents::cursor_agent_args("grok-4.7-high", "full-auto");
        let argv = start_argv("c-1", "cursor", "w2:p1", &fa);
        assert!(argv.iter().any(|a| a == "--"));
        assert!(argv.iter().any(|a| a == "--force"));
        assert!(argv.iter().any(|a| a == "--trust"));
    }

    #[test]
    fn agent_start_args_never_pass_the_grok_binary() {
        let args = agent_start_args("c-1", "grok", "w2:p1");
        assert_eq!(
            args,
            vec![
                "agent",
                "start",
                "c-1",
                "--kind",
                "grok",
                "--pane",
                "w2:p1",
                "--timeout",
                "30000",
            ]
        );
        assert!(!args.iter().any(|a| *a == "--" || a.contains("/grok") || a.ends_with("bin/grok")));
    }

    #[test]
    fn expand_home_law() {
        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(expand_home("~"), home);
        assert_eq!(expand_home("~/x"), format!("{home}/x"));
        assert_eq!(expand_home("/tmp"), "/tmp");
    }

    #[test]
    fn boot_liveness_only_dead_on_real_answers() {
        let ok = || Ok(());
        let err = |m: &str| Err::<(), String>(m.to_string());
        // Genuinely down → panes gone.
        assert_eq!(alive_from(err("server_not_running: no socket"), None), Some(false));
        // Probe failures are unknown, never dead.
        assert_eq!(alive_from(err("timeout"), None), None);
        assert_eq!(alive_from(err("spawn: No such file"), None), None);
        assert_eq!(alive_from(err("bad json (exit 1): x"), None), None);
        // Server up: pane answers → alive; herdr's own not-found → dead.
        assert_eq!(alive_from(ok(), Some(ok())), Some(true));
        assert_eq!(
            alive_from(ok(), Some(err("agent_not_found: agent target x not found"))),
            Some(false)
        );
        assert_eq!(alive_from(ok(), Some(err("timeout"))), None);
        assert_eq!(alive_from(ok(), Some(err("some_other_code: y"))), None);
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
