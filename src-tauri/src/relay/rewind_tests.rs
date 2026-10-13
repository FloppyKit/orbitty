use crate::relay::history::*;
use crate::state::{load_path, save_path, CardLink, KanbanCard, PersistedState, Project, Subtask};
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
