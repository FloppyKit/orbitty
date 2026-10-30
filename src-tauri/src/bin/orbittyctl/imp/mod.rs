use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::process::exit;

mod call;
mod ledger;
mod parse;
mod print;
mod usage;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub(crate) use call::*;
pub(crate) use parse::*;
pub(crate) use print::*;
pub(crate) use usage::*;

pub fn run() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let opts = match parse(&argv) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            exit(EXIT_OTHER);
        }
    };
    if opts.words.first().map(String::as_str) == Some("ledger") {
        match ledger::run(&opts) {
            Ok(text) => {
                print!("{text}");
                exit(0);
            }
            Err(e) => {
                eprintln!("{e}");
                exit(EXIT_OTHER);
            }
        }
    }
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

    let mut req = json!({
        "v": 1,
        "id": "1",
        "tool": tool,
        "args": args,
        "caller": format!("ctl:{}", std::process::id()),
    });
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
