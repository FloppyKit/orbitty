//! One alarm per seat. At its time it writes one inbox line and nothing else.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::DateTime;
use serde_json::json;

use super::route::{clip, Clock};
use super::store;
use super::SeatId;

pub fn parse_at(raw: &str, now: u64) -> Result<String, String> {
    let s = raw.trim();
    if let Some(rest) = s.strip_prefix('+') {
        let (n, unit) = split_offset(rest)?;
        let secs = match unit {
            "s" => n,
            "m" => n.saturating_mul(60),
            "h" => n.saturating_mul(3600),
            _ => return Err(format!("bad alarm offset {raw:?}")),
        };
        return Ok(iso(now.saturating_add(secs)));
    }
    let dt = DateTime::parse_from_rfc3339(s).map_err(|_| format!("bad alarm time {raw:?}"))?;
    Ok(dt.to_rfc3339())
}

fn split_offset(s: &str) -> Result<(u64, &str), String> {
    let unit = s
        .chars()
        .last()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| &s[s.len() - c.len_utf8()..])
        .ok_or_else(|| format!("bad alarm offset +{s}"))?;
    let n: u64 = s[..s.len() - unit.len()]
        .parse()
        .map_err(|_| format!("bad alarm offset +{s}"))?;
    Ok((n, unit))
}

pub fn iso(unix: u64) -> String {
    Clock { unix }.iso()
}

pub fn unix_of(at: &str) -> Option<u64> {
    DateTime::parse_from_rfc3339(at)
        .ok()
        .map(|dt| dt.timestamp().max(0) as u64)
}

pub fn set(app_dir: &Path, seat: &SeatId, at: &str) -> Result<(), String> {
    let mut alarms = store::load_alarms(app_dir);
    alarms.insert(seat.as_key(), at.to_string());
    store::save_alarms(app_dir, &alarms)
}

pub fn get(app_dir: &Path, seat: &SeatId) -> Option<String> {
    store::load_alarms(app_dir).get(&seat.as_key()).cloned()
}

pub fn earliest(app_dir: &Path, seat: &SeatId) -> Option<String> {
    get(app_dir, seat)
}

/// Due alarms become one inbox line each, then the alarm is gone.
/// A repeated call does not write a second line.
pub fn fire_due(app_dir: &Path, clock: &Clock) -> Result<Vec<(SeatId, serde_json::Value)>, String> {
    let alarms = store::load_alarms(app_dir);
    if alarms.is_empty() {
        return Ok(Vec::new());
    }
    let mut keep = BTreeMap::new();
    let mut fired = Vec::new();
    let mut state = store::load_state(app_dir);
    for (key, at) in alarms {
        let Ok(seat) = SeatId::parse(&key) else {
            continue;
        };
        let due = unix_of(&at).unwrap_or(u64::MAX) <= clock.unix;
        let mark = format!("{key}|{at}");
        if !due {
            keep.insert(key, at);
            continue;
        }
        if state.fired_alarms.iter().any(|m| m == &mark) {
            continue;
        }
        state.fired_alarms.push(mark);
        let seq = clock.unix.max(1);
        let line = json!({
            "seq": seq,
            "ts": clock.iso(),
            "sub": "",
            "event_id": format!("alarm-{key}-{at}"),
            "e_type": "wake-alarm",
            "project": "",
            "card": "",
            "summary": clip(&format!("alarm {at}"), 200),
            "dedupe_key": format!("alarm:{key}:{at}"),
        });
        store::append_inbox(app_dir, &seat, &line)?;
        let _ = store::log_summary(
            app_dir,
            "wake-alarm",
            &json!({ "seat": key, "at": at, "source": "relay" }),
        );
        fired.push((seat, line));
    }
    if state.fired_alarms.len() > 200 {
        let drop_n = state.fired_alarms.len() - 200;
        state.fired_alarms.drain(0..drop_n);
    }
    store::save_state(app_dir, &state)?;
    store::save_alarms(app_dir, &keep)?;
    Ok(fired)
}
