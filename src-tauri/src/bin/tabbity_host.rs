//! Native messaging host for Tabbity. Forwards only the seven tabbity verbs.
//! std + serde_json. No Tauri. Stdout is framed; logs go to stderr.

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const EXT_ID: &str = "pbdnjlgnnkgbmnikjnfakajdkndgdicn";
const CALLER: &str = "tabbity";
const MAX_FRAME: u32 = 1 << 20;
const VERBS: &[&str] = &[
    "tabbity.hello",
    "tabbity.planets",
    "tabbity.claim",
    "tabbity.unclaim",
    "tabbity.update",
    "tabbity.poll",
    "tabbity.ack",
];

fn main() {
    eprintln!("tabbity-host: ppid {}", std::os::unix::process::parent_id());
    let code = run(std::io::stdin(), std::io::stdout(), &sock_path());
    std::process::exit(code);
}

fn sock_path() -> PathBuf {
    if let Ok(p) = std::env::var("ORBITTY_TABBITY_SOCK") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Application Support/space.orbitty.mac/control.sock")
}

fn token_for(sock: &Path) -> Option<String> {
    if let Ok(t) = std::env::var("ORBITTY_CONTROL_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    let file = sock.parent()?.join("control.token");
    std::fs::read_to_string(file)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Build the socket line, or refuse before any connect.
fn gate(msg: &Value) -> Result<Value, String> {
    let origin = msg.get("origin").and_then(|v| v.as_str()).unwrap_or("");
    if origin != EXT_ID {
        return Err("origin refused".into());
    }
    let tool = msg.get("tool").and_then(|v| v.as_str()).unwrap_or("");
    if !VERBS.contains(&tool) {
        return Err(format!("verb refused: {tool}"));
    }
    let args = msg.get("args").cloned().unwrap_or(json!({}));
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    Ok(json!({
        "v": 1,
        "id": id,
        "tool": tool,
        "args": args,
        "caller": CALLER,
    }))
}

fn write_frame(out: &mut impl Write, payload: &Value) -> std::io::Result<()> {
    let body = serde_json::to_vec(payload).unwrap_or_else(|_| b"{}".to_vec());
    let len = body.len() as u32;
    out.write_all(&len.to_ne_bytes())?;
    out.write_all(&body)?;
    out.flush()
}

fn read_frame(input: &mut impl Read) -> Result<Option<Vec<u8>>, String> {
    let mut len_buf = [0u8; 4];
    let mut got = 0;
    while got < 4 {
        match input.read(&mut len_buf[got..]) {
            Ok(0) if got == 0 => return Ok(None),
            Ok(0) => return Err("short length".into()),
            Ok(n) => got += n,
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let len = u32::from_ne_bytes(len_buf);
    if len > MAX_FRAME {
        return Err("frame too large".into());
    }
    let mut body = vec![0u8; len as usize];
    input.read_exact(&mut body).map_err(|e| format!("body: {e}"))?;
    Ok(Some(body))
}

fn socket_call(sock: &Path, req: &Value) -> Result<Value, String> {
    let mut stream = UnixStream::connect(sock).map_err(|e| format!("connect: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(25)));
    let mut line = serde_json::to_string(req).map_err(|e| e.to_string())?;
    line.push('\n');
    stream.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;
    let mut buf = String::new();
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        buf.push(byte[0] as char);
        if byte[0] == b'\n' {
            break;
        }
    }
    serde_json::from_str(buf.trim()).map_err(|e| format!("reply: {e}"))
}

struct HostState {
    browser: Mutex<Option<String>>,
}

fn pin_browser(state: &HostState, args: &Value) {
    let browser = args
        .get("browser")
        .and_then(|v| v.as_str())
        .or_else(|| {
            args.get("claims")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|c| c.get("browser"))
                .and_then(|v| v.as_str())
        });
    if let Some(b) = browser {
        if let Ok(mut slot) = state.browser.lock() {
            if slot.is_none() {
                *slot = Some(b.to_string());
            }
        }
    }
}

fn push_commands(out: &Mutex<impl Write>, data: &Value) {
    let Some(cmds) = data.get("commands").and_then(|v| v.as_array()) else {
        return;
    };
    if let Ok(mut w) = out.lock() {
        for c in cmds {
            let _ = write_frame(&mut *w, c);
        }
        if let Some(ids) = data.get("claimIds") {
            let _ = write_frame(
                &mut *w,
                &json!({"cmd": "reconcile", "claimIds": ids}),
            );
        }
    }
}

fn poll_loop(sock: PathBuf, browser: String, out: Arc<Mutex<impl Write + Send + 'static>>) {
    loop {
        let mut req = json!({
            "v": 1,
            "id": "poll",
            "tool": "tabbity.poll",
            "args": {"browser": browser},
            "caller": CALLER,
        });
        if let Some(t) = token_for(&sock) {
            req["token"] = Value::String(t);
        }
        match socket_call(&sock, &req) {
            Ok(reply) => {
                if let Some(data) = reply.get("data") {
                    push_commands(&out, data);
                }
            }
            Err(e) => {
                eprintln!("tabbity-host poll: {e}");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn handle_body(
    body: &[u8],
    sock: &Path,
    out: &Mutex<impl Write>,
    state: &HostState,
    connects: &mut u32,
) -> Result<(), String> {
    let msg: Value = serde_json::from_slice(body).map_err(|e| format!("bad json: {e}"))?;
    let req = match gate(&msg) {
        Ok(req) => req,
        Err(e) => {
            if let Ok(mut w) = out.lock() {
                let _ = write_frame(
                    &mut *w,
                    &json!({"id": msg.get("id").cloned().unwrap_or(Value::Null), "ok": false, "error": e}),
                );
            }
            return Ok(());
        }
    };
    pin_browser(state, req.get("args").unwrap_or(&Value::Null));
    *connects += 1;
    let mut req = req;
    if let Some(t) = token_for(sock) {
        req["token"] = Value::String(t);
    }
    let reply = socket_call(sock, &req)?;
    if let Ok(mut w) = out.lock() {
        write_frame(&mut *w, &reply).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn run(mut input: impl Read, output: impl Write + Send + 'static, sock: &Path) -> i32 {
    let out = Arc::new(Mutex::new(output));
    let state = HostState {
        browser: Mutex::new(None),
    };
    let mut connects = 0u32;
    let mut poll_started = false;
    loop {
        match read_frame(&mut input) {
            Ok(None) => return 0,
            Ok(Some(body)) => {
                if let Err(e) = handle_body(&body, sock, &out, &state, &mut connects) {
                    if e == "frame too large" {
                        return 1;
                    }
                    eprintln!("tabbity-host: {e}");
                }
                if !poll_started {
                    if let Some(browser) = state.browser.lock().ok().and_then(|b| b.clone()) {
                        poll_started = true;
                        let sock = sock.to_path_buf();
                        let out = Arc::clone(&out);
                        let _ = std::thread::Builder::new()
                            .name("tabbity-poll".into())
                            .spawn(move || poll_loop(sock, browser, out));
                    }
                }
            }
            Err(e) => {
                eprintln!("tabbity-host: {e}");
                if e == "frame too large" {
                    return 1;
                }
                return 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn frame(payload: &Value) -> Vec<u8> {
        let body = serde_json::to_vec(payload).unwrap();
        let mut out = (body.len() as u32).to_ne_bytes().to_vec();
        out.extend(body);
        out
    }

    fn hello() -> Value {
        json!({
            "id": "m1",
            "origin": EXT_ID,
            "tool": "tabbity.hello",
            "args": {"browser": "brave"},
            "caller": "spoof",
            "extra": 1
        })
    }

    #[test]
    fn refused_verbs_do_not_connect() {
        for tool in ["go.start", "claim.front", "companion.send", "state.get"] {
            let msg = json!({"id": "x", "origin": EXT_ID, "tool": tool, "args": {}});
            assert!(gate(&msg).is_err(), "{tool}");
        }
        assert!(gate(&json!({"origin": "nope", "tool": "tabbity.hello", "args": {}})).is_err());
        let ok = gate(&hello()).unwrap();
        assert_eq!(ok["caller"], CALLER);
        assert!(ok.get("extra").is_none());
        assert!(ok.get("origin").is_none());
    }

    #[test]
    fn oversize_frame_refused() {
        let mut input = Vec::new();
        input.extend((MAX_FRAME + 1).to_ne_bytes());
        let err = read_frame(&mut Cursor::new(input)).unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn eof_exits_zero() {
        let dir = std::env::temp_dir().join(format!("tabbity-eof-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let sock = dir.join("none.sock");
        let code = run(Cursor::new(Vec::<u8>::new()), Vec::<u8>::new(), &sock);
        assert_eq!(code, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round_trip_and_refused_without_accept() {
        let dir = std::env::temp_dir().join(format!("tabbity-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("control.sock");
        let listener = UnixListener::bind(&sock_path).unwrap();
        let accepts = Arc::new(AtomicUsize::new(0));
        let accepts2 = Arc::clone(&accepts);
        let path_for_thread = sock_path.clone();
        let server = std::thread::spawn(move || {
            let _ = path_for_thread;
            if let Ok((mut stream, _)) = listener.accept() {
                accepts2.fetch_add(1, Ordering::SeqCst);
                let mut line = String::new();
                let mut byte = [0u8; 1];
                loop {
                    if stream.read(&mut byte).unwrap_or(0) == 0 {
                        break;
                    }
                    line.push(byte[0] as char);
                    if byte[0] == b'\n' {
                        break;
                    }
                }
                let req: Value = serde_json::from_str(line.trim()).unwrap();
                assert_eq!(req["tool"], "tabbity.hello");
                assert_eq!(req["caller"], CALLER);
                let reply = json!({"v": 1, "id": req["id"], "ok": true, "data": {"ok": true}});
                let mut out = serde_json::to_string(&reply).unwrap();
                out.push('\n');
                stream.write_all(out.as_bytes()).unwrap();
            }
        });

        let bad = json!({"id": "b", "origin": EXT_ID, "tool": "go.start", "args": {}});
        let mut input = frame(&bad);
        input.extend(frame(&hello()));
        let output = Arc::new(Mutex::new(Vec::<u8>::new()));
        // Drive one refused message and one round trip without the poll thread
        // by calling handle_body, so a refused verb never accepts.
        let state = HostState {
            browser: Mutex::new(None),
        };
        let mut connects = 0u32;
        handle_body(&serde_json::to_vec(&bad).unwrap(), &sock_path, &output, &state, &mut connects)
            .unwrap();
        assert_eq!(connects, 0);
        assert_eq!(accepts.load(Ordering::SeqCst), 0);
        handle_body(
            &serde_json::to_vec(&hello()).unwrap(),
            &sock_path,
            &output,
            &state,
            &mut connects,
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(connects, 1);
        assert_eq!(accepts.load(Ordering::SeqCst), 1);
        let bytes = output.lock().unwrap().clone();
        assert!(bytes.len() > 4);
        let len = u32::from_ne_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let reply: Value = serde_json::from_slice(&bytes[4..4 + len]).unwrap();
        assert_eq!(reply["ok"], false);
        let _ = input;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn poll_command_is_framed_to_stdout() {
        let out = Mutex::new(Vec::<u8>::new());
        push_commands(
            &out,
            &json!({"commands": [{"cmdId": "c1", "cmd": "focus", "claimId": "tab:x"}], "claimIds": ["tab:x"]}),
        );
        let bytes = out.into_inner().unwrap();
        let len = u32::from_ne_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let cmd: Value = serde_json::from_slice(&bytes[4..4 + len]).unwrap();
        assert_eq!(cmd["cmd"], "focus");
        assert_eq!(cmd["claimId"], "tab:x");
    }
}
