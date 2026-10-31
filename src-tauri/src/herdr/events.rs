//! `hc events --json` reader. A missing binary or a stream that ends is not a hang.

use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::core;

pub enum Pull {
    Missing,
    Open(Vec<Value>),
    Ended(Vec<Value>),
}

enum Msg {
    Line(String),
    End,
}

struct KillOnDrop(Option<Child>);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// One short read of `hc events`. EOF → [`Pull::Ended`]. Still running at
/// the timeout → [`Pull::Open`] and the child is stopped.
pub fn pull_once(timeout: Duration) -> Pull {
    let Some(bin) = core::locate() else {
        return Pull::Missing;
    };
    let mut child = match core::command(&bin)
        .args(["events", "--json", "--label", "orbitty=1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return Pull::Missing,
    };
    let Some(out) = child.stdout.take() else {
        return Pull::Missing;
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(out);
        for line in reader.lines() {
            match line {
                Ok(l) if !l.trim().is_empty() => {
                    if tx.send(Msg::Line(l)).is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }
        let _ = tx.send(Msg::End);
    });
    let start = Instant::now();
    let mut lines = Vec::new();
    let mut ended = false;
    while start.elapsed() < timeout {
        match rx.recv_timeout(Duration::from_millis(40)) {
            Ok(Msg::Line(l)) => lines.push(l),
            Ok(Msg::End) => {
                ended = true;
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    ended = true;
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                ended = true;
                break;
            }
        }
    }
    let _guard = KillOnDrop(Some(child));
    let vals = lines
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    if ended {
        Pull::Ended(vals)
    } else {
        Pull::Open(vals)
    }
}

/// Map one hc JSON object onto the fields the wake matcher reads.
pub fn map_value(v: &Value) -> Option<Value> {
    if v.get("seq").is_none() && v.get("id").is_none() && v.get("e_type").is_none() {
        return None;
    }
    Some(v.clone())
}
