use std::path::{ Path };
use std::process::{Command, Stdio};
use tauri::{ Emitter };
use super::action::{find_on_path};
use super::allowlist::{hermes_profile_dirs, load_companion_session_id, resolve_agent};
use super::hermes::{app_data};
use super::intercept::{ActionIntercept, log_companion_route, match_action_line, persist_action_intercept, persist_companion_you, wrap_with_pocket};
use super::mouth_cli::{is_goose_session_id, is_uuid, mint_uuid, run_mouth_cli};
use super::plug::{GROK_ROLL_EVERY, PlugState, berdctl_cmd, companion_soul_text, find_berdctl, find_grok, load_plug, save_plug};

/// Companion composer send path.
/// Harness order: env override, saved companion.backend, legacy goose
/// when goose is on PATH, else one plain row and no model call.
/// `hermes` keeps its profile thread. `berd` is the hidden env escape
/// (bound session from the companion plug via `berdctl`).
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
    // The app composer: a bare yes confirms a chat-made board write only.
    companion_send_from(app, line, path, crate::mouth_route::Origin::Chat).await
}

/// [`companion_send`] naming its surface. Socket `companion.send` passes
/// `Origin::Companion`, so it never proposes or confirms a board write.
pub async fn companion_send_from(
    app: tauri::AppHandle,
    line: String,
    path: Option<String>,
    origin: crate::mouth_route::Origin,
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
    let roots = crate::policy_guard::PolicyRoots::from_app(&app);
    let snap = crate::policy_guard::Snapshot::take(&roots);
    let out = companion_send_inner(app.clone(), line, attach, origin).await;
    let out = crate::policy_guard::finish_guard(
        &snap,
        &roots,
        out,
        |title| {
            let _ = crate::board::needed_open(&app, title, "", Some(crate::policy_guard::NEEDED_WHY));
        },
        |ev| {
            crate::event_log::append_best(
                &app,
                "companion-policy-guard",
                None,
                None,
                Some("agent"),
                true,
                ev,
            );
        },
    );
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

pub(crate) async fn companion_send_inner(
    app: tauri::AppHandle,
    line: String,
    attach: Option<crate::attach::AttachInfo>,
    origin: crate::mouth_route::Origin,
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
            return Ok(persist_action_intercept(&app, Some(line.as_str()), &them));
        }
    }

    #[cfg(unix)]
    if !no_intercepts {
        if let Some(out) = crate::companion_go::intercept(&app, &line) {
            let them = match out {
                Ok(s) => s,
                Err(e) => e,
            };
            return Ok(persist_action_intercept(&app, Some(line.as_str()), &them));
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
                let action_name = shortcut.name.clone();
                let action_app = app.clone();
                // Same door as the chord and action.run: a thin row goes to companion_thin.
                let result = tauri::async_runtime::spawn_blocking(move || {
                    crate::actions::dispatch_action(&action_app, &action_name, command, cwd)
                })
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
                return Ok(persist_action_intercept(
                    &app,
                    keep_you.then_some(line.as_str()),
                    &them,
                ));
            }
            Some(ActionIntercept::Ambiguous) => {
                let them = "which action? use the Launchpad well.".to_string();
                return Ok(persist_action_intercept(&app, None, &them));
            }
            None => {}
        }
        }
    }

    let resolved = crate::companion_backend::resolve_companion_backend(&app);
    let reply_lane = crate::mouth_hop::effective_reply_lane(&app);
    // A file-only send still needs a you-row — the
    // chip's name stands in for the empty line (never a blank bubble).
    let you_line = match &attach {
        Some(a) if line.is_empty() => format!("[file] {}", a.name),
        _ => line.clone(),
    };
    let miss = crate::direct_lane::send_route(crate::direct_lane::RouteInput {
        local_hit: false,
        attach: attach.is_some(),
        resolve: resolved.clone(),
        reply_lane: reply_lane.clone(),
    });
    // A picked harness still saves the you-row before the verb, as before.
    // Unset / unknown wait until a local hit, so a miss does not persist
    // a line that is about to error.
    let picked = matches!(
        miss,
        crate::direct_lane::SendRoute::Direct(_) | crate::direct_lane::SendRoute::Mouth(_)
    );
    if picked {
        persist_companion_you(&app, &you_line);
    }
    // Local board/status verbs answer before NoHarness / Unknown,
    // including when the env value is unknown. An attachment never
    // takes this door (N5).
    // Existing verbs: the tool answers. A miss stays on the mouth below.
    // Board add / agent spawn are writes. They still answer here, after
    // one confirm, so a write does not fall through into a board snapshot.
    if attach.is_none() {
        if let Some(spoken) = crate::mouth_route::run(&app, &line, origin) {
            if !picked {
                persist_companion_you(&app, &you_line);
            }
            let hit = crate::direct_lane::send_route(crate::direct_lane::RouteInput {
                local_hit: true,
                attach: false,
                resolve: resolved.clone(),
                reply_lane: reply_lane.clone(),
            });
            log_companion_route(&app, hit.log_name());
            let spoken = crate::mouth_hop::strip_handoff_marks(&spoken);
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
    log_companion_route(&app, miss.log_name());
    if let crate::direct_lane::SendRoute::Unknown(msg) = &miss {
        return Err(msg.clone());
    }
    let backend_source = resolved.pick.source;
    let backend = resolved.pick.id;
    if matches!(miss, crate::direct_lane::SendRoute::NoHarness) {
        let row = crate::companion_backend::NO_HARNESS_ROW.to_string();
        persist_companion_you(&app, &you_line);
        if crate::state::update(&app, |st| {
            st.companion.last_bot_line = row.clone();
            st.companion.push_turn("them", &row);
            Ok(())
        })
        .is_ok()
        {
            let _ = app.emit("state:changed", ());
        }
        return Ok(row);
    }
    if !picked {
        persist_companion_you(&app, &you_line);
    }
    // Pin Quick mouth when LM Studio is serving the catalog E4B and
    // replyModel is still empty. One models GET, then the field sticks.
    crate::mouth_scan::pin_reply_if_empty(&app);
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
                return Ok(persist_action_intercept(&app, None, &them));
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
    // Chip: original path always. Office may add MarkItDown markdown.
    // Images ride only when the resolved lane's model can see.
    let (prompt, vision_path, lane, model_vision) = match &attach {
        Some(a) => {
            let a = a.clone();
            let p = prompt.clone();
            let b = backend.clone();
            let line_c = line.clone();
            let app_c = app.clone();
            match tauri::async_runtime::spawn_blocking(move || {
                let image = crate::attach_vision::is_image(&a.path);
                let (lane, model_vision) = if b == "grok" || b == "direct" {
                    let settings = crate::mouth_hop::effective_reply_lane(&app_c);
                    if b == "direct"
                        && crate::mouth_hop::normalize_reply_lane(&settings) == "compat"
                    {
                        crate::direct_lane::direct_vision(&settings, "compat", false)
                    } else {
                        let (hop, vision) =
                            crate::mouth_hop::image_lane_vision(&app_c, &line_c, image);
                        if b == "direct" {
                            crate::direct_lane::direct_vision(&settings, &hop, vision)
                        } else {
                            (hop, vision)
                        }
                    }
                } else {
                    (b, false)
                };
                let prep = crate::attach_send::prepare(&a, &p, &lane, model_vision);
                (prep.prompt, prep.vision_path, lane, model_vision)
            })
            .await
            {
                Ok(prep) => prep,
                Err(_) => (prompt, None, backend.clone(), false),
            }
        }
        None => (prompt, None, backend.clone(), false),
    };
    // Desk wrap only when the line looks like a board mint — not every
    // "hi". Planet ids stay available for todo: intercept without a chew.
    let prompt = if crate::companion_desk::line_needs_desk(&line) {
        crate::companion_desk::wrap_with_desk(&app, &prompt)
    } else {
        prompt
    };
    // A lane that can't see the image does not get to guess.
    let image = attach
        .as_ref()
        .is_some_and(|a| crate::attach_vision::is_image(&a.path));
    if image && crate::attach_vision::vision_gate_applies(&lane, model_vision) {
        let path = attach
            .as_ref()
            .map(|a| a.path.clone())
            .unwrap_or_default();
        let reply =
            crate::mouth_hop::refuse_or_handoff(&app, &line, &path, &lane).await?;
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
        return Ok(reply);
    }
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
        "direct" => {
            crate::direct_lane::send(
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
            return Err(crate::companion_backend::unknown_backend_message(
                other, backend_source,
            ))
        }
    };
    // No model or agent reply may carry a handoff trailer (a forged
    // "Ask online agent" button). Only the vision gate above adds one.
    let reply = crate::mouth_hop::strip_handoff_marks(&reply);

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

pub(crate) async fn companion_send_berd(line: &str) -> Result<String, String> {
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
pub(crate) async fn companion_send_goose(app: &tauri::AppHandle, line: &str) -> Result<String, String> {
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
    // Same equals form for the line: `-t <line>` rejects a line starting
    // with `-`.
    args.push(format!("--text={line}"));
    args
}

pub(crate) fn run_goose_reply(
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
    let tools_ok = companion_mode_tools(app) && crate::policy_guard::sandbox_ready();
    let args = goose_args(soul, line, resume_id.as_deref(), tools_ok);

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
        // Attached: `-p <line>` rejects a line starting with `-`.
        None => vec![format!("--single={line}")],
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

pub(crate) fn run_grok_reply(
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
    let tools_ok = companion_mode_tools(app) && crate::policy_guard::sandbox_ready();
    let args = grok_args(
        &sid,
        plug.grok_session_minted,
        soul,
        line,
        tools_ok,
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
                tools_ok,
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

pub(crate) async fn companion_send_hermes(app: &tauri::AppHandle, line: &str) -> Result<String, String> {
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
pub(crate) fn hermes_rejects_safe_mode(e: &str) -> bool {
    let l = e.to_ascii_lowercase();
    l.contains("safe-mode")
        || l.contains("unexpected argument")
        || l.contains("unrecognized")
        || l.contains("unknown option")
}

/// berdctl send (fire-and-forget) then poll session get until a new assistant
/// message appears or timeout. Berd must be running.
pub(crate) fn run_berd_reply(bin: &Path, session_id: &str, line: &str) -> Result<String, String> {
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

pub(crate) struct BerdSnap {
    is_running: bool,
    last_assistant: String,
    message_count: i64,
}

pub(crate) fn berd_session_snapshot(bin: &Path, session_id: &str) -> Result<BerdSnap, String> {
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
pub(crate) fn run_hermes_reply(bin: &Path, args: &[String]) -> Result<String, String> {
    let mut child = crate::policy_guard::prepare_hermes(bin)?
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
pub(crate) fn tool_error_line(tool: &str, stderr: &[u8], stdout: &[u8], code: Option<i32>) -> String {
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

