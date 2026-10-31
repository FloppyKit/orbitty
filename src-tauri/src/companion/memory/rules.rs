//! User rules. `<app data>/companion/rules.json` (0600, dir 0700).
//! Not `state.json`. Chat may only propose; accept writes that batch.
//! `set` / `rm` are the user verbs (orbittyctl / MCP).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::ensure_dir;
use super::tags::NeededDraft;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct UserRule {
    pub id: String,
    pub text: String,
    pub created_ts: String,
    pub updated_ts: String,
    pub source: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct RuleFile {
    #[serde(default)]
    rules: Vec<UserRule>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RuleProposal {
    pub id: Option<String>,
    pub text: String,
    pub remove: bool,
}

pub(crate) fn rules_path(app_data: &Path) -> PathBuf {
    app_data.join("companion").join("rules.json")
}

pub(crate) fn list(app_data: &Path) -> Vec<UserRule> {
    read_file(app_data).rules
}

pub(crate) fn render_text(app_data: &Path) -> String {
    list(app_data)
        .into_iter()
        .map(|r| r.text)
        .filter(|t| !t.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn set(
    app_data: &Path,
    id: Option<&str>,
    text: &str,
    source: &str,
) -> io::Result<UserRule> {
    let text = text.trim();
    if text.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty rule"));
    }
    let source = norm_source(source);
    let mut file = read_file(app_data);
    let now = now_ts();
    let want = id.map(str::trim).filter(|s| !s.is_empty());
    if let Some(id) = want {
        if let Some(row) = file.rules.iter_mut().find(|r| r.id == id) {
            row.text = text.to_string();
            row.updated_ts = now;
            row.source = source;
            let out = row.clone();
            write_atomic(app_data, &file)?;
            return Ok(out);
        }
        let row = UserRule {
            id: id.to_string(),
            text: text.to_string(),
            created_ts: now.clone(),
            updated_ts: now,
            source,
        };
        file.rules.push(row.clone());
        write_atomic(app_data, &file)?;
        return Ok(row);
    }
    let row = UserRule {
        id: new_id(),
        text: text.to_string(),
        created_ts: now.clone(),
        updated_ts: now,
        source,
    };
    file.rules.push(row.clone());
    write_atomic(app_data, &file)?;
    Ok(row)
}

pub(crate) fn rm(app_data: &Path, id: &str) -> io::Result<bool> {
    let id = id.trim();
    if id.is_empty() {
        return Ok(false);
    }
    let mut file = read_file(app_data);
    let before = file.rules.len();
    file.rules.retain(|r| r.id != id);
    if file.rules.len() == before {
        return Ok(false);
    }
    write_atomic(app_data, &file)?;
    Ok(true)
}

fn live_queue() -> MutexGuard<'static, Vec<RuleProposal>> {
    static QUEUE: Mutex<Vec<RuleProposal>> = Mutex::new(Vec::new());
    QUEUE.lock().unwrap_or_else(|p| p.into_inner())
}

pub(crate) fn propose_live(
    text: &str,
    id: Option<String>,
    remove: bool,
) -> Result<NeededDraft, String> {
    let mut queue = live_queue();
    propose_into(&mut queue, text, id, remove)
}

pub(crate) fn rollback_live() {
    live_queue().pop();
}

pub(crate) fn accept_live(app_data: &Path) -> Result<serde_json::Value, String> {
    let mut queue = live_queue();
    accept_into(&mut queue, app_data)
}

pub(crate) fn propose_into(
    queue: &mut Vec<RuleProposal>,
    text: &str,
    id: Option<String>,
    remove: bool,
) -> Result<NeededDraft, String> {
    let text = text.trim();
    let id = id.and_then(|s| {
        let s = s.trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    });
    if remove {
        let id = id.ok_or("rule remove needs an id")?;
        let why = format!("remove rule {id}");
        let draft = NeededDraft {
            title: format!("Approve: {why}"),
            kind: "Approve".into(),
            why: why.clone(),
            names: vec![id.clone()],
        };
        queue.push(RuleProposal {
            id: Some(id),
            text: String::new(),
            remove: true,
        });
        return Ok(draft);
    }
    if text.is_empty() {
        return Err("rule proposal needs text".into());
    }
    let why = if let Some(id) = &id {
        format!("replace {id}: {text}")
    } else {
        text.to_string()
    };
    let draft = NeededDraft {
        title: format!("Approve: {why}"),
        kind: "Approve".into(),
        why: why.clone(),
        names: vec![text.to_string()],
    };
    queue.push(RuleProposal {
        id,
        text: text.to_string(),
        remove: false,
    });
    Ok(draft)
}

pub(crate) fn accept_into(
    queue: &mut Vec<RuleProposal>,
    app_data: &Path,
) -> Result<serde_json::Value, String> {
    let Some(batch) = queue.pop() else {
        return Err("no rule proposal to accept".into());
    };
    if batch.remove {
        let id = batch.id.unwrap_or_default();
        let removed = rm(app_data, &id).map_err(|e| e.to_string())?;
        return Ok(serde_json::json!({ "removed": removed, "id": id }));
    }
    let row = set(
        app_data,
        batch.id.as_deref(),
        &batch.text,
        "accepted_proposal",
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "id": row.id,
        "text": row.text,
        "source": row.source,
    }))
}

fn read_file(app_data: &Path) -> RuleFile {
    let Ok(raw) = fs::read_to_string(rules_path(app_data)) else {
        return RuleFile::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn write_atomic(app_data: &Path, file: &RuleFile) -> io::Result<()> {
    let path = rules_path(app_data);
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let body = serde_json::to_string_pretty(file)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut opts = fs::OpenOptions::new();
        opts.create(true).write(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(body.as_bytes())?;
        if !body.ends_with('\n') {
            f.write_all(b"\n")?;
        }
        f.flush()?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(&path)?.permissions();
        p.set_mode(0o600);
        let _ = fs::set_permissions(&path, p);
    }
    Ok(())
}

fn norm_source(source: &str) -> String {
    if source.trim() == "accepted_proposal" {
        "accepted_proposal".into()
    } else {
        "user".into()
    }
}

fn now_ts() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    chrono::DateTime::from_timestamp(secs as i64, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| secs.to_string())
}

fn new_id() -> String {
    static N: AtomicU64 = AtomicU64::new(1);
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    format!("ur-{ms}-{n}")
}
