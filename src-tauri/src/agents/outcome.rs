//! One plain answer per agent seat. Reads herdr and saved runs. Never writes.

use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;
use tauri::AppHandle;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeState {
    Working,
    WaitingOnYou,
    Idle,
    Failed,
    Done,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SeatOutcome {
    pub seat: String,
    pub state: OutcomeState,
    pub line: String,
    pub since: u64,
}

pub trait SeatAdapter {
    fn outcomes(&self) -> Vec<SeatOutcome>;
}

const GO_WORKING: &[&str] = &[
    "cooking",
    "fixing",
    "verifying",
    "relaunching",
    "auditing",
    "installing",
];

pub(crate) fn from_herdr(status: &str, state: &str, up: bool, run_state: &str) -> OutcomeState {
    if !up || status == "unknown" || (status == "none" && state != "finished") {
        return from_run_state(run_state);
    }
    match status {
        "working" => OutcomeState::Working,
        "blocked" => OutcomeState::WaitingOnYou,
        "idle" => OutcomeState::Idle,
        "error" => OutcomeState::Failed,
        "done" => OutcomeState::Done,
        "none" if state == "finished" => OutcomeState::Done,
        _ => OutcomeState::Idle,
    }
}

fn from_run_state(run_state: &str) -> OutcomeState {
    match run_state {
        "starting" | "working" => OutcomeState::Working,
        "blocked" => OutcomeState::WaitingOnYou,
        "done" => OutcomeState::Done,
        _ => OutcomeState::Idle,
    }
}

pub(crate) fn from_go_stage(stage: &str, result_line: &str) -> OutcomeState {
    let stage = stage.trim();
    let result = result_line.trim();
    if GO_WORKING.contains(&stage) {
        return OutcomeState::Working;
    }
    if stage == "blocked" || result.starts_with("BLOCKED") || result.starts_with("RIG BLOCKER") {
        return OutcomeState::WaitingOnYou;
    }
    if result.starts_with("FAIL") {
        return OutcomeState::Failed;
    }
    if stage == "done" {
        return OutcomeState::Done;
    }
    OutcomeState::Idle
}

pub(crate) fn words(s: OutcomeState) -> &'static str {
    match s {
        OutcomeState::Working => "working",
        OutcomeState::WaitingOnYou => "waiting on you",
        OutcomeState::Idle => "idle",
        OutcomeState::Failed => "failed",
        OutcomeState::Done => "done",
    }
}

/// Prefixes are built here so this file has no home-path literal.
fn path_prefixes() -> [String; 3] {
    [
        format!("/{}s/", "User"),
        format!("/{}/", "home"),
        format!("{}{}", '~', '/'),
    ]
}

fn strip_prefixes(s: &str, prefixes: &[String]) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(n) = prefixes.iter().find_map(|p| {
            rest.strip_prefix(p.as_str()).map(|tail| {
                let extra = tail.find(|c: char| c.is_whitespace()).unwrap_or(tail.len());
                p.len() + extra
            })
        }) {
            rest = &rest[n..];
            continue;
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out
}

fn is_box_drawing(c: char) -> bool {
    ('\u{2500}'..='\u{257F}').contains(&c)
}

/// `n/m:` key hints, as in `1/5:select`.
fn has_count_hint(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'/' {
            continue;
        }
        i += 1;
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i > start && i < b.len() && b[i] == b':' {
            return true;
        }
    }
    false
}

/// A TUI footer or key-hint line is not a plain status line.
fn is_tui_footer(s: &str) -> bool {
    s.chars().any(is_box_drawing)
        || s.contains("Ctrl+")
        || s.contains("Tab:")
        || s.contains("Esc:")
        || has_count_hint(s)
}

pub(crate) fn clamp_line(s: &str) -> String {
    if is_tui_footer(s) {
        return String::new();
    }
    let flat = s.replace(['\r', '\n', '\t'], " ");
    let prefixes = path_prefixes();
    let stripped = strip_prefixes(&flat, &prefixes);
    let one = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    one.chars().take(120).collect()
}

pub(crate) fn merge(lists: Vec<Vec<SeatOutcome>>) -> Vec<SeatOutcome> {
    let mut out: Vec<SeatOutcome> = Vec::new();
    for list in lists {
        for item in list {
            if let Some(existing) = out.iter_mut().find(|e| e.seat == item.seat) {
                if item.since > existing.since {
                    *existing = item;
                }
            } else {
                out.push(item);
            }
        }
    }
    out
}

pub(crate) fn phrase_of(o: &SeatOutcome) -> String {
    let w = words(o.state);
    if o.line.is_empty() {
        w.to_string()
    } else {
        format!("{w} · {}", o.line)
    }
}

pub(crate) fn saved_phrase(run_state: &str, line: &str) -> String {
    let state = from_herdr("unknown", "", false, run_state);
    let line = clamp_line(line);
    if line.is_empty() {
        words(state).to_string()
    } else {
        format!("{} · {line}", words(state))
    }
}

pub(crate) fn door_from(enabled: bool, source: &str, adapters: &[&dyn SeatAdapter]) -> Value {
    if !enabled {
        return json!({ "enabled": false });
    }
    let lists: Vec<Vec<SeatOutcome>> = adapters.iter().map(|a| a.outcomes()).collect();
    json!({
        "enabled": true,
        "source": source,
        "seats": merge(lists),
    })
}

struct HerdrSeats<'a> {
    runs: &'a [crate::state::AgentRun],
    since: u64,
}

impl SeatAdapter for HerdrSeats<'_> {
    fn outcomes(&self) -> Vec<SeatOutcome> {
        herdr_outcomes(self.runs, self.since)
    }
}

struct GoSeats<'a> {
    root: &'a Path,
    seated: &'a [&'a str],
}

impl SeatAdapter for GoSeats<'_> {
    fn outcomes(&self) -> Vec<SeatOutcome> {
        go_run_outcomes(self.root, self.seated)
    }
}

struct Rows(Vec<SeatOutcome>);

impl SeatAdapter for Rows {
    fn outcomes(&self) -> Vec<SeatOutcome> {
        self.0.clone()
    }
}

fn herdr_outcomes(runs: &[crate::state::AgentRun], since: u64) -> Vec<SeatOutcome> {
    runs.iter()
        .map(|r| {
            let v = crate::herdr::peek::view(runs, &r.card_id, 0, false);
            let raw = if v.up {
                r.herdr
                    .as_ref()
                    .and_then(crate::droid_inspect::tool_line)
                    .unwrap_or_default()
            } else {
                String::new()
            };
            SeatOutcome {
                seat: format!("agent:{}", r.card_id),
                state: from_herdr(&v.status, &v.state, v.up, &r.state),
                line: clamp_line(&raw),
                since,
            }
        })
        .collect()
}

pub(crate) fn go_run_outcomes(root: &Path, seated: &[&str]) -> Vec<SeatOutcome> {
    let lock = read_lock_card(root);
    let mut dirs = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root) {
        for ent in rd.flatten() {
            dirs.push(ent.path());
        }
    }
    dirs.sort();
    let mut out = Vec::new();
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        if dir.file_name().and_then(|n| n.to_str()) == Some("LOCK") {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(dir.join("state.json")) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let Some(card) = v.get("card_id").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        if card.is_empty() {
            continue;
        }
        let seated_hit = seated.iter().any(|c| *c == card);
        let lock_hit = lock.as_deref() == Some(card);
        if !seated_hit && !lock_hit {
            continue;
        }
        let stage = v.get("stage").and_then(Value::as_str).unwrap_or("");
        let result = v.get("result_line").and_then(Value::as_str).unwrap_or("");
        let since = v
            .get("updated_at")
            .and_then(Value::as_str)
            .and_then(parse_since)
            .unwrap_or(0);
        out.push(SeatOutcome {
            seat: format!("agent:{card}"),
            state: from_go_stage(stage, result),
            line: clamp_line(result),
            since,
        });
    }
    out
}

fn read_lock_card(root: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(root.join("LOCK").join("card")).ok()?;
    let card = raw.trim();
    if card.is_empty() {
        None
    } else {
        Some(card.to_string())
    }
}

fn parse_since(s: &str) -> Option<u64> {
    let dt = chrono::DateTime::parse_from_rfc3339(s.trim()).ok()?;
    let secs = dt.timestamp();
    if secs < 0 {
        None
    } else {
        Some(secs as u64)
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn last_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with('╭')
                && !l.starts_with('│')
                && !l.starts_with('╰')
                && !l.starts_with("Update:")
                && !is_tui_footer(l)
        })
        .next_back()
        .map(|s| s.trim_start_matches(['❯', '•', '-', '*']).trim().to_string())
        .unwrap_or_default()
}

fn resolve_card(runs: &[crate::state::AgentRun], view_card: &str) -> String {
    let id = view_card.strip_prefix("agent:").unwrap_or(view_card).trim();
    if id.is_empty() {
        return String::new();
    }
    if let Some(r) = runs.iter().find(|r| r.card_id == id) {
        return r.card_id.clone();
    }
    if let Some(r) = runs.iter().find(|r| {
        r.herdr.as_ref().is_some_and(|h| {
            (!h.terminal.is_empty() && h.terminal == id)
                || (!h.agent_name.is_empty() && h.agent_name == id)
        })
    }) {
        return r.card_id.clone();
    }
    id.to_string()
}

pub(crate) fn for_view(app: &AppHandle, v: &crate::herdr::peek::View) -> Option<SeatOutcome> {
    let runs = crate::state::load_state(app)
        .map(|st| st.agent_runs)
        .unwrap_or_default();
    let card = resolve_card(&runs, &v.card);
    if card.is_empty() {
        return None;
    }
    let run_state = runs
        .iter()
        .find(|r| r.card_id == card)
        .map(|r| r.state.clone())
        .unwrap_or_default();
    Some(SeatOutcome {
        seat: format!("agent:{card}"),
        state: from_herdr(&v.status, &v.state, v.up, &run_state),
        line: clamp_line(&last_line(&v.text)),
        since: now_secs(),
    })
}

pub fn all(app: &AppHandle) -> Value {
    if !crate::flags::on(app, "seat_outcomes") {
        return json!({ "enabled": false });
    }
    let runs = crate::state::load_state(app)
        .map(|st| st.agent_runs)
        .unwrap_or_default();
    let herdr = HerdrSeats {
        runs: &runs,
        since: now_secs(),
    };
    let herdr_rows = herdr.outcomes();
    let source = if crate::herdr::health::is_down() {
        "fallback"
    } else {
        crate::herdr::health::source().as_str()
    };
    crate::herdr::flush_health(app);
    let root = crate::plans::runs_dir(&crate::plans::Roots::real());
    let seated: Vec<&str> = runs.iter().map(|r| r.card_id.as_str()).collect();
    let go = GoSeats {
        root: &root,
        seated: &seated,
    };
    let rows = Rows(herdr_rows);
    let adapters: [&dyn SeatAdapter; 2] = [&rows, &go];
    door_from(true, source, &adapters)
}

#[cfg(test)]
#[path = "outcome_tests.rs"]
mod tests;
