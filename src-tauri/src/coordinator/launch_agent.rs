use super::action::find_on_path;
use super::commands::set_companion_hack;
use super::cursor_perm::{install_cursor_permissions, launch_refused, open_agent_terminal};
use super::paths::{terminal_run_paths, write_agent_query, GoLaunch};
use super::plug::find_grok;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Launch a user-authored agent shortcut in its external harness. Orbitty
/// writes the query file and hands off — it never runs the model.
/// The harness is per row (cursor | goose | grok
/// | hermes | pi) with a named door each — an unknown harness is an honest
/// error, never a silent hermes dump. Each door also owns the dock LED
/// off for the run it launched (pickup lit it): the Terminal `.command`
/// traps EXIT, the hermes door waits on the child.
pub fn launch_agent(
    app: tauri::AppHandle,
    mut shortcut: crate::state::AgentShortcut,
    project_name: Option<String>,
    titles: Vec<String>,
    mut go_launch: Option<GoLaunch>,
) -> Result<String, String> {
    #[cfg(unix)]
    crate::gate_caps::check(&app)?;
    // push_allowed is enforced by the launcher rule in the prompt only; no
    // harness has a push flag. git_push without push_allowed is refused here.
    let push_ok = if let Some(g) = go_launch.as_mut() {
        let primary_ok = match g.run.harness.as_str() {
            "cursor" => find_on_path("cursor-agent").is_some(),
            "goose" => find_on_path("goose").is_some(),
            "grok" => find_on_path("grok").is_some(),
            "herdr" => find_on_path("herdr").is_some() || find_on_path("grok").is_some(),
            _ => true,
        };
        g.run = crate::plans::apply_fallback(&g.run, primary_ok);
        let push_ok = crate::plans::push_decision(&g.run)?;
        let door = crate::plans::door_harness(&g.run.harness);
        if door == "manual" {
            let _ = std::process::Command::new("open")
                .arg(&g.parsed.path)
                .status();
            record_go_launch(&app, g);
            return Ok(format!(
                "Opened {} (harness manual). Nothing launched.",
                g.parsed.path.display()
            ));
        }
        shortcut.harness = door;
        if !g.run.model.is_empty() {
            if shortcut.harness == "cursor" {
                crate::agents::validate_model(&g.run.model)?;
            }
            shortcut.model = Some(g.run.model.clone());
        }
        push_ok
    } else {
        false
    };
    let pid = shortcut.project_id.clone().unwrap_or_default();
    let card_id = crate::state::load_state(&app).ok().and_then(|st| {
        titles.first().and_then(|t| {
            st.kanban_cards
                .iter()
                .find(|c| &c.title == t && (pid.is_empty() || c.project_id == pid))
                .map(|c| c.id.clone())
        })
    });
    let task_id = if pid.is_empty() {
        None
    } else {
        crate::task::start(&app, &pid, card_id.as_deref())
    };
    let go_text = go_launch
        .as_ref()
        .map(|g| crate::plans::launch_text(&g.parsed, push_ok));
    let (query, skipped) = write_agent_query(
        &app,
        &shortcut,
        project_name.as_deref(),
        &titles,
        go_text.as_deref(),
    )?;
    let skip_note = if skipped > 0 {
        format!(" Skipped {skipped} unreadable md path(s).")
    } else {
        String::new()
    };
    // Boot liveness (task::boot): what we handed off to, per door.
    let mut hint = crate::task::LiveHint::default();
    let run_tag = task_id
        .clone()
        .or_else(|| card_id.clone())
        .unwrap_or_else(|| {
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
            let model = crate::agents::launch_model(&shortcut)
                .map_err(|why| refused_model(&app, planet_for_log, &why))?;
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
                &crate::agents::goose_exec_line(&bin.display().to_string()),
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
                &crate::agents::grok_exec_line(&bin.display().to_string()),
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
            // Profile + model reach hermes argv: validated before any door.
            let args = crate::agents::hermes_args(&shortcut, &query)
                .map_err(|why| launch_refused(&app, planet_for_log, "hermes", &why))?;
            match crate::herdr::branch(&app, &shortcut, project_name.as_deref(), &titles, Some(query.as_path())) {
                crate::herdr::Branch::Skip => {}
                crate::herdr::Branch::Up(m, agent) | crate::herdr::Branch::Blocked(m, agent) => {
                    hint.herdr_agent = Some(agent);
                    return Ok(format!("{m} Query: {}.{skip_note}", query.display()));
                }
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
                        "Launched hermes {} chat. Query: {}.{skip_note}",
                        args[0],
                        query.display()
                    ))
                }
                Err(_) => Err(format!(
                    "`hermes` not on PATH. Query file ready: {}",
                    query.display()
                )),
            }
        }
        // Pi door: same visible Terminal handoff as cursor. The query
        // file is the first message. A model on the row is optional;
        // otherwise Pi uses its own default.
        "pi" => {
            let Some(bin) = find_on_path("pi") else {
                return Err(
                    "`pi` not on PATH — install from https://pi.dev/install.sh or pick another harness."
                        .to_string(),
                );
            };
            let model_arg = match shortcut
                .model
                .as_deref()
                .map(str::trim)
                .filter(|m| !m.is_empty())
            {
                Some(m) => {
                    crate::agents::validate_model(m)
                        .map_err(|why| launch_refused(&app, planet_for_log, "pi", &why))?;
                    format!(" --model {}", crate::coordinator::sq(m))
                }
                None => String::new(),
            };
            let exec = format!(
                "{}{model_arg} -- \"$prompt\"",
                crate::coordinator::sq(&bin.display().to_string())
            );
            let opened = open_agent_terminal(
                &app,
                &query,
                &terminal_script,
                &terminal_pid_file,
                &exec,
                Some(workspace.as_str()),
                planet_for_log,
            )?;
            hint.pid_file = Some(terminal_pid_file.clone());
            Ok(format!(
                "Opened pi in {opened}. Query: {}.{skip_note}",
                query.display()
            ))
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
            "unknown harness {other:?} — cursor | goose | grok | hermes | pi (set per agent row; never a silent hermes)"
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
    if out.is_ok() {
        if let Some(g) = go_launch.as_ref() {
            record_go_launch(&app, g);
            crate::verify_close::arm_launch(app.clone(), g.card_id.clone(), &hint);
        }
    }
    out
}

fn record_go_launch(app: &tauri::AppHandle, g: &GoLaunch) {
    let id = g.parsed.id();
    let unsupported = crate::plans::unsupported_tools(&g.run.harness, &g.run.tools);
    crate::event_log::append_best(
        app,
        "go-launch",
        None,
        Some(&g.project_id),
        Some("agent"),
        true,
        &serde_json::json!({
            "id": id,
            "file": g.parsed.path.display().to_string(),
            "sha256": crate::plans::sha256_hex(g.parsed.raw.as_bytes()),
            "runner": g.run.runner,
            "harness": g.run.harness,
            "tools": g.run.tools,
            "route": g.route,
            "cardId": g.card_id,
            "fallback": g.run.fell_back,
            "fallbackWhy": g.run.fallback_why,
            "unsupported": unsupported,
        }),
    );
    let line = if let Some(why) = g.run.fallback_why.as_deref() {
        format!(
            "Launched {id} via fallback {} / {} — {why}",
            g.run.runner, g.run.harness
        )
    } else {
        format!("Launched {id} via {} / {}.", g.run.runner, g.run.harness)
    };
    let _ = crate::board::kanban_note_append(app, &g.card_id, &line);
}

/// The cursor door refused the row model. Count it, then the same refusal the door already returned.
fn refused_model(app: &tauri::AppHandle, planet: Option<&str>, why: &str) -> String {
    crate::shims::fired(
        app,
        "model_argv_refuse",
        crate::shims::ShimCtx {
            seat: Some("cursor"),
        },
    );
    launch_refused(app, planet, "cursor", why)
}
