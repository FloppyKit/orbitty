//! Exact-lookup graph. Append-only `<app data>/companion/graph.jsonl` (0600).
//! Same id+rel+to does not append again. `build` stays pure; callers note.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::ensure_dir;
use super::ram::{RamCore, RamCtx, ANSWERED_TTL_TURNS};
use crate::mouth_archive::{by_id, tail, ArchiveLine};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Link {
    pub rel: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Node {
    pub id: String,
    pub kind: String,
    pub ts: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<String>,
    #[serde(default)]
    pub links: Vec<Link>,
}

pub(crate) fn graph_path(app_data: &Path) -> PathBuf {
    app_data.join("companion").join("graph.jsonl")
}

/// Fill answer ids and turn counts from the graph + archive. Writes an
/// `asked_in` binding the first time an answered ask is seen so later
/// prompts can count turns. The `answers` link is written only on age-out.
pub(crate) fn prepare(app_data: &Path, archive: &Path, ctx: &RamCtx) -> RamCtx {
    let mut ctx = ctx.clone();
    for row in &mut ctx.answered {
        if row.answer_id.trim().is_empty() {
            if let Some(id) = binding_turn(app_data, &row.id) {
                row.answer_id = id;
            } else if let Some(id) = last_archive_id(archive) {
                row.answer_id = id.clone();
                let node = Node {
                    id: row.id.clone(),
                    kind: "ask".into(),
                    ts: now_ts(),
                    archive_id: Some(id.clone()),
                    planet: None,
                    card: None,
                    links: vec![Link {
                        rel: "asked_in".into(),
                        to: id,
                    }],
                };
                if let Err(e) = note_aged(app_data, &node) {
                    eprintln!("orbitty: companion graph: bind: {e}");
                }
            }
        }
        if !row.answer_id.is_empty() {
            row.turns_since = turns_after(archive, &row.answer_id);
            if row.answer.is_empty() {
                if let Some(line) = by_id(archive, &row.answer_id) {
                    row.answer = line.text;
                }
            }
        }
    }
    ctx
}

pub(crate) fn note_leaves(
    app_data: &Path,
    archive: &Path,
    ctx: &RamCtx,
    fitted: &RamCore,
) -> io::Result<()> {
    let ids = archive_ids(archive);
    for row in &ctx.answered {
        if row.turns_since > ANSWERED_TTL_TURNS && !row.answer_id.is_empty() {
            note_aged(
                app_data,
                &Node {
                    id: row.answer_id.clone(),
                    kind: "answer".into(),
                    ts: now_ts(),
                    archive_id: Some(row.answer_id.clone()),
                    planet: None,
                    card: None,
                    links: vec![
                        Link {
                            rel: "answers".into(),
                            to: row.id.clone(),
                        },
                        Link {
                            rel: "asked_in".into(),
                            to: row.answer_id.clone(),
                        },
                    ],
                },
            )?;
        }
    }
    let kept: Vec<&str> = fitted.turns.iter().map(|t| t.text.as_str()).collect();
    let mut prev: Option<String> = None;
    for turn in &ctx.turns {
        let id = ids
            .iter()
            .find(|(role, text, _)| role == &turn.role && text == &turn.text)
            .map(|(_, _, id)| id.clone());
        let Some(id) = id else {
            continue;
        };
        if kept.iter().any(|t| *t == turn.text.as_str()) {
            prev = Some(id);
            continue;
        }
        let mut links = Vec::new();
        if let Some(next) = &prev {
            links.push(Link {
                rel: "next_turn".into(),
                to: next.clone(),
            });
        }
        note_aged(
            app_data,
            &Node {
                id: id.clone(),
                kind: "turn".into(),
                ts: now_ts(),
                archive_id: Some(id),
                planet: None,
                card: None,
                links,
            },
        )?;
    }
    for (id, _) in &ctx.open_asks {
        if fitted.open_asks.iter().any(|(k, _)| k == id) {
            continue;
        }
        let mut links = Vec::new();
        if let Some(card) = ctx.card_id.as_ref() {
            if !card.is_empty() {
                links.push(Link {
                    rel: "about_card".into(),
                    to: card.clone(),
                });
            }
        }
        note_aged(
            app_data,
            &Node {
                id: id.clone(),
                kind: "ask".into(),
                ts: now_ts(),
                archive_id: None,
                planet: None,
                card: ctx.card_id.clone(),
                links,
            },
        )?;
    }
    let _ = fitted.card_id;
    Ok(())
}

pub(crate) fn note_aged(app_data: &Path, node: &Node) -> io::Result<()> {
    let existing = read_nodes(app_data);
    let mut fresh = Vec::new();
    if node.links.is_empty() {
        if existing.iter().any(|n| n.id == node.id) {
            return Ok(());
        }
    } else {
        for link in &node.links {
            if !seen(&existing, &node.id, &link.rel, &link.to) {
                fresh.push(link.clone());
            }
        }
        if fresh.is_empty() {
            return Ok(());
        }
    }
    let mut line = node.clone();
    if !node.links.is_empty() {
        line.links = fresh;
    }
    append_line(app_data, &line)
}

pub(crate) fn get(
    app_data: &Path,
    archive: &Path,
    id: &str,
) -> Option<(Node, Option<ArchiveLine>)> {
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    let node = merge_id(app_data, id);
    let arch = by_id(archive, id).or_else(|| {
        node.as_ref()
            .and_then(|n| n.archive_id.as_deref())
            .and_then(|a| by_id(archive, a))
    });
    match (node, arch) {
        (Some(n), arch) => Some((n, arch)),
        (None, Some(line)) => Some((
            Node {
                id: line.id.clone(),
                kind: "turn".into(),
                ts: line.ts.clone(),
                archive_id: Some(line.id.clone()),
                planet: None,
                card: None,
                links: Vec::new(),
            },
            Some(line),
        )),
        (None, None) => None,
    }
}

pub(crate) fn links(app_data: &Path, id: &str, rel: Option<&str>) -> Vec<Link> {
    let id = id.trim();
    if id.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for node in read_nodes(app_data) {
        if node.id == id {
            for link in node.links {
                if rel.is_none_or(|r| link.rel == r) {
                    push_link(&mut out, link);
                }
            }
        } else {
            for link in node.links {
                if link.to == id && rel.is_none_or(|r| link.rel == r) {
                    push_link(
                        &mut out,
                        Link {
                            rel: link.rel,
                            to: node.id.clone(),
                        },
                    );
                }
            }
        }
    }
    out
}

#[cfg(test)]
pub(crate) fn line_count(app_data: &Path) -> usize {
    read_nodes(app_data).len()
}

fn binding_turn(app_data: &Path, ask_id: &str) -> Option<String> {
    for node in read_nodes(app_data) {
        if node.id == ask_id {
            if let Some(link) = node.links.iter().find(|l| l.rel == "asked_in") {
                return Some(link.to.clone());
            }
            if let Some(id) = node.archive_id {
                return Some(id);
            }
        }
        if node.links.iter().any(|l| l.rel == "answers" && l.to == ask_id) {
            return Some(node.id);
        }
    }
    None
}

fn turns_after(archive: &Path, id: &str) -> usize {
    let rows = tail(archive, usize::MAX, None);
    match rows.iter().position(|r| r.id == id) {
        Some(i) => rows.len() - i - 1,
        None => 0,
    }
}

fn last_archive_id(archive: &Path) -> Option<String> {
    tail(archive, 1, None).into_iter().next().map(|r| r.id)
}

fn archive_ids(archive: &Path) -> Vec<(String, String, String)> {
    tail(archive, usize::MAX, None)
        .into_iter()
        .map(|r| (r.role, r.text, r.id))
        .collect()
}

fn seen(nodes: &[Node], id: &str, rel: &str, to: &str) -> bool {
    nodes.iter().any(|n| {
        n.id == id && n.links.iter().any(|l| l.rel == rel && l.to == to)
    })
}

fn merge_id(app_data: &Path, id: &str) -> Option<Node> {
    let mut merged: Option<Node> = None;
    for node in read_nodes(app_data) {
        if node.id != id {
            continue;
        }
        match &mut merged {
            None => merged = Some(node),
            Some(acc) => {
                if acc.kind.is_empty() {
                    acc.kind = node.kind;
                }
                if acc.archive_id.is_none() {
                    acc.archive_id = node.archive_id;
                }
                if acc.planet.is_none() {
                    acc.planet = node.planet;
                }
                if acc.card.is_none() {
                    acc.card = node.card;
                }
                for link in node.links {
                    push_link(&mut acc.links, link);
                }
            }
        }
    }
    merged
}

fn push_link(out: &mut Vec<Link>, link: Link) {
    if out.iter().any(|l| l.rel == link.rel && l.to == link.to) {
        return;
    }
    out.push(link);
}

fn read_nodes(app_data: &Path) -> Vec<Node> {
    let Ok(text) = fs::read_to_string(graph_path(app_data)) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<Node>(line).ok())
        .collect()
}

fn append_line(app_data: &Path, node: &Node) -> io::Result<()> {
    let path = graph_path(app_data);
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let mut body = serde_json::to_string(node)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    body.push('\n');
    let mut opts = OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(&path)?.permissions();
        p.set_mode(0o600);
        let _ = fs::set_permissions(&path, p);
    }
    f.write_all(body.as_bytes())?;
    f.flush()?;
    Ok(())
}

fn now_ts() -> String {
    chrono::Utc::now().to_rfc3339()
}
