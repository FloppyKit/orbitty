//! One websocket client. AUTH first, then EVENT/REQ/CLOSE.

use serde_json::{json, Value};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use tungstenite::{accept, Message, WebSocket};

use super::nip;
use super::store::Store;

pub fn handle(
    stream: TcpStream,
    store: Arc<Mutex<Store>>,
    device_pubkey: String,
    relay_url: String,
) {
    let mut ws = match accept(stream) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("orbitty-relay: handshake: {e}");
            return;
        }
    };
    let mut challenge = [0u8; 16];
    nip::fill_random(&mut challenge);
    let challenge = hex16(&challenge);
    if send(&mut ws, json!(["AUTH", challenge])).is_err() {
        return;
    }
    let mut authed = false;
    while let Ok(msg) = ws.read() {
        let text = match msg {
            Message::Text(t) => t,
            Message::Ping(p) => {
                let _ = ws.send(Message::Pong(p));
                continue;
            }
            Message::Close(_) | Message::Binary(_) => break,
            Message::Pong(_) | Message::Frame(_) => continue,
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            let _ = send(&mut ws, json!(["NOTICE", "invalid: bad json"]));
            continue;
        };
        let Some(arr) = v.as_array() else {
            let _ = send(&mut ws, json!(["NOTICE", "invalid: not an array"]));
            continue;
        };
        let cmd = arr.first().and_then(Value::as_str).unwrap_or("");
        match cmd {
            "AUTH" => {
                let Some(ev) = arr.get(1) else {
                    let _ = send(&mut ws, json!(["NOTICE", "auth-required: missing event"]));
                    continue;
                };
                match nip::auth_ok(ev, &device_pubkey, &challenge, &relay_url) {
                    Ok(()) => {
                        authed = true;
                        let id = ev.get("id").and_then(Value::as_str).unwrap_or("");
                        let _ = send(&mut ws, json!(["OK", id, true, ""]));
                    }
                    Err(e) => {
                        let id = ev.get("id").and_then(Value::as_str).unwrap_or("");
                        eprintln!("orbitty-relay: AUTH reject: {e}");
                        let _ = send(&mut ws, json!(["OK", id, false, e]));
                    }
                }
            }
            "EVENT" => {
                let Some(ev) = arr.get(1).cloned() else {
                    let _ = send(&mut ws, json!(["NOTICE", "invalid: missing event"]));
                    continue;
                };
                let id = ev.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                if !authed {
                    eprintln!("orbitty-relay: EVENT reject: auth-required");
                    let _ = send(&mut ws, json!(["OK", id, false, "auth-required: AUTH first"]));
                    continue;
                }
                match nip::allow_store(&ev, &device_pubkey) {
                    Ok(()) => match store.lock() {
                        Ok(mut st) => match st.insert(ev) {
                            Ok(true) => {
                                let _ = send(&mut ws, json!(["OK", id, true, ""]));
                            }
                            Ok(false) => {
                                let _ = send(&mut ws, json!(["OK", id, true, "duplicate:"]));
                            }
                            Err(e) => {
                                eprintln!("orbitty-relay: store: {e}");
                                let _ = send(&mut ws, json!(["OK", id, false, format!("error: {e}")]));
                            }
                        },
                        Err(_) => {
                            let _ = send(&mut ws, json!(["OK", id, false, "error: lock"]));
                        }
                    },
                    Err(e) => {
                        eprintln!("orbitty-relay: EVENT reject: {e}");
                        let _ = send(&mut ws, json!(["OK", id, false, e]));
                    }
                }
            }
            "REQ" => {
                if !authed {
                    let sub = arr.get(1).and_then(Value::as_str).unwrap_or("");
                    let _ = send(&mut ws, json!(["CLOSED", sub, "auth-required: AUTH first"]));
                    continue;
                }
                let sub = arr.get(1).and_then(Value::as_str).unwrap_or("").to_string();
                let filters: Vec<Value> = arr.iter().skip(2).cloned().collect();
                let events = match store.lock() {
                    Ok(st) => st.query(&filters),
                    Err(_) => Vec::new(),
                };
                for ev in events {
                    if send(&mut ws, json!(["EVENT", sub, ev])).is_err() {
                        return;
                    }
                }
                let _ = send(&mut ws, json!(["EOSE", sub]));
            }
            "CLOSE" => {
                let sub = arr.get(1).and_then(Value::as_str).unwrap_or("");
                let _ = send(&mut ws, json!(["CLOSED", sub, ""]));
            }
            _ => {
                let _ = send(&mut ws, json!(["NOTICE", "invalid: unknown command"]));
            }
        }
    }
}

fn send(ws: &mut WebSocket<TcpStream>, v: Value) -> Result<(), ()> {
    let s = serde_json::to_string(&v).map_err(|_| ())?;
    ws.send(Message::Text(s)).map_err(|_| ())
}

fn hex16(bytes: &[u8; 16]) -> String {
    let mut s = String::with_capacity(32);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Client helper for `orbitty-relay smoke`.
pub fn client_connect(url: &str) -> Result<WebSocket<TcpStream>, String> {
    let host = url
        .strip_prefix("ws://")
        .ok_or("need ws://host:port")?;
    let stream = TcpStream::connect(host).map_err(|e| format!("tcp: {e}"))?;
    let (ws, _) = tungstenite::client::client(url, stream).map_err(|e| format!("ws: {e}"))?;
    Ok(ws)
}

pub fn client_read(ws: &mut WebSocket<TcpStream>) -> Result<Value, String> {
    match ws.read().map_err(|e| format!("read: {e}"))? {
        Message::Text(t) => serde_json::from_str(&t).map_err(|e| format!("json: {e}")),
        other => Err(format!("unexpected {other:?}")),
    }
}

pub fn client_send(ws: &mut WebSocket<TcpStream>, v: Value) -> Result<(), String> {
    let s = serde_json::to_string(&v).map_err(|e| e.to_string())?;
    ws.send(Message::Text(s)).map_err(|e| format!("send: {e}"))
}
