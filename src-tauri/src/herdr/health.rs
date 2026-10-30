//! Shared herdr rule: DOWN when the hc binary is missing, `hc server
//! status` is not ok, or the events stream ends. Recheck every 30s.
//! Switch back after 2 consecutive healthy checks. At most one switch
//! per 30s. Each switch records one source name: herdr | fallback.
//! While DOWN, callers return unknown — never Died.

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

struct State {
    source: Source,
    /// A failed read/status. Stays set until 2 healthy checks, even when
    /// the source switch itself is waiting out the 30s limit.
    faulted: bool,
    consecutive_ok: u32,
    last_check_ms: u64,
    last_switch_ms: u64,
    events: Vec<&'static str>,
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
        events: Vec::new(),
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
    s.events.clear();
    s.logged = 0;
    s.clock_ms = None;
}

/// Test setup: already on hc, no switch event yet.
pub(crate) fn assume_up() {
    let mut s = lock();
    s.source = Source::Herdr;
    s.faulted = false;
    s.consecutive_ok = HEALTHY_NEED;
    s.events.clear();
    s.logged = 0;
}

pub(crate) fn source() -> Source {
    lock().source
}

pub(crate) fn is_down() -> bool {
    let s = lock();
    s.source != Source::Herdr || s.faulted
}

pub(crate) fn events() -> Vec<&'static str> {
    lock().events.clone()
}

pub(crate) fn take_unlogged() -> Vec<&'static str> {
    let mut s = lock();
    let out = s.events[s.logged..].to_vec();
    s.logged = s.events.len();
    out
}

fn switch_to(s: &mut State, next: Source, now: u64) {
    if s.source == next {
        return;
    }
    if s.last_switch_ms != 0 && now.saturating_sub(s.last_switch_ms) < RECHECK_MS {
        return;
    }
    s.source = next;
    s.last_switch_ms = now;
    s.events.push(next.as_str());
    if next == Source::Fallback {
        s.consecutive_ok = 0;
        s.faulted = true;
    } else {
        s.faulted = false;
    }
}

/// Immediate DOWN (stream ended, binary gone, read failed). Still ≤1 switch / 30s.
pub(crate) fn force_fallback(now: u64) {
    let mut s = lock();
    s.consecutive_ok = 0;
    s.faulted = true;
    switch_to(&mut s, Source::Fallback, now);
}

pub(crate) fn observe(ok: bool, now: u64) {
    let mut s = lock();
    if s.last_check_ms != 0 && now.saturating_sub(s.last_check_ms) < RECHECK_MS {
        return;
    }
    s.last_check_ms = now;
    if !ok {
        s.consecutive_ok = 0;
        switch_to(&mut s, Source::Fallback, now);
        return;
    }
    s.consecutive_ok = s.consecutive_ok.saturating_add(1);
    if s.consecutive_ok >= HEALTHY_NEED {
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
