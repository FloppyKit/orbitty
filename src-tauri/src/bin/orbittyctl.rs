//! orbittyctl — CLI for the Orbitty control
//! socket. Pure std client: no Tauri, no WebView, no model, no shell.
//! One newline-JSON request → one reply.
//!
//! Exit codes: 0 ok · 2 Orbitty not running · 3 auth · 4 tool denied ·
//! 5 AX/degraded · 1 other.

#[cfg(unix)]
mod imp {
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::exit;

const EXIT_OTHER: i32 = 1;
const EXIT_NOT_RUNNING: i32 = 2;

const USAGE: &str = "orbittyctl — drive a running Orbitty over its local control socket

usage: orbittyctl [--json] [--token T] [--sock PATH] <command>

commands:
  status                          app health + AX summary
  windows list
  windows focus|minimize|restore|close --id <id>
  companion open
  companion send [--file PATH] -- \"line\"
  companion agents
  companion set-agent -- <profile>
  companion play -- <clip>         idle · think · nod · look-left · look-right
  companion mode -- spark|chat|work|hold
  companion free-mode -- on|off
  companion note [--mode spark|chat|work|hold] -- \"line\"
  companion thin                    label this chat thin (mouth keeps going)
  companion now [-- '{\"job\":\"…\"}']   current-job packet (now.json)
  companion hack -- on|off [--who cursor] [--project P]
  kanban open
  kanban add --title \"t\" [--project P] [--status todo|doing] [--notes \"n\"]
  kanban move --id <id> --status todo|doing|done
  kanban notes --id <card> -- \"text\"      empty \"\" clears
  kanban assign --id <card> --agent <agent>   empty --agent \"\" unassigns
  agent list
  agent add --name N --harness cursor|goose|grok|hermes [--profile P]
            [--model M] [--md PATH]... [--extra TEXT] [--project P]
  agent rm --id <id>
  agent pin --id <id> --project P
  agent pickup --id <id> | --name <name> [--card <card>]
  shift status
  shift tick
  shift on|off
  gate status
  gate inject CHATTER|STOP
  relay status
  relay replica --url ws://100.x:13120   empty --url \"\" = local only
  project add --name N
  claim front                         claim frontmost non-Orbitty window
  claim unclaim --id <windowId>        drop a claim (Layer-0 cleanup)
  claim undo                          undo last claim (~3s window)
  claim to --project <id>             claim front window to a planet
  claim change --project <id>         retarget pending claim
  project rm --id <id>
  project rename --id <id> --name <new>
  project set-root --id <id> --cwd PATH
  task undo --id <card>
  action list
  action add --name N --cmd \"C\" [--cwd D] [--combo cmd+alt+m]
  action run --id <id> | --name <name>
  action rm --id <id>
  needed open --title \"t\" [--project P] [--why \"w\"]
  needed answer --id <id> -- \"text\"
  log append --type T [--project P] [--actor agent] -- '{\"k\":1}'
  stars open
  packs list
  sleeper wake --id <id>
  state get

flags:
  --json        machine JSON on stdout (data, or the full error object)
  --token T     auth token (else $ORBITTY_CONTROL_TOKEN, else control.token)
  --sock PATH   socket override (else $ORBITTY_CONTROL_SOCK, else
                ~/Library/Application Support/space.orbitty.mac/control.sock)

The four write verbs (kanban add/move, needed open,
log append) mutate through the app's one save door — the UI live-updates.
ACL: control.json profile full, or a named custom allowlist entry; read /
companion are denied (exit 4).

kanban add takes --notes (the board row's one-line
summary; first line paints, the drawer edits the rest) and kanban notes
sets/clears it on an existing card — same save door, same write ACL.

project add / rm / rename mint, kill, and rename
planets through the same save door (habitat paints whatever projects[]
holds). Empty --name refused; duplicate display names allowed (ids are
unique). project rm keeps cards' projectId and live claims — walkers
vanish with the disc on the next habitat sync. Same ACL as kanban.add.

action list is read-safe; the companion profile
may action add / action run (Spark mints named recipes — never a raw
shell); action rm needs full or a custom entry naming it (or \"action.*\").
Combos register globally on persist; reserved (cmd+shift+o, cmd+space) or
taken chords show [conflict] in action list.

agent add mints a row (--harness required —
cursor|goose|grok|hermes, never a silent default), agent pin sets its
planet, kanban assign writes the card's assignedAgentId (empty --agent
unassigns), and agent pickup moves the oldest assigned todo to doing and
launches the row's harness with that title (every harness lights the dock
LED while the run is live). agent list is read-safe; the rest are writes
(full / named custom).

shift status is read-safe. shift tick parks ghost working
rows (they eat the runner cap), Needed-escalates blocked, and pickups
the next assigned todo while Work/Hold is on. shift on starts a 15s
ticker; off stops it. Spark/Chat never auto-pickup.

gate status is read-safe. Hang is companion focus: click the
companion and status reports armed; click anywhere else and it reports
disarmed, and a freeze clears. gate inject CHATTER holds (no freeze).
gate inject STOP aborts, freezes shift pickup, and stops live working
runs — write-class ACL.

relay status is read-safe. relay replica --url sets the
optional one-user VPS URL (Tailscale 100.64/10:13120 or
ws://127.0.0.1:port for SSH -L). Empty URL = local sidecar only.
Never 0.0.0.0. Write-class ACL (full / named custom).

needed answer --id n-… -- \"text\" applies an
answer through the same needed-answer event path as the composer (the
watch dispatches on it) — scripts never need the UI. needed open itself
now consults the user-minted standing rules first: a can-run hit is born
answered (actor \"rule\"); a miss/collide still pings.

exit codes: 0 ok · 2 Orbitty not running · 3 auth · 4 denied · 5 AX · 1 other";

struct Opts {
    json: bool,
    token: Option<String>,
    sock: Option<PathBuf>,
    id: Option<String>,
    title: Option<String>,
    project: Option<String>,
    status: Option<String>,
    notes: Option<String>,
    why: Option<String>,
    etype: Option<String>,
    actor: Option<String>,
    name: Option<String>,
    cmd: Option<String>,
    cwd: Option<String>,
    combo: Option<String>,
    mode: Option<String>,
    who: Option<String>,
    // Agent roster flags. `md` is repeatable.
    harness: Option<String>,
    profile: Option<String>,
    model: Option<String>,
    md: Vec<String>,
    extra: Option<String>,
    agent: Option<String>,
    /// kanban assign --window (empty clears).
    window: Option<String>,
    card: Option<String>,
    file: Option<String>,
    url: Option<String>,
    words: Vec<String>,
    tail: Vec<String>,
}

fn parse(argv: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        json: false,
        token: None,
        sock: None,
        id: None,
        title: None,
        project: None,
        status: None,
        notes: None,
        why: None,
        etype: None,
        actor: None,
        name: None,
        cmd: None,
        cwd: None,
        combo: None,
        mode: None,
        who: None,
        harness: None,
        profile: None,
        model: None,
        md: Vec::new(),
        extra: None,
        agent: None,
        window: None,
        card: None,
        file: None,
        url: None,
        words: Vec::new(),
        tail: Vec::new(),
    };
    let mut it = argv.iter();
    let mut after_ddash = false;
    while let Some(a) = it.next() {
        if after_ddash {
            o.tail.push(a.clone());
            continue;
        }
        match a.as_str() {
            "--" => after_ddash = true,
            "--json" => o.json = true,
            "--token" => {
                o.token = Some(it.next().ok_or("--token needs a value")?.clone())
            }
            "--sock" => {
                o.sock = Some(PathBuf::from(it.next().ok_or("--sock needs a path")?))
            }
            "--id" => o.id = Some(it.next().ok_or("--id needs a value")?.clone()),
            "--title" => o.title = Some(it.next().ok_or("--title needs a value")?.clone()),
            "--project" => {
                o.project = Some(it.next().ok_or("--project needs a value")?.clone())
            }
            "--status" => {
                o.status = Some(it.next().ok_or("--status needs a value")?.clone())
            }
            "--notes" => o.notes = Some(it.next().ok_or("--notes needs a value")?.clone()),
            "--why" => o.why = Some(it.next().ok_or("--why needs a value")?.clone()),
            "--type" => o.etype = Some(it.next().ok_or("--type needs a value")?.clone()),
            "--actor" => o.actor = Some(it.next().ok_or("--actor needs a value")?.clone()),
            "--name" => o.name = Some(it.next().ok_or("--name needs a value")?.clone()),
            "--cmd" => o.cmd = Some(it.next().ok_or("--cmd needs a value")?.clone()),
            "--cwd" => o.cwd = Some(it.next().ok_or("--cwd needs a value")?.clone()),
            "--combo" => {
                o.combo = Some(it.next().ok_or("--combo needs a value")?.clone())
            }
            "--mode" => o.mode = Some(it.next().ok_or("--mode needs a value")?.clone()),
            "--who" => o.who = Some(it.next().ok_or("--who needs a value")?.clone()),
            "--harness" => {
                o.harness = Some(it.next().ok_or("--harness needs a value")?.clone())
            }
            "--profile" => {
                o.profile = Some(it.next().ok_or("--profile needs a value")?.clone())
            }
            "--model" => o.model = Some(it.next().ok_or("--model needs a value")?.clone()),
            "--md" => o.md.push(it.next().ok_or("--md needs a path")?.clone()),
            "--extra" => o.extra = Some(it.next().ok_or("--extra needs a value")?.clone()),
            // Kept as Some("") when passed empty — kanban assign unassigns.
            "--agent" => o.agent = Some(it.next().ok_or("--agent needs a value")?.clone()),
            "--window" => o.window = Some(it.next().ok_or("--window needs a value")?.clone()),
            "--card" => o.card = Some(it.next().ok_or("--card needs a value")?.clone()),
            "--file" => o.file = Some(it.next().ok_or("--file needs a path")?.clone()),
            // Kept as Some("") when passed empty — relay replica clears.
            "--url" => o.url = Some(it.next().ok_or("--url needs a value")?.clone()),
            "-h" | "--help" => {
                println!("{USAGE}");
                exit(0);
            }
            s if s.starts_with("--token=") => o.token = Some(s["--token=".len()..].to_string()),
            s if s.starts_with("--sock=") => o.sock = Some(PathBuf::from(&s["--sock=".len()..])),
            s if s.starts_with("--id=") => o.id = Some(s["--id=".len()..].to_string()),
            s if s.starts_with("--title=") => o.title = Some(s["--title=".len()..].to_string()),
            s if s.starts_with("--project=") => {
                o.project = Some(s["--project=".len()..].to_string())
            }
            s if s.starts_with("--status=") => o.status = Some(s["--status=".len()..].to_string()),
            s if s.starts_with("--notes=") => o.notes = Some(s["--notes=".len()..].to_string()),
            s if s.starts_with("--why=") => o.why = Some(s["--why=".len()..].to_string()),
            s if s.starts_with("--type=") => o.etype = Some(s["--type=".len()..].to_string()),
            s if s.starts_with("--actor=") => o.actor = Some(s["--actor=".len()..].to_string()),
            s if s.starts_with("--name=") => o.name = Some(s["--name=".len()..].to_string()),
            s if s.starts_with("--cmd=") => o.cmd = Some(s["--cmd=".len()..].to_string()),
            s if s.starts_with("--cwd=") => o.cwd = Some(s["--cwd=".len()..].to_string()),
            s if s.starts_with("--combo=") => {
                o.combo = Some(s["--combo=".len()..].to_string())
            }
            s if s.starts_with("--mode=") => o.mode = Some(s["--mode=".len()..].to_string()),
            s if s.starts_with("--who=") => o.who = Some(s["--who=".len()..].to_string()),
            s if s.starts_with("--harness=") => {
                o.harness = Some(s["--harness=".len()..].to_string())
            }
            s if s.starts_with("--profile=") => {
                o.profile = Some(s["--profile=".len()..].to_string())
            }
            s if s.starts_with("--model=") => o.model = Some(s["--model=".len()..].to_string()),
            s if s.starts_with("--file=") => o.file = Some(s["--file=".len()..].to_string()),
            s if s.starts_with("--md=") => o.md.push(s["--md=".len()..].to_string()),
            s if s.starts_with("--extra=") => o.extra = Some(s["--extra=".len()..].to_string()),
            s if s.starts_with("--agent=") => o.agent = Some(s["--agent=".len()..].to_string()),
            s if s.starts_with("--url=") => o.url = Some(s["--url=".len()..].to_string()),
            s if s.starts_with('-') && s.len() > 1 => {
                return Err(format!("unknown flag {s:?}\n\n{USAGE}"))
            }
            s => o.words.push(s.to_string()),
        }
    }
    Ok(o)
}

/// Payload for send / set-agent / play: everything after `--`, else
/// trailing words.
fn payload(o: &Opts) -> Result<String, String> {
    if !o.tail.is_empty() {
        return Ok(o.tail.join(" "));
    }
    if o.words.len() > 2 {
        return Ok(o.words[2..].join(" "));
    }
    Err("missing payload — e.g. orbittyctl companion send -- \"line\"".into())
}

fn need_id(o: &Opts) -> Result<String, String> {
    o.id
        .clone()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "missing --id <id>".to_string())
}

/// Optional flag value: blank-after-trim = absent.
fn opt(v: &Option<String>) -> Option<String> {
    v.clone()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn need_flag(v: &Option<String>, flag: &str) -> Result<String, String> {
    opt(v).ok_or_else(|| format!("missing {flag} <value>"))
}

/// Words → (tool, args). Only the v0 tool list exists.
fn tool_call(o: &Opts) -> Result<(&'static str, Value), String> {
    let w: Vec<&str> = o.words.iter().map(String::as_str).collect();
    match w.as_slice() {
        ["status"] => Ok(("app.status", json!({}))),
        ["windows", "list"] => Ok(("windows.list", json!({}))),
        ["windows", "focus"] => Ok(("windows.focus", json!({ "id": need_id(o)? }))),
        ["windows", "minimize"] => Ok(("windows.minimize", json!({ "id": need_id(o)? }))),
        ["windows", "restore"] => Ok(("windows.restore", json!({ "id": need_id(o)? }))),
        ["windows", "close"] => Ok(("windows.close", json!({ "id": need_id(o)? }))),
        ["companion", "open"] => Ok(("companion.open", json!({}))),
        ["companion", "send"] => {
            let mut a = json!({ "line": payload(o)? });
            if let Some(p) = opt(&o.file) {
                a["path"] = json!(p);
            }
            Ok(("companion.send", a))
        }
        ["companion", "agents"] => Ok(("companion.agents", json!({}))),
        ["companion", "set-agent"] => {
            Ok(("companion.set_agent", json!({ "profile": payload(o)? })))
        }
        // `companion play nod` and `companion play -- nod` both land here.
        ["companion", "play", ..] => Ok(("companion.play", json!({ "clip": payload(o)? }))),
        // The bus verbs. Fail CLOSED on an unknown
        // mode client-side — a typo never reaches the app as a silent
        // spark. The server re-validates anyway.
        ["companion", "mode"] => {
            let m = payload(o)?;
            match m.as_str() {
                "spark" | "chat" | "work" | "hold" => {
                    Ok(("companion.mode", json!({ "mode": m })))
                }
                other => Err(format!(
                    "unknown mode {other:?} — spark | chat | work | hold"
                )),
            }
        }
        // the Free Mode switch — same
        // companion.* ACL as companion.mode. Fail CLOSED on an unknown
        // verb — a typo never flips the lane router.
        ["companion", "free-mode"] => {
            let v = payload(o)?;
            match v.as_str() {
                "on" => Ok(("companion.free_mode", json!({ "enabled": true }))),
                "off" => Ok(("companion.free_mode", json!({ "enabled": false }))),
                other => Err(format!("unknown free-mode state {other:?} — on | off")),
            }
        }
        ["companion", "note"] => {
            let mut a = json!({ "line": payload(o)? });
            if let Some(m) = opt(&o.mode) {
                match m.as_str() {
                    "spark" | "chat" | "work" | "hold" => a["mode"] = json!(m),
                    other => {
                        return Err(format!(
                            "unknown mode {other:?} — spark | chat | work | hold"
                        ))
                    }
                }
            }
            Ok(("companion.note", a))
        }
        ["companion", "thin"] => Ok(("companion.thin", json!({}))),
        ["companion", "now"] => {
            let p = payload(o).unwrap_or_default();
            if p.trim().is_empty() {
                Ok(("companion.now", json!({})))
            } else if p.trim().starts_with('{') {
                let v: Value = serde_json::from_str(&p)
                    .map_err(|e| format!("companion now JSON: {e}"))?;
                Ok(("companion.now", v))
            } else {
                Ok(("companion.now", json!({ "job": p })))
            }
        }
        // The dock LED flag. Fail CLOSED on an
        // unknown verb — a typo never leaves a stuck light. `--who` may
        // ride either side of `--` (after it, it's tail text we parse).
        // `--project` rides the same way — the
        // live agent's pinned planet, for the dest/planet menu glow.
        ["companion", "hack", ..] => {
            let p = payload(o)?;
            let mut toks = p.split_whitespace();
            let live = match toks.next().unwrap_or("") {
                "on" => true,
                "off" => false,
                other => return Err(format!("unknown hack state {other:?} — on | off")),
            };
            let mut who = opt(&o.who);
            let mut project = opt(&o.project);
            let mut rest = toks;
            while let Some(t) = rest.next() {
                if t == "--who" {
                    who = rest
                        .next()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                } else if let Some(v) = t.strip_prefix("--who=") {
                    who = Some(v.trim().to_string()).filter(|s| !s.is_empty());
                } else if t == "--project" {
                    project = rest
                        .next()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                } else if let Some(v) = t.strip_prefix("--project=") {
                    project = Some(v.trim().to_string()).filter(|s| !s.is_empty());
                }
            }
            let mut a = json!({ "live": live });
            if let Some(w) = who {
                a["who"] = json!(w);
            }
            if let Some(p) = project {
                a["projectId"] = json!(p);
            }
            Ok(("companion.hack", a))
        }
        ["kanban", "open"] => Ok(("kanban.open", json!({}))),
        // Write verbs. The server validates values
        // (status enums, unknown ids) and owns the honest errors.
        ["kanban", "add"] => {
            let mut a = json!({ "title": need_flag(&o.title, "--title")? });
            if let Some(p) = opt(&o.project) {
                a["projectId"] = json!(p);
            }
            if let Some(s) = opt(&o.status) {
                a["status"] = json!(s);
            }
            // Blank --notes = absent (none minted).
            if let Some(n) = opt(&o.notes) {
                a["notes"] = json!(n);
            }
            Ok(("kanban.add", a))
        }
        // Set/clear a card's one-line summary. The
        // text is the `--` payload; `-- ""` clears (payload keeps Some("")),
        // so the clear always reaches the server as an empty string.
        ["kanban", "notes"] => Ok((
            "kanban.notes",
            json!({ "id": need_id(o)?, "notes": payload(o)? }),
        )),
        ["kanban", "move"] => Ok((
            "kanban.move",
            json!({ "id": need_id(o)?, "status": need_flag(&o.status, "--status")? }),
        )),
        // Assign a card to an agent row (id or
        // name server-side). `--agent ""` unassigns — the flag itself is
        // required so a bare `kanban assign --id` never silently clears.
        ["kanban", "assign"] => {
            let id = need_id(o)?;
            match (o.agent.clone(), o.window.clone()) {
                (Some(agent), None) => Ok(("kanban.assign", json!({ "id": id, "agent": agent }))),
                (None, Some(window)) => Ok((
                    "kanban.assign_window",
                    json!({ "id": id, "windowId": window }),
                )),
                (Some(_), Some(_)) => Err(
                    "kanban assign: pass --agent or --window, not both".into(),
                ),
                (None, None) => Err(
                    "kanban assign: need --agent <agent> or --window <windowId> (pass empty \"\" to clear)".into(),
                ),
            }
        }
        // The agent roster. The harness is
        // required on add and fails CLOSED here — a typo never reaches
        // the app (the server re-validates anyway).
        ["agent", "list"] => Ok(("agent.list", json!({}))),
        ["agent", "add"] => {
            let h = need_flag(&o.harness, "--harness")?.to_ascii_lowercase();
            match h.as_str() {
                "cursor" | "goose" | "grok" | "hermes" => {}
                other => {
                    return Err(format!(
                        "unknown harness {other:?} — cursor | goose | grok | hermes"
                    ))
                }
            }
            let mut a = json!({
                "name": need_flag(&o.name, "--name")?,
                "harness": h,
            });
            if let Some(p) = opt(&o.profile) {
                a["profile"] = json!(p);
            }
            if let Some(m) = opt(&o.model) {
                a["model"] = json!(m);
            }
            let mds: Vec<String> = o
                .md
                .iter()
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            if !mds.is_empty() {
                a["mdPaths"] = json!(mds);
            }
            if let Some(x) = opt(&o.extra) {
                a["extra"] = json!(x);
            }
            if let Some(p) = opt(&o.project) {
                a["projectId"] = json!(p);
            }
            Ok(("agent.add", a))
        }
        ["agent", "rm"] => Ok(("agent.rm", json!({ "id": need_id(o)? }))),
        ["agent", "pin"] => Ok((
            "agent.pin",
            json!({ "id": need_id(o)?, "projectId": need_flag(&o.project, "--project")? }),
        )),
        ["agent", "pickup"] => {
            let mut a = json!({});
            if let Some(id) = opt(&o.id) {
                a["id"] = json!(id);
            } else if let Some(n) = opt(&o.name) {
                a["name"] = json!(n);
            } else {
                return Err("missing --id <id> or --name <name>".to_string());
            }
            if let Some(c) = opt(&o.card) {
                a["cardId"] = json!(c);
            }
            Ok(("agent.pickup", a))
        }
        ["shift", "status"] => Ok(("shift.status", json!({}))),
        ["gate", "status"] => Ok(("gate.status", json!({}))),
        ["gate", "inject", kind] => Ok(("gate.inject", json!({ "kind": *kind }))),
        ["shift", "tick"] => Ok(("shift.tick", json!({}))),
        ["shift", "on"] => Ok(("shift.on", json!({}))),
        ["shift", "off"] => Ok(("shift.off", json!({}))),
        ["relay", "status"] => Ok(("relay.status", json!({}))),
        ["relay", "on"] => Ok(("relay.on", json!({}))),
        ["relay", "off"] => Ok(("relay.off", json!({}))),
        ["relay", "replica"] => {
            let url = o
                .url
                .clone()
                .ok_or("missing --url (pass --url \"\" to clear)".to_string())?;
            Ok(("relay.replica", json!({ "url": url })))
        }
        // Planet control plane. need_flag trims and
        // refuses an empty --name; the server owns unknown-id errors.
        ["claim", "front"] => Ok(("claim.front", json!({}))),
        ["claim", "unclaim"] => Ok(("claim.unclaim", json!({ "windowId": need_id(o)? }))),
        ["claim", "undo"] => Ok(("claim.undo", json!({}))),
        ["claim", "to"] => Ok((
            "claim.to",
            json!({ "projectId": need_flag(&o.project, "--project")? }),
        )),
        ["claim", "change"] => Ok((
            "claim.change",
            json!({ "projectId": need_flag(&o.project, "--project")? }),
        )),
        ["project", "add"] => Ok((
            "project.add",
            json!({ "name": need_flag(&o.name, "--name")? }),
        )),
        ["project", "rm"] => Ok(("project.rm", json!({ "id": need_id(o)? }))),
        ["project", "rename"] => Ok((
            "project.rename",
            json!({ "id": need_id(o)?, "name": need_flag(&o.name, "--name")? }),
        )),
        ["project", "set-root"] => Ok((
            "project.set_root",
            json!({
                "id": need_id(o)?,
                "path": need_flag(&o.cwd, "--cwd")?,
            }),
        )),
        ["task", "undo"] => Ok(("task.undo", json!({ "id": need_id(o)? }))),
        // The action recipe file. The server owns
        // validation (parse, reserved chords, unknown ids) and the ACL.
        ["action", "list"] => Ok(("action.list", json!({}))),
        ["action", "add"] => {
            let mut a = json!({
                "name": need_flag(&o.name, "--name")?,
                "cmd": need_flag(&o.cmd, "--cmd")?,
            });
            if let Some(d) = opt(&o.cwd) {
                a["cwd"] = json!(d);
            }
            if let Some(k) = opt(&o.combo) {
                a["combo"] = json!(k);
            }
            Ok(("action.add", a))
        }
        ["action", "run"] => {
            if let Some(id) = opt(&o.id) {
                Ok(("action.run", json!({ "id": id })))
            } else if let Some(n) = opt(&o.name) {
                Ok(("action.run", json!({ "name": n })))
            } else {
                Err("missing --id <id> or --name <name>".to_string())
            }
        }
        ["action", "rm"] => Ok(("action.rm", json!({ "id": need_id(o)? }))),
        ["needed", "open"] => {
            let mut a = json!({ "title": need_flag(&o.title, "--title")? });
            if let Some(p) = opt(&o.project) {
                a["projectId"] = json!(p);
            }
            if let Some(w) = opt(&o.why) {
                a["why"] = json!(w);
            }
            Ok(("needed.open", a))
        }
        // Answer a Needed from the CLI —
        // the same needed-answer event path as the composer, so the watch
        // dispatches on it. The answer text is the `--` payload.
        ["needed", "answer"] => Ok((
            "needed.answer",
            json!({ "id": need_id(o)?, "answer": payload(o)? }),
        )),
        ["log", "append"] => {
            let mut a = json!({ "eType": need_flag(&o.etype, "--type")? });
            if let Some(p) = opt(&o.project) {
                a["projectId"] = json!(p);
            }
            if let Some(ac) = opt(&o.actor) {
                a["actor"] = json!(ac);
            }
            if !o.tail.is_empty() {
                let raw = o.tail.join(" ");
                let c: Value = serde_json::from_str(&raw)
                    .map_err(|e| format!("content JSON: {e}"))?;
                if !c.is_object() {
                    return Err("content must be a JSON object".to_string());
                }
                a["content"] = c;
            }
            Ok(("log.append", a))
        }
        ["stars", "open"] => Ok(("stars.open", json!({}))),
        ["packs", "list"] => Ok(("packs.list", json!({}))),
        ["sleeper", "wake"] => Ok(("sleeper.wake", json!({ "id": need_id(o)? }))),
        ["state", "get"] => Ok(("state.get", json!({}))),
        [] => Err(format!("no command\n\n{USAGE}")),
        other => Err(format!("unknown command: {}\n\n{USAGE}", other.join(" "))),
    }
}

fn default_sock() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Application Support/space.orbitty.mac/control.sock")
}

fn sock_path(o: &Opts) -> PathBuf {
    if let Some(p) = &o.sock {
        return p.clone();
    }
    if let Ok(p) = std::env::var("ORBITTY_CONTROL_SOCK") {
        let p = p.trim().to_string();
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    default_sock()
}

/// Token precedence: --token flag, then env, then control.token next to
/// the socket in use.
fn token(o: &Opts, sock: &Path) -> Option<String> {
    if let Some(t) = &o.token {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    if let Ok(t) = std::env::var("ORBITTY_CONTROL_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    sock.parent().and_then(|dir| {
        std::fs::read_to_string(dir.join("control.token"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    })
}

fn print_gate(data: &Value) {
    let armed = data["armed"].as_bool().unwrap_or(false);
    let frozen = data["frozen"].as_bool().unwrap_or(false);
    println!(
        "gate {} · freeze {} · chatter {} · stop {}",
        if armed { "armed" } else { "disarmed" },
        if frozen { "on" } else { "off" },
        data["chatterHeld"].as_u64().unwrap_or(0),
        data["stopHonored"].as_u64().unwrap_or(0)
    );
}

fn print_human(tool: &str, data: &Value) {
    match tool {
        "app.status" => {
            let trusted = data["ax"]["trusted"].as_bool().unwrap_or(false);
            println!(
                "Orbitty up · socket ok · AX {}",
                if trusted { "trusted" } else { "NOT trusted" }
            );
            if !trusted {
                println!("  grant: System Settings → Privacy & Security → Accessibility");
            }
        }
        "app.accessibility_status" => {
            println!("trusted:  {}", data["trusted"].as_bool().unwrap_or(false));
            println!("bundle:   {}", data["bundlePath"].as_str().unwrap_or(""));
            let s = data["signingLine"].as_str().unwrap_or("");
            if !s.is_empty() {
                println!("signing:  {s}");
            }
        }
        "windows.list" => {
            let Some(arr) = data.as_array() else {
                return;
            };
            if arr.is_empty() {
                println!("(no windows)");
            }
            for w in arr {
                let id = w["id"].as_str().unwrap_or("?");
                let app = w["appName"].as_str().unwrap_or("?");
                let title = w["title"].as_str().unwrap_or("");
                let mut marks = String::new();
                if w["isFrontmost"].as_bool().unwrap_or(false) {
                    marks.push_str(" [frontmost]");
                }
                if w["isMinimized"].as_bool().unwrap_or(false) {
                    marks.push_str(" [minimized]");
                }
                if title.is_empty() {
                    println!("{id}  {app}{marks}");
                } else {
                    println!("{id}  {app} — {title}{marks}");
                }
            }
        }
        "windows.focus" => println!("focused"),
        "windows.minimize" => println!("minimized"),
        "windows.restore" => println!("restored"),
        "windows.close" => println!("close pressed"),
        "companion.open" => println!("companion open"),
        "kanban.open" => println!("kanban open"),
        "stars.open" => println!("stars open"),
        "companion.send" => println!("{}", data["reply"].as_str().unwrap_or("")),
        "companion.agents" => {
            let sel = data["selected"].as_str().unwrap_or("");
            if let Some(arr) = data["agents"].as_array() {
                for a in arr {
                    let n = a.as_str().unwrap_or("");
                    println!("{} {}", if n == sel { "*" } else { " " }, n);
                }
            }
        }
        "companion.set_agent" => println!("agent: {}", data.as_str().unwrap_or("")),
        "companion.play" => println!("playing: {}", data["played"].as_str().unwrap_or("")),
        "companion.mode" => println!("mode: {}", data.as_str().unwrap_or("")),
        "companion.free_mode" => println!(
            "free mode: {}",
            if data["freeMode"].as_bool() == Some(true) {
                "on"
            } else {
                "off"
            }
        ),
        "companion.note" => println!("noted ({})", data.as_str().unwrap_or("")),
        "companion.thin" => println!("{}", data.as_str().unwrap_or("thin")),
        "companion.now" => {
            if data.is_string() {
                println!("{}", data.as_str().unwrap_or(""));
            } else {
                println!("{}", serde_json::to_string_pretty(&data).unwrap_or_default());
            }
        }
        "companion.hack" => {
            let live = data["live"].as_bool().unwrap_or(false);
            let who = data["who"].as_str().unwrap_or("");
            match (live, who.is_empty()) {
                (true, false) => println!("hack: on ({who})"),
                (true, true) => println!("hack: on"),
                _ => println!("hack: off"),
            }
        }
        "packs.list" => {
            let Some(arr) = data.as_array() else {
                return;
            };
            if arr.is_empty() {
                println!("(no packs)");
            }
            for p in arr {
                let id = p["id"].as_str().unwrap_or("?");
                let name = p["name"].as_str().unwrap_or("");
                let frames = p["frameCount"].as_u64().unwrap_or(0);
                let warn = if p["warnLicense"].as_bool().unwrap_or(false) {
                    "  [no license]"
                } else {
                    ""
                };
                println!("{id}  {name} ({frames} frames){warn}");
            }
        }
        "sleeper.wake" => println!("{}", data.as_str().unwrap_or("")),
        "kanban.add" => println!(
            "added {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["title"].as_str().unwrap_or("")
        ),
        "kanban.move" => println!(
            "{} → {}",
            data["id"].as_str().unwrap_or("?"),
            data["status"].as_str().unwrap_or("?")
        ),
        // The reply is the card; notes
        // absent = cleared back to title-only.
        "kanban.notes" => {
            let id = data["id"].as_str().unwrap_or("?");
            match data["notes"].as_str().filter(|n| !n.is_empty()) {
                Some(n) => println!("notes {id} — {}", n.lines().next().unwrap_or("")),
                None => println!("notes cleared {id}"),
            }
        }
        // Agent roster printers.
        "kanban.assign" => {
            let id = data["id"].as_str().unwrap_or("?");
            let title = data["title"].as_str().unwrap_or("");
            match data["agent"].as_str() {
                Some(a) => println!("assigned {id} — {title} → {a}"),
                None => println!("unassigned {id} — {title}"),
            }
        }
        "agent.list" => {
            let Some(arr) = data.as_array() else {
                return;
            };
            if arr.is_empty() {
                println!("(no agents)");
            }
            for a in arr {
                let id = a["id"].as_str().unwrap_or("?");
                let name = a["name"].as_str().unwrap_or("?");
                let harness = a["harness"].as_str().unwrap_or("?");
                let profile = a["profile"].as_str().unwrap_or("");
                let project = a["project"]
                    .as_str()
                    .or_else(|| a["projectId"].as_str())
                    .unwrap_or("—");
                let mut marks = String::new();
                if let Some(m) = a["model"].as_str().filter(|m| !m.is_empty()) {
                    marks.push_str(&format!(" model={m}"));
                }
                let md_n = a["mdPaths"].as_array().map(|x| x.len()).unwrap_or(0);
                if md_n > 0 {
                    marks.push_str(&format!(" md={md_n}"));
                }
                if a["extra"].as_str().is_some_and(|x| !x.is_empty()) {
                    marks.push_str(" +extra");
                }
                let prof = if profile.is_empty() {
                    String::new()
                } else {
                    format!("  {profile}")
                };
                println!("{id}  {name}  {harness}{prof}  → {project}{marks}");
            }
        }
        "agent.add" => {
            let id = data["id"].as_str().unwrap_or("?");
            let name = data["name"].as_str().unwrap_or("");
            match data["project"].as_str() {
                Some(p) => println!("added {id} — {name} on {p}"),
                None => println!("added {id} — {name}"),
            }
        }
        "agent.rm" => println!(
            "removed {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["name"].as_str().unwrap_or("")
        ),
        "agent.pin" => println!(
            "pinned {} — {} → {}",
            data["id"].as_str().unwrap_or("?"),
            data["name"].as_str().unwrap_or(""),
            data["project"].as_str().unwrap_or("?")
        ),
        "agent.pickup" => {
            if data["picked"].as_bool() == Some(true) {
                println!(
                    "picked {} — {} → doing",
                    data["cardId"].as_str().unwrap_or("?"),
                    data["title"].as_str().unwrap_or("")
                );
                if let Some(l) = data["launch"].as_str() {
                    println!("{l}");
                }
            } else {
                println!(
                    "nothing assigned to {} — no spawn",
                    data["agent"].as_str().unwrap_or("?")
                );
            }
        }
        "gate.status" => print_gate(data),
        "gate.inject" => {
            let kind = data["kind"].as_str().unwrap_or("?");
            let verdict = data["verdict"].as_str().unwrap_or("?");
            println!("{kind} → {verdict}");
            print_gate(&data["status"]);
        }
        "shift.status" => {
            let on = data["on"].as_bool().unwrap_or(false);
            let mode = data["mode"].as_str().unwrap_or("?");
            let live = data["runners"]["live"].as_u64().unwrap_or(0);
            let cap = data["runners"]["cap"].as_u64().unwrap_or(0);
            let ax = data["axTrusted"].as_bool().unwrap_or(false);
            println!(
                "shift {} · mode {mode} · runners {live}/{cap} · AX {}",
                if on { "on" } else { "off" },
                if ax { "trusted" } else { "NOT trusted" }
            );
            if let Some(runs) = data["runs"].as_array() {
                for r in runs {
                    let ghost = if r["ghost"].as_bool().unwrap_or(false) {
                        " ghost"
                    } else {
                        ""
                    };
                    println!(
                        "  {}  {}  {}{ghost}",
                        r["state"].as_str().unwrap_or("?"),
                        r["cardId"].as_str().unwrap_or("?"),
                        r["title"].as_str().unwrap_or("")
                    );
                }
            }
            if let Some(next) = data["next"].as_array() {
                for n in next {
                    println!(
                        "  next {} — {} ({})",
                        n["agent"].as_str().unwrap_or("?"),
                        n["title"].as_str().unwrap_or(""),
                        n["cardId"].as_str().unwrap_or("?")
                    );
                }
            }
        }
        "shift.tick" => {
            let parked = data["parked"].as_array().map(|a| a.len()).unwrap_or(0);
            let escalated = data["escalated"].as_array().map(|a| a.len()).unwrap_or(0);
            let picked = data["picked"].as_array().map(|a| a.len()).unwrap_or(0);
            println!("tick · parked {parked} · stuck {escalated} · picked {picked}");
            if let Some(notes) = data["notes"].as_array() {
                for n in notes {
                    if let Some(s) = n.as_str() {
                        println!("  {s}");
                    }
                }
            }
        }
        "shift.on" => println!(
            "shift on{}",
            if data["tick"].is_object() {
                " · ticked"
            } else {
                ""
            }
        ),
        "shift.off" => println!("shift off"),
        "relay.status" => {
            let on = data["on"].as_bool().unwrap_or(false);
            let url = data["url"].as_str().unwrap_or("").trim();
            let replica = if url.is_empty() {
                "local only"
            } else {
                url
            };
            println!(
                "private relay {} · replica {replica}",
                if on { "on" } else { "off" }
            );
        }
        "relay.replica" => {
            let url = data["url"].as_str().unwrap_or("").trim();
            if url.is_empty() {
                println!("replica cleared — local only");
            } else {
                println!("replica {url}");
            }
        }
        "project.add" => println!(
            "added {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["name"].as_str().unwrap_or("")
        ),
        "project.rm" => println!(
            "removed {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["name"].as_str().unwrap_or("")
        ),
        "project.rename" => println!(
            "renamed {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["name"].as_str().unwrap_or("")
        ),
        "action.list" => {
            let Some(arr) = data.as_array() else {
                return;
            };
            if arr.is_empty() {
                println!("(no actions)");
            }
            for a in arr {
                let id = a["id"].as_str().unwrap_or("?");
                let name = a["name"].as_str().unwrap_or("?");
                let combo = a["combo"].as_str().unwrap_or("—");
                let cmd = a["command"].as_str().unwrap_or("");
                let mark = if a["status"].as_str() == Some("conflict") {
                    "  [conflict]"
                } else {
                    ""
                };
                println!("{id}  {name}  {combo}  {cmd}{mark}");
            }
        }
        "action.add" => println!(
            "added {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["name"].as_str().unwrap_or("")
        ),
        "action.rm" => println!("removed {}", data["id"].as_str().unwrap_or("?")),
        "action.run" => {
            let ok = data["ok"].as_bool().unwrap_or(false);
            let head = if ok {
                "done".to_string()
            } else {
                format!(
                    "exit {}",
                    data["code"]
                        .as_i64()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "?".into())
                )
            };
            println!("{head}");
            for s in ["stdout", "stderr"]
                .into_iter()
                .filter_map(|k| data[k].as_str())
                .filter(|s| !s.is_empty())
            {
                println!("{s}");
            }
        }
        "needed.open" => println!(
            "needed {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["title"].as_str().unwrap_or("")
        ),
        "needed.answer" => println!(
            "answered {} — {}",
            data["id"].as_str().unwrap_or("?"),
            data["title"].as_str().unwrap_or("")
        ),
        "log.append" => {
            if data["skipped"].as_bool() == Some(true) {
                println!("log append skipped (local log unavailable — not an error)");
            } else {
                println!("logged {}", data["id"].as_str().unwrap_or("?"));
            }
        }
        // state.get and anything else: pretty JSON is the honest human view.
        _ => println!(
            "{}",
            serde_json::to_string_pretty(data).unwrap_or_else(|_| "null".into())
        ),
    }
}

pub fn run() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let opts = match parse(&argv) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            exit(EXIT_OTHER);
        }
    };
    let (tool, args) = match tool_call(&opts) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{e}");
            exit(EXIT_OTHER);
        }
    };

    let sock = sock_path(&opts);
    let tok = token(&opts, &sock);

    // Missing sock / refused connect both fail fast — never a hang.
    let mut stream = match UnixStream::connect(&sock) {
        Ok(s) => s,
        Err(e) => {
            let why = match e.kind() {
                std::io::ErrorKind::NotFound => "no control socket".to_string(),
                _ => e.to_string(),
            };
            eprintln!("Orbitty is not running ({why}: {})", sock.display());
            exit(EXIT_NOT_RUNNING);
        }
    };
    // companion.send can legitimately poll Berd for up to 5 minutes; the
    // read guard exists only so a wedged server can't hold us forever.
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(360)));

    let mut req = json!({ "v": 1, "id": "1", "tool": tool, "args": args });
    if let Some(t) = tok {
        req["token"] = Value::String(t);
    }
    let mut line = serde_json::to_string(&req).unwrap_or_else(|_| "{}".into());
    line.push('\n');
    if let Err(e) = stream.write_all(line.as_bytes()) {
        eprintln!("write to Orbitty: {e}");
        exit(EXIT_OTHER);
    }

    let mut reply_line = String::new();
    let mut reader = BufReader::new((&stream).take(64 << 20));
    match reader.read_line(&mut reply_line) {
        Ok(0) => {
            eprintln!("Orbitty closed the connection without a reply");
            exit(EXIT_OTHER);
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("read from Orbitty: {e}");
            exit(EXIT_OTHER);
        }
    }
    let v: Value = match serde_json::from_str(reply_line.trim()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("bad reply from Orbitty: {e}");
            exit(EXIT_OTHER);
        }
    };

    if v["ok"].as_bool() == Some(true) {
        let data = &v["data"];
        if opts.json {
            println!("{}", serde_json::to_string_pretty(data).unwrap_or_else(|_| "null".into()));
        } else {
            print_human(tool, data);
        }
        exit(0);
    }

    let code = v["code"].as_i64().unwrap_or(1) as i32;
    let err = v["error"].as_str().unwrap_or("unknown error");
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into()));
    } else {
        eprintln!("{err}");
    }
    exit(if code > 0 { code } else { EXIT_OTHER });
}

} // mod imp

#[cfg(unix)]
fn main() {
    imp::run();
}

#[cfg(not(unix))]
fn main() {
    eprintln!("orbittyctl is unix-only (Orbitty control socket)");
    std::process::exit(1);
}
