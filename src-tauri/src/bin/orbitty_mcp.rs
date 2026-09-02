//! orbitty-mcp — GO-ORBITTY-MCP-0 thin stdio adapter.
//!
//! Speaks MCP JSON-RPC 2.0 on stdin/stdout (newline JSON for Goose/rmcp;
//! Content-Length still accepted for the Pixel smoke test).
//! Forwards tools/call to the Orbitty Unix control socket using the
//! same newline-JSON protocol as orbittyctl. Never links Tauri, never
//! opens a WebView, never binds TCP, never writes goose config.

#[cfg(unix)]
mod imp {
use serde_json::{json, Value};
use std::io::{self, BufRead, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

const PROTOCOL: &str = "2024-11-05";
const SERVER_NAME: &str = "orbitty";
const SERVER_VERSION: &str = "0.1.0";

struct ToolDef {
    name: &'static str,
    description: &'static str,
    /// "none" | "id" | "line" | "profile"
    args: &'static str,
}

const TOOLS: &[ToolDef] = &[
    ToolDef { name: "app.status", description: "Orbitty up + Accessibility summary", args: "none" },
    ToolDef { name: "app.accessibility_status", description: "Accessibility grant status", args: "none" },
    ToolDef { name: "windows.list", description: "List windows the rail sees", args: "none" },
    ToolDef { name: "windows.focus", description: "Focus a window by id", args: "id" },
    ToolDef { name: "windows.minimize", description: "Minimize a window by id", args: "id" },
    ToolDef { name: "windows.restore", description: "Restore a window by id", args: "id" },
    ToolDef { name: "windows.close", description: "Close a window by id", args: "id" },
    ToolDef { name: "companion.open", description: "Show the companion window", args: "none" },
    ToolDef { name: "kanban.open", description: "Show the shared pane in Board mode", args: "none" },
    ToolDef { name: "stars.open", description: "Show the shared pane in STARS mode", args: "none" },
    ToolDef { name: "companion.send", description: "Send a line on the companion Berd path", args: "line" },
    ToolDef { name: "companion.agents", description: "List companion agent profiles", args: "none" },
    ToolDef { name: "companion.set_agent", description: "Set the companion agent profile", args: "profile" },
    ToolDef { name: "packs.list", description: "List installed sprite packs", args: "none" },
    ToolDef { name: "sleeper.wake", description: "Wake a sleeper by id", args: "id" },
    ToolDef { name: "state.get", description: "Read persisted Orbitty state", args: "none" },
];

fn known_tool(name: &str) -> bool {
    TOOLS.iter().any(|t| t.name == name)
}

fn input_schema(kind: &str) -> Value {
    match kind {
        "id" => json!({
            "type": "object",
            "properties": { "id": { "type": "string", "description": "Window or sleeper id" } },
            "required": ["id"]
        }),
        "line" => json!({
            "type": "object",
            "properties": { "line": { "type": "string", "description": "Text to send" } },
            "required": ["line"]
        }),
        "profile" => json!({
            "type": "object",
            "properties": { "profile": { "type": "string", "description": "Agent profile id" } },
            "required": ["profile"]
        }),
        _ => json!({ "type": "object", "properties": {} }),
    }
}

fn tools_list() -> Value {
    let tools: Vec<Value> = TOOLS
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": input_schema(t.args),
            })
        })
        .collect();
    json!({ "tools": tools })
}

fn default_sock() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Application Support/space.orbitty.mac/control.sock")
}

fn sock_path() -> PathBuf {
    if let Ok(p) = std::env::var("ORBITTY_CONTROL_SOCK") {
        let p = p.trim().to_string();
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    default_sock()
}

fn token(sock: &std::path::Path) -> Option<String> {
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

/// One request → one reply on the Unix control socket.
fn unix_call(tool: &str, args: Value) -> Result<Value, String> {
    let sock = sock_path();
    let mut stream = match UnixStream::connect(&sock) {
        Ok(s) => s,
        Err(e) => {
            let why = match e.kind() {
                io::ErrorKind::NotFound => "no control socket".to_string(),
                _ => e.to_string(),
            };
            return Err(format!(
                "Orbitty is not running ({why}: {})",
                sock.display()
            ));
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(360)));

    let mut req = json!({ "v": 1, "id": "1", "tool": tool, "args": args });
    if let Some(t) = token(&sock) {
        req["token"] = Value::String(t);
    }
    let mut line = serde_json::to_string(&req).unwrap_or_else(|_| "{}".into());
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .map_err(|e| format!("write to Orbitty: {e}"))?;

    let mut reply_line = String::new();
    let mut reader = io::BufReader::new((&stream).take(64 << 20));
    match reader.read_line(&mut reply_line) {
        Ok(0) => return Err("Orbitty closed the connection without a reply".into()),
        Ok(_) => {}
        Err(e) => return Err(format!("read from Orbitty: {e}")),
    }
    serde_json::from_str(reply_line.trim())
        .map_err(|e| format!("bad reply from Orbitty: {e}"))
}

fn rpc_ok(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_err(id: &Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

fn call_result_ok(data: &Value) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(data).unwrap_or_else(|_| "null".into())
        }]
    })
}

fn call_result_err(msg: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": msg }],
        "isError": true
    })
}

fn dispatch(msg: &Value) -> Option<Value> {
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = msg.get("id").cloned();
    // Notifications have no id — no reply.
    if id.is_none() {
        return None;
    }
    let id = id.unwrap();

    match method {
        "initialize" => {
            let client_ver = msg
                .pointer("/params/protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or(PROTOCOL);
            let ver = if client_ver.is_empty() { PROTOCOL } else { client_ver };
            Some(rpc_ok(
                &id,
                json!({
                    "protocolVersion": ver,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
                }),
            ))
        }
        "ping" => Some(rpc_ok(&id, json!({}))),
        "tools/list" => Some(rpc_ok(&id, tools_list())),
        "tools/call" => {
            let name = msg
                .pointer("/params/name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                return Some(rpc_err(&id, -32602, "tools/call missing params.name"));
            }
            if !known_tool(&name) {
                return Some(rpc_err(
                    &id,
                    -32602,
                    &format!("unknown tool {name:?} (v0 list only)"),
                ));
            }
            let args = msg
                .pointer("/params/arguments")
                .cloned()
                .unwrap_or(json!({}));
            let args = if args.is_null() {
                json!({})
            } else if args.is_object() {
                args
            } else {
                json!({})
            };
            match unix_call(&name, args) {
                Err(e) => Some(rpc_err(&id, -32002, &e)),
                Ok(reply) => {
                    if reply.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                        let data = reply.get("data").cloned().unwrap_or(Value::Null);
                        Some(rpc_ok(&id, call_result_ok(&data)))
                    } else {
                        let code = reply.get("code").and_then(|v| v.as_i64()).unwrap_or(1);
                        let err = reply
                            .get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown error");
                        let tagged = match code {
                            3 => format!("auth failed (3): {err}"),
                            4 => format!("denied (4): {err}"),
                            5 => format!("accessibility (5): {err}"),
                            _ => format!("orbitty error ({code}): {err}"),
                        };
                        Some(rpc_ok(&id, call_result_err(&tagged)))
                    }
                }
            }
        }
        "" => {
            // Bare JSON-RPC without method — ignore if we somehow got here.
            Some(rpc_err(&id, -32600, "missing method"))
        }
        other => Some(rpc_err(&id, -32601, &format!("method not found: {other}"))),
    }
}

/// Goose / rmcp speak MCP newline-delimited JSON. Older smoke tests used
/// LSP Content-Length. Accept both; reply in the same framing as the request.
#[derive(Clone, Copy)]
enum Framing {
    Ndjson,
    Lsp,
}

fn parse_json_bytes(buf: &[u8]) -> io::Result<Value> {
    serde_json::from_slice(buf)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bad JSON: {e}")))
}

fn read_lsp_body(stdin: &mut impl BufRead, first_len: usize) -> io::Result<Option<Value>> {
    if first_len > 16 << 20 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message too large",
        ));
    }
    // Consume remaining headers until blank line.
    loop {
        let mut header = String::new();
        let n = stdin.read_line(&mut header)?;
        if n == 0 {
            return Ok(None);
        }
        if header.trim_end_matches(['\r', '\n']).is_empty() {
            break;
        }
    }
    let mut buf = vec![0u8; first_len];
    stdin.read_exact(&mut buf)?;
    Ok(Some(parse_json_bytes(&buf)?))
}

/// Read one MCP message. `framing` is set from the first request.
fn read_message(
    stdin: &mut impl BufRead,
    framing: &mut Option<Framing>,
) -> io::Result<Option<(Value, Framing)>> {
    loop {
        let mut line = String::new();
        let n = stdin.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            let len = rest.trim().parse::<usize>().map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("Content-Length: {e}"))
            })?;
            let mode = Framing::Lsp;
            *framing = Some(mode);
            let Some(v) = read_lsp_body(stdin, len)? else {
                return Ok(None);
            };
            return Ok(Some((v, mode)));
        }
        let mode = Framing::Ndjson;
        *framing = Some(mode);
        let v = parse_json_bytes(trimmed.as_bytes())?;
        return Ok(Some((v, mode)));
    }
}

fn write_message(stdout: &mut impl Write, v: &Value, framing: Framing) -> io::Result<()> {
    let body = serde_json::to_vec(v).unwrap_or_else(|_| b"{}".to_vec());
    match framing {
        Framing::Ndjson => {
            stdout.write_all(&body)?;
            stdout.write_all(b"\n")?;
        }
        Framing::Lsp => {
            write!(stdout, "Content-Length: {}\r\n\r\n", body.len())?;
            stdout.write_all(&body)?;
        }
    }
    stdout.flush()
}

pub fn run() {
    let mut stdin = io::BufReader::new(io::stdin());
    let mut stdout = io::stdout();
    let mut framing: Option<Framing> = None;
    loop {
        let (msg, mode) = match read_message(&mut stdin, &mut framing) {
            Ok(Some(pair)) => pair,
            Ok(None) => break,
            Err(e) => {
                eprintln!("orbitty-mcp: read: {e}");
                break;
            }
        };
        // Batch arrays not required for MCP-0.
        if let Some(reply) = dispatch(&msg) {
            if let Err(e) = write_message(&mut stdout, &reply, mode) {
                eprintln!("orbitty-mcp: write: {e}");
                break;
            }
        }
    }
}

} // mod imp

#[cfg(unix)]
fn main() {
    imp::run();
}

#[cfg(not(unix))]
fn main() {
    eprintln!("orbitty-mcp is unix-only (Orbitty control socket)");
    std::process::exit(1);
}
