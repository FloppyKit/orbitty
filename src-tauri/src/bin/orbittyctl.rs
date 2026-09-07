//! orbittyctl — GO-ORBITTY-CONTROL-PLANE-0 CLI for the Orbitty control
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
  companion send -- \"line\"
  companion agents
  companion set-agent -- <profile>
  companion play -- <clip>         idle · think · nod · look-left · look-right
  kanban open
  kanban add --title \"t\" [--project P] [--status todo|doing]
  kanban move --id <id> --status todo|doing|done
  action list
  action add --name N --cmd \"C\" [--cwd D] [--combo cmd+alt+m]
  action run --id <id> | --name <name>
  action rm --id <id>
  needed open --title \"t\" [--project P] [--why \"w\"]
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

GO-ORBITTY-MCP-WRITE: the four write verbs (kanban add/move, needed open,
log append) mutate through the app's one save door — the UI live-updates.
ACL: control.json profile full, or a named custom allowlist entry; read /
companion are denied (exit 4).

GO-ORBITTY-ACTION-FILE: action list is read-safe; the companion profile
may action add / action run (Spark mints named recipes — never a raw
shell); action rm needs full or a custom entry naming it (or \"action.*\").
Combos register globally on persist; reserved (cmd+shift+o, cmd+space) or
taken chords show [conflict] in action list.

exit codes: 0 ok · 2 Orbitty not running · 3 auth · 4 denied · 5 AX · 1 other";

struct Opts {
    json: bool,
    token: Option<String>,
    sock: Option<PathBuf>,
    id: Option<String>,
    title: Option<String>,
    project: Option<String>,
    status: Option<String>,
    why: Option<String>,
    etype: Option<String>,
    actor: Option<String>,
    name: Option<String>,
    cmd: Option<String>,
    cwd: Option<String>,
    combo: Option<String>,
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
        why: None,
        etype: None,
        actor: None,
        name: None,
        cmd: None,
        cwd: None,
        combo: None,
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
            "--why" => o.why = Some(it.next().ok_or("--why needs a value")?.clone()),
            "--type" => o.etype = Some(it.next().ok_or("--type needs a value")?.clone()),
            "--actor" => o.actor = Some(it.next().ok_or("--actor needs a value")?.clone()),
            "--name" => o.name = Some(it.next().ok_or("--name needs a value")?.clone()),
            "--cmd" => o.cmd = Some(it.next().ok_or("--cmd needs a value")?.clone()),
            "--cwd" => o.cwd = Some(it.next().ok_or("--cwd needs a value")?.clone()),
            "--combo" => {
                o.combo = Some(it.next().ok_or("--combo needs a value")?.clone())
            }
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
            s if s.starts_with("--why=") => o.why = Some(s["--why=".len()..].to_string()),
            s if s.starts_with("--type=") => o.etype = Some(s["--type=".len()..].to_string()),
            s if s.starts_with("--actor=") => o.actor = Some(s["--actor=".len()..].to_string()),
            s if s.starts_with("--name=") => o.name = Some(s["--name=".len()..].to_string()),
            s if s.starts_with("--cmd=") => o.cmd = Some(s["--cmd=".len()..].to_string()),
            s if s.starts_with("--cwd=") => o.cwd = Some(s["--cwd=".len()..].to_string()),
            s if s.starts_with("--combo=") => {
                o.combo = Some(s["--combo=".len()..].to_string())
            }
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
        ["companion", "send"] => Ok(("companion.send", json!({ "line": payload(o)? }))),
        ["companion", "agents"] => Ok(("companion.agents", json!({}))),
        ["companion", "set-agent"] => {
            Ok(("companion.set_agent", json!({ "profile": payload(o)? })))
        }
        // `companion play nod` and `companion play -- nod` both land here.
        ["companion", "play", ..] => Ok(("companion.play", json!({ "clip": payload(o)? }))),
        ["kanban", "open"] => Ok(("kanban.open", json!({}))),
        // GO-ORBITTY-MCP-WRITE: write verbs. The server validates values
        // (status enums, unknown ids) and owns the honest errors.
        ["kanban", "add"] => {
            let mut a = json!({ "title": need_flag(&o.title, "--title")? });
            if let Some(p) = opt(&o.project) {
                a["projectId"] = json!(p);
            }
            if let Some(s) = opt(&o.status) {
                a["status"] = json!(s);
            }
            Ok(("kanban.add", a))
        }
        ["kanban", "move"] => Ok((
            "kanban.move",
            json!({ "id": need_id(o)?, "status": need_flag(&o.status, "--status")? }),
        )),
        // GO-ORBITTY-ACTION-FILE: the action recipe file. The server owns
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
