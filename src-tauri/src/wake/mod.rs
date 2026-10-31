//! Subscribe and wake. A seat gets one inbox line. Nothing is launched.

mod alarm;
mod herdr_bus;
mod route;
mod store;
mod worker;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tauri::Manager;

pub use route::{Caller, Clock, EventView, Sub, Trigger};
pub use worker::notify;

use route::{permit, Perm};
use store::wake_tree_exists;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeatId {
    Primary,
    Agent(String),
    Claim(String),
    Watch(String),
    Remote(String),
}

impl SeatId {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let s = raw.trim();
        if s.is_empty() || s.contains('/') || s.contains('\\') {
            return Err(format!("refused seat {raw:?}"));
        }
        if s == "primary" {
            return Ok(SeatId::Primary);
        }
        let (kind, rest) = s
            .split_once(':')
            .ok_or_else(|| format!("refused seat {raw:?}"))?;
        if rest.is_empty() {
            return Err(format!("refused seat {raw:?}"));
        }
        match kind {
            "agent" if !rest.contains(':') => Ok(SeatId::Agent(rest.to_string())),
            "claim" => Ok(SeatId::Claim(rest.to_string())),
            "watch" if !rest.contains(':') => Ok(SeatId::Watch(rest.to_string())),
            "remote" if !rest.contains(':') => Ok(SeatId::Remote(rest.to_string())),
            _ => Err(format!("refused seat {raw:?}")),
        }
    }

    pub fn as_key(&self) -> String {
        match self {
            SeatId::Primary => "primary".into(),
            SeatId::Agent(s) => format!("agent:{s}"),
            SeatId::Claim(s) => format!("claim:{s}"),
            SeatId::Watch(s) => format!("watch:{s}"),
            SeatId::Remote(s) => format!("remote:{s}"),
        }
    }
}

pub fn wake_on(dir: &Path) -> bool {
    crate::flags::on_in_dir(dir, "wake_core")
}

pub fn herdr_primary(dir: &Path) -> bool {
    crate::flags::on_in_dir(dir, "herdr_primary")
}

fn refuse_off(tool: &str) -> String {
    crate::flags::refusal(tool, "wake_core")
}

pub fn boot(app: &tauri::AppHandle) {
    if !crate::flags::on(app, "wake_core") {
        worker::stop();
        return;
    }
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    worker::bind_app(app.clone());
    worker::ensure_started(dir);
}

pub fn stop() {
    worker::stop();
    worker::clear_app();
}

pub fn call(
    app: &tauri::AppHandle,
    tool: &str,
    args: &Value,
    caller_raw: &str,
) -> Result<Value, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    let _ = std::fs::create_dir_all(&dir);
    dispatch(&dir, tool, args, caller_raw)
}

pub fn dispatch(dir: &Path, tool: &str, args: &Value, caller_raw: &str) -> Result<Value, String> {
    if !wake_on(dir) {
        return Err(refuse_off(tool));
    }
    let caller = Caller::parse(caller_raw);
    match tool {
        "wake.list" => Ok(json!({ "subs": list_json(dir) })),
        "wake.inbox" => {
            let seat = seat_arg(args)?;
            check_read(&caller, &seat)?;
            let after = args.get("after_seq").and_then(Value::as_u64).unwrap_or(0);
            let n = limit_n(args);
            Ok(json!({ "lines": inbox(dir, &seat, after, n) }))
        }
        "wake.status" => {
            let seat = seat_arg(args)?;
            check_read(&caller, &seat)?;
            Ok(status_json(dir, &seat, &Clock::now()))
        }
        "wake.resume" => {
            let seat = seat_arg(args)?;
            check_read(&caller, &seat)?;
            let n = limit_n(args);
            Ok(json!({ "lines": resume(dir, &seat, n) }))
        }
        "wake.subscribe" => subscribe(dir, args, &caller),
        "wake.unsubscribe" => unsubscribe(dir, args, &caller),
        "wake.ack" => {
            let seat = seat_arg(args)?;
            check_write(&caller, &seat, &Trigger::events(vec![]))?;
            let upto = args
                .get("upto_seq")
                .and_then(Value::as_u64)
                .ok_or_else(|| "wake.ack needs upto_seq".to_string())?;
            ack(dir, &seat, upto)?;
            Ok(json!({ "ok": true, "upto_seq": store::load_ack(dir, &seat) }))
        }
        "wake.alarm" => {
            let seat = seat_arg(args)?;
            check_write(&caller, &seat, &Trigger::events(vec![]))?;
            let at_raw = args
                .get("at")
                .and_then(Value::as_str)
                .ok_or_else(|| "wake.alarm needs at".to_string())?;
            let at = alarm::parse_at(at_raw, crate::event_log::unix_now())?;
            alarm::set(dir, &seat, &at)?;
            Ok(status_json(dir, &seat, &Clock::now()))
        }
        "wake.emit" => emit(dir, args, &caller),
        other => Err(format!("unknown tool {other:?}")),
    }
}

fn seat_arg(args: &Value) -> Result<SeatId, String> {
    let raw = args
        .get("seat")
        .and_then(Value::as_str)
        .ok_or_else(|| "need seat".to_string())?;
    SeatId::parse(raw)
}

fn limit_n(args: &Value) -> usize {
    args.get("n")
        .and_then(Value::as_u64)
        .unwrap_or(50)
        .min(50) as usize
}

fn check_read(caller: &Caller, seat: &SeatId) -> Result<(), String> {
    permit(caller, seat, &Trigger::events(vec!["*".into()]))
        .or_else(|_| permit_read(caller, seat))
}

fn permit_read(caller: &Caller, seat: &SeatId) -> Result<(), String> {
    match caller {
        Caller::User | Caller::System => Ok(()),
        Caller::Companion => {
            if matches!(seat, SeatId::Primary) {
                Ok(())
            } else {
                Err("companion may only use seat primary".into())
            }
        }
        Caller::Agent { card, .. } => match seat {
            SeatId::Agent(id) if id == card && !card.is_empty() => Ok(()),
            _ => Err("agent may only read its own seat".into()),
        },
    }
}

fn check_write(caller: &Caller, seat: &SeatId, trigger: &Trigger) -> Result<(), String> {
    if matches!(caller, Caller::Agent { .. }) && trigger.e_types.is_empty() && trigger.card.is_none()
    {
        return permit_read(caller, seat);
    }
    permit(caller, seat, trigger)
}

fn list_json(dir: &Path) -> Vec<Sub> {
    store::load_subs(dir)
}

pub fn inbox(dir: &Path, seat: &SeatId, after: u64, n: usize) -> Vec<Value> {
    store::read_inbox(dir, seat)
        .into_iter()
        .filter(|l| l.get("seq").and_then(Value::as_u64).unwrap_or(0) > after)
        .take(n.min(50))
        .collect()
}

pub fn resume(dir: &Path, seat: &SeatId, n: usize) -> Vec<Value> {
    let ack = store::load_ack(dir, seat);
    inbox(dir, seat, ack, n.min(50))
}

pub fn ack(dir: &Path, seat: &SeatId, upto: u64) -> Result<(), String> {
    let cur = store::load_ack(dir, seat);
    if upto < cur {
        return Ok(());
    }
    store::save_ack(dir, seat, upto)
}

pub fn status_json(dir: &Path, seat: &SeatId, clock: &Clock) -> Value {
    let lines = store::read_inbox(dir, seat);
    let ack_seq = store::load_ack(dir, seat);
    let unacked = lines
        .iter()
        .filter(|l| l.get("seq").and_then(Value::as_u64).unwrap_or(0) > ack_seq)
        .count() as u32;
    let at = alarm::earliest(dir, seat);
    let due = at
        .as_deref()
        .and_then(alarm::unix_of)
        .is_some_and(|t| t <= clock.unix);
    json!({
        "active": unacked > 0 || due,
        "at": at,
        "unacked": unacked,
    })
}

fn subscribe(dir: &Path, args: &Value, caller: &Caller) -> Result<Value, String> {
    let seat = seat_arg(args)?;
    let trigger = trigger_from(args)?;
    permit(caller, &seat, &trigger)?;
    let mut subs = store::load_subs(dir);
    let id = args
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(crate::event_log::uuid_v4_hex);
    let sub = Sub {
        v: 1,
        id: id.clone(),
        seat: seat.as_key(),
        trigger,
        dedupe_s: args
            .get("dedupe_s")
            .and_then(Value::as_u64)
            .unwrap_or(300) as u32,
        max_per_hour: args
            .get("max_per_hour")
            .and_then(Value::as_u64)
            .unwrap_or(6) as u32,
        created_by: match caller {
            Caller::User => "user".into(),
            Caller::Companion => "companion".into(),
            Caller::Agent { card, .. } => format!("agent:{card}"),
            Caller::System => "system".into(),
        },
        created_at: Clock::now().iso(),
        expires_at: args
            .get("expires_at")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    subs.retain(|s| s.id != id);
    subs.push(sub);
    store::save_subs(dir, &subs)?;
    Ok(json!({ "id": id, "seat": seat.as_key() }))
}

fn trigger_from(args: &Value) -> Result<Trigger, String> {
    let source = args
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("events");
    if source != "events" && source != "emit" {
        return Err(format!("refused source {source:?}"));
    }
    let e_types = string_list(args, "e_types");
    let kinds = args
        .get("kinds")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();
    Ok(Trigger {
        source: source.into(),
        e_types,
        kinds,
        project: args
            .get("project")
            .and_then(Value::as_str)
            .map(str::to_string),
        card: args.get("card").and_then(Value::as_str).map(str::to_string),
        key_prefix: args
            .get("key_prefix")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn string_list(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn unsubscribe(dir: &Path, args: &Value, caller: &Caller) -> Result<Value, String> {
    let id = args
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| "wake.unsubscribe needs id".to_string())?;
    let mut subs = store::load_subs(dir);
    let Some(pos) = subs.iter().position(|s| s.id == id) else {
        return Ok(json!({ "ok": true, "removed": false }));
    };
    let seat = SeatId::parse(&subs[pos].seat)?;
    permit(caller, &seat, &subs[pos].trigger)?;
    subs.remove(pos);
    store::save_subs(dir, &subs)?;
    Ok(json!({ "ok": true, "removed": true }))
}

fn emit(dir: &Path, args: &Value, caller: &Caller) -> Result<Value, String> {
    let source = args
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| "wake.emit needs source".to_string())?;
    let key = args.get("key").and_then(Value::as_str).unwrap_or("");
    let summary = args
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("");
    let seat_filter = args
        .get("seat")
        .and_then(Value::as_str)
        .map(SeatId::parse)
        .transpose()?;
    if let Some(seat) = &seat_filter {
        check_write(caller, seat, &Trigger::events(vec![]))?;
    } else if matches!(caller, Caller::Companion) {
        return Err("companion may only use seat primary".into());
    } else if matches!(caller, Caller::Agent { .. }) {
        return Err("agent may only emit to its own seat".into());
    }
    let mut subs = store::load_subs(dir);
    if let Some(seat) = &seat_filter {
        let key = seat.as_key();
        subs.retain(|s| s.seat == key && s.trigger.source == "emit");
    } else {
        subs.retain(|s| s.trigger.source == "emit");
    }
    let seq = store::log_tip(dir);
    let ev = EventView {
        seq,
        id: crate::event_log::uuid_v4_hex(),
        kind: 0,
        e_type: source.to_string(),
        project: String::new(),
        card: seat_filter
            .as_ref()
            .map(|s| s.as_key())
            .unwrap_or_default(),
        key: key.to_string(),
        summary: route::clip(summary, 200),
        source: "emit".into(),
    };
    let mut state = store::load_state(dir);
    let clock = Clock::now();
    let decisions = route::route(
        &ev,
        &subs,
        &Perm {
            caller: Caller::System,
        },
        &clock,
        &mut state,
    );
    store::save_state(dir, &state)?;
    let mut n = 0;
    for d in decisions {
        if d.line.get("e_type").and_then(Value::as_str) == Some("wake-drop") {
            let mut line = d.line.clone();
            line["seq"] = json!(seq);
            let sub = line
                .get("sub")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            line["event_id"] = json!(format!("fold-{sub}-{seq}"));
            line["dedupe_key"] = json!(format!("fold:{sub}:{seq}"));
            store::append_inbox(dir, &d.seat, &line)?;
            n += 1;
            continue;
        }
        let mut line = d.line.clone();
        line["seq"] = json!(seq);
        store::append_inbox(dir, &d.seat, &line)?;
        n += 1;
    }
    let _ = store::log_summary(
        dir,
        "wake",
        &json!({ "source": "relay", "kind": "emit", "key": key }),
    );
    Ok(json!({ "ok": true, "lines": n }))
}

pub fn put_sub(dir: &Path, sub: Sub) -> Result<(), String> {
    let mut subs = store::load_subs(dir);
    subs.retain(|s| s.id != sub.id);
    subs.push(sub);
    store::save_subs(dir, &subs)
}

pub fn write_flag(dir: &Path, id: &str, on: bool) {
    let path = crate::flags::flags_path(dir, crate::flags::Channel::Stable);
    let mut file = crate::flags::FileFlags::empty();
    if path.is_file() {
        if let Ok(raw) = std::fs::read(&path) {
            file = crate::flags::parse_file(&raw).0;
        }
    }
    file.on.insert(id.to_string(), on);
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(&path, crate::flags::serialize_file(&file));
}

pub fn scratch_dir(label: &str) -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir()
        .join("orbitty-wake")
        .join(format!("space.orbitty.mac-{label}-{n}"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn arm(dir: &Path, wake: bool, herdr: bool) {
    write_flag(dir, "wake_core", wake);
    write_flag(dir, "herdr_primary", herdr);
}

pub fn run_drain(dir: &Path, clock: &Clock) -> Result<worker::DrainOut, String> {
    worker::drain(dir, clock)
}

pub fn files_exist(dir: &Path) -> bool {
    wake_tree_exists(dir)
}

pub fn events_of(dir: &Path) -> Vec<Value> {
    store::summary_lines(dir)
}

pub fn switch_events(dir: &Path) -> Vec<Value> {
    events_of(dir)
        .into_iter()
        .filter(|v| store::json_body(v).get("switch") == Some(&Value::Bool(true)))
        .collect()
}
