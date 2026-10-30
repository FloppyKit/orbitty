//! Shim-ledger paths and the on-disk format.
//! `<app data>/ledger/shims/{fires.jsonl,counts.json}`.
//! Proof rows and leases are a later GO; this module does not grow them.
//!
//! `fires.jsonl` lines are `{ts, id, seat?, count}` only.
//! `counts.json` is replaced by an atomic rename. Both files are mode 0600.

use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const TSV_HEADER: &str = "shim_id\tsymptom\tpatches\tfile\tadded\tfirst_fired\tlast_fired\tfires_7d\tfires_30d\tfires_total\tmodels_seen\tstatus";

static LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShimRow {
    pub id: String,
    pub site: String,
    pub since: String,
    pub note: String,
    pub patches: String,
}

#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct CountRow {
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub first: String,
    #[serde(default)]
    pub last: String,
    #[serde(default)]
    pub by_day: BTreeMap<String, u64>,
}

pub fn shims_dir(app_data: &Path) -> PathBuf {
    app_data.join("ledger").join("shims")
}

pub fn fires_path(app_data: &Path) -> PathBuf {
    shims_dir(app_data).join("fires.jsonl")
}

pub fn counts_path(app_data: &Path) -> PathBuf {
    shims_dir(app_data).join("counts.json")
}

/// One firing. A write error is returned; callers ignore it.
/// `count` on the jsonl line is always 1 (this firing, not a running total).
pub fn record(app_data: &Path, id: &str, seat: Option<&str>) -> std::io::Result<()> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = shims_dir(app_data);
    ensure_dir(&dir)?;
    let (ts, day) = stamp();
    let seat = seat.map(str::trim).filter(|s| !s.is_empty());
    let path = counts_path(app_data);
    let mut counts = read_counts(&path);
    {
        let row = counts.entry(id.to_string()).or_default();
        if row.first.is_empty() {
            row.first = ts.clone();
        }
        row.last = ts.clone();
        row.total = row.total.saturating_add(1);
        *row.by_day.entry(day).or_insert(0) += 1;
    }
    let mut bytes = serde_json::to_vec(&counts)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    bytes.push(b'\n');
    atomic_write(&path, &bytes)?;
    append_fire(&fires_path(app_data), &ts, id, seat)?;
    Ok(())
}

pub fn load_counts(path: &Path) -> BTreeMap<String, CountRow> {
    read_counts(path)
}

pub fn render_tsv(rows: &[ShimRow], counts: &BTreeMap<String, CountRow>, today: &str) -> String {
    let empty = BTreeMap::new();
    let mut out = String::new();
    out.push_str(TSV_HEADER);
    out.push('\n');
    for row in rows {
        let found = counts.get(&row.id);
        let total = found.map(|c| c.total).unwrap_or(0);
        let first = found.map(|c| c.first.as_str()).unwrap_or("");
        let last = found.map(|c| c.last.as_str()).unwrap_or("");
        let by = found.map(|c| &c.by_day).unwrap_or(&empty);
        let d7 = fires_in_last(by, today, 7);
        let d30 = fires_in_last(by, today, 30);
        let st = status(&row.since, by, today);
        out.push_str(&row.id);
        out.push('\t');
        out.push_str(&row.note);
        out.push('\t');
        out.push_str(&row.patches);
        out.push('\t');
        out.push_str(&row.site);
        out.push('\t');
        out.push_str(&row.since);
        out.push('\t');
        out.push_str(first);
        out.push('\t');
        out.push_str(last);
        out.push('\t');
        out.push_str(&d7.to_string());
        out.push('\t');
        out.push_str(&d30.to_string());
        out.push('\t');
        out.push_str(&total.to_string());
        out.push('\t');
        // models_seen stays empty: a fire line has no model (counts only).
        out.push('\t');
        out.push_str(st);
        out.push('\n');
    }
    out
}

pub fn export_at(
    home: &Path,
    app_data: &Path,
    rows: &[ShimRow],
    out: Option<&str>,
    today: &str,
) -> Result<String, String> {
    let counts = load_counts(&counts_path(app_data));
    let body = render_tsv(rows, &counts, today);
    if let Some(out) = out {
        let dest = resolve_out(home, out)?;
        atomic_write(&dest, body.as_bytes()).map_err(|e| format!("ledger export: {e}"))?;
    }
    Ok(body)
}

/// `--out` must resolve inside `home`. `~/…` is that home, not the env.
pub fn resolve_out(home: &Path, out: &str) -> Result<PathBuf, String> {
    if out.is_empty() || out.contains('\0') || out.contains('\n') {
        return Err("ledger export: --out needs a path".into());
    }
    let home = home
        .canonicalize()
        .map_err(|e| format!("ledger export: HOME: {e}"))?;
    let raw = if out == "~" {
        home.clone()
    } else if let Some(rest) = out.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(out)
    };
    let abs = if raw.is_absolute() {
        raw
    } else {
        std::env::current_dir()
            .map_err(|e| format!("ledger export: cwd: {e}"))?
            .join(raw)
    };
    let name = abs
        .file_name()
        .filter(|n| !n.is_empty())
        .ok_or("ledger export: --out needs a file name")?
        .to_os_string();
    let parent = abs.parent().unwrap_or_else(|| Path::new("/"));
    if !parent.exists() {
        return Err("ledger export: --out directory does not exist".into());
    }
    let parent = parent
        .canonicalize()
        .map_err(|e| format!("ledger export: --out: {e}"))?;
    let full = parent.join(name);
    if !full.starts_with(&home) {
        return Err("ledger export: --out must stay under $HOME".into());
    }
    if full.is_symlink() || full.exists() {
        if let Ok(canon) = full.canonicalize() {
            if !canon.starts_with(&home) {
                return Err("ledger export: --out must stay under $HOME".into());
            }
            return Ok(canon);
        }
    }
    Ok(full)
}

/// `active` has a fire in the last 14 CT days.
/// `delete_candidate` is at least 14 days old and has none in that window.
/// `quiet` is younger than that, with none in the window.
pub fn status(since: &str, by_day: &BTreeMap<String, u64>, today: &str) -> &'static str {
    if fires_in_last(by_day, today, 14) > 0 {
        return "active";
    }
    if age_at_least(since, today, 14) {
        return "delete_candidate";
    }
    "quiet"
}

pub fn parse_shims(src: &str) -> Result<Vec<ShimRow>, String> {
    let start = src.find("pub const SHIMS:").ok_or("shim table missing")?;
    let rest = &src[start..];
    let end = rest.find("\n];").ok_or("shim table unclosed")?;
    let body = &rest[..end];
    let mut rows = Vec::new();
    for block in body.split("ShimDef {").skip(1) {
        rows.push(ShimRow {
            id: field(block, "id")?.to_string(),
            site: field(block, "site")?.to_string(),
            since: field(block, "since")?.to_string(),
            note: field(block, "note")?.to_string(),
            patches: field(block, "patches")?.to_string(),
        });
    }
    if rows.is_empty() {
        return Err("shim table missing".into());
    }
    Ok(rows)
}

fn field<'a>(block: &'a str, key: &str) -> Result<&'a str, String> {
    let pat = format!("{key}: \"");
    let i = block
        .find(&pat)
        .ok_or_else(|| format!("shim table missing {key}"))?;
    let rest = &block[i + pat.len()..];
    let j = rest
        .find('"')
        .ok_or_else(|| format!("shim table unclosed {key}"))?;
    Ok(&rest[..j])
}

fn fires_in_last(by_day: &BTreeMap<String, u64>, today: &str, days: i64) -> u64 {
    let Some(today) = parse_day(today) else {
        return 0;
    };
    let start = today - chrono::Duration::days(days - 1);
    by_day
        .iter()
        .filter_map(|(day, n)| {
            let day = parse_day(day)?;
            if day >= start && day <= today {
                Some(*n)
            } else {
                None
            }
        })
        .sum()
}

fn age_at_least(since: &str, today: &str, days: i64) -> bool {
    match (parse_day(since), parse_day(today)) {
        (Some(since), Some(today)) => (today - since).num_days() >= days,
        _ => false,
    }
}

fn parse_day(s: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn read_counts(path: &Path) -> BTreeMap<String, CountRow> {
    let Ok(bytes) = fs::read(path) else {
        return BTreeMap::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn stamp() -> (String, String) {
    let now = chrono::Local::now();
    (
        now.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        now.format("%Y-%m-%d").to_string(),
    )
}

#[derive(Serialize)]
struct FireLine<'a> {
    ts: &'a str,
    id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    seat: Option<&'a str>,
    count: u64,
}

fn append_fire(path: &Path, ts: &str, id: &str, seat: Option<&str>) -> std::io::Result<()> {
    let line = serde_json::to_string(&FireLine {
        ts,
        id,
        seat,
        count: 1,
    })
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let mut f = open_0600(path, true)?;
    writeln!(f, "{line}")?;
    f.sync_all()?;
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = open_0600(&tmp, false)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    restrict_0600(path);
    Ok(())
}

fn ensure_dir(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn open_0600(path: &Path, append: bool) -> std::io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.create(true);
    if append {
        opts.append(true);
    } else {
        opts.write(true).truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let f = opts.open(path)?;
    restrict_0600(path);
    Ok(f)
}

fn restrict_0600(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}
