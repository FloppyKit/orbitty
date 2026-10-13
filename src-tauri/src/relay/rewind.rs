//! Rewind and drift over the board history in `history.rs`. Pure parts
//! first: the board at a line, the newest full card, plans, drift. The
//! app parts apply a plan through `state::update` under actor `rewind` or
//! `repair`, so a rewind is itself in the history.

use serde_json::{Map, Value};
use std::path::Path;

use super::history::{self, Board, Checkpoint, HistoryLine, KIND_CARD, KIND_CHECKPOINT};
use crate::state::KanbanCard;

/// At launch, a new checkpoint once more lines than this follow the newest.
pub(crate) const AUTO_CHECKPOINT_LINES: usize = 2_000;

pub(crate) fn local_time(secs: u64) -> String {
    chrono::DateTime::from_timestamp(secs as i64, 0)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| secs.to_string())
}

pub(crate) fn index_of(lines: &[HistoryLine], at: &str) -> Result<usize, String> {
    lines
        .iter()
        .position(|l| l.id == at)
        .ok_or_else(|| "That point is not in the history.".to_string())
}

fn checkpoint_file(line: &HistoryLine) -> Option<&str> {
    (line.kind == KIND_CHECKPOINT)
        .then(|| line.content.get("file").and_then(Value::as_str))
        .flatten()
}

/// The newest 426 line at or before `upto`, with its file.
pub(crate) fn anchor_line(lines: &[HistoryLine], upto: usize) -> Option<(usize, &str)> {
    lines[..=upto.min(lines.len().saturating_sub(1))]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, l)| checkpoint_file(l).map(|f| (i, f)))
}

fn starts_at(lines: &[HistoryLine]) -> String {
    match lines.iter().find(|l| l.kind == KIND_CHECKPOINT) {
        Some(l) => format!("History starts at {}.", local_time(l.created_at)),
        None => "History has no checkpoint yet.".into(),
    }
}

/// Pure. The newest checkpoint whose 426 line is at or before line `at`,
/// folded to `at`.
pub fn board_at(checkpoints: &[Checkpoint], lines: &[HistoryLine], at: &str) -> Result<Board, String> {
    let i = index_of(lines, at)?;
    let cp = lines[..=i]
        .iter()
        .rev()
        .filter_map(checkpoint_file)
        .find_map(|f| checkpoints.iter().find(|c| c.file == f))
        .ok_or_else(|| starts_at(lines))?;
    Ok(history::fold(cp, lines, Some(at)))
}

/// [`board_at`] for the newest line.
pub fn board_now(checkpoints: &[Checkpoint], lines: &[HistoryLine]) -> Result<Board, String> {
    let last = lines.last().ok_or("There is no history yet.")?;
    board_at(checkpoints, lines, &last.id)
}

/// Rollback fields from a line merged onto a whole card. A null clears an
/// optional field. Unknown names are ignored.
pub(crate) fn set_card_fields(card: &mut KanbanCard, fields: &Map<String, Value>) {
    let Ok(Value::Object(mut obj)) = serde_json::to_value(&*card) else {
        return;
    };
    for (k, v) in fields {
        if !history::CARD_FIELDS.contains(&k.as_str()) {
            continue;
        }
        let mut trial = obj.clone();
        if v.is_null() {
            trial.remove(k);
        } else {
            trial.insert(k.clone(), v.clone());
        }
        if serde_json::from_value::<KanbanCard>(Value::Object(trial.clone())).is_ok() {
            obj = trial;
        }
    }
    if let Ok(c) = serde_json::from_value(Value::Object(obj)) {
        *card = c;
    }
}

/// Pure. The newest whole JSON of card `id` in `lines` (a checkpoint that
/// holds it, or its 423 add, whichever is later), then every 423 set for
/// it after that. Pass lines up to the point you want. A card whose last
/// line is a tombstone comes back as it was just before it.
pub fn full_card(id: &str, checkpoints: &[Checkpoint], lines: &[HistoryLine]) -> Option<KanbanCard> {
    let mut src: Option<(usize, KanbanCard)> = None;
    for (i, l) in lines.iter().enumerate().rev() {
        if l.kind == KIND_CARD
            && l.content.get("op").and_then(Value::as_str) == Some("add")
            && l.content.pointer("/card/id").and_then(Value::as_str) == Some(id)
        {
            if let Ok(c) = serde_json::from_value(l.content["card"].clone()) {
                src = Some((i, c));
                break;
            }
        }
        if let Some(f) = checkpoint_file(l) {
            let held = checkpoints
                .iter()
                .find(|c| c.file == f)
                .and_then(|c| c.cards.iter().find(|c| c.id == id));
            if let Some(c) = held {
                src = Some((i, c.clone()));
                break;
            }
        }
    }
    let (from, mut card) = src?;
    for l in &lines[from + 1..] {
        if l.kind == KIND_CARD
            && l.content.get("op").and_then(Value::as_str) == Some("set")
            && l.content.get("id").and_then(Value::as_str) == Some(id)
        {
            if let Some(fields) = l.content.get("fields").and_then(Value::as_object) {
                set_card_fields(&mut card, fields);
            }
        }
    }
    Some(card)
}

/// Lines after the newest 426 line (all lines when there is none).
pub(crate) fn lines_after_checkpoint(lines: &[HistoryLine]) -> usize {
    match lines.iter().rposition(|l| l.kind == KIND_CHECKPOINT) {
        Some(i) => lines.len() - i - 1,
        None => lines.len(),
    }
}

pub(crate) fn needs_checkpoint(lines: &[HistoryLine]) -> bool {
    lines_after_checkpoint(lines) > AUTO_CHECKPOINT_LINES
}

/// History lines from the newest 426 line on, parsing only those. Drift
/// and `board_now` need no more.
pub(crate) fn read_since_checkpoint(events: &Path) -> Result<Vec<HistoryLine>, String> {
    let raw = match std::fs::read_to_string(events) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("read {}: {e}", events.display())),
    };
    let rows: Vec<&str> = raw.lines().collect();
    let from = rows
        .iter()
        .rposition(|l| {
            l.contains("\"kind\":426")
                && l.contains("\"e-type\",\"history\"")
                && history::parse_line(l).is_some_and(|h| h.kind == KIND_CHECKPOINT)
        })
        .unwrap_or(0);
    Ok(rows[from..]
        .iter()
        .filter(|l| l.contains("\"e-type\",\"history\""))
        .filter_map(|l| history::parse_line(l))
        .collect())
}

/// The checkpoints the given lines name, loaded from `support`. A file
/// that is gone or unreadable is skipped.
pub(crate) fn load_named(support: &Path, lines: &[HistoryLine]) -> Vec<Checkpoint> {
    lines
        .iter()
        .filter_map(checkpoint_file)
        .filter_map(|f| history::load_checkpoint(support, f).ok())
        .collect()
}

/// Only the checkpoint [`board_at`] would pick for line index `upto`.
pub(crate) fn load_anchor(support: &Path, lines: &[HistoryLine], upto: usize) -> Vec<Checkpoint> {
    if lines.is_empty() {
        return Vec::new();
    }
    anchor_line(lines, upto)
        .and_then(|(_, f)| history::load_checkpoint(support, f).ok())
        .into_iter()
        .collect()
}

#[cfg(test)]
#[path = "rewind_tests.rs"]
mod tests;
