//! Rewind and drift over the board history in `history.rs`. Pure parts
//! first: the board at a line, the newest full card, plans, drift. The
//! app parts apply a plan through `state::update` under actor `rewind` or
//! `repair`, so a rewind is itself in the history.

use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use tauri::Emitter;

use super::history::{
    self, Board, Checkpoint, HistoryLine, Ident, View, CARD_FIELDS, KIND_CARD, KIND_CHECKPOINT,
    KIND_PLANET, KIND_TOMBSTONE, PLANET_FIELDS,
};
use crate::state::{KanbanCard, PersistedState, Project};

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

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
    pub field: String,
    pub from: Value,
    pub to: Value,
}

/// A card or planet whose rollback fields change. `title` is the name now.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub id: String,
    pub title: String,
    pub fields: Vec<FieldChange>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Named {
    pub id: String,
    pub title: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub id: String,
    pub title: String,
    pub why: String,
}

/// What a rewind or repair would change. `added` cards stay unless
/// `remove_added`. `planets_added` stay unless `remove_planets`, which only
/// Undo sets, for planets the rewind itself brought back.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// Unix seconds of the point the board goes back to.
    pub at: u64,
    pub cards: Vec<Change>,
    pub revive: Vec<Named>,
    pub added: Vec<Named>,
    pub remove_added: bool,
    pub planets: Vec<Change>,
    pub planets_revive: Vec<Named>,
    pub planets_added: Vec<Named>,
    pub remove_planets: bool,
    pub skipped: Vec<Skipped>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
            && self.revive.is_empty()
            && (!self.remove_added || self.added.is_empty())
            && self.planets.is_empty()
            && self.planets_revive.is_empty()
            && (!self.remove_planets || self.planets_added.is_empty())
    }
}

fn text(view: &View, field: &str) -> String {
    view.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn changes(now: &View, then: &View, names: &[&str]) -> Vec<FieldChange> {
    names
        .iter()
        .filter_map(|f| {
            let from = now.get(*f).cloned().unwrap_or(Value::Null);
            let to = then.get(*f).cloned().unwrap_or(Value::Null);
            (from != to).then(|| FieldChange {
                field: f.to_string(),
                from,
                to,
            })
        })
        .collect()
}

/// Why a card has live work a rewind must not move, if it has any.
pub(crate) fn live_why(now: &PersistedState, c: &KanbanCard) -> Option<&'static str> {
    if now
        .agent_runs
        .iter()
        .any(|r| r.card_id == c.id && r.state != "done")
    {
        return Some("an agent is working on it");
    }
    if c.task_id.is_some() && c.task_reason.is_none() {
        return Some("a task is running");
    }
    if let Some(w) = c.window_id.as_deref() {
        if now.claims.iter().any(|cl| cl.window_id == w) {
            return Some("it is bound to a claimed window");
        }
    }
    None
}

fn first_card<'a>(st: &'a PersistedState, id: &str) -> Option<&'a KanbanCard> {
    st.kanban_cards.iter().find(|c| c.id == id)
}

fn card_change(plan: &mut Plan, now: &PersistedState, c: &KanbanCard, then: &View) {
    let fields = changes(&history::card_view(c), then, CARD_FIELDS);
    if fields.is_empty() {
        return;
    }
    match live_why(now, c) {
        Some(why) => plan.skipped.push(Skipped {
            id: c.id.clone(),
            title: c.title.clone(),
            why: why.into(),
        }),
        None => plan.cards.push(Change {
            id: c.id.clone(),
            title: c.title.clone(),
            fields,
        }),
    }
}

/// Pure. Card `id` back to its view in `target`, or back whole when it is
/// gone now. Nothing else changes.
pub fn plan_card(now: &PersistedState, target: &Board, id: &str) -> Result<Plan, String> {
    let mut plan = Plan::default();
    match (first_card(now, id), target.cards.get(id)) {
        (Some(c), Some(then)) => card_change(&mut plan, now, c, then),
        (None, Some(then)) => plan.revive.push(Named {
            id: id.into(),
            title: text(then, "title"),
        }),
        (Some(_), None) => return Err("That card did not exist at that point.".into()),
        (None, None) => return Err("That card is not on the board or in its history.".into()),
    }
    Ok(plan)
}

/// Pure. Every card and planet back to `target`. Cards added since stay
/// unless `remove_added`; planets added since always stay.
pub fn plan_board(now: &PersistedState, target: &Board, remove_added: bool) -> Plan {
    let disk = history::board_of(&now.kanban_cards, &now.projects);
    let mut plan = Plan {
        remove_added,
        ..Plan::default()
    };
    for (id, then) in &target.cards {
        match first_card(now, id) {
            Some(c) => card_change(&mut plan, now, c, then),
            None => plan.revive.push(Named {
                id: id.clone(),
                title: text(then, "title"),
            }),
        }
    }
    for id in disk.cards.keys().filter(|id| !target.cards.contains_key(*id)) {
        let Some(c) = first_card(now, id) else {
            continue;
        };
        if remove_added {
            if let Some(why) = live_why(now, c) {
                plan.skipped.push(Skipped {
                    id: id.clone(),
                    title: c.title.clone(),
                    why: why.into(),
                });
                continue;
            }
        }
        plan.added.push(Named {
            id: id.clone(),
            title: c.title.clone(),
        });
    }
    for (id, then) in &target.projects {
        match disk.projects.get(id) {
            Some(v) => {
                let fields = changes(v, then, PLANET_FIELDS);
                if !fields.is_empty() {
                    plan.planets.push(Change {
                        id: id.clone(),
                        title: text(v, "name"),
                        fields,
                    });
                }
            }
            None => plan.planets_revive.push(Named {
                id: id.clone(),
                title: text(then, "name"),
            }),
        }
    }
    for (id, v) in &disk.projects {
        if !target.projects.contains_key(id) {
            plan.planets_added.push(Named {
                id: id.clone(),
                title: text(v, "name"),
            });
        }
    }
    plan
}

/// What one rewind's own save wrote: the lines right after its checkpoint
/// with the checkpoint's actor.
#[derive(Default, Debug)]
pub(crate) struct Own {
    sets: BTreeMap<String, BTreeSet<String>>,
    adds: BTreeSet<String>,
    tombs: BTreeSet<String>,
    planet_sets: BTreeMap<String, BTreeSet<String>>,
    planet_adds: BTreeSet<String>,
    planet_rms: BTreeSet<String>,
}

impl Own {
    pub(crate) fn of(after: &[HistoryLine], actor: &str) -> Self {
        let mut own = Own::default();
        for l in after
            .iter()
            .take_while(|l| l.actor == actor && l.kind != KIND_CHECKPOINT)
        {
            let op = l.content.get("op").and_then(Value::as_str);
            let names = || -> BTreeSet<String> {
                l.content
                    .get("fields")
                    .and_then(Value::as_object)
                    .map(|f| f.keys().cloned().collect())
                    .unwrap_or_default()
            };
            match (l.kind, op) {
                (KIND_CARD, Some("set")) => own.sets.entry(l.d.clone()).or_default().extend(names()),
                (KIND_CARD, Some("add")) => {
                    own.adds.insert(l.d.clone());
                }
                (KIND_TOMBSTONE, _) => {
                    own.tombs.insert(l.d.clone());
                }
                (KIND_PLANET, Some("set")) => {
                    own.planet_sets.entry(l.d.clone()).or_default().extend(names())
                }
                (KIND_PLANET, Some("add")) => {
                    own.planet_adds.insert(l.d.clone());
                }
                (KIND_PLANET, Some("rm")) => {
                    own.planet_rms.insert(l.d.clone());
                }
                _ => {}
            }
        }
        own
    }

    /// Keep only what the rewind touched: its fields, the cards and planets
    /// it brought back (removed again), the ones it removed (brought back).
    pub(crate) fn limit(&self, plan: &mut Plan) {
        let keep = |c: &mut Change, sets: &BTreeMap<String, BTreeSet<String>>| match sets.get(&c.id) {
            Some(f) => {
                c.fields.retain(|x| f.contains(&x.field));
                !c.fields.is_empty()
            }
            None => false,
        };
        plan.cards.retain_mut(|c| keep(c, &self.sets));
        plan.planets.retain_mut(|c| keep(c, &self.planet_sets));
        plan.revive.retain(|n| self.tombs.contains(&n.id));
        plan.added.retain(|n| self.adds.contains(&n.id));
        plan.remove_added = true;
        plan.skipped
            .retain(|s| self.sets.contains_key(&s.id) || self.adds.contains(&s.id));
        plan.planets_revive.retain(|n| self.planet_rms.contains(&n.id));
        plan.planets_added.retain(|n| self.planet_adds.contains(&n.id));
        plan.remove_planets = true;
    }
}

const PRE_REWIND: &str = "pre-rewind";
const PRE_REPAIR: &str = "pre-repair";
pub(crate) const KEEP: &str = "keep";

/// The newest checkpoint a rewind or repair wrote before its save.
pub(crate) fn last_rewind(lines: &[HistoryLine]) -> Option<usize> {
    lines.iter().rposition(|l| {
        l.kind == KIND_CHECKPOINT
            && matches!(
                l.content.get("reason").and_then(Value::as_str),
                Some(PRE_REWIND | PRE_REPAIR)
            )
    })
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Req {
    /// The board (or one card, with `card`) back to how it was right after
    /// line `to`. A card's own tombstone means just before it.
    Rewind {
        to: String,
        card: Option<String>,
        remove_added: bool,
    },
    /// Undo the newest rewind or repair.
    Undo,
    /// Disk back to what the history says.
    Repair,
}

impl Req {
    pub(crate) fn actor(&self) -> &'static str {
        match self {
            Req::Repair => "repair",
            _ => "rewind",
        }
    }

    fn reason(&self) -> &'static str {
        match self {
            Req::Repair => PRE_REPAIR,
            _ => PRE_REWIND,
        }
    }
}

pub(crate) struct Resolved {
    pub plan: Plan,
    target: Board,
    revived: Vec<KanbanCard>,
}

fn revive_cards(plan: &mut Plan, cps: &[Checkpoint], upto: &[HistoryLine]) -> Vec<KanbanCard> {
    let mut out = Vec::new();
    let mut lost = Vec::new();
    for n in &plan.revive {
        match full_card(&n.id, cps, upto) {
            Some(c) => out.push(c),
            None => lost.push(n.clone()),
        }
    }
    for n in lost {
        plan.revive.retain(|r| r.id != n.id);
        plan.skipped.push(Skipped {
            id: n.id,
            title: n.title,
            why: "its full card is not in the history".into(),
        });
    }
    out
}

/// Pure over the given files: the plan for `req` against the board `st`.
pub(crate) fn resolve(
    st: &PersistedState,
    support: &Path,
    lines: &[HistoryLine],
    req: &Req,
) -> Result<Resolved, String> {
    let i = match req {
        Req::Rewind { to, card, .. } => {
            let mut i = index_of(lines, to)?;
            if let Some(id) = card {
                if lines[i].kind == KIND_TOMBSTONE && lines[i].d == *id {
                    i = i.checked_sub(1).ok_or_else(|| starts_at(lines))?;
                }
            }
            i
        }
        Req::Undo => last_rewind(lines).ok_or("There is no rewind to undo.")?,
        Req::Repair => lines.len().checked_sub(1).ok_or("There is no history yet.")?,
    };
    let cps = load_anchor(support, lines, i);
    let target = board_at(&cps, lines, &lines[i].id)?;
    let mut plan = match req {
        Req::Rewind {
            card: Some(id), ..
        } => plan_card(st, &target, id)?,
        Req::Rewind { remove_added, .. } => plan_board(st, &target, *remove_added),
        Req::Undo => {
            let mut plan = plan_board(st, &target, true);
            Own::of(&lines[i + 1..], &lines[i].actor).limit(&mut plan);
            plan
        }
        Req::Repair => plan_board(st, &target, false),
    };
    plan.at = lines[i].created_at;
    let revived = revive_cards(&mut plan, &cps, &lines[..=i]);
    Ok(Resolved {
        plan,
        target,
        revived,
    })
}

/// Same rule as `task.rs` touch: strictly above the old stamp, so a stale
/// window's equal-stamp copy never wins `merge_window_save`.
fn touch(c: &mut KanbanCard, now: f64) {
    let prev = c.last_touched_at.min(now + 60.0);
    c.last_touched_at = now.max(prev + 0.001);
}

/// Pure. Sets only the planned rollback fields, brings cards and planets
/// back, removes what the plan removes.
pub(crate) fn apply_to(st: &mut PersistedState, r: &Resolved, now: f64) {
    let plan = &r.plan;
    for ch in &plan.cards {
        let Some(c) = st.kanban_cards.iter_mut().find(|c| c.id == ch.id) else {
            continue;
        };
        let fields: Map<String, Value> = ch
            .fields
            .iter()
            .map(|f| (f.field.clone(), f.to.clone()))
            .collect();
        set_card_fields(c, &fields);
        touch(c, now);
    }
    if plan.remove_added {
        for n in &plan.added {
            if let Some(i) = st.kanban_cards.iter().position(|c| c.id == n.id) {
                let mut gone = st.kanban_cards.remove(i);
                touch(&mut gone, now);
                crate::state::upsert_tombstone(&mut st.card_tombstones, &gone.id, gone.last_touched_at);
            }
        }
    }
    for n in &plan.revive {
        if first_card(st, &n.id).is_some() {
            continue;
        }
        let Some(mut c) = r.revived.iter().find(|c| c.id == n.id).cloned() else {
            continue;
        };
        if let Some(t) = crate::state::tomb_at(&st.card_tombstones, &c.id) {
            c.last_touched_at = c.last_touched_at.max(t);
        }
        st.card_tombstones.retain(|t| t.id != c.id);
        c.window_id = None;
        c.task_id = None;
        c.task_reason = None;
        touch(&mut c, now);
        st.kanban_cards.push(c);
    }
    for ch in &plan.planets {
        let Some(p) = st.projects.iter_mut().find(|p| p.id == ch.id) else {
            continue;
        };
        for f in &ch.fields {
            match f.field.as_str() {
                "name" => p.name = f.to.as_str().unwrap_or_default().to_string(),
                "workspaceRoot" => p.workspace_root = f.to.as_str().map(str::to_string),
                _ => {}
            }
        }
    }
    if plan.remove_planets {
        for n in &plan.planets_added {
            st.projects.retain(|p| p.id != n.id);
        }
    }
    for n in &plan.planets_revive {
        if st.projects.iter().any(|p| p.id == n.id) {
            continue;
        }
        let Some(v) = r.target.projects.get(&n.id) else {
            continue;
        };
        st.projects.push(Project {
            id: n.id.clone(),
            name: text(v, "name"),
            trackers: None,
            workspace_root: v.get("workspaceRoot").and_then(Value::as_str).map(str::to_string),
        });
    }
    crate::todo_trees::reconcile(st);
}

/// Inside the save locks: plan, then the checkpoint, then the change. The
/// caller's save writes the history lines. An empty plan writes nothing.
pub(crate) fn apply_in(
    st: &mut PersistedState,
    support: &Path,
    events: &Path,
    req: &Req,
    ident: Ident,
    now: f64,
) -> Result<Plan, String> {
    let lines = history::read_lines(events)?;
    let r = resolve(st, support, &lines, req)?;
    if r.plan.is_empty() {
        return Ok(r.plan);
    }
    history::write_checkpoint_in(support, events, st, req.actor(), Some(req.reason()), ident)?;
    apply_to(st, &r, now);
    Ok(r.plan)
}

fn support_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let path = crate::state::state_path(app)?;
    path.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "state path has no folder".to_string())
}

/// Dry run under the save locks. Writes nothing.
pub(crate) fn plan(app: &tauri::AppHandle, req: &Req) -> Result<Plan, String> {
    let events = super::event_log::events_path(app)?;
    crate::state::read_locked(app, |path, st| {
        let support = path.parent().ok_or("state path has no folder")?;
        let lines = history::read_lines(&events)?;
        resolve(st, support, &lines, req).map(|r| r.plan)
    })
}

/// A checkpoint, then one `state::update` under actor `rewind` or `repair`,
/// then `state:changed`.
pub(crate) fn apply(app: &tauri::AppHandle, req: &Req) -> Result<Plan, String> {
    let events = super::event_log::events_path(app)?;
    let support = support_dir(app)?;
    let now = crate::state::wall_now_secs();
    let plan = crate::state::with_actor(req.actor(), || {
        crate::state::update(app, |st| {
            let signed = st.signed_locally;
            apply_in(
                st,
                &support,
                &events,
                req,
                &|at, kind, tags, content| {
                    super::event_log::identity_as(app, signed, at, kind, tags, content)
                },
                now,
            )
        })
    })?;
    if !plan.is_empty() {
        let _ = app.emit("state:changed", ());
    }
    Ok(plan)
}

#[cfg(test)]
#[path = "rewind_tests.rs"]
mod tests;
