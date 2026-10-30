//! Local control socket (Unix only). Same functions as Tauri invoke.
//! Sock 0600 + optional token. Unreadable allowlist = fail closed.
//! Writes are never companion.* (Spark does not spawn a harness).

use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use tauri::Manager;

use crate::stars::stars_ctl;

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
    /// Top-level caller. Not read from `args`, so a tool argument cannot
    /// impersonate the socket client. Missing or blank becomes `"socket"`.
    #[serde(default)]
    caller: Option<String>,
}

fn claim_origin_from(raw: Option<&str>) -> crate::claim_front::ClaimOrigin {
    let trimmed = raw.unwrap_or("").trim();
    let name = if trimmed.is_empty() {
        "socket".to_string()
    } else {
        trimmed.chars().take(128).collect()
    };
    crate::claim_front::ClaimOrigin::Agent(name)
}

/// Claim-caller pins live here, not at the tail of `mod tests`, so a
/// later append in that module (kanban open-link) does not collide.
#[cfg(test)]
mod claim_caller_tests {
    use super::*;

    fn strip_rust_comments(src: &str) -> String {
        let chars: Vec<char> = src.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '"' {
                out.push('"');
                i += 1;
                while i < chars.len() {
                    let c = chars[i];
                    out.push(c);
                    i += 1;
                    if c == '\\' && i < chars.len() {
                        out.push(chars[i]);
                        i += 1;
                        continue;
                    }
                    if c == '"' {
                        break;
                    }
                }
                continue;
            }
            if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                i += 2;
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                if i + 1 < chars.len() {
                    i += 2;
                }
                continue;
            }
            out.push(chars[i]);
            i += 1;
        }
        out
    }

    /// T8. Caller is the top-level request field. Claim arms never mint User.
    #[test]
    fn claim_caller_is_top_level_only() {
        let req: Request = serde_json::from_str(
            r#"{"v":1,"tool":"claim.undo","caller":"  mcp:cursor:9  ","args":{"caller":"spoof","projectId":"p"}}"#,
        )
        .unwrap();
        assert_eq!(
            claim_origin_from(req.caller.as_deref()),
            crate::claim_front::ClaimOrigin::Agent("mcp:cursor:9".into())
        );
        assert_eq!(req.args.unwrap()["caller"], "spoof");

        let missing: Request =
            serde_json::from_str(r#"{"tool":"claim.undo","args":{"caller":"spoof"}}"#).unwrap();
        assert_eq!(
            claim_origin_from(missing.caller.as_deref()),
            crate::claim_front::ClaimOrigin::Agent("socket".into())
        );

        let empty: Request =
            serde_json::from_str(r#"{"caller":"   ","tool":"claim.front"}"#).unwrap();
        assert_eq!(
            claim_origin_from(empty.caller.as_deref()),
            crate::claim_front::ClaimOrigin::Agent("socket".into())
        );

        assert_eq!(
            claim_origin_from(Some("user")),
            crate::claim_front::ClaimOrigin::Agent("user".into())
        );

        let long = "y".repeat(200);
        match claim_origin_from(Some(&long)) {
            crate::claim_front::ClaimOrigin::Agent(s) => assert_eq!(s.chars().count(), 128),
            crate::claim_front::ClaimOrigin::User => panic!("caller must stay Agent"),
        }

        let stripped = strip_rust_comments(include_str!("control.rs"));
        // Newline anchor: this module sits above `run_tool`, so a bare
        // `fn run_tool(` would hit the string in this test first.
        let start = stripped.find("\nfn run_tool(").expect("run_tool");
        let body = &stripped[start..];
        let claim_at = body.find("\"claim.").expect("claim arm");
        let end = body[claim_at..]
            .find("\"task.undo\"")
            .expect("task.undo follows claim arms");
        let region = &body[claim_at..claim_at + end];
        assert!(
            !region.contains("ClaimOrigin::User"),
            "claim arms must not construct a User origin:\n{region}"
        );
    }
}

fn reply_ok(id: &Value, data: Value) -> Value {
    json!({ "v": 1, "id": id, "ok": true, "data": data })
}

fn reply_err(id: &Value, code: i32, msg: &str) -> Value {
    json!({ "v": 1, "id": id, "ok": false, "code": code, "error": msg })
}

fn handle_conn(stream: UnixStream, app: tauri::AppHandle) {
    let reply = match read_request(&stream) {
        Ok(req) => crate::state::with_actor("ctl", || dispatch(&app, req)),
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
    "go.run_status",
    "go.run_result",
    "go.runner_get",
    "plans.migrate",
    "rail.state",
    // history.checkpoint writes a file and a 426 line, so it stays off,
    // as do history.rewind / repair / keep.
    "history.list",
    "history.drift",
    // Local replay cursor. Flag off answers {"enabled":false}.
    "events.since",
    "tabbity.hello",
    "tabbity.planets",
    // The list is the read face. flags.set stays write-class (socket only).
    "flags.list",
    // Archive reads. companion.roll stays write-class (manual_test only).
    "companion.archive_tail",
    "companion.resets",
    "companion.recall",
    "herdr.status",
    "herdr.peek",
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

    let origin = claim_origin_from(req.caller.as_deref());
    let args = req.args.unwrap_or(Value::Null);
    match run_tool(app, &tool, &args, &origin) {
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

fn arg_usize(args: &Value, key: &str, default: usize) -> usize {
    let Some(v) = args.get(key) else {
        return default;
    };
    if let Some(n) = v.as_u64() {
        return n as usize;
    }
    v.as_str()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(default)
}

/// Optional string arg: absent / non-string / blank-after-trim = None.
fn arg_opt(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn tag_names(args: &Value) -> Result<Vec<String>, (i32, String)> {
    if let Some(arr) = args.get("names").and_then(Value::as_array) {
        let names: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect();
        if names.is_empty() {
            return Err((ERR_OTHER, "companion.tags_propose: need names".into()));
        }
        return Ok(names);
    }
    let raw = arg_str(args, "names")?;
    let names: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        return Err((ERR_OTHER, "companion.tags_propose: need names".into()));
    }
    Ok(names)
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

/// `history.rewind` / `repair` / `keep`. Without `confirm: true` each
/// returns what it would do and writes nothing.
fn history_write(app: &tauri::AppHandle, tool: &str, args: &Value) -> Result<Value, (i32, String)> {
    use crate::relay::rewind::{self, Req};
    let confirm = args.get("confirm").and_then(Value::as_bool).unwrap_or(false);
    let req = match tool {
        "history.rewind" => Req::Rewind {
            to: arg_str(args, "to")?,
            card: arg_opt(args, "card"),
            remove_added: args.get("removeAdded").and_then(Value::as_bool).unwrap_or(false),
        },
        "history.repair" => Req::Repair,
        _ => {
            return match confirm {
                false => rewind::check(app).map(|d| json!({ "dryRun": true, "drift": d })),
                true => rewind::keep(app).map(|a| json!({ "dryRun": false, "checkpoint": a })),
            }
            .map_err(tool_err);
        }
    };
    match (confirm, &req) {
        (false, _) => rewind::plan(app, &req),
        (true, Req::Repair) => rewind::repair(app),
        (true, _) => rewind::apply(app, &req),
    }
    .map(|p| json!({ "dryRun": !confirm, "plan": p }))
    .map_err(tool_err)
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
fn run_tool(
    app: &tauri::AppHandle,
    tool: &str,
    args: &Value,
    origin: &crate::claim_front::ClaimOrigin,
) -> Result<Value, (i32, String)> {
    match tool {
        "app.status" => Ok(json!({
            "ok": true,
            "ax": crate::windows::accessibility_status(),
            "socket": true,
        })),
        "rail.state" => Ok(crate::rail_edge::status_json(app)),
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
        "companion.archive_tail" => {
            let n = arg_usize(args, "n", 20);
            let date = arg_opt(args, "date");
            let root = crate::coordinator::companions_root();
            let lines = crate::mouth_archive::tail(&root, n, date.as_deref());
            Ok(json!({ "lines": lines }))
        }
        "companion.resets" => {
            let n = arg_usize(args, "n", 20);
            let root = crate::coordinator::companions_root();
            let lines = crate::mouth_archive::resets_tail(&root, n);
            Ok(json!({ "lines": lines }))
        }
        "companion.roll" => {
            let reason = arg_str(args, "reason")?;
            if reason != "manual_test" {
                return Err((
                    ERR_OTHER,
                    "companion.roll: reason must be manual_test".into(),
                ));
            }
            crate::mouth_archive::arm_manual_roll()
                .map(|_| json!({ "reason": "manual_test", "pending": true }))
                .map_err(tool_err)
        }
        "companion.recall" => {
            let query = arg_str(args, "query")?;
            crate::companion::memory::recall_for_app(app, &query, arg_opt(args, "planet").as_deref())
                .map_err(tool_err)
        }
        "companion.tags_propose" => {
            let names = tag_names(args)?;
            crate::companion::memory::propose_for_app(app, &names, arg_opt(args, "planet"))
                .map_err(tool_err)
        }
        "companion.tags_accept" => {
            crate::companion::memory::accept_for_app(app, arg_opt(args, "planet").as_deref())
                .map_err(tool_err)
        }
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
        "verify.close" => {
            let id = arg_str(args, "id")?;
            crate::verify_close::arm(app.clone(), id, None);
            Ok(json!({ "armed": true }))
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
        "history.list" => {
            let limit = match args.get("limit") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_u64().ok_or((
                    ERR_OTHER,
                    "history.list: limit must be an integer".to_string(),
                ))? as usize),
            };
            let card = arg_opt(args, "card");
            let project = arg_opt(args, "project");
            crate::relay::history::list(app, card.as_deref(), project.as_deref(), limit)
                .map(to_data)
                .map_err(tool_err)
        }
        "events.since" => {
            if !crate::flags::on(app, "event_seq") {
                return Ok(json!({ "enabled": false }));
            }
            let after = args.get("after").and_then(Value::as_u64).ok_or((
                ERR_OTHER,
                "events.since needs integer after".to_string(),
            ))?;
            let limit = match args.get("limit") {
                None | Some(Value::Null) => 500usize,
                Some(v) => {
                    let n = v.as_u64().ok_or((
                        ERR_OTHER,
                        "events.since: limit must be an integer".to_string(),
                    ))? as usize;
                    if n == 0 || n > 500 {
                        return Err((
                            ERR_OTHER,
                            "events.since: limit must be 1-500".to_string(),
                        ));
                    }
                    n
                }
            };
            let path = crate::event_log::events_path(app).map_err(tool_err)?;
            let (lines, next_after) =
                crate::relay::event_seq::since(&path, after, limit).map_err(tool_err)?;
            Ok(json!({
                "enabled": true,
                "lines": lines,
                "next_after": next_after,
            }))
        }
        "history.checkpoint" => crate::relay::history::checkpoint(app, None)
            .map(to_data)
            .map_err(tool_err),
        "history.drift" => crate::relay::rewind::check(app).map(to_data).map_err(tool_err),
        "history.rewind" | "history.repair" | "history.keep" => history_write(app, tool, args),
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
        "claim.front" => crate::claim_front::claim_front(app, origin)
            .map(|_| ok_true())
            .map_err(tool_err),
        "claim.undo" => crate::claim_front::undo_last(app, origin)
            .map(|_| ok_true())
            .map_err(tool_err),
        "claim.to" => {
            let id = arg_str(args, "projectId")?;
            crate::claim_front::claim_front_to_planet_origin(app, &id, origin)
                .map(|_| ok_true())
                .map_err(tool_err)
        }
        "claim.change" => {
            let id = arg_str(args, "projectId")?;
            crate::claim_front::change_planet(app, &id, origin)
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
        "go.start" => {
            if let Some(msg) = crate::flags::blocked(app, "go_start", "go.start") {
                if let Ok(card) = arg_str(args, "card") {
                    let _ = crate::board::kanban_note_append(app, &card, &msg);
                }
                return Err(tool_err(msg));
            }
            let card = arg_str(args, "card")?;
            match crate::plans::prepare_start(&card) {
                Err(reason) => {
                    let _ = crate::board::kanban_note_append(app, &card, &reason);
                    Err(tool_err(reason))
                }
                Ok(plan) => crate::plans::spawn_plan(&plan).map_err(tool_err),
            }
        }
        "go.run_status" => {
            let id = arg_str(args, "run_id")?;
            crate::plans::read_run_state(&id).map_err(tool_err)
        }
        "go.run_result" => {
            let id = arg_str(args, "run_id")?;
            crate::plans::run_result_value(&id).map_err(tool_err)
        }
        "go.runner_get" => Ok(crate::plans::runner_get_value(&crate::plans::Roots::real())),
        "go.runner_set" => {
            crate::plans::apply_runner_set(&crate::plans::Roots::real(), args).map_err(tool_err)
        }
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
        // flags.set is write-class. The mouth child is not a separate caller
        // identity (claim_origin_from always yields Agent), so the fence is
        // this ACL plus policy_paths on the flag files. No new identity.
        "herdr.status" => Ok(to_data(crate::herdr::status_for(app))),
        "herdr.peek" => {
            let card = arg_opt(args, "card")
                .or_else(|| arg_opt(args, "run"))
                .unwrap_or_default();
            let lines = args
                .get("lines")
                .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
                .unwrap_or(40) as u32;
            Ok(to_data(crate::herdr::peek_for(app, &card, lines)))
        }
        "flags.list" => Ok(to_data(crate::flags::list_app(app))),
        "flags.set" => {
            let id = arg_str(args, "id")?;
            let on = args.get("on").and_then(Value::as_bool).ok_or_else(|| {
                (ERR_OTHER, "flags.set needs bool on".to_string())
            })?;
            crate::flags::set(app, &id, on).map(to_data).map_err(tool_err)
        }
        "flags.auto" => {
            let on = args.get("on").and_then(Value::as_bool).ok_or_else(|| {
                (ERR_OTHER, "flags.auto needs bool on".to_string())
            })?;
            crate::flags::set_auto(app, on).map(to_data).map_err(tool_err)
        }
        tool @ ("tabbity.hello"
        | "tabbity.planets"
        | "tabbity.claim"
        | "tabbity.unclaim"
        | "tabbity.update"
        | "tabbity.poll"
        | "tabbity.ack") => {
            // Nightly only. Stable has no host and answers unknown tool.
            #[cfg(feature = "dev-thin")]
            {
                crate::tabbity::handle(app, tool, args).map_err(tool_err)
            }
            #[cfg(not(feature = "dev-thin"))]
            {
                let _ = (app, args);
                Err(unknown_tool(tool))
            }
        }
        other => Err(unknown_tool(other)),
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
    fn tabbity_reads_are_read_and_writes_are_not() {
        assert!(READ_TOOLS.contains(&"tabbity.hello"));
        assert!(READ_TOOLS.contains(&"tabbity.planets"));
        assert!(allowed(&Acl::Read, "tabbity.hello"));
        assert!(allowed(&Acl::Read, "tabbity.planets"));
        for tool in [
            "tabbity.claim",
            "tabbity.unclaim",
            "tabbity.update",
            "tabbity.poll",
            "tabbity.ack",
        ] {
            assert!(!READ_TOOLS.contains(&tool), "{tool}");
            assert!(!allowed(&Acl::Read, tool), "{tool}");
            assert!(!allowed(&Acl::Companion, tool), "{tool}");
            assert!(allowed(&Acl::Full, tool), "{tool}");
        }
        let src = include_str!("../tabbity/mod.rs");
        for bad in ["plans::", "kanban", "companion", "settings", "go_"] {
            assert!(!src.contains(bad), "{bad}");
        }
    }

    #[test]
    fn history_list_is_read_and_checkpoint_is_full_only() {
        assert!(READ_TOOLS.contains(&"history.list"));
        assert!(!READ_TOOLS.contains(&"history.checkpoint"));
        for acl in [Acl::Read, Acl::Companion, Acl::Full] {
            assert!(allowed(&acl, "history.list"));
        }
        assert!(!allowed(&Acl::Read, "history.checkpoint"));
        assert!(!allowed(&Acl::Companion, "history.checkpoint"));
        assert!(allowed(&Acl::Full, "history.checkpoint"));
        assert!(allowed(&Acl::Custom(vec!["history.*".into()]), "history.checkpoint"));
        assert!(!allowed(&Acl::Custom(vec!["history.list".into()]), "history.checkpoint"));
    }

    #[test]
    fn history_drift_is_read_and_its_writers_are_full_only() {
        for acl in [Acl::Read, Acl::Companion, Acl::Full] {
            assert!(allowed(&acl, "history.drift"));
        }
        for tool in ["history.rewind", "history.repair", "history.keep"] {
            assert!(!READ_TOOLS.contains(&tool), "{tool}");
            assert!(!allowed(&Acl::Read, tool), "{tool}");
            assert!(!allowed(&Acl::Companion, tool), "{tool}");
            assert!(allowed(&Acl::Full, tool), "{tool}");
        }
        assert!(!allowed(&Acl::Custom(vec!["history.drift".into()]), "history.repair"));
        let src = include_str!("control.rs");
        let body = &src[src.find("fn history_write(").unwrap()..];
        let body = &body[..body.find("\n}\n").unwrap()];
        assert!(body.contains("unwrap_or(false)"), "confirm defaults to a dry run");
        assert!(body.contains("(false, _) => rewind::plan(app, &req)"));
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

    fn dispatch_arms(src: &str) -> Vec<String> {
        let mut names = Vec::new();
        for line in src.lines() {
            let t = line.trim_start();
            let Some(rest) = t.strip_prefix('"') else {
                continue;
            };
            let Some(end) = rest.find('"') else {
                continue;
            };
            let name = &rest[..end];
            if !name.contains('.') {
                continue;
            }
            if rest[end + 1..].trim_start().starts_with("=>") {
                names.push(name.to_string());
            }
        }
        names
    }

    #[test]
    fn go_reads_are_read_and_start_and_runner_set_are_not() {
        for tool in ["go.run_status", "go.run_result", "go.runner_get", "go.status"] {
            assert!(READ_TOOLS.contains(&tool), "{tool}");
            assert!(allowed(&Acl::Read, tool), "{tool}");
            assert!(allowed(&Acl::Companion, tool), "{tool}");
        }
        for tool in ["go.start", "go.runner_set"] {
            assert!(!READ_TOOLS.contains(&tool), "{tool}");
            assert!(!allowed(&Acl::Read, tool), "{tool}");
            assert!(!allowed(&Acl::Companion, tool), "{tool}");
            assert!(allowed(&Acl::Full, tool), "{tool}");
        }
    }

    #[test]
    fn herdr_reads_are_read() {
        for tool in ["herdr.status", "herdr.peek"] {
            assert!(READ_TOOLS.contains(&tool), "{tool}");
            assert!(allowed(&Acl::Read, tool), "{tool}");
            assert!(allowed(&Acl::Companion, tool), "{tool}");
        }
    }

    #[test]
    fn flags_list_is_read_flags_set_is_not() {
        assert!(READ_TOOLS.contains(&"flags.list"));
        assert!(!READ_TOOLS.contains(&"flags.set"));
        assert!(!READ_TOOLS.contains(&"flags.auto"));
        assert!(allowed(&Acl::Read, "flags.list"));
        assert!(allowed(&Acl::Companion, "flags.list"));
        assert!(!allowed(&Acl::Read, "flags.set"));
        assert!(!allowed(&Acl::Companion, "flags.set"));
        assert!(!allowed(&Acl::Read, "flags.auto"));
        assert!(!allowed(&Acl::Companion, "flags.auto"));
        assert!(allowed(&Acl::Full, "flags.set"));
        assert!(allowed(&Acl::Full, "flags.auto"));
    }

    #[test]
    fn events_since_is_read() {
        assert!(READ_TOOLS.contains(&"events.since"));
        assert!(allowed(&Acl::Read, "events.since"));
        assert!(allowed(&Acl::Companion, "events.since"));
        assert!(allowed(&Acl::Full, "events.since"));
    }

    #[test]
    fn no_arm_writes_permissions_or_approves_a_runner() {
        let arms = dispatch_arms(include_str!("control.rs"));
        for name in &arms {
            let n = name.to_ascii_lowercase();
            assert!(
                !n.contains("permission")
                    && !n.contains("developer")
                    && !n.contains("approve")
                    && !n.contains("runner_add"),
                "{name}"
            );
        }
        let raw = serde_json::json!({"tool": "go.permissions_set"});
        let tool = raw["tool"].as_str().unwrap();
        assert!(
            !arms.iter().any(|a| a == tool),
            "raw control request {tool} must not match an arm"
        );
        let (code, msg) = unknown_tool(tool);
        assert_eq!(code, ERR_OTHER);
        assert!(msg.contains("unknown tool"), "{msg}");
    }
}

fn unknown_tool(tool: &str) -> (i32, String) {
    (ERR_OTHER, format!("unknown tool {tool:?}"))
}
