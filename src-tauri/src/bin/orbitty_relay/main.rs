//! Loopback NIP-01 sidecar for t=orbitty 420/421/422.
//! Bind 127.0.0.1 only. Device nsec is read for pubkey; never logged.

mod nip;
mod store;
mod ws;

use serde_json::json;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

const BIND: &str = "127.0.0.1:13120";
const RELAY_URL: &str = "ws://127.0.0.1:13120";

fn default_data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join("Library/Application Support/space.orbitty.mac")
}

fn parse_args() -> Result<(PathBuf, String, bool), String> {
    let mut data = default_data_dir();
    let mut bind = BIND.to_string();
    let mut smoke = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "smoke" | "--smoke" => smoke = true,
            "--data-dir" => {
                data = PathBuf::from(it.next().ok_or("--data-dir needs a path")?);
            }
            "--bind" => {
                bind = it.next().ok_or("--bind needs host:port")?;
            }
            "-h" | "--help" => {
                eprintln!(
                    "orbitty-relay [--data-dir DIR] [--bind 127.0.0.1:13120]\n  smoke   AUTH + EVENT 420 + reject 31200/nch"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown arg {other}")),
        }
    }
    Ok((data, bind, smoke))
}

fn assert_loopback(bind: &str) -> Result<(), String> {
    let host = bind.rsplit_once(':').map(|(h, _)| h).unwrap_or(bind);
    if host == "127.0.0.1" || host == "::1" || host == "[::1]" {
        Ok(())
    } else {
        Err(format!("refusing bind {bind} — loopback only"))
    }
}

fn main() {
    if std::env::var("ORBITTY_RELAY_DISABLE").ok().as_deref() == Some("1") {
        eprintln!("orbitty-relay: ORBITTY_RELAY_DISABLE=1 — not binding");
        return;
    }
    let (data_dir, bind, smoke) = match parse_args() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("orbitty-relay: {e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = assert_loopback(&bind) {
        eprintln!("orbitty-relay: {e}");
        std::process::exit(2);
    }
    if smoke {
        if let Err(e) = run_smoke(&data_dir, &bind) {
            eprintln!("orbitty-relay smoke: {e}");
            std::process::exit(1);
        }
        println!("orbitty-relay smoke ok");
        return;
    }
    if let Err(e) = serve(&data_dir, &bind) {
        eprintln!("orbitty-relay: {e}");
        std::process::exit(1);
    }
}

fn serve(data_dir: &PathBuf, bind: &str) -> Result<(), String> {
    let key_path = data_dir.join("device.nsec");
    let device = nip::load_device(&key_path)?;
    eprintln!("orbitty-relay: pubkey {}", device.pubkey);
    let relay_dir = data_dir.join("relay");
    let store = store::Store::open(&relay_dir)?;
    let store = Arc::new(store);
    let listener = TcpListener::bind(bind).map_err(|e| format!("bind {bind}: {e}"))?;
    eprintln!("orbitty-relay: listen {bind}");
    for incoming in listener.incoming() {
        let Ok(stream) = incoming else { continue };
        let _ = stream.set_nodelay(true);
        let store = Arc::clone(&store);
        let pk = device.pubkey.clone();
        let _ = thread::Builder::new()
            .name("orbitty-relay-conn".into())
            .spawn(move || ws::handle(stream, store, pk));
    }
    Ok(())
}

fn run_smoke(data_dir: &PathBuf, bind: &str) -> Result<(), String> {
    let url = if bind == BIND {
        RELAY_URL.to_string()
    } else {
        format!("ws://{bind}")
    };
    let device = nip::load_device(&data_dir.join("device.nsec"))?;
    let mut ws = ws::client_connect(&url)?;
    let hello = ws::client_read(&mut ws)?;
    let challenge = hello
        .as_array()
        .filter(|a| a.first().and_then(|x| x.as_str()) == Some("AUTH"))
        .and_then(|a| a.get(1))
        .and_then(|x| x.as_str())
        .ok_or("expected AUTH challenge")?
        .to_string();
    let now = nip::unix_now();
    let auth = nip::sign_event(
        &device,
        now,
        nip::KIND_AUTH,
        &[
            vec!["relay".into(), url.clone()],
            vec!["challenge".into(), challenge],
        ],
        "",
    )?;
    ws::client_send(&mut ws, json!(["AUTH", auth]))?;
    let ok = ws::client_read(&mut ws)?;
    if ok.get(2) != Some(&json!(true)) {
        return Err(format!("AUTH not ok: {ok}"));
    }

    let ev = nip::sign_event(
        &device,
        now,
        nip::KIND_TODO,
        &[
            vec!["t".into(), "orbitty".into()],
            vec!["d".into(), "c-relay-smoke".into()],
        ],
        r#"{"cardId":"c-relay-smoke","title":"relay sidecar smoke"}"#,
    )?;
    let id = ev.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
    ws::client_send(&mut ws, json!(["EVENT", ev]))?;
    let ok = ws::client_read(&mut ws)?;
    if ok.get(2) != Some(&json!(true)) {
        return Err(format!("EVENT 420 not ok: {ok}"));
    }

    ws::client_send(
        &mut ws,
        json!(["REQ", "s1", { "kinds": [420], "#d": ["c-relay-smoke"] }]),
    )?;
    let mut saw = false;
    loop {
        let msg = ws::client_read(&mut ws)?;
        let cmd = msg.as_array().and_then(|a| a.first()).and_then(|x| x.as_str());
        match cmd {
            Some("EVENT") => {
                let got = msg.get(2).and_then(|e| e.get("id")).and_then(|x| x.as_str());
                if got == Some(id.as_str()) {
                    saw = true;
                }
            }
            Some("EOSE") => break,
            _ => return Err(format!("unexpected {msg}")),
        }
    }
    if !saw {
        return Err("REQ missed the 420".into());
    }

    let bad_kind = nip::sign_event(
        &device,
        now + 1,
        31200,
        &[vec!["t".into(), "local-only".into()]],
        "{}",
    )?;
    ws::client_send(&mut ws, json!(["EVENT", bad_kind]))?;
    let ok = ws::client_read(&mut ws)?;
    if ok.get(2) != Some(&json!(false)) {
        return Err(format!("31200 should reject: {ok}"));
    }

    let bad_t = nip::sign_event(
        &device,
        now + 2,
        nip::KIND_TODO,
        &[
            vec!["t".into(), "nch".into()],
            vec!["d".into(), "c-nch".into()],
        ],
        "{}",
    )?;
    ws::client_send(&mut ws, json!(["EVENT", bad_t]))?;
    let ok = ws::client_read(&mut ws)?;
    if ok.get(2) != Some(&json!(false)) {
        return Err(format!("t=nch should reject: {ok}"));
    }
    Ok(())
}
