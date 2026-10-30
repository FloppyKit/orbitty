//! Search archive + planet notes + tabs. Never writes the archive.

use std::fs;
use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;

use super::planet_dir;
use crate::mouth_archive::{tail, ArchiveLine};

pub(crate) const RECALL_MAX: usize = 8;
const EXCERPT: usize = 240;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Hit {
    pub date: String,
    pub source: String,
    pub text: String,
}

static CITE: Mutex<Option<String>> = Mutex::new(None);

pub(crate) fn recall(
    archive_root: &Path,
    app_data: &Path,
    query: &str,
    planet: Option<&str>,
) -> Vec<Hit> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_ascii_lowercase();
    let mut hits = Vec::new();
    for row in tail(archive_root, usize::MAX, None) {
        if let Some(hit) = hit_line(&row, &needle) {
            hits.push(hit);
        }
    }
    let planets: Vec<std::path::PathBuf> = if let Some(id) = planet {
        planet_dir(app_data, id).into_iter().collect()
    } else if let Ok(rd) = fs::read_dir(app_data.join("planets")) {
        rd.filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect()
    } else {
        Vec::new()
    };
    for dir in planets {
        let notes = dir.join("notes.md");
        if let Some(hit) = hit_file(&notes, "notes", &needle) {
            hits.push(hit);
        }
        collect_tabs(&dir.join("tabs"), &needle, &mut hits, 0);
    }
    hits.sort_by(|a, b| b.date.cmp(&a.date).then(a.source.cmp(&b.source)));
    if hits.len() > RECALL_MAX {
        hits.truncate(RECALL_MAX);
    }
    hits
}

fn hit_line(row: &ArchiveLine, needle: &str) -> Option<Hit> {
    if !row.text.to_ascii_lowercase().contains(needle) {
        return None;
    }
    Some(Hit {
        date: date_of_ts(&row.ts),
        source: "archive".into(),
        text: excerpt(&row.text),
    })
}

fn hit_file(path: &Path, source: &str, needle: &str) -> Option<Hit> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > 256 * 1024 {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    if !text.to_ascii_lowercase().contains(needle) {
        return None;
    }
    Some(Hit {
        date: date_of_mtime(path),
        source: source.into(),
        text: excerpt(&text),
    })
}

fn collect_tabs(dir: &Path, needle: &str, hits: &mut Vec<Hit>, depth: usize) {
    if depth > 4 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        if path.is_dir() {
            collect_tabs(&path, needle, hits, depth + 1);
            continue;
        }
        if let Some(hit) = hit_file(&path, "tabs", needle) {
            hits.push(hit);
        }
    }
}

fn excerpt(text: &str) -> String {
    let one = text.trim().replace('\n', " ");
    if one.chars().count() <= EXCERPT {
        one
    } else {
        one.chars().take(EXCERPT).collect()
    }
}

fn date_of_ts(ts: &str) -> String {
    let t = ts.trim();
    if t.len() >= 10 && t.as_bytes().get(4) == Some(&b'-') && t.as_bytes().get(7) == Some(&b'-') {
        t[..10].to_string()
    } else if t.is_empty() {
        "1970-01-01".into()
    } else {
        t.to_string()
    }
}

fn date_of_mtime(path: &Path) -> String {
    let Ok(modified) = fs::metadata(path).and_then(|m| m.modified()) else {
        return "1970-01-01".into();
    };
    let dt: chrono::DateTime<chrono::Utc> = modified.into();
    dt.format("%Y-%m-%d").to_string()
}

pub(crate) fn mark_recalled(query: &str, hits: &[Hit]) {
    let dates: Vec<&str> = hits.iter().map(|h| h.date.as_str()).collect();
    let line = format!("{query} | {} | {}", dates.join(","), hits.len());
    *CITE.lock().unwrap_or_else(|p| p.into_inner()) = Some(line);
}

fn take_cite() -> Option<String> {
    CITE.lock().unwrap_or_else(|p| p.into_inner()).take()
}

pub(crate) fn claims_decision(text: &str) -> bool {
    let l = text.to_ascii_lowercase();
    const NEEDLES: &[&str] = &[
        "we decided",
        "we decide",
        "did we decide",
        "decided to",
        "the decision",
        "decision was",
        "we agreed",
        "we chose",
        "we choose",
        "our decision",
    ];
    NEEDLES.iter().any(|n| l.contains(n))
}

fn has_checked(text: &str) -> bool {
    text.to_ascii_lowercase().contains("checked:")
}

pub(crate) fn needs_checked_line(reply: &str, recalled: bool) -> bool {
    claims_decision(reply) && !recalled && !has_checked(reply)
}

pub(crate) fn apply_checked(reply: &str, recalled: bool) -> String {
    if !needs_checked_line(reply, recalled) {
        return reply.to_string();
    }
    format!("{}\nChecked: none", reply.trim_end())
}

pub(crate) fn finish_reply(app: &tauri::AppHandle, reply: &str) -> String {
    if !super::enabled(app) {
        return reply.to_string();
    }
    let cite = take_cite();
    if has_checked(reply) || !claims_decision(reply) {
        return reply.to_string();
    }
    match cite.as_deref() {
        Some(c) if !c.is_empty() => format!("{}\nChecked: {c}", reply.trim_end()),
        _ => apply_checked(reply, false),
    }
}
