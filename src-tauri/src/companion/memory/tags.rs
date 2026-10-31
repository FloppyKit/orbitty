//! tag.v0 lives beside state.json, never inside it.
//! Proposals sit in memory until accept.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use super::{global_tags_path, planet_tags_path, write_0600};
use crate::state::Tag;

pub(crate) const PROPOSE_MAX: usize = 5;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TagV0 {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub scope: String,
    pub source: String,
}

impl TagV0 {
    pub(crate) fn from_tag(tag: &Tag, scope: &str, source: &str) -> Option<Self> {
        let name = crate::microtags::normalize(&tag.name)?;
        if tag.id.trim().is_empty() {
            return None;
        }
        Some(Self {
            id: tag.id.trim().to_string(),
            name,
            color: tag.color.clone(),
            scope: scope.to_string(),
            source: source.to_string(),
        })
    }

    pub(crate) fn to_tag(&self) -> Tag {
        Tag {
            id: self.id.clone(),
            name: self.name.clone(),
            color: self.color.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TagFile {
    #[serde(default)]
    pinned: Vec<String>,
    #[serde(default)]
    tags: Vec<TagV0>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AcceptBatch {
    pub names: Vec<String>,
    pub planet: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NeededDraft {
    pub title: String,
    pub kind: String,
    pub why: String,
    pub names: Vec<String>,
}

fn live_queue() -> MutexGuard<'static, Vec<AcceptBatch>> {
    static QUEUE: Mutex<Vec<AcceptBatch>> = Mutex::new(Vec::new());
    QUEUE.lock().unwrap_or_else(|p| p.into_inner())
}

pub(crate) fn propose_live(
    app_data: &Path,
    names: &[String],
    planet: Option<String>,
) -> Result<NeededDraft, String> {
    let mut queue = live_queue();
    propose_into(&mut queue, app_data, names, planet)
}

pub(crate) fn rollback_live() {
    live_queue().pop();
}

pub(crate) fn accept_live(
    app_data: &Path,
    planet: Option<&str>,
) -> Result<Vec<TagV0>, String> {
    let mut queue = live_queue();
    accept_into(&mut queue, app_data, planet)
}

pub(crate) fn propose_into(
    queue: &mut Vec<AcceptBatch>,
    app_data: &Path,
    names: &[String],
    planet: Option<String>,
) -> Result<NeededDraft, String> {
    // Accept is the only writer. `app_data` is the directory a proposal must
    // not touch; the test watches it.
    let _ = app_data;
    let mut kept = Vec::new();
    for raw in names {
        let Some(name) = crate::microtags::normalize(raw) else {
            continue;
        };
        if kept.iter().any(|n: &String| n == &name) {
            continue;
        }
        kept.push(name);
        if kept.len() == PROPOSE_MAX {
            break;
        }
    }
    if kept.is_empty() {
        return Err("no tag names in microtags normal form".into());
    }
    let why = kept.join(", ");
    let draft = NeededDraft {
        title: format!("Approve: {why}"),
        kind: "Approve".into(),
        why: why.clone(),
        names: kept.clone(),
    };
    queue.push(AcceptBatch {
        names: kept,
        planet,
    });
    Ok(draft)
}

pub(crate) fn accept_into(
    queue: &mut Vec<AcceptBatch>,
    app_data: &Path,
    planet: Option<&str>,
) -> Result<Vec<TagV0>, String> {
    let idx = queue.iter().rposition(|b| match (&b.planet, planet) {
        (Some(p), Some(want)) => p == want,
        (None, None) => true,
        _ => false,
    });
    let Some(idx) = idx else {
        return Err("no tag proposal to accept".into());
    };
    let batch = queue.remove(idx);
    let (path, scope) = match batch.planet.as_deref() {
        Some(id) => {
            let path = planet_tags_path(app_data, id).ok_or("bad planet id")?;
            (path, format!("planet:{id}"))
        }
        None => (global_tags_path(app_data), "global".into()),
    };
    let mut file = read_tag_file(&path);
    let mut wrote = Vec::new();
    for name in &batch.names {
        if file.tags.iter().any(|t| &t.name == name) {
            continue;
        }
        let row = TagV0 {
            id: new_tag_id(),
            name: name.clone(),
            color: None,
            scope: scope.clone(),
            source: "accepted_proposal".into(),
        };
        file.tags.push(row.clone());
        wrote.push(row);
    }
    let body = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    write_0600(&path, &body).map_err(|e| e.to_string())?;
    Ok(wrote)
}

/// Drop one pinned name. Planet set: that file only. Else the global file.
/// Does not delete the tag row. Does not touch state.json.
pub(crate) fn unpin(app_data: &Path, name: &str, planet: Option<&str>) -> Result<bool, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("need a tag name".into());
    }
    let norm = crate::microtags::normalize(name);
    let path = match planet {
        Some(id) => planet_tags_path(app_data, id).ok_or("bad planet id")?,
        None => global_tags_path(app_data),
    };
    let mut file = read_tag_file(&path);
    let before = file.pinned.len();
    file.pinned.retain(|n| {
        let n = n.trim();
        n != name && norm.as_deref() != Some(n) && crate::microtags::normalize(n).as_deref() != Some(name)
    });
    if file.pinned.len() == before {
        return Ok(false);
    }
    let body = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    write_0600(&path, &body).map_err(|e| e.to_string())?;
    Ok(true)
}

pub(crate) fn pinned_names(app_data: Option<&Path>, planet: Option<&str>) -> Vec<String> {
    let Some(app_data) = app_data else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(id) = planet {
        if let Some(path) = planet_tags_path(app_data, id) {
            push_pinned(&read_tag_file(&path), &mut out);
        }
    }
    push_pinned(&read_tag_file(&global_tags_path(app_data)), &mut out);
    out
}

fn push_pinned(file: &TagFile, out: &mut Vec<String>) {
    for name in &file.pinned {
        let name = name.trim();
        if name.is_empty() || out.iter().any(|n: &String| n == name) {
            continue;
        }
        out.push(name.to_string());
        if out.len() == super::ram::PINNED_MAX {
            break;
        }
    }
}

fn read_tag_file(path: &Path) -> TagFile {
    let Ok(raw) = fs::read_to_string(path) else {
        return TagFile::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn new_tag_id() -> String {
    static N: AtomicU64 = AtomicU64::new(1);
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    format!("tg-{ms}-{n}")
}
