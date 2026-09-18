//! Best-effort EVENT to the private relay after a signed board proof.
//! Default ws://127.0.0.1:13120; ORBITTY_RELAY_* for Tailscale replica.
//! jsonl stays SoT. Failures never fail the board. Never logs the nsec.

use serde_json::{json, Value};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::Manager;
use tungstenite::{client::client, Message, WebSocket};

use crate::event_keys;
use crate::event_log::{KIND_BOARD_DOING, KIND_BOARD_DONE, KIND_BOARD_TODO};

const RELAY_WS_DEFAULT: &str = "ws://127.0.0.1:13120";
const RELAY_ADDR_DEFAULT: &str = "127.0.0.1:13120";
const CONNECT_MS: u64 = 400;
const IO_MS: u64 = 800;
const KIND_AUTH: u64 = 22242;

/// Default loopback sidecar. Override with ORBITTY_RELAY_WS /
/// ORBITTY_RELAY_ADDR for a one-user Tailscale replica (never 0.0.0.0).
fn relay_ws() -> String {
    std::env::var("ORBITTY_RELAY_WS").unwrap_or_else(|_| RELAY_WS_DEFAULT.to_string())
}
fn relay_addr() -> String {
    std::env::var("ORBITTY_RELAY_ADDR").unwrap_or_else(|_| RELAY_ADDR_DEFAULT.to_string())
}

static WARNED: AtomicBool = AtomicBool::new(false);

fn warn_once(why: &str) {
    if !WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("orbitty: private relay publish failed (further failures silent): {why}");
    }
}

fn private_relay_on(app: &tauri::AppHandle) -> bool {
    crate::state::load_state(app)
        .map(|s| s.private_relay)
        .unwrap_or(false)
}

fn tag_values(v: &Value, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(tags) = v.get("tags").and_then(Value::as_array) else {
        return out;
    };
    for t in tags {
        let Some(arr) = t.as_array() else { continue };
        if arr.first().and_then(Value::as_str) == Some(name) {
            if let Some(val) = arr.get(1).and_then(Value::as_str) {
                out.push(val.to_string());
            }
        }
    }
    out
}

/// Kind 420/421/422, `t=orbitty`, signed 64-hex id. Never 31200, never
/// unsigned, never `t=publishable` / `t=nch` / `t=local-only`.
pub fn publishable(line: &Value) -> bool {
    let id = line.get("id").and_then(Value::as_str).unwrap_or("").trim();
    let pubkey = line.get("pubkey").and_then(Value::as_str).unwrap_or("").trim();
    let sig = line.get("sig").and_then(Value::as_str).unwrap_or("").trim();
    if id.len() != 64 || pubkey.is_empty() || sig.is_empty() {
        return false;
    }
    let Some(kind) = line.get("kind").and_then(Value::as_u64) else {
        return false;
    };
    if kind != KIND_BOARD_TODO && kind != KIND_BOARD_DOING && kind != KIND_BOARD_DONE {
        return false;
    }
    let t_tags = tag_values(line, "t");
    if t_tags
        .iter()
        .any(|t| t == "nch" || t == "publishable" || t == "local-only")
    {
        return false;
    }
    t_tags.iter().any(|t| t == "orbitty")
}

/// After a successful jsonl append: if Settings private relay is on and
/// the line is a signed board proof, fire-and-forget EVENT. No-op otherwise.
pub fn publish_best(app: &tauri::AppHandle, line: &Value) {
    if !private_relay_on(app) || !publishable(line) {
        return;
    }
    let dir = match app.path().app_data_dir() {
        Ok(d) => d,
        Err(e) => {
            warn_once(&format!("app data dir: {e}"));
            return;
        }
    };
    let line = line.clone();
    if std::thread::Builder::new()
        .name("orbitty-relay-pub".into())
        .spawn(move || {
            if let Err(e) = publish_now(&dir, &line) {
                warn_once(&e);
            }
        })
        .is_err()
    {
        warn_once("thread");
    }
}

fn publish_now(dir: &Path, line: &Value) -> Result<(), String> {
    let key = event_keys::load_or_create_in(dir)?;
    let mut ws = connect()?;
    let hello = read_json(&mut ws)?;
    let challenge = hello
        .as_array()
        .filter(|a| a.first().and_then(Value::as_str) == Some("AUTH"))
        .and_then(|a| a.get(1))
        .and_then(Value::as_str)
        .ok_or("expected AUTH challenge")?
        .to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let tags = [
        ["relay".to_string(), relay_ws()],
        ["challenge".to_string(), challenge],
    ];
    let signed = event_keys::sign_with(&key, now, KIND_AUTH, &tags, "")?;
    let auth = json!({
        "id": signed.id,
        "pubkey": signed.pubkey,
        "created_at": now,
        "kind": KIND_AUTH,
        "tags": tags,
        "content": "",
        "sig": signed.sig,
    });
    send(&mut ws, json!(["AUTH", auth]))?;
    let ok = read_json(&mut ws)?;
    if ok.get(2) != Some(&json!(true)) {
        return Err(format!("AUTH not ok: {ok}"));
    }
    send(&mut ws, json!(["EVENT", line]))?;
    let ok = read_json(&mut ws)?;
    if ok.get(2) != Some(&json!(true)) {
        return Err(format!("EVENT not ok: {ok}"));
    }
    Ok(())
}

fn addr_allowed(a: SocketAddr) -> bool {
    match a {
        SocketAddr::V4(v) => {
            let ip = v.ip();
            if ip.is_loopback() {
                return true;
            }
            let o = ip.octets();
            // Tailscale CGNAT 100.64.0.0/10 — one-user VPS replica only.
            o[0] == 100 && (o[1] & 0xc0) == 64
        }
        SocketAddr::V6(v) => v.ip().is_loopback(),
    }
}

fn connect() -> Result<WebSocket<TcpStream>, String> {
    let ws_url = relay_ws();
    let addr = relay_addr()
        .to_socket_addrs()
        .map_err(|e| format!("resolve: {e}"))?
        .find(|a| addr_allowed(*a))
        .ok_or_else(|| "relay addr not loopback or Tailscale 100.64/10".to_string())?;
    if addr.port() != 13120 {
        return Err("named port 13120 only".into());
    }
    let stream = TcpStream::connect_timeout(&addr, Duration::from_millis(CONNECT_MS))
        .map_err(|e| format!("tcp: {e}"))?;
    let _ = stream.set_nodelay(true);
    let timeout = Some(Duration::from_millis(IO_MS));
    stream
        .set_read_timeout(timeout)
        .map_err(|e| format!("read timeout: {e}"))?;
    stream
        .set_write_timeout(timeout)
        .map_err(|e| format!("write timeout: {e}"))?;
    let (ws, _) = client(&ws_url, stream).map_err(|e| format!("ws: {e}"))?;
    Ok(ws)
}

fn send(ws: &mut WebSocket<TcpStream>, v: Value) -> Result<(), String> {
    let s = serde_json::to_string(&v).map_err(|e| e.to_string())?;
    ws.send(Message::Text(s)).map_err(|e| format!("send: {e}"))
}

fn read_json(ws: &mut WebSocket<TcpStream>) -> Result<Value, String> {
    loop {
        match ws.read().map_err(|e| format!("read: {e}"))? {
            Message::Text(t) => {
                return serde_json::from_str(&t).map_err(|e| format!("json: {e}"));
            }
            Message::Ping(p) => {
                let _ = ws.send(Message::Pong(p));
            }
            Message::Close(_) => return Err("closed".into()),
            Message::Pong(_) | Message::Frame(_) => {}
            Message::Binary(_) => return Err("binary".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(kind: u64, t: &str, signed: bool) -> Value {
        json!({
            "id": if signed { "ab".repeat(32) } else { "ab".repeat(16) },
            "pubkey": if signed { "cd".repeat(32) } else { String::new() },
            "created_at": 1,
            "kind": kind,
            "tags": [["t", t], ["d", "c-1"]],
            "content": "{}",
            "sig": if signed { "ef".repeat(64) } else { String::new() },
        })
    }

    #[test]
    fn signed_orbitty_420_is_publishable() {
        assert!(publishable(&line(420, "orbitty", true)));
        assert!(publishable(&line(421, "orbitty", true)));
        assert!(publishable(&line(422, "orbitty", true)));
    }

    #[test]
    fn unsigned_is_not_publishable() {
        assert!(!publishable(&line(420, "orbitty", false)));
    }

    #[test]
    fn journal_31200_is_not_publishable() {
        assert!(!publishable(&line(31200, "local-only", true)));
        assert!(!publishable(&line(31200, "orbitty", true)));
    }

    #[test]
    fn nch_and_publishable_tags_are_illegal() {
        assert!(!publishable(&line(420, "nch", true)));
        assert!(!publishable(&line(420, "publishable", true)));
        assert!(!publishable(&line(420, "local-only", true)));
    }
}
