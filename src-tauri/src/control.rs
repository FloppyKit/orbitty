//! Local control socket (Unix only). Same functions as Tauri invoke.
//! Sock 0600 + optional token. Unreadable allowlist = fail closed.
//! Writes are never companion.* (Spark does not spawn a harness).

use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use tauri::Manager;

#[path = "stars_ctl.rs"]
mod stars_ctl;

pub const ERR_OTHER: i32 = 1;
pub const ERR_AUTH: i32 = 3;
pub const ERR_DENIED: i32 = 4;
pub const ERR_AX: i32 = 5;

/// Set when WE bound the sock, so app-exit cleanup only unlinks our own.
static BOUND_SOCK: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

fn control_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Setup hook: bind + serve on a background thread. Any failure logs once
/// and the app runs on without the control plane.
pub fn start(app: &tauri::AppHandle) {
    let dir = match control_dir(app) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("orbitty control: {e} — control socket disabled");
            return;
        }
    };
    let sock = dir.join("control.sock");
    if sock.exists() {
        let _ = std::fs::remove_file(&sock); // stale sock from a crash
    }
    let listener = match UnixListener::bind(&sock) {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "orbitty control: bind {}: {e} — control socket disabled",
                sock.display()
            );
            return;
        }
    };
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) =
            std::fs::set_permissions(&sock, std::fs::Permissions::from_mode(0o600))
        {
            eprintln!("orbitty control: chmod 0600 {}: {e}", sock.display());
        }
    }
    let _ = BOUND_SOCK.set(sock.clone());
    let handle = app.clone();
    let spawned = std::thread::Builder::new()
        .name("orbitty-control".into())
        .spawn(move || accept_loop(listener, handle));
    if spawned.is_ok() {
        eprintln!("orbitty control: listening on {}", sock.display());
    } else {
        eprintln!("orbitty control: couldn't spawn accept thread — disabled");
    }
}

/// Best-effort app-exit unlink (only a sock we bound ourselves).
pub fn cleanup() {
    if let Some(p) = BOUND_SOCK.get() {
        let _ = std::fs::remove_file(p);
    }
}

fn accept_loop(listener: UnixListener, app: tauri::AppHandle) {
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let app = app.clone();
                // Thread per connection: a 5-minute companion.send must not
                // queue windows.list behind it. Never the UI thread.
                let _ = std::thread::Builder::new()
                    .name("orbitty-control-conn".into())
                    .spawn(move || handle_conn(s, app));
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
}

#[derive(Deserialize)]
struct Request {
    v: Option<u32>,
    id: Option<Value>,
    token: Option<String>,
    tool: Option<String>,
    args: Option<Value>,
}

fn reply_ok(id: &Value, data: Value) -> Value {
    json!({ "v": 1, "id": id, "ok": true, "data": data })
}

fn reply_err(id: &Value, code: i32, msg: &str) -> Value {
    json!({ "v": 1, "id": id, "ok": false, "code": code, "error": msg })
}

fn handle_conn(stream: UnixStream, app: tauri::AppHandle) {
    let reply = match read_request(&stream) {
        Ok(req) => dispatch(&app, req),
        Err((code, msg)) => reply_err(&Value::Null, code, &msg),
    };
    let mut line = serde_json::to_string(&reply).unwrap_or_else(|_| {
        r#"{"v":1,"id":null,"ok":false,"code":1,"error":"serialize reply"}"#.to_string()
    });
    line.push('\n');
    let mut w = &stream;
    let _ = w.write_all(line.as_bytes());
    let _ = w.flush();
}

/// One request = one line of JSON, capped at 1 MiB (requests are tiny;
/// the cap keeps a broken client from streaming forever).
fn read_request(stream: &UnixStream) -> Result<Request, (i32, String)> {
    let mut line = String::new();
    let mut reader = BufReader::new(stream.take(1 << 20));
    reader
        .read_line(&mut line)
        .map_err(|e| (ERR_OTHER, format!("read: {e}")))?;
    let line = line.trim();
    if line.is_empty() {
        return Err((ERR_OTHER, "empty request".into()));
    }
    serde_json::from_str(line).map_err(|e| (ERR_OTHER, format!("bad request JSON: {e}")))
}

/// Configured token, if any: env wins over the file. Empty after trim =
/// not configured.
fn configured_token(dir: &Path) -> Option<String> {
    if let Ok(t) = std::env::var("ORBITTY_CONTROL_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    std::fs::read_to_string(dir.join("control.token"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

enum Acl {
    Full,
    Companion,
    Read,
    Custom(Vec<String>),
}

/// Missing control.json = full (spec default). Present but unreadable /
/// malformed = fail closed.
fn load_acl(dir: &Path) -> Result<Acl, String> {
    let f = dir.join("control.json");
    if !f.exists() {
        return Ok(Acl::Full);
    }
    let raw = std::fs::read_to_string(&f).map_err(|e| format!("read {}: {e}", f.display()))?;
    let v: Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", f.display()))?;
    if let Some(p) = v.get("profile").and_then(|x| x.as_str()) {
        return match p {
            "full" => Ok(Acl::Full),
            "companion" => Ok(Acl::Companion),
            "read" => Ok(Acl::Read),
            other => Err(format!("control.json: unknown profile {other:?}")),
        };
    }
    if let Some(arr) = v.get("allow").and_then(|x| x.as_array()) {
        return Ok(Acl::Custom(
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect(),
        ));
    }
    Err("control.json: need \"profile\" or \"allow\"".into())
}

// The write tools (kanban.add / kanban.move /
// needed.open / log.append) are deliberately NOT here and not companion.* —
// `read` and `companion` profiles deny them (4); only `full` or a named
// `custom` entry allows a write. project.add /
// project.rm / project.rename ride the same rule.
// kanban.notes too.
const READ_TOOLS: &[&str] = &[
    "app.status",
    "app.accessibility_status",
    "windows.list",
    "packs.list",
    "companion.agents",
    "state.get",
    // The list is the read face of the file.
    "action.list",
    // The roster is the read face; add / rm /
    // pin / pickup / kanban.assign / kanban.assign_window stay write-class.
    "agent.list",
    "shift.status",
    "relay.status",
    // Hang readout: armed while the companion window is focused.
    "gate.status",
    // Hang route names only. route.run stays write-class
    // (full / named custom). The companion profile's prefix is
    // companion.*, so it does not reach route.run.
    "route.table",
    "route.classify",
    // Plans folder and GO status are reads. Setting them, sealing,
    // and migrate --apply stay write-class (full / named custom).
    "settings.plans_dir",
    "settings.runner",
    "go.status",
    "plans.migrate",
];

// Stars reads join this list in `read_ok` via stars_ctl::is_read.
// Their writes (checkin.set, log.set, journal.append, …) stay off it.

fn read_ok(tool: &str) -> bool {
    READ_TOOLS.contains(&tool) || stars_ctl::is_read(tool)
}

/// Classify only. Same ids as `mouth_route::classify`. No state write,
/// no event log, no window change.
fn route_classify_value(line: &str) -> Value {
    let tool = crate::mouth_route::classify(line);
    json!({
        "tool": tool,
        "chat": tool.is_none(),
    })
}

fn allowed(acl: &Acl, tool: &str) -> bool {
    match acl {
        // Full = the whole v0 list; unknown names still bounce at dispatch.
        Acl::Full => true,
        Acl::Read => read_ok(tool),
        Acl::Companion => {
            // Spark mints and runs named recipes
            // (action.add / action.run) — never a raw shell, never rm.
            read_ok(tool)
                || tool.starts_with("companion.")
                || tool == "action.run"
                || tool == "action.add"
        }
        // A custom entry names a tool exactly, or a whole prefix with a
        // trailing `*` (e.g. "action.*" reaches all four action tools).
        Acl::Custom(list) => list.iter().any(|t| match t.strip_suffix('*') {
            Some(prefix) => tool.starts_with(prefix),
            None => t == tool,
        }),
    }
}

fn dispatch(app: &tauri::AppHandle, req: Request) -> Value {
    let id = req.id.unwrap_or(Value::Null);
    if let Some(v) = req.v {
        if v != 1 {
            return reply_err(&id, ERR_OTHER, &format!("unsupported protocol v{v}"));
        }
    }
    let tool = req.tool.unwrap_or_default();
    if tool.is_empty() {
        return reply_err(&id, ERR_OTHER, "missing tool");
    }

    let dir = match control_dir(app) {
        Ok(d) => d,
        Err(e) => return reply_err(&id, ERR_OTHER, &e),
    };
    if let Some(want) = configured_token(&dir) {
        let got = req.token.as_deref().map(str::trim).unwrap_or("");
        if got != want {
            return reply_err(&id, ERR_AUTH, "auth failed: valid token required");
        }
    }
    let acl = match load_acl(&dir) {
        Ok(a) => a,
        Err(e) => return reply_err(&id, ERR_DENIED, &e),
    };
    if !allowed(&acl, &tool) {
        return reply_err(
            &id,
            ERR_DENIED,
            &format!("tool {tool:?} denied by control.json"),
        );
    }
    // go.seal is Matt-only: profile full, plus a confirm flag in the handler.
    if tool == "go.seal" && !matches!(acl, Acl::Full) {
        return reply_err(&id, ERR_DENIED, "go.seal requires profile full");
    }

    let args = req.args.unwrap_or(Value::Null);
    match run_tool(app, &tool, &args) {
        Ok(data) => reply_ok(&id, data),
        Err((code, msg)) => reply_err(&id, code, &msg),
    }
}

fn arg_str(args: &Value, key: &str) -> Result<String, (i32, String)> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| (ERR_OTHER, format!("missing arg {key:?}")))
}

/// Optional string arg: absent / non-string / blank-after-trim = None.
fn arg_opt(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// `kanban.add` links: omitted = none. Each item is `{url, label?}`.
fn arg_links(args: &Value) -> Result<Vec<crate::state::CardLink>, (i32, String)> {
    let Some(v) = args.get("links") else {
        return Ok(Vec::new());
    };
    if v.is_null() {
        return Ok(Vec::new());
    }
    let arr = v.as_array().ok_or((
        ERR_OTHER,
        "kanban.add: links must be an array".to_string(),
    ))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let url = item
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or((
                ERR_OTHER,
                "kanban.add: each link needs a string \"url\"".to_string(),
            ))?;
        let label = item
            .get("label")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        out.push(crate::state::CardLink {
            url: url.to_string(),
            label,
        });
    }
    Ok(out)
}

/// AX-gated fns already return "accessibility not granted" — surface that
/// as code 5 (degraded) instead of a generic 1.
fn tool_err(e: String) -> (i32, String) {
    if e.contains("accessibility not granted") {
        (ERR_AX, e)
    } else {
        (ERR_OTHER, e)
    }
}

fn ok_true() -> Value {
    json!({ "ok": true })
}

fn to_data<T: serde::Serialize>(v: T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// Stars writes use the same save door as kanban.add: load, mutate the
/// stars slice or planet trackers, atomic save, `state:changed`.
fn run_stars(app: &tauri::AppHandle, tool: &str, args: &Value) -> Result<Value, (i32, String)> {
    let mut st = crate::state::load_state(app).map_err(tool_err)?;
    let before = st.clone();
    let data = stars_ctl::handle(&mut st, tool, args, &stars_ctl::Clock::now()).map_err(tool_err)?;
    if !stars_ctl::is_read(tool) && stars_ctl::slices_changed(&before, &st) {
        crate::board::save_with_events(app, &before, &st).map_err(tool_err)?;
    }
    Ok(data)
}

/// Missing or non-boolean `run` stays false. ctl and MCP must not default to running a program.
pub(crate) fn kanban_open_run_flag(args: &Value) -> bool {
    let run = args.get("run").and_then(Value::as_bool).unwrap_or(false);
    run
}

/// The v0 tool list — each arm calls the exact function the matching
/// Tauri invoke handler calls. No new product behavior lives here.
fn run_tool(app: &tauri::AppHandle, tool: &str, args: &Value) -> Result<Value, (i32, String)> {
    match tool {
        "app.status" => Ok(json!({
            "ok": true,
            "ax": crate::windows::accessibility_status(),
            "socket": true,
        })),
        "app.accessibility_status" => Ok(to_data(crate::windows::accessibility_status())),
        "windows.list" => crate::windows::list_windows().map(to_data).map_err(tool_err),
        "windows.focus" => {
            let id = arg_str(args, "id")?;
            crate::windows::focus_window(&id).map(|_| ok_true()).map_err(tool_err)
        }
        "windows.minimize" => {
            let id = arg_str(args, "id")?;
            crate::windows::minimize_window(&id).map(|_| ok_true()).map_err(tool_err)
        }
        "windows.restore" => {
            let id = arg_str(args, "id")?;
            crate::windows::restore_window(&id).map(|_| ok_true()).map_err(tool_err)
        }
        "windows.close" => {
            let id = arg_str(args, "id")?;
            crate::windows::close_window(&id).map(|_| ok_true()).map_err(tool_err)
        }
        "companion.open" => crate::panes::show_companion(app).map(|_| ok_true()).map_err(tool_err),
        // One shared 780×520 pane — these pick the
        // page inside it; neither spawns the legacy 420×560 stars window.
        "kanban.open" => crate::panes::show_shared_pane(app, "board").map(|_| ok_true()).map_err(tool_err),
        "stars.open" => crate::panes::show_shared_pane(app, "stars").map(|_| ok_true()).map_err(tool_err),
        "companion.send" => {
            let line = arg_str(args, "line")?;
            // Same Berd path as the composer — the async command body,
            // driven from this plain std thread via the app's runtime.
            // Origin::Companion: a socket companion.send never proposes or
            // confirms a board write (route.run --pending is the socket door).
            tauri::async_runtime::block_on(crate::coordinator::companion_send_from(
                app.clone(),
                line,
                arg_opt(args, "path"),
                crate::mouth_route::Origin::Companion,
            ))
            .map(|reply| json!({ "reply": reply }))
            .map_err(tool_err)
        }
        "companion.agents" => crate::coordinator::companion_agents(app.clone())
            .map(to_data)
            .map_err(tool_err),
        "companion.set_agent" => {
            let profile = arg_str(args, "profile")?;
            crate::coordinator::set_companion_agent(app.clone(), profile)
                .map(to_data)
                .map_err(tool_err)
        }
        // Same companion.* ACL class as companion.set_agent (not a read).
        "companion.harnesses" => crate::companion_backend::companion_harnesses(app.clone())
            .map(to_data)
            .map_err(tool_err),
        "companion.set_backend" => {
            let id = match args.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.trim().to_string(),
                None => {
                    return Err((ERR_OTHER, "missing arg \"id\"".to_string()));
                }
            };
            crate::companion_backend::set_companion_backend(app.clone(), id)
                .map(to_data)
                .map_err(tool_err)
        }
        // Dry run. companion.* ACL, not a read. Never calls a model or writes.
        // The key is not settable on this socket.
        "companion.route" => {
            let line = arg_str(args, "line")?;
            crate::direct_lane::companion_route(app.clone(), line)
                .map(to_data)
                .map_err(tool_err)
        }
        // Same ACL class as companion.send
        // (companion.* — never read-only). Unknown clip = honest error.
        "companion.play" => {
            let clip = arg_str(args, "clip")?;
            crate::companion::companion_play(app, &clip).map_err(tool_err)
        }
        // The bus verbs. companion.* ACL like
        // play/send; unknown modes fail closed at the command.
        "companion.mode" => {
            let mode = arg_str(args, "mode")?;
            crate::coordinator::set_companion_mode(app.clone(), mode)
                .map(to_data)
                .map_err(tool_err)
        }
        "companion.note" => {
            let line = arg_str(args, "line")?;
            crate::coordinator::companion_note(app.clone(), line, arg_opt(args, "mode"))
                .map(to_data)
                .map_err(tool_err)
        }
        "companion.thin" => crate::coordinator::companion_thin(app.clone())
            .map(to_data)
            .map_err(tool_err),
        "companion.now" => {
            let files = args
                .get("files")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                });
            crate::coordinator::companion_now(
                arg_opt(args, "job"),
                arg_opt(args, "last_error"),
                arg_opt(args, "last_green"),
                arg_opt(args, "do_not_retry"),
                files,
            )
            .map(to_data)
            .map_err(tool_err)
        }
        // the Free Mode switch — same
        // companion.* ACL as companion.mode. `enabled` must be a real
        // bool (fail closed, same as companion.hack's `live`).
        "companion.free_mode" => {
            let enabled = args
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or_else(|| {
                    (
                        ERR_OTHER,
                        "companion.free_mode: need bool arg \"enabled\"".to_string(),
                    )
                })?;
            crate::mouth_hop::set_companion_free_mode(app.clone(), enabled)
                .map(|_| json!({ "freeMode": enabled }))
                .map_err(tool_err)
        }
        // The dock LED flag — companion.* ACL like
        // play/send. `live` must be a real bool (fail closed); `who` is
        // an optional source label ("cursor", "goose", …) for the title.
        // optional `projectId` — the live agent's
        // pinned planet, so the dest/planet menus glow it while live.
        "companion.hack" => {
            let live = args
                .get("live")
                .and_then(Value::as_bool)
                .ok_or_else(|| {
                    (ERR_OTHER, "companion.hack: need bool arg \"live\"".to_string())
                })?;
            crate::coordinator::set_companion_hack(
                app.clone(),
                live,
                arg_opt(args, "who"),
                arg_opt(args, "projectId"),
            )
            .map(to_data)
            .map_err(tool_err)
        }
        "packs.list" => crate::packs::list_packs_sync(app.clone())
            .map(to_data)
            .map_err(tool_err),
        "sleeper.wake" => {
            let id = arg_str(args, "id")?;
            crate::coordinator::wake_sleeper(app.clone(), id)
                .map(to_data)
                .map_err(tool_err)
        }
        "state.get" => crate::state::load_state(app).map(to_data).map_err(tool_err),
        // The action recipe file. list = read
        // profile; add/run = companion (Spark mints) and up; rm = full /
        // named custom only. Runs go through the same logged /bin/zsh -lc
        // 120s door as the Settings list click — lengths-only event log.
        "action.list" => crate::actions::action_list(app).map_err(tool_err),
        "action.add" => {
            let name = arg_str(args, "name")?;
            let cmd = arg_str(args, "cmd")?;
            crate::actions::action_add(
                app,
                &name,
                &cmd,
                arg_opt(args, "cwd").as_deref(),
                arg_opt(args, "combo").as_deref(),
            )
            .map(to_data)
            .map_err(tool_err)
        }
        "action.run" => {
            let id = arg_opt(args, "id");
            let name = arg_opt(args, "name");
            crate::actions::action_run(app, id.as_deref(), name.as_deref())
                .map(to_data)
                .map_err(tool_err)
        }
        "action.rm" => {
            let id = arg_str(args, "id")?;
            crate::actions::action_rm(app, &id).map_err(tool_err)
        }
        // The four write tools. Same mutate path as
        // the save_state command (claim diff + state:changed emit) so the
        // UI live-updates. ACL: full / named custom only (see READ_TOOLS).
        "kanban.add" => {
            let title = arg_str(args, "title")?;
            let project = arg_opt(args, "projectId").unwrap_or_default();
            let status = arg_opt(args, "status").unwrap_or_default();
            let links = arg_links(args)?;
            // Optional one-line summary (notes).
            crate::board::kanban_add(
                app,
                &title,
                &project,
                &status,
                arg_opt(args, "notes").as_deref(),
                links,
            )
            .map(to_data)
            .map_err(tool_err)
        }
        // Set a card's notes (first line = the
        // board-row summary). `notes` must be a string; empty clears —
        // same shape as kanban.assign's empty-unassigns. Same write ACL.
        "kanban.notes" => {
            let id = arg_str(args, "id")?;
            let notes = args.get("notes").and_then(Value::as_str).ok_or_else(|| {
                (
                    ERR_OTHER,
                    "kanban.notes: need string arg \"notes\" (empty clears)".to_string(),
                )
            })?;
            crate::board::kanban_notes(app, &id, notes)
                .map(to_data)
                .map_err(tool_err)
        }
        // Append one line to the notes (same write ACL). Unknown to an
        // older app, so a new client never falls back to a replace.
        "kanban.note_append" => {
            let id = arg_str(args, "id")?;
            let line = arg_str(args, "line")?;
            crate::board::kanban_note_append(app, &id, &line)
                .map(to_data)
                .map_err(tool_err)
        }
        // Same write ACL as kanban.notes. Exactly one of add / rm / clear.
        "kanban.link" => {
            let id = arg_str(args, "id")?;
            let clear = args.get("clear").and_then(Value::as_bool).unwrap_or(false);
            let rm = arg_opt(args, "rm");
            let add = arg_opt(args, "add");
            let modes = [clear, rm.is_some(), add.is_some()]
                .into_iter()
                .filter(|on| *on)
                .count();
            if modes != 1 {
                return Err((
                    ERR_OTHER,
                    "kanban.link: pass exactly one of add, rm, clear".into(),
                ));
            }
            if clear {
                crate::board::kanban_link_clear(app, &id)
                    .map(to_data)
                    .map_err(tool_err)
            } else if let Some(url) = rm {
                crate::board::kanban_link_rm(app, &id, &url)
                    .map(to_data)
                    .map_err(tool_err)
            } else {
                let url = add.unwrap_or_default();
                let label = arg_opt(args, "label");
                crate::board::kanban_link_add(app, &id, &url, label)
                    .map(to_data)
                    .map_err(tool_err)
            }
        }
        "kanban.open_link" => {
            let id = arg_str(args, "id")?;
            let n = match args.get("n") {
                None | Some(Value::Null) => 0usize,
                Some(v) => v
                    .as_u64()
                    .ok_or((ERR_OTHER, "kanban.open_link: n must be an integer".to_string()))?
                    as usize,
            };
            let run = kanban_open_run_flag(args);
            crate::board::kanban_open_link(app, &id, n, run)
                .map(to_data)
                .map_err(tool_err)
        }
        "kanban.move" => {
            let id = arg_str(args, "id")?;
            let status = arg_str(args, "status")?;
            crate::board::kanban_move(app, &id, &status)
                .map(to_data)
                .map_err(tool_err)
        }
        // Assign a card to an agent row. `agent`
        // must be present (id or unique name); an empty string unassigns.
        "kanban.assign" => {
            let id = arg_str(args, "id")?;
            let agent = args.get("agent").and_then(Value::as_str).ok_or_else(|| {
                (
                    ERR_OTHER,
                    "kanban.assign: need string arg \"agent\" (empty unassigns)".to_string(),
                )
            })?;
            crate::agents::kanban_assign(app, &id, agent)
                .map(to_data)
                .map_err(tool_err)
        }
        "kanban.assign_window" => {
            let id = arg_str(args, "id")?;
            let window = args.get("windowId").and_then(Value::as_str).ok_or_else(|| {
                (
                    ERR_OTHER,
                    "kanban.assign_window: need string arg \"windowId\" (empty unassigns)".to_string(),
                )
            })?;
            crate::board::kanban_assign_window(app, &id, window)
                .map(|c| {
                    to_data(serde_json::json!({
                        "id": c.id,
                        "title": c.title,
                        "windowId": c.window_id,
                        "status": c.status,
                        "projectId": c.project_id,
                    }))
                })
                .map_err(tool_err)
        }
        // The agent roster. list rides read;
        // add / rm / pin / pickup are writes (full / named custom only).
        // Harness is required on add and allowlisted in agents.rs — an
        // unknown harness is an honest error, never a silent hermes.
        "agent.list" => crate::agents::agent_list(app).map_err(tool_err),
        "agent.add" => {
            let name = arg_str(args, "name")?;
            let harness = arg_str(args, "harness")?;
            let md_paths: Vec<String> = args
                .get("mdPaths")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            crate::agents::agent_add(
                app,
                &name,
                &harness,
                arg_opt(args, "profile").as_deref(),
                arg_opt(args, "model").as_deref(),
                md_paths,
                arg_opt(args, "extra").as_deref(),
                arg_opt(args, "projectId").as_deref(),
                arg_opt(args, "permissions").as_deref(),
            )
            .map(to_data)
            .map_err(tool_err)
        }
        "agent.rm" => {
            let id = arg_str(args, "id")?;
            crate::agents::agent_rm(app, &id).map(to_data).map_err(tool_err)
        }
        "agent.pin" => {
            let id = arg_str(args, "id")?;
            let project = arg_str(args, "projectId")?;
            crate::agents::agent_pin(app, &id, &project)
                .map(to_data)
                .map_err(tool_err)
        }
        "agent.pickup" => {
            let id = arg_opt(args, "id");
            let name = arg_opt(args, "name");
            let card = arg_opt(args, "cardId");
            crate::agents::agent_pickup(app, id.as_deref(), name.as_deref(), card.as_deref())
                .map(to_data)
                .map_err(tool_err)
        }
        "shift.status" => crate::shift::status(app).map_err(tool_err),
        "gate.status" => Ok(to_data(crate::interrupt_gate::interrupt_gate_status())),
        // Hang route. table / classify are read-safe. run is the same
        // door as Hang (`mouth_route::run`, including its hop log) and
        // is write-class. A miss is {tool:null, spoken:null}, not an error.
        // This does not push companion turns or set last_bot_line.
        "route.table" => Ok(json!({
            "table": crate::mouth_route::TABLE,
            "max": 15,
        })),
        "route.classify" => {
            let line = arg_str(args, "line")?;
            Ok(route_classify_value(&line))
        }
        "route.run" => {
            let line = arg_str(args, "line")?;
            // A yes / no only answers this caller's own proposal: it must
            // carry the pendingId that proposal returned.
            let confirm = arg_opt(args, "pendingId");
            let out = crate::mouth_route::run_outcome(
                app,
                &line,
                crate::mouth_route::Origin::Socket,
                confirm.as_deref(),
            );
            Ok(json!({
                "tool": out.tool,
                "spoken": out.spoken,
                "pendingId": out.pending_id,
            }))
        }
        "gate.inject" => {
            let kind = arg_str(args, "kind")?;
            crate::interrupt_gate::interrupt_gate_inject(app.clone(), kind).map_err(tool_err)
        }
        "shift.tick" => crate::shift::tick(app).map_err(tool_err),
        "shift.on" => crate::shift::set_on(app, true).map_err(tool_err),
        "shift.off" => crate::shift::set_on(app, false).map_err(tool_err),
        "relay.status" => crate::event_relay::status(app).map_err(tool_err),
        "relay.on" => crate::event_relay::set_enabled(app, true)
            .map_err(tool_err),
        "relay.off" => crate::event_relay::set_enabled(app, false)
            .map_err(tool_err),
        "relay.replica" => {
            let url = args
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| (ERR_OTHER, "missing arg \"url\"".to_string()))?;
            crate::event_relay::set_replica_url(app, url).map_err(tool_err)
        }
        "needed.open" => {
            let title = arg_str(args, "title")?;
            let project = arg_opt(args, "projectId").unwrap_or_default();
            let why = arg_opt(args, "why");
            crate::board::needed_open(app, &title, &project, why.as_deref())
                .map(to_data)
                .map_err(tool_err)
        }
        // needed.answer — the exact path of
        // the Tauri needed_answer command (same save door, same
        // needed-answer event the watch dispatches on), so orbittyctl and
        // the python wrapper can apply an answer without the UI. Same
        // write-class ACL as needed.open (full / named custom only).
        "needed.answer" => {
            let id = arg_str(args, "id")?;
            let answer = arg_str(args, "answer")?;
            crate::board::needed_answer(app, &id, &answer)
                .map(to_data)
                .map_err(tool_err)
        }
        // Mint / kill / rename planets. Same one
        // save door as the kanban writes (state:changed emit), so the
        // habitat paints the new projects[] on the next sync. ACL: full /
        // named custom only (not in READ_TOOLS, never companion.*).
        "project.add" => {
            let name = arg_str(args, "name")?;
            crate::board::project_add(app, &name)
                .map(to_data)
                .map_err(tool_err)
        }
        "project.rm" => {
            let id = arg_str(args, "id")?;
            crate::board::project_rm(app, &id)
                .map(to_data)
                .map_err(tool_err)
        }
        "project.rename" => {
            let id = arg_str(args, "id")?;
            let name = arg_str(args, "name")?;
            crate::board::project_rename(app, &id, &name)
                .map(to_data)
                .map_err(tool_err)
        }
        "project.set_root" => {
            let id = arg_str(args, "id")?;
            let path = arg_str(args, "path")?;
            crate::task::set_workspace_root(app, &id, &path)
                .map(|_| to_data(serde_json::json!({ "projectId": id, "path": path })))
                .map_err(tool_err)
        }
        "claim.unclaim" => {
            let id = arg_str(args, "windowId")?;
            crate::claim_front::unclaim_window(app, &id)
                .map(|_| ok_true())
                .map_err(tool_err)
        }
        "claim.front" => crate::claim_front::claim_front(app)
            .map(|_| ok_true())
            .map_err(tool_err),
        "claim.undo" => crate::claim_front::undo_last(app)
            .map(|_| ok_true())
            .map_err(tool_err),
        "claim.to" => {
            let id = arg_str(args, "projectId")?;
            crate::claim_front::claim_front_to_planet(app, &id)
                .map(|_| ok_true())
                .map_err(tool_err)
        }
        "claim.change" => {
            let id = arg_str(args, "projectId")?;
            crate::claim_front::change_planet(app, &id)
                .map(|_| ok_true())
                .map_err(tool_err)
        }
        "task.undo" => {
            let id = arg_str(args, "id")?;
            crate::task::undo(app, &id)
                .map(|_| to_data(serde_json::json!({ "cardId": id, "ok": true })))
                .map_err(tool_err)
        }
        other if other.starts_with("stars.") => run_stars(app, other, args),
        "log.append" => {
            let e_type = arg_str(args, "eType")?;
            let content = match args.get("content") {
                None | Some(Value::Null) => None,
                Some(v) if v.is_object() => Some(v.clone()),
                Some(_) => {
                    return Err((ERR_OTHER, "log.append: content must be an object".into()));
                }
            };
            let actor = arg_opt(args, "actor").unwrap_or_else(|| "agent".to_string());
            // Same skip/ack contract as the UI's log_event command: a dead
            // log never fails the caller.
            let ack = crate::event_log::log_event(
                app.clone(),
                e_type,
                None,
                arg_opt(args, "projectId"),
                Some(actor),
                Some(true),
                content,
            );
            Ok(to_data(ack))
        }
        "settings.plans_dir" => crate::plans::settings_snapshot().map_err(tool_err),
        "settings.plans_dir_set" => {
            let path = arg_str(args, "path")?;
            crate::plans::set_plans_dir(&path).map_err(tool_err)
        }
        "settings.runner" => crate::plans::settings_snapshot().map_err(tool_err),
        "settings.runner_set" => {
            let runner = arg_opt(args, "runner").unwrap_or_default();
            let harness = arg_opt(args, "harness").unwrap_or_default();
            let model = arg_opt(args, "model").unwrap_or_default();
            crate::plans::set_runner(&runner, &harness, &model).map_err(tool_err)
        }
        "go.status" => crate::plans::status_value(
            arg_opt(args, "file").as_deref(),
            arg_opt(args, "go").as_deref(),
            arg_opt(args, "card").as_deref(),
            arg_opt(args, "title").as_deref(),
        )
        .map_err(tool_err),
        "go.seal" => {
            let confirm = args.get("confirm").and_then(Value::as_bool).unwrap_or(false);
            if !confirm {
                return Err((ERR_OTHER, "go.seal: pass --confirm".into()));
            }
            let file = arg_str(args, "file")?;
            let by = arg_str(args, "by")?;
            crate::plans::seal_file(std::path::Path::new(&file), &by)
                .map(|g| {
                    to_data(serde_json::json!({
                        "id": g.id(),
                        "file": g.path.display().to_string(),
                        "status": g.header.status,
                        "sealedAt": g.header.sealed_at,
                        "sealedBy": g.header.sealed_by,
                        "sealHash": g.header.seal_hash,
                    }))
                })
                .map_err(tool_err)
        }
        "plans.migrate" => {
            let roots = crate::plans::Roots::real();
            Ok(to_data(serde_json::json!({
                "dryRun": true,
                "files": crate::plans::migrate_list(&roots),
            })))
        }
        "plans.migrate_apply" => {
            let roots = crate::plans::Roots::real();
            crate::plans::migrate_apply(&roots).map_err(tool_err)
        }
        other => Err((ERR_OTHER, format!("unknown tool {other:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phrases already covered by `mouth_route` tests. This checks the
    /// socket payload, not a second classifier.
    const PHRASES: &[&str] = &[
        "tell me about the todo list on planet orbitty",
        "what's on the board",
        "open planet orbitty",
        "show planet clinic",
        "note: mouth ping",
        "gate status",
        "shift status",
        "list planets",
        "list agents",
        "needed list on planet orbitty",
        "add a card Buy milk to planet Kansas todo",
        "add an agent in doing to planet Kansas",
        "tell me a joke",
        "STOP",
        "stop",
        "ping",
        "how is the weather",
    ];

    #[test]
    fn route_classify_matches_mouth_route() {
        for line in PHRASES {
            let got = route_classify_value(line);
            let expect = crate::mouth_route::classify(line);
            assert_eq!(got["tool"].as_str(), expect, "{line}");
            assert_eq!(got["chat"].as_bool(), Some(expect.is_none()), "{line}");
        }
    }

    #[test]
    fn route_classify_is_read_and_route_run_is_not() {
        assert!(READ_TOOLS.contains(&"route.table"));
        assert!(READ_TOOLS.contains(&"route.classify"));
        assert!(!READ_TOOLS.contains(&"route.run"));
        assert!(read_ok("route.classify"));
        assert!(!read_ok("route.run"));
        assert!(!READ_TOOLS.contains(&"companion.route"));
        assert!(!read_ok("companion.route"));
        assert!(!allowed(&Acl::Read, "companion.route"));
        assert!(allowed(&Acl::Companion, "companion.route"));
        assert!(allowed(&Acl::Full, "companion.route"));
        assert!(allowed(&Acl::Read, "route.classify"));
        assert!(allowed(&Acl::Read, "route.table"));
        assert!(!allowed(&Acl::Read, "route.run"));
        assert!(allowed(&Acl::Companion, "route.classify"));
        assert!(!allowed(&Acl::Companion, "route.run"));
        assert!(allowed(&Acl::Full, "route.run"));
    }

    #[test]
    fn kanban_open_link_run_defaults_false() {
        assert!(!kanban_open_run_flag(&serde_json::json!({})));
        assert!(!kanban_open_run_flag(&serde_json::json!({"id": "c", "run": false})));
        assert!(kanban_open_run_flag(&serde_json::json!({"run": true})));
        assert!(!kanban_open_run_flag(&serde_json::json!({"run": "yes"})));
        let src = include_str!("control.rs");
        assert!(src.contains("let run = kanban_open_run_flag(args);"));
    }
}
