use super::*;
use crate::relay::history::*;
use crate::state::{
    load_path, save_path, update_at_logged, write_lock, CardLink, KanbanCard, PersistedState,
    Project, Subtask,
};
use serde_json::json;
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "orbitty-rewind-{tag}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A synthetic board on disk with a checkpoint and `history` history lines
/// after it, one 31200 noise line between each. Invented values only.
pub(super) struct Fixture {
    pub dir: PathBuf,
    pub state: PathBuf,
    pub events: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn big_card(i: usize, planets: usize) -> KanbanCard {
    let mut c = crate::board::todo_card(
        format!("c-{i:04}"),
        &format!("Synthetic card number {i} with a realistic title"),
        &format!("p-{}", i % planets),
        ["todo", "doing", "done"][i % 3],
        "user",
        Some(&"Synthetic note line for the timing fixture. ".repeat(9)),
        i as f64,
    );
    c.tag_ids = Some(vec!["t-1".into(), "t-2".into()]);
    c.subtasks = Some(vec![Subtask {
        id: format!("s-{i}"),
        title: "Synthetic subtask".into(),
        done: i % 2 == 0,
    }]);
    c.links = Some(vec![CardLink {
        url: format!("https://example.com/doc/{i}"),
        label: None,
    }]);
    c
}

pub(super) fn big_state(cards: usize, planets: usize) -> PersistedState {
    let mut st = PersistedState::default();
    for p in 0..planets {
        st.projects.push(Project {
            id: format!("p-{p}"),
            name: format!("Planet {p}"),
            trackers: None,
            workspace_root: None,
        });
    }
    for i in 0..cards {
        st.kanban_cards.push(big_card(i, planets));
    }
    st
}

/// One synthetic edit per step, rotating cards and fields, with an add and
/// a delete every 50 steps. The state on disk ends equal to the fold.
fn step(st: &mut PersistedState, n: usize, planets: usize) -> Vec<Entry> {
    let cards = st.kanban_cards.len();
    if n % 50 == 49 {
        let before = st.kanban_cards.clone();
        let gone = st.kanban_cards.remove(n % cards);
        let mut fresh = big_card(10_000 + n, planets);
        fresh.id = format!("c-new-{n}");
        st.kanban_cards.push(fresh);
        let _ = gone;
        return diff(&before, &[], &st.kanban_cards, &[]);
    }
    let i = (n * 7) % cards;
    let old = st.kanban_cards[i].clone();
    let c = &mut st.kanban_cards[i];
    match n % 5 {
        0 => c.title = format!("Renamed card {i} at step {n}"),
        1 => c.notes = Some(format!("Step {n} note. {}", "More words here. ".repeat(20))),
        2 => c.status = ["todo", "doing", "done"][n % 3].into(),
        3 => c.links = None,
        _ => c.tag_ids = Some(vec![format!("t-{}", n % 9)]),
    }
    diff(&[old], &[], &[c.clone()], &[])
}

pub(super) fn big_fixture(tag: &str, cards: usize, planets: usize, history: usize) -> Fixture {
    let dir = temp_dir(tag);
    let state = dir.join("state.json");
    let events = dir.join("events.jsonl");
    let mut st = big_state(cards, planets);
    save_path(&state, &st).unwrap();
    write_checkpoint_in(&dir, &events, &st, "app", None, &unsigned).unwrap();
    let noise = json!({
        "id": "noise", "pubkey": "", "created_at": 1, "kind": 31200,
        "tags": [["t", "orbitty"], ["e-type", "board-move"]],
        "content": "{\"cardId\":\"c-0001\",\"from\":\"todo\",\"to\":\"doing\"}", "sig": ""
    })
    .to_string();
    let mut text = String::new();
    let mut written = 0;
    let mut n = 0;
    while written < history {
        let entries = step(&mut st, n, planets);
        n += 1;
        for e in entries {
            if written == history {
                break;
            }
            let tags = vec![
                ["t".to_string(), "local-only".to_string()],
                ["d".to_string(), e.d.clone()],
                ["actor".to_string(), "user".to_string()],
                ["e-type".to_string(), "history".to_string()],
            ];
            let line = json!({
                "id": format!("h-{written:06}"), "pubkey": "", "created_at": 1_800_000_000u64 + written as u64,
                "kind": e.kind, "tags": tags, "content": e.content.to_string(), "sig": ""
            });
            text.push_str(&line.to_string());
            text.push('\n');
            text.push_str(&noise);
            text.push('\n');
            written += 1;
        }
    }
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&events)
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
    save_path(&state, &st).unwrap();
    Fixture { dir, state, events }
}

fn median(mut ms: Vec<f64>) -> f64 {
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ms[ms.len() / 2]
}

fn time<T>(runs: usize, mut f: impl FnMut() -> T) -> f64 {
    let mut ms = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = std::time::Instant::now();
        std::hint::black_box(f());
        ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    median(ms)
}

/// Phase-0 timing on synthetic boards (650 cards, 19 planets).
/// `cargo test --lib rewind_timing -- --ignored --nocapture`.
#[test]
#[ignore]
fn rewind_timing_650_cards() {
    for history in [5_000, 20_000] {
        let fx = big_fixture("timing", 650, 19, history);
        let lines = read_lines(&fx.events).unwrap();
        assert_eq!(lines.len(), history + 1);
        let cp_line = &lines[0];
        let file = cp_line.content["file"].as_str().unwrap().to_string();
        let cp = load_checkpoint(&fx.dir, &file).unwrap();
        let st = load_path(&fx.state).unwrap();
        assert_eq!(
            fold(&cp, &lines, None),
            board_of(&st.kanban_cards, &st.projects),
            "fixture disk equals its history"
        );
        let bytes = std::fs::metadata(&fx.events).unwrap().len();
        let read = time(15, || read_lines(&fx.events).unwrap());
        let at_newest = time(15, || fold(&cp, &lines, None));
        let full = time(15, || {
            let lines = read_lines(&fx.events).unwrap();
            let cp = load_checkpoint(&fx.dir, &file).unwrap();
            let st = load_path(&fx.state).unwrap();
            fold(&cp, &lines, None) == board_of(&st.kanban_cards, &st.projects)
        });
        println!(
            "history={history} events.jsonl bytes={bytes} read_lines={read:.2}ms \
             fold-to-newest={at_newest:.2}ms full-compare(read+checkpoint+state+fold+board_of+eq)={full:.2}ms"
        );
    }
}

pub(super) struct Run {
    pub dir: PathBuf,
    pub state: PathBuf,
    pub events: PathBuf,
}

impl Run {
    pub fn new(tag: &str, initial: &PersistedState) -> Self {
        let dir = temp_dir(tag);
        let state = dir.join("state.json");
        let events = dir.join("events.jsonl");
        save_path(&state, initial).unwrap();
        Self { dir, state, events }
    }

    pub fn step(&self, f: impl FnOnce(&mut PersistedState)) {
        let _g = write_lock();
        update_at_logged(&self.state, &self.events, |st| {
            f(st);
            Ok(())
        })
        .unwrap();
    }

    pub fn checkpoint(&self) -> CheckpointAck {
        let st = load_path(&self.state).unwrap();
        write_checkpoint_in(&self.dir, &self.events, &st, "app", None, &unsigned).unwrap()
    }

    pub fn st(&self) -> PersistedState {
        load_path(&self.state).unwrap()
    }

    pub fn board(&self) -> Board {
        let st = self.st();
        board_of(&st.kanban_cards, &st.projects)
    }

    pub fn lines(&self) -> Vec<HistoryLine> {
        read_lines(&self.events).unwrap()
    }

    pub fn checkpoints(&self) -> Vec<Checkpoint> {
        load_named(&self.dir, &self.lines())
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub(super) fn card(id: &str, title: &str, project: &str) -> KanbanCard {
    crate::board::todo_card(id.into(), title, project, "todo", "user", None, 1.0)
}

pub(super) fn planet(id: &str, name: &str) -> Project {
    Project {
        id: id.into(),
        name: name.into(),
        trackers: None,
        workspace_root: None,
    }
}

pub(super) fn card_mut<'a>(st: &'a mut PersistedState, id: &str) -> &'a mut KanbanCard {
    st.kanban_cards.iter_mut().find(|c| c.id == id).unwrap()
}

pub(super) fn start_state() -> PersistedState {
    let mut st = PersistedState::default();
    st.projects.push(planet("p-1", "Home"));
    st.projects.push(planet("p-2", "Mars"));
    let mut a = card("c-a", "Alpha", "p-1");
    a.notes = Some("alpha notes".into());
    a.links = Some(vec![CardLink {
        url: "https://example.com/a".into(),
        label: Some("a".into()),
    }]);
    st.kanban_cards.push(a);
    st.kanban_cards.push(card("c-b", "Beta", "p-2"));
    st
}

type Step = Box<dyn Fn(&mut PersistedState)>;

fn script() -> Vec<Step> {
    vec![
        Box::new(|st| card_mut(st, "c-a").title = "Alpha 2".into()),
        Box::new(|st| card_mut(st, "c-b").status = "doing".into()),
        Box::new(|st| st.kanban_cards.push(card("c-c", "Gamma", "p-1"))),
        Box::new(|st| card_mut(st, "c-c").notes = Some("gamma".into())),
        Box::new(|st| st.projects[1].name = "Red".into()),
        Box::new(|st| card_mut(st, "c-a").links = None),
        Box::new(|st| st.kanban_cards.retain(|c| c.id != "c-b")),
        Box::new(|st| card_mut(st, "c-c").tag_ids = Some(vec!["t-1".into()])),
        Box::new(|st| st.projects.push(planet("p-3", "Moon"))),
        Box::new(|st| card_mut(st, "c-c").project_id = "p-3".into()),
        Box::new(|st| card_mut(st, "c-a").status = "done".into()),
        Box::new(|st| st.kanban_cards.push(card("c-b", "Beta again", "p-1"))),
        Box::new(|st| card_mut(st, "c-a").title = "Alpha 3".into()),
        Box::new(|st| st.projects.retain(|p| p.id != "p-3")),
        Box::new(|st| card_mut(st, "c-c").notes = None),
    ]
}

/// The board at every step's last line equals the saved board, across
/// two later checkpoints. A write with no line just before the second
/// checkpoint proves the newest checkpoint is the one folded.
#[test]
fn board_at_matches_every_saved_step_across_two_checkpoints() {
    let run = Run::new("at", &start_state());
    run.checkpoint();
    let mut at_step: Vec<(usize, Board)> = Vec::new();
    for (i, s) in script().iter().enumerate() {
        run.step(|st| s(st));
        at_step.push((run.lines().len(), run.board()));
        if i == 4 {
            run.checkpoint();
        }
        if i == 9 {
            let mut st = run.st();
            card_mut(&mut st, "c-a").notes = Some("outside writer".into());
            save_path(&run.state, &st).unwrap();
            run.checkpoint();
        }
    }
    let lines = run.lines();
    let cps = run.checkpoints();
    assert_eq!(cps.len(), 3);
    assert_eq!(lines.iter().filter(|l| l.kind == KIND_CHECKPOINT).count(), 3);
    for (i, (n, board)) in at_step.iter().enumerate() {
        let at = &lines[n - 1].id;
        assert_eq!(&board_at(&cps, &lines, at).unwrap(), board, "step {i}");
    }
    let third = lines.iter().rposition(|l| l.kind == KIND_CHECKPOINT).unwrap();
    let at_third = board_at(&cps, &lines, &lines[third].id).unwrap();
    assert_eq!(at_third.cards["c-a"]["notes"], "outside writer");
    assert_eq!(board_now(&cps, &lines).unwrap(), run.board());
    assert_eq!(board_now(&cps, &lines).unwrap().cards["c-a"]["notes"], "outside writer");
    let only_first = &cps[..1];
    assert_ne!(
        board_now(only_first, &lines).unwrap(),
        run.board(),
        "without the newest checkpoint the outside write is missed"
    );
}

#[test]
fn board_at_errors_on_unknown_ids_and_lines_before_history() {
    let run = Run::new("err", &start_state());
    run.step(|st| card_mut(st, "c-a").title = "before any checkpoint".into());
    run.checkpoint();
    run.step(|st| card_mut(st, "c-a").title = "after".into());
    let lines = run.lines();
    let cps = run.checkpoints();
    assert_eq!(
        board_at(&cps, &lines, "nope").unwrap_err(),
        "That point is not in the history."
    );
    let err = board_at(&cps, &lines, &lines[0].id).unwrap_err();
    assert!(err.starts_with("History starts at "), "{err}");
    assert!(board_at(&cps, &lines, &lines[1].id).is_ok());
    assert_eq!(board_now(&cps, &[]).unwrap_err(), "There is no history yet.");
    assert_eq!(
        board_at(&[], &lines, &lines[2].id).unwrap_err(),
        err,
        "a checkpoint line whose file is not loaded"
    );
}

#[test]
fn full_card_is_the_newest_whole_card_with_later_sets() {
    let run = Run::new("full", &start_state());
    run.checkpoint();
    run.step(|st| {
        let c = card_mut(st, "c-a");
        c.title = "Alpha 2".into();
        c.subtasks = Some(vec![Subtask {
            id: "s1".into(),
            title: "one".into(),
            done: false,
        }]);
        c.points_awarded = 2;
    });
    run.step(|st| card_mut(st, "c-a").tag_ids = Some(vec!["t-1".into()]));
    let before_delete = run
        .st()
        .kanban_cards
        .into_iter()
        .find(|c| c.id == "c-a")
        .unwrap();
    run.step(|st| st.kanban_cards.retain(|c| c.id != "c-a"));
    let lines = run.lines();
    let cps = run.checkpoints();
    let tomb = lines.iter().position(|l| l.kind == KIND_TOMBSTONE).unwrap();
    let whole = full_card("c-a", &cps, &lines[..tomb]).unwrap();
    let mut want = serde_json::to_value(&before_delete).unwrap();
    want["pointsAwarded"] = json!(0);
    assert_eq!(
        serde_json::to_value(&whole).unwrap(),
        want,
        "rollback fields from the lines, the rest from the checkpoint"
    );
    assert_eq!(
        serde_json::to_value(full_card("c-a", &cps, &lines).unwrap()).unwrap(),
        want,
        "a tombstone at the end still gives the card as it was"
    );
    assert_eq!(
        history::card_view(&whole),
        board_at(&cps, &lines, &lines[tomb - 1].id).unwrap().cards["c-a"]
    );
    assert!(full_card("c-zzz", &cps, &lines).is_none());

    run.step(|st| {
        let mut c = card("c-n", "Fresh", "p-1");
        c.notes = Some("born with notes".into());
        st.kanban_cards.push(c);
    });
    run.step(|st| card_mut(st, "c-n").title = "Fresh 2".into());
    let lines = run.lines();
    let fresh = full_card("c-n", &cps, &lines).unwrap();
    assert_eq!(fresh.title, "Fresh 2");
    assert_eq!(fresh.notes.as_deref(), Some("born with notes"));
}

#[test]
fn set_card_fields_clears_and_ignores_non_rollback_names() {
    let mut c = card("c-1", "T", "p-1");
    c.notes = Some("n".into());
    let fields = json!({
        "notes": null,
        "title": "New",
        "windowId": "w-9",
        "taskId": "t",
        "starred": true,
        "rating": "not a number",
    });
    set_card_fields(&mut c, fields.as_object().unwrap());
    assert_eq!(c.notes, None);
    assert_eq!(c.title, "New");
    assert!(c.starred);
    assert_eq!(c.window_id, None);
    assert_eq!(c.task_id, None);
    assert_eq!(c.rating, None, "a bad value is skipped, not fatal");
}

fn fake_line(i: usize, kind: u64) -> HistoryLine {
    HistoryLine {
        id: format!("l-{i}"),
        created_at: i as u64,
        kind,
        d: "c-1".into(),
        actor: "user".into(),
        content: if kind == KIND_CHECKPOINT {
            json!({ "file": format!("history/checkpoint-{i}.json") })
        } else {
            json!({ "op": "set", "id": "c-1", "fields": { "title": "x" } })
        },
    }
}

#[test]
fn auto_checkpoint_after_more_than_two_thousand_lines() {
    let mut lines = vec![fake_line(0, KIND_CHECKPOINT)];
    lines.extend((1..=AUTO_CHECKPOINT_LINES).map(|i| fake_line(i, KIND_CARD)));
    assert_eq!(lines_after_checkpoint(&lines), 2_000);
    assert!(!needs_checkpoint(&lines));
    lines.push(fake_line(2_001, KIND_CARD));
    assert!(needs_checkpoint(&lines));
    lines.push(fake_line(2_002, KIND_CHECKPOINT));
    assert_eq!(lines_after_checkpoint(&lines), 0);
    assert!(!needs_checkpoint(&lines));
    let bare: Vec<HistoryLine> = (0..2_001).map(|i| fake_line(i, KIND_CARD)).collect();
    assert!(needs_checkpoint(&bare));
}

#[test]
fn read_since_checkpoint_is_the_tail_of_read_lines() {
    let run = Run::new("tail", &start_state());
    run.step(|st| card_mut(st, "c-a").title = "one".into());
    run.checkpoint();
    run.step(|st| card_mut(st, "c-a").title = "two".into());
    run.checkpoint();
    run.step(|st| card_mut(st, "c-a").title = "three".into());
    run.step(|st| card_mut(st, "c-b").notes = Some("with \"kind\":426 inside".into()));
    let all = run.lines();
    let from = all.iter().rposition(|l| l.kind == KIND_CHECKPOINT).unwrap();
    assert_eq!(read_since_checkpoint(&run.events).unwrap(), all[from..].to_vec());
    let missing = run.dir.join("nope.jsonl");
    assert!(read_since_checkpoint(&missing).unwrap().is_empty());
}

fn apply_run(run: &Run, req: &Req) -> Result<Plan, String> {
    let _g = write_lock();
    crate::state::with_actor(req.actor(), || {
        update_at_logged(&run.state, &run.events, |st| {
            apply_in(st, &run.dir, &run.events, req, &unsigned, crate::state::wall_now_secs())
        })
    })
}

fn rewind_to(to: &str, card: Option<&str>, remove_added: bool) -> Req {
    Req::Rewind {
        to: to.into(),
        card: card.map(str::to_string),
        remove_added,
    }
}

fn view_bytes(b: &Board) -> String {
    let side = |m: &std::collections::BTreeMap<String, View>| -> serde_json::Value {
        m.iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::Object(v.clone())))
            .collect::<serde_json::Map<_, _>>()
            .into()
    };
    serde_json::to_string(&json!({ "cards": side(&b.cards), "projects": side(&b.projects) })).unwrap()
}

fn line_of<'a>(lines: &'a [HistoryLine], d: &str, field: &str) -> &'a HistoryLine {
    lines
        .iter()
        .rev()
        .find(|l| l.d == d && l.content.pointer(&format!("/fields/{field}")).is_some())
        .unwrap()
}

#[test]
fn restore_one_card_changes_only_that_card_in_one_logged_save() {
    let run = Run::new("one", &start_state());
    run.checkpoint();
    run.step(|st| card_mut(st, "c-a").title = "First rename".into());
    let first = run.lines().last().unwrap().id.clone();
    run.step(|st| card_mut(st, "c-a").title = "Second rename".into());
    run.step(|st| card_mut(st, "c-b").title = "Beta moved on".into());
    let before = run.board();
    let n = run.lines().len();

    let dry = {
        let st = run.st();
        resolve(&st, &run.dir, &run.lines(), &rewind_to(&first, Some("c-a"), false))
            .unwrap()
            .plan
    };
    assert_eq!(dry.cards.len(), 1);
    assert_eq!(
        dry.cards[0].fields,
        vec![FieldChange {
            field: "title".into(),
            from: json!("Second rename"),
            to: json!("First rename"),
        }]
    );
    assert_eq!(run.lines().len(), n, "a plan writes nothing");

    let plan = apply_run(&run, &rewind_to(&first, Some("c-a"), false)).unwrap();
    assert_eq!(plan, dry);
    let after = run.board();
    assert_eq!(after.cards["c-a"]["title"], "First rename");
    let mut want = before.clone();
    want.cards.get_mut("c-a").unwrap().insert("title".into(), json!("First rename"));
    assert_eq!(after, want, "no other card or field changes");

    let lines = run.lines();
    let new = &lines[n..];
    assert_eq!(new.len(), 2, "{new:#?}");
    assert_eq!(new[0].kind, KIND_CHECKPOINT, "the checkpoint comes first");
    assert_eq!(new[0].content["reason"], "pre-rewind");
    assert_eq!(new[0].actor, "rewind");
    assert_eq!(new[1].kind, KIND_CARD);
    assert_eq!(new[1].actor, "rewind");
    assert_eq!(new[1].content["fields"], json!({ "title": "First rename" }));
    let cp = history::load_checkpoint(&run.dir, new[0].content["file"].as_str().unwrap()).unwrap();
    assert_eq!(history::board_of(&cp.cards, &[]).cards, before.cards);

    let again = apply_run(&run, &rewind_to(&first, Some("c-a"), false)).unwrap();
    assert!(again.is_empty());
    assert_eq!(run.lines().len(), lines.len(), "an empty plan writes no checkpoint");
}

#[test]
fn bring_back_a_deleted_card_whole() {
    let mut start = start_state();
    {
        let a = start.kanban_cards.iter_mut().find(|c| c.id == "c-a").unwrap();
        a.tag_ids = Some(vec!["t-1".into(), "t-2".into()]);
        a.created_at = 42.0;
        a.points_awarded = 1;
    }
    let run = Run::new("back", &start);
    run.checkpoint();
    run.step(|st| {
        let a = card_mut(st, "c-a");
        a.notes = Some("last notes".into());
        a.window_id = Some("w-old".into());
        a.task_id = Some("task-old".into());
        a.last_touched_at = 500.0;
    });
    run.step(|st| {
        st.kanban_cards.retain(|c| c.id != "c-a");
        crate::state::upsert_tombstone(&mut st.card_tombstones, "c-a", 600.0);
    });
    let tomb = run.lines().last().unwrap().clone();
    assert_eq!(tomb.kind, KIND_TOMBSTONE);
    let plan = apply_run(&run, &rewind_to(&tomb.id, Some("c-a"), false)).unwrap();
    assert_eq!(
        plan.revive,
        vec![Named {
            id: "c-a".into(),
            title: "Alpha".into()
        }]
    );
    let st = run.st();
    let a = st.kanban_cards.iter().find(|c| c.id == "c-a").unwrap();
    assert_eq!(a.notes.as_deref(), Some("last notes"));
    assert_eq!(a.tag_ids, Some(vec!["t-1".into(), "t-2".into()]));
    assert_eq!(a.links.as_ref().unwrap()[0].url, "https://example.com/a");
    assert_eq!((a.created_at, a.points_awarded), (42.0, 1));
    assert_eq!((a.window_id.as_deref(), a.task_id.as_deref()), (None, None));
    assert!(a.last_touched_at > 600.0);
    assert!(st.card_tombstones.iter().all(|t| t.id != "c-a"));
    let add = run.lines().last().unwrap().clone();
    assert_eq!((add.kind, add.actor.as_str()), (KIND_CARD, "rewind"));
    assert_eq!(add.content["op"], "add");
}

fn rewind_script(run: &Run) -> String {
    run.checkpoint();
    run.step(|st| st.projects.push(planet("p-9", "Pluto")));
    let point = run.lines().last().unwrap().id.clone();
    run.step(|st| card_mut(st, "c-a").title = "Alpha later".into());
    run.step(|st| card_mut(st, "c-b").notes = Some("beta later".into()));
    run.step(|st| card_mut(st, "c-a").links = None);
    run.step(|st| st.kanban_cards.retain(|c| c.id != "c-b"));
    run.step(|st| st.kanban_cards.push(card("c-new", "New since", "p-1")));
    run.step(|st| st.projects[1].name = "Red".into());
    run.step(|st| st.projects.retain(|p| p.id != "p-9"));
    run.step(|st| st.projects.push(planet("p-moon", "Moon")));
    point
}

#[test]
fn rewind_the_board_keeps_added_cards_unless_asked() {
    let run = Run::new("board", &start_state());
    let point = rewind_script(&run);
    let lines = run.lines();
    let cps = run.checkpoints();
    let then = board_at(&cps, &lines, &point).unwrap();

    let plan = apply_run(&run, &rewind_to(&point, None, false)).unwrap();
    assert_eq!(plan.cards.len(), 1, "{plan:#?}");
    assert_eq!(plan.revive.len(), 1);
    assert_eq!(plan.added, vec![Named { id: "c-new".into(), title: "New since".into() }]);
    assert_eq!(plan.planets.len(), 1);
    assert_eq!(plan.planets_revive.len(), 1);
    assert_eq!(plan.planets_added.len(), 1);
    let after = run.board();
    let mut want = then.clone();
    want.cards.insert("c-new".into(), after.cards["c-new"].clone());
    want.projects.insert("p-moon".into(), after.projects["p-moon"].clone());
    assert_eq!(after, want, "back to the point, plus what was added since");

    let lines = run.lines();
    let point2 = lines[lines.len() - 1].id.clone();
    run.step(|st| st.kanban_cards.push(card("c-newer", "Newer", "p-1")));
    let plan = apply_run(&run, &rewind_to(&point2, None, true)).unwrap();
    assert_eq!(plan.added, vec![Named { id: "c-newer".into(), title: "Newer".into() }]);
    let st = run.st();
    assert!(st.kanban_cards.iter().all(|c| c.id != "c-newer"));
    assert!(st.card_tombstones.iter().any(|t| t.id == "c-newer"));
    assert!(st.projects.iter().any(|p| p.id == "p-moon"), "planets added since stay");
}

#[test]
fn rewind_then_undo_is_byte_equal() {
    let run = Run::new("undo", &start_state());
    let point = rewind_script(&run);
    run.step(|st| st.kanban_cards.push(card("c-x", "Added before rewind", "p-1")));
    let before = view_bytes(&run.board());
    let plan = apply_run(&run, &rewind_to(&point, None, true)).unwrap();
    assert!(!plan.is_empty());
    assert!(plan.added.iter().any(|n| n.id == "c-x"));
    assert_ne!(view_bytes(&run.board()), before);

    run.step(|st| st.kanban_cards.push(card("c-later", "Made after the rewind", "p-1")));
    run.step(|st| card_mut(st, "c-later").notes = Some("kept".into()));
    let undo_plan = {
        let st = run.st();
        resolve(&st, &run.dir, &run.lines(), &Req::Undo).unwrap().plan
    };
    assert!(undo_plan.added.iter().all(|n| n.id != "c-later"), "{undo_plan:#?}");
    apply_run(&run, &Req::Undo).unwrap();
    let mut now = run.board();
    let later = now.cards.remove("c-later").expect("an edit made after the rewind survives undo");
    assert_eq!(later["notes"], "kept");
    assert_eq!(view_bytes(&now), before);

    apply_run(&run, &Req::Undo).unwrap();
    let mut redo = run.board();
    redo.cards.remove("c-later");
    assert_ne!(view_bytes(&redo), before, "undo twice redoes the rewind");
}

#[test]
fn undo_with_nothing_to_undo_is_an_error() {
    let run = Run::new("noundo", &start_state());
    run.checkpoint();
    run.step(|st| card_mut(st, "c-a").title = "x".into());
    assert_eq!(apply_run(&run, &Req::Undo).unwrap_err(), "There is no rewind to undo.");
}

#[test]
fn cards_with_live_work_are_skipped_and_never_changed() {
    let mut start = start_state();
    start.kanban_cards.push(card("c-run", "Agent card", "p-1"));
    start.kanban_cards.push(card("c-task", "Task card", "p-1"));
    start.kanban_cards.push(card("c-win", "Window card", "p-1"));
    let run = Run::new("live", &start);
    run.checkpoint();
    let point = run.lines().last().unwrap().id.clone();
    run.step(|st| {
        for id in ["c-run", "c-task", "c-win", "c-a"] {
            card_mut(st, id).title = format!("{id} renamed");
        }
        card_mut(st, "c-task").task_id = Some("task-1".into());
        card_mut(st, "c-win").window_id = Some("w-1".into());
        st.agent_runs.push(crate::state::AgentRun {
            card_id: "c-run".into(),
            planet_id: "p-1".into(),
            ..Default::default()
        });
        st.claims
            .push(serde_json::from_value(json!({ "windowId": "w-1", "projectId": "p-1" })).unwrap());
        let mut live_new = card("c-live-new", "Live and new", "p-1");
        live_new.task_id = Some("task-2".into());
        st.kanban_cards.push(live_new);
    });
    let before = run.st();
    let plan = apply_run(&run, &rewind_to(&point, None, true)).unwrap();
    let why: Vec<(&str, &str)> = plan
        .skipped
        .iter()
        .map(|s| (s.id.as_str(), s.why.as_str()))
        .collect();
    assert_eq!(
        why,
        [
            ("c-run", "an agent is working on it"),
            ("c-task", "a task is running"),
            ("c-win", "it is bound to a claimed window"),
            ("c-live-new", "a task is running"),
        ]
    );
    let after = run.st();
    for id in ["c-run", "c-task", "c-win", "c-live-new"] {
        let find = |st: &PersistedState| {
            serde_json::to_value(st.kanban_cards.iter().find(|c| c.id == id).unwrap()).unwrap()
        };
        assert_eq!(find(&after), find(&before), "{id} untouched");
    }
    assert_eq!(card_mut(&mut run.st(), "c-a").title, "Alpha");

    let mut done = run.st();
    done.agent_runs[0].state = "done".into();
    done.kanban_cards.iter_mut().find(|c| c.id == "c-task").unwrap().task_reason = Some("done".into());
    assert_eq!(live_why(&done, done.kanban_cards.iter().find(|c| c.id == "c-run").unwrap()), None);
    assert_eq!(live_why(&done, done.kanban_cards.iter().find(|c| c.id == "c-task").unwrap()), None);
}

/// Phase 0 item 1: a window that saves the board it held before the
/// rewind keeps none of its old rollback fields, because the rewind bumps
/// every card it touches.
#[test]
fn a_stale_window_save_cannot_undo_a_rewind() {
    let mut start = start_state();
    start.kanban_cards.push(card("c-still", "Not part of the rewind", "p-1"));
    let run = Run::new("stale", &start);
    let point = rewind_script(&run);
    run.step(|st| st.kanban_cards.push(card("c-x", "Removed by the rewind", "p-1")));
    let stale = run.st();
    apply_run(&run, &rewind_to(&point, None, true)).unwrap();
    let disk = run.st();

    let mut incoming = stale.clone();
    {
        let c = incoming.kanban_cards.iter_mut().find(|c| c.id == "c-still").unwrap();
        c.notes = Some("an edit in the window".into());
        c.last_touched_at = crate::state::wall_now_secs() + 1.0;
    }
    let (merged, _) = crate::state::merge_window_save(&disk, &incoming);
    let mut want = history::board_of(&disk.kanban_cards, &disk.projects);
    want.cards
        .get_mut("c-still")
        .unwrap()
        .insert("notes".into(), json!("an edit in the window"));
    let got = history::board_of(&merged.kanban_cards, &disk.projects);
    assert_eq!(got.cards, want.cards);
    assert!(merged.kanban_cards.iter().any(|c| c.id == "c-b"), "a revived card stays");
    assert!(merged.kanban_cards.iter().all(|c| c.id != "c-x"), "a removed card stays gone");

    let mut unbumped = disk.clone();
    for c in &mut unbumped.kanban_cards {
        if let Some(old) = stale.kanban_cards.iter().find(|s| s.id == c.id) {
            c.last_touched_at = old.last_touched_at;
        }
    }
    let (merged, _) = crate::state::merge_window_save(&unbumped, &incoming);
    let a = merged.kanban_cards.iter().find(|c| c.id == "c-a").unwrap();
    assert_eq!(a.title, "Alpha later", "without the bump the tie takes the stale window");
}

fn drift_of(run: &Run) -> Drift {
    drift_in(&run.st(), &run.dir, &read_since_checkpoint(&run.events).unwrap()).unwrap()
}

fn outside_write(run: &Run, f: impl FnOnce(&mut PersistedState)) {
    let mut st = run.st();
    f(&mut st);
    save_path(&run.state, &st).unwrap();
}

fn named(id: &str, title: &str) -> Named {
    Named {
        id: id.into(),
        title: title.into(),
    }
}

#[test]
fn drift_finds_fields_missing_unknown_and_planets() {
    let run = Run::new("drift", &start_state());
    run.checkpoint();
    run.step(|st| card_mut(st, "c-a").title = "Alpha 2".into());
    run.step(|st| st.projects.push(planet("p-3", "Moon")));
    assert!(drift_of(&run).is_empty());

    outside_write(&run, |st| {
        card_mut(st, "c-a").title = "Outside".into();
        card_mut(st, "c-a").last_touched_at = 9e9;
        st.kanban_cards.retain(|c| c.id != "c-b");
        st.kanban_cards.push(card("c-out", "From outside", "p-1"));
        st.projects[1].name = "Red".into();
        st.projects.retain(|p| p.id != "p-3");
        st.projects.push(planet("p-out", "Elsewhere"));
    });
    let d = drift_of(&run);
    assert_eq!(
        d.cards,
        vec![ItemDrift {
            id: "c-a".into(),
            title: "Outside".into(),
            fields: vec![FieldDrift {
                field: "title".into(),
                history: json!("Alpha 2"),
                now: json!("Outside"),
            }],
        }],
        "lastTouchedAt is not a rollback field"
    );
    assert_eq!(d.missing, vec![named("c-b", "Beta")]);
    assert_eq!(d.unknown, vec![named("c-out", "From outside")]);
    assert_eq!(d.planets[0].fields[0].field, "name");
    assert_eq!(d.planets_missing, vec![named("p-3", "Moon")]);
    assert_eq!(d.planets_unknown, vec![named("p-out", "Elsewhere")]);
    assert_eq!(d.headline(), "3 cards and 3 planets differ from the saved history");
    assert_eq!(
        d.counts(),
        json!({"cards": 1, "missing": 1, "unknown": 1, "planets": 1, "planetsMissing": 1, "planetsUnknown": 1})
    );

    let one = Drift {
        missing: vec![named("c-b", "Beta")],
        ..Drift::default()
    };
    assert!(one.why().starts_with("1 card differs from the saved history (for example \"Beta\"). "));
}

/// The 2026-10-13 shape: 40 cards lose their links to a writer outside
/// Orbitty, with no history lines.
#[test]
fn incident_forty_cards_lose_links_and_repair_restores_them() {
    let mut start = PersistedState::default();
    start.projects.push(planet("p-1", "Home"));
    for i in 0..60 {
        let mut c = card(&format!("c-{i:02}"), &format!("Card {i:02}"), "p-1");
        if i < 40 {
            c.links = Some(vec![
                CardLink {
                    url: format!("https://example.com/{i}"),
                    label: Some(format!("doc {i}")),
                },
                CardLink {
                    url: format!("file:///tmp/notes-{i}.md"),
                    label: None,
                },
            ]);
        }
        start.kanban_cards.push(c);
    }
    let run = Run::new("incident", &start);
    run.checkpoint();
    run.step(|st| card_mut(st, "c-03").notes = Some("edited in the app".into()));
    run.step(|st| card_mut(st, "c-50").status = "doing".into());
    let links = |st: &PersistedState| -> Vec<String> {
        st.kanban_cards
            .iter()
            .map(|c| serde_json::to_string(&c.links).unwrap())
            .collect()
    };
    let before = run.st();
    let before_view = view_bytes(&run.board());
    let n = run.lines().len();

    outside_write(&run, |st| {
        for c in &mut st.kanban_cards {
            c.links = None;
        }
    });
    assert_eq!(run.lines().len(), n, "the outside writer leaves no lines");

    let d = drift_of(&run);
    let ids: Vec<String> = d.cards.iter().map(|c| c.id.clone()).collect();
    assert_eq!(ids, (0..40).map(|i| format!("c-{i:02}")).collect::<Vec<_>>());
    for c in &d.cards {
        assert_eq!(c.fields.len(), 1);
        assert_eq!((c.fields[0].field.as_str(), &c.fields[0].now), ("links", &json!(null)));
    }
    assert!(d.missing.is_empty() && d.unknown.is_empty());
    assert!(d.planets.is_empty() && d.planets_missing.is_empty() && d.planets_unknown.is_empty());
    assert_eq!(DRIFT_TITLE, "Board differs from its history");
    assert_eq!(
        d.why(),
        "40 cards differ from the saved history (for example \"Card 00\", \"Card 01\", \
         \"Card 02\": links). Something outside Orbitty, or an older Orbitty, wrote the board. \
         Open Settings → History to repair it or keep it as it is."
    );

    let plan = apply_run(&run, &Req::Repair).unwrap();
    assert_eq!(plan.cards.len(), 40);
    assert!(plan.revive.is_empty() && plan.added.is_empty() && plan.skipped.is_empty());
    assert_eq!(links(&run.st()), links(&before), "links back byte-equal");
    assert_eq!(view_bytes(&run.board()), before_view);
    assert!(drift_of(&run).is_empty(), "the next drift is empty");

    let lines = run.lines();
    let new = &lines[n..];
    assert_eq!(new[0].content["reason"], "pre-repair");
    assert_eq!(new.len(), 41);
    assert!(new.iter().all(|l| l.actor == "repair"));
    assert!(new[1..].iter().all(|l| l.content["fields"].as_object().unwrap().keys().eq(["links"])));
}

#[test]
fn keep_makes_the_board_the_history() {
    let run = Run::new("keep", &start_state());
    run.checkpoint();
    outside_write(&run, |st| card_mut(st, "c-a").notes = None);
    assert_eq!(drift_of(&run).cards.len(), 1);
    let st = run.st();
    write_checkpoint_in(&run.dir, &run.events, &st, "user", Some(KEEP), &unsigned).unwrap();
    assert!(drift_of(&run).is_empty());
    assert_eq!(run.lines().last().unwrap().content["reason"], "keep");
    assert!(last_rewind(&run.lines()).is_none(), "keep is not a rewind to undo");
}

#[test]
fn launch_checkpoints_only_a_board_that_matches() {
    let fx = big_fixture("launch", 30, 3, AUTO_CHECKPOINT_LINES + 1);
    let st = load_path(&fx.state).unwrap();
    let cps = |fx: &Fixture| read_lines(&fx.events).unwrap().iter().filter(|l| l.kind == KIND_CHECKPOINT).count();
    let (d, wrote) = launch_in(&st, &fx.dir, &fx.events, &unsigned).unwrap();
    assert!(d.is_empty() && wrote);
    assert_eq!(cps(&fx), 2);
    let (_, wrote) = launch_in(&st, &fx.dir, &fx.events, &unsigned).unwrap();
    assert!(!wrote, "no lines follow the new one");

    let fx = big_fixture("launch-drift", 30, 3, AUTO_CHECKPOINT_LINES + 1);
    let mut st = load_path(&fx.state).unwrap();
    st.kanban_cards[0].title = "Changed outside".into();
    let (d, wrote) = launch_in(&st, &fx.dir, &fx.events, &unsigned).unwrap();
    assert_eq!((d.cards.len(), wrote), (1, false));
    assert_eq!(cps(&fx), 1, "a board that differs is never kept without asking");
    let _ = std::fs::remove_dir_all(&fx.dir);
}

#[test]
fn drift_closes_its_card_quietly_and_runs_after_setup() {
    let board = include_str!("../board/board.rs");
    let start = board.find("pub fn needed_close_quiet(").unwrap();
    let body = &board[start..start + board[start..].find("\n}\n").unwrap()];
    assert!(!body.contains("needed-answer"), "no clerk is dispatched");
    assert!(body.contains("with_actor(\"app\", || save_with_events("));
    let me = include_str!("rewind.rs");
    for (sig, answer) in [("pub(crate) fn repair(", "REPAIRED"), ("pub(crate) fn keep(", "KEPT")] {
        let start = me.find(sig).unwrap();
        let body = &me[start..start + me[start..].find("\n}\n").unwrap()];
        assert!(body.contains(&format!("needed_close_quiet(app, DRIFT_TITLE, {answer})")), "{sig}");
    }
    assert!(me.contains("const REPAIRED: &str = \"Repaired from history\";"));
    assert!(me.contains("const KEPT: &str = \"Kept the board as it is\";"));
    let launch = &me[me.find("pub(crate) fn launch(").unwrap()..];
    assert!(launch.contains("std::thread::Builder::new()"));
    let on = &me[me.find("fn launch_on(").unwrap()..];
    let unlocked = on.find("crate::state::load_state(app)?").unwrap();
    assert!(unlocked < on.find("crate::state::read_locked(").unwrap(), "first pass is lock-free");
    let lib = include_str!("../lib.rs");
    let setup = &lib[lib.find(".setup(|app| {").unwrap()..];
    let boot = setup.find("history::boot_checkpoint(app.handle())").unwrap();
    assert!(boot < setup.find("crate::relay::rewind::launch(app.handle());").unwrap());
}

/// Gate: the launch check on the 5,000-line fixture, 650 cards.
/// `cargo test --lib drift_timing -- --ignored --nocapture`.
#[test]
#[ignore]
fn drift_timing_5k_lines() {
    let fx = big_fixture("drift-timing", 650, 19, 5_000);
    let ms = time(15, || {
        let st = load_path(&fx.state).unwrap();
        let (d, _) = launch_in(&st, &fx.dir, &fx.events, &unsigned).unwrap();
        assert!(d.is_empty());
    });
    println!("launch drift check, 5,000 lines: median {ms:.2}ms");
    assert!(ms <= 300.0, "{ms}");
    let _ = std::fs::remove_dir_all(&fx.dir);
}

#[test]
fn rows_replay_old_values_and_fold_one_save() {
    let run = Run::new("rows", &start_state());
    run.checkpoint();
    run.step(|st| card_mut(st, "c-a").title = "Alpha 2".into());
    run.step(|st| {
        card_mut(st, "c-a").links = None;
        card_mut(st, "c-b").notes = Some("beta notes".into());
    });
    run.step(|st| st.kanban_cards.retain(|c| c.id != "c-b"));
    run.step(|st| st.projects[1].name = "Red".into());
    let mut lines = run.lines();
    assert_eq!(lines.len(), 6);
    for (l, at) in lines.iter_mut().zip([100, 101, 102, 102, 103, 104]) {
        l.created_at = at;
    }
    let page = rows(&run.checkpoints(), &lines, None, 50);
    let what: Vec<Vec<&str>> = page
        .rows
        .iter()
        .map(|r| r.events.iter().map(|e| e.what).collect())
        .collect();
    assert_eq!(what, [vec!["planet"], vec!["deleted"], vec!["card", "card"], vec!["card"], vec!["checkpoint"]]);
    let r = &page.rows;
    assert_eq!(
        r[0].events[0].changes,
        vec![FieldChange { field: "name".into(), from: json!("Mars"), to: json!("Red") }]
    );
    assert_eq!(r[1].events[0].title, "Beta");
    assert_eq!(r[2].id, lines[3].id, "a folded row rewinds past its whole save");
    assert_eq!(r[2].events[0].changes[0].field, "links");
    assert_eq!(r[2].events[1].changes[0].field, "notes");
    assert_eq!(
        r[3].events[0].changes,
        vec![FieldChange { field: "title".into(), from: json!("Alpha"), to: json!("Alpha 2") }]
    );
    assert_eq!((r[3].events[0].title.as_str(), r[3].actor.as_str()), ("Alpha 2", "app"));
    assert!(!page.more && !page.can_undo);

    let short = rows(&run.checkpoints(), &lines, None, 2);
    assert_eq!((short.rows.len(), short.more), (2, true));
    let one = rows(&run.checkpoints(), &lines, Some("c-a"), 50);
    assert_eq!(one.rows.len(), 2);
    assert!(one.rows.iter().all(|r| r.events.iter().all(|e| e.target == "c-a")));
}

#[test]
fn rows_start_over_at_a_keep_after_an_outside_write() {
    let run = Run::new("rows-keep", &start_state());
    run.checkpoint();
    run.checkpoint();
    outside_write(&run, |st| card_mut(st, "c-a").title = "Outside".into());
    let st = run.st();
    write_checkpoint_in(&run.dir, &run.events, &st, "user", Some(KEEP), &unsigned).unwrap();
    run.step(|st| card_mut(st, "c-a").title = "After".into());
    let lines = run.lines();
    assert_eq!(row_checkpoints(&run.dir, &lines).len(), 2, "the first and the keep");
    let page = rows(&row_checkpoints(&run.dir, &lines), &lines, Some("c-a"), 50);
    assert_eq!(page.rows[0].events[0].changes[0].from, json!("Outside"));

    apply_run(&run, &rewind_to(&lines[0].id, None, false)).unwrap();
    let page = rows(&run.checkpoints(), &run.lines(), None, 50);
    assert!(page.can_undo);
    assert_eq!(page.rows[0].actor, "rewind");
    assert_eq!(page.rows[1].events[0].reason.as_deref(), Some("pre-rewind"));
}
