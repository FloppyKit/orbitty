use super::*;
use crate::state::{
    load_path, save_path, update_at_logged, with_actor, write_lock, CardLink, PersistedState,
    Project, Subtask, Tracker,
};
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "orbitty-history-{tag}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn card(id: &str, title: &str, project: &str) -> KanbanCard {
    crate::board::todo_card(id.into(), title, project, "todo", "user", None, 1.0)
}

fn planet(id: &str, name: &str) -> Project {
    Project {
        id: id.into(),
        name: name.into(),
        trackers: None,
        workspace_root: None,
    }
}

fn one(entries: &[Entry]) -> &Entry {
    assert_eq!(entries.len(), 1, "{entries:#?}");
    &entries[0]
}

#[test]
fn rollback_fields_are_exactly_the_go_list() {
    assert_eq!(
        CARD_FIELDS,
        &[
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
        ]
    );
    assert_eq!(PLANET_FIELDS, &["name", "workspaceRoot"]);
}

/// One edit per rollback field, named by hand (not from CARD_FIELDS), so
/// dropping a name from the list fails here.
#[test]
fn each_rollback_field_change_writes_a_set_line_with_the_new_value() {
    type Edit = fn(&mut KanbanCard);
    let edits: &[(&str, Edit, Value)] = &[
        ("title", |c| c.title = "New title".into(), json!("New title")),
        ("status", |c| c.status = "doing".into(), json!("doing")),
        ("projectId", |c| c.project_id = "p-2".into(), json!("p-2")),
        ("kind", |c| c.kind = Some("needed".into()), json!("needed")),
        ("notes", |c| c.notes = Some("typed".into()), json!("typed")),
        ("difficulty", |c| c.difficulty = Some("hard".into()), json!("hard")),
        (
            "subtasks",
            |c| {
                c.subtasks = Some(vec![Subtask {
                    id: "s1".into(),
                    title: "step".into(),
                    done: true,
                }])
            },
            json!([{ "id": "s1", "title": "step", "done": true }]),
        ),
        ("dueAt", |c| c.due_at = Some(1_800_000_000.0), json!(1_800_000_000.0)),
        ("tagIds", |c| c.tag_ids = Some(vec!["t-1".into()]), json!(["t-1"])),
        ("neededState", |c| c.needed_state = Some("answered".into()), json!("answered")),
        ("neededWhy", |c| c.needed_why = Some("because".into()), json!("because")),
        ("neededAnswer", |c| c.needed_answer = Some("yes, ship it".into()), json!("yes, ship it")),
        ("assignedAgentId", |c| c.assigned_agent_id = Some("a-1".into()), json!("a-1")),
        ("parentId", |c| c.parent_id = Some("c-parent".into()), json!("c-parent")),
        ("rating", |c| c.rating = Some(6), json!(6)),
        (
            "links",
            |c| {
                c.links = Some(vec![CardLink {
                    url: "https://example.com/x".into(),
                    label: None,
                }])
            },
            json!([{ "url": "https://example.com/x" }]),
        ),
        ("completedAt", |c| c.completed_at = Some(5.0), json!(5.0)),
        ("starred", |c| c.starred = true, json!(true)),
        ("goalAligned", |c| c.goal_aligned = false, json!(false)),
        ("clinicalOrMaster", |c| c.clinical_or_master = true, json!(true)),
    ];
    assert_eq!(edits.len(), 20);
    let base = card("c-1", "Old", "p-1");
    for (field, edit, want) in edits {
        let mut after = base.clone();
        edit(&mut after);
        let entries = diff(&[base.clone()], &[], &[after], &[]);
        let e = one(&entries);
        assert_eq!(e.kind, KIND_CARD, "{field}");
        assert_eq!(e.d, "c-1");
        assert_eq!(
            e.content,
            json!({ "v": 1, "op": "set", "id": "c-1", "fields": { *field: want } }),
            "{field}"
        );
    }
}

#[test]
fn clearing_a_field_logs_null() {
    let mut before = card("c-1", "T", "p-1");
    before.notes = Some("old".into());
    let mut after = before.clone();
    after.notes = None;
    let entries = diff(&[before], &[], &[after], &[]);
    assert_eq!(one(&entries).content["fields"], json!({ "notes": null }));
}

#[test]
fn non_rollback_changes_write_no_line() {
    let base = card("c-1", "T", "p-1");
    let mut after = base.clone();
    after.last_touched_at = 99.0;
    after.window_id = Some("w-1".into());
    after.points_awarded = 3;
    after.actor = Some("agent".into());
    after.trackers = Some(vec![]);
    after.task_id = Some("task".into());
    after.task_reason = Some("why".into());
    after.created_at = 42.0;
    assert!(diff(&[base.clone()], &[], &[after], &[]).is_empty());

    let mut p = planet("p-1", "Home");
    let before_p = p.clone();
    p.trackers = Some(vec![Tracker {
        id: "tr".into(),
        label: "Timer".into(),
        value: 9,
        kind: Some("timer".into()),
        running_since: Some(1.0),
        history: None,
    }]);
    assert!(diff(&[], &[before_p], &[], &[p]).is_empty());
}

#[test]
fn add_carries_the_whole_card_and_delete_is_a_tombstone() {
    let mut c = card("c-new", "Fresh", "p-1");
    c.notes = Some("first note".into());
    let added = diff(&[], &[], &[c.clone()], &[]);
    let e = one(&added);
    assert_eq!(e.kind, KIND_CARD);
    assert_eq!(e.content["op"], "add");
    assert_eq!(e.content["card"], serde_json::to_value(&c).unwrap());

    let gone = diff(&[c], &[], &[], &[]);
    let e = one(&gone);
    assert_eq!(e.kind, KIND_TOMBSTONE);
    assert_eq!(e.d, "c-new");
    assert_eq!(e.content, json!({ "v": 1, "id": "c-new", "title": "Fresh" }));
}

#[test]
fn planet_add_rename_root_and_rm() {
    let p = planet("p-9", "Mars");
    let e = diff(&[], &[], &[], &[p.clone()]);
    assert_eq!(
        one(&e).content,
        json!({ "v": 1, "op": "add", "id": "p-9", "fields": { "name": "Mars", "workspaceRoot": null } })
    );
    let mut renamed = p.clone();
    renamed.name = "Red".into();
    let e = diff(&[], &[p.clone()], &[], &[renamed.clone()]);
    assert_eq!(
        one(&e).content,
        json!({ "v": 1, "op": "set", "id": "p-9", "fields": { "name": "Red" } })
    );
    assert_eq!(one(&e).kind, KIND_PLANET);
    let mut rooted = renamed.clone();
    rooted.workspace_root = Some("/tmp/red".into());
    let e = diff(&[], &[renamed.clone()], &[], &[rooted.clone()]);
    assert_eq!(one(&e).content["fields"], json!({ "workspaceRoot": "/tmp/red" }));
    let e = diff(&[], &[rooted], &[], &[]);
    assert_eq!(one(&e).content["op"], "rm");
    assert_eq!(one(&e).d, "p-9");
}

#[test]
fn every_rollback_field_has_a_value_arm() {
    let mut c = card("c-1", "T", "p-1");
    c.notes = Some("n".into());
    c.difficulty = Some("easy".into());
    c.subtasks = Some(vec![]);
    c.due_at = Some(1.0);
    c.tag_ids = Some(vec![]);
    c.needed_state = Some("open".into());
    c.needed_why = Some("w".into());
    c.needed_answer = Some("a".into());
    c.assigned_agent_id = Some("a".into());
    c.parent_id = Some("p".into());
    c.rating = Some(1);
    c.links = Some(vec![]);
    c.completed_at = Some(1.0);
    for f in CARD_FIELDS {
        assert!(!card_value(&c, f).is_null(), "{f} has no card_value arm");
        assert!(card_same(&c, &c, f), "{f}");
    }
}

struct Run {
    dir: PathBuf,
    state: PathBuf,
    events: PathBuf,
}

impl Run {
    fn new(tag: &str, initial: &PersistedState) -> Self {
        let dir = temp_dir(tag);
        let state = dir.join("state.json");
        let events = dir.join("events.jsonl");
        save_path(&state, initial).unwrap();
        Self { dir, state, events }
    }

    fn step(&self, f: impl FnOnce(&mut PersistedState)) {
        let _g = write_lock();
        update_at_logged(&self.state, &self.events, |st| {
            f(st);
            Ok(())
        })
        .unwrap();
    }

    fn checkpoint(&self) -> CheckpointAck {
        let st = load_path(&self.state).unwrap();
        write_checkpoint_in(&self.dir, &self.events, &st, "app", &unsigned).unwrap()
    }

    fn board(&self) -> Board {
        let st = load_path(&self.state).unwrap();
        board_of(&st.kanban_cards, &st.projects)
    }

    fn lines(&self) -> Vec<HistoryLine> {
        read_lines(&self.events).unwrap()
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn card_mut<'a>(st: &'a mut PersistedState, id: &str) -> &'a mut KanbanCard {
    st.kanban_cards.iter_mut().find(|c| c.id == id).unwrap()
}

fn start_state() -> PersistedState {
    let mut st = PersistedState::default();
    st.projects.push(planet("p-1", "Home"));
    st.kanban_cards.push(card("c-old", "Already here", "p-1"));
    st
}

type Step = Box<dyn Fn(&mut PersistedState)>;

fn script() -> Vec<Step> {
    vec![
        Box::new(|st| st.kanban_cards.push(card("c-a", "Draft", "p-1"))),
        Box::new(|st| card_mut(st, "c-a").title = "Renamed".into()),
        Box::new(|st| card_mut(st, "c-a").status = "doing".into()),
        Box::new(|st| {
            let c = card_mut(st, "c-a");
            c.status = "done".into();
            c.completed_at = Some(10.0);
        }),
        Box::new(|st| st.projects.push(planet("p-2", "Mars"))),
        Box::new(|st| card_mut(st, "c-a").project_id = "p-2".into()),
        Box::new(|st| card_mut(st, "c-a").notes = Some("typed notes".into())),
        Box::new(|st| card_mut(st, "c-a").notes = None),
        Box::new(|st| {
            let mut n = card("n-1", "Ship it?", "p-1");
            n.kind = Some("needed".into());
            n.needed_state = Some("open".into());
            n.needed_why = Some("go gate".into());
            st.kanban_cards.push(n);
        }),
        Box::new(|st| {
            let n = card_mut(st, "n-1");
            n.needed_answer = Some("yes, after verify".into());
            n.needed_state = Some("answered".into());
        }),
        Box::new(|st| card_mut(st, "c-a").due_at = Some(1_900_000_000.0)),
        Box::new(|st| card_mut(st, "c-a").tag_ids = Some(vec!["t-1".into()])),
        Box::new(|st| {
            card_mut(st, "c-a").subtasks = Some(vec![Subtask {
                id: "s1".into(),
                title: "first".into(),
                done: false,
            }])
        }),
        Box::new(|st| card_mut(st, "c-a").subtasks.as_mut().unwrap()[0].done = true),
        Box::new(|st| st.kanban_cards.push(card("c-b", "Child", "p-1"))),
        Box::new(|st| card_mut(st, "c-b").parent_id = Some("c-a".into())),
        Box::new(|st| st.projects[1].name = "Red Planet".into()),
        Box::new(|st| st.projects[1].workspace_root = Some("/tmp/red".into())),
        Box::new(|st| st.kanban_cards.retain(|c| c.id != "c-a")),
        Box::new(|st| st.kanban_cards.push(card("c-a", "Back again", "p-1"))),
        Box::new(|st| card_mut(st, "c-b").rating = Some(5)),
        Box::new(|st| {
            card_mut(st, "c-b").links = Some(vec![CardLink {
                url: "https://example.com/doc".into(),
                label: Some("doc".into()),
            }])
        }),
        Box::new(|st| card_mut(st, "c-b").starred = true),
        Box::new(|st| card_mut(st, "c-b").goal_aligned = false),
        Box::new(|st| card_mut(st, "c-b").clinical_or_master = true),
        Box::new(|st| card_mut(st, "c-b").assigned_agent_id = Some("agent-7".into())),
        Box::new(|st| card_mut(st, "c-b").difficulty = Some("medium".into())),
        Box::new(|st| card_mut(st, "c-b").last_touched_at = 77.0),
        Box::new(|st| st.projects.retain(|p| p.id != "p-2")),
        Box::new(|st| st.kanban_cards.retain(|c| c.id != "n-1")),
        Box::new(|st| {
            let c = card_mut(st, "c-old");
            c.title = "Old card renamed".into();
            c.status = "doing".into();
        }),
    ]
}

/// The fold of checkpoint plus lines equals the saved board after every
/// step, so `stop_at` mid-sequence is the board at that step.
#[test]
fn fold_of_checkpoint_and_lines_matches_every_saved_step() {
    let run = Run::new("fold", &start_state());
    let ack = run.checkpoint();
    let steps = script();
    assert!(steps.len() >= 30);
    let mut at_step: Vec<(usize, Board)> = vec![(run.lines().len(), run.board())];
    for s in &steps {
        run.step(|st| s(st));
        at_step.push((run.lines().len(), run.board()));
    }
    let lines = run.lines();
    assert_eq!(lines[0].kind, KIND_CHECKPOINT);
    let cp = load_checkpoint(&run.dir, &ack.file).unwrap();
    assert_eq!(fold(&cp, &lines, None), run.board());
    for (i, (n, board)) in at_step.iter().enumerate() {
        let stop = &lines[n - 1].id;
        assert_eq!(&fold(&cp, &lines, Some(stop)), board, "stop after step {i}");
    }
    let final_board = run.board();
    assert!(final_board.cards.contains_key("c-a"), "revived");
    assert!(!final_board.cards.contains_key("n-1"));
    assert!(!final_board.projects.contains_key("p-2"));
    assert_eq!(final_board.cards["c-a"]["title"], "Back again");
    assert_eq!(final_board.cards["c-old"]["status"], "doing");

    let kinds: HashSet<u64> = lines.iter().map(|l| l.kind).collect();
    assert_eq!(kinds, HashSet::from([423, 424, 425, 426]));
    let answer = lines
        .iter()
        .find(|l| l.d == "n-1" && l.content.pointer("/fields/neededAnswer").is_some())
        .expect("needed answer line");
    assert_eq!(answer.content["fields"]["neededAnswer"], "yes, after verify");
    assert_eq!(
        at_step[28].0, at_step[27].0,
        "a lastTouchedAt-only save writes no line"
    );
}

#[test]
fn stop_at_the_checkpoint_line_is_the_checkpoint_board() {
    let run = Run::new("stop0", &start_state());
    let before = run.board();
    let ack = run.checkpoint();
    run.step(|st| st.kanban_cards.clear());
    let lines = run.lines();
    let cp = load_checkpoint(&run.dir, &ack.file).unwrap();
    assert_eq!(fold(&cp, &lines, Some(&lines[0].id)), before);
    assert!(fold(&cp, &lines, None).cards.is_empty());
}

#[test]
fn a_dead_history_log_never_fails_the_save() {
    use std::os::unix::fs::PermissionsExt;
    let run = Run::new("ro", &start_state());
    let ro = run.dir.join("ro");
    std::fs::create_dir_all(&ro).unwrap();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o500)).unwrap();
    let events = ro.join("events.jsonl");
    let out = {
        let _g = write_lock();
        update_at_logged(&run.state, &events, |st| {
            st.kanban_cards.push(card("c-saved", "Saved anyway", "p-1"));
            Ok(7)
        })
    };
    assert_eq!(out, Ok(7));
    assert!(!events.exists());
    let st = load_path(&run.state).unwrap();
    assert!(st.kanban_cards.iter().any(|c| c.id == "c-saved"));
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn actor_scope_nests_and_restores() {
    use crate::state::current_actor;
    assert_eq!(current_actor(), "app");
    with_actor("ctl", || {
        assert_eq!(current_actor(), "ctl");
        with_actor("agent", || assert_eq!(current_actor(), "agent"));
        assert_eq!(current_actor(), "ctl");
    });
    assert_eq!(current_actor(), "app");
    let _ = std::panic::catch_unwind(|| with_actor("user", || panic!("boom")));
    assert_eq!(current_actor(), "app");
    let other = std::thread::spawn(|| current_actor()).join().unwrap();
    assert_eq!(other, "app");

    let run = Run::new("actor", &start_state());
    with_actor("user", || run.step(|st| card_mut(st, "c-old").title = "by user".into()));
    with_actor("ctl", || run.step(|st| st.kanban_cards.push(card("c-ctl", "by ctl", "p-1"))));
    run.step(|st| card_mut(st, "c-ctl").title = "by app".into());
    let actors: Vec<String> = run.lines().into_iter().map(|l| l.actor).collect();
    assert_eq!(actors, ["user", "ctl", "app"]);
}

#[test]
fn lines_are_local_only_history_and_never_publishable() {
    let run = Run::new("tags", &start_state());
    run.checkpoint();
    run.step(|st| st.projects.push(planet("p-3", "Moon")));
    run.step(|st| st.kanban_cards.push(card("c-t", "Tagged", "p-3")));
    run.step(|st| st.kanban_cards.retain(|c| c.id != "c-t"));
    let raw = std::fs::read_to_string(&run.events).unwrap();
    assert_eq!(raw.lines().count(), 4);
    for line in raw.lines() {
        let v: Value = serde_json::from_str(line).unwrap();
        let kind = v["kind"].as_u64().unwrap();
        assert!((423..=426).contains(&kind), "{line}");
        let tags = v["tags"].as_array().unwrap();
        assert_eq!(tag(tags, "t"), Some("local-only"), "{line}");
        assert_eq!(tag(tags, "e-type"), Some("history"), "{line}");
        assert!(tag(tags, "d").is_some_and(|d| !d.is_empty()), "{line}");
        assert!(tag(tags, "actor").is_some(), "{line}");
        assert!(v["content"].is_string());
        assert!(!crate::event_relay::publishable(&v), "{line}");
        assert!(parse_line(line).is_some());
    }
}

#[test]
fn checkpoint_file_modes_sha_and_426_line() {
    use std::os::unix::fs::PermissionsExt;
    let run = Run::new("cp", &start_state());
    let ack = run.checkpoint();
    assert!(ack.file.starts_with("history/checkpoint-") && ack.file.ends_with(".json"));
    assert_eq!((ack.cards, ack.projects), (1, 1));
    let path = run.dir.join(&ack.file);
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(sha256_hex(&bytes), ack.sha256);
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&path), 0o600);
    assert_eq!(mode(&run.dir.join(HISTORY_DIR)), 0o700);
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["v"], 1);
    assert_eq!(v["projects"], json!([{ "id": "p-1", "name": "Home", "workspaceRoot": null }]));
    assert_eq!(v["cards"][0]["id"], "c-old");
    let lines = run.lines();
    let line = one_line(&lines);
    assert_eq!(line.kind, KIND_CHECKPOINT);
    assert_eq!(
        line.content,
        json!({ "v": 1, "file": ack.file, "sha256": ack.sha256, "cards": 1, "projects": 1 })
    );
    assert!(has_checkpoint(&run.dir));
    let second = run.checkpoint();
    assert_ne!(second.file, ack.file, "same-second checkpoints never collide");
}

fn one_line(lines: &[HistoryLine]) -> &HistoryLine {
    assert_eq!(lines.len(), 1, "{lines:#?}");
    &lines[0]
}

#[test]
fn list_filters_newest_first_with_a_limit() {
    let run = Run::new("list", &start_state());
    run.step(|st| st.projects.push(planet("p-2", "Mars")));
    run.step(|st| st.kanban_cards.push(card("c-1", "One", "p-1")));
    run.step(|st| st.kanban_cards.push(card("c-2", "Two", "p-2")));
    run.step(|st| card_mut(st, "c-1").title = "One again".into());
    run.step(|st| card_mut(st, "c-1").project_id = "p-2".into());
    let all = list_in(&run.events, None, None, None).unwrap();
    assert_eq!(all.len(), 5);
    assert_eq!(all[0].content["fields"]["projectId"], "p-2", "newest first");
    let c1 = list_in(&run.events, Some("c-1"), None, None).unwrap();
    assert_eq!(c1.len(), 3);
    assert!(c1.iter().all(|l| l.d == "c-1"));
    assert_eq!(c1[1].content["fields"]["title"], "One again");
    let p2 = list_in(&run.events, None, Some("p-2"), None).unwrap();
    let ds: Vec<&str> = p2.iter().map(|l| l.d.as_str()).collect();
    assert_eq!(ds, ["c-1", "c-2", "p-2"]);
    assert_eq!(list_in(&run.events, None, None, Some(2)).unwrap().len(), 2);
    assert_eq!(list_in(&run.events, None, None, Some(0)).unwrap().len(), 1);
    assert_eq!(LIST_DEFAULT, 50);
    assert_eq!(LIST_MAX, 500);
    let missing = run.dir.join("nope.jsonl");
    assert!(list_in(&missing, None, None, None).unwrap().is_empty());
}

#[test]
fn only_history_tagged_lines_parse() {
    let foreign = json!({
        "id": "x", "pubkey": "", "created_at": 1, "kind": 423,
        "tags": [["t", "nch"], ["e-type", "compute"]], "content": "{}", "sig": ""
    });
    assert!(parse_line(&foreign.to_string()).is_none());
    let board = json!({
        "id": "y", "pubkey": "", "created_at": 1, "kind": 420,
        "tags": [["t", "orbitty"], ["e-type", "history"]], "content": "{}", "sig": ""
    });
    assert!(parse_line(&board.to_string()).is_none());
}

const SENTINEL: &str = "SECRET-SENTINEL";

fn secret_state() -> PersistedState {
    let s = |tag: &str| format!("{SENTINEL}-{tag}");
    let mut st: PersistedState = serde_json::from_value(json!({
        "projects": [{ "id": "p-1", "name": "Home" }],
        "claims": [{
            "windowId": "w-1", "projectId": "p-1",
            "command": s("claim-command"), "cwd": s("claim-cwd")
        }],
        "actionShortcuts": [{
            "id": "a-1", "name": "act", "command": s("action-command"), "cwd": s("action-cwd")
        }],
        "agentShortcuts": [{
            "id": "ag-1", "name": "agent", "harness": "cursor", "profile": s("agent-profile"),
            "model": s("agent-model"), "extra": s("agent-extra"), "mdPaths": [s("agent-md")]
        }],
        "companion": {
            "lastUserLine": s("companion-user"),
            "lastBotLine": s("companion-bot"),
            "turns": [{ "role": "user", "text": s("companion-turn") }]
        },
        "privateRelayUrl": s("relay-url"),
    }))
    .unwrap();
    st.kanban_cards.push(card("c-1", "Plain card", "p-1"));
    let raw = serde_json::to_string(&st).unwrap();
    for tag in [
        "claim-command",
        "action-command",
        "agent-extra",
        "companion-turn",
        "relay-url",
    ] {
        assert!(raw.contains(&s(tag)), "fixture lost {tag}");
    }
    st
}

#[test]
fn no_secret_ever_lands_in_a_line_or_checkpoint() {
    let run = Run::new("secret", &secret_state());
    run.checkpoint();
    run.step(|st| st.kanban_cards.push(card("c-2", "Second", "p-1")));
    run.step(|st| card_mut(st, "c-1").title = "Renamed".into());
    run.step(|st| {
        st.companion.last_user_line = format!("{SENTINEL}-later");
        card_mut(st, "c-1").notes = Some("plain note".into());
    });
    run.step(|st| st.projects.push(planet("p-2", "Mars")));
    run.step(|st| st.kanban_cards.retain(|c| c.id != "c-2"));
    run.checkpoint();
    let events = std::fs::read_to_string(&run.events).unwrap();
    assert!(events.lines().count() >= 7, "{events}");
    assert!(!events.contains(SENTINEL), "{events}");
    let mut checkpoints = 0;
    for entry in std::fs::read_dir(run.dir.join(HISTORY_DIR)).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        assert!(!text.contains(SENTINEL), "{text}");
        checkpoints += 1;
    }
    assert_eq!(checkpoints, 2);
    let state = std::fs::read_to_string(&run.state).unwrap();
    assert!(state.contains(SENTINEL), "the sentinels are really in the saved state");
}

#[test]
fn update_records_history_after_the_save_only() {
    let src = include_str!("../state/io.rs");
    let start = src.find("pub fn update<T>(").unwrap();
    let body = &src[start..start + src[start..].find("\n}\n").unwrap()];
    let save = body.find("change_and_save(").unwrap();
    let record = body.find("history::record_best(").unwrap();
    assert!(save < record, "{body}");
    assert!(body.contains("if let Some((before, after)) = saved"));
}

#[test]
fn actor_scope_holds_until_drop() {
    with_actor("ctl", || {
        {
            let _a = crate::state::actor_scope("agent");
            assert_eq!(crate::state::current_actor(), "agent");
        }
        assert_eq!(crate::state::current_actor(), "ctl");
    });
    assert_eq!(crate::state::current_actor(), "app");
}

#[test]
fn every_door_names_its_actor() {
    let fn_body = |src: &'static str, sig: &str| -> &'static str {
        let start = src.find(sig).unwrap_or_else(|| panic!("{sig} missing"));
        &src[start..start + src[start..].find("\n}\n").unwrap()]
    };
    let lib = include_str!("../lib.rs");
    assert!(lib.contains(".invoke_handler(as_user(tauri::generate_handler!["));
    assert!(fn_body(lib, "fn as_user<F>(").contains("with_actor(\"user\""));
    let setup = &lib[lib.find(".setup(|app| {").unwrap()..];
    let boot = setup.find("history::boot_checkpoint(app.handle())").unwrap();
    assert!(boot < setup.find("state::migrate_stars_scale(").unwrap());
    let ctl = fn_body(include_str!("../agents/control.rs"), "fn handle_conn(");
    assert!(ctl.contains("with_actor(\"ctl\", || dispatch(&app, req))"), "{ctl}");
    let needed = fn_body(include_str!("../board/board.rs"), "pub fn needed_open(");
    assert!(needed.contains("with_actor(\"rule\", || save_with_events("), "{needed}");
    let fire = fn_body(include_str!("../agents/doing_fire.rs"), "pub fn spawn_fire_and_attach(");
    assert!(fire.contains("with_actor(\"agent\""), "{fire}");
    let pickup = fn_body(include_str!("../agents/agents.rs"), "pub fn agent_pickup(");
    let scope = pickup.find("let _actor = crate::state::actor_scope(\"agent\");").unwrap();
    assert!(scope < pickup.find("load_state(").unwrap());
    let verify = include_str!("../agents/verify_close.rs");
    for sig in ["pub fn arm_launch(", "pub fn arm("] {
        assert!(fn_body(verify, sig).contains("with_actor(\"agent\""), "{sig}");
    }
    let paint = fn_body(include_str!("../habitat/herdr.rs"), "fn paint(");
    assert!(paint.contains("with_actor(\"agent\", || crate::board::save_with_events("));
    let shift = fn_body(include_str!("../agents/shift.rs"), "pub fn boot(");
    assert!(shift.contains("with_actor(\"shift\", || tick(&handle))"), "{shift}");
}
