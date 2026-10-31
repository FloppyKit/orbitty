//! Read-only status capsule and terminal preview.
//! Refresh is the caller's timer (2s while open). This module only reads.

use serde::Serialize;

use super::client;
use super::core;
use super::health::{self, Source};
use super::redact;
use super::tui;

const MAX_LINES: usize = 200;
const MAX_LINE_CHARS: usize = 400;
const MAX_BYTES: usize = 16 * 1024;
pub(crate) const DEFAULT_LINES: u32 = 40;

#[derive(Clone, Debug, Serialize)]
pub struct View {
    pub card: String,
    pub status: String,
    pub state: String,
    pub text: String,
    pub source: String,
    pub up: bool,
}

/// Which backend a peek reads, and the id it passes. A run row wins over
/// the live flag: core reads `herdr.terminal`, tui reads the agent name
/// (today's pane path). No row, or a core row with an empty terminal,
/// keeps today's flag choice and the card id. An empty terminal is not
/// sent to hc.
pub(crate) fn resolve_peek(runs: &[crate::state::AgentRun], card: &str, core_flag: bool) -> (bool, String) {
    let id = card.strip_prefix("agent:").unwrap_or(card);
    if let Some(h) = runs
        .iter()
        .find(|r| r.card_id == id)
        .and_then(|r| r.herdr.as_ref())
    {
        if h.backend == "core" {
            let term = h.terminal.trim();
            if !term.is_empty() {
                return (true, term.to_string());
            }
        } else {
            let name = h.agent_name.trim();
            let target = if name.is_empty() { id } else { name };
            return (false, target.to_string());
        }
    }
    (core_flag, id.to_string())
}

pub(crate) fn view(runs: &[crate::state::AgentRun], card: &str, lines: u32, body: bool) -> View {
    let (use_core, target) = resolve_peek(runs, card, client::core_on());
    if use_core {
        core_view(&target, lines, body)
    } else {
        tui_view(&target, body)
    }
}

fn core_view(target: &str, lines: u32, body: bool) -> View {
    if health::is_down() {
        return unknown(target);
    }
    let Some(bin) = core::locate() else {
        health::force_fallback(health::now_ms());
        return unknown(target);
    };
    let term = match core::terminal(&bin, target) {
        Ok(v) => v,
        Err(e) if e.starts_with("not_found") => {
            return View {
                card: target.into(),
                status: "none".into(),
                state: "finished".into(),
                text: String::new(),
                source: Source::Herdr.as_str().into(),
                up: true,
            };
        }
        Err(_) => {
            health::force_fallback(health::now_ms());
            return unknown(target);
        }
    };
    let status = core::map_status(&term);
    let state = core::map_state(&term);
    let text = if body {
        let n = clamp_lines(lines);
        match core::read_text(&bin, target, n) {
            Ok(raw) => finish_text(&raw),
            Err(_) => {
                health::force_fallback(health::now_ms());
                return unknown(target);
            }
        }
    } else {
        String::new()
    };
    View {
        card: target.into(),
        status,
        state,
        text,
        source: Source::Herdr.as_str().into(),
        up: true,
    }
}

fn tui_view(target: &str, body: bool) -> View {
    let Some(bin) = tui::locate() else {
        return unknown(target);
    };
    let status = tui::agent_status(&bin, target).unwrap_or_else(|| "none".into());
    let text = if body {
        match tui::agent_read(&bin, target) {
            Ok(raw) => finish_text(&raw),
            Err(_) => return unknown(target),
        }
    } else {
        String::new()
    };
    View {
        card: target.into(),
        status,
        state: "live".into(),
        text,
        source: "tui".into(),
        up: true,
    }
}

fn unknown(card: &str) -> View {
    View {
        card: card.into(),
        status: "unknown".into(),
        state: String::new(),
        text: String::new(),
        source: Source::Fallback.as_str().into(),
        up: false,
    }
}

pub(crate) fn clamp_lines(lines: u32) -> u32 {
    if lines == 0 {
        DEFAULT_LINES
    } else {
        lines.min(MAX_LINES as u32)
    }
}

pub(crate) fn finish_text(raw: &str) -> String {
    let stripped = strip_terminal(raw);
    let redacted = redact::redact(&stripped);
    cap_preview(&redacted)
}

/// CSI and OSC (7501 status, OSC 8 links) are removed. `<` cannot form HTML.
pub fn strip_terminal(input: &str) -> String {
    let b = input.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == 0x1b {
            i += 1;
            if i >= b.len() {
                break;
            }
            if b[i] == b'[' {
                i += 1;
                while i < b.len() {
                    let c = b[i];
                    i += 1;
                    if (0x40..=0x7e).contains(&c) {
                        break;
                    }
                }
                continue;
            }
            if b[i] == b']' {
                i += 1;
                while i < b.len() {
                    if b[i] == 0x07 {
                        i += 1;
                        break;
                    }
                    if b[i] == 0x1b && i + 1 < b.len() && b[i + 1] == b'\\' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            i += 1;
            continue;
        }
        let ch = input[i..].chars().next().unwrap_or('\u{FFFD}');
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            _ => out.push(ch),
        }
        i += ch.len_utf8();
    }
    out
}

pub fn cap_preview(text: &str) -> String {
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| l.chars().take(MAX_LINE_CHARS).collect())
        .collect();
    let mut hidden: u32 = 0;
    let content_cap = MAX_LINES.saturating_sub(1);
    if lines.len() > content_cap {
        hidden += (lines.len() - content_cap) as u32;
        lines.truncate(content_cap);
    }
    loop {
        let marker = marker_line(hidden);
        let body = lines.join("\n");
        let extra = if hidden > 0 { 1 + marker.len() } else { 0 };
        if body.len() + extra <= MAX_BYTES || lines.is_empty() {
            break;
        }
        lines.pop();
        hidden += 1;
    }
    let mut body = lines.join("\n");
    if hidden > 0 {
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(&marker_line(hidden));
    }
    if body.len() > MAX_BYTES {
        body.truncate(MAX_BYTES);
    }
    body
}

fn marker_line(hidden: u32) -> String {
    format!("… {hidden} lines hidden")
}

/// Snapshot + one events sample + reads across `duration_ms` at a 2s step.
/// Virtual clock: the caller does not sleep. Used to prove the read set.
pub(crate) fn exercise_open(target: &str, duration_ms: u64) {
    if let Some(bin) = core::locate() {
        let _ = core::events_open(&bin);
    }
    let mut t = 0u64;
    let mut last_ls: Option<u64> = None;
    while t <= duration_ms {
        if last_ls.map(|p| t.saturating_sub(p) >= 30_000).unwrap_or(true) {
            if let Some(bin) = core::locate() {
                let _ = core::list(&bin);
            }
            last_ls = Some(t);
        }
        let _ = view(&[], target, DEFAULT_LINES, true);
        t += 2_000;
    }
}

pub(crate) fn hover_get(target: &str) {
    if let Some(bin) = core::locate() {
        let _ = core::terminal(&bin, target);
    }
}
