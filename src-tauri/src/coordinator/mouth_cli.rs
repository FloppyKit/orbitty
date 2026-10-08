#![allow(unused_imports)]
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};
#[allow(unused_imports)]
use super::hermes::*;
#[allow(unused_imports)]
use super::allowlist::*;
#[allow(unused_imports)]
use super::plug::*;
#[allow(unused_imports)]
use super::commands::*;
#[allow(unused_imports)]
use super::intercept::*;
#[allow(unused_imports)]
use super::send::*;
#[allow(unused_imports)]
use super::soul_wake::*;
#[allow(unused_imports)]
use super::action::*;
#[allow(unused_imports)]
use super::paths::*;
#[allow(unused_imports)]
use super::cursor_perm::*;
#[allow(unused_imports)]
use super::launch_agent::*;
#[cfg(target_os = "macos")]
#[allow(unused_imports)]
use super::mac;

pub(crate) fn is_uuid(s: &str) -> bool {
    let lens = [8usize, 4, 4, 4, 12];
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip(lens)
            .all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// v4 UUID via macOS `uuidgen` (always present on the dogfood target).
pub(crate) fn mint_uuid() -> Result<String, String> {
    let out = Command::new("uuidgen")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("launch uuidgen: {e}"))?;
    if !out.status.success() {
        return Err("uuidgen failed — cannot mint a grok session id.".into());
    }
    let id = String::from_utf8_lossy(&out.stdout).trim().to_ascii_lowercase();
    if !is_uuid(&id) {
        return Err("uuidgen returned a bad id.".into());
    }
    Ok(id)
}

/// Goose session ids look like `20260909_2` (YYYYMMDD_N).
pub(crate) fn is_goose_session_id(s: &str) -> bool {
    let mut parts = s.split('_');
    let (Some(d), Some(n), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    d.len() == 8
        && d.chars().all(|c| c.is_ascii_digit())
        && !n.is_empty()
        && n.chars().all(|c| c.is_ascii_digit())
}

/// Spawn a mouth CLI, poll with a timeout, kill the child on expiry.
/// stdout on exit 0; one honest stderr line otherwise. No desktop lock is
/// ever set here — BERDCTL_LOCK belongs to the berd escape path only.
pub(crate) fn run_mouth_cli(
    tool: &str,
    bin: &Path,
    args: &[String],
    envs: &[(&str, &str)],
    timeout_secs: u64,
) -> Result<String, String> {
    run_mouth_cli_watched(tool, bin, args, envs, timeout_secs, None)
}

/// run_mouth_cli + a stderr watch: a reader thread forwards stderr lines
/// as they arrive, and `kill_on` matching a line kills the child AT ONCE,
/// returning that line as the error. Grok prints a
/// dead-resume 404 (`session get failed: 404 Not Found`) in ~0.5s but
/// STAYS ALIVE — an unwatched poll waits out the whole timeout for a
/// corpse. The kill is line-matched, never time-based: a fat `--rules`
/// soul may slow startup, and a live slow generate keeps the full
/// timeout.
pub(crate) fn run_mouth_cli_watched(
    tool: &str,
    bin: &Path,
    args: &[String],
    envs: &[(&str, &str)],
    timeout_secs: u64,
    kill_on: Option<&dyn Fn(&str) -> bool>,
) -> Result<String, String> {
    use std::io::BufRead as _;
    let mut child = Command::new(bin)
        .args(args)
        .envs(envs.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("launch {}: {e}", bin.display()))?;
    // stderr → line channel. EOF (child dead, pipe closed) drops the
    // sender. The thread is never joined — a grandchild inheriting the
    // pipe must not hang the send path.
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let mut stderr_lines: Vec<String> = Vec::new();
    loop {
        // Drain arrived stderr lines; a kill-line ends it now.
        while let Ok(line) = rx.try_recv() {
            if kill_on.is_some_and(|k| k(&line)) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{tool}: {line}"));
            }
            stderr_lines.push(line);
        }
        match child.try_wait() {
            Ok(Some(_)) => {
                // stderr was taken for the watch, so wait_with_output
                // only collects stdout here.
                let out = child
                    .wait_with_output()
                    .map_err(|e| format!("collect {tool} output: {e}"))?;
                if !out.status.success() {
                    // The last stderr lines may still be in flight behind
                    // the exit — drain bounded (Disconnected = reader hit
                    // EOF; a line cap guards a grandchild holding the
                    // pipe), then the usual one honest error line.
                    loop {
                        match rx.recv_timeout(std::time::Duration::from_secs(2)) {
                            Ok(line) if stderr_lines.len() < 400 => stderr_lines.push(line),
                            Ok(_) | Err(_) => break,
                        }
                    }
                    let stderr = stderr_lines.join("\n").into_bytes();
                    return Err(tool_error_line(tool, &stderr, &out.stdout, out.status.code()));
                }
                return Ok(String::from_utf8_lossy(&out.stdout).trim().to_string());
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "{tool} took longer than {timeout_secs}s — killed. Try again."
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(format!("wait on {}: {e}", tool.to_ascii_lowercase())),
        }
    }
}

