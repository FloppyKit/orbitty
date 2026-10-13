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
    write_checkpoint_in(&dir, &events, &st, "app", &unsigned).unwrap();
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
        write_checkpoint_in(&self.dir, &self.events, &st, "app", &unsigned).unwrap()
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
