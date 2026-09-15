//! Ephemeral wait-slot blips from a Grok `streaming-json` mouth.
//! Thought / tool names paint `companion:ack` while `companion_send` is
//! in flight. Never a saved turn — the assembled `text` chunks are the
//! only them-row. No invented "searching…" copy: no payload, no blip.

use std::io::BufRead;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::Emitter;

const BLIP_MS: u128 = 350;
const BLIP_CAP: usize = 480;
const BLIP_LINES: usize = 4;

/// Spawn grok (or the same argv), stream NDJSON stdout, splash real
/// thought/tool lines, return concatenated assistant text.
pub fn run_streamed(
    app: &tauri::AppHandle,
    tool: &str,
    bin: &Path,
    args: &[String],
    timeout_secs: u64,
    kill_on: Option<&dyn Fn(&str) -> bool>,
) -> Result<String, String> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("launch {}: {e}", bin.display()))?;

    let (tx_err, rx_err) = std::sync::mpsc::channel::<String>();
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                if tx_err.send(line).is_err() {
                    break;
                }
            }
        });
    }
    let (tx_out, rx_out) = std::sync::mpsc::channel::<String>();
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx_out.send(line).is_err() {
                    break;
                }
            }
        });
    }

    let mut feed = Feed::new();
    let mut stderr_lines: Vec<String> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        while let Ok(line) = rx_err.try_recv() {
            if kill_on.is_some_and(|k| k(&line)) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{tool}: {line}"));
            }
            stderr_lines.push(line);
        }
        while let Ok(line) = rx_out.try_recv() {
            if let Some(msg) = feed.push(app, &line) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{tool}: {msg}"));
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                loop {
                    match rx_out.recv_timeout(Duration::from_millis(50)) {
                        Ok(line) => {
                            if let Some(msg) = feed.push(app, &line) {
                                return Err(format!("{tool}: {msg}"));
                            }
                        }
                        Err(_) => break,
                    }
                }
                if let Some(msg) = feed.error {
                    return Err(format!("{tool}: {msg}"));
                }
                if !status.success() {
                    loop {
                        match rx_err.recv_timeout(Duration::from_millis(50)) {
                            Ok(line) if stderr_lines.len() < 400 => stderr_lines.push(line),
                            Ok(_) | Err(_) => break,
                        }
                    }
                    let stderr = stderr_lines.join("\n");
                    let line = stderr
                        .lines()
                        .rev()
                        .find(|l| !l.trim().is_empty())
                        .unwrap_or("failed");
                    return Err(format!("{tool}: {line}"));
                }
                return Ok(feed.text.trim().to_string());
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "{tool} took longer than {timeout_secs}s — killed. Try again."
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("wait on {}: {e}", tool.to_ascii_lowercase())),
        }
    }
}

struct Feed {
    text: String,
    thought: String,
    error: Option<String>,
    last_blip: Option<Instant>,
}

impl Feed {
    fn new() -> Self {
        Self {
            text: String::new(),
            thought: String::new(),
            error: None,
            last_blip: None,
        }
    }

    /// Parse one NDJSON line. Some(err) = fatal stream error.
    fn push(&mut self, app: &tauri::AppHandle, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let v: Value = serde_json::from_str(line).ok()?;
        if let Some(msg) = event_error(&v) {
            self.error = Some(msg.clone());
            return Some(msg);
        }
        if let Some(chunk) = event_text(&v) {
            self.text.push_str(chunk);
        }
        match v.get("type").and_then(|t| t.as_str()) {
            Some("thought") => {
                if let Some(d) = v.get("data").and_then(|d| d.as_str()) {
                    self.thought.push_str(d);
                    if let Some(blip) = clip(&self.thought) {
                        self.emit(app, &blip);
                    }
                }
            }
            Some("tool_call") => {
                if let Some(blip) = event_blip(&v) {
                    self.emit(app, &blip);
                }
            }
            _ => {}
        }
        None
    }

    fn emit(&mut self, app: &tauri::AppHandle, line: &str) {
        let now = Instant::now();
        if let Some(prev) = self.last_blip {
            if now.duration_since(prev).as_millis() < BLIP_MS {
                return;
            }
        }
        self.last_blip = Some(now);
        let _ = app.emit("companion:ack", json!({ "line": line, "verb": "blip" }));
    }
}

fn clip(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    let mut lines: Vec<String> = t
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect();
    if lines.len() > BLIP_LINES {
        lines = lines[lines.len() - BLIP_LINES..].to_vec();
    }
    let mut s = lines.join("\n");
    if s.chars().count() > BLIP_CAP {
        s = s.chars().take(BLIP_CAP - 1).collect::<String>() + "…";
    }
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn event_blip(v: &Value) -> Option<String> {
    match v.get("type").and_then(|t| t.as_str()) {
        Some("thought") => clip(v.get("data").and_then(|d| d.as_str())?),
        Some("tool_call") => {
            let name = v
                .get("toolName")
                .and_then(|t| t.as_str())
                .or_else(|| v.get("title").and_then(|t| t.as_str()))
                .unwrap_or("");
            clip(name)
        }
        _ => None,
    }
}

fn event_text(v: &Value) -> Option<&str> {
    if v.get("type").and_then(|t| t.as_str()) == Some("text") {
        v.get("data").and_then(|d| d.as_str())
    } else {
        None
    }
}

fn event_error(v: &Value) -> Option<String> {
    if v.get("type").and_then(|t| t.as_str()) == Some("error") {
        v.get("message")
            .and_then(|m| m.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn thought_becomes_blip() {
        let b = event_blip(&v(r#"{"type":"thought","data":"Analyzing the directory structure..."}"#))
            .unwrap();
        assert!(b.starts_with("Analyzing"));
        assert!(event_text(&v(r#"{"type":"thought","data":"x"}"#)).is_none());
    }

    #[test]
    fn tool_name_beats_title() {
        let b = event_blip(&v(
            r#"{"type":"tool_call","toolName":"read_file","title":"Read"}"#,
        ))
        .unwrap();
        assert_eq!(b, "read_file");
    }

    #[test]
    fn empty_thought_is_not_invented() {
        assert!(event_blip(&v(r#"{"type":"thought","data":"  "}"#)).is_none());
        assert!(event_blip(&v(r#"{"type":"text","data":"hello"}"#)).is_none());
    }

    #[test]
    fn text_chunks_assemble() {
        assert_eq!(event_text(&v(r#"{"type":"text","data":"Hello"}"#)), Some("Hello"));
        assert!(event_blip(&v(r#"{"type":"text","data":"Hello"}"#)).is_none());
    }

    #[test]
    fn clip_caps_long() {
        let long = "a".repeat(500);
        let b = clip(&long).unwrap();
        assert!(b.ends_with('…'));
        assert!(b.chars().count() <= BLIP_CAP);
    }

    fn clip_keeps_multiple_lines() {
        let b = clip("one\ntwo\nthree").unwrap();
        assert!(b.contains("one"));
        assert!(b.contains('\n'));
    }
}
