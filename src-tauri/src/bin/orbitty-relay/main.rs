//! Loopback NIP-01 sidecar for t=orbitty 420/421/422.
//! Default bind 127.0.0.1. Tailscale 100.64/10 ok for one-user VPS.
//! Never 0.0.0.0. Device nsec is read for pubkey; never logged.
//! A non-loopback serve refuses an nsec that matches device.npub (copied Mac key)
//! and an nsec that has no valid 64-hex owner npub.

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
                    "orbitty-relay [--data-dir DIR] [--bind 127.0.0.1:13120|100.x.x.x:13120]\n  smoke   AUTH + EVENT 420 + reject 31200/nch\n  (refuses 0.0.0.0 — Tailscale/SSH replica only)"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown arg {other}")),
        }
    }
    Ok((data, bind, smoke))
}

/// Loopback always. Tailscale CGNAT (100.64.0.0/10) for one-user VPS
/// replicas. Never 0.0.0.0 / :: / other public wildcard listeners.
fn assert_private_bind(bind: &str) -> Result<(), String> {
    let host = bind
        .rsplit_once(':')
        .map(|(h, _)| h.trim_matches(|c| c == '[' || c == ']'))
        .unwrap_or(bind);
    if host == "127.0.0.1" || host == "::1" || host == "localhost" {
        return Ok(());
    }
    if host == "0.0.0.0" || host == "::" || host == "*" {
        return Err(format!("refusing bind {bind} — never 0.0.0.0/::"));
    }
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        let o = ip.octets();
        // Tailscale CGNAT 100.64.0.0/10
        if o[0] == 100 && (o[1] & 0xc0) == 64 {
            return Ok(());
        }
        return Err(format!(
            "refusing bind {bind} — use 127.0.0.1 or a Tailscale 100.64/10 address (not public)"
        ));
    }
    if host.starts_with("fe80:") || host == "::1" {
        return Ok(());
    }
    Err(format!(
        "refusing bind {bind} — use 127.0.0.1 or a Tailscale 100.64/10 address (not public)"
    ))
}

fn relay_ws_url(bind: &str) -> String {
    format!("ws://{bind}")
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
    if let Err(e) = assert_private_bind(&bind) {
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
    nip::replica_refuses_owner_nsec(bind, data_dir)?;
    let pk = nip::load_allowlist_pubkey(data_dir)?;
    eprintln!("orbitty-relay: pubkey {pk}");
    let relay_dir = data_dir.join("relay");
    let store = store::Store::open(&relay_dir)?;
    let store = Arc::new(store);
    let listener = TcpListener::bind(bind).map_err(|e| format!("bind {bind}: {e}"))?;
    let relay_url = relay_ws_url(bind);
    eprintln!("orbitty-relay: listen {bind} ({relay_url})");
    for incoming in listener.incoming() {
        let Ok(stream) = incoming else { continue };
        let _ = stream.set_nodelay(true);
        let store = Arc::clone(&store);
        let pk = pk.clone();
        let relay_url = relay_url.clone();
        let _ = thread::Builder::new()
            .name("orbitty-relay-conn".into())
            .spawn(move || ws::handle(stream, store, pk, relay_url));
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


#[cfg(test)]
mod bind_tests {
    use super::assert_private_bind;
    use super::serve;

    fn mint_pair() -> (String, String) {
        use secp256k1::{Keypair, Secp256k1, SecretKey};
        use std::io::Read;
        let secp = Secp256k1::signing_only();
        let mut urandom = std::fs::File::open("/dev/urandom").unwrap();
        for _ in 0..8 {
            let mut buf = [0u8; 32];
            urandom.read_exact(&mut buf).unwrap();
            if let Ok(sk) = SecretKey::from_slice(&buf) {
                let kp = Keypair::from_secret_key(&secp, &sk);
                let (xonly, _) = kp.x_only_public_key();
                let sec: String = buf.iter().map(|b| format!("{b:02x}")).collect();
                let pk: String = xonly.serialize().iter().map(|b| format!("{b:02x}")).collect();
                buf.fill(0);
                return (sec, pk);
            }
            buf.fill(0);
        }
        panic!("urandom produced no valid secret");
    }

    #[test]
    fn serve_copied_nsec_refuses_before_store() {
        let dir = std::env::temp_dir().join(format!(
            "orbitty-relay-serve-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (sec, pk) = mint_pair();
        std::fs::write(dir.join("device.nsec"), format!("{sec}\n")).unwrap();
        std::fs::write(dir.join("device.npub"), &pk).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                dir.join("device.nsec"),
                std::fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
        let err = serve(&dir, "192.0.2.1:0").unwrap_err();
        assert!(err.contains("refusing replica"), "{err}");
        assert!(!err.contains(&sec), "refusal must not contain the secret hex");
        assert!(
            !dir.join("relay").exists(),
            "serve must not open the store before the replica check"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn loopback_ok() {
        assert!(assert_private_bind("127.0.0.1:13120").is_ok());
        assert!(assert_private_bind("[::1]:13120").is_ok());
    }

    #[test]
    fn tailscale_cgnat_ok() {
        assert!(assert_private_bind("100.64.0.1:13120").is_ok());
        assert!(assert_private_bind("100.127.1.2:13120").is_ok());
        assert!(assert_private_bind("100.70.23.74:13120").is_ok());
    }

    #[test]
    fn wildcard_and_public_refused() {
        assert!(assert_private_bind("0.0.0.0:13120").is_err());
        assert!(assert_private_bind("[::]:13120").is_err());
        assert!(assert_private_bind("1.2.3.4:13120").is_err());
        assert!(assert_private_bind("8.8.8.8:13120").is_err());
        // not Tailscale CGNAT
        assert!(assert_private_bind("100.63.0.1:13120").is_err());
        assert!(assert_private_bind("100.128.0.1:13120").is_err());
    }
}
