//! Best-effort EVENT to the private relay after a signed board proof.
//! Default local sidecar `ws://127.0.0.1:13120`. Optional replica URL
//! from Settings/ctl (empty = local only). jsonl stays SoT. Failures
//! never fail the board. Never logs the nsec.

use serde_json::{json, Value};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{Emitter, Manager};
use tungstenite::{client::client, Message, WebSocket};

use crate::event_keys;
use crate::event_log::{KIND_BOARD_DOING, KIND_BOARD_DONE, KIND_BOARD_TODO};

const LOCAL_WS: &str = "ws://127.0.0.1:13120";
const LOCAL_ADDR: &str = "127.0.0.1:13120";
const CONNECT_MS: u64 = 400;
const IO_MS: u64 = 800;
const KIND_AUTH: u64 = 22242;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayTarget {
    pub ws: String,
    pub addr: String,
}

fn local_target() -> RelayTarget {
    RelayTarget {
        ws: LOCAL_WS.to_string(),
        addr: LOCAL_ADDR.to_string(),
    }
}

static WARNED_LOCAL: AtomicBool = AtomicBool::new(false);
static WARNED_REPLICA: AtomicBool = AtomicBool::new(false);

fn warn_target(replica: bool, why: &str) {
    let flag = if replica {
        &WARNED_REPLICA
    } else {
        &WARNED_LOCAL
    };
    if flag.swap(true, Ordering::Relaxed) {
        return;
    }
    let which = if replica { "replica" } else { "local" };
    eprintln!(
        "orbitty: private relay {which} publish failed (further {which} failures silent): {why}"
    );
}

fn private_relay_on(app: &tauri::AppHandle) -> bool {
    crate::state::load_state(app)
        .map(|s| s.private_relay)
        .unwrap_or(false)
}

fn replica_url_from_state(app: &tauri::AppHandle) -> String {
    crate::state::load_state(app)
        .map(|s| s.private_relay_url)
        .unwrap_or_default()
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

/// Empty = local sidecar only. Else `ws://` loopback (SSH -L) or
/// Tailscale CGNAT `100.64/10:13120`. Never `0.0.0.0` / public / `wss`.
pub fn parse_replica_url(raw: &str) -> Result<Option<RelayTarget>, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let rest = s
        .strip_prefix("ws://")
        .ok_or_else(|| "replica URL must be ws://host:port (empty = local only)".to_string())?;
    let hostport = rest.trim_end_matches('/');
    if hostport.is_empty() || hostport.contains('/') {
        return Err("replica URL is ws://host:port only".into());
    }
    let (host, port) = split_host_port(hostport)?;
    if host == "0.0.0.0" || host == "::" || host == "*" {
        return Err("refusing 0.0.0.0 / :: — Tailscale or loopback only".into());
    }
    let loopback = host == "127.0.0.1" || host == "::1" || host == "localhost";
    let tailscale = match host.parse::<std::net::Ipv4Addr>() {
        Ok(ip) => {
            let o = ip.octets();
            o[0] == 100 && (o[1] & 0xc0) == 64
        }
        Err(_) => false,
    };
    if !loopback && !tailscale {
        return Err(
            "replica host must be 127.0.0.1 (SSH tunnel) or Tailscale 100.64/10".into(),
        );
    }
    if tailscale && port != 13120 {
        return Err("Tailscale replica uses named port 13120".into());
    }
    if port == 0 {
        return Err("bad port".into());
    }
    let addr_host = if host == "localhost" { "127.0.0.1" } else { &host };
    let addr = if host.contains(':') && !host.starts_with('[') {
        format!("[{addr_host}]:{port}")
    } else {
        format!("{addr_host}:{port}")
    };
    Ok(Some(RelayTarget {
        ws: format!("ws://{hostport}"),
        addr,
    }))
}

fn split_host_port(hostport: &str) -> Result<(String, u16), String> {
    if let Some(rest) = hostport.strip_prefix('[') {
        let (host, port) = rest
            .split_once("]:")
            .ok_or_else(|| "need [ipv6]:port".to_string())?;
        let port: u16 = port.parse().map_err(|_| "bad port".to_string())?;
        return Ok((host.to_string(), port));
    }
    let (host, port) = hostport
        .rsplit_once(':')
        .ok_or_else(|| "need host:port".to_string())?;
    if host.is_empty() {
        return Err("need host:port".into());
    }
    let port: u16 = port.parse().map_err(|_| "bad port".to_string())?;
    Ok((host.to_string(), port))
}

pub fn status(app: &tauri::AppHandle) -> Result<Value, String> {
    let st = crate::state::load_state(app)?;
    Ok(json!({
        "on": st.private_relay,
        "url": st.private_relay_url,
        "local": LOCAL_WS,
    }))
}

pub fn set_enabled(app: &tauri::AppHandle, on: bool) -> Result<Value, String> {
    let st = crate::state::update(app, |st| {
        st.private_relay = on;
        Ok((st.private_relay, st.private_relay_url.clone()))
    })?;
    let _ = app.emit("state:changed", ());
    Ok(json!({ "on": st.0, "url": st.1 }))
}

pub fn set_replica_url(app: &tauri::AppHandle, raw: &str) -> Result<Value, String> {
    let parsed = parse_replica_url(raw)?;
    let stored = parsed
        .as_ref()
        .map(|t| t.ws.clone())
        .unwrap_or_default();
    let on = crate::state::update(app, |st| {
        st.private_relay_url = stored.clone();
        Ok(st.private_relay)
    })?;
    let _ = app.emit("state:changed", ());
    Ok(json!({ "url": stored, "on": on }))
}

/// After a successful jsonl append: if Settings private relay is on and
/// the line is a signed board proof, fire-and-forget EVENT. Local
/// sidecar always; replica URL if set. No-op otherwise.
pub fn publish_best(app: &tauri::AppHandle, line: &Value) {
    if !private_relay_on(app) || !publishable(line) {
        return;
    }
    let dir = match app.path().app_data_dir() {
        Ok(d) => d,
        Err(e) => {
            warn_target(false, &format!("app data dir: {e}"));
            return;
        }
    };
    let replica = match parse_replica_url(&replica_url_from_state(app)) {
        Ok(t) => t,
        Err(e) => {
            warn_target(true, &e);
            None
        }
    };
    let line = line.clone();
    if std::thread::Builder::new()
        .name("orbitty-relay-pub".into())
        .spawn(move || {
            if let Err(e) = publish_now(&dir, &line, &local_target()) {
                warn_target(false, &e);
            }
            if let Some(t) = replica {
                if t.addr != LOCAL_ADDR {
                    if let Err(e) = publish_now(&dir, &line, &t) {
                        warn_target(true, &e);
                    }
                }
            }
        })
        .is_err()
    {
        warn_target(false, "thread");
    }
}

fn publish_now(dir: &Path, line: &Value, target: &RelayTarget) -> Result<(), String> {
    let key = event_keys::load_or_create_in(dir)?;
    let mut ws = connect(target)?;
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
        ["relay".to_string(), target.ws.clone()],
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
            o[0] == 100 && (o[1] & 0xc0) == 64
        }
        SocketAddr::V6(v) => v.ip().is_loopback(),
    }
}

fn connect(target: &RelayTarget) -> Result<WebSocket<TcpStream>, String> {
    let addr = target
        .addr
        .to_socket_addrs()
        .map_err(|e| format!("resolve: {e}"))?
        .find(|a| addr_allowed(*a))
        .ok_or_else(|| "relay addr not loopback or Tailscale 100.64/10".to_string())?;
    if !addr.ip().is_loopback() && addr.port() != 13120 {
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
    let (ws, _) = client(&target.ws, stream).map_err(|e| format!("ws: {e}"))?;
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

    #[test]
    fn empty_replica_url_is_local_only() {
        assert_eq!(parse_replica_url("").unwrap(), None);
        assert_eq!(parse_replica_url("  ").unwrap(), None);
    }

    #[test]
    fn tailscale_and_loopback_replica_ok() {
        let ts = parse_replica_url("ws://100.64.0.1:13120").unwrap().unwrap();
        assert_eq!(ts.ws, "ws://100.64.0.1:13120");
        assert_eq!(ts.addr, "100.64.0.1:13120");
        let tun = parse_replica_url("ws://127.0.0.1:13122").unwrap().unwrap();
        assert_eq!(tun.addr, "127.0.0.1:13122");
        let loc = parse_replica_url("ws://localhost:13120").unwrap().unwrap();
        assert_eq!(loc.addr, "127.0.0.1:13120");
    }

    #[test]
    fn public_and_wildcard_replica_refused() {
        assert!(parse_replica_url("ws://0.0.0.0:13120").is_err());
        assert!(parse_replica_url("ws://1.2.3.4:13120").is_err());
        assert!(parse_replica_url("wss://100.64.0.1:13120").is_err());
        assert!(parse_replica_url("ws://100.64.0.1:443").is_err());
        assert!(parse_replica_url("ws://100.63.0.1:13120").is_err());
        assert!(parse_replica_url("ws://relay.example:13120").is_err());
    }
}
