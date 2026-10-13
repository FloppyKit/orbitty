//! Restorable board history. A save that changes a card or a planet
//! appends lines carrying the NEW values to `events.jsonl`: 423 card
//! add/set, 424 card tombstone, 425 planet, 426 checkpoint. `t=local-only`,
//! never published. A checkpoint file anchors the lines and [`fold`]
//! rebuilds the board from both.
//!
//! Only `KanbanCard` and `Project` values are ever serialized here.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::event_log;
use crate::state::{KanbanCard, PersistedState, Project, Subtask};

pub const KIND_CARD: u64 = 423;
pub const KIND_TOMBSTONE: u64 = 424;
pub const KIND_PLANET: u64 = 425;
pub const KIND_CHECKPOINT: u64 = 426;

/// Card fields a rewind restores. A change to any other field writes no
/// line. Every name needs an arm in `card_same` and `card_value`.
pub const CARD_FIELDS: &[&str] = &[
    "title",
    "status",
    "projectId",
    "kind",
    "notes",
    "difficulty",
    "subtasks",
    "dueAt",
    "tagIds",
    "neededState",
    "neededWhy",
    "neededAnswer",
    "assignedAgentId",
    "parentId",
    "rating",
    "links",
    "completedAt",
    "starred",
    "goalAligned",
    "clinicalOrMaster",
];

/// Planet fields. Trackers are timer noise and are not logged.
pub const PLANET_FIELDS: &[&str] = &["name", "workspaceRoot"];

pub(crate) const HISTORY_DIR: &str = "history";

static WARNED: AtomicBool = AtomicBool::new(false);

fn warn_once(why: &str) {
    if !WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("orbitty: history append failed (further failures silent): {why}");
    }
}

/// The board half of the state before a save.
pub(crate) struct Snapshot {
    cards: Vec<KanbanCard>,
    projects: Vec<Project>,
}

impl Snapshot {
    pub(crate) fn of(st: &PersistedState) -> Self {
        Self {
            cards: st.kanban_cards.clone(),
            projects: st.projects.clone(),
        }
    }
}

/// One line to write: kind, its `d` tag, and the content object.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub kind: u64,
    pub d: String,
    pub content: Value,
}

fn subtasks_eq(a: &Option<Vec<Subtask>>, b: &Option<Vec<Subtask>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y)
                    .all(|(p, q)| p.id == q.id && p.title == q.title && p.done == q.done)
        }
        _ => false,
    }
}

fn card_same(a: &KanbanCard, b: &KanbanCard, field: &str) -> bool {
    match field {
        "title" => a.title == b.title,
        "status" => a.status == b.status,
        "projectId" => a.project_id == b.project_id,
        "kind" => a.kind == b.kind,
        "notes" => a.notes == b.notes,
        "difficulty" => a.difficulty == b.difficulty,
        "subtasks" => subtasks_eq(&a.subtasks, &b.subtasks),
        "dueAt" => a.due_at == b.due_at,
        "tagIds" => a.tag_ids == b.tag_ids,
        "neededState" => a.needed_state == b.needed_state,
        "neededWhy" => a.needed_why == b.needed_why,
        "neededAnswer" => a.needed_answer == b.needed_answer,
        "assignedAgentId" => a.assigned_agent_id == b.assigned_agent_id,
        "parentId" => a.parent_id == b.parent_id,
        "rating" => a.rating == b.rating,
        "links" => a.links == b.links,
        "completedAt" => a.completed_at == b.completed_at,
        "starred" => a.starred == b.starred,
        "goalAligned" => a.goal_aligned == b.goal_aligned,
        "clinicalOrMaster" => a.clinical_or_master == b.clinical_or_master,
        _ => true,
    }
}

fn card_value(c: &KanbanCard, field: &str) -> Value {
    match field {
        "title" => json!(c.title),
        "status" => json!(c.status),
        "projectId" => json!(c.project_id),
        "kind" => json!(c.kind),
        "notes" => json!(c.notes),
        "difficulty" => json!(c.difficulty),
        "subtasks" => json!(c.subtasks),
        "dueAt" => json!(c.due_at),
        "tagIds" => json!(c.tag_ids),
        "neededState" => json!(c.needed_state),
        "neededWhy" => json!(c.needed_why),
        "neededAnswer" => json!(c.needed_answer),
        "assignedAgentId" => json!(c.assigned_agent_id),
        "parentId" => json!(c.parent_id),
        "rating" => json!(c.rating),
        "links" => json!(c.links),
        "completedAt" => json!(c.completed_at),
        "starred" => json!(c.starred),
        "goalAligned" => json!(c.goal_aligned),
        "clinicalOrMaster" => json!(c.clinical_or_master),
        _ => Value::Null,
    }
}

fn planet_value(name: &str, root: &Option<String>, field: &str) -> Value {
    match field {
        "name" => json!(name),
        "workspaceRoot" => json!(root),
        _ => Value::Null,
    }
}

fn planet_fields(p: &Project) -> Map<String, Value> {
    PLANET_FIELDS
        .iter()
        .map(|f| (f.to_string(), planet_value(&p.name, &p.workspace_root, f)))
        .collect()
}

fn first_by_id<'a, T>(items: &'a [T], id: impl Fn(&T) -> &str) -> HashMap<&'a str, &'a T> {
    let mut out = HashMap::with_capacity(items.len());
    for it in items {
        out.entry(id(it)).or_insert(it);
    }
    out
}

/// Lines for one save, in write order: planets added or changed, cards
/// added or changed, cards removed, planets removed. A duplicate id counts
/// once (first wins), the same as [`board_of`].
pub(crate) fn diff(
    before_cards: &[KanbanCard],
    before_projects: &[Project],
    after_cards: &[KanbanCard],
    after_projects: &[Project],
) -> Vec<Entry> {
    let mut out = Vec::new();
    let old_planets = first_by_id(before_projects, |p: &Project| p.id.as_str());
    let mut seen = HashSet::new();
    for p in after_projects {
        if !seen.insert(p.id.as_str()) {
            continue;
        }
        let (op, fields) = match old_planets.get(p.id.as_str()) {
            None => ("add", planet_fields(p)),
            Some(old) => {
                let fields: Map<String, Value> = PLANET_FIELDS
                    .iter()
                    .filter(|f| {
                        planet_value(&old.name, &old.workspace_root, f)
                            != planet_value(&p.name, &p.workspace_root, f)
                    })
                    .map(|f| (f.to_string(), planet_value(&p.name, &p.workspace_root, f)))
                    .collect();
                if fields.is_empty() {
                    continue;
                }
                ("set", fields)
            }
        };
        out.push(Entry {
            kind: KIND_PLANET,
            d: p.id.clone(),
            content: json!({ "v": 1, "op": op, "id": p.id, "fields": fields }),
        });
    }

    let old_cards = first_by_id(before_cards, |c: &KanbanCard| c.id.as_str());
    let mut seen = HashSet::new();
    for c in after_cards {
        if !seen.insert(c.id.as_str()) {
            continue;
        }
        match old_cards.get(c.id.as_str()) {
            None => out.push(Entry {
                kind: KIND_CARD,
                d: c.id.clone(),
                content: json!({ "v": 1, "op": "add", "card": c }),
            }),
            Some(old) => {
                let fields: Map<String, Value> = CARD_FIELDS
                    .iter()
                    .filter(|f| !card_same(old, c, f))
                    .map(|f| (f.to_string(), card_value(c, f)))
                    .collect();
                if !fields.is_empty() {
                    out.push(Entry {
                        kind: KIND_CARD,
                        d: c.id.clone(),
                        content: json!({ "v": 1, "op": "set", "id": c.id, "fields": fields }),
                    });
                }
            }
        }
    }

    let mut gone = HashSet::new();
    for c in before_cards {
        if !seen.contains(c.id.as_str()) && gone.insert(c.id.as_str()) {
            out.push(Entry {
                kind: KIND_TOMBSTONE,
                d: c.id.clone(),
                content: json!({ "v": 1, "id": c.id, "title": c.title }),
            });
        }
    }

    let now_planets: HashSet<&str> = after_projects.iter().map(|p| p.id.as_str()).collect();
    let mut gone = HashSet::new();
    for p in before_projects {
        if !now_planets.contains(p.id.as_str()) && gone.insert(p.id.as_str()) {
            out.push(Entry {
                kind: KIND_PLANET,
                d: p.id.clone(),
                content: json!({ "v": 1, "op": "rm", "id": p.id, "fields": planet_fields(p) }),
            });
        }
    }
    out
}

fn tags_for(d: &str, actor: &str) -> Vec<[String; 2]> {
    vec![
        ["t".into(), "local-only".into()],
        ["d".into(), d.to_string()],
        ["actor".into(), actor.to_string()],
        ["e-type".into(), "history".into()],
    ]
}

/// (created_at, kind, tags, content) → (id, pubkey, sig).
pub(crate) type Ident<'a> = &'a dyn Fn(u64, u64, &[[String; 2]], &str) -> (String, String, String);

/// Each entry through the shared event line writer. Returns the line ids.
pub(crate) fn write_entries(
    events: &Path,
    entries: &[Entry],
    actor: &str,
    ident: Ident,
) -> Result<Vec<String>, String> {
    let created_at = event_log::unix_now();
    let mut ids = Vec::with_capacity(entries.len());
    for e in entries {
        let tags = tags_for(&e.d, actor);
        let content =
            serde_json::to_string(&e.content).map_err(|err| format!("serialize history: {err}"))?;
        let id = ident(created_at, e.kind, &tags, &content);
        ids.push(event_log::write_event(events, created_at, e.kind, tags, content, id)?);
    }
    Ok(ids)
}

/// After a successful save, inside the save locks so lines keep save
/// order. Never fails the save.
pub(crate) fn record_best(app: &tauri::AppHandle, before: &Snapshot, after: &PersistedState) {
    let entries = diff(&before.cards, &before.projects, &after.kanban_cards, &after.projects);
    if entries.is_empty() {
        return;
    }
    let signed = after.signed_locally;
    let actor = crate::state::current_actor();
    let res = event_log::events_path(app).and_then(|events| {
        write_entries(&events, &entries, actor, &|at, kind, tags, content| {
            event_log::identity_as(app, signed, at, kind, tags, content)
        })
    });
    if let Err(e) = res {
        warn_once(&e);
    }
}

#[cfg(test)]
pub(crate) fn unsigned(_: u64, _: u64, _: &[[String; 2]], _: &str) -> (String, String, String) {
    (event_log::uuid_v4_hex(), String::new(), String::new())
}

/// [`record_best`] into a given file with a given signer. Tests only.
#[cfg(test)]
pub(crate) fn record_at(events: &Path, before: &Snapshot, after: &PersistedState, ident: Ident) {
    let entries = diff(&before.cards, &before.projects, &after.kanban_cards, &after.projects);
    if entries.is_empty() {
        return;
    }
    if let Err(e) = write_entries(events, &entries, crate::state::current_actor(), ident) {
        warn_once(&e);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanetRow {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub workspace_root: Option<String>,
}

/// A checkpoint file: the whole board at one moment.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Checkpoint {
    pub v: u32,
    pub at: u64,
    pub projects: Vec<PlanetRow>,
    pub cards: Vec<KanbanCard>,
    /// `history/checkpoint-<unix>.json`, the path its 426 line names.
    #[serde(skip)]
    pub file: String,
}

#[derive(Serialize)]
struct CheckpointOut<'a> {
    v: u32,
    at: u64,
    projects: Vec<PlanetRow>,
    cards: &'a [KanbanCard],
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub(crate) struct CheckpointAck {
    pub file: String,
    pub sha256: String,
    pub cards: usize,
    pub projects: usize,
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn mode(path: &Path, bits: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(bits))
            .map_err(|e| format!("chmod {}: {e}", path.display()))?;
    }
    Ok(())
}

/// Writes `<support>/history/checkpoint-<unix>.json` (dir 0700, file
/// 0600), then its 426 line. The caller holds the save locks.
pub(crate) fn write_checkpoint_in(
    support: &Path,
    events: &Path,
    st: &PersistedState,
    actor: &str,
    ident: Ident,
) -> Result<CheckpointAck, String> {
    let dir = support.join(HISTORY_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    mode(&dir, 0o700)?;
    let at = event_log::unix_now();
    let mut name = format!("checkpoint-{at}.json");
    let mut n = 1;
    while dir.join(&name).exists() {
        name = format!("checkpoint-{at}-{n}.json");
        n += 1;
    }
    let out = CheckpointOut {
        v: 1,
        at,
        projects: st
            .projects
            .iter()
            .map(|p| PlanetRow {
                id: p.id.clone(),
                name: p.name.clone(),
                workspace_root: p.workspace_root.clone(),
            })
            .collect(),
        cards: &st.kanban_cards,
    };
    let text = serde_json::to_string(&out).map_err(|e| format!("serialize checkpoint: {e}"))?;
    let path = dir.join(&name);
    let tmp = dir.join(format!("{name}.tmp"));
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(&tmp)
            .map_err(|e| format!("open {}: {e}", tmp.display()))?;
        f.write_all(text.as_bytes())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
    }
    std::fs::rename(&tmp, &path).map_err(|e| format!("rename to {}: {e}", path.display()))?;
    let ack = CheckpointAck {
        file: format!("{HISTORY_DIR}/{name}"),
        sha256: sha256_hex(text.as_bytes()),
        cards: st.kanban_cards.len(),
        projects: st.projects.len(),
    };
    let entry = Entry {
        kind: KIND_CHECKPOINT,
        d: name.trim_end_matches(".json").to_string(),
        content: json!({
            "v": 1,
            "file": ack.file,
            "sha256": ack.sha256,
            "cards": ack.cards,
            "projects": ack.projects,
        }),
    };
    write_entries(events, &[entry], actor, ident)?;
    Ok(ack)
}

/// `history.checkpoint`, and the boot checkpoint. Holds the `update` locks
/// while it reads, so it agrees with a save.
pub(crate) fn checkpoint(app: &tauri::AppHandle) -> Result<CheckpointAck, String> {
    crate::state::read_locked(app, |path, st| {
        let support = path.parent().ok_or("state path has no folder")?;
        let events = event_log::events_path(app)?;
        let signed = st.signed_locally;
        write_checkpoint_in(
            support,
            &events,
            st,
            crate::state::current_actor(),
            &|at, kind, tags, content| event_log::identity_as(app, signed, at, kind, tags, content),
        )
    })
}

pub(crate) fn has_checkpoint(support: &Path) -> bool {
    std::fs::read_dir(support.join(HISTORY_DIR))
        .map(|rd| {
            rd.flatten().any(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                name.starts_with("checkpoint-") && name.ends_with(".json")
            })
        })
        .unwrap_or(false)
}

/// Once at boot, only when no checkpoint exists yet. Best-effort.
pub(crate) fn boot_checkpoint(app: &tauri::AppHandle) {
    let support = match crate::state::state_path(app) {
        Ok(p) => p.parent().map(Path::to_path_buf),
        Err(e) => {
            warn_once(&e);
            return;
        }
    };
    let Some(support) = support else {
        return;
    };
    if has_checkpoint(&support) {
        return;
    }
    if let Err(e) = checkpoint(app) {
        warn_once(&e);
    }
}

#[allow(dead_code)] // No caller until HISTORY-REWIND.
pub fn load_checkpoint(support: &Path, file: &str) -> Result<Checkpoint, String> {
    let path = support.join(file);
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut cp: Checkpoint =
        serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    cp.file = file.to_string();
    Ok(cp)
}

/// One parsed history line (423–426).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct HistoryLine {
    pub id: String,
    pub created_at: u64,
    pub kind: u64,
    pub d: String,
    pub actor: String,
    pub content: Value,
}

fn tag<'a>(tags: &'a [Value], name: &str) -> Option<&'a str> {
    tags.iter().find_map(|t| {
        let a = t.as_array()?;
        (a.len() >= 2 && a[0].as_str() == Some(name))
            .then(|| a[1].as_str())
            .flatten()
    })
}

/// A 423–426 line tagged `e-type=history`, else None.
pub fn parse_line(raw: &str) -> Option<HistoryLine> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let kind = v.get("kind")?.as_u64()?;
    if !(KIND_CARD..=KIND_CHECKPOINT).contains(&kind) {
        return None;
    }
    let tags = v.get("tags")?.as_array()?;
    if tag(tags, "e-type") != Some("history") {
        return None;
    }
    Some(HistoryLine {
        id: v.get("id")?.as_str()?.to_string(),
        created_at: v.get("created_at")?.as_u64()?,
        kind,
        d: tag(tags, "d").unwrap_or("").to_string(),
        actor: tag(tags, "actor").unwrap_or("").to_string(),
        content: serde_json::from_str(v.get("content")?.as_str()?).ok()?,
    })
}

/// Every history line in file order. A missing file is empty.
pub(crate) fn read_lines(events: &Path) -> Result<Vec<HistoryLine>, String> {
    let raw = match std::fs::read_to_string(events) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("read {}: {e}", events.display())),
    };
    Ok(raw
        .lines()
        .filter(|l| l.contains("\"e-type\",\"history\""))
        .filter_map(parse_line)
        .collect())
}

fn names_project(line: &HistoryLine, project: &str) -> bool {
    let c = &line.content;
    (line.kind == KIND_PLANET && line.d == project)
        || c.pointer("/card/projectId").and_then(Value::as_str) == Some(project)
        || c.pointer("/fields/projectId").and_then(Value::as_str) == Some(project)
}

pub(crate) const LIST_DEFAULT: usize = 50;
pub(crate) const LIST_MAX: usize = 500;

/// Newest first. `card` matches the `d` tag. `project` matches that
/// planet's 425 lines and card lines that put a card on it.
pub(crate) fn list_in(
    events: &Path,
    card: Option<&str>,
    project: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<HistoryLine>, String> {
    let limit = limit.unwrap_or(LIST_DEFAULT).clamp(1, LIST_MAX);
    let mut out: Vec<HistoryLine> = read_lines(events)?
        .into_iter()
        .rev()
        .filter(|l| card.is_none_or(|c| l.d == c))
        .filter(|l| project.is_none_or(|p| names_project(l, p)))
        .take(limit)
        .collect();
    out.shrink_to_fit();
    Ok(out)
}

pub(crate) fn list(
    app: &tauri::AppHandle,
    card: Option<&str>,
    project: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<HistoryLine>, String> {
    list_in(&event_log::events_path(app)?, card, project, limit)
}

pub type View = Map<String, Value>;

/// Planets and cards by id, rollback fields only.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Board {
    pub projects: BTreeMap<String, View>,
    pub cards: BTreeMap<String, View>,
}

pub fn card_view(c: &KanbanCard) -> View {
    CARD_FIELDS
        .iter()
        .map(|f| (f.to_string(), card_value(c, f)))
        .collect()
}

fn row_view(name: &str, root: &Option<String>) -> View {
    PLANET_FIELDS
        .iter()
        .map(|f| (f.to_string(), planet_value(name, root, f)))
        .collect()
}

/// The rollback view of a saved state. First id wins, like [`diff`].
pub fn board_of(cards: &[KanbanCard], projects: &[Project]) -> Board {
    let mut b = Board::default();
    for p in projects {
        b.projects
            .entry(p.id.clone())
            .or_insert_with(|| row_view(&p.name, &p.workspace_root));
    }
    for c in cards {
        b.cards.entry(c.id.clone()).or_insert_with(|| card_view(c));
    }
    b
}

fn merge_fields(view: &mut View, fields: &Map<String, Value>, allowed: &[&str]) {
    for (k, v) in fields {
        if allowed.contains(&k.as_str()) {
            view.insert(k.clone(), v.clone());
        }
    }
}

fn apply(board: &mut Board, line: &HistoryLine) {
    let c = &line.content;
    let id = c.get("id").and_then(Value::as_str);
    let fields = c.get("fields").and_then(Value::as_object);
    match (line.kind, c.get("op").and_then(Value::as_str)) {
        (KIND_CARD, Some("add")) => {
            if let Some(card) = c
                .get("card")
                .and_then(|v| serde_json::from_value::<KanbanCard>(v.clone()).ok())
            {
                board.cards.insert(card.id.clone(), card_view(&card));
            }
        }
        (KIND_CARD, Some("set")) => {
            if let (Some(id), Some(fields)) = (id, fields) {
                if let Some(view) = board.cards.get_mut(id) {
                    merge_fields(view, fields, CARD_FIELDS);
                }
            }
        }
        (KIND_TOMBSTONE, _) => {
            if let Some(id) = id {
                board.cards.remove(id);
            }
        }
        (KIND_PLANET, Some("add")) => {
            if let Some(id) = id {
                let mut view = row_view("", &None);
                if let Some(fields) = fields {
                    merge_fields(&mut view, fields, PLANET_FIELDS);
                }
                board.projects.insert(id.to_string(), view);
            }
        }
        (KIND_PLANET, Some("set")) => {
            if let (Some(id), Some(fields)) = (id, fields) {
                if let Some(view) = board.projects.get_mut(id) {
                    merge_fields(view, fields, PLANET_FIELDS);
                }
            }
        }
        (KIND_PLANET, Some("rm")) => {
            if let Some(id) = id {
                board.projects.remove(id);
            }
        }
        _ => {}
    }
}

/// Pure. The checkpoint's board, then each 423/424/425 in file order after
/// the checkpoint's own 426 line (all lines when that 426 is not in
/// `lines`), stopping after the line whose id is `stop_at`. A `stop_at` at
/// or before the checkpoint line is the checkpoint itself. A tombstoned id
/// stays gone until a later `add` revives it.
#[allow(dead_code)] // No caller until HISTORY-REWIND.
pub fn fold(checkpoint: &Checkpoint, lines: &[HistoryLine], stop_at: Option<&str>) -> Board {
    let mut board = Board::default();
    for p in &checkpoint.projects {
        board
            .projects
            .entry(p.id.clone())
            .or_insert_with(|| row_view(&p.name, &p.workspace_root));
    }
    for c in &checkpoint.cards {
        board.cards.entry(c.id.clone()).or_insert_with(|| card_view(c));
    }
    let start = lines
        .iter()
        .position(|l| {
            l.kind == KIND_CHECKPOINT
                && l.content.get("file").and_then(Value::as_str) == Some(checkpoint.file.as_str())
        })
        .map_or(0, |i| i + 1);
    if stop_at.is_some_and(|stop| lines[..start].iter().any(|l| l.id == stop)) {
        return board;
    }
    for line in &lines[start..] {
        apply(&mut board, line);
        if stop_at == Some(line.id.as_str()) {
            break;
        }
    }
    board
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
