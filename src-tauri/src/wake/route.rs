//! Pure match, dedupe, noise cap, and caller permission. No files, no spawn.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

use super::store;
use super::SeatId;

pub const WAKE_ETYPES: &[&str] = &["wake", "wake-drop", "wake-alarm"];
const SEEN_CAP: usize = 4_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Caller {
    User,
    Companion,
    Agent {
        card: String,
        planet: Option<String>,
    },
    /// Worker path. Not a socket caller.
    System,
}

impl Caller {
    pub fn parse(raw: &str) -> Self {
        let s = raw.trim();
        if s.is_empty() || s == "socket" || s == "user" || s.starts_with("ctl:") {
            return Caller::User;
        }
        if s == "companion" || s.starts_with("companion:") {
            return Caller::Companion;
        }
        if let Some(rest) = s.strip_prefix("agent:") {
            let (card, planet) = match rest.split_once("@") {
                Some((c, p)) if !p.is_empty() => (c.to_string(), Some(p.to_string())),
                _ => (rest.to_string(), None),
            };
            return Caller::Agent { card, planet };
        }
        Caller::Agent {
            card: String::new(),
            planet: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Perm {
    pub caller: Caller,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Trigger {
    pub source: String,
    #[serde(default)]
    pub e_types: Vec<String>,
    #[serde(default)]
    pub kinds: Vec<u64>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub card: Option<String>,
    #[serde(default)]
    pub key_prefix: Option<String>,
}

impl Trigger {
    pub fn events(e_types: Vec<String>) -> Self {
        Self {
            source: "events".into(),
            e_types,
            kinds: Vec::new(),
            project: None,
            card: None,
            key_prefix: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sub {
    pub v: u8,
    pub id: String,
    pub seat: String,
    pub trigger: Trigger,
    pub dedupe_s: u32,
    pub max_per_hour: u32,
    pub created_by: String,
    pub created_at: String,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct EventView {
    pub seq: u64,
    pub id: String,
    pub kind: u64,
    pub e_type: String,
    pub project: String,
    pub card: String,
    pub key: String,
    pub summary: String,
    /// `events` or `emit`.
    pub source: String,
}

impl EventView {
    pub fn from_log(v: &Value) -> Option<Self> {
        let seq = v.get("seq").and_then(Value::as_u64)?;
        Some(Self::from_fields(v, seq, "events"))
    }

    pub fn from_hc(v: &Value) -> Option<Self> {
        let seq = v.get("seq").and_then(Value::as_u64).unwrap_or(0);
        if seq == 0 && v.get("id").and_then(Value::as_str).unwrap_or("").is_empty() {
            return None;
        }
        Some(Self::from_fields(v, seq, "events"))
    }

    fn from_fields(v: &Value, seq: u64, source: &str) -> Self {
        let body = if v.get("tags").is_some() {
            store::json_body(v)
        } else {
            v.clone()
        };
        let mut e_type = store::tag(v, "e-type");
        if e_type.is_empty() {
            e_type = v
                .get("e_type")
                .or_else(|| v.get("e-type"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
        }
        let kind = v.get("kind").and_then(Value::as_u64).unwrap_or(0);
        if e_type.is_empty() && matches!(kind, 420 | 421 | 422) {
            e_type = "board-move".into();
        }
        let project = first_nonempty(&[
            store::tag(v, "project"),
            str_field(v, "project"),
            str_field(&body, "project"),
        ]);
        let card = first_nonempty(&[
            store::tag(v, "d"),
            str_field(v, "card"),
            str_field(&body, "card"),
            str_field(&body, "cardId"),
        ]);
        let key = first_nonempty(&[str_field(v, "key"), str_field(&body, "key")]);
        let summary = first_nonempty(&[
            str_field(v, "summary"),
            str_field(&body, "summary"),
            str_field(&body, "title"),
            e_type.clone(),
        ]);
        let id = first_nonempty(&[
            str_field(v, "id"),
            store::tag(v, "trace"),
            format!("s{seq}"),
        ]);
        Self {
            seq,
            id,
            kind,
            e_type,
            project,
            card,
            key,
            summary: clip(&summary, 200),
            source: source.into(),
        }
    }
}

fn str_field(v: &Value, name: &str) -> String {
    v.get(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn first_nonempty(parts: &[String]) -> String {
    parts
        .iter()
        .find(|s| !s.is_empty())
        .cloned()
        .unwrap_or_default()
}

pub fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct HourCount {
    pub delivered: u32,
    pub pending: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogicalSeen {
    pub key: String,
    pub unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RouteState {
    pub hour: i64,
    pub counts: BTreeMap<String, HourCount>,
    pub seen: Vec<String>,
    pub logical: Vec<LogicalSeen>,
    pub bus: String,
    #[serde(default)]
    pub fired_alarms: Vec<String>,
    #[serde(default)]
    pub healthy_checks: u32,
    #[serde(default)]
    pub last_healthy_unix: u64,
}

impl Default for RouteState {
    fn default() -> Self {
        Self {
            hour: 0,
            counts: BTreeMap::new(),
            seen: Vec::new(),
            logical: Vec::new(),
            bus: String::new(),
            fired_alarms: Vec::new(),
            healthy_checks: 0,
            last_healthy_unix: 0,
        }
    }
}

impl RouteState {
    pub fn remember(&mut self, key: &str) {
        if self.seen.iter().any(|k| k == key) {
            return;
        }
        self.seen.push(key.to_string());
        if self.seen.len() > SEEN_CAP {
            let drop_n = self.seen.len() - SEEN_CAP;
            self.seen.drain(0..drop_n);
        }
    }

    pub fn seen_has(&self, key: &str) -> bool {
        self.seen.iter().any(|k| k == key)
    }
}

#[derive(Clone, Debug)]
pub struct Clock {
    pub unix: u64,
}

impl Clock {
    pub fn now() -> Self {
        Self {
            unix: crate::event_log::unix_now(),
        }
    }

    pub fn hour(&self) -> i64 {
        (self.unix / 3600) as i64
    }

    pub fn iso(&self) -> String {
        let utc = chrono::TimeZone::timestamp_opt(&chrono::Utc, self.unix as i64, 0)
            .single()
            .unwrap_or_else(chrono::Utc::now);
        crate::companion::mouth_archive::ct_stamp(utc).1
    }
}

#[derive(Clone, Debug)]
pub struct Decision {
    pub seat: SeatId,
    pub line: Value,
    pub e_type_log: &'static str,
    pub deliver: bool,
}

pub fn permit(caller: &Caller, seat: &SeatId, trigger: &Trigger) -> Result<(), String> {
    match caller {
        Caller::System | Caller::User => Ok(()),
        Caller::Companion => {
            if !matches!(seat, SeatId::Primary) {
                return Err("companion may only use seat primary".into());
            }
            Ok(())
        }
        Caller::Agent { card, planet } => {
            if card.is_empty() {
                return Err("agent seat refused".into());
            }
            match seat {
                SeatId::Agent(id) if id == card => {}
                _ => return Err("agent may only subscribe its own seat".into()),
            }
            if let Some(c) = trigger.card.as_deref() {
                if c != card {
                    return Err("agent trigger card is not its own".into());
                }
            }
            if let Some(p) = trigger.project.as_deref() {
                match planet {
                    Some(own) if p == own => {}
                    _ => return Err("agent trigger project is not its own".into()),
                }
            }
            let card_scoped = trigger.card.as_deref() == Some(card.as_str());
            let planet_scoped = trigger
                .project
                .as_deref()
                .zip(planet.as_deref())
                .is_some_and(|(p, own)| p == own);
            if !card_scoped && !planet_scoped {
                return Err("agent trigger must be scoped to its own card or planet".into());
            }
            Ok(())
        }
    }
}

pub fn is_wake_etype(e_type: &str) -> bool {
    WAKE_ETYPES.contains(&e_type)
}

fn count_key(seat: &str, sub: &str) -> String {
    format!("{seat}|{sub}")
}

fn dedupe_key(sub: &str, ev: &EventView) -> String {
    format!("{sub}:{}:{}", ev.seq, ev.id)
}

fn logical_key(sub: &str, ev: &EventView) -> String {
    format!(
        "{sub}|{}|{}|{}|{}",
        ev.e_type, ev.project, ev.card, ev.key
    )
}

pub fn matches(ev: &EventView, sub: &Sub) -> bool {
    if is_wake_etype(&ev.e_type) {
        return false;
    }
    let want = if sub.trigger.source.is_empty() {
        "events"
    } else {
        sub.trigger.source.as_str()
    };
    if want != ev.source {
        return false;
    }
    if !sub.trigger.e_types.is_empty() && !e_type_hit(ev, &sub.trigger.e_types) {
        return false;
    }
    if !sub.trigger.kinds.is_empty() && !sub.trigger.kinds.contains(&ev.kind) {
        return false;
    }
    if let Some(p) = sub.trigger.project.as_deref() {
        if ev.project != p {
            return false;
        }
    }
    if let Some(c) = sub.trigger.card.as_deref() {
        if ev.card != c {
            return false;
        }
    }
    if let Some(prefix) = sub.trigger.key_prefix.as_deref() {
        if prefix.is_empty() || !ev.key.starts_with(prefix) {
            return false;
        }
    }
    true
}

fn e_type_hit(ev: &EventView, want: &[String]) -> bool {
    want.iter().any(|t| t == &ev.e_type)
}

fn expired(sub: &Sub, clock: &Clock) -> bool {
    let Some(at) = sub.expires_at.as_deref() else {
        return false;
    };
    match chrono::DateTime::parse_from_rfc3339(at) {
        Ok(dt) => dt.timestamp() as u64 <= clock.unix,
        Err(_) => false,
    }
}

pub fn roll_noise(subs: &[Sub], clock: &Clock, state: &mut RouteState) -> Vec<Decision> {
    let hour = clock.hour();
    if state.hour == 0 {
        state.hour = hour;
        return Vec::new();
    }
    if state.hour == hour {
        return Vec::new();
    }
    let mut out = Vec::new();
    let keys: Vec<String> = state.counts.keys().cloned().collect();
    for key in keys {
        let Some(count) = state.counts.get_mut(&key) else {
            continue;
        };
        let pending = count.pending;
        count.delivered = 0;
        count.pending = 0;
        if pending == 0 {
            continue;
        }
        let Some((seat_s, sub_id)) = key.split_once('|') else {
            continue;
        };
        let Ok(seat) = SeatId::parse(seat_s) else {
            continue;
        };
        let sub = subs.iter().find(|s| s.id == sub_id);
        let summary = format!("{pending} more");
        out.push(fold_line(seat, sub_id, &summary, clock, sub.map(|s| s.id.as_str())));
    }
    state.hour = hour;
    out
}

fn fold_line(seat: SeatId, sub: &str, summary: &str, clock: &Clock, _id: Option<&str>) -> Decision {
    // Placeholder. Persist stamps the events.jsonl tip. Never wall-clock.
    let seq = 0u64;
    Decision {
        seat,
        deliver: false,
        e_type_log: "wake-drop",
        line: json!({
            "seq": seq,
            "ts": clock.iso(),
            "sub": sub,
            "event_id": format!("fold-{sub}-{seq}"),
            "e_type": "wake-drop",
            "project": "",
            "card": "",
            "summary": clip(summary, 200),
            "dedupe_key": format!("fold:{sub}:{seq}"),
        }),
    }
}

pub fn route(
    ev: &EventView,
    subs: &[Sub],
    perm: &Perm,
    clock: &Clock,
    state: &mut RouteState,
) -> Vec<Decision> {
    let mut out = roll_noise(subs, clock, state);
    if is_wake_etype(&ev.e_type) {
        return out;
    }
    for sub in subs {
        if expired(sub, clock) {
            continue;
        }
        let Ok(seat) = SeatId::parse(&sub.seat) else {
            continue;
        };
        if !matches!(perm.caller, Caller::System) && permit(&perm.caller, &seat, &sub.trigger).is_err()
        {
            continue;
        }
        if !matches(ev, sub) {
            continue;
        }
        let dkey = dedupe_key(&sub.id, ev);
        if state.seen_has(&dkey) {
            continue;
        }
        let lkey = logical_key(&sub.id, ev);
        if sub.dedupe_s > 0 {
            let window = sub.dedupe_s as u64;
            let dup = state.logical.iter().any(|s| {
                s.key == lkey && clock.unix.saturating_sub(s.unix) < window
            });
            if dup {
                state.remember(&dkey);
                continue;
            }
        }
        let ck = count_key(&sub.seat, &sub.id);
        let cap = sub.max_per_hour;
        let count = state.counts.entry(ck).or_default();
        if cap > 0 && count.delivered >= cap {
            count.pending = count.pending.saturating_add(1);
            state.remember(&dkey);
            continue;
        }
        count.delivered = count.delivered.saturating_add(1);
        state.remember(&dkey);
        state.logical.push(LogicalSeen {
            key: lkey,
            unix: clock.unix,
        });
        if state.logical.len() > SEEN_CAP {
            let drop_n = state.logical.len() - SEEN_CAP;
            state.logical.drain(0..drop_n);
        }
        out.push(Decision {
            seat,
            deliver: true,
            e_type_log: "wake",
            line: json!({
                "seq": ev.seq,
                "ts": clock.iso(),
                "sub": sub.id,
                "event_id": ev.id,
                "e_type": ev.e_type,
                "project": ev.project,
                "card": ev.card,
                "summary": clip(&ev.summary, 200),
                "dedupe_key": dkey,
            }),
        });
    }
    out
}
