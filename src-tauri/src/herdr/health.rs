//! Shared herdr rule: DOWN when the hc binary is missing, `hc server
//! status` is not ok, or the events stream ends. Recheck every 30s.
//! A cold start (`last_check_ms == 0`) treats the first healthy sample
//! as proven. Recovery after a sample has already been taken still
//! needs 2 consecutive healthy checks. At most one switch per 30s.
//! Each switch records source (herdr | fallback) and, on a forced
//! fallback, the error kind. While DOWN, callers return unknown — never Died.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use super::core;

const RECHECK_MS: u64 = 30_000;
const HEALTHY_NEED: u32 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Source {
    Herdr,
    Fallback,
}

impl Source {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Source::Herdr => "herdr",
            Source::Fallback => "fallback",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HealthEvent {
    pub source: &'static str,
    pub kind: &'static str,
}

struct Note {
    source: &'static str,
    kind: &'static str,
}

struct State {
    source: Source,
    /// A failed read/status. Stays set until enough healthy checks, even
    /// when the source switch itself is waiting out the 30s limit.
    faulted: bool,
    consecutive_ok: u32,
    last_check_ms: u64,
    last_switch_ms: u64,
    notes: Vec<Note>,
    logged: usize,
    clock_ms: Option<u64>,
}

fn state() -> &'static Mutex<State> {
    static STATE: Mutex<State> = Mutex::new(State {
        source: Source::Fallback,
        faulted: true,
        consecutive_ok: 0,
        last_check_ms: 0,
        last_switch_ms: 0,
        notes: Vec::new(),
        logged: 0,
        clock_ms: None,
    });
    &STATE
}

fn lock() -> std::sync::MutexGuard<'static, State> {
    state().lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) fn now_ms() -> u64 {
    if let Some(ms) = lock().clock_ms {
        return ms;
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn set_clock_ms(ms: Option<u64>) {
    lock().clock_ms = ms;
}

pub(crate) fn reset() {
    let mut s = lock();
    s.source = Source::Fallback;
    s.faulted = true;
    s.consecutive_ok = 0;
    s.last_check_ms = 0;
    s.last_switch_ms = 0;
    s.notes.clear();
    s.logged = 0;
    s.clock_ms = None;
}

/// Test setup: already on hc, no switch event yet.
pub(crate) fn assume_up() {
    let mut s = lock();
    s.source = Source::Herdr;
    s.faulted = false;
    s.consecutive_ok = HEALTHY_NEED;
    s.notes.clear();
    s.logged = 0;
}

/// Test: source stays Herdr, but a fault is held so status and peek agree
/// on down. The sample window is closed so a peek poll cannot clear it.
pub(crate) fn fault_held(now: u64) {
    let mut s = lock();
    s.source = Source::Herdr;
    s.faulted = true;
    s.consecutive_ok = 0;
    s.last_check_ms = now;
    s.last_switch_ms = now;
}

pub(crate) fn source() -> Source {
    lock().source
}

pub(crate) fn is_down() -> bool {
    let s = lock();
    s.source != Source::Herdr || s.faulted
}

pub(crate) fn events() -> Vec<&'static str> {
    lock().notes.iter().map(|n| n.source).collect()
}

pub(crate) fn take_unlogged() -> Vec<HealthEvent> {
    let mut s = lock();
    let out = s.notes[s.logged..]
        .iter()
        .map(|n| HealthEvent {
            source: n.source,
            kind: n.kind,
        })
        .collect();
    s.logged = s.notes.len();
    out
}

/// Map an hc error string onto socket | timeout | transport | status.
pub(crate) fn kind_of(err: &str) -> &'static str {
    let head = err.trim().split([':', ' ']).next().unwrap_or("");
    match head {
        "socket" => "socket",
        "timeout" => "timeout",
        "transport" => "transport",
        "status" => "status",
        _ => {
            let low = err.to_ascii_lowercase();
            if low.contains("timeout") {
                "timeout"
            } else if low.contains("socket") || low.contains("connection refused") {
                "socket"
            } else if low.contains("status") {
                "status"
            } else {
                "transport"
            }
        }
    }
}

fn switch_to(s: &mut State, next: Source, now: u64) -> bool {
    if s.source == next {
        return false;
    }
    if s.last_switch_ms != 0 && now.saturating_sub(s.last_switch_ms) < RECHECK_MS {
        return false;
    }
    s.source = next;
    s.last_switch_ms = now;
    s.notes.push(Note {
        source: next.as_str(),
        kind: "",
    });
    if next == Source::Fallback {
        s.consecutive_ok = 0;
        s.faulted = true;
    } else {
        s.faulted = false;
    }
    true
}

/// Immediate DOWN (stream ended, binary gone, read failed). Still ≤1 switch / 30s.
/// `kind` is socket | timeout | transport | status, stored on the herdr-health note.
pub(crate) fn force_fallback(now: u64, kind: &'static str) {
    let mut s = lock();
    s.consecutive_ok = 0;
    s.faulted = true;
    let switched = switch_to(&mut s, Source::Fallback, now);
    if switched {
        if let Some(n) = s.notes.last_mut() {
            n.kind = kind;
        }
        return;
    }
    let stale = match s.notes.last() {
        None => true,
        Some(n) => n.kind != kind,
    };
    if !kind.is_empty() && stale {
        s.notes.push(Note {
            source: Source::Fallback.as_str(),
            kind,
        });
    }
}

pub(crate) fn observe(ok: bool, now: u64) {
    let mut s = lock();
    if s.last_check_ms != 0 && now.saturating_sub(s.last_check_ms) < RECHECK_MS {
        return;
    }
    // Cold start has never sampled. One healthy probe is enough. After a
    // sample exists, recovery still needs two in a row.
    let cold = s.last_check_ms == 0;
    s.last_check_ms = now;
    if !ok {
        s.consecutive_ok = 0;
        s.faulted = true;
        switch_to(&mut s, Source::Fallback, now);
        return;
    }
    s.consecutive_ok = s.consecutive_ok.saturating_add(1);
    let need = if cold { 1 } else { HEALTHY_NEED };
    if s.consecutive_ok >= need {
        s.faulted = false;
        switch_to(&mut s, Source::Herdr, now);
    }
}

/// One health sample against hc. Missing binary, bad status, or a dead
/// events stream is not ok.
pub(crate) fn poll() {
    let now = now_ms();
    {
        let s = lock();
        if s.last_check_ms != 0 && now.saturating_sub(s.last_check_ms) < RECHECK_MS {
            return;
        }
    }
    let ok = probe_ok();
    observe(ok, now);
}

fn probe_ok() -> bool {
    let Some(bin) = core::locate() else {
        return false;
    };
    if core::server_status(&bin).is_err() {
        return false;
    }
    core::events_open(&bin)
}

/// Two healthy samples, 30s apart, so a test can start from Herdr.
pub(crate) fn prove_up() {
    reset();
    set_clock_ms(Some(1_000));
    poll();
    set_clock_ms(Some(1_000 + RECHECK_MS));
    poll();
}
