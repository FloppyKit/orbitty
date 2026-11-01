//! Companion archive. One JSON line per turn, written before the 40-turn
//! window can drop it. Flag `mh_archive` off: today's drain, no files.
//! Tests install a temp root; a test build never writes `~/.agents`.

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(test))]
use std::sync::atomic::AtomicBool;

use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::state::{cap_companion_turns, CompanionState, CompanionTurn};

const PARKED_MODES: &[&str] = &["spark", "chat", "work", "hold"];

/// Session-char budget that replaces the every-16-send roll when the flag is on.
pub const MH_SESSION_BUDGET_CHARS: u64 = 120_000;

#[derive(Clone, Debug)]
pub struct TurnMeta {
    pub lane: String,
    pub model: String,
    pub session: Option<String>,
    pub src: String,
}

impl TurnMeta {
    pub fn unknown() -> Self {
        Self {
            lane: "?".into(),
            model: "?".into(),
            session: None,
            src: "live".into(),
        }
    }
}

/// Grok CLI is spawned with no `--model`. Do not copy Settings `reply_model`.
pub const GROK_CLI_MODEL: &str = "grok-cli-default";

static ANSWERED: std::sync::Mutex<Option<(String, String)>> = std::sync::Mutex::new(None);

/// Record the lane and model that actually returned the reply.
pub fn note_answered(lane: &str, model: &str) {
    if let Ok(mut slot) = ANSWERED.lock() {
        *slot = Some((lane.to_string(), model.to_string()));
    }
}

pub fn answered_pending() -> bool {
    ANSWERED.lock().map(|g| g.is_some()).unwrap_or(false)
}

pub fn take_answered() -> Option<(String, String)> {
    ANSWERED.lock().ok().and_then(|mut slot| slot.take())
}

/// Archive label. `reply_model` is written only when the local lane answered.
pub fn label_from_answer(lane: &str, model: &str, reply_model: &str) -> (String, String) {
    let lane = lane.trim();
    let model = model.trim();
    let reply_model = reply_model.trim();
    if lane == "local" {
        let id = if model.is_empty() { reply_model } else { model };
        return ("local".into(), id.to_string());
    }
    if lane == "grok" {
        let id = if model.is_empty() || model == reply_model {
            GROK_CLI_MODEL
        } else {
            model
        };
        return ("grok".into(), id.to_string());
    }
    let id = if model.is_empty() { "?" } else { model };
    (lane.to_string(), id.to_string())
}

thread_local! {
    static META: std::cell::RefCell<TurnMeta> = std::cell::RefCell::new(TurnMeta::unknown());
}

pub struct TurnGuard(TurnMeta);

impl TurnGuard {
    pub fn set(meta: TurnMeta) -> Self {
        let prev = META.with(|c| c.replace(meta));
        Self(prev)
    }
}

impl Drop for TurnGuard {
    fn drop(&mut self) {
        let prev = std::mem::replace(&mut self.0, TurnMeta::unknown());
        META.with(|c| *c.borrow_mut() = prev);
    }
}

fn current_meta() -> TurnMeta {
    META.with(|c| c.borrow().clone())
}

#[cfg(test)]
thread_local! {
    static FORCE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    static ROOT: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
    static WARNED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static WARN_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(not(test))]
static WARNED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
pub(crate) fn force_flag(on: Option<bool>) {
    FORCE.with(|c| c.set(on));
}

#[cfg(test)]
pub(crate) fn install_root(root: Option<PathBuf>) {
    ROOT.with(|c| *c.borrow_mut() = root);
}

#[cfg(test)]
pub(crate) fn reset_warn() {
    WARNED.with(|c| c.set(false));
    WARN_COUNT.with(|c| c.set(0));
}

#[cfg(test)]
pub(crate) fn warn_count() -> usize {
    WARN_COUNT.with(|c| c.get())
}

/// Flag on and a root we are allowed to write. Test builds stay dark
/// until a test installs both.
pub fn enabled() -> bool {
    #[cfg(test)]
    {
        return FORCE.with(|c| c.get()) == Some(true) && ROOT.with(|c| c.borrow().is_some());
    }
    #[cfg(not(test))]
    {
        crate::flags::on_ambient("mh_archive")
    }
}

pub fn archive_root() -> Option<PathBuf> {
    #[cfg(test)]
    {
        return ROOT.with(|c| c.borrow().clone());
    }
    #[cfg(not(test))]
    {
        Some(crate::coordinator::companions_root())
    }
}

fn warn_once(msg: &str) {
    #[cfg(test)]
    {
        if !WARNED.with(|c| c.replace(true)) {
            WARN_COUNT.with(|c| c.set(c.get() + 1));
            eprintln!("orbitty: companion archive: {msg}");
        }
    }
    #[cfg(not(test))]
    {
        if !WARNED.swap(true, Ordering::Relaxed) {
            eprintln!("orbitty: companion archive: {msg}");
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ArchiveLine {
    pub v: u8,
    pub id: String,
    pub ts: String,
    pub role: String,
    pub mode: String,
    pub lane: String,
    pub model: String,
    pub session: Option<String>,
    pub text: String,
    pub src: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Carried {
    pub soul_chars: usize,
    pub rules_chars: usize,
    pub open_items: usize,
    pub dropped_items: usize,
    pub verbatim_pairs: usize,
    pub archive_upto: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResetLine {
    pub ts: String,
    pub reason: String,
    pub backend: String,
    pub old: String,
    pub new: String,
    pub carried: Carried,
}

fn new_id() -> String {
    static N: AtomicU64 = AtomicU64::new(1);
    let ms = Utc::now().timestamp_millis();
    let n = N.fetch_add(1, Ordering::Relaxed);
    format!("t-{ms}-{n}")
}

fn nth_sunday(year: i32, month: u32, nth: u32) -> chrono::NaiveDate {
    let mut day = chrono::NaiveDate::from_ymd_opt(year, month, 1).expect("date");
    while day.weekday() != chrono::Weekday::Sun {
        day += Duration::days(1);
    }
    day + Duration::days((nth as i64 - 1) * 7)
}

/// America/Chicago. Second Sunday in March 02:00 through the first Sunday
/// in November 02:00 is UTC-5. The rest of the year is UTC-6.
pub fn chicago_offset(utc: DateTime<Utc>) -> FixedOffset {
    let year = utc.year();
    let start_local = nth_sunday(year, 3, 2).and_time(NaiveTime::from_hms_opt(2, 0, 0).unwrap());
    let end_local = nth_sunday(year, 11, 1).and_time(NaiveTime::from_hms_opt(2, 0, 0).unwrap());
    let cst = FixedOffset::west_opt(6 * 3600).unwrap();
    let cdt = FixedOffset::west_opt(5 * 3600).unwrap();
    let start_utc = cst.from_local_datetime(&start_local).single().unwrap();
    let end_utc = cdt.from_local_datetime(&end_local).single().unwrap();
    if utc >= start_utc && utc < end_utc {
        cdt
    } else {
        cst
    }
}

pub fn ct_stamp(utc: DateTime<Utc>) -> (String, String) {
    let local = utc.with_timezone(&chicago_offset(utc));
    (
        local.format("%Y-%m-%d").to_string(),
        local.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
    )
}

fn ct_now() -> (String, String) {
    ct_stamp(Utc::now())
}

fn ensure_dir(dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let mut b = fs::DirBuilder::new();
        b.recursive(true).mode(0o700);
        b.create(dir)?;
        let mut p = fs::metadata(dir)?.permissions();
        p.set_mode(0o700);
        let _ = fs::set_permissions(dir, p);
        return Ok(());
    }
    #[cfg(not(unix))]
    fs::create_dir_all(dir)
}

fn append_file(path: &Path, body: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let mut opts = OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(path)?.permissions();
        p.set_mode(0o600);
        let _ = fs::set_permissions(path, p);
    }
    f.write_all(body.as_bytes())?;
    if !body.ends_with('\n') {
        f.write_all(b"\n")?;
    }
    f.flush()?;
    f.sync_all()?;
    Ok(())
}

pub fn append(root: &Path, line: &ArchiveLine) -> io::Result<()> {
    let (day, _) = ct_now();
    let dir = root.join("archive");
    ensure_dir(&dir)?;
    let path = dir.join(format!("{day}.jsonl"));
    let body = serde_json::to_string(line)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    append_file(&path, &body)
}

fn held_path(root: &Path) -> PathBuf {
    root.join(".archive-held")
}

fn read_held(root: &Path) -> usize {
    fs::read_to_string(held_path(root))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

fn write_held(root: &Path, n: usize) -> io::Result<()> {
    ensure_dir(root)?;
    let body = format!("{n}\n");
    let path = held_path(root);
    // Rewrite, not append: the count is one number.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut opts = OpenOptions::new();
        opts.create(true).write(true).truncate(true).mode(0o600);
        let mut f = opts.open(&path)?;
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(&path)?.permissions();
        p.set_mode(0o600);
        let _ = fs::set_permissions(&path, p);
        f.write_all(body.as_bytes())?;
        f.flush()?;
        f.sync_all()?;
        return Ok(());
    }
    #[cfg(not(unix))]
    {
        fs::write(&path, body)?;
        Ok(())
    }
}

pub fn marker_path(root: &Path) -> PathBuf {
    root.join("archive").join(".backfill-v1")
}

pub fn marker_exists(root: &Path) -> bool {
    marker_path(root).is_file()
}

fn write_marker(root: &Path) -> io::Result<()> {
    let (_, ts) = ct_now();
    append_file(&marker_path(root), &format!("v1 {ts}"))
}

fn archived_pairs(root: &Path) -> HashSet<(String, String)> {
    let mut set = HashSet::new();
    let dir = root.join("archive");
    let Ok(rd) = fs::read_dir(&dir) else {
        return set;
    };
    for ent in rd.flatten() {
        let name = ent.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.ends_with(".jsonl") {
            continue;
        }
        let Ok(text) = fs::read_to_string(ent.path()) else {
            continue;
        };
        for line in text.lines() {
            if let Ok(row) = serde_json::from_str::<ArchiveLine>(line) {
                set.insert((row.role, row.text));
            }
        }
    }
    set
}

fn backfill_line(mode: &str, turn: &CompanionTurn, ts: &str, src: &str) -> ArchiveLine {
    ArchiveLine {
        v: 1,
        id: new_id(),
        ts: ts.into(),
        role: turn.role.clone(),
        mode: mode.into(),
        lane: "?".into(),
        model: "?".into(),
        session: None,
        text: turn.text.clone(),
        src: src.into(),
    }
}

/// Turns, then parked spark/chat/work/hold rows that are not already in
/// `turns` or already in the archive. Marker `.backfill-v1` makes a second
/// call append nothing.
pub fn backfill_once(root: &Path, st: &CompanionState) -> io::Result<usize> {
    if marker_exists(root) {
        return Ok(0);
    }
    let have = archived_pairs(root);
    let (_, ts) = ct_now();
    let mut n = 0usize;
    for turn in &st.turns {
        let key = (turn.role.clone(), turn.text.clone());
        if have.contains(&key) {
            continue;
        }
        append(root, &backfill_line(&st.mode, turn, &ts, "backfill:state.turns"))?;
        n += 1;
    }
    let in_turns: HashSet<(String, String)> = st
        .turns
        .iter()
        .map(|t| (t.role.clone(), t.text.clone()))
        .collect();
    for mode in PARKED_MODES {
        let Some(rows) = st.threads.get(*mode) else {
            continue;
        };
        for turn in rows {
            let key = (turn.role.clone(), turn.text.clone());
            if in_turns.contains(&key) || have.contains(&key) {
                continue;
            }
            let src = format!("backfill:state.threads.{mode}");
            append(root, &backfill_line(mode, turn, &ts, &src))?;
            n += 1;
        }
    }
    write_marker(root)?;
    Ok(n)
}

fn live_line(st: &CompanionState, turn: &CompanionTurn) -> ArchiveLine {
    let meta = current_meta();
    let (_, ts) = ct_now();
    ArchiveLine {
        v: 1,
        id: new_id(),
        ts,
        role: turn.role.clone(),
        mode: st.mode.clone(),
        lane: meta.lane,
        model: meta.model,
        session: meta.session,
        text: turn.text.clone(),
        src: meta.src,
    }
}

fn flush_held(st: &CompanionState, root: &Path, held: usize) -> Result<(), (io::Error, usize)> {
    let start = st.turns.len().saturating_sub(held);
    for (i, turn) in st.turns[start..].iter().enumerate() {
        if let Err(e) = append(root, &live_line(st, turn)) {
            return Err((e, held - i));
        }
    }
    Ok(())
}

fn mirror(st: &mut CompanionState) {
    st.threads.insert(st.mode.clone(), st.turns.clone());
}

/// Flag-on push. Archive the new turn (and any held tail) before the cap.
/// Append failure keeps the extra turns and warns once.
pub fn push_archived(st: &mut CompanionState, role: &str, text: &str) {
    let Some(root) = archive_root() else {
        return;
    };
    let mut held = read_held(&root);
    if held == 0 && !marker_exists(&root) {
        if let Err(e) = backfill_once(&root, st) {
            warn_once(&format!("backfill: {e}"));
        }
    }
    st.turns.push(CompanionTurn {
        role: role.into(),
        text: text.into(),
    });
    held += 1;
    st.unarchived = held;
    st.hold_drain = true;
    let _ = write_held(&root, held);
    match flush_held(st, &root, held) {
        Ok(()) => {
            st.unarchived = 0;
            st.hold_drain = false;
            let _ = write_held(&root, 0);
            if !marker_exists(&root) {
                if let Err(e) = backfill_once(&root, st) {
                    warn_once(&format!("backfill: {e}"));
                }
            }
            if marker_exists(&root) {
                cap_companion_turns(&mut st.turns);
                mirror(st);
            }
        }
        Err((e, left)) => {
            st.unarchived = left;
            st.hold_drain = true;
            let _ = write_held(&root, left);
            warn_once(&format!("append: {e}"));
            if marker_exists(&root) {
                mirror(st);
            }
        }
    }
}

/// Try the one-time backfill before a mode switch. The caller skips the
/// thread overwrite until the marker exists.
pub fn prepare_switch(st: &CompanionState) {
    let Some(root) = archive_root() else {
        return;
    };
    if read_held(&root) == 0 && !marker_exists(&root) {
        if let Err(e) = backfill_once(&root, st) {
            warn_once(&format!("backfill: {e}"));
        }
    }
}

pub fn marker_ready() -> bool {
    archive_root().is_some_and(|r| marker_exists(&r))
}

pub fn tail(root: &Path, n: usize, date: Option<&str>) -> Vec<ArchiveLine> {
    let dir = root.join("archive");
    let mut files: Vec<PathBuf> = Vec::new();
    if let Some(day) = date {
        files.push(dir.join(format!("{day}.jsonl")));
    } else if let Ok(rd) = fs::read_dir(&dir) {
        for ent in rd.flatten() {
            let name = ent.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.ends_with(".jsonl") {
                files.push(ent.path());
            }
        }
        files.sort();
    }
    let mut rows = Vec::new();
    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            if let Ok(row) = serde_json::from_str::<ArchiveLine>(line) {
                rows.push(row);
            }
        }
    }
    if n == 0 || rows.len() <= n {
        rows
    } else {
        rows.split_off(rows.len() - n)
    }
}

pub fn last_archive_id(root: &Path) -> String {
    tail(root, 1, None)
        .into_iter()
        .next()
        .map(|r| r.id)
        .unwrap_or_default()
}

/// Read-only lookup. Archive line format is unchanged.
pub fn by_id(root: &Path, id: &str) -> Option<ArchiveLine> {
    let want = id.trim();
    if want.is_empty() {
        return None;
    }
    tail(root, usize::MAX, None)
        .into_iter()
        .find(|r| r.id == want)
}

pub fn log_reset(root: &Path, line: &ResetLine) -> io::Result<()> {
    let body = serde_json::to_string(line)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    append_file(&root.join("resets.jsonl"), &body)
}

pub fn resets_tail(root: &Path, n: usize) -> Vec<ResetLine> {
    let Ok(text) = fs::read_to_string(root.join("resets.jsonl")) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for line in text.lines() {
        if let Ok(row) = serde_json::from_str::<ResetLine>(line) {
            rows.push(row);
        }
    }
    if n == 0 || rows.len() <= n {
        rows
    } else {
        rows.split_off(rows.len() - n)
    }
}

pub fn session_budget() -> u64 {
    std::env::var("ORBITTY_MH_SESSION_BUDGET_CHARS")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .filter(|n: &u64| *n > 0)
        .unwrap_or(MH_SESSION_BUDGET_CHARS)
}

/// Planned reset while the flag is on. The 16-send roll is not one of these.
pub fn planned_reason(pending: Option<&str>, chars: u64, budget: u64) -> Option<&'static str> {
    match pending {
        Some("manual_test") => Some("manual_test"),
        Some("thin") => Some("thin"),
        _ => None,
    }
    .or_else(|| {
        if chars >= budget && budget > 0 {
            Some("budget")
        } else {
            None
        }
    })
}

/// Flag off keeps today's every-16 Grok roll. The old constant name is gone.
pub fn legacy_send_roll(archive_on: bool, minted: bool, sends: u32) -> bool {
    !archive_on && minted && sends >= 16
}

pub fn arm_manual_roll() -> Result<(), String> {
    let mut plug = crate::coordinator::load_plug();
    plug.reset_pending = Some("manual_test".into());
    crate::coordinator::save_plug(&plug);
    Ok(())
}

#[cfg(test)]
#[path = "mouth_archive_tests.rs"]
mod tests;

