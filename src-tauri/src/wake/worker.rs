//! Tail events by seq. Herdr is primary when the flag is on and hc is healthy.
//! The relay log is the fallback and the durable record. Append never waits
//! on this loop: `notify` is `try_send` on a 1-slot channel.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::{json, Value};

use super::alarm;
use super::herdr_bus::{self, BusPull};
use super::route::{self, Clock, Decision, EventView, Perm};
use super::store;
use super::SeatId;

pub static TEST_LOCK: Mutex<()> = Mutex::new(());
static STOP: AtomicBool = AtomicBool::new(false);
static PARKED: AtomicBool = AtomicBool::new(false);
static STALL: AtomicBool = AtomicBool::new(false);
static NOTIFY: Mutex<Option<SyncSender<()>>> = Mutex::new(None);
static JOIN: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

fn app_slot() -> &'static Mutex<Option<tauri::AppHandle>> {
    static APP: OnceLock<Mutex<Option<tauri::AppHandle>>> = OnceLock::new();
    APP.get_or_init(|| Mutex::new(None))
}

pub fn bind_app(app: tauri::AppHandle) {
    *app_slot().lock().unwrap_or_else(|e| e.into_inner()) = Some(app);
}

pub fn clear_app() {
    *app_slot().lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Non-blocking. Flag off, or no worker, is a no-op.
pub fn notify() {
    let guard = NOTIFY.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(tx) = guard.as_ref() {
        let _ = tx.try_send(());
    }
}

pub fn stall_for_test(on: bool) {
    STALL.store(on, Ordering::SeqCst);
}

pub fn wait_parked(timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if PARKED.load(Ordering::SeqCst) {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    false
}

pub fn ensure_started(dir: PathBuf) {
    let mut join = JOIN.lock().unwrap_or_else(|e| e.into_inner());
    if join.as_ref().is_some_and(|h| !h.is_finished()) {
        return;
    }
    STOP.store(false, Ordering::SeqCst);
    let (tx, rx) = mpsc::sync_channel(1);
    *NOTIFY.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    *join = Some(thread::spawn(move || loop_dir(dir, rx)));
}

pub fn stop() {
    STOP.store(true, Ordering::SeqCst);
    STALL.store(false, Ordering::SeqCst);
    *NOTIFY.lock().unwrap_or_else(|e| e.into_inner()) = None;
    if let Some(handle) = JOIN.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = handle.join();
    }
    PARKED.store(false, Ordering::SeqCst);
}

fn loop_dir(dir: PathBuf, rx: Receiver<()>) {
    if STALL.load(Ordering::SeqCst) {
        PARKED.store(true, Ordering::SeqCst);
        while STALL.load(Ordering::SeqCst) && !STOP.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(5));
        }
        PARKED.store(false, Ordering::SeqCst);
    }
    while !STOP.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {
                let _ = drain(&dir, &Clock::now());
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

pub struct DrainOut {
    pub lines: usize,
    pub bus: String,
}

pub fn drain(dir: &Path, clock: &Clock) -> Result<DrainOut, String> {
    if !super::wake_on(dir) {
        return Ok(DrainOut {
            lines: 0,
            bus: "off".into(),
        });
    }
    let primary = super::herdr_primary(dir);
    let pull = if primary {
        herdr_bus::pull(Duration::from_millis(250))
    } else {
        BusPull::RelayOnly
    };
    apply(dir, clock, pull, primary)
}

fn apply(dir: &Path, clock: &Clock, pull: BusPull, primary: bool) -> Result<DrainOut, String> {
    let mut state = store::load_state(dir);
    let bus = sync_bus(dir, &mut state, &pull, primary, clock)?;
    let subs = store::load_subs(dir);
    let perm = Perm {
        caller: route::Caller::System,
    };
    let mut n = 0u32;
    let mut cursor = store::load_cursor(dir);

    if let Some(hc) = pull.hc_lines() {
        for raw in hc {
            let Some(mapped) = crate::herdr::events::map_value(&raw) else {
                continue;
            };
            let Some(mut ev) = EventView::from_hc(&mapped) else {
                continue;
            };
            route::stamp_board_title(dir, &mut ev);
            n += apply_event(dir, &ev, &subs, &perm, clock, &mut state, &bus)?;
        }
    }

    if pull.use_relay() {
        let pulled = route_relay(dir, cursor, &subs, &perm, clock, &mut state, &bus)?;
        n += pulled.n;
        cursor = pulled.cursor;
        // Cursor moves only after the batch is routed.
        store::save_cursor(dir, cursor)?;
        if pulled.legacy_off > 0 || !pulled.legacy_ids.is_empty() {
            store::save_legacy(dir, pulled.legacy_off, &pulled.legacy_ids)?;
        }
    }

    for d in route::roll_noise(&subs, clock, &mut state) {
        commit_decision(dir, &d, &bus)?;
        n += 1;
    }
    store::save_state(dir, &state)?;
    let fired = alarm::fire_due(dir, clock)?;
    n += fired.len() as u32;
    emit_fired(dir);
    Ok(DrainOut {
        lines: n as usize,
        bus,
    })
}

struct RelayPull {
    n: u32,
    cursor: u64,
    legacy_off: u64,
    legacy_ids: Vec<String>,
}

const LEGACY_ID_CAP: usize = 4_000;
const SEQLESS_CAP: usize = 500;

fn id_seen(ids: &[String], id: &str) -> bool {
    !id.is_empty() && ids.iter().any(|s| s == id)
}

fn remember_id(ids: &mut Vec<String>, id: &str) {
    if id.is_empty() || ids.iter().any(|s| s == id) {
        return;
    }
    ids.push(id.to_string());
    if ids.len() > LEGACY_ID_CAP {
        let drop_n = ids.len() - LEGACY_ID_CAP;
        ids.drain(0..drop_n);
    }
}

/// Real-seq lines still come from `since`. Seq-less lines (Stable writes)
/// are walked in file order, stamped with `log_tip`, and remembered by id
/// under wake/legacy.json. events.jsonl is only read.
fn route_relay(
    dir: &Path,
    mut cursor: u64,
    subs: &[route::Sub],
    perm: &Perm,
    clock: &Clock,
    state: &mut route::RouteState,
    bus: &str,
) -> Result<RelayPull, String> {
    let path = store::events_path(dir);
    let (batch, _) =
        crate::relay::event_seq::since(&path, cursor, 500).map_err(|e| format!("since: {e}"))?;
    let allow: BTreeSet<u64> = batch
        .iter()
        .filter_map(|v| v.get("seq").and_then(Value::as_u64))
        .collect();
    let (mark, mut ids) = store::load_legacy(dir);
    let raw = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RelayPull {
                n: 0,
                cursor,
                legacy_off: mark,
                legacy_ids: ids,
            });
        }
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let tip = store::log_tip(dir);
    let cap_off = if allow.is_empty() {
        u64::MAX
    } else {
        last_allow_end(&raw, &allow).unwrap_or(u64::MAX)
    };
    let mut n = 0u32;
    let mut off = mark;
    let mut seqless_n = 0usize;
    let mut i = 0usize;
    while i < raw.len() {
        let start = i;
        let Some(rel) = raw[i..].iter().position(|b| *b == b'\n') else {
            break;
        };
        let end = i + rel;
        let next = end + 1;
        i = next;
        if cap_off != u64::MAX && (next as u64) > cap_off {
            break;
        }
        let line = &raw[start..end];
        if line.is_empty() {
            off = next as u64;
            continue;
        }
        let Ok(text) = std::str::from_utf8(line) else {
            off = next as u64;
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            off = next as u64;
            continue;
        };
        if let Some(seq) = v.get("seq").and_then(Value::as_u64) {
            if allow.contains(&seq) {
                if let Some(mut ev) = EventView::from_log(&v) {
                    route::stamp_board_title(dir, &mut ev);
                    if !id_seen(&ids, &ev.id) {
                        n += apply_event(dir, &ev, subs, perm, clock, state, bus)?;
                    }
                    cursor = cursor.max(ev.seq);
                }
            }
            off = next as u64;
            continue;
        }
        if (next as u64) <= mark {
            off = next as u64;
            continue;
        }
        if seqless_n >= SEQLESS_CAP {
            break;
        }
        if let Some(mut ev) = EventView::from_seqless(&v, tip, start as u64) {
            route::stamp_board_title(dir, &mut ev);
            if !id_seen(&ids, &ev.id) {
                n += apply_event(dir, &ev, subs, perm, clock, state, bus)?;
                remember_id(&mut ids, &ev.id);
            }
            seqless_n += 1;
        }
        off = next as u64;
    }
    Ok(RelayPull {
        n,
        cursor,
        legacy_off: off,
        legacy_ids: ids,
    })
}

fn last_allow_end(raw: &[u8], allow: &BTreeSet<u64>) -> Option<u64> {
    let mut i = 0usize;
    let mut found = None;
    while i < raw.len() {
        let start = i;
        let Some(rel) = raw[i..].iter().position(|b| *b == b'\n') else {
            break;
        };
        let end = i + rel;
        let next = end + 1;
        i = next;
        let line = &raw[start..end];
        if line.is_empty() {
            continue;
        }
        let Ok(text) = std::str::from_utf8(line) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            continue;
        };
        if let Some(seq) = v.get("seq").and_then(Value::as_u64) {
            if allow.contains(&seq) {
                found = Some(next as u64);
            }
        }
    }
    found
}

fn apply_event(
    dir: &Path,
    ev: &EventView,
    subs: &[route::Sub],
    perm: &Perm,
    clock: &Clock,
    state: &mut route::RouteState,
    bus: &str,
) -> Result<u32, String> {
    let mut n = 0;
    for d in route::route(ev, subs, perm, clock, state) {
        commit_decision(dir, &d, bus)?;
        n += 1;
    }
    Ok(n)
}

fn commit_decision(dir: &Path, d: &Decision, bus: &str) -> Result<(), String> {
    let line = stamped(dir, d);
    if inbox_has(&d.seat, dir, &line) {
        return Ok(());
    }
    store::append_inbox(dir, &d.seat, &line)?;
    let body = json!({
        "seat": d.seat.as_key(),
        "summary": line.get("summary").and_then(Value::as_str).unwrap_or(""),
        "event_id": line.get("event_id").and_then(Value::as_str).unwrap_or(""),
        "source": bus,
    });
    let _ = store::log_summary(dir, d.e_type_log, &body);
    if d.deliver && bus == "herdr" {
        herdr_bus::deliver(
            &d.seat,
            line.get("summary").and_then(Value::as_str).unwrap_or(""),
        );
    }
    emit_one(&line);
    Ok(())
}

/// Fold lines are not event-log rows. Their inbox seq is the log tip,
/// so ack cannot jump past a later board-move.
fn stamped(dir: &Path, d: &Decision) -> Value {
    if d.e_type_log != "wake-drop" {
        return d.line.clone();
    }
    let mut line = d.line.clone();
    let seq = store::log_tip(dir);
    let sub = line
        .get("sub")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    line["seq"] = json!(seq);
    line["event_id"] = json!(format!("fold-{sub}-{seq}"));
    line["dedupe_key"] = json!(format!("fold:{sub}:{seq}"));
    line
}

fn inbox_has(seat: &SeatId, dir: &Path, line: &Value) -> bool {
    let key = line.get("dedupe_key").and_then(Value::as_str).unwrap_or("");
    if key.is_empty() {
        return false;
    }
    store::read_inbox(dir, seat)
        .iter()
        .any(|row| row.get("dedupe_key").and_then(Value::as_str) == Some(key))
}

fn sync_bus(
    dir: &Path,
    state: &mut route::RouteState,
    pull: &BusPull,
    primary: bool,
    clock: &Clock,
) -> Result<String, String> {
    let next = if !primary {
        "relay".to_string()
    } else {
        match pull {
            BusPull::RelayOnly => "relay".into(),
            BusPull::Ended(_) | BusPull::Missing => {
                state.healthy_checks = 0;
                "fallback".into()
            }
            BusPull::Open(_) => healthy_bus(state, clock),
        }
    };
    if state.bus != next && primary {
        let kind = match pull {
            BusPull::Missing => "missing",
            BusPull::Ended(_) => "stream",
            BusPull::Open(_) => "healthy",
            BusPull::RelayOnly => "relay",
        };
        let _ = store::log_summary(
            dir,
            "wake",
            &json!({ "source": next, "kind": kind, "switch": true }),
        );
    }
    state.bus = next.clone();
    Ok(next)
}

/// Cold start accepts one healthy sample. After a fallback, two samples
/// at least 30s apart are required before the bus returns to herdr.
fn healthy_bus(state: &mut route::RouteState, clock: &Clock) -> String {
    if state.bus != "fallback" {
        state.healthy_checks = 2;
        state.last_healthy_unix = clock.unix;
        return "herdr".into();
    }
    if state.last_healthy_unix == 0
        || clock.unix.saturating_sub(state.last_healthy_unix) >= 30
    {
        state.healthy_checks = state.healthy_checks.saturating_add(1);
        state.last_healthy_unix = clock.unix;
    }
    if state.healthy_checks >= 2 {
        "herdr".into()
    } else {
        "fallback".into()
    }
}

fn emit_one(line: &Value) {
    let guard = app_slot().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(app) = guard.as_ref() {
        use tauri::Emitter;
        let _ = app.emit("wake:fired", line);
    }
}

fn emit_fired(_dir: &Path) {}
