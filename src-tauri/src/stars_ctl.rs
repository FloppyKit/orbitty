//! Stars tools for the control socket.
//!
//! Check-ins, journal pages, and planet trackers are the `stars` slice and
//! `projects[].trackers` in state.json — the same fields the page writes.
//! The caller saves through `board::save_with_events`, which emits
//! `state:changed` so the page reloads. The cloud bag is derived here with
//! the same rules as the Cloud cover. It is not stored.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::state::{
    KanbanCard, LedgerPage, MetricFlags, PersistedState, Project, StarsCheckin, Tracker,
};

/// Read-class Stars tools. Writes stay off this list so `read` and
/// `companion` profiles deny them.
pub(crate) fn is_read(tool: &str) -> bool {
    matches!(
        tool,
        "stars.checkin.get"
            | "stars.log.get"
            | "stars.journal.list"
            | "stars.journal.read"
            | "stars.reflect.templates"
            | "stars.tracker.list"
            | "stars.timer.status"
            | "stars.cloud.words"
    )
}

pub(crate) struct Clock {
    pub today: String,
    pub now_secs: f64,
    pub now_ms: f64,
}

impl Clock {
    pub(crate) fn now() -> Self {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        Self {
            today: chrono::Local::now().format("%Y-%m-%d").to_string(),
            now_secs: ms / 1000.0,
            now_ms: ms,
        }
    }
}

pub(crate) fn slices_changed(before: &PersistedState, after: &PersistedState) -> bool {
    serde_json::to_string(&before.stars).ok() != serde_json::to_string(&after.stars).ok()
        || serde_json::to_string(&before.projects).ok()
            != serde_json::to_string(&after.projects).ok()
}

pub(crate) fn handle(
    st: &mut PersistedState,
    tool: &str,
    args: &Value,
    clock: &Clock,
) -> Result<Value, String> {
    match tool {
        "stars.checkin.get" => Ok(checkin_view(&date_arg(args, clock, false)?, st)),
        "stars.checkin.set" => checkin_set(st, args, clock),
        "stars.log.get" => Ok(log_view(&date_arg(args, clock, false)?, st)),
        "stars.log.set" => log_set(st, args, clock),
        "stars.log.save" => log_save(st, args, clock),
        "stars.journal.list" => journal_list(st, args),
        "stars.journal.read" => journal_read(st, args),
        "stars.journal.append" => journal_append(st, args, clock),
        "stars.reflect.templates" => Ok(reflect_templates()),
        "stars.reflect.write" => reflect_write(st, args, clock),
        "stars.tracker.list" => tracker_list(st, args, clock),
        "stars.tracker.inc" => tracker_bump(st, args, clock, 1),
        "stars.tracker.dec" => tracker_bump(st, args, clock, -1),
        "stars.tracker.set" => tracker_set(st, args, clock),
        "stars.timer.start" => timer_start(st, args, clock),
        "stars.timer.stop" => timer_stop(st, args, clock),
        "stars.timer.status" => Ok(timer_status(st, clock)),
        "stars.cloud.words" => cloud_words(st, args, clock),
        other => Err(format!("unknown tool {other:?}")),
    }
}

const AXES: [&str; 5] = ["sleep", "terroir", "athleticism", "relationships", "self"];

fn parse_day(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    let d = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
        .map_err(|_| format!("date must be YYYY-MM-DD, got {raw:?}"))?;
    Ok(d.format("%Y-%m-%d").to_string())
}

fn date_arg(args: &Value, clock: &Clock, required: bool) -> Result<String, String> {
    match args.get("date") {
        None | Some(Value::Null) if !required => Ok(clock.today.clone()),
        None | Some(Value::Null) => Err("missing arg \"date\"".into()),
        Some(Value::String(s)) if s.trim().is_empty() && !required => Ok(clock.today.clone()),
        Some(Value::String(s)) if s.trim().is_empty() => Err("missing arg \"date\"".into()),
        Some(Value::String(s)) => parse_day(s),
        Some(_) => Err("date must be YYYY-MM-DD".into()),
    }
}

fn opt_day(args: &Value, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(parse_day(s)?)),
        Some(_) => Err(format!("{key} must be YYYY-MM-DD")),
    }
}

fn opt_str(args: &Value, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                Ok(None)
            } else {
                Ok(Some(t.to_string()))
            }
        }
        Some(_) => Err(format!("{key} must be a string")),
    }
}

fn req_str(args: &Value, key: &str) -> Result<String, String> {
    opt_str(args, key)?.ok_or_else(|| format!("missing arg {key:?}"))
}

/// Absent = leave the field. Null or blank = clear it.
fn log_field(args: &Value, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(String::new())),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("{key} must be a string")),
    }
}

fn json_i32(v: &Value, key: &str) -> Result<i32, String> {
    let n = if let Some(i) = v.as_i64() {
        i
    } else if let Some(u) = v.as_u64() {
        i64::try_from(u).map_err(|_| format!("{key} is out of range"))?
    } else if let Some(f) = v.as_f64() {
        if !f.is_finite() || f.fract() != 0.0 {
            return Err(format!("{key} must be an integer"));
        }
        f as i64
    } else {
        return Err(format!("{key} must be an integer"));
    };
    i32::try_from(n).map_err(|_| format!("{key} is out of range"))
}

fn checkin_of<'a>(st: &'a PersistedState, date: &str) -> Option<&'a StarsCheckin> {
    st.stars.checkins.get(date)
}

fn checkin_view(date: &str, st: &PersistedState) -> Value {
    let saved = checkin_of(st, date).is_some();
    let c = checkin_of(st, date).cloned().unwrap_or_default();
    json!({
        "date": date,
        "saved": saved,
        "sleep": c.sleep,
        "terroir": c.terroir,
        "athleticism": c.athleticism,
        "relationships": c.relationships,
        "self": c.self_,
        "win1": c.win1,
        "win2": c.win2,
        "grateful": c.grateful,
    })
}

fn log_view(date: &str, st: &PersistedState) -> Value {
    let saved = checkin_of(st, date).is_some();
    let c = checkin_of(st, date).cloned().unwrap_or_default();
    json!({
        "date": date,
        "saved": saved,
        "win1": c.win1,
        "win2": c.win2,
        "grateful": c.grateful,
    })
}

fn axis_set(c: &mut StarsCheckin, axis: &str, value: u8) -> Result<(), String> {
    match axis {
        "sleep" => c.sleep = value,
        "terroir" => c.terroir = value,
        "athleticism" => c.athleticism = value,
        "relationships" => c.relationships = value,
        "self" => c.self_ = value,
        other => {
            return Err(format!("unknown axis {other:?} — {}", AXES.join(" | ")));
        }
    }
    Ok(())
}

fn checkin_set(st: &mut PersistedState, args: &Value, clock: &Clock) -> Result<Value, String> {
    let date = date_arg(args, clock, false)?;
    let axis = req_str(args, "axis")?.to_ascii_lowercase();
    if !AXES.contains(&axis.as_str()) {
        return Err(format!("unknown axis {axis:?} — {}", AXES.join(" | ")));
    }
    let value = args
        .get("value")
        .ok_or_else(|| "missing arg \"value\"".to_string())?;
    let n = json_i32(value, "value")?;
    if !(1..=5).contains(&n) {
        return Err("value must be an integer 1-5".into());
    }
    let c = st.stars.checkins.entry(date.clone()).or_default();
    axis_set(c, &axis, n as u8)?;
    Ok(checkin_view(&date, st))
}

fn store_win(slot: &mut Option<String>, raw: &str) {
    let t = raw.trim();
    if t.is_empty() {
        *slot = None;
    } else {
        *slot = Some(t.to_string());
    }
}

fn log_set(st: &mut PersistedState, args: &Value, clock: &Clock) -> Result<Value, String> {
    let date = date_arg(args, clock, false)?;
    let win1 = log_field(args, "win1")?;
    let win2 = log_field(args, "win2")?;
    let grateful = log_field(args, "grateful")?;
    if win1.is_none() && win2.is_none() && grateful.is_none() {
        return Err("need win1, win2, or grateful".into());
    }
    let c = st.stars.checkins.entry(date.clone()).or_default();
    if let Some(v) = &win1 {
        store_win(&mut c.win1, v);
    }
    if let Some(v) = &win2 {
        store_win(&mut c.win2, v);
    }
    if let Some(v) = &grateful {
        store_win(&mut c.grateful, v);
    }
    Ok(log_view(&date, st))
}

fn clamp_spoke(n: u8) -> u8 {
    if n == 0 {
        0
    } else {
        n.clamp(1, 5)
    }
}

fn log_save(st: &mut PersistedState, args: &Value, clock: &Clock) -> Result<Value, String> {
    let date = date_arg(args, clock, false)?;
    if let Some(c) = st.stars.checkins.get_mut(&date) {
        c.sleep = clamp_spoke(c.sleep);
        c.terroir = clamp_spoke(c.terroir);
        c.athleticism = clamp_spoke(c.athleticism);
        c.relationships = clamp_spoke(c.relationships);
        c.self_ = clamp_spoke(c.self_);
        if let Some(s) = c.win1.clone() {
            store_win(&mut c.win1, &s);
        }
        if let Some(s) = c.win2.clone() {
            store_win(&mut c.win2, &s);
        }
        if let Some(s) = c.grateful.clone() {
            store_win(&mut c.grateful, &s);
        }
    }
    Ok(checkin_view(&date, st))
}

fn page_json(p: &LedgerPage) -> Value {
    json!({
        "id": p.id,
        "createdAt": p.created_at,
        "dayKey": p.day_key,
        "scope": p.scope,
        "body": p.body,
        "metricFlags": p.metric_flags.as_ref().map(|f| json!({
            "spokes": f.spokes,
            "wins": f.wins,
            "grateful": f.grateful,
            "trackers": f.trackers,
            "cloud": f.cloud,
        })),
    })
}

fn scope_is_overall(scope: Option<&str>) -> bool {
    match scope.map(str::trim) {
        None | Some("") | Some("overall") => true,
        Some(_) => false,
    }
}

enum PlanetSel {
    All,
    Overall,
    Id(String),
}

fn planet_sel(st: &PersistedState, raw: Option<&str>, who: &str) -> Result<PlanetSel, String> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(PlanetSel::All);
    };
    if raw.eq_ignore_ascii_case("overall") || raw.eq_ignore_ascii_case("all") {
        return Ok(PlanetSel::Overall);
    }
    Ok(PlanetSel::Id(crate::board::resolve_project(st, raw, who)?))
}

fn page_matches(
    p: &LedgerPage,
    from: &Option<String>,
    to: &Option<String>,
    sel: &PlanetSel,
) -> bool {
    if from.is_some() || to.is_some() {
        let Some(day) = p.day_key.as_deref() else {
            return false;
        };
        if let Some(from) = from {
            if day < from.as_str() {
                return false;
            }
        }
        if let Some(to) = to {
            if day > to.as_str() {
                return false;
            }
        }
    }
    match sel {
        PlanetSel::All => true,
        PlanetSel::Overall => scope_is_overall(p.scope.as_deref()),
        PlanetSel::Id(id) => p.scope.as_deref() == Some(id.as_str()),
    }
}

fn journal_list(st: &PersistedState, args: &Value) -> Result<Value, String> {
    let from = opt_day(args, "from")?;
    let to = opt_day(args, "to")?;
    if let (Some(a), Some(b)) = (&from, &to) {
        if a > b {
            return Err("from is after to".into());
        }
    }
    let sel = planet_sel(
        st,
        opt_str(args, "planet")?.as_deref(),
        "stars.journal.list",
    )?;
    let mut pages: Vec<&LedgerPage> = st
        .stars
        .ledger_pages
        .iter()
        .filter(|p| page_matches(p, &from, &to, &sel))
        .collect();
    pages.sort_by(|a, b| {
        a.created_at
            .partial_cmp(&b.created_at)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    let planet = match &sel {
        PlanetSel::All => Value::Null,
        PlanetSel::Overall => json!("overall"),
        PlanetSel::Id(id) => json!(id),
    };
    Ok(json!({
        "from": from,
        "to": to,
        "planet": planet,
        "pages": pages.iter().map(|p| page_json(p)).collect::<Vec<_>>(),
    }))
}

fn journal_read(st: &PersistedState, args: &Value) -> Result<Value, String> {
    let date = date_arg(
        args,
        &Clock {
            today: String::new(),
            now_secs: 0.0,
            now_ms: 0.0,
        },
        true,
    )?;
    let pages: Vec<Value> = st
        .stars
        .ledger_pages
        .iter()
        .filter(|p| p.day_key.as_deref() == Some(date.as_str()))
        .map(page_json)
        .collect();
    Ok(json!({ "date": date, "pages": pages }))
}

fn journal_append(st: &mut PersistedState, args: &Value, clock: &Clock) -> Result<Value, String> {
    let date = date_arg(args, clock, false)?;
    let text = match args.get("text") {
        Some(Value::String(s)) => s.clone(),
        None | Some(Value::Null) => return Err("missing arg \"text\"".into()),
        Some(_) => return Err("text must be a string".into()),
    };
    if text.trim().is_empty() {
        return Err("blank body is not saved".into());
    }
    let sel = planet_sel(
        st,
        opt_str(args, "planet")?.as_deref(),
        "stars.journal.append",
    )?;
    let scope = match sel {
        PlanetSel::All | PlanetSel::Overall => "overall".to_string(),
        PlanetSel::Id(id) => id,
    };
    let page = LedgerPage {
        id: crate::board::new_id("led"),
        created_at: clock.now_ms,
        day_key: Some(date),
        day_key_end: None,
        scope: Some(scope),
        body: Some(text.trim().to_string()),
        metric_flags: None,
    };
    let out = page_json(&page);
    st.stars.ledger_pages.push(page);
    Ok(out)
}

fn reflect_templates() -> Value {
    json!({
        "templates": [{
            "id": "ledger",
            "name": "Reflect",
            "sections": ["spokes", "wins", "grateful", "trackers", "cloud"],
            "note": "Local template. Write fills the page from that day's saved check-in, wins, current trackers, and a fixed cloud line. No model call. A blank body is not saved."
        }]
    })
}

fn answers_obj(args: &Value) -> Result<Value, String> {
    match args.get("answers") {
        None | Some(Value::Null) => Ok(json!({})),
        Some(v) if v.is_object() => Ok(v.clone()),
        Some(_) => Err("answers must be an object".into()),
    }
}

fn flag(answers: &Value, key: &str) -> Result<bool, String> {
    match answers.get(key) {
        None | Some(Value::Null) => Ok(true),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(format!("{key} must be a boolean")),
    }
}

struct SpokeLine {
    label: &'static str,
    value: u8,
}

fn ledger_body(
    day: &str,
    scope_label: &str,
    flags: &MetricFlags,
    spokes: &[SpokeLine],
    wins: &[String],
    grateful: &str,
    trackers: &[(String, String, String)],
) -> String {
    let on = flags.spokes || flags.wins || flags.grateful || flags.trackers || flags.cloud;
    if !on {
        return String::new();
    }
    let mut lines = vec![format!("{day} · {scope_label}")];
    if flags.spokes {
        lines.push(String::new());
        lines.push("Spokes".into());
        let set: Vec<_> = spokes.iter().filter(|s| s.value >= 1).collect();
        if set.is_empty() {
            lines.push("None set today.".into());
        } else {
            for s in set {
                lines.push(format!("{} {}", s.label, s.value));
            }
        }
    }
    if flags.wins {
        lines.push(String::new());
        lines.push("Wins".into());
        let wins: Vec<_> = wins
            .iter()
            .map(|w| w.trim())
            .filter(|w| !w.is_empty())
            .collect();
        if wins.is_empty() {
            lines.push("None written.".into());
        } else {
            for w in wins {
                lines.push(w.to_string());
            }
        }
    }
    if flags.grateful {
        lines.push(String::new());
        lines.push("Grateful".into());
        let g = grateful.trim();
        lines.push(if g.is_empty() {
            "Nothing noted.".into()
        } else {
            g.to_string()
        });
    }
    if flags.trackers {
        lines.push(String::new());
        lines.push("Trackers".into());
        let rows: Vec<String> = trackers
            .iter()
            .filter_map(|(planet, label, value)| {
                let label = label.trim();
                if label.is_empty() {
                    return None;
                }
                let planet = planet.trim();
                Some(if planet.is_empty() {
                    format!("{label} {value}")
                } else {
                    format!("{planet} · {label} {value}")
                })
            })
            .collect();
        if rows.is_empty() {
            lines.push("None yet.".into());
        } else {
            lines.extend(rows);
        }
    }
    if flags.cloud {
        lines.push(String::new());
        lines.push("Cloud".into());
        lines.push("The sky stays on the cover of this page.".into());
    }
    lines.join("\n").trim().to_string()
}

fn project_name(st: &PersistedState, id: &str) -> String {
    st.projects
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "All".into())
}

fn tracker_rows(
    st: &PersistedState,
    sel: &PlanetSel,
    now_ms: f64,
) -> Vec<(String, String, String)> {
    let mut rows = Vec::new();
    for p in &st.projects {
        let keep = match sel {
            PlanetSel::All | PlanetSel::Overall => true,
            PlanetSel::Id(id) => p.id == *id,
        };
        if !keep {
            continue;
        }
        for t in p.trackers.as_deref().unwrap_or(&[]) {
            let value = if is_timer(t) {
                format_elapsed(elapsed_sec(t, now_ms))
            } else {
                t.value.to_string()
            };
            rows.push((p.name.clone(), t.label.clone(), value));
        }
    }
    rows
}

fn reflect_write(st: &mut PersistedState, args: &Value, clock: &Clock) -> Result<Value, String> {
    let date = date_arg(args, clock, false)?;
    let template = req_str(args, "template")?;
    if template != "ledger" {
        return Err(format!("unknown template {template:?} — ledger"));
    }
    let answers = answers_obj(args)?;
    let flags = MetricFlags {
        spokes: flag(&answers, "spokes")?,
        wins: flag(&answers, "wins")?,
        grateful: flag(&answers, "grateful")?,
        trackers: flag(&answers, "trackers")?,
        cloud: flag(&answers, "cloud")?,
    };
    if !flags.spokes && !flags.wins && !flags.grateful && !flags.trackers && !flags.cloud {
        return Err("pick at least one section".into());
    }
    let planet_raw = opt_str(args, "planet")?.or(opt_str(&answers, "planet")?);
    let sel = planet_sel(st, planet_raw.as_deref(), "stars.reflect.write")?;
    let (scope, scope_label) = match &sel {
        PlanetSel::All | PlanetSel::Overall => ("overall".to_string(), "All".to_string()),
        PlanetSel::Id(id) => (id.clone(), project_name(st, id)),
    };
    let c = checkin_of(st, &date).cloned().unwrap_or_default();
    let spokes = [
        SpokeLine {
            label: "Sleep",
            value: c.sleep,
        },
        SpokeLine {
            label: "Terroir",
            value: c.terroir,
        },
        SpokeLine {
            label: "Athleticism",
            value: c.athleticism,
        },
        SpokeLine {
            label: "Relationships",
            value: c.relationships,
        },
        SpokeLine {
            label: "Self",
            value: c.self_,
        },
    ];
    let wins = [
        c.win1.clone().unwrap_or_default(),
        c.win2.clone().unwrap_or_default(),
    ];
    let grateful = c.grateful.clone().unwrap_or_default();
    let rows = tracker_rows(st, &sel, clock.now_ms);
    let body = ledger_body(
        &date,
        &scope_label,
        &flags,
        &spokes,
        &wins,
        &grateful,
        &rows,
    );
    if body.trim().is_empty() {
        return Err("blank body is not saved".into());
    }
    let page = LedgerPage {
        id: crate::board::new_id("led"),
        created_at: clock.now_ms,
        day_key: Some(date),
        day_key_end: None,
        scope: Some(scope),
        body: Some(body),
        metric_flags: Some(flags),
    };
    let out = json!({
        "template": "ledger",
        "page": page_json(&page),
    });
    st.stars.ledger_pages.push(page);
    Ok(out)
}

fn is_timer(t: &Tracker) -> bool {
    t.kind.as_deref() == Some("timer")
}

fn elapsed_sec(t: &Tracker, now_ms: f64) -> f64 {
    let base = t.value as f64;
    if !is_timer(t) {
        return base;
    }
    let Some(since) = t.running_since else {
        return base;
    };
    base + ((now_ms - since) / 1000.0).max(0.0)
}

fn format_elapsed(sec: f64) -> String {
    let s = sec.max(0.0).floor() as u64;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let r = s % 60;
    if h > 0 {
        format!("{h}:{m:02}:{r:02}")
    } else {
        format!("{m}:{r:02}")
    }
}

fn find_tracker(st: &PersistedState, id: &str) -> Result<(usize, usize), String> {
    let mut hits = Vec::new();
    for (pi, p) in st.projects.iter().enumerate() {
        if let Some(ts) = &p.trackers {
            for (ti, t) in ts.iter().enumerate() {
                if t.id == id {
                    hits.push((pi, ti));
                }
            }
        }
    }
    match hits.len() {
        0 => Err(format!("unknown tracker id {id:?}")),
        1 => Ok(hits[0]),
        n => Err(format!("ambiguous tracker id {id:?} — {n} matches")),
    }
}

fn tracker_at(st: &PersistedState, pi: usize, ti: usize) -> &Tracker {
    &st.projects[pi].trackers.as_ref().unwrap()[ti]
}

fn tracker_json(st: &PersistedState, pi: usize, ti: usize, now_ms: f64) -> Value {
    let p = &st.projects[pi];
    let t = tracker_at(st, pi, ti);
    let timer = is_timer(t);
    json!({
        "id": t.id,
        "label": t.label,
        "planetId": p.id,
        "planet": p.name,
        "kind": if timer { "timer" } else { "number" },
        "value": t.value,
        "running": timer && t.running_since.is_some(),
        "runningSince": t.running_since,
        "elapsedSec": if timer {
            Value::from(elapsed_sec(t, now_ms).round() as i64)
        } else {
            Value::Null
        },
    })
}

fn tracker_list(st: &PersistedState, args: &Value, clock: &Clock) -> Result<Value, String> {
    let sel = planet_sel(
        st,
        opt_str(args, "planet")?.as_deref(),
        "stars.tracker.list",
    )?;
    let planet = match &sel {
        PlanetSel::All => Value::Null,
        PlanetSel::Overall => json!("overall"),
        PlanetSel::Id(id) => json!(id),
    };
    let mut trackers = Vec::new();
    for (pi, p) in st.projects.iter().enumerate() {
        let keep = match &sel {
            PlanetSel::All | PlanetSel::Overall => true,
            PlanetSel::Id(id) => p.id == *id,
        };
        if !keep {
            continue;
        }
        let n = p.trackers.as_ref().map(|t| t.len()).unwrap_or(0);
        for ti in 0..n {
            trackers.push(tracker_json(st, pi, ti, clock.now_ms));
        }
    }
    Ok(json!({ "planet": planet, "trackers": trackers }))
}

fn by_steps(args: &Value, sign: i32) -> Result<i32, String> {
    let n = match args.get("by") {
        None | Some(Value::Null) => 1,
        Some(v) => {
            let n = json_i32(v, "by")?;
            if n <= 0 {
                return Err("by must be a positive integer".into());
            }
            n
        }
    };
    Ok(n.saturating_mul(sign))
}

fn tracker_bump(
    st: &mut PersistedState,
    args: &Value,
    clock: &Clock,
    sign: i32,
) -> Result<Value, String> {
    let id = req_str(args, "id")?;
    let (pi, ti) = find_tracker(st, &id)?;
    if is_timer(tracker_at(st, pi, ti)) {
        return Err("timer — use stars.timer.start / stars.timer.stop".into());
    }
    let delta = by_steps(args, sign)?;
    let t = &mut st.projects[pi].trackers.as_mut().unwrap()[ti];
    if sign > 0 {
        t.value = t.value.saturating_add(delta);
    } else {
        t.value = t.value.saturating_sub(delta.abs());
    }
    Ok(tracker_json(st, pi, ti, clock.now_ms))
}

fn tracker_set(st: &mut PersistedState, args: &Value, clock: &Clock) -> Result<Value, String> {
    let id = req_str(args, "id")?;
    let value = args
        .get("value")
        .ok_or_else(|| "missing arg \"value\"".to_string())?;
    let n = json_i32(value, "value")?;
    let (pi, ti) = find_tracker(st, &id)?;
    if is_timer(tracker_at(st, pi, ti)) {
        return Err("timer — use stars.timer.start / stars.timer.stop".into());
    }
    st.projects[pi].trackers.as_mut().unwrap()[ti].value = n;
    Ok(tracker_json(st, pi, ti, clock.now_ms))
}

fn timer_start(st: &mut PersistedState, args: &Value, clock: &Clock) -> Result<Value, String> {
    let id = req_str(args, "id")?;
    let (pi, ti) = find_tracker(st, &id)?;
    let t = &mut st.projects[pi].trackers.as_mut().unwrap()[ti];
    t.kind = Some("timer".into());
    if t.running_since.is_none() {
        t.running_since = Some(clock.now_ms);
    }
    Ok(tracker_json(st, pi, ti, clock.now_ms))
}

fn timer_stop(st: &mut PersistedState, args: &Value, clock: &Clock) -> Result<Value, String> {
    let id = req_str(args, "id")?;
    let (pi, ti) = find_tracker(st, &id)?;
    if !is_timer(tracker_at(st, pi, ti)) {
        return Err("not a timer".into());
    }
    let t = &mut st.projects[pi].trackers.as_mut().unwrap()[ti];
    t.value = elapsed_sec(t, clock.now_ms).round() as i32;
    t.running_since = None;
    t.kind = Some("timer".into());
    Ok(tracker_json(st, pi, ti, clock.now_ms))
}

fn timer_status(st: &PersistedState, clock: &Clock) -> Value {
    let mut timers = Vec::new();
    for (pi, p) in st.projects.iter().enumerate() {
        let n = p.trackers.as_ref().map(|t| t.len()).unwrap_or(0);
        for ti in 0..n {
            if is_timer(tracker_at(st, pi, ti)) {
                timers.push(tracker_json(st, pi, ti, clock.now_ms));
            }
        }
    }
    json!({ "timers": timers })
}

// Same 163-word set as SkyField.svelte. Sorted for binary search.
const STOPWORDS: &[&str] = &[
    "about",
    "after",
    "again",
    "ain",
    "all",
    "also",
    "and",
    "another",
    "any",
    "are",
    "aren",
    "back",
    "because",
    "been",
    "before",
    "being",
    "bit",
    "both",
    "but",
    "can",
    "cant",
    "could",
    "couldn",
    "didn",
    "doesn",
    "don",
    "during",
    "each",
    "either",
    "else",
    "etc",
    "even",
    "every",
    "feel",
    "few",
    "for",
    "from",
    "get",
    "going",
    "gonna",
    "good",
    "got",
    "had",
    "hadn",
    "has",
    "hasn",
    "have",
    "haven",
    "her",
    "here",
    "hers",
    "him",
    "his",
    "how",
    "into",
    "isn",
    "its",
    "just",
    "know",
    "let",
    "lets",
    "like",
    "lot",
    "lots",
    "made",
    "make",
    "many",
    "maybe",
    "might",
    "mine",
    "more",
    "most",
    "much",
    "must",
    "need",
    "needs",
    "neither",
    "new",
    "nor",
    "not",
    "now",
    "off",
    "okay",
    "old",
    "one",
    "only",
    "other",
    "our",
    "ours",
    "out",
    "over",
    "own",
    "per",
    "really",
    "right",
    "said",
    "same",
    "say",
    "says",
    "see",
    "seen",
    "shall",
    "she",
    "should",
    "shouldn",
    "some",
    "still",
    "stuff",
    "take",
    "tell",
    "than",
    "that",
    "the",
    "their",
    "them",
    "then",
    "there",
    "they",
    "thing",
    "things",
    "think",
    "this",
    "though",
    "through",
    "today",
    "told",
    "tomorrow",
    "too",
    "took",
    "two",
    "under",
    "until",
    "very",
    "via",
    "want",
    "wants",
    "was",
    "way",
    "well",
    "were",
    "what",
    "when",
    "where",
    "which",
    "while",
    "who",
    "whom",
    "whose",
    "why",
    "will",
    "with",
    "within",
    "without",
    "won",
    "would",
    "wouldn",
    "yeah",
    "yes",
    "yesterday",
    "yet",
    "you",
    "your",
    "yours",
];

fn is_stop(w: &str) -> bool {
    STOPWORDS.binary_search(&w).is_ok()
}

fn tokenize(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in lower.chars() {
        if ch.is_alphabetic() {
            cur.push(ch);
        } else if !cur.is_empty() {
            push_tok(&mut out, &cur);
            cur.clear();
        }
    }
    if !cur.is_empty() {
        push_tok(&mut out, &cur);
    }
    out
}

fn push_tok(out: &mut Vec<String>, tok: &str) {
    if tok.chars().count() >= 3 && !is_stop(tok) {
        out.push(tok.to_string());
    }
}

struct RangeWin {
    label: &'static str,
    secs: Option<f64>,
}

fn range_win(args: &Value) -> Result<RangeWin, String> {
    let raw = opt_str(args, "range")?;
    let key = raw.unwrap_or_default().to_ascii_lowercase();
    let (label, secs) = match key.as_str() {
        "" | "all" => ("all", None),
        "week" | "7" | "7d" => ("week", Some(7.0 * 86400.0)),
        "14" | "14d" => ("14", Some(14.0 * 86400.0)),
        "month" | "30" | "30d" => ("month", Some(30.0 * 86400.0)),
        "year" | "365" | "365d" => ("year", Some(365.0 * 86400.0)),
        other => {
            return Err(format!(
                "unknown range {other:?} — 14 | week | month | year | all"
            ));
        }
    };
    Ok(RangeWin { label, secs })
}

struct WordInfo {
    word: String,
    count: f64,
    last_at: f64,
}

fn cloud_words(st: &PersistedState, args: &Value, clock: &Clock) -> Result<Value, String> {
    let range = range_win(args)?;
    let tag_by_id: HashMap<&str, &str> = st
        .tags
        .iter()
        .map(|t| (t.id.as_str(), t.name.as_str()))
        .collect();
    let proj_by_id: HashMap<&str, &str> = st
        .projects
        .iter()
        .map(|p| (p.id.as_str(), p.name.as_str()))
        .collect();

    struct Doc {
        entries: Vec<(String, f64)>,
        at: f64,
    }
    let mut docs: Vec<Doc> = Vec::new();

    for c in &st.kanban_cards {
        if c.status != "done" {
            continue;
        }
        let mut entries = Vec::new();
        let push_all = |entries: &mut Vec<(String, f64)>, text: &str, weight: f64| {
            for t in tokenize(text) {
                entries.push((t, weight));
            }
        };
        push_all(&mut entries, &c.title, 1.0);
        if let Some(notes) = &c.notes {
            push_all(&mut entries, notes, 1.0);
        }
        if let Some(ids) = &c.tag_ids {
            for tid in ids {
                if let Some(name) = tag_by_id.get(tid.as_str()) {
                    push_all(&mut entries, name, 1.0);
                }
            }
        }
        if let Some(name) = proj_by_id.get(c.project_id.as_str()) {
            push_all(&mut entries, name, 0.5);
        }
        if entries.is_empty() {
            continue;
        }
        let at = c.completed_at.unwrap_or(c.last_touched_at);
        docs.push(Doc { entries, at });
    }

    let mut user_lines: Vec<String> = st
        .companion
        .turns
        .iter()
        .filter(|t| t.role == "you")
        .map(|t| t.text.clone())
        .collect();
    let last_user = st.companion.last_user_line.trim();
    let last_turn = user_lines.last().map(|s| s.trim()).unwrap_or("");
    if !last_user.is_empty() && last_turn != last_user {
        user_lines.push(last_user.to_string());
    }
    for line in user_lines {
        let toks = tokenize(&line);
        if toks.is_empty() {
            continue;
        }
        docs.push(Doc {
            entries: toks.into_iter().map(|t| (t, 1.0)).collect(),
            at: clock.now_secs,
        });
    }

    if let Some(window) = range.secs {
        docs.retain(|d| clock.now_secs - d.at <= window);
    }

    let mut by_word: HashMap<String, WordInfo> = HashMap::new();
    for doc in &docs {
        let mut present: HashMap<String, f64> = HashMap::new();
        for (tok, weight) in &doc.entries {
            present
                .entry(tok.clone())
                .and_modify(|w| *w = w.max(*weight))
                .or_insert(*weight);
        }
        for (tok, w) in present {
            match by_word.get_mut(&tok) {
                Some(cur) => {
                    cur.count += w;
                    cur.last_at = cur.last_at.max(doc.at);
                }
                None => {
                    by_word.insert(
                        tok.clone(),
                        WordInfo {
                            word: tok,
                            count: w,
                            last_at: doc.at,
                        },
                    );
                }
            }
        }
    }

    let muted: Vec<&str> = st.sky.muted.iter().map(String::as_str).collect();
    let muted_set: std::collections::HashSet<&str> = muted.iter().copied().collect();
    let mut words: Vec<WordInfo> = by_word
        .into_values()
        .filter(|w| !muted_set.contains(w.word.as_str()))
        .collect();
    words.sort_by(|a, b| {
        b.count
            .partial_cmp(&a.count)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.word.cmp(&b.word))
    });
    let total = words.len();
    words.truncate(80);
    let fresh_window = 14.0 * 86400.0;
    let out: Vec<Value> = words
        .iter()
        .map(|w| {
            json!({
                "word": w.word,
                "count": w.count,
                "lastAt": w.last_at,
                "fresh": clock.now_secs - w.last_at <= fresh_window,
            })
        })
        .collect();
    Ok(json!({
        "range": range.label,
        "total": total,
        "words": out,
        "muted": st.sky.muted,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Tag, Tracker};

    fn clock() -> Clock {
        Clock {
            today: "2099-01-02".into(),
            now_secs: 1_800_000_000.0,
            now_ms: 1_800_000_000_000.0,
        }
    }

    fn card(
        title: &str,
        status: &str,
        project: &str,
        completed: Option<f64>,
        notes: Option<&str>,
    ) -> KanbanCard {
        KanbanCard {
            id: format!("c-{title}"),
            title: title.into(),
            project_id: project.into(),
            window_id: None,
            kind: Some("todo".into()),
            status: status.into(),
            created_at: 0.0,
            last_touched_at: completed.unwrap_or(0.0),
            completed_at: completed,
            goal_aligned: false,
            clinical_or_master: false,
            points_awarded: 0,
            starred: false,
            notes: notes.map(str::to_string),
            difficulty: None,
            subtasks: None,
            due_at: None,
            tag_ids: None,
            needed_state: None,
            needed_why: None,
            needed_answer: None,
            actor: None,
            assigned_agent_id: None,
            parent_id: None,
            rating: None,
            trackers: None,
            task_id: None,
            task_reason: None,
            links: None,
        }
    }

    fn project(id: &str, name: &str, trackers: Vec<Tracker>) -> Project {
        Project {
            id: id.into(),
            name: name.into(),
            trackers: if trackers.is_empty() {
                None
            } else {
                Some(trackers)
            },
            workspace_root: None,
        }
    }

    fn tracker(
        id: &str,
        label: &str,
        value: i32,
        kind: Option<&str>,
        running: Option<f64>,
    ) -> Tracker {
        Tracker {
            id: id.into(),
            label: label.into(),
            value,
            kind: kind.map(str::to_string),
            running_since: running,
        }
    }

    #[test]
    fn checkin_set_keeps_other_axes_and_rejects_bad_values() {
        let mut st = PersistedState::default();
        let c = clock();
        let day = json!({"date": "2099-12-31", "axis": "sleep", "value": 4});
        let v = handle(&mut st, "stars.checkin.set", &day, &c).unwrap();
        assert_eq!(v["sleep"], 4);
        assert_eq!(v["self"], 0);
        assert_eq!(v["saved"], true);
        handle(
            &mut st,
            "stars.checkin.set",
            &json!({"date": "2099-12-31", "axis": "self", "value": 2}),
            &c,
        )
        .unwrap();
        assert_eq!(st.stars.checkins["2099-12-31"].sleep, 4);
        assert_eq!(st.stars.checkins["2099-12-31"].self_, 2);
        assert!(handle(
            &mut st,
            "stars.checkin.set",
            &json!({"date": "2099-12-31", "axis": "sleep", "value": 0}),
            &c,
        )
        .is_err());
        assert!(handle(
            &mut st,
            "stars.checkin.set",
            &json!({"date": "2099-12-31", "axis": "sleep", "value": 6}),
            &c,
        )
        .is_err());
        assert!(handle(
            &mut st,
            "stars.checkin.set",
            &json!({"date": "2026-02-31", "axis": "sleep", "value": 3}),
            &c,
        )
        .is_err());
        let missing = handle(
            &mut st,
            "stars.checkin.get",
            &json!({"date": "2099-01-01"}),
            &c,
        )
        .unwrap();
        assert_eq!(missing["saved"], false);
        assert!(st.stars.checkins.get("2099-01-01").is_none());
    }

    #[test]
    fn log_set_merges_and_save_trims_without_inventing_a_day() {
        let mut st = PersistedState::default();
        let c = clock();
        handle(
            &mut st,
            "stars.log.set",
            &json!({"date": "2099-12-31", "win1": "  shipped  ", "grateful": ""}),
            &c,
        )
        .unwrap();
        handle(
            &mut st,
            "stars.log.set",
            &json!({"date": "2099-12-31", "win2": "second"}),
            &c,
        )
        .unwrap();
        let day = &st.stars.checkins["2099-12-31"];
        assert_eq!(day.win1.as_deref(), Some("shipped"));
        assert_eq!(day.win2.as_deref(), Some("second"));
        assert!(day.grateful.is_none());
        st.stars.checkins.get_mut("2099-12-31").unwrap().win1 = Some("  pad  ".into());
        handle(
            &mut st,
            "stars.log.save",
            &json!({"date": "2099-12-31"}),
            &c,
        )
        .unwrap();
        assert_eq!(st.stars.checkins["2099-12-31"].win1.as_deref(), Some("pad"));
        let before = st.stars.checkins.len();
        let empty = handle(
            &mut st,
            "stars.log.save",
            &json!({"date": "2098-01-01"}),
            &c,
        )
        .unwrap();
        assert_eq!(empty["saved"], false);
        assert_eq!(st.stars.checkins.len(), before);
    }

    #[test]
    fn journal_filters_and_refuses_a_blank_page() {
        let mut st = PersistedState::default();
        st.projects.push(project("p1", "Orbitty", vec![]));
        let c = clock();
        assert!(handle(
            &mut st,
            "stars.journal.append",
            &json!({"date": "2099-12-31", "text": "   "}),
            &c,
        )
        .is_err());
        assert!(st.stars.ledger_pages.is_empty());
        handle(
            &mut st,
            "stars.journal.append",
            &json!({"date": "2099-12-31", "text": "a note", "planet": "orbitty"}),
            &c,
        )
        .unwrap();
        handle(
            &mut st,
            "stars.journal.append",
            &json!({"date": "2099-12-30", "text": "earlier"}),
            &c,
        )
        .unwrap();
        let listed = handle(
            &mut st,
            "stars.journal.list",
            &json!({"from": "2099-12-31", "to": "2099-12-31", "planet": "p1"}),
            &c,
        )
        .unwrap();
        assert_eq!(listed["pages"].as_array().unwrap().len(), 1);
        assert_eq!(listed["pages"][0]["body"], "a note");
        let read = handle(
            &mut st,
            "stars.journal.read",
            &json!({"date": "2099-12-30"}),
            &c,
        )
        .unwrap();
        assert_eq!(read["pages"][0]["scope"], "overall");
        assert!(handle(
            &mut st,
            "stars.journal.list",
            &json!({"from": "2099-12-31", "to": "2099-12-30"}),
            &c,
        )
        .is_err());
    }

    #[test]
    fn reflect_writes_the_ledger_template_and_skips_a_blank_body() {
        let mut st = PersistedState::default();
        st.projects.push(project(
            "p1",
            "Orbitty",
            vec![tracker("tr-water", "water", 3, Some("number"), None)],
        ));
        st.stars.checkins.insert(
            "2099-12-31".into(),
            StarsCheckin {
                sleep: 4,
                self_: 2,
                win1: Some("shipped the tool".into()),
                ..StarsCheckin::default()
            },
        );
        let c = clock();
        let names = handle(&mut st, "stars.reflect.templates", &json!({}), &c).unwrap();
        assert_eq!(names["templates"][0]["id"], "ledger");
        let wrote = handle(
            &mut st,
            "stars.reflect.write",
            &json!({"date": "2099-12-31", "template": "ledger", "answers": {"planet": "p1"}}),
            &c,
        )
        .unwrap();
        assert_eq!(
            wrote["page"]["body"].as_str().unwrap(),
            "2099-12-31 · Orbitty\n\nSpokes\nSleep 4\nSelf 2\n\nWins\nshipped the tool\n\nGrateful\nNothing noted.\n\nTrackers\nOrbitty · water 3\n\nCloud\nThe sky stays on the cover of this page."
        );
        let n = st.stars.ledger_pages.len();
        assert!(handle(
            &mut st,
            "stars.reflect.write",
            &json!({
                "date": "2099-12-31",
                "template": "ledger",
                "answers": {
                    "spokes": false,
                    "wins": false,
                    "grateful": false,
                    "trackers": false,
                    "cloud": false
                }
            }),
            &c,
        )
        .is_err());
        assert_eq!(st.stars.ledger_pages.len(), n);
        assert!(handle(
            &mut st,
            "stars.reflect.write",
            &json!({"template": "nope", "answers": {}}),
            &c,
        )
        .is_err());
    }

    #[test]
    fn trackers_and_timers_follow_the_page() {
        let mut st = PersistedState::default();
        st.projects.push(project(
            "p1",
            "Orbitty",
            vec![
                tracker("tr-water", "water", 1, Some("number"), None),
                tracker("tr-focus", "focus", 10, Some("timer"), None),
            ],
        ));
        st.projects.push(project(
            "p2",
            "Other",
            vec![tracker("tr-water", "dup", 0, None, None)],
        ));
        let c = clock();
        assert!(handle(
            &mut st,
            "stars.tracker.inc",
            &json!({"id": "tr-water", "by": 2}),
            &c
        )
        .is_err());
        st.projects.pop();
        handle(
            &mut st,
            "stars.tracker.inc",
            &json!({"id": "tr-water", "by": 2}),
            &c,
        )
        .unwrap();
        assert_eq!(st.projects[0].trackers.as_ref().unwrap()[0].value, 3);
        handle(&mut st, "stars.tracker.dec", &json!({"id": "tr-water"}), &c).unwrap();
        assert_eq!(st.projects[0].trackers.as_ref().unwrap()[0].value, 2);
        handle(
            &mut st,
            "stars.tracker.set",
            &json!({"id": "tr-water", "value": 9}),
            &c,
        )
        .unwrap();
        assert_eq!(st.projects[0].trackers.as_ref().unwrap()[0].value, 9);
        assert!(handle(&mut st, "stars.tracker.inc", &json!({"id": "tr-focus"}), &c).is_err());
        handle(&mut st, "stars.timer.start", &json!({"id": "tr-focus"}), &c).unwrap();
        let running = &st.projects[0].trackers.as_ref().unwrap()[1];
        assert_eq!(running.running_since, Some(c.now_ms));
        let again = c.now_ms;
        let mut later = clock();
        later.now_ms = again + 5_000.0;
        later.now_secs = later.now_ms / 1000.0;
        handle(
            &mut st,
            "stars.timer.start",
            &json!({"id": "tr-focus"}),
            &later,
        )
        .unwrap();
        assert_eq!(
            st.projects[0].trackers.as_ref().unwrap()[1].running_since,
            Some(again)
        );
        handle(
            &mut st,
            "stars.timer.stop",
            &json!({"id": "tr-focus"}),
            &later,
        )
        .unwrap();
        let stopped = &st.projects[0].trackers.as_ref().unwrap()[1];
        assert!(stopped.running_since.is_none());
        assert_eq!(stopped.value, 15);
        let status = handle(&mut st, "stars.timer.status", &json!({}), &c).unwrap();
        assert_eq!(status["timers"].as_array().unwrap().len(), 1);
        assert!(handle(&mut st, "stars.timer.stop", &json!({"id": "tr-water"}), &c).is_err());
        let listed = handle(
            &mut st,
            "stars.tracker.list",
            &json!({"planet": "orbitty"}),
            &c,
        )
        .unwrap();
        assert_eq!(listed["trackers"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn cloud_matches_the_cover_bag() {
        assert!(tokenize("don't").is_empty());
        assert_eq!(
            tokenize("Orbitty ships the boat"),
            vec!["orbitty", "ships", "boat"]
        );
        assert_eq!(STOPWORDS.len(), 163);
        assert!(STOPWORDS.windows(2).all(|w| w[0] < w[1]));

        let mut st = PersistedState::default();
        st.projects.push(project("p1", "Zephyr", vec![]));
        st.tags.push(Tag {
            id: "t1".into(),
            name: "alpha".into(),
            color: None,
        });
        let mut done = card(
            "Orbitty ships",
            "done",
            "p1",
            Some(1_800_000_000.0),
            Some("the clinic"),
        );
        done.tag_ids = Some(vec!["t1".into()]);
        st.kanban_cards.push(done);
        st.kanban_cards.push(card(
            "ancientword",
            "done",
            "p1",
            Some(1_800_000_000.0 - 100.0 * 86400.0),
            None,
        ));
        st.kanban_cards
            .push(card("notdoneword", "todo", "p1", None, None));
        st.companion.turns.push(crate::state::CompanionTurn {
            role: "you".into(),
            text: "hello orbitty".into(),
        });
        st.companion.turns.push(crate::state::CompanionTurn {
            role: "them".into(),
            text: "zzbotword".into(),
        });
        st.companion.last_user_line = "hello orbitty".into();
        st.sky.muted.push("ships".into());
        let c = clock();
        let week = handle(&mut st, "stars.cloud.words", &json!({"range": "week"}), &c).unwrap();
        let words: Vec<&str> = week["words"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["word"].as_str().unwrap())
            .collect();
        assert!(words.contains(&"orbitty"));
        assert!(words.contains(&"zephyr"));
        assert!(words.contains(&"clinic"));
        assert!(words.contains(&"alpha"));
        assert!(words.contains(&"hello"));
        assert!(!words.contains(&"ships"));
        assert!(!words.contains(&"ancientword"));
        assert!(!words.contains(&"notdoneword"));
        assert!(!words.contains(&"zzbotword"));
        assert!(!words.contains(&"the"));
        let orbitty = week["words"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["word"] == "orbitty")
            .unwrap();
        assert_eq!(orbitty["count"], 2.0);
        let zephyr = week["words"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["word"] == "zephyr")
            .unwrap();
        assert_eq!(zephyr["count"], 0.5);
        assert_eq!(week["muted"][0], "ships");
        let all = handle(&mut st, "stars.cloud.words", &json!({}), &c).unwrap();
        let all_words: Vec<&str> = all["words"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["word"].as_str().unwrap())
            .collect();
        assert!(all_words.contains(&"ancientword"));
        assert_eq!(all["range"], "all");
    }

    #[test]
    fn writes_do_not_drop_the_other_slice() {
        let mut st = PersistedState::default();
        st.projects.push(project(
            "p1",
            "Orbitty",
            vec![tracker("tr-water", "water", 1, None, None)],
        ));
        st.stars.ledger_pages.push(LedgerPage {
            id: "led-keep".into(),
            created_at: 1.0,
            day_key: Some("2099-01-01".into()),
            day_key_end: None,
            scope: Some("overall".into()),
            body: Some("keep".into()),
            metric_flags: None,
        });
        let c = clock();
        handle(
            &mut st,
            "stars.checkin.set",
            &json!({"date": "2099-12-31", "axis": "terroir", "value": 3}),
            &c,
        )
        .unwrap();
        assert_eq!(st.stars.ledger_pages[0].id, "led-keep");
        assert_eq!(st.projects[0].trackers.as_ref().unwrap()[0].value, 1);
        handle(&mut st, "stars.tracker.inc", &json!({"id": "tr-water"}), &c).unwrap();
        assert_eq!(st.stars.checkins["2099-12-31"].terroir, 3);
    }

    #[test]
    fn read_tools_are_the_get_list() {
        assert!(is_read("stars.checkin.get"));
        assert!(is_read("stars.cloud.words"));
        assert!(!is_read("stars.checkin.set"));
        assert!(!is_read("stars.journal.append"));
        assert!(!is_read("stars.timer.start"));
        assert!(!is_read("stars.open"));
    }
}
