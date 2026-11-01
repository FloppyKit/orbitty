//! Card blocker edges. Nightly-only sidecar
//! `<app data>/board/blockers.json`, shape `{ cardId: [blockerIds] }`.
//! No `state.json` field. History rewind does not read or write this file.
//! A missing sidecar is no blockers. Writes are atomic and mode 0600.
//! Each real write appends one local-only `board-dep` line `{id, blocker, op}`.
//! Flag `card_blockers` defaults off. Off: `link_dep` / `unlink_dep` return
//! one plain error and do not write. Doing doors and the chip then see an
//! empty sidecar. On: the edges enforce.

use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use tauri::{Emitter, Manager};

use crate::state::{KanbanCard, PersistedState};

pub(crate) const MAX_BLOCKERS: usize = 16;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Edges {
    pub(crate) map: BTreeMap<String, Vec<String>>,
}

pub(crate) fn blockers_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    Ok(dir.join("board").join("blockers.json"))
}

/// Flag off drops the sidecar. Callers then see no blockers.
pub(crate) fn edges_if_enabled(enabled: bool, edges: Edges) -> Edges {
    if enabled {
        edges
    } else {
        Edges::default()
    }
}

pub(crate) fn load_app(app: &tauri::AppHandle) -> Edges {
    let enabled = crate::flags::on(app, "card_blockers");
    let loaded = if enabled {
        blockers_path(app)
            .map(|p| load_path(&p))
            .unwrap_or_default()
    } else {
        Edges::default()
    };
    edges_if_enabled(enabled, loaded)
}

fn require_card_blockers(app: &tauri::AppHandle, feature: &str) -> Result<(), String> {
    match crate::flags::blocked(app, "card_blockers", feature) {
        Some(msg) => Err(msg),
        None => Ok(()),
    }
}

pub(crate) fn load_path(path: &Path) -> Edges {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Edges::default();
    };
    parse_edges(&text).unwrap_or_default()
}

fn parse_edges(text: &str) -> Option<Edges> {
    let v: Value = serde_json::from_str(text).ok()?;
    let obj = v.as_object()?;
    let mut map = BTreeMap::new();
    for (k, val) in obj {
        let key = k.trim();
        if key.is_empty() {
            continue;
        }
        let Some(arr) = val.as_array() else {
            continue;
        };
        let mut ids = Vec::new();
        for item in arr {
            let Some(id) = item.as_str().map(str::trim).filter(|s| !s.is_empty()) else {
                continue;
            };
            if !ids.iter().any(|e: &String| e == id) {
                ids.push(id.to_string());
            }
        }
        if !ids.is_empty() {
            map.insert(key.to_string(), ids);
        }
    }
    Some(Edges { map })
}

pub(crate) fn edges_value(edges: &Edges) -> Value {
    let mut obj = serde_json::Map::new();
    for (k, v) in &edges.map {
        obj.insert(k.clone(), json!(v));
    }
    Value::Object(obj)
}

pub(crate) fn save_path(path: &Path, edges: &Edges) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| format!("blockers dir: {e}"))?;
        }
    }
    let text = serde_json::to_string_pretty(&edges_value(edges)).map_err(|e| e.to_string())?;
    let tmp = path.with_file_name(format!("blockers.json.{}.tmp", std::process::id()));
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp).map_err(|e| format!("blockers tmp: {e}"))?;
        f.write_all(text.as_bytes())
            .map_err(|e| format!("blockers write: {e}"))?;
        f.write_all(b"\n")
            .map_err(|e| format!("blockers write: {e}"))?;
        f.sync_all().map_err(|e| format!("blockers sync: {e}"))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("blockers rename: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("blockers mode: {e}"))?;
    }
    Ok(())
}

/// Blocker ids that exist and are not done, in sidecar order.
pub(crate) fn open_blockers(edges: &Edges, cards: &[KanbanCard], id: &str) -> Vec<String> {
    let Some(ids) = edges.map.get(id) else {
        return Vec::new();
    };
    ids.iter()
        .filter(|bid| {
            cards
                .iter()
                .find(|c| c.id == **bid)
                .is_some_and(|c| c.status != "done")
        })
        .cloned()
        .collect()
}

pub(crate) fn card_blocked(edges: &Edges, cards: &[KanbanCard], id: &str) -> bool {
    !open_blockers(edges, cards, id).is_empty()
}

pub(crate) fn open_blocker_titles(edges: &Edges, cards: &[KanbanCard], id: &str) -> Vec<String> {
    open_blockers(edges, cards, id)
        .into_iter()
        .map(|bid| {
            let title = cards
                .iter()
                .find(|c| c.id == bid)
                .map(|c| c.title.trim().to_string());
            match title {
                Some(t) if !t.is_empty() => t,
                _ => bid,
            }
        })
        .collect()
}

/// Only a move into Doing is refused. Todo and done stay open.
pub(crate) fn move_refusal(
    edges: &Edges,
    cards: &[KanbanCard],
    id: &str,
    target: &str,
) -> Option<String> {
    if target != "doing" {
        return None;
    }
    let open = open_blockers(edges, cards, id);
    if open.is_empty() {
        return None;
    }
    Some(format!(
        "kanban.move: blocked by {}; not moving to doing",
        open.join(", ")
    ))
}

/// Adding `id` waits-on `blocker` would loop back to `id`.
pub(crate) fn would_cycle(edges: &Edges, id: &str, blocker: &str) -> bool {
    if id == blocker {
        return true;
    }
    let mut stack = vec![blocker.to_string()];
    let mut seen = HashSet::new();
    while let Some(n) = stack.pop() {
        if n == id {
            return true;
        }
        if !seen.insert(n.clone()) {
            continue;
        }
        if let Some(next) = edges.map.get(&n) {
            stack.extend(next.iter().cloned());
        }
    }
    false
}

fn is_todo(card: &KanbanCard) -> bool {
    card.kind.as_deref() == Some("todo")
}

fn find_card<'a>(cards: &'a [KanbanCard], id: &str) -> Option<&'a KanbanCard> {
    cards.iter().find(|c| c.id == id)
}

/// `Ok(true)` adds the edge. `Ok(false)` is the duplicate no-op.
fn plan_link(edges: &Edges, cards: &[KanbanCard], id: &str, blocker: &str) -> Result<bool, String> {
    if id.is_empty() {
        return Err(format!("kanban.link_dep: unknown card id {id:?}"));
    }
    if find_card(cards, id).is_none() {
        return Err(format!("kanban.link_dep: unknown card id {id:?}"));
    }
    if blocker.is_empty() || find_card(cards, blocker).is_none() {
        return Err(format!("kanban.link_dep: unknown card id {blocker:?}"));
    }
    if id == blocker {
        return Err("kanban.link_dep: a card cannot block itself".into());
    }
    let waiter = find_card(cards, id).expect("waiter");
    let other = find_card(cards, blocker).expect("blocker");
    if !is_todo(waiter) || !is_todo(other) {
        return Err("kanban.link_dep: only a todo card can take a blocker".into());
    }
    if edges
        .map
        .get(id)
        .is_some_and(|list| list.iter().any(|b| b == blocker))
    {
        return Ok(false);
    }
    if would_cycle(edges, id, blocker) {
        return Err("kanban.link_dep: that link would cycle".into());
    }
    let n = edges.map.get(id).map(|list| list.len()).unwrap_or(0);
    if n >= MAX_BLOCKERS {
        return Err(format!(
            "kanban.link_dep: a card holds at most {MAX_BLOCKERS} blockers"
        ));
    }
    Ok(true)
}

fn plan_unlink(
    cards: &[KanbanCard],
    id: &str,
    blocker: &str,
    edges: &Edges,
) -> Result<bool, String> {
    if id.is_empty() || find_card(cards, id).is_none() {
        return Err(format!("kanban.unlink_dep: unknown card id {id:?}"));
    }
    let present = edges
        .map
        .get(id)
        .is_some_and(|list| list.iter().any(|b| b == blocker));
    Ok(present)
}

fn add_edge(edges: &mut Edges, id: &str, blocker: &str) {
    let list = edges.map.entry(id.to_string()).or_default();
    if !list.iter().any(|b| b == blocker) {
        list.push(blocker.to_string());
    }
}

fn remove_edge(edges: &mut Edges, id: &str, blocker: &str) {
    let Some(list) = edges.map.get_mut(id) else {
        return;
    };
    list.retain(|b| b != blocker);
    if list.is_empty() {
        edges.map.remove(id);
    }
}

fn dep_response(card: &KanbanCard, edges: &Edges, cards: &[KanbanCard]) -> Value {
    let open = open_blockers(edges, cards, &card.id);
    let mut v = serde_json::to_value(card).unwrap_or_else(|_| json!({ "id": card.id }));
    v["openBlockers"] = json!(open);
    v["blocked"] = json!(!open.is_empty());
    v
}

fn publish(
    app: &tauri::AppHandle,
    edges: &Edges,
    id: &str,
    blocker: &str,
    op: &str,
    project: Option<&str>,
) {
    crate::event_log::append_best(
        app,
        "board-dep",
        None,
        project,
        Some(crate::state::current_actor()),
        true,
        &json!({ "id": id, "blocker": blocker, "op": op }),
    );
    let _ = app.emit("board:blockers", edges_value(edges));
}

pub fn link_dep(app: &tauri::AppHandle, id: &str, blocker: &str) -> Result<Value, String> {
    require_card_blockers(app, "kanban.link_dep")?;
    let id = id.trim();
    let blocker = blocker.trim();
    let st = crate::state::load_state(app)?;
    let path = blockers_path(app)?;
    let mut edges = load_path(&path);
    if plan_link(&edges, &st.kanban_cards, id, blocker)? {
        add_edge(&mut edges, id, blocker);
        save_path(&path, &edges)?;
        let project = find_card(&st.kanban_cards, id).map(|c| c.project_id.as_str());
        publish(app, &edges, id, blocker, "link", project);
    }
    let card = find_card(&st.kanban_cards, id)
        .cloned()
        .ok_or_else(|| format!("kanban.link_dep: unknown card id {id:?}"))?;
    Ok(dep_response(&card, &edges, &st.kanban_cards))
}

pub fn unlink_dep(app: &tauri::AppHandle, id: &str, blocker: &str) -> Result<Value, String> {
    require_card_blockers(app, "kanban.unlink_dep")?;
    let id = id.trim();
    let blocker = blocker.trim();
    let st = crate::state::load_state(app)?;
    let path = blockers_path(app)?;
    let mut edges = load_path(&path);
    if plan_unlink(&st.kanban_cards, id, blocker, &edges)? {
        remove_edge(&mut edges, id, blocker);
        save_path(&path, &edges)?;
        let project = find_card(&st.kanban_cards, id).map(|c| c.project_id.as_str());
        publish(app, &edges, id, blocker, "unlink", project);
    }
    let card = find_card(&st.kanban_cards, id)
        .cloned()
        .ok_or_else(|| format!("kanban.unlink_dep: unknown card id {id:?}"))?;
    Ok(dep_response(&card, &edges, &st.kanban_cards))
}

/// Cards that were not done and now are, oldest first.
pub(crate) fn newly_done_ids(before: &PersistedState, after: &PersistedState) -> Vec<String> {
    let mut ids: Vec<&KanbanCard> = after
        .kanban_cards
        .iter()
        .filter(|c| {
            c.status == "done"
                && before
                    .kanban_cards
                    .iter()
                    .any(|b| b.id == c.id && b.status != "done")
        })
        .collect();
    ids.sort_by(|a, b| {
        a.created_at
            .partial_cmp(&b.created_at)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    ids.into_iter().map(|c| c.id.clone()).collect()
}

/// Todo dependents of `done_ids` that have zero open blockers on `cards`
/// (the after-state). Oldest `created_at` first. A card already clear of
/// this done set is not listed.
pub(crate) fn ready_after_done(
    edges: &Edges,
    cards: &[KanbanCard],
    done_ids: &[String],
) -> Vec<String> {
    if done_ids.is_empty() {
        return Vec::new();
    }
    let done: HashSet<&str> = done_ids.iter().map(String::as_str).collect();
    let mut ready: Vec<&KanbanCard> = cards
        .iter()
        .filter(|c| {
            c.kind.as_deref() == Some("todo")
                && c.status == "todo"
                && edges
                    .map
                    .get(&c.id)
                    .is_some_and(|bs| bs.iter().any(|b| done.contains(b.as_str())))
                && open_blockers(edges, cards, &c.id).is_empty()
        })
        .collect();
    ready.sort_by(|a, b| {
        a.created_at
            .partial_cmp(&b.created_at)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    ready.into_iter().map(|c| c.id.clone()).collect()
}

/// One `board-ready` line for a not-done → done transition. Reports only.
pub(crate) fn note_ready_on_done(
    app: &tauri::AppHandle,
    before: &PersistedState,
    after: &PersistedState,
) {
    let done_ids = newly_done_ids(before, after);
    if done_ids.is_empty() {
        return;
    }
    let edges = load_app(app);
    let ready_ids = ready_after_done(&edges, &after.kanban_cards, &done_ids);
    let project = done_ids.first().and_then(|id| {
        after
            .kanban_cards
            .iter()
            .find(|c| c.id == *id)
            .map(|c| c.project_id.as_str())
    });
    crate::event_log::append_best(
        app,
        "board-ready",
        None,
        project,
        Some(crate::state::current_actor()),
        true,
        &json!({ "doneIds": done_ids, "readyIds": ready_ids }),
    );
}

#[tauri::command]
pub fn board_blockers(app: tauri::AppHandle) -> Result<Value, String> {
    Ok(edges_value(&load_app(&app)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: &str, status: &str, at: f64) -> KanbanCard {
        crate::board::todo_card(id.into(), id, "p-1", status, "user", None, at)
    }

    fn temp_dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "orbitty-blockers-{}-{}-{}",
            std::process::id(),
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn blockers_absent_loads_empty() {
        let dir = temp_dir("absent");
        let path = dir.join("blockers.json");
        assert!(load_path(&path).map.is_empty(), "missing sidecar");
        std::fs::write(&path, "{}\n").unwrap();
        assert!(load_path(&path).map.is_empty(), "empty object");
        std::fs::write(&path, "not-json").unwrap();
        assert!(load_path(&path).map.is_empty(), "unreadable sidecar");
        let v = serde_json::to_value(card("c", "todo", 1.0)).unwrap();
        assert!(v.get("blockedBy").is_none());
        assert!(v.get("blocked_by").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sidecar_roundtrip_is_0600() {
        let dir = temp_dir("mode");
        let path = dir.join("board").join("blockers.json");
        let mut edges = Edges::default();
        edges.map.insert("c".into(), vec!["a".into(), "b".into()]);
        save_path(&path, &edges).unwrap();
        let loaded = load_path(&path);
        assert_eq!(
            loaded.map.get("c").map(Vec::as_slice),
            Some(["a".to_string(), "b".to_string()].as_slice())
        );
        let tmp = path.with_file_name(format!("blockers.json.{}.tmp", std::process::id()));
        assert!(!tmp.exists(), "tmp file is gone after rename");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_blocker_rules() {
        let cards = vec![
            card("a", "todo", 1.0),
            card("b", "doing", 2.0),
            card("d", "done", 3.0),
            card("c", "todo", 4.0),
        ];
        let mut edges = Edges::default();
        edges.map.insert(
            "c".into(),
            vec!["a".into(), "b".into(), "d".into(), "missing".into()],
        );
        assert_eq!(
            open_blockers(&edges, &cards, "c"),
            vec!["a".to_string(), "b".to_string()]
        );
        assert!(card_blocked(&edges, &cards, "c"));
        edges
            .map
            .insert("c".into(), vec!["d".into(), "missing".into()]);
        assert!(open_blockers(&edges, &cards, "c").is_empty());
        assert!(!card_blocked(&edges, &cards, "c"));
        assert!(open_blockers(&edges, &cards, "nobody").is_empty());
    }

    #[test]
    fn link_dep_refusals() {
        let mut needed = card("n", "todo", 1.0);
        needed.kind = Some("needed".into());
        let cards = vec![
            card("a", "todo", 1.0),
            card("b", "todo", 2.0),
            card("c", "todo", 3.0),
            needed,
        ];
        let edges = Edges::default();
        let self_err = plan_link(&edges, &cards, "a", "a").unwrap_err();
        assert!(self_err.contains("itself"), "{self_err}");
        let unknown = plan_link(&edges, &cards, "a", "nope").unwrap_err();
        assert!(unknown.contains("unknown"), "{unknown}");
        let missing_waiter = plan_link(&edges, &cards, "nope", "a").unwrap_err();
        assert!(missing_waiter.contains("unknown"), "{missing_waiter}");
        let kind = plan_link(&edges, &cards, "n", "a").unwrap_err();
        assert!(kind.contains("todo"), "{kind}");
        let kind_blocker = plan_link(&edges, &cards, "a", "n").unwrap_err();
        assert!(kind_blocker.contains("todo"), "{kind_blocker}");

        let mut ab = Edges::default();
        add_edge(&mut ab, "a", "b");
        let cycle = plan_link(&ab, &cards, "b", "a").unwrap_err();
        assert!(cycle.contains("cycle"), "{cycle}");
        assert!(would_cycle(&ab, "b", "a"));

        let mut chain = Edges::default();
        add_edge(&mut chain, "a", "b");
        add_edge(&mut chain, "b", "c");
        let long = plan_link(&chain, &cards, "c", "a").unwrap_err();
        assert!(long.contains("cycle"), "{long}");

        let mut full = Edges::default();
        let mut many = cards.clone();
        for i in 0..MAX_BLOCKERS {
            let id = format!("x{i}");
            many.push(card(&id, "todo", 10.0 + i as f64));
            add_edge(&mut full, "a", &id);
        }
        let extra = card("x16", "todo", 40.0);
        many.push(extra);
        let cap = plan_link(&full, &many, "a", "x16").unwrap_err();
        assert!(cap.contains("16"), "{cap}");
    }

    #[test]
    fn link_dep_dup_noop() {
        let cards = vec![card("a", "todo", 1.0), card("b", "todo", 2.0)];
        let mut edges = Edges::default();
        assert!(plan_link(&edges, &cards, "a", "b").unwrap());
        add_edge(&mut edges, "a", "b");
        assert!(!plan_link(&edges, &cards, "a", "b").unwrap());
        assert!(!plan_unlink(&cards, "a", "missing", &edges).unwrap());
        assert!(plan_unlink(&cards, "a", "b", &edges).unwrap());
        let gone = plan_unlink(&cards, "nope", "b", &edges).unwrap_err();
        assert!(gone.contains("unknown"), "{gone}");
    }

    #[test]
    fn link_onto_doing_is_allowed() {
        let doing = card("d", "doing", 1.0);
        let blocker = card("b", "todo", 2.0);
        let cards = vec![doing.clone(), blocker];
        let mut edges = Edges::default();
        assert!(plan_link(&edges, &cards, "d", "b").unwrap());
        add_edge(&mut edges, "d", "b");
        let v = dep_response(&doing, &edges, &cards);
        assert_eq!(v["blocked"], json!(true));
        assert_eq!(v["status"], "doing");
        assert_eq!(v["openBlockers"][0], "b");
    }

    #[test]
    fn move_refusal_only_for_doing() {
        let cards = vec![
            card("a", "todo", 1.0),
            card("c", "todo", 2.0),
            card("d", "done", 3.0),
        ];
        let mut edges = Edges::default();
        edges.map.insert("c".into(), vec!["a".into()]);
        let doing = move_refusal(&edges, &cards, "c", "doing").unwrap();
        assert!(doing.contains("not moving to doing"), "{doing}");
        assert!(move_refusal(&edges, &cards, "c", "todo").is_none());
        assert!(move_refusal(&edges, &cards, "c", "done").is_none());
        edges
            .map
            .insert("c".into(), vec!["d".into(), "missing".into()]);
        assert!(move_refusal(&edges, &cards, "c", "doing").is_none());

        let src = include_str!("board.rs");
        let start = src.find("fn kanban_move_full(").expect("kanban_move_full");
        let body = &src[start..];
        let end = body.find("\n}\n").expect("end");
        let body = &body[..end];
        let gate = body.find("move_refusal(").expect("doing gate");
        let save = body.find("save_with_events(").expect("save");
        assert!(gate < save, "the doing gate runs before the save");
    }

    #[test]
    fn ready_after_done_lists_only_clear_todo_dependents() {
        let mut a = card("a", "done", 1.0);
        a.title = "Alpha".into();
        let mut z = card("z", "todo", 0.5);
        z.title = "Still open".into();
        let mut already = card("old", "done", 0.2);
        already.title = "Already".into();
        let c1 = card("c1", "todo", 2.0);
        let c2 = card("c2", "todo", 4.0);
        let still = card("still", "todo", 3.0);
        let running = card("run", "doing", 1.5);
        let finished = card("fin", "done", 5.0);
        let mut needed = card("need", "todo", 6.0);
        needed.kind = Some("needed".into());
        let last = card("last", "todo", 8.0);
        let cards = vec![
            a,
            z,
            already.clone(),
            c1,
            c2,
            still,
            running,
            finished,
            needed,
            last,
        ];
        let mut edges = Edges::default();
        edges.map.insert("c1".into(), vec!["a".into()]);
        edges.map.insert("c2".into(), vec!["a".into()]);
        edges
            .map
            .insert("still".into(), vec!["a".into(), "z".into()]);
        edges.map.insert("run".into(), vec!["a".into()]);
        edges.map.insert("fin".into(), vec!["a".into()]);
        edges.map.insert("need".into(), vec!["a".into()]);
        edges
            .map
            .insert("last".into(), vec!["a".into(), "old".into()]);
        edges.map.insert("clear".into(), vec!["old".into()]);
        let ready = ready_after_done(&edges, &cards, &["a".into()]);
        assert_eq!(
            ready,
            vec!["c1".to_string(), "c2".to_string(), "last".to_string()]
        );

        let again = ready_after_done(&edges, &cards, &["zzz".into()]);
        assert!(
            again.is_empty(),
            "an unrelated done does not relist {again:?}"
        );

        let mut before = PersistedState::default();
        before.kanban_cards = vec![card("a", "todo", 1.0), card("c1", "todo", 2.0)];
        let mut after = before.clone();
        after.kanban_cards[0].status = "done".into();
        assert_eq!(newly_done_ids(&before, &after), vec!["a".to_string()]);
        let before2 = after.clone();
        let mut after2 = before2.clone();
        after2.kanban_cards[1].title = "touched".into();
        assert!(newly_done_ids(&before2, &after2).is_empty());
    }

    #[test]
    fn ready_hook_never_launches() {
        let src = include_str!("blockers.rs");
        let start = src.find("pub(crate) fn note_ready_on_done(").expect("hook");
        let body = &src[start..];
        let end = body.find("\n}\n").expect("end");
        let body = &body[..end];
        for bad in ["agent_pickup", "kanban_move", "launch_agent", "shift::tick"] {
            assert!(!body.contains(bad), "{bad} in {body}");
        }
        let call = body.find("ready_after_done(").expect("after-state");
        assert!(body[call..].contains("&after.kanban_cards"));
        assert!(!body.contains("&before.kanban_cards"));
        let spawn = include_str!("../agents/doing_fire.rs");
        assert!(spawn.contains("note_ready_on_done("));
    }

    fn fn_body<'a>(src: &'a str, sig: &str) -> &'a str {
        let start = src.find(sig).unwrap_or_else(|| panic!("{sig} missing"));
        let rest = &src[start..];
        let end = rest.find("\n}\n").unwrap_or(rest.len());
        &rest[..end]
    }

    /// Q1=B: one new row, default off. Existing defaults stay put.
    #[test]
    fn card_blockers_defaults_off() {
        let prior = [
            ("tabbity_bridge", true),
            ("go_start", true),
            ("event_seq", false),
            ("ax_safe", true),
            ("microtags", false),
            ("mh_archive", false),
            ("shim_ledger", false),
            ("memory_ram", false),
            ("herdr_core", true),
            ("wake_core", false),
            ("herdr_primary", false),
        ];
        for (id, default) in prior {
            let def = crate::flags::def_by_id(id).unwrap_or_else(|| panic!("missing {id}"));
            assert_eq!(def.default, default, "{id}");
        }
        let row = crate::flags::def_by_id("card_blockers").expect("card_blockers row");
        assert!(!row.default, "card_blockers default off");
        assert_eq!(row.go, "GO-ORBITTY-CARD-BLOCKERS");
        let (on, source) = crate::flags::resolve(row, None, &crate::flags::FileFlags::empty());
        assert!(!on);
        assert_eq!(source, crate::flags::Source::Default);
        let (on, source) =
            crate::flags::resolve(row, Some("on"), &crate::flags::FileFlags::empty());
        assert!(on);
        assert_eq!(source, crate::flags::Source::Env);
        let (off, _) = crate::flags::resolve(row, Some("0"), &crate::flags::FileFlags::empty());
        assert!(!off);

        let flags = include_str!("../flags.rs");
        let at = flags.find("id: \"card_blockers\"").expect("row text");
        let window = &flags[at..at + 280];
        assert!(window.contains("default: false"), "{window}");
        assert!(!window.contains("default: true"), "{window}");
    }

    /// Flag off: verbs refuse in one line and do not write. Doors see no edges.
    #[test]
    fn flag_off_refuses_verbs_and_hides_edges() {
        let link_err = crate::flags::refusal("kanban.link_dep", "card_blockers");
        let unlink_err = crate::flags::refusal("kanban.unlink_dep", "card_blockers");
        assert_eq!(
            link_err,
            "kanban.link_dep is off (flag card_blockers). orbittyctl flags set card_blockers on"
        );
        assert_eq!(
            unlink_err,
            "kanban.unlink_dep is off (flag card_blockers). orbittyctl flags set card_blockers on"
        );
        assert!(!link_err.contains('\n'));
        assert!(!unlink_err.contains('\n'));

        let cards = vec![card("a", "todo", 1.0), card("c", "todo", 2.0)];
        let mut edges = Edges::default();
        edges.map.insert("c".into(), vec!["a".into()]);
        let hidden = edges_if_enabled(false, edges.clone());
        assert!(hidden.map.is_empty());
        assert!(!card_blocked(&hidden, &cards, "c"));
        assert!(move_refusal(&hidden, &cards, "c", "doing").is_none());
        assert!(ready_after_done(&hidden, &cards, &["a".into()]).is_empty());
        let shown = edges_if_enabled(true, edges);
        let doing = move_refusal(&shown, &cards, "c", "doing").unwrap();
        assert!(doing.contains("not moving to doing"), "{doing}");
        assert!(card_blocked(&shown, &cards, "c"));

        let src = include_str!("blockers.rs");
        let load = fn_body(src, "pub(crate) fn load_app(");
        let flag_at = load.find("card_blockers").expect("load gate");
        let read_at = load.find("load_path(").expect("read");
        assert!(flag_at < read_at, "flag check sits above the sidecar read");
        assert!(load.contains("edges_if_enabled("));
        let gate = fn_body(src, "fn require_card_blockers(");
        assert!(gate.contains("flags::blocked(app, \"card_blockers\", feature)"));
        for (sig, feature) in [
            ("pub fn link_dep(", "kanban.link_dep"),
            ("pub fn unlink_dep(", "kanban.unlink_dep"),
        ] {
            let body = fn_body(src, sig);
            let refuse = body.find("require_card_blockers(").expect(sig);
            let save = body.find("save_path(").expect("save");
            assert!(refuse < save, "{feature} must refuse before write");
            assert!(
                body[refuse..save].contains(feature),
                "{feature} missing in the gate call"
            );
            assert!(body.find("load_path(").unwrap() > refuse);
        }
        let chip = fn_body(src, "pub fn board_blockers(");
        assert!(chip.contains("load_app("));
        for door in [
            include_str!("board.rs"),
            include_str!("../agents/agents.rs"),
            include_str!("../agents/shift.rs"),
            include_str!("../agents/doing_fire.rs"),
        ] {
            assert!(door.contains("blockers::load_app"));
            assert!(
                !door.contains("blockers::load_path"),
                "doors must not read the sidecar around the flag"
            );
        }
    }
}
